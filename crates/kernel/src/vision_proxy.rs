//! Internal JPEG proxy images for vision jobs.
//!
//! Capture storage remains lossless WebP, but vision dispatch does not need to repeatedly
//! decode a native-resolution WebP only to downscale for the VLM. The capture loop writes a
//! bounded, asynchronous `.vision.jpg` beside each WebP; the worker pool lazily creates it if
//! an older frame has no proxy yet.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use image::{ExtendedColorType, ImageEncoder, RgbaImage};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

const VISION_PROXY_MAX_EDGE: u32 = 1280;
const VISION_PROXY_JPEG_QUALITY: u8 = 80;
const VISION_PROXY_QUEUE_CAP: usize = 8;

pub(crate) struct VisionProxyWriter {
    sender: mpsc::Sender<ProxyJob>,
    join: JoinHandle<()>,
}

struct ProxyJob {
    pixels: Arc<RgbaImage>,
    proxy_path: PathBuf,
    max_edge: u32,
}

impl VisionProxyWriter {
    pub(crate) fn spawn() -> Self {
        let (sender, receiver) = mpsc::channel(VISION_PROXY_QUEUE_CAP);
        let join = tokio::spawn(proxy_writer_loop(receiver));
        Self { sender, join }
    }

    pub(crate) fn try_enqueue(
        &self,
        pixels: Arc<RgbaImage>,
        stored_path: PathBuf,
        storage_max_width: u32,
    ) {
        let Some(proxy_path) = proxy_path_for(&stored_path) else {
            return;
        };
        let job = ProxyJob {
            pixels,
            proxy_path,
            max_edge: proxy_max_edge(storage_max_width),
        };
        if let Err(e) = self.sender.try_send(job) {
            match e {
                mpsc::error::TrySendError::Full(job) => tracing::warn!(
                    path = %job.proxy_path.display(),
                    "vision proxy writer queue full; worker will create proxy on demand"
                ),
                mpsc::error::TrySendError::Closed(job) => tracing::warn!(
                    path = %job.proxy_path.display(),
                    "vision proxy writer stopped before enqueue"
                ),
            }
        }
    }

    pub(crate) async fn shutdown(self) {
        let Self { sender, join } = self;
        drop(sender);
        if let Err(e) = join.await {
            tracing::warn!(error = %e, "vision proxy writer join failed");
        }
    }
}

async fn proxy_writer_loop(mut receiver: mpsc::Receiver<ProxyJob>) {
    while let Some(job) = receiver.recv().await {
        let proxy_path = job.proxy_path.clone();
        let result = tokio::task::spawn_blocking(move || {
            let proxy = proxy_image(&job.pixels, job.max_edge);
            write_proxy_jpeg(&proxy, &job.proxy_path)
        })
        .await;
        match result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => tracing::warn!(
                path = %proxy_path.display(),
                error = %e,
                "vision proxy writer failed"
            ),
            Err(e) => tracing::warn!(
                path = %proxy_path.display(),
                error = %e,
                "vision proxy writer task failed"
            ),
        }
    }
}

pub(crate) async fn load_for_vision(path: PathBuf) -> Result<RgbaImage> {
    let Some(proxy_path) = proxy_path_for(&path) else {
        return decode_rgba(path).await;
    };
    if let Err(e) = tokio::fs::metadata(&path).await {
        return Err(e).with_context(|| format!("source image missing {}", path.display()));
    }
    if tokio::fs::metadata(&proxy_path).await.is_ok() {
        match decode_rgba(proxy_path.clone()).await {
            Ok(image) => return Ok(image),
            Err(e) => {
                tracing::warn!(
                    path = %proxy_path.display(),
                    error = %e,
                    "vision proxy decode failed; rebuilding from WebP"
                );
                let _ = tokio::fs::remove_file(&proxy_path).await;
            }
        }
    }
    tokio::task::spawn_blocking(move || create_proxy_from_file(&path, &proxy_path))
        .await
        .map_err(|e| anyhow!("vision proxy task failed: {e}"))?
}

pub(crate) fn proxy_path_for(path: &Path) -> Option<PathBuf> {
    if !path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("webp"))
    {
        return None;
    }
    let mut file_name = path.file_stem()?.to_os_string();
    file_name.push(".vision.jpg");
    Some(path.with_file_name(file_name))
}

async fn decode_rgba(path: PathBuf) -> Result<RgbaImage> {
    tokio::task::spawn_blocking(move || {
        image::open(&path)
            .map(|img| img.to_rgba8())
            .with_context(|| format!("decode image {}", path.display()))
    })
    .await
    .map_err(|e| anyhow!("image decode task failed: {e}"))?
}

fn create_proxy_from_file(source_path: &Path, proxy_path: &Path) -> Result<RgbaImage> {
    let image = image::open(source_path)
        .with_context(|| format!("decode WebP {}", source_path.display()))?
        .to_rgba8();
    let proxy = proxy_image(&image, VISION_PROXY_MAX_EDGE);
    write_proxy_jpeg(&proxy, proxy_path)?;
    Ok(proxy)
}

fn proxy_max_edge(storage_max_width: u32) -> u32 {
    if storage_max_width == 0 {
        VISION_PROXY_MAX_EDGE
    } else {
        storage_max_width.min(VISION_PROXY_MAX_EDGE)
    }
}

fn proxy_image(pixels: &RgbaImage, target_max_edge: u32) -> RgbaImage {
    let source_max_edge = pixels.width().max(pixels.height());
    if source_max_edge <= target_max_edge {
        return pixels.clone();
    }
    let ratio = f64::from(target_max_edge) / f64::from(source_max_edge);
    let new_w = ((f64::from(pixels.width()) * ratio).round() as u32).max(1);
    let new_h = ((f64::from(pixels.height()) * ratio).round() as u32).max(1);
    image::imageops::resize(pixels, new_w, new_h, image::imageops::FilterType::Triangle)
}

fn write_proxy_jpeg(proxy: &RgbaImage, proxy_path: &Path) -> Result<()> {
    if proxy_path.exists() {
        return Ok(());
    }
    if let Some(parent) = proxy_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create proxy dir {}", parent.display()))?;
    }
    let tmp_path = temp_path_for(proxy_path)?;
    let rgb = image::DynamicImage::ImageRgba8(proxy.clone()).into_rgb8();
    {
        let file = std::io::BufWriter::new(
            std::fs::File::create(&tmp_path)
                .with_context(|| format!("create proxy temp {}", tmp_path.display()))?,
        );
        image::codecs::jpeg::JpegEncoder::new_with_quality(file, VISION_PROXY_JPEG_QUALITY)
            .write_image(
                rgb.as_raw(),
                rgb.width(),
                rgb.height(),
                ExtendedColorType::Rgb8,
            )
            .with_context(|| format!("encode proxy {}", tmp_path.display()))?;
    }
    match std::fs::rename(&tmp_path, proxy_path) {
        Ok(()) => Ok(()),
        Err(e) if proxy_path.exists() => {
            let _ = std::fs::remove_file(&tmp_path);
            tracing::debug!(
                path = %proxy_path.display(),
                error = %e,
                "vision proxy appeared before rename"
            );
            Ok(())
        }
        Err(e) => Err(e).with_context(|| {
            format!(
                "publish proxy {} from {}",
                proxy_path.display(),
                tmp_path.display()
            )
        }),
    }
}

fn temp_path_for(path: &Path) -> Result<PathBuf> {
    let mut file_name = path
        .file_name()
        .context("proxy path has no file name")?
        .to_os_string();
    file_name.push(".tmp");
    Ok(path.with_file_name(file_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn webp_load_creates_downscaled_jpeg_proxy() {
        let tmp = tempfile::tempdir().unwrap();
        let webp = tmp.path().join("frame.webp");
        let rgba = RgbaImage::from_pixel(1600, 800, image::Rgba([10, 20, 30, 255]));
        let rgb = image::DynamicImage::ImageRgba8(rgba).into_rgb8();
        {
            let file = std::io::BufWriter::new(std::fs::File::create(&webp).unwrap());
            image::codecs::webp::WebPEncoder::new_lossless(file)
                .write_image(
                    rgb.as_raw(),
                    rgb.width(),
                    rgb.height(),
                    ExtendedColorType::Rgb8,
                )
                .unwrap();
        }

        let loaded = load_for_vision(webp.clone()).await.unwrap();

        assert_eq!((loaded.width(), loaded.height()), (1280, 640));
        let proxy = proxy_path_for(&webp).unwrap();
        assert!(proxy.exists());
        let proxy_image = image::open(&proxy).unwrap();
        assert_eq!((proxy_image.width(), proxy_image.height()), (1280, 640));
    }

    #[tokio::test]
    async fn existing_proxy_serves_without_decoding_webp_source() {
        let tmp = tempfile::tempdir().unwrap();
        let webp = tmp.path().join("frame.webp");
        let proxy = proxy_path_for(&webp).unwrap();
        std::fs::write(&webp, b"not a valid webp").unwrap();
        let rgba = RgbaImage::from_pixel(320, 200, image::Rgba([40, 50, 60, 255]));
        write_proxy_jpeg(&rgba, &proxy).unwrap();

        let loaded = load_for_vision(webp).await.unwrap();

        assert_eq!((loaded.width(), loaded.height()), (320, 200));
    }

    #[tokio::test]
    async fn queued_proxy_respects_storage_max_width_below_proxy_cap() {
        let tmp = tempfile::tempdir().unwrap();
        let webp = tmp.path().join("frame.webp");
        let proxy = proxy_path_for(&webp).unwrap();
        let writer = VisionProxyWriter::spawn();
        let rgba = Arc::new(RgbaImage::from_pixel(
            1600,
            800,
            image::Rgba([70, 80, 90, 255]),
        ));

        writer.try_enqueue(rgba, webp, 640);
        writer.shutdown().await;

        let proxy_image = image::open(&proxy).unwrap();
        assert_eq!((proxy_image.width(), proxy_image.height()), (640, 320));
    }
}
