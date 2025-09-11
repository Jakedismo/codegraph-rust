use std::fs::File;
use std::io::Result as IoResult;
use std::path::{Path, PathBuf};

/// Cross-platform, read-only memory-mapped file with page advice helpers.
///
/// Goals:
/// - Minimize memory footprint by avoiding copies
/// - Reduce page faults via OS advice (sequential/random, will-need)
/// - Allow dropping pages after processing to keep RSS low
pub struct MappedFile {
    #[allow(dead_code)]
    file: File,
    mmap: memmap2::Mmap,
    len: usize,
    path: PathBuf,
}

impl std::fmt::Debug for MappedFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MappedFile")
            .field("path", &self.path)
            .field("len", &self.len)
            .finish()
    }
}

impl MappedFile {
    /// Open a file and map it read-only. Returns an empty map for zero-length files.
    pub fn open_readonly(path: impl AsRef<Path>) -> IoResult<Self> {
        let p = path.as_ref().to_path_buf();
        let file = File::open(&p)?;
        let meta = file.metadata()?;
        let len = meta.len() as usize;
        let mmap = unsafe { memmap2::MmapOptions::new().map(&file)? };
        Ok(Self {
            file,
            mmap,
            len,
            path: p,
        })
    }

    /// Raw bytes of the mapped file.
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.mmap
    }

    /// File length in bytes.
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// True if file is zero-length.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Hint: access pattern is sequential.
    pub fn advise_sequential(&self) {
        advise_sequential_impl(&self)
    }

    /// Hint: access pattern is random.
    pub fn advise_random(&self) {
        advise_random_impl(&self)
    }

    /// Hint: the given range will be needed soon. Offset+len are clamped to file size.
    pub fn will_need_range(&self, offset: usize, len: usize) {
        will_need_range_impl(&self, offset, len)
    }

    /// Advise the OS that we no longer need pages in the given range.
    /// Useful after processing a chunk to keep RSS low.
    pub fn dont_need_range(&self, offset: usize, len: usize) {
        dont_need_range_impl(&self, offset, len)
    }

    /// Attempt to prefetch the given range into memory. Best-effort.
    pub fn prefetch_range(&self, offset: usize, len: usize) {
        prefetch_range_impl(&self, offset, len)
    }
}

#[cfg(unix)]
fn clamp_range(len: usize, offset: usize, size: usize) -> (usize, usize) {
    if size == 0 {
        return (0, 0);
    }
    let o = offset.min(size);
    let end = o.saturating_add(len).min(size);
    let l = end.saturating_sub(o);
    (o, l)
}

#[cfg(unix)]
fn slice_ptr_len(bytes: &[u8]) -> (*mut libc::c_void, usize) {
    let ptr = bytes.as_ptr() as *mut libc::c_void;
    let len = bytes.len();
    (ptr, len)
}

#[cfg(unix)]
fn advise_sequential_impl(m: &MappedFile) {
    // Use memmap2 global advise, fall back to libc::madvise for range control if needed
    let _ = m.mmap.advise(memmap2::Advice::Sequential);
}

#[cfg(windows)]
fn advise_sequential_impl(_m: &MappedFile) {
    // No-op on Windows; use prefetch_range for similar effect
}

#[cfg(unix)]
fn advise_random_impl(m: &MappedFile) {
    let _ = m.mmap.advise(memmap2::Advice::Random);
}

#[cfg(windows)]
fn advise_random_impl(_m: &MappedFile) {
    // No-op
}

#[cfg(unix)]
fn will_need_range_impl(m: &MappedFile, offset: usize, len: usize) {
    // Best-effort range hint
    let (o, l) = clamp_range(len, offset, m.len);
    if l == 0 {
        return;
    }
    let bytes = &m.mmap[o..o + l];
    let (ptr, len) = slice_ptr_len(bytes);
    unsafe {
        let _ = libc::madvise(ptr, len, libc::MADV_WILLNEED);
    }
}

#[cfg(windows)]
fn will_need_range_impl(m: &MappedFile, offset: usize, len: usize) {
    prefetch_range_impl(m, offset, len)
}

#[cfg(unix)]
fn dont_need_range_impl(m: &MappedFile, offset: usize, len: usize) {
    let (o, l) = clamp_range(len, offset, m.len);
    if l == 0 {
        return;
    }
    let bytes = &m.mmap[o..o + l];
    let (ptr, len) = slice_ptr_len(bytes);
    unsafe {
        let _ = libc::madvise(ptr, len, libc::MADV_DONTNEED);
    }
}

#[cfg(windows)]
fn dont_need_range_impl(_m: &MappedFile, _offset: usize, _len: usize) {
    // No-op; Windows has DiscardVirtualMemory but not universally available/safe here.
}

#[cfg(unix)]
fn prefetch_range_impl(m: &MappedFile, offset: usize, len: usize) {
    // On Unix, use WILLNEED + optional read touch (best-effort)
    will_need_range_impl(m, offset, len)
}

#[cfg(windows)]
fn prefetch_range_impl(m: &MappedFile, offset: usize, len: usize) {
    use core::mem::size_of;
    use windows_sys::Win32::System::Memory::{PrefetchVirtualMemory, _WIN32_MEMORY_RANGE_ENTRY};

    let end = offset.saturating_add(len).min(m.len);
    if end <= offset {
        return;
    }
    let ptr = unsafe { m.mmap.as_ptr().add(offset) } as *mut core::ffi::c_void;
    let bytes = end - offset;

    let mut range = _WIN32_MEMORY_RANGE_ENTRY {
        VirtualAddress: ptr,
        NumberOfBytes: bytes,
    };
    unsafe {
        // Best-effort; ignore failure.
        let _ = PrefetchVirtualMemory(0, 1, &mut range as *mut _ as *mut _, 0);
    }
}

// No additional imports

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mmap_open_and_read() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("sample.txt");
        std::fs::write(&path, b"abcdef\n12345").unwrap();

        let mm = MappedFile::open_readonly(&path).unwrap();
        assert_eq!(mm.len(), 12);
        assert_eq!(&mm.as_bytes()[..6], b"abcdef");

        // Advice calls should be no-op or succeed across platforms
        mm.advise_sequential();
        mm.will_need_range(0, 6);
        mm.prefetch_range(6, 6);
        mm.dont_need_range(0, 6);
        mm.advise_random();
    }
}
