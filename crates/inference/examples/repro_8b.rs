//! DIAGNOSTIC (not shipped): exercise the model-download path in isolation, including the
//! lock-contention backoff added for the concurrent-instance race that made the Qwen3-VL-8B
//! "Quality" vision download fail (~5 s `LockAcquisition` + retry storm).
//!
//!   cargo run -p inference --example repro_8b
//!
//! Run two copies against the SAME cache to reproduce/verify the contention handling:
//!   REPRO_CACHE=<shared> cargo run -p inference --example repro_8b   # x2 concurrently
//! The loser now logs "backing off and retrying" instead of failing instantly.
//!
//! Env:
//!   REPRO_CACHE=<dir>   cache dir (resume aware); default = fresh temp.
//!   REPRO_REPO=<id>     default = Qwen/Qwen3-VL-8B-Instruct-GGUF
//!   REPRO_FILE=<name>   default = Qwen3VL-8B-Instruct-Q4_K_M.gguf

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let cache = std::env::var("REPRO_CACHE").unwrap_or_else(|_| {
        std::env::temp_dir()
            .join("ssv2c-repro-cache")
            .to_string_lossy()
            .into_owned()
    });
    let repo = std::env::var("REPRO_REPO")
        .unwrap_or_else(|_| "Qwen/Qwen3-VL-8B-Instruct-GGUF".to_string());
    let file = std::env::var("REPRO_FILE")
        .unwrap_or_else(|_| "Qwen3VL-8B-Instruct-Q4_K_M.gguf".to_string());
    println!("repo={repo}\nfile={file}\ncache={cache}\n");

    let t = std::time::Instant::now();
    match inference::download::download_file_with_lock_retry_for_diagnostics(
        std::path::Path::new(&cache),
        &repo,
        &file,
    )
    .await
    {
        Ok(path) => println!("\nOK in {:?}: {}", t.elapsed(), path.display()),
        Err(e) => {
            // anyhow's `{:?}` prints the full context chain (download {file} -> root cause).
            println!("\nERR in {:?}\nDisplay: {e}\nChain:   {e:?}", t.elapsed());
            std::process::exit(1);
        }
    }
}
