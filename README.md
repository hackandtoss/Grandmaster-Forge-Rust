# Grandmaster Forge

A Rust desktop chess training platform combining ideas from Lichess, Chess.com, AIMchess, ChessReps, and ChessIgma — opening repertoire builder, spaced-repetition drills, game review with engine analysis, weakness profiling, puzzles, and Lichess sync.

---

## Features

### Opening Repertoire Builder
- Interactive 8×8 board with two-click move entry, undo, and reset
- White/Black color picker — the trainer knows which side you play
- Name and save lines directly to your repertoire
- "Suggest from Last PGN" extracts the first 15 opening moves from your most recently imported game (SAN→UCI via shakmaty) and pre-loads them for editing

### SM-2 Spaced Repetition Drills
- Every line carries SM-2 state: interval, ease factor, repetition count, due date
- Pass → grade 5: interval grows per SM-2, confidence ticks up
- Fail → grade 1: interval resets to 1 day, ease decays
- Daily queue shows only lines due today, sorted by due date

### Color-Aware Drill Engine
- Black repertoire lines: engine auto-plays White's first move before waiting for your reply
- White repertoire lines: you move first as normal

### Branch Quizzing
- At the first ply of a drill, all lines sharing the same start position are loaded
- Any of your repertoire responses is accepted as correct
- Branch count displayed in the UI

### Game Review
- Import PGN; Stockfish analyses every position in a background thread
- Centipawn loss per ply, mistake classification (Inaccuracy / Mistake / Blunder)
- Move timeline with board visualization
- First critical mistake generates a corrective repertoire line automatically

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
- Deduplication via `lichess_sync` table — already-synced games are skipped
- Status shown in UI; `LICHESS_API_KEY` loaded from `.env`

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
│       ├── srs.rs        # SM-2 algorithm (review(), SrsCard)
│       ├── accuracy.rs   # Chess.com accuracy formula, phase breakdown
│       ├── weakness.rs   # WeaknessReport builder
│       └── scraper.rs    # Async BFS opening tree (no Mutex across .await)
├── db_manager/           # SQLite schema, all CRUD, migrations
├── engine_controller/    # Stockfish UCI process management, multi-PV analysis
├── pgn_processor/        # PGN parser, SAN extractor, mistake classifier
├── lichess_client/       # Lichess REST v2: games (NDJSON), puzzles, explorer
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
| `pgn_processor` | PGN parse → `ParsedGame`, SAN move list, mistake detection |
| `lichess_client` | Games (NDJSON stream), puzzles, opening explorer |
| `mastery_ui` | `recommend_next_action()`, `UserSnapshot` training router |

---

## Database Schema

| Table | Purpose |
|---|---|
| `games` | Imported/synced games with accuracy columns |
| `positions` | Per-ply FEN, centipawn loss, mistake class |
| `opening_lines` | Repertoire lines with SM-2 state + side (White/Black) |
| `opening_tree` | Master-game BFS tree nodes |
| `puzzles` | Puzzle positions with solution and difficulty |
| `training_events` | Log of every drill/puzzle/review outcome |
| `lichess_sync` | Dedup ledger for synced Lichess game IDs |

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

---

## Tests

```bash
cargo test --workspace
# 26 tests across db_manager, app (srs, accuracy, weakness), lichess_client, pgn_processor
```
