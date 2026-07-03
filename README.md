# Grandmaster Forge

A Rust desktop chess training platform combining ideas from Lichess, Chess.com, AIMchess, ChessReps, and ChessIgma — opening repertoire tree builder, per-move-edge spaced-repetition drills, play vs a book-driven bot, game review with engine analysis, weakness profiling, puzzles, and Lichess + Chess.com sync.

---

## Features

### Opening Repertoire Builder
- Interactive 8×8 board with two-click move entry, undo, and reset
- White/Black color picker — the trainer knows which side you play
- Name and save lines directly to your repertoire
- "Suggest from Last PGN" extracts the first 15 opening moves from your most recently imported game (SAN→UCI via shakmaty) and pre-loads them for editing

### Import Lines (PGN variations + Lichess studies)
- Paste multi-game PGN with nested variations — a recursive walker flattens every branch into normalized-FEN move edges
- "Sync My Lichess Studies" pulls your studies through the same import pipeline
- Each imported edge is tagged with your repertoire side and becomes immediately drillable

### Explorer Adopt
- Browse the Lichess opening explorer from any board position
- "Adopt" a master/community move to graft it straight into your repertoire tree as a new edge

### SM-2 Spaced Repetition Drills
- SM-2 state lives on each **move edge** of the repertoire tree (one card per position→move), not per whole line: interval, ease factor, repetition count, due date
- Pass → grade 5: interval grows per SM-2, confidence ticks up
- Fail → grade 1: interval resets to 1 day, ease decays — a single wrong branch schedules just that edge for review, not the entire line
- Daily queue shows only edges due today, sorted by due date

### Color-Aware Drill Engine
- Black repertoire lines: engine auto-plays White's first move before waiting for your reply
- White repertoire lines: you move first as normal

### Branch Quizzing
- At the first ply of a drill, all lines sharing the same start position are loaded
- Any of your repertoire responses is accepted as correct
- Branch count displayed in the UI

### Play vs Book Bot
- Play a full game against a bot that follows *your* prepared repertoire while both sides are in book, branching uniformly at random among your stored opponent replies
- When the position leaves your prep, a Stockfish `PlaySession` takes over at your chosen Elo (clamped to a 1320 floor) — the UI flips between "In book (your prep)" and "Out of book (Stockfish)"
- Deviating from a known position grades every expected my-move edge as a lapse (SM-2 grade 1) and records the deviation; play continues so you can finish the game
- Resign or reach game-over to get a summary (plies in book, where you left book, deviations now due for review); "Review This Game" hands the game to Game Review for full engine analysis

### Game Review
- Import PGN; Stockfish analyses every position in a background thread
- Centipawn loss per ply, mistake classification (Inaccuracy / Mistake / Blunder)
- Move timeline with board visualization
- First critical mistake generates a corrective repertoire line automatically

### Mistake Trees
- Recurring blunders are grouped by the position they occur in, not treated as one-off errors
- Each mistake position becomes a corrective node in the repertoire tree so the fix is drilled like any other line

### Accuracy Scoring
- Chess.com formula: `103.1668 × e^(−0.04354 × ACPL) − 3.1669` clamped to [0, 100]
- Phase-aware breakdown: Opening (plies 1–30), Middlegame (31–70), Endgame (71+)
- Per-game accuracy stored in DB for historical tracking

### Weakness Profiler
- Aggregates positions across all games to find ECO openings with highest average centipawn loss
- Detects recurring blunder move-number clusters
- Flags endgame weakness when endgame accuracy < 70%
- Dashboard shows top training priority

### Lichess Sync
- Streams up to 50 recent games via Lichess NDJSON API in a background thread
- Deduplication via the per-source `game_sync` ledger — already-synced games are skipped
- Status shown in UI; `LICHESS_API_KEY` / `LICHESS_USERNAME` loaded from `.env`

### Chess.com Sync
- Pulls your recent Chess.com monthly archives via the public API in a background thread
- Same `game_sync` dedup ledger keyed by source, so re-syncing only imports new games
- Uses `CHESSCOM_USERNAME` and a required `CHESSCOM_USER_AGENT` (Chess.com's API demands a contact User-Agent)

### Opening Tree (Master Games)
- BFS expansion of the Lichess master-games explorer up to configurable depth
- Minimum game threshold filters noise
- Nodes stored in `opening_tree` table with win/draw/loss counts

### Puzzle Training
- Puzzles stored in DB with FEN, solution UCI, theme, and difficulty
- `load-puzzle` callback picks a random puzzle up to a difficulty ceiling and sets up the board

---

## Architecture

```
Grandmaster-Forge-Rust/
├── app/                  # Slint desktop app — main binary, all UI and callbacks
│   └── src/
│       ├── main.rs       # slint! macro, AppState, all on_* callback wiring
│       ├── tree.rs       # Repertoire position tree: position_key, edge insert/walk, grade_edge, legacy migration
│       ├── srs.rs        # SM-2 algorithm (review(), SrsCard)
│       ├── accuracy.rs   # Chess.com accuracy formula, phase breakdown
│       ├── weakness.rs   # WeaknessReport builder
│       └── scraper.rs    # Async BFS opening tree (no Mutex across .await)
├── db_manager/           # SQLite schema, all CRUD, migrations
├── engine_controller/    # Stockfish UCI process management, multi-PV analysis, play sessions
├── pgn_processor/        # PGN parser, SAN extractor, recursive variation walker, mistake classifier
├── lichess_client/       # Lichess REST v2: games (NDJSON), puzzles, explorer, studies
├── chesscom_client/      # Chess.com public API: monthly archive game import
└── mastery_ui/           # Training-mode routing and recommendation logic
```

**Tech stack**
| Concern | Library |
|---|---|
| GUI | Slint 1.6 |
| Chess logic | shakmaty 0.26 (FEN, SAN, UCI, position) |
| Database | rusqlite 0.31 (bundled SQLite) |
| HTTP / async | reqwest 0.12, tokio rt-multi-thread |
| NDJSON streaming | futures-util 0.3 |
| Environment | dotenvy 0.15 |

---

## Workspace Crates

| Crate | Role |
|---|---|
| `app` | Slint UI, AppState, all callback wiring |
| `db_manager` | SQLite schema, migrations, all CRUD methods |
| `engine_controller` | Stockfish UCI: spawn, analyze, parse scores |
| `pgn_processor` | PGN parse → `ParsedGame`, SAN move list, recursive variation walker, mistake detection |
| `lichess_client` | Games (NDJSON stream), puzzles, opening explorer, studies |
| `chesscom_client` | Chess.com public API: monthly archive game import |
| `mastery_ui` | `recommend_next_action()`, `UserSnapshot` training router |

---

## Database Schema

| Table | Purpose |
|---|---|
| `games` | Imported/synced games with accuracy columns |
| `positions` | Per-ply FEN, centipawn loss, mistake class |
| `repertoire_nodes` | Repertoire tree positions: normalized FEN + side (White/Black) |
| `repertoire_moves` | Move edges between nodes, each carrying its own SM-2 state (interval/ease/reps/due) and is-my-move flag |
| `opening_tree` | Master-game BFS tree nodes |
| `puzzles` | Puzzle positions with solution and difficulty |
| `training_events` | Log of every drill/puzzle/review outcome |
| `game_sync` | Per-source dedup ledger for synced game IDs (Lichess + Chess.com) |
| `opening_lines` | *Retired* — legacy repertoire lines; migrated into `repertoire_nodes`/`repertoire_moves` on startup, then no longer written |

---

## Setup

```bash
# Prerequisites: Rust stable, Stockfish in PATH or adjacent binary

# Clone and build
git clone <repo>
cd Grandmaster-Forge-Rust
cargo build --release

# Create .env with your Lichess API token
echo "LICHESS_API_KEY=lip_yourtoken" > .env

# Run
cargo run --release -p app
```

---

## Environment

| Variable | Description |
|---|---|
| `LICHESS_API_KEY` | Personal Lichess API token (read:games scope) |
| `LICHESS_USERNAME` | Your Lichess username, used to sync your games/studies |
| `CHESSCOM_USERNAME` | Your Chess.com username, used to pull monthly archives |
| `CHESSCOM_USER_AGENT` | Contact User-Agent string required by the Chess.com public API (e.g. `Forge/1.0 you@example.com`) |

---

## Tests

```bash
cargo test --workspace
# 48 tests across db_manager, app (srs, accuracy, weakness, tree), engine_controller, lichess_client, chesscom_client, pgn_processor
```
