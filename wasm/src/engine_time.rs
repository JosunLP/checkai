//! WebAssembly clock shim for the shared search engine.
//!
//! `std::time::Instant` and `std::time::SystemTime` panic on
//! `wasm32-unknown-unknown`. `web_time` provides drop-in replacements backed
//! by the browser's `performance.now()` and `Date.now()`, so the search
//! source file itself needs no `cfg` and stays byte-identical with the native
//! crate. See `src/engine_time.rs` for the native counterpart.

pub use web_time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Milliseconds since the Unix epoch, or `0` if the clock is unavailable.
pub fn epoch_nanos() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

/// WebAssembly has no usable `std::thread::spawn`, so Lazy SMP stays off and
/// the engine always searches on the calling thread.
pub const THREADS_SUPPORTED: bool = false;
