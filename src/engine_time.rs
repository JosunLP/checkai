//! Clock types used by the search engine.
//!
//! [`crate::search`] is shared verbatim with the WebAssembly crate through a
//! `#[path]` include, but `std::time::Instant` and `std::time::SystemTime`
//! both panic on `wasm32-unknown-unknown`. Routing every clock access through
//! this module lets each crate supply the implementation that works for it —
//! the native build uses `std::time`, the WASM build swaps in `web_time`,
//! which is backed by the browser's performance clock.
//!
//! Keeping the indirection in one tiny module means the search itself never
//! needs a `cfg` and the two crates cannot drift apart.

pub use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Milliseconds since the Unix epoch, or `0` if the clock is unavailable.
///
/// Used only to seed the engine's pseudo-random generator, so a zero value
/// on an exotic platform costs variety, never correctness.
pub fn epoch_nanos() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

/// `true` when the current target can actually run search threads.
///
/// `wasm32-unknown-unknown` has `std::thread`, but spawning panics at
/// runtime, so Lazy SMP has to stay disabled there.
pub const THREADS_SUPPORTED: bool = !cfg!(target_arch = "wasm32");
