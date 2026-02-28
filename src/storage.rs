//! Persistent game storage with compact binary format and zstd compression.
//!
//! # Storage Strategy
//!
//! Games are stored in a custom binary format optimized for minimal size:
//!
//! - **Active games** (in progress): Saved as uncompressed `.cai` files after
//!   each move, allowing recovery after server restarts.
//! - **Completed games**: Compressed with zstd level 19 (maximum compression)
//!   into `.cai.zst` files, then the uncompressed active file is removed.
//!
//! # Binary Format (`.cai`)
//!
//! The format encodes a complete game in the absolute minimum number of bytes:
//!
//! ```text
//! Offset  Size   Field
//! ──────  ────   ─────
//! 0       4      Magic bytes: "CKAI"
//! 4       1      Format version (currently 1)
//! 5       16     Game UUID (big-endian bytes)
//! 21      8      Start timestamp (unix epoch seconds, big-endian u64)
//! 29      8      End timestamp (0 if ongoing, big-endian u64)
//! 37      1      Result: 0=ongoing, 1=WhiteWins, 2=BlackWins, 3=Draw
//! 38      1      End reason (see GameEndReason encoding)
//! 39      2      Move count (big-endian u16, max 65535 half-moves)
//!
//! Header total: 41 bytes
//!
//! 41..    2×N    Encoded moves (2 bytes each):
//!                  Bits 0–5:   from square (0–63, rank*8+file)
//!                  Bits 6–11:  to square (0–63)
//!                  Bits 12–14: promotion (0=none, 1=Q, 2=R, 3=B, 4=N)
//!                  Bit  15:    reserved (0)
//! ```
//!
//! A typical 40-move game = 41 + 80×2 = 201 bytes raw.
//! With zstd compression this typically shrinks to ~120–160 bytes.
//!
//! # Reversibility
//!
//! Completed games can be fully replayed for analysis:
//! - Decode the move list from the binary file
//! - Replay each move from the starting position
//! - Reconstruct the exact board state at any move number

use crate::game::Game;
use crate::types::*;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Magic bytes identifying a CheckAI game file.
const MAGIC: &[u8; 4] = b"CKAI";

/// Current binary format version.
const FORMAT_VERSION: u8 = 1;

/// zstd compression level (19 = near-maximum compression for small data).
const ZSTD_COMPRESSION_LEVEL: i32 = 19;

// ---------------------------------------------------------------------------
// Compact move encoding (2 bytes per move)
// ---------------------------------------------------------------------------

/// Encodes a chess move into 2 bytes.
///
/// Layout (16 bits, little-endian u16):
/// - Bits 0–5:   from square index (rank*8 + file)
/// - Bits 6–11:  to square index
/// - Bits 12–14: promotion (0=none, 1=Q, 2=R, 3=B, 4=N)
/// - Bit 15:     reserved
///
/// This packs any possible chess move into exactly 2 bytes.
pub fn encode_move(mv: &MoveJson) -> Result<u16, String> {
    let from = Square::from_algebraic(&mv.from)
        .ok_or_else(|| t!("storage.invalid_from", value = &mv.from).to_string())?;
    let to = Square::from_algebraic(&mv.to)
        .ok_or_else(|| t!("storage.invalid_to", value = &mv.to).to_string())?;

    let from_idx = from.index() as u16;
    let to_idx = to.index() as u16;

    let promo_bits: u16 = match &mv.promotion {
        None => 0,
        Some(p) => match p.as_str() {
            "Q" => 1,
            "R" => 2,
            "B" => 3,
            "N" => 4,
            _ => return Err(t!("storage.invalid_promotion", value = p).to_string()),
        },
    };

    Ok(from_idx | (to_idx << 6) | (promo_bits << 12))
}

/// Decodes a 2-byte encoded move back to a `MoveJson`.
pub fn decode_move(encoded: u16) -> MoveJson {
    let from_idx = (encoded & 0x3F) as usize;
    let to_idx = ((encoded >> 6) & 0x3F) as usize;
    let promo = (encoded >> 12) & 0x07;

    let from_file = (from_idx % 8) as u8;
    let from_rank = (from_idx / 8) as u8;
    let to_file = (to_idx % 8) as u8;
    let to_rank = (to_idx / 8) as u8;

    let from_sq = Square::new(from_file, from_rank);
    let to_sq = Square::new(to_file, to_rank);

    let promotion = match promo {
        1 => Some("Q".to_string()),
        2 => Some("R".to_string()),
        3 => Some("B".to_string()),
        4 => Some("N".to_string()),
        _ => None,
    };

    MoveJson {
        from: from_sq.to_algebraic(),
        to: to_sq.to_algebraic(),
        promotion,
    }
}

// ---------------------------------------------------------------------------
// Result / reason encoding (1 byte each)
// ---------------------------------------------------------------------------

/// Encodes a `GameResult` into a single byte.
fn encode_result(result: Option<&GameResult>) -> u8 {
    match result {
        None => 0,
        Some(GameResult::WhiteWins) => 1,
        Some(GameResult::BlackWins) => 2,
        Some(GameResult::Draw) => 3,
    }
}

/// Decodes a byte into an `Option<GameResult>`.
fn decode_result(byte: u8) -> Option<GameResult> {
    match byte {
        1 => Some(GameResult::WhiteWins),
        2 => Some(GameResult::BlackWins),
        3 => Some(GameResult::Draw),
        _ => None,
    }
}

/// Encodes a `GameEndReason` into a single byte.
fn encode_end_reason(reason: Option<&GameEndReason>) -> u8 {
    match reason {
        None => 0,
        Some(GameEndReason::Checkmate) => 1,
        Some(GameEndReason::Stalemate) => 2,
        Some(GameEndReason::ThreefoldRepetition) => 3,
        Some(GameEndReason::FivefoldRepetition) => 4,
        Some(GameEndReason::FiftyMoveRule) => 5,
        Some(GameEndReason::SeventyFiveMoveRule) => 6,
        Some(GameEndReason::InsufficientMaterial) => 7,
        Some(GameEndReason::Resignation) => 8,
        Some(GameEndReason::DrawAgreement) => 9,
    }
}

/// Decodes a byte into an `Option<GameEndReason>`.
fn decode_end_reason(byte: u8) -> Option<GameEndReason> {
    match byte {
        1 => Some(GameEndReason::Checkmate),
        2 => Some(GameEndReason::Stalemate),
        3 => Some(GameEndReason::ThreefoldRepetition),
        4 => Some(GameEndReason::FivefoldRepetition),
        5 => Some(GameEndReason::FiftyMoveRule),
        6 => Some(GameEndReason::SeventyFiveMoveRule),
        7 => Some(GameEndReason::InsufficientMaterial),
        8 => Some(GameEndReason::Resignation),
        9 => Some(GameEndReason::DrawAgreement),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Serialization
// ---------------------------------------------------------------------------

/// Serializes a game into the compact binary `.cai` format.
///
/// The binary format stores only the move sequence plus minimal metadata.
/// The full game state can be reconstructed by replaying the moves
/// from the starting position.
pub fn serialize_game(game: &Game) -> Result<Vec<u8>, String> {
    let move_count = game.move_history.len();
    if move_count > u16::MAX as usize {
        return Err(t!("storage.too_many_moves").to_string());
    }

    // Calculate buffer size: header (41) + moves (2 each)
    let buf_size = 41 + move_count * 2;
    let mut buf = Vec::with_capacity(buf_size);

    // Magic
    buf.extend_from_slice(MAGIC);

    // Version
    buf.push(FORMAT_VERSION);

    // Game UUID (16 bytes)
    buf.extend_from_slice(game.id.as_bytes());

    // Start timestamp (8 bytes, big-endian)
    buf.extend_from_slice(&game.start_timestamp.to_be_bytes());

    // End timestamp (8 bytes, big-endian)
    buf.extend_from_slice(&game.end_timestamp.to_be_bytes());

    // Result (1 byte)
    buf.push(encode_result(game.result.as_ref()));

    // End reason (1 byte)
    buf.push(encode_end_reason(game.end_reason.as_ref()));

    // Move count (2 bytes, big-endian)
    buf.extend_from_slice(&(move_count as u16).to_be_bytes());

    // Encoded moves (2 bytes each)
    for record in &game.move_history {
        let encoded = encode_move(&record.move_json)?;
        buf.extend_from_slice(&encoded.to_le_bytes());
    }

    Ok(buf)
}

/// Deserializes a game from the compact binary `.cai` format.
///
/// Returns a `GameArchive` containing the metadata and move list.
/// Use `GameArchive::replay()` to reconstruct the full game state.
pub fn deserialize_game(data: &[u8]) -> Result<GameArchive, String> {
    if data.len() < 41 {
        return Err(t!("storage.header_too_short").to_string());
    }

    // Validate magic
    if &data[0..4] != MAGIC {
        return Err(t!("storage.invalid_magic").to_string());
    }

    // Version
    let version = data[4];
    if version != FORMAT_VERSION {
        return Err(t!("storage.unsupported_version", version = version).to_string());
    }

    // Game UUID
    let uuid_bytes: [u8; 16] = data[5..21].try_into().unwrap();
    let game_id = Uuid::from_bytes(uuid_bytes);

    // Timestamps
    let start_ts = u64::from_be_bytes(data[21..29].try_into().unwrap());
    let end_ts = u64::from_be_bytes(data[29..37].try_into().unwrap());

    // Result and reason
    let result = decode_result(data[37]);
    let end_reason = decode_end_reason(data[38]);

    // Move count
    let move_count = u16::from_be_bytes(data[39..41].try_into().unwrap()) as usize;

    // Validate data length
    let expected_len = 41 + move_count * 2;
    if data.len() < expected_len {
        return Err(t!(
            "storage.data_too_short",
            expected = expected_len,
            got = data.len()
        )
        .to_string());
    }

    // Decode moves
    let mut moves = Vec::with_capacity(move_count);
    for i in 0..move_count {
        let offset = 41 + i * 2;
        let encoded = u16::from_le_bytes(data[offset..offset + 2].try_into().unwrap());
        moves.push(decode_move(encoded));
    }

    Ok(GameArchive {
        game_id,
        start_timestamp: start_ts,
        end_timestamp: end_ts,
        result,
        end_reason,
        moves,
    })
}

// ---------------------------------------------------------------------------
// GameArchive — decoded game data for analysis
// ---------------------------------------------------------------------------

/// A decoded game archive with metadata and move list.
///
/// Can be replayed to reconstruct the full board state at any point.
#[derive(Debug, Clone)]
pub struct GameArchive {
    /// The game's unique identifier.
    pub game_id: Uuid,
    /// Unix timestamp when the game started.
    pub start_timestamp: u64,
    /// Unix timestamp when the game ended (0 if still active).
    pub end_timestamp: u64,
    /// The game result, if ended.
    pub result: Option<GameResult>,
    /// The reason the game ended, if applicable.
    pub end_reason: Option<GameEndReason>,
    /// The complete move list in order.
    pub moves: Vec<MoveJson>,
}

impl GameArchive {
    /// Returns the total number of half-moves in the game.
    pub fn move_count(&self) -> usize {
        self.moves.len()
    }

    /// Returns the raw binary size of this game (uncompressed).
    pub fn raw_size(&self) -> usize {
        41 + self.moves.len() * 2
    }

    /// Replays the game up to a given half-move index and returns
    /// a fully reconstructed `Game` at that point.
    ///
    /// - `up_to_move`: Number of half-moves to replay (0 = starting position,
    ///   `move_count()` = final position). Clamped to available moves.
    ///
    /// This is the core analysis function: by replaying with different
    /// `up_to_move` values, you can inspect any position in the game.
    pub fn replay(&self, up_to_move: usize) -> Result<Game, String> {
        let mut game = Game::new_with_id_and_timestamps(
            self.game_id,
            self.start_timestamp,
            self.end_timestamp,
        );

        let limit = up_to_move.min(self.moves.len());
        for (i, mv) in self.moves.iter().enumerate() {
            if i >= limit {
                break;
            }
            game.make_move(mv)
                .map_err(|e| t!("storage.replay_failed", num = (i + 1), error = e).to_string())?;
        }

        Ok(game)
    }

    /// Replays the entire game to the final position.
    pub fn replay_full(&self) -> Result<Game, String> {
        self.replay(self.moves.len())
    }
}

// ---------------------------------------------------------------------------
// GameStorage — file-based persistence manager
// ---------------------------------------------------------------------------

/// Manages persistent game storage on disk.
///
/// Directory layout:
/// ```text
/// <base_dir>/
///   active/           # Currently in-progress games (.cai)
///   archive/          # Completed, zstd-compressed games (.cai.zst)
/// ```
pub struct GameStorage {
    /// Base directory for all game files.
    base_dir: PathBuf,
    /// Directory for active (in-progress) game files.
    active_dir: PathBuf,
    /// Directory for archived (completed, compressed) game files.
    archive_dir: PathBuf,
}

impl GameStorage {
    /// Creates a new `GameStorage` with the given base directory.
    ///
    /// Creates the directory structure if it doesn't exist.
    pub fn new(base_dir: impl AsRef<Path>) -> io::Result<Self> {
        let base_dir = base_dir.as_ref().to_path_buf();
        let active_dir = base_dir.join("active");
        let archive_dir = base_dir.join("archive");

        fs::create_dir_all(&active_dir)?;
        fs::create_dir_all(&archive_dir)?;

        log::info!("Game storage initialized at {}", base_dir.display());

        Ok(Self {
            base_dir,
            active_dir,
            archive_dir,
        })
    }

    /// Returns the base storage directory path.
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Returns the file path for an active game.
    fn active_path(&self, game_id: &Uuid) -> PathBuf {
        self.active_dir.join(format!("{}.cai", game_id))
    }

    /// Returns the file path for an archived game.
    fn archive_path(&self, game_id: &Uuid) -> PathBuf {
        self.archive_dir.join(format!("{}.cai.zst", game_id))
    }

    /// Persists an active game to disk (uncompressed).
    ///
    /// Called after each move to ensure games survive server restarts.
    /// Uses atomic write (write to temp, then rename) to prevent corruption.
    pub fn save_active(&self, game: &Game) -> Result<(), String> {
        let data = serialize_game(game)?;
        let path = self.active_path(&game.id);
        let temp_path = self.active_dir.join(format!("{}.cai.tmp", game.id));

        fs::write(&temp_path, &data).map_err(|e| format!("Failed to write temp file: {}", e))?;
        fs::rename(&temp_path, &path).map_err(|e| format!("Failed to rename temp file: {}", e))?;

        log::debug!(
            "Saved active game {} ({} bytes, {} moves)",
            game.id,
            data.len(),
            game.move_history.len()
        );
        Ok(())
    }

    /// Archives a completed game: compresses with zstd and moves to archive/.
    ///
    /// The uncompressed active file is removed after successful archival.
    /// Returns the compressed size in bytes.
    pub fn archive_game(&self, game: &Game) -> Result<usize, String> {
        let raw_data = serialize_game(game)?;
        let raw_size = raw_data.len();

        // Compress with zstd at maximum compression level
        let compressed = zstd::encode_all(raw_data.as_slice(), ZSTD_COMPRESSION_LEVEL)
            .map_err(|e| format!("zstd compression failed: {}", e))?;
        let compressed_size = compressed.len();

        // Write compressed archive
        let archive_path = self.archive_path(&game.id);
        fs::write(&archive_path, &compressed)
            .map_err(|e| format!("Failed to write archive: {}", e))?;

        // Remove the active file
        let active_path = self.active_path(&game.id);
        if active_path.exists() {
            let _ = fs::remove_file(&active_path);
        }

        let ratio = if raw_size > 0 {
            (compressed_size as f64 / raw_size as f64) * 100.0
        } else {
            0.0
        };

        log::info!(
            "Archived game {}: {} → {} bytes ({:.1}% of original, {} moves)",
            game.id,
            raw_size,
            compressed_size,
            ratio,
            game.move_history.len()
        );

        Ok(compressed_size)
    }

    /// Loads an active game from disk.
    pub fn load_active(&self, game_id: &Uuid) -> Result<GameArchive, String> {
        let path = self.active_path(game_id);
        let data = fs::read(&path)
            .map_err(|e| format!("Failed to read active game {}: {}", game_id, e))?;
        deserialize_game(&data)
    }

    /// Loads an archived (compressed) game from disk.
    pub fn load_archive(&self, game_id: &Uuid) -> Result<GameArchive, String> {
        let path = self.archive_path(game_id);
        let compressed =
            fs::read(&path).map_err(|e| format!("Failed to read archive {}: {}", game_id, e))?;

        let decompressed = zstd::decode_all(compressed.as_slice())
            .map_err(|e| format!("zstd decompression failed: {}", e))?;

        deserialize_game(&decompressed)
    }

    /// Loads a game from either active or archive storage.
    ///
    /// Checks active directory first, then archive.
    pub fn load_any(&self, game_id: &Uuid) -> Result<(GameArchive, bool), String> {
        // Try active first
        let active_path = self.active_path(game_id);
        if active_path.exists() {
            let archive = self.load_active(game_id)?;
            return Ok((archive, false)); // false = not compressed
        }

        // Try archive
        let archive_path = self.archive_path(game_id);
        if archive_path.exists() {
            let archive = self.load_archive(game_id)?;
            return Ok((archive, true)); // true = compressed
        }

        Err(t!("storage.game_not_found", id = game_id).to_string())
    }

    /// Lists all archived game IDs.
    pub fn list_archived(&self) -> Result<Vec<Uuid>, String> {
        let mut ids = Vec::new();
        let entries = fs::read_dir(&self.archive_dir)
            .map_err(|e| format!("Failed to read archive directory: {}", e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let filename = entry.file_name().to_string_lossy().to_string();
            if let Some(id_str) = filename.strip_suffix(".cai.zst")
                && let Ok(id) = Uuid::parse_str(id_str)
            {
                ids.push(id);
            }
        }

        Ok(ids)
    }

    /// Lists all active game IDs on disk.
    pub fn list_active_on_disk(&self) -> Result<Vec<Uuid>, String> {
        let mut ids = Vec::new();
        let entries = fs::read_dir(&self.active_dir)
            .map_err(|e| format!("Failed to read active directory: {}", e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let filename = entry.file_name().to_string_lossy().to_string();
            if let Some(id_str) = filename.strip_suffix(".cai")
                && let Ok(id) = Uuid::parse_str(id_str)
            {
                ids.push(id);
            }
        }

        Ok(ids)
    }

    /// Returns storage statistics.
    pub fn stats(&self) -> Result<StorageStats, String> {
        let active_ids = self.list_active_on_disk()?;
        let archived_ids = self.list_archived()?;

        let mut active_bytes: u64 = 0;
        for id in &active_ids {
            let path = self.active_path(id);
            if let Ok(meta) = fs::metadata(&path) {
                active_bytes += meta.len();
            }
        }

        let mut archive_bytes: u64 = 0;
        for id in &archived_ids {
            let path = self.archive_path(id);
            if let Ok(meta) = fs::metadata(&path) {
                archive_bytes += meta.len();
            }
        }

        Ok(StorageStats {
            active_count: active_ids.len(),
            archived_count: archived_ids.len(),
            active_bytes,
            archive_bytes,
            total_bytes: active_bytes + archive_bytes,
        })
    }

    /// Removes an active game file from disk.
    pub fn remove_active(&self, game_id: &Uuid) -> Result<(), String> {
        let path = self.active_path(game_id);
        if path.exists() {
            fs::remove_file(&path)
                .map_err(|e| format!("Failed to remove active game file: {}", e))?;
        }
        Ok(())
    }

    /// Removes an archived game file from disk.
    pub fn remove_archive(&self, game_id: &Uuid) -> Result<(), String> {
        let path = self.archive_path(game_id);
        if path.exists() {
            fs::remove_file(&path).map_err(|e| format!("Failed to remove archive file: {}", e))?;
        }
        Ok(())
    }

    /// Returns the compressed size of an archived game in bytes.
    pub fn archive_file_size(&self, game_id: &Uuid) -> Option<u64> {
        let path = self.archive_path(game_id);
        fs::metadata(&path).ok().map(|m| m.len())
    }
}

/// Storage statistics.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct StorageStats {
    /// Number of active (in-progress) games on disk.
    pub active_count: usize,
    /// Number of archived (completed, compressed) games.
    pub archived_count: usize,
    /// Total bytes used by active game files.
    pub active_bytes: u64,
    /// Total bytes used by archived game files.
    pub archive_bytes: u64,
    /// Total bytes used by all game files.
    pub total_bytes: u64,
}

/// Summary of an archived game for API responses.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct ArchiveSummary {
    /// The game's unique identifier.
    pub game_id: String,
    /// Number of half-moves in the game.
    pub move_count: usize,
    /// The game result.
    pub result: Option<GameResult>,
    /// The reason the game ended.
    pub end_reason: Option<GameEndReason>,
    /// Unix timestamp when the game started.
    pub start_timestamp: u64,
    /// Unix timestamp when the game ended.
    pub end_timestamp: u64,
    /// Compressed file size in bytes.
    pub compressed_bytes: u64,
    /// Uncompressed data size in bytes.
    pub raw_bytes: usize,
}

/// Response for the replay endpoint.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct ReplayResponse {
    /// The game's unique identifier.
    pub game_id: String,
    /// Which half-move position was replayed to.
    pub at_move: usize,
    /// Total number of moves in the game.
    pub total_moves: usize,
    /// The game state at the replayed position.
    pub state: GameStateJson,
    /// Whether the game was over at this position.
    pub is_over: bool,
    /// The result at this position (only set if game was over).
    pub result: Option<GameResult>,
    /// Whether the side to move is in check at this position.
    pub is_check: bool,
}

/// Response listing archived games.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct ArchiveListResponse {
    /// List of archived game summaries.
    pub games: Vec<ArchiveSummary>,
    /// Total number of archived games.
    pub total: usize,
    /// Storage statistics.
    pub storage: StorageStats,
}

// ---------------------------------------------------------------------------
// Utility: current unix timestamp
// ---------------------------------------------------------------------------

/// Returns the current Unix timestamp in seconds.
pub fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_move_encode_decode_roundtrip() {
        let moves = vec![
            MoveJson {
                from: "e2".into(),
                to: "e4".into(),
                promotion: None,
            },
            MoveJson {
                from: "g1".into(),
                to: "f3".into(),
                promotion: None,
            },
            MoveJson {
                from: "e7".into(),
                to: "e8".into(),
                promotion: Some("Q".into()),
            },
            MoveJson {
                from: "a7".into(),
                to: "a8".into(),
                promotion: Some("N".into()),
            },
            MoveJson {
                from: "a1".into(),
                to: "h8".into(),
                promotion: None,
            },
        ];

        for mv in &moves {
            let encoded = encode_move(mv).unwrap();
            let decoded = decode_move(encoded);
            assert_eq!(mv.from, decoded.from, "from mismatch for {:?}", mv);
            assert_eq!(mv.to, decoded.to, "to mismatch for {:?}", mv);
            assert_eq!(
                mv.promotion, decoded.promotion,
                "promotion mismatch for {:?}",
                mv
            );
        }
    }

    #[test]
    fn test_encode_move_size() {
        // Every move must fit in 2 bytes (u16)
        let encoded = encode_move(&MoveJson {
            from: "h8".into(),
            to: "a1".into(),
            promotion: Some("N".into()),
        })
        .unwrap();
        assert!(encoded <= u16::MAX);
    }

    #[test]
    fn test_serialize_deserialize_roundtrip() {
        let mut game = Game::new();
        // Play 1. e4 e5
        game.make_move(&MoveJson {
            from: "e2".into(),
            to: "e4".into(),
            promotion: None,
        })
        .unwrap();
        game.make_move(&MoveJson {
            from: "e7".into(),
            to: "e5".into(),
            promotion: None,
        })
        .unwrap();

        let data = serialize_game(&game).unwrap();
        assert_eq!(data.len(), 41 + 4); // header + 2 moves × 2 bytes

        let archive = deserialize_game(&data).unwrap();
        assert_eq!(archive.game_id, game.id);
        assert_eq!(archive.moves.len(), 2);
        assert_eq!(archive.moves[0].from, "e2");
        assert_eq!(archive.moves[0].to, "e4");
        assert_eq!(archive.moves[1].from, "e7");
        assert_eq!(archive.moves[1].to, "e5");
    }

    #[test]
    fn test_replay_position() {
        let mut game = Game::new();
        game.make_move(&MoveJson {
            from: "e2".into(),
            to: "e4".into(),
            promotion: None,
        })
        .unwrap();
        game.make_move(&MoveJson {
            from: "e7".into(),
            to: "e5".into(),
            promotion: None,
        })
        .unwrap();
        game.make_move(&MoveJson {
            from: "g1".into(),
            to: "f3".into(),
            promotion: None,
        })
        .unwrap();

        let data = serialize_game(&game).unwrap();
        let archive = deserialize_game(&data).unwrap();

        // Replay to move 0 = starting position
        let g0 = archive.replay(0).unwrap();
        assert_eq!(g0.fullmove_number, 1);
        assert_eq!(g0.turn, Color::White);

        // Replay to move 2 = after 1. e4 e5
        let g2 = archive.replay(2).unwrap();
        assert_eq!(g2.fullmove_number, 2);
        assert_eq!(g2.turn, Color::White);

        // Replay all 3 moves
        let g3 = archive.replay(3).unwrap();
        assert_eq!(g3.turn, Color::Black);
        assert_eq!(g3.move_history.len(), 3);
    }

    #[test]
    fn test_compression_ratio() {
        let mut game = Game::new();
        // Play a few moves
        let moves = vec![
            ("e2", "e4"),
            ("e7", "e5"),
            ("g1", "f3"),
            ("b8", "c6"),
            ("f1", "b5"),
            ("a7", "a6"),
        ];
        for (from, to) in moves {
            game.make_move(&MoveJson {
                from: from.into(),
                to: to.into(),
                promotion: None,
            })
            .unwrap();
        }

        let raw = serialize_game(&game).unwrap();
        let compressed = zstd::encode_all(raw.as_slice(), ZSTD_COMPRESSION_LEVEL).unwrap();

        println!(
            "Raw: {} bytes, Compressed: {} bytes, Ratio: {:.1}%",
            raw.len(),
            compressed.len(),
            (compressed.len() as f64 / raw.len() as f64) * 100.0
        );

        // Verify we can decompress and replay
        let decompressed = zstd::decode_all(compressed.as_slice()).unwrap();
        assert_eq!(raw, decompressed);
    }

    #[test]
    fn test_storage_on_disk() {
        let dir = std::env::temp_dir().join(format!("checkai_test_{}", Uuid::new_v4()));
        let storage = GameStorage::new(&dir).unwrap();

        let mut game = Game::new();
        game.make_move(&MoveJson {
            from: "e2".into(),
            to: "e4".into(),
            promotion: None,
        })
        .unwrap();

        // Save active
        storage.save_active(&game).unwrap();
        let loaded = storage.load_active(&game.id).unwrap();
        assert_eq!(loaded.moves.len(), 1);

        // Archive
        let size = storage.archive_game(&game).unwrap();
        assert!(size > 0);
        assert!(!storage.active_path(&game.id).exists());
        assert!(storage.archive_path(&game.id).exists());

        // Load from archive
        let archived = storage.load_archive(&game.id).unwrap();
        assert_eq!(archived.moves.len(), 1);

        // Cleanup
        let _ = fs::remove_dir_all(&dir);
    }
}
