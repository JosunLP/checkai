# The Search Engine

CheckAI ships a single search engine that every surface shares: the CLI, the
UCI adapter, the REST analysis API, the web and desktop UIs, and the
WebAssembly build behind the npm package. The same source file is compiled
into all of them — there is no "lite" version.

This page describes what the engine does, how to configure it, and which knobs
matter for which use case.

## At a glance

| Property           | Value                                                     |
| ------------------ | --------------------------------------------------------- |
| Search             | Principal Variation Search with iterative deepening        |
| Parallelism        | Lazy SMP, 1–64 threads (native only)                       |
| Transposition table| Lock-free, 16 bytes per slot, shared across threads        |
| MultiPV            | 1–16 principal variations                                  |
| Knowledge          | Polyglot opening book, Syzygy tablebase probing            |
| Strength limiting  | Skill 0–20, or a target `UCI_Elo`                          |
| Maximum depth      | 128 plies                                                  |

## Configuration

Every engine-backed CLI command accepts the same option group:

```bash
checkai play      --threads 4 --hash 256 --book book.bin
checkai watch     --threads 4 --multipv 3
checkai analyze   --pgn game.pgn --threads 8 --hash 512
checkai eval      --fen "<FEN>" --multipv 10
checkai bench     --threads 4
```

| Flag           | Meaning                                                     | Default |
| -------------- | ----------------------------------------------------------- | ------- |
| `--threads N`  | Lazy SMP search threads; `0` means one per CPU core          | `1`     |
| `--hash MB`    | Transposition table size                                     | per command |
| `--nodes N`    | Node budget per search                                       | unlimited |
| `--multipv N`  | Number of principal variations to report (1–16)              | `1`     |
| `--book FILE`  | Polyglot opening book (`.bin`)                               | none    |
| `--book-best`  | Always play the most popular book move instead of sampling   | off     |
| `--tablebase D`| Syzygy tablebase directory                                   | none    |

The same settings are available over UCI (`Threads`, `Hash`, `MultiPV`,
`OwnBook`, `BookFile`, `SyzygyPath`, …) — see [CLI Commands](/guide/cli) for
the full option table — and over the REST API through
`POST /api/analysis/position`.

## How a search runs

```text
             ┌──────────────┐
  position → │ opening book │ ── hit ──▶ book move, zero nodes searched
             └──────┬───────┘
                    │ miss
             ┌──────▼───────┐
             │  tablebase   │ ── in range ──▶ verdict attached to the result
             └──────┬───────┘
                    │
             ┌──────▼───────────────────────────────┐
             │ iterative deepening, depth 1 … N     │
             │  ├─ aspiration window per MultiPV    │
             │  ├─ root move loop (LMR at the root) │
             │  └─ alpha-beta / PVS tree            │
             └──────────────────────────────────────┘
```

Each completed iteration reports a snapshot — depth, selective depth, score,
node count, node rate, hash usage and the principal variation — which is what
the CLI's live panel and the UCI `info` lines display.

### Inside the tree

The alpha-beta search implements the techniques a modern engine is expected to
have. Grouped by what they do:

**Cutting the tree short**

- Transposition table cutoffs with depth-preferred, generation-aged replacement
- Mate-distance pruning
- Reverse futility pruning (static null move), relaxed when the side to move is improving
- Adaptive null-move pruning with a verification search at high depth
- Razoring into quiescence at shallow depth
- Futility pruning of quiet frontier moves
- Late move pruning, tightened on non-improving nodes
- History pruning of quiets that have repeatedly failed
- Static Exchange Evaluation pruning of losing captures

**Spending effort where it matters**

- Check extensions
- Singular extensions: when the transposition move is provably better than every
  alternative, it gets an extra ply
- Late Move Reductions from a precomputed log-log table, adjusted for PV nodes,
  killers, counter-moves, history scores and the improving flag
- Internal Iterative Reduction when no transposition move is available

**Move ordering**

1. Transposition-table move
2. Queen promotions
3. Winning captures (SEE ≥ 0), ordered by MVV-LVA
4. Killer moves
5. Counter-move of the opponent's previous move
6. Quiet moves by butterfly history + one-ply continuation history
7. Losing captures (SEE < 0)

**Leaf handling**

Quiescence search resolves captures, promotions and check evasions with
stand-pat, delta pruning, SEE pruning and its own transposition-table entries,
so the static evaluation is only trusted in quiet positions.

## Time management

Two budgets steer a timed search:

- The **hard limit** is enforced inside the tree and never exceeded.
- The **soft limit** (55% of the hard limit by default) gates the *start* of a
  new iteration. It stretches by 65% while the best root move keeps changing,
  and shrinks to 70% once the same move has survived five iterations in a row.

The net effect: quiet positions are answered quickly, and the engine spends its
time where the answer is still in doubt. `Move Overhead` (UCI) subtracts a fixed
latency allowance from every budget so a GUI connection never causes a flag fall.

## Lazy SMP

With `--threads N` the engine starts `N-1` helper workers. Every worker searches
the same position with its own killer, history and continuation tables, but they
all share one transposition table, so a line proven by one thread immediately
speeds up the others. Helpers start one ply ahead on odd thread ids, which
desynchronises their trees — that diversity is what makes the scheme pay off.

Two consequences worth knowing:

- **Node counts stop being comparable.** `checkai bench` prints a node signature
  only in single-threaded mode; threaded runs should be compared on time.
- **Results stop being bit-reproducible.** Single-threaded search is fully
  deterministic; Lazy SMP is not, by design.

WebAssembly builds have no worker threads, so the thread count is clamped to 1
there regardless of what is requested.

## Strength limiting

Two independent mechanisms weaken the engine:

- **Depth and time caps** make it short-sighted.
- **Skill limiting** (`0`–`20`) makes it choose from a band of near-best moves
  instead of always the very best one.

The second matters more for playability. A pure depth cap produces uniformly
short-sighted, robotic play; a skill limit produces the occasional human-looking
inaccuracy on top of otherwise sensible moves. The CLI's difficulty ladder
combines both — see the table in [CLI Commands](/guide/cli#difficulty-levels).

Over UCI, set `UCI_LimitStrength` plus `UCI_Elo` (800–2850), or set
`Skill Level` directly.

## Evaluation

The evaluation is a tapered PeSTO-style function that interpolates between
midgame and endgame terms by game phase:

- Material and piece-square tables
- Bishop pair, rooks on open and semi-open files
- Pawn structure: doubled, isolated, backward, connected and passed pawns
- King safety: pawn shield, open files near the king, piece tropism
- Mobility per piece type
- Space and a tempo bonus

Run `checkai eval --fen "<FEN>"` to see the static evaluation, the ranked move
list, the search statistics, and any book or tablebase knowledge for a position.

## Verifying a build

```bash
# Move generation: exact node counts against the known references
checkai perft 5
checkai perft 6 --threads 0

# Search: a fixed twelve-position suite
checkai bench --depth 12

# Tactics: does it find the point of a position?
checkai eval --fen "2rr3k/pp3pp1/1nnqbN1p/3pN3/2pP4/2P3Q1/PPB4P/R4RK1 w - - 0 1"
```

`checkai bench` prints a **bench signature** — the total node count over the
suite. In single-threaded mode that number is deterministic, so comparing it
between two builds shows at a glance whether a change altered the search.
