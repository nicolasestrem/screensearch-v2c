//! The WGC capture worker (`03 §3`): a dedicated COM-initialized thread that owns a
//! per-monitor D3D11 device + Windows.Graphics.Capture session, and on each request
//! reads back the latest frame, applies the diff gate, and returns the *changed*
//! frames. Living on one thread keeps the (non-thread-safe) D3D11 device contexts
//! correctly serialized (`01 §6`).

use std::sync::mpsc;
use std::time::Duration;

use anyhow::{bail, Result};
use image::RgbaImage;
use tokio::sync::oneshot;
use traits::{CaptureConfig, MonitorInfo};

use windows::core::Interface;
use windows::Graphics::Capture::{
    Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession,
};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D, D3D11_CPU_ACCESS_READ,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ, D3D11_SDK_VERSION,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;

use crate::diff::{self, Fingerprint};
use crate::monitors::{self, EnumeratedMonitor};

/// A captured, changed frame's pixels (RGBA8) plus metadata, handed back to the
/// async side which wraps it into a `CapturedFrame`.
pub struct FrameData {
    pub monitor_index: u32,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub content_hash: String,
}

/// One "capture now" request; the worker replies with the changed frames.
pub struct CaptureRequest {
    pub resp: oneshot::Sender<Result<Vec<FrameData>>>,
}

/// Worker thread entry point. Initializes COM, sets up per-monitor capture, reports
/// the monitor list back via `ready`, then services requests until the channel
/// closes.
pub fn worker_main(
    config: CaptureConfig,
    req_rx: mpsc::Receiver<CaptureRequest>,
    ready: mpsc::Sender<Result<Vec<MonitorInfo>>>,
) {
    // SAFETY: WGC/D3D interop must run on a COM-initialized thread (`01 §6`).
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };

    let mut monitors = match setup(&config) {
        Ok(m) => m,
        Err(e) => {
            let _ = ready.send(Err(e));
            unsafe { CoUninitialize() };
            return;
        }
    };
    let infos: Vec<MonitorInfo> = monitors.iter().map(|m| m.info.clone()).collect();
    let _ = ready.send(Ok(infos));

    while let Ok(req) = req_rx.recv() {
        let mut changed = Vec::new();
        for ms in monitors.iter_mut() {
            match ms.capture_changed() {
                Ok(Some(fd)) => changed.push(fd),
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!(monitor = ms.info.index, error = %e, "capture: readback failed")
                }
            }
        }
        let _ = req.resp.send(Ok(changed));
    }

    for ms in &monitors {
        let _ = ms.session.Close();
        let _ = ms.pool.Close();
    }
    // SAFETY: pair the CoInitializeEx above.
    unsafe { CoUninitialize() };
}

fn setup(config: &CaptureConfig) -> Result<Vec<MonitorState>> {
    let all = monitors::enumerate();
    if all.is_empty() {
        bail!("no monitors found");
    }
    let selected: Vec<EnumeratedMonitor> = all
        .into_iter()
        .filter(|m| config.monitors.is_empty() || config.monitors.contains(&m.info.index))
        .collect();
    if selected.is_empty() {
        bail!(
            "no monitors match the capture.monitors filter {:?}",
            config.monitors
        );
    }
    selected
        .into_iter()
        .map(|em| MonitorState::new(em, config.diff_threshold))
        .collect()
}

/// Per-monitor capture state: its own D3D11 device/context and WGC session, plus
/// the previous fingerprint for the diff gate.
struct MonitorState {
    info: MonitorInfo,
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    // Kept alive for the session's lifetime (WGC holds it internally).
    _d3d_device: IDirect3DDevice,
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    prev: Option<Fingerprint>,
    diff_threshold: f32,
}

impl MonitorState {
    fn new(em: EnumeratedMonitor, diff_threshold: f32) -> Result<Self> {
        let (device, context) = create_d3d_device()?;
        let d3d_device = create_winrt_device(&device)?;

        let interop: IGraphicsCaptureItemInterop =
            windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
        // SAFETY: standard WGC item creation from an HMONITOR.
        let item: GraphicsCaptureItem = unsafe { interop.CreateForMonitor(em.hmonitor)? };
        let size = item.Size()?;

        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &d3d_device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            2,
            size,
        )?;
        let session = pool.CreateCaptureSession(&item)?;
        // Drop the capture border where supported (Win11+); harmless if it fails.
        let _ = session.SetIsBorderRequired(false);
        session.StartCapture()?;

        Ok(Self {
            info: em.info,
            device,
            context,
            _d3d_device: d3d_device,
            pool,
            session,
            prev: None,
            diff_threshold,
        })
    }

    /// Reads the latest frame and returns it only if it changed past the threshold.
    fn capture_changed(&mut self) -> Result<Option<FrameData>> {
        let Some(img) = self.grab_latest()? else {
            return Ok(None);
        };
        let fp = diff::fingerprint(&img);
        let changed = match &self.prev {
            None => true,
            Some(prev) => diff::difference(prev, &fp) > self.diff_threshold,
        };
        if !changed {
            return Ok(None);
        }
        self.prev = Some(fp);
        Ok(Some(FrameData {
            monitor_index: self.info.index,
            width: img.width(),
            height: img.height(),
            content_hash: diff::content_hash(&img),
            rgba: img.into_raw(),
        }))
    }

    /// Drains the frame pool to the most recent frame and reads it back to RGBA.
    /// Returns `None` if no frame is available this cycle (e.g. just after start).
    fn grab_latest(&self) -> Result<Option<RgbaImage>> {
        let mut kept = None;
        for _ in 0..16 {
            match self.pool.TryGetNextFrame() {
                Ok(frame) => {
                    if let Some(old) = kept.take() {
                        let _ = drop_frame(old);
                    }
                    kept = Some(frame);
                }
                Err(_) => break,
            }
        }
        if kept.is_none() {
            // Cold start: give the pool a moment to produce the first frame.
            std::thread::sleep(Duration::from_millis(50));
            if let Ok(frame) = self.pool.TryGetNextFrame() {
                kept = Some(frame);
            }
        }
        let Some(frame) = kept else {
            return Ok(None);
        };
        // A "no frame" TryGetNextFrame can hand back a surface-less object; treat a
        // failed Surface() as "nothing this cycle" rather than an error.
        let surface = match frame.Surface() {
            Ok(s) => s,
            Err(_) => {
                let _ = drop_frame(frame);
                return Ok(None);
            }
        };
        let access: IDirect3DDxgiInterfaceAccess = surface.cast()?;
        // SAFETY: GetInterface returns the backing D3D11 texture for the surface.
        let texture: ID3D11Texture2D = unsafe { access.GetInterface()? };
        let img = readback(&self.device, &self.context, &texture);
        let _ = drop_frame(frame);
        Ok(Some(img?))
    }
}

/// Closes a capture frame (recycles its pool buffer).
fn drop_frame(frame: windows::Graphics::Capture::Direct3D11CaptureFrame) -> Result<()> {
    frame.Close()?;
    Ok(())
}

fn create_d3d_device() -> Result<(ID3D11Device, ID3D11DeviceContext)> {
    let mut device = None;
    let mut context = None;
    // SAFETY: standard hardware D3D11 device creation; BGRA support is required for
    // WGC interop.
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )?;
    }
    Ok((
        device.ok_or_else(|| anyhow::anyhow!("D3D11CreateDevice returned no device"))?,
        context.ok_or_else(|| anyhow::anyhow!("D3D11CreateDevice returned no context"))?,
    ))
}

fn create_winrt_device(device: &ID3D11Device) -> Result<IDirect3DDevice> {
    let dxgi: IDXGIDevice = device.cast()?;
    // SAFETY: wraps the DXGI device as a WinRT IDirect3DDevice for the frame pool.
    let inspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi)? };
    Ok(inspectable.cast()?)
}

/// Copies a GPU texture to a CPU-readable staging texture, maps it, and converts
/// the BGRA8 rows (with stride padding) into a tight RGBA8 [`RgbaImage`].
fn readback(
    device: &ID3D11Device,
    context: &ID3D11DeviceContext,
    src: &ID3D11Texture2D,
) -> Result<RgbaImage> {
    let mut desc = D3D11_TEXTURE2D_DESC::default();
    // SAFETY: fills `desc` from the source texture.
    unsafe { src.GetDesc(&mut desc) };
    let (w, h) = (desc.Width, desc.Height);

    let staging_desc = D3D11_TEXTURE2D_DESC {
        Width: w,
        Height: h,
        MipLevels: 1,
        ArraySize: 1,
        Format: desc.Format,
        SampleDesc: desc.SampleDesc,
        Usage: D3D11_USAGE_STAGING,
        BindFlags: 0,
        CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
        MiscFlags: 0,
    };

    let mut staging: Option<ID3D11Texture2D> = None;
    // SAFETY: creates the staging texture and copies + maps it for CPU read.
    let img = unsafe {
        device.CreateTexture2D(&staging_desc, None, Some(&mut staging))?;
        let staging = staging.ok_or_else(|| anyhow::anyhow!("no staging texture"))?;
        context.CopyResource(&staging, src);

        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;

        let row_pitch = mapped.RowPitch as usize;
        let row_bytes = (w as usize) * 4;
        let mut rgba = vec![0u8; row_bytes * h as usize];
        for y in 0..h as usize {
            let src_row = std::slice::from_raw_parts(
                (mapped.pData as *const u8).add(y * row_pitch),
                row_bytes,
            );
            let dst_row = &mut rgba[y * row_bytes..(y + 1) * row_bytes];
            for x in 0..w as usize {
                let s = &src_row[x * 4..x * 4 + 4]; // BGRA
                let d = &mut dst_row[x * 4..x * 4 + 4];
                d[0] = s[2];
                d[1] = s[1];
                d[2] = s[0];
                d[3] = s[3];
            }
        }
        context.Unmap(&staging, 0);
        RgbaImage::from_raw(w, h, rgba)
    };

    img.ok_or_else(|| anyhow::anyhow!("rgba buffer did not match {w}x{h}"))
}
