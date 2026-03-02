# Chess Rules

Complete reference for the FIDE 2023 Laws of Chess as implemented by CheckAI.

## Piece Movement (FIDE Art. 3)

### General Rules

- No piece may move to a square occupied by a friendly piece
- Moving to a square with an opponent piece captures it
- Bishops, rooks, and queens cannot jump over other pieces
- Knights are the only pieces that can jump
- **No move may leave or place your own king in check**

### King (`K` / `k`)

- Moves exactly **one square** in any direction
- May not move to an attacked square
- Special: **castling** (see below)

### Queen (`Q` / `q`)

- Moves any number of squares along a file, rank, or diagonal
- Combines rook and bishop movement
- Cannot jump

### Rook (`R` / `r`)

- Moves any number of squares horizontally or vertically
- Cannot jump

### Bishop (`B` / `b`)

- Moves any number of squares diagonally
- Always stays on its starting square color
- Cannot jump

### Knight (`N` / `n`)

- Moves in an L-shape: two squares in one direction, then one square perpendicular
- **Jumps** over other pieces
- From `e4`: possible targets are `d6`, `f6`, `g5`, `g3`, `f2`, `d2`, `c3`, `c5`

### Pawn (`P` / `p`)

**Forward movement** (non-capturing):

- One square forward to an empty square
- **Initial double-step**: From the starting rank (2 for White, 7 for Black), may move two squares if both are empty

**Capturing**:

- Diagonally one square forward
- Cannot capture straight forward

**En passant**:

- If an opponent pawn double-steps past your pawn, you may capture it as if it moved one square
- Must be done on the very next half-move or the right expires
- The target square is given in the `en_passant` field

**Promotion**:

- A pawn reaching the last rank must be promoted to Q, R, B, or N
- Multiple queens (or other pieces) are allowed

## Castling (FIDE Art. 3.8.2)

Castling is a combined king-and-rook move that counts as one king move.

### Procedure

| Type        | King        | Rook        |
| ----------- | ----------- | ----------- |
| White O-O   | `e1` → `g1` | `h1` → `f1` |
| White O-O-O | `e1` → `c1` | `a1` → `d1` |
| Black O-O   | `e8` → `g8` | `h8` → `f8` |
| Black O-O-O | `e8` → `c8` | `a8` → `d8` |

### Permanently Impossible If

1. The king has already moved
2. The corresponding rook has already moved

### Temporarily Prevented If

1. Squares between king and rook are occupied
2. The king is currently in check
3. The king passes through an attacked square
4. The king's destination is attacked

> **Mnemonic:** The king may not castle out of, through, or into check. The rook may pass through or land on attacked squares.

## Check, Checkmate, Stalemate

### Check

- The king is attacked by one or more opponent pieces
- Must escape check immediately by:
  - Capturing the attacker
  - Blocking the attack
  - Moving the king

### Checkmate — Win

- The king is in check with no legal escape
- The checkmated side loses

### Stalemate — Draw

- The side to move is NOT in check but has no legal move
- The game ends as a draw

## Draw Conditions (FIDE Art. 5 & 9)

### Threefold Repetition

- Same position occurring 3 times → **claimable draw**
- Same position occurring 5 times → **automatic draw**
- "Same position" = identical pieces, same side to move, same castling rights, same en passant

### 50-Move Rule

- 50 moves by each side (100 half-moves) with no pawn move or capture → **claimable draw**
- 75 moves (150 half-moves) → **automatic draw** (unless the 75th move is checkmate)
- Check via `halfmove_clock`: ≥ 100 = claimable, ≥ 150 = enforced

### Insufficient Material (Dead Position)

Automatic draw when checkmate is impossible:

| Position                                    | Result |
| ------------------------------------------- | ------ |
| King vs King                                | Draw   |
| King + Bishop vs King                       | Draw   |
| King + Knight vs King                       | Draw   |
| King + Bishop vs King + Bishop (same color) | Draw   |

::: info
Two knights vs king is not considered a dead position — checkmate is possible with opponent cooperation.
:::

### Draw by Agreement

Both sides may offer and accept a draw via special actions.
