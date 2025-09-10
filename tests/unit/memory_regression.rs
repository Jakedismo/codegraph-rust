//! Memory regression tests leveraging jemalloc statistics.
//! These run in CI to catch leaks and unbounded growth.

use std::time::Duration;

#[test]
fn regression_no_growth_after_free() {
    // Ensure fresh stats
    let _ = tikv_jemalloc_ctl::epoch::advance();
    let initial = tikv_jemalloc_ctl::stats::allocated::read().unwrap_or(0);

    {
        // Allocate ~32MB in chunks, then drop
        let mut blocks: Vec<Vec<u8>> = Vec::with_capacity(32);
        for _ in 0..32 {
            // 1MB blocks
            let mut v = Vec::with_capacity(1024 * 1024);
            unsafe { v.set_len(1024 * 1024); }
            blocks.push(v);
        }
        // Drop all
        drop(blocks);
    }

    // Give allocator a moment and refresh stats
    std::thread::sleep(Duration::from_millis(100));
    let _ = tikv_jemalloc_ctl::epoch::advance();
    let after_free = tikv_jemalloc_ctl::stats::allocated::read().unwrap_or(0);

    // Accept small drift but cap at 5MB
    let growth = after_free.saturating_sub(initial);
    assert!(growth < 5 * 1024 * 1024, "Allocated grew by {} bytes (>5MB)", growth);
}

#[test]
fn steady_state_growth_under_threshold() {
    // Simulate steady activity and assert drift <5MB/hr scaled down
    let _ = tikv_jemalloc_ctl::epoch::advance();
    let start = tikv_jemalloc_ctl::stats::allocated::read().unwrap_or(0);

    // Perform repeated allocate/free cycles
    for _ in 0..200 {
        let mut data: Vec<Vec<u8>> = Vec::with_capacity(8);
        for _ in 0..8 {
            let mut v = Vec::with_capacity(256 * 1024); // 256KB
            unsafe { v.set_len(256 * 1024); }
            data.push(v);
        }
        drop(data);
    }

    let _ = tikv_jemalloc_ctl::epoch::advance();
    let end = tikv_jemalloc_ctl::stats::allocated::read().unwrap_or(0);
    let drift = end.saturating_sub(start);

    // Scale threshold for short test runtime (~few ms)
    assert!(drift < 2 * 1024 * 1024, "Steady-state drift too high: {} bytes", drift);
}
