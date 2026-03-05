# Examples

Complete worked examples showing the agent protocol in action.

## Example 1: Opening Move (Sicilian Defense)

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
  "en_passant": "e3",
  "halfmove_clock": 0,
  "fullmove_number": 1,
  "position_history": []
}
```

### Output (Black plays 1...c5 — Sicilian)

```json
{ "from": "c7", "to": "c5", "promotion": null }
```

Note: `en_passant` is `"e3"` because White's pawn moved two squares from `e2` to `e4`.

---

## Example 2: Kingside Castling

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

### Output (White O-O)

```json
{ "from": "e1", "to": "g1", "promotion": null }
```

The system automatically moves the rook from `h1` to `f1`.

---

## Example 3: En Passant

### Situation

White pawn on `e5`; Black just moved a pawn from `d7` to `d5` (double-step). The `en_passant` field is `"d6"`.

### Output (White captures en passant)

```json
{ "from": "e5", "to": "d6", "promotion": null }
```

The system removes the Black pawn on `d5`.

---

## Example 4: Pawn Promotion

### Situation

White pawn on `e7`, White to move, square `e8` is free.

### Output (Promote to queen)

```json
{ "from": "e7", "to": "e8", "promotion": "Q" }
```

### Output (Knight underpromotion)

```json
{ "from": "e7", "to": "e8", "promotion": "N" }
```

---

## Example 5: Threefold Repetition Draw Claim

### Situation

The `position_history` array contains three occurrences of the current position hash. Both sides have been shuffling pieces back and forth.

### Output

```json
{ "action": "claim_draw", "reason": "threefold_repetition" }
```

---

## Example 6: Fifty-Move Rule Draw Claim

### Situation

The `halfmove_clock` is 100 (50 full moves without a pawn move or capture).

### Output

```json
{ "action": "claim_draw", "reason": "fifty_move_rule" }
```

---

## Example 7: Resignation

### Output

```json
{ "action": "resign" }
```

---

## Agent Behavior Rules

1. **Output exactly one object** — no text, no Markdown, only JSON
2. **Always play legally** — illegal moves result in game disqualification
3. **Simulate moves** — Always verify your king is safe after your move
4. **Recognize draws** — Use the correct special action when a draw condition is met
5. **Never forget promotion** — A pawn on the last rank must be promoted
6. **En passant is time-critical** — If not used immediately, the right expires
