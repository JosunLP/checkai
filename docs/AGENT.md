
# AGENT.md — Chess Rulebook & Protocol (FIDE 2023)

> This file completely defines the rules, data format, and behavior
> for AI agents that play a game of chess.
> Basis: FIDE Laws of Chess, effective January 1, 2023.

---

## 1. ROLE OF THE AGENT

You are a chess-playing agent. Before each move, you receive the complete
game state as a JSON object. You must return **exactly one legal move** as a
JSON object. Illegal moves are under no circumstances acceptable.
Analyze the position carefully, calculate all legal moves, and choose one.

---

## 2. THE CHESSBOARD

The board consists of an 8×8 grid with 64 squares.

- **Files (columns):** `a` to `h` (from left to right from White’s perspective)
- **Ranks (rows):** `1` to `8` (rank 1 = White home rank, rank 8 = Black home rank)
- Squares are represented as two-character strings, e.g. `"e4"`, `"a1"`, `"h8"`.
- The bottom-right square from White’s perspective (`h1`) is light;
  its neighboring square to the left (`g1`) is dark.

---

## 3. PIECE SYMBOLS (Short Notation)

| Symbol | Piece  | Color |
| ------ | ------ | ----- |
| `K`    | King   | White |
| `Q`    | Queen  | White |
| `R`    | Rook   | White |
| `B`    | Bishop | White |
| `N`    | Knight | White |
| `P`    | Pawn   | White |
| `k`    | King   | Black |
| `q`    | Queen  | Black |
| `r`    | Rook   | Black |
| `b`    | Bishop | Black |
| `n`    | Knight | Black |
| `p`    | Pawn   | Black |

Uppercase = White, lowercase = Black. This follows the FEN convention.

---

## 4. STARTING POSITION

```txt

Rank 8 (Black): r n b q k b n r
Rank 7 (Black): p p p p p p p p
Ranks 6–3:      (empty)
Rank 2 (White): P P P P P P P P
Rank 1 (White): R N B Q K B N R

```

Files a–h, rank 1:

- a1=R, b1=N, c1=B, d1=Q, e1=K, f1=B, g1=N, h1=R

Files a–h, rank 8:

- a8=r, b8=n, c8=b, d8=q, e8=k, f8=b, g8=n, h8=r

---

## 5. GAME STATE — INCOMING JSON (Input)

Before each move, the agent receives a JSON object with the following schema:

```json
{
  "board": {
    "<Square>": "<Symbol>"
  },
  "turn": "white" | "black",
  "castling": {
    "white": {
      "kingside": true | false,
      "queenside": true | false
    },
    "black": {
      "kingside": true | false,
      "queenside": true | false
    }
  },
  "en_passant": "<Square>" | null,
  "halfmove_clock": <Number>,
  "fullmove_number": <Number>,
  "position_history": ["<FEN>", ...]
}
```

### Field Description

| Field              | Type            | Description                                                                                                                             |
| ------------------ | --------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| `board`            | Object          | Contains only **occupied** squares. Key = square name (`"e4"`), value = piece symbol. Empty squares are **not** listed.                 |
| `turn`             | String          | `"white"` or `"black"` — side to move.                                                                                                  |
| `castling`         | Object          | Castling rights. `true` = right still available (king and rook have never moved), `false` = right lost.                                 |
| `en_passant`       | String \| null  | If a pawn advanced two squares in the last move, this is the skipped square (the possible en passant capture square). Otherwise `null`. |
| `halfmove_clock`   | Number          | Number of halfmoves since the last pawn move or capture. Used for the 50-move rule.                                                     |
| `fullmove_number`  | Number          | Full-move counter. Starts at 1, incremented after each Black move.                                                                      |
| `position_history` | Array\<String\> | List of all previous positions as simplified FEN strings (without move numbers), for the threefold repetition rule.                     |

### Example Input (Starting position, White to move)

```json
{
  "board": {
    "a1": "R", "b1": "N", "c1": "B", "d1": "Q", "e1": "K",
    "f1": "B", "g1": "N", "h1": "R",
    "a2": "P", "b2": "P", "c2": "P", "d2": "P", "e2": "P",
    "f2": "P", "g2": "P", "h2": "P",
    "a7": "p", "b7": "p", "c7": "p", "d7": "p", "e7": "p",
    "f7": "p", "g7": "p", "h7": "p",
    "a8": "r", "b8": "n", "c8": "b", "d8": "q", "e8": "k",
    "f8": "b", "g8": "n", "h8": "r"
  },
  "turn": "white",
  "castling": {
    "white": { "kingside": true, "queenside": true },
    "black": { "kingside": true, "queenside": true }
  },
  "en_passant": null,
  "halfmove_clock": 0,
  "fullmove_number": 1,
  "position_history": []
}
```

---

## 6. MOVE OUTPUT — OUTGOING JSON (Output)

The agent returns **exactly one** JSON object — no text before or after,
no Markdown, only raw JSON.

```json
{
  "from": "<Square>",
  "to": "<Square>",
  "promotion": "<Symbol>" | null
}
```

| Field       | Type           | Description                                                                                                                          |
| ----------- | -------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| `from`      | String         | Starting square of the piece, e.g. `"e2"`.                                                                                           |
| `to`        | String         | Target square of the piece, e.g. `"e4"`.                                                                                             |
| `promotion` | String \| null | Only for pawn promotion: target piece as an **uppercase letter** (`"Q"`, `"R"`, `"B"`, `"N"`), regardless of side. Otherwise `null`. |

### Special Cases in Output

**Castling:** Encoded as a king move (king moves two squares).

- White kingside: `"from": "e1", "to": "g1"`
- White queenside: `"from": "e1", "to": "c1"`
- Black kingside: `"from": "e8", "to": "g8"`
- Black queenside: `"from": "e8", "to": "c8"`

**En passant:** Encoded as a normal pawn move. The captured pawn
is removed by the system.

- Example: `"from": "e5", "to": "d6"` (captures pawn on d5 en passant)

**Pawn promotion:** `promotion` must contain a value.

- Example: `"from": "e7", "to": "e8", "promotion": "Q"`

### Examples

```json
{ "from": "e2", "to": "e4", "promotion": null }
```

```json
{ "from": "g1", "to": "f3", "promotion": null }
```

```json
{ "from": "e1", "to": "g1", "promotion": null }
```

```json
{ "from": "d7", "to": "d8", "promotion": "Q" }
```

---

## 7. PIECE MOVEMENT RULES (FIDE Art. 3)

### 7.1 General Rules

- No move to a square occupied by your own piece.
- If a piece moves to a square occupied by an opponent piece,
  that opponent piece is captured and removed from the board.
- Bishops, rooks, and queens **cannot jump over other pieces**.
- The knight is the only piece (except during castling) that can jump over
  other pieces.
- **No move may leave or place your own king in check.**

### 7.2 King (`K` / `k`)

- Moves **exactly one square** in any direction (horizontal, vertical,
  diagonal).
- May not move to a square attacked by an opponent piece.
- Special case: **castling** (see Section 8).

### 7.3 Queen (`Q` / `q`)

- Moves **any number of squares** along a file, rank, or diagonal.
- Combines the movement capabilities of rook and bishop.
- Cannot jump over other pieces.

### 7.4 Rook (`R` / `r`)

- Moves **any number of squares** horizontally or vertically.
- Cannot jump over other pieces.

### 7.5 Bishop (`B` / `b`)

- Moves **any number of squares** diagonally.
- Always remains on its original square color.
- Cannot jump over other pieces.

### 7.6 Knight (`N` / `n`)

- Moves in an **L-shape**: two squares in one direction
  (horizontal or vertical), then one square perpendicular.
- The knight **jumps** over other pieces — intermediate squares
  do not matter.
- From e4: possible target squares are d6, f6, g5, g3, f2, d2, c3, c5.

### 7.7 Pawn (`P` / `p`)

Pawns are the most complex pieces:

**Forward movement (non-capturing):**

- One square forward to an **empty** square.
  - White: from rank n to rank n+1.
  - Black: from rank n to rank n−1.
- **Initial double-step:** If a pawn is still on its starting rank
  (White: rank 2, Black: rank 7), it may alternatively move **two squares**
  forward — but **only if both squares are empty**.

**Capturing:**

- Pawns capture **diagonally** one square forward to an adjacent file.
- They may **not** capture straight forward.

**En passant:**

- If an opponent pawn advances two squares in one move and passes through a
  square where your pawn could have captured it, your pawn may capture it
  **on the very next halfmove** as if it had moved only one square.
- The capturable square is given in the `en_passant` input field.
- **Important:** This special right expires immediately if not used
  on the immediately following move.

**Promotion:**

- If a pawn reaches the last rank (White: rank 8, Black: rank 1), it **must**
  be promoted on the same move to a queen, rook, bishop, or knight of the
  same color.
- The choice is not limited to captured pieces — e.g. a second queen is legal.
- The new piece takes effect immediately.
- In output: `"promotion"` contains the uppercase letter of the promoted piece
  (`"Q"`, `"R"`, `"B"`, `"N"`).

---

## 8. CASTLING (FIDE Art. 3.8.2)

Castling is a combined king-and-rook move and counts as **one** king move.

**Procedure:**

- The king moves **two squares** toward the rook.
- The rook jumps to the king’s side, onto the square the king
  just crossed.

**Kingside castling (short castling):**

- White: king e1 → g1, rook h1 → f1
- Black: king e8 → g8, rook h8 → f8

**Queenside castling (long castling):**

- White: king e1 → c1, rook a1 → d1
- Black: king e8 → c8, rook a8 → d8

### Conditions — castling is **permanently impossible** if

1. The king has already moved. (`castling.white.kingside` and
   `castling.white.queenside` = `false`)
2. The corresponding rook has already moved. (right for that side
   = `false`)

### Conditions — castling is **temporarily prevented** if

1. Any square between king and rook is occupied (i.e. not all required
   squares are free).
2. The king’s current square (`e1`/`e8`) is attacked
   (king is in check).
3. A square the king must cross is attacked by an opponent piece.
4. The king’s destination square is attacked by an opponent piece.

**Mnemonic:** The king may not castle out of, through, or into check.
The rook may, however, pass through or land on an attacked square.

---

## 9. CHECK, CHECKMATE, STALEMATE

### 9.1 Check

- The king is **in check** if it is attacked by one or more opponent pieces.
- A move that leaves or places your own king in check is **illegal**.
- You must get out of check on the next move by:
  a) capturing the attacking piece,
  b) interposing a piece (blocking),
  c) moving the king to a non-attacked square.

### 9.2 Checkmate — End of Game: Win

- The king is in check **and** there is no legal move.
- The side whose king is checkmated **loses**.
- In a mate position, the mating agent has already made its move —
  the system detects checkmate. The checkmated agent has no move and
  does not need to output anything.

### 9.3 Stalemate — End of Game: Draw

- The side to move is **not** in check, but has
  **no legal move at all**.
- The game ends immediately as a **draw**.

---

## 10. DRAW CONDITIONS (FIDE Art. 5 & 9)

### 10.1 Stalemate (automatic)

As described in Section 9.3.

### 10.2 Threefold Repetition

- If **the same position** (identical piece placement, same side to move,
  same castling rights, same en passant possibility) occurs **three times**,
  the side to move may **claim** a draw.
- From the **fifth** repetition onward, the draw is **mandatory**
  (without a claim).
- Use `position_history` for verification.
- The agent **should** claim a draw if it reaches threefold repetition in a
  losing position. It may also do so in balanced positions.
  Use the special output for this (see Section 11).

### 10.3 50-Move Rule

- If in the last **50 moves by each player** (= 100 halfmoves) there has been
  neither a pawn move nor a capture, the side to move may **claim** a draw.
- After **75 moves** by each player without a pawn move or capture, the draw
  is **mandatory** (without claim), unless the 75th move gives checkmate.
- Check via `halfmove_clock`. At ≥ 100, a draw can be claimed;
  at ≥ 150, it is enforced.

### 10.4 Insufficient Material (Dead Position)

The game is immediately drawn if no sequence of legal moves can lead to
checkmate (a so-called “dead position”). Mandatory draw in cases of:

- King vs King
- King + bishop vs King
- King + knight vs King
- King + bishop vs King + bishop (both bishops on same-colored squares)

> Note: Two knights vs king is not officially considered forced mate;
> in practice, checkmate can still occur with opponent cooperation.
> The system recognizes this as a “live position”.

### 10.5 Draw by Agreement

Both sides may offer and accept a draw. Output: see Section 11.

---

## 11. SPECIAL OUTPUTS (Non-Moves)

In exceptional situations, the agent returns a special JSON object
instead of a move:

**Draw offer / draw claim:**

```json
{ "action": "claim_draw", "reason": "threefold_repetition" }
{ "action": "claim_draw", "reason": "fifty_move_rule" }
{ "action": "offer_draw" }
```

**Resignation:**

```json
{ "action": "resign" }
```

> An agent **must** use `claim_draw` (not resign) when a draw condition
> is met and the position is clearly lost or hopeless.

---

## 12. LEGALITY CHECKS — AGENT RESPONSIBILITIES

Before outputting a move, the agent **must** perform the following checks:

1. **Own piece:** The `from` square contains one of your own pieces
   (matching `turn`).
2. **No own target:** The `to` square is not occupied by your own piece.
3. **Correct move pattern:** The move follows the piece movement rules
   (Sections 7 & 8).
4. **No jumping:** Bishop, rook, queen, pawn, and king (except castling)
   do not jump over pieces on their path.
5. **No self-check:** After the move, your own king is not in check
   (the move must be simulated on a virtual board).
6. **Castling conditions:** All castling conditions from Section 8 are met.
7. **En passant:** Allowed only if `en_passant` in input is not `null`
   and your pawn is on the correct square.
8. **Promotion:** If a pawn reaches the last rank, `promotion` **must**
   be set. `null` is not allowed.
9. **No promotion for non-pawns:** `promotion` must be `null`
   if no promotion occurs.

A move that violates any of these conditions is **illegal** and must
not be output.

---

## 13. END OF GAME — SUMMARY

| Event                                 | Result                  | Trigger          |
| ------------------------------------- | ----------------------- | ---------------- |
| Checkmate                             | Win for the attacker    | Automatic        |
| Stalemate                             | Draw                    | Automatic        |
| Threefold repetition (3×)             | Draw (claimable)        | Agent output     |
| Fivefold repetition (5×)              | Draw                    | Automatic        |
| 50-move rule (≥100 HM)                | Draw (claimable)        | Agent output     |
| 75-move rule (≥150 HM)                | Draw                    | Automatic        |
| Dead position / insufficient material | Draw                    | Automatic        |
| Agreement                             | Draw                    | Both sides agree |
| Resignation                           | Loss for resigning side | Agent output     |

---

## 14. IMPORTANT BEHAVIOR RULES

- **Output exactly one object.** Never output text, explanations, Markdown,
  or multipart responses — only JSON.
- **Always play legally.** No exceptions. Illegal moves lead to
  disqualification of the game.
- **Think completely.** Always verify that after your move, your king
  is not in check (simulate on a virtual board).
- **Recognize draws.** If the game reaches any draw condition,
  use the corresponding special output.
- **Never forget promotion.** A pawn that reaches the last rank
  must be promoted — typically to a queen (`"Q"`), unless there are
  tactical reasons for underpromotion.
- **En passant is time-critical.** If not used immediately, the right expires.

---

## 15. COMPLETE EXAMPLE (Move 1, Sicilian Defense)

### Input (after 1. e4 by White — Black to move)

```json
{
  "board": {
    "a1": "R", "b1": "N", "c1": "B", "d1": "Q", "e1": "K",
    "f1": "B", "g1": "N", "h1": "R",
    "a2": "P", "b2": "P", "c2": "P", "d2": "P",
    "f2": "P", "g2": "P", "h2": "P",
    "e4": "P",
    "a7": "p", "b7": "p", "c7": "p", "d7": "p", "e7": "p",
    "f7": "p", "g7": "p", "h7": "p",
    "a8": "r", "b8": "n", "c8": "b", "d8": "q", "e8": "k",
    "f8": "b", "g8": "n", "h8": "r"
  },
  "turn": "black",
  "castling": {
    "white": { "kingside": true, "queenside": true },
    "black": { "kingside": true, "queenside": true }
  },
  "en_passant": null,
  "halfmove_clock": 0,
  "fullmove_number": 1,
  "position_history": []
}
```

### Output (Black plays c5 — Sicilian)

```json
{ "from": "c7", "to": "c5", "promotion": null }
```

---

## 16. COMPLETE EXAMPLE (Castling)

### Input (White wants to castle kingside)

```json
{
  "board": {
    "a1": "R", "e1": "K", "h1": "R",
    "e8": "k"
  },
  "turn": "white",
  "castling": {
    "white": { "kingside": true, "queenside": true },
    "black": { "kingside": false, "queenside": false }
  },
  "en_passant": null,
  "halfmove_clock": 10,
  "fullmove_number": 6,
  "position_history": []
}
```

### Output (White short castling)

```json
{ "from": "e1", "to": "g1", "promotion": null }
```

---

## 17. COMPLETE EXAMPLE (En passant)

### Situation

White pawn on e5; Black pawn moved from d7 to d5
in the last halfmove (double-step). `en_passant` is `"d6"`.

### Output (White captures en passant)

```json
{ "from": "e5", "to": "d6", "promotion": null }
```

The black pawn on d5 is removed by the system.

---

## 18. COMPLETE EXAMPLE (Pawn Promotion)

### Situation

White pawn is on e7, White to move, square e8 is free.

### Output (promote to queen)

```json
{ "from": "e7", "to": "e8", "promotion": "Q" }
```

---

*Rule basis: FIDE Laws of Chess, effective January 1, 2023,
adopted at the 93rd FIDE Congress in Chennai, India.*
