//! The difficulty ladder: maps a 1–10 level to engine search limits.
//!
//! | Level | Max depth | Move time | TT size |
//! |-------|-----------|-----------|---------|
//! | 1     | 1         | 60 ms     | 4 MB    |
//! | 2     | 2         | 120 ms    | 8 MB    |
//! | 3     | 4         | 250 ms    | 16 MB   |
//! | 4     | 6         | 500 ms    | 32 MB   |
//! | 5     | 10        | 1000 ms   | 64 MB   |
//! | 6     | 14        | 2000 ms   | 64 MB   |
//! | 7     | MAX       | 3000 ms   | 128 MB  |
//! | 8     | MAX       | 5000 ms   | 128 MB  |
//! | 9     | MAX       | 7500 ms   | 256 MB  |
//! | 10    | MAX       | 10000 ms  | 256 MB  |
//!
//! Low levels are weakened twice over: a hard depth cap *and* a tiny
//! time budget (whichever hits first), plus a small transposition table.
//! High levels run at full depth and are paced purely by move time.

use crate::search::{MAX_DEPTH, SearchLimits};

/// Lowest selectable difficulty level.
pub const MIN_LEVEL: u8 = 1;
/// Highest selectable difficulty level.
pub const MAX_LEVEL: u8 = 10;
/// Default difficulty level.
pub const DEFAULT_LEVEL: u8 = 5;

/// Ladder rows: `(max_depth, move_time_ms, tt_size_mb)` for levels 1–10.
const LADDER: [(i32, u64, usize); 10] = [
    (1, 60, 4),
    (2, 120, 8),
    (4, 250, 16),
    (6, 500, 32),
    (10, 1000, 64),
    (14, 2000, 64),
    (MAX_DEPTH, 3000, 128),
    (MAX_DEPTH, 5000, 128),
    (MAX_DEPTH, 7500, 256),
    (MAX_DEPTH, 10000, 256),
];

/// Engine configuration derived from a difficulty level.
#[derive(Debug, Clone)]
pub struct LevelSettings {
    /// The (clamped) level this configuration was derived from.
    pub level: u8,
    /// Search limits to pass to `SearchEngine::search_limited`.
    pub limits: SearchLimits,
    /// Transposition table size in MB for the engine instance.
    pub tt_size_mb: usize,
}

impl LevelSettings {
    /// Builds the settings for a level (clamped to `1..=10`).
    ///
    /// `movetime_override` / `depth_override` replace the ladder values
    /// when the user passes explicit `--movetime` / `--depth` flags.
    pub fn for_level(
        level: u8,
        movetime_override: Option<u64>,
        depth_override: Option<i32>,
    ) -> Self {
        let level = level.clamp(MIN_LEVEL, MAX_LEVEL);
        let (depth, movetime, tt) = LADDER[(level - 1) as usize];
        let limits = SearchLimits {
            max_depth: depth_override.unwrap_or(depth).clamp(1, MAX_DEPTH),
            move_time_ms: Some(movetime_override.unwrap_or(movetime)),
            max_nodes: None,
        };
        Self {
            level,
            limits,
            tt_size_mb: tt,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_level_1_is_shallow_and_fast() {
        let s = LevelSettings::for_level(1, None, None);
        assert_eq!(s.limits.max_depth, 1);
        assert_eq!(s.limits.move_time_ms, Some(60));
        assert_eq!(s.tt_size_mb, 4);
    }

    #[test]
    fn test_level_10_is_full_depth_long_time() {
        let s = LevelSettings::for_level(10, None, None);
        assert_eq!(s.limits.max_depth, MAX_DEPTH);
        assert_eq!(s.limits.move_time_ms, Some(10_000));
        assert_eq!(s.tt_size_mb, 256);
    }

    #[test]
    fn test_levels_are_monotonic_in_time() {
        let mut prev = 0u64;
        for level in MIN_LEVEL..=MAX_LEVEL {
            let s = LevelSettings::for_level(level, None, None);
            let mt = s.limits.move_time_ms.unwrap();
            assert!(mt > prev, "level {level} must be slower than {}", level - 1);
            prev = mt;
        }
    }

    #[test]
    fn test_out_of_range_levels_clamp() {
        assert_eq!(LevelSettings::for_level(0, None, None).level, MIN_LEVEL);
        assert_eq!(LevelSettings::for_level(99, None, None).level, MAX_LEVEL);
    }

    #[test]
    fn test_overrides_replace_ladder_values() {
        let s = LevelSettings::for_level(5, Some(42), Some(7));
        assert_eq!(s.limits.move_time_ms, Some(42));
        assert_eq!(s.limits.max_depth, 7);
    }
}
