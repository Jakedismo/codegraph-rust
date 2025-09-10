use std::path::Path;
use std::io;
use codegraph_core::MappedFile;

/// High-performance file reading helpers with optional io_uring acceleration on Linux.
///
/// The API returns a full `String` for compatibility with existing parser code.
/// Internally it selects the best available strategy:
/// - Linux + feature `io-uring`: read using tokio-uring
/// - Otherwise: fallback to `tokio::fs::read_to_string`
pub async fn read_file_to_string(path: &str) -> io::Result<String> {
    let p = Path::new(path);

    // Choose strategy based on file size and platform features
    let file_len = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
    const SMALL_THRESHOLD: u64 = 256 * 1024; // 256 KiB
    const LARGE_THRESHOLD: u64 = 4 * 1024 * 1024; // 4 MiB

    // io_uring (Linux only, optional)
    #[cfg(all(feature = "io-uring", target_os = "linux"))]
    {
        // For smaller files, io_uring can be faster than mapping due to
        // reduced per-op overhead and better batching. Prefer it below threshold.
        if file_len > 0 && file_len <= SMALL_THRESHOLD {
            // Run the tokio-uring runtime on a blocking thread to avoid
            // interfering with the main Tokio executor. This performs an async
            // io_uring read and returns the file contents as a Vec<u8>.
            let path_owned = p.to_path_buf();
            let bytes = tokio::task::spawn_blocking(move || -> io::Result<Vec<u8>> {
                tokio_uring::start(async move {
                    use tokio_uring::fs::File;
                    let file = File::open(&path_owned).await?;
                    let meta = std::fs::metadata(&path_owned)?;
                    let mut remaining = meta.len() as usize;
                    let mut offset: u64 = 0;
                    let mut out: Vec<u8> = Vec::with_capacity(remaining);
                    const CHUNK: usize = 1 << 20; // 1 MiB
                    while remaining > 0 {
                        let to_read = remaining.min(CHUNK);
                        let buf = vec![0u8; to_read];
                        let (res, buf) = file.read_at(buf, offset).await;
                        let n = res?;
                        if n == 0 { break; }
                        out.extend_from_slice(&buf[..n]);
                        remaining = remaining.saturating_sub(n);
                        offset += n as u64;
                    }
                    Ok::<Vec<u8>, io::Error>(out)
                })
            })
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("join error: {e}")))??;

            return Ok(String::from_utf8_lossy(&bytes).into_owned());
        }
    }

    // For larger files, try memory mapping to avoid explicit reads
    if file_len >= LARGE_THRESHOLD {
        let path_owned = p.to_path_buf();
        if let Ok(mapped) = tokio::task::spawn_blocking(move || -> io::Result<String> {
            let mm = MappedFile::open_readonly(&path_owned)?;
            // Hint OS for sequential read; prefetch first window
            mm.advise_sequential();
            let prefetch = (mm.len()).min(2 * 1024 * 1024);
            mm.prefetch_range(0, prefetch);
            Ok(String::from_utf8_lossy(mm.as_bytes()).into_owned())
        })
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("join error: {e}")))
        {
            return mapped;
        }
    }

    // Fallback: standard async file read
    #[allow(unreachable_code)]
    {
        tokio::fs::read_to_string(p).await
    }
}
