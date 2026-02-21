#![allow(dead_code)]
//! Debug-mode allocation tracker (Step 0.5).
//!
//! When enabled via `--features alloc-tracking` in debug builds,
//! counts per-frame heap allocations and warns if any occur in steady state.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};

/// Per-frame allocation counter.
static ALLOC_COUNT: AtomicU64 = AtomicU64::new(0);

/// Whether tracking is installed.
static INSTALLED: AtomicU64 = AtomicU64::new(0);

/// Tracking allocator that wraps System and counts allocations.
struct TrackingAllocator;

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if INSTALLED.load(Ordering::Relaxed) == 1 {
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if INSTALLED.load(Ordering::Relaxed) == 1 {
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if INSTALLED.load(Ordering::Relaxed) == 1 {
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[cfg(all(debug_assertions, feature = "alloc-tracking"))]
#[global_allocator]
static GLOBAL: TrackingAllocator = TrackingAllocator;

/// Enable allocation tracking (call once at startup).
#[cfg(all(debug_assertions, feature = "alloc-tracking"))]
pub fn install() {
    INSTALLED.store(1, Ordering::SeqCst);
    tracing::info!("Allocation tracking enabled");
}

/// Disable at compile time when feature is off.
#[cfg(not(all(debug_assertions, feature = "alloc-tracking")))]
pub fn install() {
    // No-op in release or when feature is disabled
}

/// Get the number of allocations since last reset.
#[cfg(all(debug_assertions, feature = "alloc-tracking"))]
pub fn frame_alloc_count() -> u64 {
    ALLOC_COUNT.load(Ordering::Relaxed)
}

/// Reset the per-frame counter.
#[cfg(all(debug_assertions, feature = "alloc-tracking"))]
pub fn reset_frame_count() {
    ALLOC_COUNT.store(0, Ordering::Relaxed);
}

// Stubs when feature is disabled
#[cfg(not(all(debug_assertions, feature = "alloc-tracking")))]
pub fn frame_alloc_count() -> u64 { 0 }

#[cfg(not(all(debug_assertions, feature = "alloc-tracking")))]
pub fn reset_frame_count() {}
