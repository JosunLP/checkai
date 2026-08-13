//! The difficulty ladder: maps a 1–10 level to engine search limits.
//!
//! | Level | Max depth | Move time | TT size | Skill | Feels like        |
//! |-------|-----------|-----------|---------|-------|-------------------|
//! | 1     | 2         | 60 ms     | 4 MB    | 2     | absolute beginner |
//! | 2     | 3         | 120 ms    | 8 MB    | 5     | casual club player|
//! | 3     | 4         | 250 ms    | 16 MB   | 8     | improving amateur |
//! | 4     | 6         | 500 ms    | 32 MB   | 11    | solid club player |
//! | 5     | 10        | 1000 ms   | 64 MB   | 14    | strong club player|
//! | 6     | 14        | 2000 ms   | 64 MB   | 17    | expert            |
//! | 7     | MAX       | 3000 ms   | 128 MB  | full  | master            |
//! | 8     | MAX       | 5000 ms   | 128 MB  | full  | strong master     |
//! | 9     | MAX       | 7500 ms   | 256 MB  | full  | very strong       |
//! | 10    | MAX       | 10000 ms  | 256 MB  | full  | full strength     |
//!
//! Levels are weakened along three axes. A hard depth cap and a small time
//! budget limit *how far* the engine looks; the skill setting additionally
//! makes it pick from a band of near-best moves rather than always the very
//! best one. The third axis matters most for playability: a pure depth cap
//! makes an engine uniformly short-sighted, which feels robotic, while a
//! skill limit produces the occasional human-looking inaccuracy on top of
//! otherwise sensible play. Levels 7 and up drop the skill limit entirely
//! and are paced purely by move time.

use crate::search::{MAX_DEPTH, SearchLimits};

/// Lowest selectable difficulty level.
pub const MIN_LEVEL: u8 = 1;
/// Highest selectable difficulty level.
pub const MAX_LEVEL: u8 = 10;
/// Default difficulty level.
pub const DEFAULT_LEVEL: u8 = 5;
/// Lowest level that plays at full strength (no skill limit).
pub const FULL_STRENGTH_LEVEL: u8 = 7;

/// Ladder rows: `(max_depth, move_time_ms, tt_size_mb, skill)` for levels
/// 1–10. `skill` is `None` for full strength.
const LADDER: [(i32, u64, usize, Option<u8>); 10] = [
    (2, 60, 4, Some(2)),
    (3, 120, 8, Some(5)),
    (4, 250, 16, Some(8)),
    (6, 500, 32, Some(11)),
    (10, 1000, 64, Some(14)),
    (14, 2000, 64, Some(17)),
    (MAX_DEPTH, 3000, 128, None),
    (MAX_DEPTH, 5000, 128, None),
    (MAX_DEPTH, 7500, 256, None),
    (MAX_DEPTH, 10000, 256, None),
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
    /// Artificial strength limit (`None` = full strength).
    pub skill: Option<u8>,
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
        let (depth, movetime, tt, skill) = LADDER[(level - 1) as usize];
        let limits = SearchLimits {
            max_depth: depth_override.unwrap_or(depth).clamp(1, MAX_DEPTH),
            move_time_ms: Some(movetime_override.unwrap_or(movetime)),
            ..SearchLimits::default()
        };
        Self {
            level,
            limits,
            tt_size_mb: tt,
            skill,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_level_1_is_shallow_and_fast() {
        let s = LevelSettings::for_level(1, None, None);
        assert_eq!(s.limits.max_depth, 2);
        assert_eq!(s.limits.move_time_ms, Some(60));
        assert_eq!(s.tt_size_mb, 4);
        assert_eq!(s.skill, Some(2));
    }

    #[test]
    fn test_level_10_is_full_depth_long_time() {
        let s = LevelSettings::for_level(10, None, None);
        assert_eq!(s.limits.max_depth, MAX_DEPTH);
        assert_eq!(s.limits.move_time_ms, Some(10_000));
        assert_eq!(s.tt_size_mb, 256);
        assert_eq!(s.skill, None, "top levels play at full strength");
    }

    #[test]
    fn test_skill_limit_only_below_full_strength_level() {
        for level in MIN_LEVEL..FULL_STRENGTH_LEVEL {
            assert!(
                LevelSettings::for_level(level, None, None).skill.is_some(),
                "level {level} must be skill-limited"
            );
        }
        for level in FULL_STRENGTH_LEVEL..=MAX_LEVEL {
            assert!(
                LevelSettings::for_level(level, None, None).skill.is_none(),
                "level {level} must play at full strength"
            );
        }
    }

    #[test]
    fn test_skill_increases_with_level() {
        let mut previous = 0u8;
        for level in MIN_LEVEL..FULL_STRENGTH_LEVEL {
            let skill = LevelSettings::for_level(level, None, None).skill.unwrap();
            assert!(skill > previous, "level {level} must be stronger");
            previous = skill;
        }
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
