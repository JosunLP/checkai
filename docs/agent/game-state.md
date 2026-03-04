# Game State (Input)

Before each move, the agent receives a JSON object with the complete game state.

## Schema

```text
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

## Fields

| Field              | Type            | Description                                                                                                                    |
| ------------------ | --------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `board`            | Object          | Contains only **occupied** squares. Key = square name (`"e4"`), value = piece symbol. Empty squares are not listed.            |
| `turn`             | String          | `"white"` or `"black"` — side to move.                                                                                         |
| `castling`         | Object          | Castling rights. `true` = right still available (king and rook never moved), `false` = right lost.                             |
| `en_passant`       | String \| null  | If a pawn advanced two squares in the last move, this is the skipped square (the en passant capture target). Otherwise `null`. |
| `halfmove_clock`   | Number          | Halfmoves since the last pawn move or capture. Used for the 50-move rule.                                                      |
| `fullmove_number`  | Number          | Full-move counter. Starts at 1, incremented after each Black move.                                                             |
| `position_history` | Array\<String\> | All previous positions as simplified FEN strings (without move numbers), for threefold repetition detection.                   |

## Example: Starting Position

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

## Key Notes

- The `board` object only contains occupied squares — an empty square simply isn't present as a key
- `position_history` grows throughout the game and is essential for detecting threefold repetition
- `halfmove_clock` resets to 0 on every pawn move or capture
- `en_passant` is only set for one half-move after a double pawn push, then reverts to `null`
