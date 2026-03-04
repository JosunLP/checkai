# Move Output

The agent returns **exactly one** JSON object — no text before or after, no Markdown, only raw JSON.

## Schema

```text
{
  "from": "<Square>",
  "to": "<Square>",
  "promotion": "<Symbol>" | null
}
```

## Fields

| Field       | Type           | Required | Description                                            |
| ----------- | -------------- | -------- | ------------------------------------------------------ |
| `from`      | String         | Yes      | Starting square of the piece, e.g. `"e2"`              |
| `to`        | String         | Yes      | Target square of the piece, e.g. `"e4"`                |
| `promotion` | String \| null | Yes      | Promotion piece (`"Q"`, `"R"`, `"B"`, `"N"`) or `null` |

::: warning Promotion Rules

- `promotion` uses **uppercase letters** regardless of which side is promoting
- Must be set when a pawn reaches the last rank (rank 8 for White, rank 1 for Black)
- Must be `null` for all non-promotion moves

:::

## Special Move Encoding

### Castling

Castling is encoded as a king move of two squares:

| Move            | `from` | `to`   |
| --------------- | ------ | ------ |
| White kingside  | `"e1"` | `"g1"` |
| White queenside | `"e1"` | `"c1"` |
| Black kingside  | `"e8"` | `"g8"` |
| Black queenside | `"e8"` | `"c8"` |

The rook movement is handled automatically by the system.

### En Passant

Encoded as a normal pawn diagonal move. The captured pawn is removed by the system.

```json
{ "from": "e5", "to": "d6", "promotion": null }
```

This captures the pawn on `d5` if `en_passant` was `"d6"`.

### Pawn Promotion

```json
{ "from": "e7", "to": "e8", "promotion": "Q" }
```

## Examples

```text
// Standard pawn move
{ "from": "e2", "to": "e4", "promotion": null }

// Knight move
{ "from": "g1", "to": "f3", "promotion": null }

// Kingside castling
{ "from": "e1", "to": "g1", "promotion": null }

// Queen promotion
{ "from": "d7", "to": "d8", "promotion": "Q" }

// Knight underpromotion
{ "from": "a7", "to": "a8", "promotion": "N" }
```

## Legality Checks

Before outputting a move, the agent **must** verify:

1. **Own piece** — The `from` square contains one of your own pieces
2. **No own target** — The `to` square is not occupied by your own piece
3. **Correct pattern** — The move follows piece movement rules
4. **No jumping** — No illegal jumps over pieces (except knight)
5. **No self-check** — After the move, your king is not in check
6. **Castling conditions** — All conditions from the rules are met
7. **En passant validity** — Only when `en_passant` is set and your pawn is positioned correctly
8. **Promotion required** — Must promote when reaching the last rank
9. **No false promotion** — `promotion` must be `null` when not promoting
