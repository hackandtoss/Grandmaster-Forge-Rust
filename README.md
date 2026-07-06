# Grandmaster Forge

A Rust desktop chess training platform combining ideas from Lichess, Chess.com, AIMchess, ChessReps, and ChessIgma — a position-tree opening repertoire with per-move spaced repetition, multi-source line import, game review with engine analysis, weakness profiling, puzzles, book-bot play, and Lichess + Chess.com game sync.

---

## Features

### Repertoire Position Tree
- The repertoire is a graph of positions (`repertoire_nodes`) and move edges (`repertoire_moves`), not flat move lists
- Positions are keyed by normalized FEN (clocks zeroed), so transpositions dedupe automatically
- Every edge knows whose move it is (`is_my_move`) and where it came from (`manual`, `pgn`, `study`, `explorer`, `mistake`)
- Legacy `opening_lines` rows are migrated into the tree once at startup, then the old table is retired

### Per-Move SM-2 Spaced Repetition
- SM-2 state (interval, ease, repetitions, due date) lives on each of *your* move edges, not on whole lines
- Pass → grade 5: interval grows per SM-2; fail → grade 1: interval resets to 1 day, ease decays
- The daily queue lists due edges sorted by due date; drilling starts from the edge's parent position — mid-game roots included

### Tree Drills with Branch Quizzing
- At any node, every outgoing my-move edge is an accepted answer; the drill follows the branch you actually play
- Opponent replies are auto-played at random from your prepared branches
- Color-aware: Black repertoires get the opponent's move first

### Import Lines (PGN + Lichess Studies)
- Paste any PGN — nested recursive variations and multi-game files included — and every branch is walked into the tree
- One-click "Sync My Lichess Studies" pulls all your studies as variation PGN into the same pipeline
- Pick which side (White/Black) the repertoire is for per import

### Opening Repertoire Builder
- Interactive 8×8 board with two-click move entry, undo, and reset
- White/Black color picker — the trainer knows which side you play
- Saves directly to the position tree (`source = 'manual'`)
- "Suggest from Last PGN" pre-loads the first 15 opening moves of your most recent game
- "Adopt Master Lines from Current Position": walks the Lichess masters explorer from the current builder position — your side takes the most popular master move, the opponent side branches into the top-3 replies

### Play vs Book Bot
- New "Play vs Bot" screen: color picker, bot Elo (1320–3190 via `UCI_LimitStrength`/`UCI_Elo`), New Game / Resign
- While in book, the bot plays random branches from *your* repertoire, so you face your whole prep in unpredictable order
- Out of book, a persistent Stockfish `PlaySession` takes over at the chosen strength
- Deviating from your prep silently records a failed review (grade 1) — fumbled lines re-enter tomorrow's drill queue
- Post-game summary shows plies in book, where each side left book, and every deviation with the expected moves
- "Review This Game" feeds the finished game straight into the import/analysis pipeline

### Game Review
- Import PGN; Stockfish analyses every position in a background thread
- Centipawn loss per ply, mistake classification (Inaccuracy / Mistake / Blunder)
- Move timeline with board visualization

### Mistake Trees
- After each analyzed game, the ply where *your* eval dropped most (≥ 60 cp) roots a mini repertoire tree:
  the engine's correction, the top-3 opponent replies (multi-PV), and your best follow-up to each — 7 edges, due immediately

### Accuracy Scoring
- Chess.com formula: `103.1668 × e^(−0.04354 × ACPL) − 3.1669` clamped to [0, 100]
- Phase-aware breakdown: Opening (plies 1–30), Middlegame (31–70), Endgame (71+)
- Per-game accuracy stored in DB for historical tracking

### Weakness Profiler
- Aggregates positions across all games to find ECO openings with highest average centipawn loss
- Detects recurring blunder move-number clusters
- Flags endgame weakness when endgame accuracy < 70%
- Dashboard shows top training priority

### Game Sync (Lichess + Chess.com)
- Lichess: streams up to 50 recent games via NDJSON API and imports them into `games`/`positions` for review
- Chess.com: public monthly-archive API (no token), newest months first, capped at 50 new games per sync
- Cross-source dedup via the `game_sync (source, external_id)` ledger — re-syncs are idempotent
- Both run in background threads and post status to the dashboard

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
│       ├── tree.rs       # Repertoire-tree walking: edge insert, imports, SM-2 grading, migration
│       ├── srs.rs        # SM-2 algorithm (review(), SrsCard)
│       ├── accuracy.rs   # Chess.com accuracy formula, phase breakdown
│       ├── weakness.rs   # WeaknessReport builder
│       └── scraper.rs    # Async BFS opening tree + explorer adopt walk
├── db_manager/           # SQLite schema, all CRUD, migrations, FEN normalization
├── engine_controller/    # Stockfish UCI: one-shot analysis + persistent PlaySession
├── pgn_processor/        # PGN parser, SAN extractor, recursive variation walker
├── lichess_client/       # Lichess REST v2: games (NDJSON), puzzles, explorer, studies
├── chesscom_client/      # Chess.com public API: monthly archives, games, dedup ids
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
| `app` | Slint UI, AppState, tree module, all callback wiring |
| `db_manager` | SQLite schema, migrations, all CRUD methods |
| `engine_controller` | Stockfish UCI: spawn, analyze, parse scores, `PlaySession` |
| `pgn_processor` | PGN parse → `ParsedGame`, variation walker, mistake detection |
| `lichess_client` | Games (NDJSON stream), puzzles, opening explorer, studies export |
| `chesscom_client` | Monthly game archives via the public API |
| `mastery_ui` | `recommend_next_action()`, `UserSnapshot` training router |

---

## Database Schema

| Table | Purpose |
|---|---|
| `games` | Imported/synced games with accuracy columns |
| `positions` | Per-ply FEN, centipawn loss, mistake class |
| `repertoire_nodes` | Repertoire positions: normalized FEN + side, unique per (fen, side) |
| `repertoire_moves` | Move edges with ownership, source, and per-edge SM-2 state |
| `game_sync` | Cross-source dedup ledger keyed by (source, external_id) |
| `opening_tree` | Master-game BFS tree nodes |
| `puzzles` | Puzzle positions with solution and difficulty |
| `training_events` | Log of every drill/puzzle/review outcome |

Legacy note: `opening_lines` is migrated into the tree on first launch and renamed to
`opening_lines_legacy`; the old `lichess_sync` ledger is folded into `game_sync`.

---

## Setup

```bash
# Prerequisites: Rust stable, Stockfish in PATH or adjacent binary

# Clone and build
git clone <repo>
cd Grandmaster-Forge-Rust
cargo build --release

# Create .env with your accounts
cat > .env <<EOF
LICHESS_API_KEY=lip_yourtoken
LICHESS_USERNAME=yourlichessname
CHESSCOM_USERNAME=yourchesscomname
EOF

# Run
cargo run --release -p app
```

---

## Environment

| Variable | Description |
|---|---|
| `LICHESS_API_KEY` | Personal Lichess API token (read:games + study:read scopes) |
| `LICHESS_USERNAME` | Lichess account for game sync, studies, and user-side detection |
| `CHESSCOM_USERNAME` | Chess.com account for archive sync (public API, no token) |
| `CHESSCOM_USER_AGENT` | Optional User-Agent for the Chess.com API (a sensible default is built in) |
| `STOCKFISH_PATH` | Optional explicit path to a Stockfish binary |

---

## Tests

```bash
cargo test --workspace
# 47 tests across db_manager, app (tree, srs, accuracy, weakness, scraper),
# engine_controller (incl. PlaySession vs mock engine), lichess_client,
# chesscom_client, and pgn_processor
```
