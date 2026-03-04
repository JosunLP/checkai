# Agent Protocol Overview

CheckAI defines a JSON-based protocol for AI agents to play chess. This protocol is the contract between the server and any AI agent connecting to it.

> **Rule Basis:** FIDE Laws of Chess, effective January 1, 2023, adopted at the 93rd FIDE Congress in Chennai, India.

## Role of the Agent

You are a chess-playing agent. Before each move, you receive the **complete game state** as a JSON object. You must return **exactly one legal move** as a JSON object.

- Illegal moves are never acceptable
- Output must be raw JSON — no text, no Markdown, no explanations
- Analyze the position, calculate all legal moves, and choose one

## Communication Flow

```bash
Server                              Agent
  │                                   │
  │  ── Game State (JSON) ──────────► │
  │                                   │  Analyze position
  │                                   │  Calculate legal moves
  │  ◄── Move Output (JSON) ──────── │
  │                                   │
  │  Validate move                    │
  │  Update board                     │
  │  Check game-end conditions        │
  │                                   │
  │  ── Updated State (JSON) ───────► │
  │                                   │
```

## Protocol Summary

| Direction      | Format | Content                          |
| -------------- | ------ | -------------------------------- |
| Server → Agent | JSON   | Complete game state              |
| Agent → Server | JSON   | One legal move or special action |

## The Chessboard

- 8×8 grid with 64 squares
- **Files** (columns): `a` to `h`
- **Ranks** (rows): `1` to `8` (1 = White home, 8 = Black home)
- Squares: two-character strings like `"e4"`, `"a1"`, `"h8"`

## Piece Symbols

Following the FEN convention (uppercase = White, lowercase = Black):

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

## Next Steps

- [Game State (Input)](./game-state) — JSON schema the agent receives
- [Move Output](./move-output) — JSON schema the agent returns
- [Chess Rules](./chess-rules) — Complete FIDE 2023 rule reference
- [Special Actions](./special-actions) — Draw claims, resignation
- [Examples](./examples) — Full worked examples
