//! Chess clocks for the interactive CLI.
//!
//! `checkai play --time 5+3` gives both sides five minutes plus a three
//! second increment; `checkai watch --time 1+0` turns the showcase into a
//! bullet match. The clock is authoritative for the engine's time budget
//! too: [`GameClock::budget_ms`] converts the remaining time into the
//! per-move allowance the search receives, so the engine paces itself
//! exactly like it would under a UCI GUI.

use std::time::{Duration, Instant};

use crate::types::Color;

/// Fraction of the remaining clock spent on a single move (1/25th), matching
/// the UCI time manager in [`crate::cli::uci`].
const CLOCK_DIVISOR: u64 = 25;

/// Never budget less than this per move.
const MIN_BUDGET_MS: u64 = 20;

/// Always keep this much on the clock as a safety reserve.
const RESERVE_MS: u64 = 60;

/// A parsed time control: base time plus per-move increment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeControl {
    /// Starting time per side, in milliseconds.
    pub base_ms: u64,
    /// Increment added after each move, in milliseconds.
    pub increment_ms: u64,
}

impl TimeControl {
    /// Parses a time control string.
    ///
    /// Accepted forms (whitespace is ignored):
    ///
    /// | Input     | Meaning                          |
    /// |-----------|----------------------------------|
    /// | `5`       | 5 minutes, no increment          |
    /// | `5+3`     | 5 minutes + 3 seconds per move    |
    /// | `90+30`   | 90 minutes + 30 seconds          |
    /// | `30s`     | 30 seconds, no increment         |
    /// | `1m+2s`   | 1 minute + 2 seconds             |
    ///
    /// A bare number is minutes for the base and seconds for the increment,
    /// which is how every chess site writes time controls.
    pub fn parse(input: &str) -> Result<Self, String> {
        let cleaned: String = input.chars().filter(|c| !c.is_whitespace()).collect();
        if cleaned.is_empty() {
            return Err("empty time control".to_string());
        }
        let (base_text, increment_text) = match cleaned.split_once('+') {
            Some((base, increment)) => (base, increment),
            None => (cleaned.as_str(), "0"),
        };
        let base_ms = parse_duration(base_text, 60_000)?;
        let increment_ms = parse_duration(increment_text, 1_000)?;
        if base_ms == 0 && increment_ms == 0 {
            return Err("time control must allow at least some time".to_string());
        }
        Ok(Self {
            base_ms,
            increment_ms,
        })
    }

    /// Human-readable form, e.g. `5+3` or `30s`.
    pub fn label(&self) -> String {
        let base = self.base_ms / 1000;
        let increment = self.increment_ms / 1000;
        if base.is_multiple_of(60) && base > 0 {
            format!("{}+{}", base / 60, increment)
        } else {
            format!("{base}s+{increment}")
        }
    }
}

/// Parses one component of a time control into milliseconds.
///
/// `default_unit_ms` is applied when the number carries no unit suffix.
fn parse_duration(text: &str, default_unit_ms: u64) -> Result<u64, String> {
    let (digits, unit_ms) = match text.strip_suffix('s') {
        Some(rest) => (rest, 1_000),
        None => match text.strip_suffix('m') {
            Some(rest) => (rest, 60_000),
            None => match text.strip_suffix('h') {
                Some(rest) => (rest, 3_600_000),
                None => (text, default_unit_ms),
            },
        },
    };
    let value: f64 = digits
        .parse()
        .map_err(|_| format!("invalid time value '{text}'"))?;
    if !value.is_finite() || value < 0.0 {
        return Err(format!("invalid time value '{text}'"));
    }
    Ok((value * unit_ms as f64).round() as u64)
}

/// A running two-sided chess clock.
#[derive(Debug, Clone)]
pub struct GameClock {
    /// Remaining milliseconds: index 0 = White, 1 = Black.
    remaining: [u64; 2],
    /// Increment added after each completed move.
    increment_ms: u64,
    /// When the side to move started thinking.
    started: Option<Instant>,
    /// Which side the running measurement belongs to.
    running_for: Color,
    /// The side that ran out of time, once one has.
    flagged: Option<Color>,
    /// The control this clock was created from, so it can be reset.
    control: TimeControl,
}

/// Index of a colour in the clock arrays.
fn slot(color: Color) -> usize {
    match color {
        Color::White => 0,
        Color::Black => 1,
    }
}

impl GameClock {
    /// Creates a clock from a time control, with both sides at the base time.
    pub fn new(control: TimeControl) -> Self {
        Self {
            remaining: [control.base_ms; 2],
            increment_ms: control.increment_ms,
            started: None,
            running_for: Color::White,
            flagged: None,
            control,
        }
    }

    /// Starts timing `color`'s move.
    ///
    /// Starting an already-running clock is a no-op. The REPL calls this at the
    /// top of every iteration — including after an informational command such
    /// as `board` or `hint`, an unrecognised token or a parse error — and
    /// resetting the stopwatch there would hand the player back every second
    /// they had already spent thinking.
    pub fn start(&mut self, color: Color) {
        if self.started.is_some() && self.running_for == color {
            return;
        }
        self.started = Some(Instant::now());
        self.running_for = color;
    }

    /// Returns the clock to its starting state for a fresh game.
    ///
    /// [`Self::flagged`] is sticky, so without this a restart after a flag
    /// fall would end the new game before its first move.
    pub fn reset(&mut self) {
        *self = Self::new(self.control);
    }

    /// Stops the running measurement, deducts the elapsed time and adds the
    /// increment. Returns the time the move took, in milliseconds.
    ///
    /// A side whose clock hits zero is recorded as flagged; the increment is
    /// not credited in that case.
    pub fn stop(&mut self) -> u64 {
        let Some(started) = self.started.take() else {
            return 0;
        };
        let elapsed = started.elapsed().as_millis() as u64;
        let index = slot(self.running_for);
        if elapsed >= self.remaining[index] {
            self.remaining[index] = 0;
            if self.flagged.is_none() {
                self.flagged = Some(self.running_for);
            }
        } else {
            self.remaining[index] -= elapsed;
            self.remaining[index] += self.increment_ms;
        }
        elapsed
    }

    /// Remaining milliseconds for a side, accounting for a running clock.
    pub fn remaining_ms(&self, color: Color) -> u64 {
        let base = self.remaining[slot(color)];
        match self.started {
            Some(started) if self.running_for == color => {
                base.saturating_sub(started.elapsed().as_millis() as u64)
            }
            _ => base,
        }
    }

    /// The side that lost on time, if any.
    pub fn flagged(&self) -> Option<Color> {
        self.flagged
    }

    /// Returns `true` once a side has run out of time.
    pub fn is_flagged(&self) -> bool {
        self.flagged.is_some()
    }

    /// Time budget for the side to move's next search, in milliseconds.
    ///
    /// Mirrors the UCI allocator: a fixed fraction of the remaining clock
    /// plus half the increment, capped so a safety reserve always survives.
    pub fn budget_ms(&self, color: Color) -> u64 {
        let remaining = self.remaining_ms(color);
        let budget = remaining / CLOCK_DIVISOR + self.increment_ms / 2;
        budget
            .min(remaining.saturating_sub(RESERVE_MS))
            .max(MIN_BUDGET_MS)
    }

    /// Formats a side's clock as `MM:SS` (or `M:SS.d` under ten seconds).
    pub fn format(&self, color: Color) -> String {
        format_clock(self.remaining_ms(color))
    }
}

/// Formats a millisecond count as a clock readout.
pub fn format_clock(ms: u64) -> String {
    let total_seconds = ms / 1000;
    let (minutes, seconds) = (total_seconds / 60, total_seconds % 60);
    if ms < 10_000 {
        format!("{}.{}", seconds, (ms % 1000) / 100)
    } else if minutes >= 60 {
        format!("{}:{:02}:{:02}", minutes / 60, minutes % 60, seconds)
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// Converts a duration to whole milliseconds (small helper for callers).
pub fn as_millis(duration: Duration) -> u64 {
    duration.as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minutes_plus_increment() {
        let tc = TimeControl::parse("5+3").unwrap();
        assert_eq!(tc.base_ms, 5 * 60_000);
        assert_eq!(tc.increment_ms, 3_000);
        assert_eq!(tc.label(), "5+3");
    }

    #[test]
    fn test_parse_bare_number_is_minutes() {
        let tc = TimeControl::parse("10").unwrap();
        assert_eq!(tc.base_ms, 600_000);
        assert_eq!(tc.increment_ms, 0);
    }

    #[test]
    fn test_parse_explicit_units() {
        assert_eq!(TimeControl::parse("30s").unwrap().base_ms, 30_000);
        assert_eq!(TimeControl::parse("1m+2s").unwrap().increment_ms, 2_000);
        assert_eq!(TimeControl::parse("1h").unwrap().base_ms, 3_600_000);
        assert_eq!(TimeControl::parse(" 5 + 3 ").unwrap().base_ms, 300_000);
    }

    #[test]
    fn test_parse_rejects_nonsense() {
        assert!(TimeControl::parse("").is_err());
        assert!(TimeControl::parse("abc").is_err());
        assert!(TimeControl::parse("-5").is_err());
        assert!(TimeControl::parse("0+0").is_err());
    }

    #[test]
    fn test_clock_deducts_and_increments() {
        let mut clock = GameClock::new(TimeControl {
            base_ms: 10_000,
            increment_ms: 1_000,
        });
        clock.start(Color::White);
        let spent = clock.stop();
        // The move took almost no time, so the increment dominates.
        assert!(clock.remaining_ms(Color::White) >= 10_000);
        assert!(spent < 1_000);
        assert_eq!(clock.remaining_ms(Color::Black), 10_000);
        assert!(!clock.is_flagged());
    }

    #[test]
    fn test_clock_flags_on_timeout() {
        let mut clock = GameClock::new(TimeControl {
            base_ms: 0,
            increment_ms: 1,
        });
        clock.start(Color::Black);
        clock.stop();
        assert_eq!(clock.flagged(), Some(Color::Black));
        assert_eq!(clock.remaining_ms(Color::Black), 0);
    }

    #[test]
    fn test_budget_scales_with_remaining_time() {
        let clock = GameClock::new(TimeControl {
            base_ms: 60_000,
            increment_ms: 2_000,
        });
        // 60000/25 + 1000 = 3400 ms
        assert_eq!(clock.budget_ms(Color::White), 3_400);

        // With almost no time left the fractional budget falls below the
        // floor, so the floor wins — but it still leaves most of the clock.
        let low = GameClock::new(TimeControl {
            base_ms: 100,
            increment_ms: 0,
        });
        assert_eq!(low.budget_ms(Color::White), MIN_BUDGET_MS);
        assert!(low.budget_ms(Color::White) < 100 - RESERVE_MS + MIN_BUDGET_MS);
    }

    #[test]
    fn test_format_clock() {
        assert_eq!(format_clock(65_000), "1:05");
        assert_eq!(format_clock(3_725_000), "1:02:05");
        assert_eq!(format_clock(4_200), "4.2");
    }

    /// The REPL calls `start` at the top of every iteration, including after
    /// `board`, `hint`, an unknown token or a parse error. Restarting the
    /// stopwatch there handed the player back all the time they had spent.
    #[test]
    fn test_start_does_not_restart_a_running_clock() {
        let mut clock = GameClock::new(TimeControl {
            base_ms: 60_000,
            increment_ms: 0,
        });
        clock.start(Color::White);
        std::thread::sleep(std::time::Duration::from_millis(40));
        clock.start(Color::White); // an informational command came in
        let spent = clock.stop();
        assert!(
            spent >= 30,
            "thinking time must survive a non-move command, got {spent}ms"
        );

        // Switching sides still starts a fresh measurement.
        clock.start(Color::Black);
        assert_eq!(clock.remaining_ms(Color::White), 60_000 - spent);
    }

    /// `flagged` is sticky, so a clock carried into a new game ended it before
    /// the first move.
    #[test]
    fn test_reset_clears_a_sticky_flag() {
        let mut clock = GameClock::new(TimeControl {
            base_ms: 1,
            increment_ms: 0,
        });
        clock.start(Color::White);
        std::thread::sleep(std::time::Duration::from_millis(5));
        clock.stop();
        assert!(clock.is_flagged());

        clock.reset();
        assert!(
            !clock.is_flagged(),
            "a restarted game must not inherit the previous flag fall"
        );
        assert_eq!(clock.remaining_ms(Color::White), 1);
        assert_eq!(clock.remaining_ms(Color::Black), 1);
    }
}
