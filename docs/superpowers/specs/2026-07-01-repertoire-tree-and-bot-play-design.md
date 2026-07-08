# Repertoire Tree, Multi-Source Line Import, and Book-Bot Play — Design

**Date:** 2026-07-01
**Status:** Approved by user (brainstorming dialogue, all sections)
**Accounts:** Lichess `hackandtoss` (token in `.env` as `LICHESS_API_KEY`), Chess.com `ElectricMindGames` (public API, no token)

> **Repo drift notice (2026-07-07): BASELINE SPEC.**
> This spec remains the source for the graph migration, multi-source import, and book-bot baseline. It is not the final opening UX.
> Current opening direction is Course -> Chapter -> Named Line -> Graph Path, with ChessReps-style catalog/practice UI backed by this graph; see `2026-07-07-opening-course-graph-design.md`.
> SM-2 in this spec is current-state scheduling. FSRS is the target replacement once the review/event model is ready.
> Normalized FEN remains canonical; any future Zobrist key is an index/cache, not a separate identity model.

## Goal

Complete Grandmaster Forge into a system that (1) imports the user's games from
both Lichess and Chess.com, (2) supports structured opening learning with deep,
branching lines from multiple sources including manual entry, and (3) provides a
bot-play mode that dynamically plays the opponent side of learned lines before
handing over to Stockfish at a chosen strength.

Build order (user-chosen, "data first"):

1. Repertoire tree data model + migration
2. PGN variation import (manual add) + Chess.com game sync
3. Mistake-driven line generation from the user's own games
4. Lichess studies sync + explorer adopt-into-repertoire
5. Book-bot + Stockfish play mode

## Architecture decision

**Approach B — full position tree** (user-selected over flat-lines and
minimal-bolt-on alternatives). The repertoire becomes a graph of positions and
move edges instead of flat move-list rows. Transpositions dedupe automatically;
"any repertoire move is a correct answer" falls out of the model; the bot's book
is a direct position lookup. Cost: schema migration and a drill-engine rewrite.

## 1. Data model (`db_manager`)

### New tables

```sql
CREATE TABLE repertoire_nodes (
  id    INTEGER PRIMARY KEY,
  fen   TEXT NOT NULL,   -- normalized: piece placement, side to move,
                         -- castling rights, en passant square; move clocks
                         -- DROPPED so transpositions collide intentionally
  side  TEXT NOT NULL CHECK (side IN ('white','black')),  -- which repertoire
  UNIQUE (fen, side)
);

CREATE TABLE repertoire_moves (
  id            INTEGER PRIMARY KEY,
  parent_id     INTEGER NOT NULL REFERENCES repertoire_nodes(id),
  child_id      INTEGER NOT NULL REFERENCES repertoire_nodes(id),
  uci           TEXT NOT NULL,
  san           TEXT NOT NULL,
  is_my_move    INTEGER NOT NULL,   -- 1 = repertoire owner's move
  source        TEXT NOT NULL CHECK (source IN
                  ('manual','pgn','study','explorer','mistake')),
  comment       TEXT,
  -- SM-2 state; meaningful only when is_my_move = 1
  interval_days REAL,
  ease          REAL,
  reps          INTEGER,
  due_date      TEXT,
  UNIQUE (parent_id, uci)
);

CREATE TABLE game_sync (
  source       TEXT NOT NULL,      -- 'lichess' | 'chesscom'
  external_id  TEXT NOT NULL,
  synced_at    TEXT NOT NULL,
  PRIMARY KEY (source, external_id)
);
```

### Semantics

- **SM-2 is per my-move edge**, not per line. Each of the user's moves is
  graded and scheduled individually. The due queue is `SELECT ... FROM
  repertoire_moves WHERE is_my_move = 1 AND due_date <= today`.
- A **root** is any node with no incoming edge. The standard start position is
  one root; mistake-generated trees create mid-game roots. No special casing.
- **Branch quizzing:** at any node, every outgoing `is_my_move` edge is an
  accepted answer.
- **Bot book:** given the current normalized FEN and the user's repertoire
  side, opponent edges (`is_my_move = 0`) at that node are the bot's book moves.

### Migration

One-time, idempotent, runs inside the existing `bootstrap()`/migration path:

1. Create the new tables if absent.
2. If `opening_lines` exists and `repertoire_moves` is empty: walk each line
   move by move from its start position, inserting nodes and edges
   (`INSERT OR IGNORE` on the unique keys). Copy the line's SM-2 state onto
   each of its my-move edges; when two lines share an edge, keep the more
   mature state (longer interval wins).
3. Rename `opening_lines` to `opening_lines_legacy` as backup. All UI counts
   and queues re-point to edge queries.
4. Migrate `lichess_sync` rows into `game_sync` with `source = 'lichess'`,
   then drop `lichess_sync`.

## 2. Data sources

### 2a. Chess.com game sync (new crate `chesscom_client`)

- Public API, no auth: `GET https://api.chess.com/pub/player/electricmindgames/games/archives`
  returns monthly archive URLs; each archive returns JSON with one entry per
  game including full `pgn` and a stable game `url`/`uuid` for dedup.
- Requires a `User-Agent` header with contact info; read from optional `.env`
  key `CHESSCOM_USER_AGENT` (fallback to a sensible default containing the
  app name).
- Runs in a background thread like the Lichess sync, newest archives first,
  posts status strings to the UI. Each new game's PGN feeds the existing
  `pgn_processor` -> `games` table pipeline, so review, accuracy, and the
  weakness profiler work unchanged on Chess.com games.
- Dedup via `game_sync (source='chesscom', external_id=game uuid)`.
- `.env` gains `CHESSCOM_USERNAME=ElectricMindGames`.

### 2b. PGN variation import (the manual-add path)

- New "Import Lines" UI screen: paste PGN text or choose a `.pgn` file, pick
  which side (White/Black) the repertoire is for, import.
- Parsing uses the `pgn-reader` crate (shakmaty-compatible, handles nested
  recursive variations). Every branch of every game in the input is walked
  into the tree.
- New edges get fresh SM-2 state (due immediately). Existing edges are left
  untouched. Import reports "imported N moves, skipped M malformed games".
- The existing click-the-board line builder now writes tree edges directly
  (`source = 'manual'`) instead of `opening_lines` rows.

### 2c. Lichess studies sync

- `GET https://lichess.org/api/study/by/hackandtoss/export.pgn` with the
  existing `LICHESS_API_KEY` returns all the user's studies as multi-game PGN
  with variations. Funnels into the same variation-import code path with
  `source = 'study'`. One-click action on the sync screen; user chooses the
  repertoire side per import (studies whose name contains "white"/"black" may
  pre-select the picker, nothing more).

### 2d. Explorer adopt-into-repertoire

- From any position in the existing master-games opening tree, an "Adopt"
  action generates lines to a chosen depth: at the user's turns take the most
  popular master move; at opponent turns take the top-N (default 3) most
  popular replies. Insert as edges with `source = 'explorer'`.
- Uses the already-implemented Lichess explorer client and `opening_tree`
  data; only the adoption walk and UI hook are new.

### 2e. Mistake-driven generation (user-specified behavior)

After Stockfish analysis of an imported game completes:

1. Find the ply where the **user's** eval dropped the most (max centipawn
   loss among the user's moves; data already in `positions`).
2. Root a mini-tree at the position **before** that move (side = user's color
   in that game).
3. Edge 1: engine best move (the correction) — my move.
4. From the resulting position, multi-PV top-3 opponent replies — 3 opponent
   edges (multi-PV support already exists in `engine_controller`).
5. For each reply, the engine best follow-up — 3 my-move edges.
6. Total 7 edges, `source = 'mistake'`, my-move edges due immediately.

Cap: one mistake tree per analyzed game. Runs automatically at the end of the
existing analysis thread.

## 3. Drill engine rework (`app`)

- The drill queue lists **due edges** (my-move, `due_date <= today`), sorted
  by due date.
- A drill session starts at the due edge's parent position (board set from its
  FEN — no assumption of the standard start), asks the user to produce a
  repertoire move, and walks down the tree until a leaf. Every my-move edge
  traversed is graded: correct-first-try -> SM-2 grade 5; wrong or hint ->
  grade 1 (matches current pass/fail mapping).
- Color-aware behavior is preserved: at opponent-to-move nodes the app
  auto-plays an opponent edge (random among siblings) before waiting.
- Any outgoing my-move edge at the current node is accepted (branch quizzing);
  the walk follows the edge the user actually played.

## 4. Book-bot + Stockfish play mode

New Slint screen "Play vs Bot": color picker, Elo slider
(`UCI_LimitStrength true`, `UCI_Elo` ~1320–3190), New Game / Resign.

Game loop:

- **Bot's turn, in book:** look up the current normalized FEN in the user's
  repertoire (side = user's chosen color). If opponent edges exist, play one
  uniformly at random — the user faces all prepared branches in unpredictable
  order.
- **User's turn:** free move entry. If the position was in book and the played
  move is not one of the my-move edges, the UI marks a deviation silently and
  play continues (the user may be improvising deliberately).
- **Out of book** (no edges at the position, from either side's deviation or
  end of line): a persistent Stockfish `PlaySession` produces the bot's moves
  at the chosen Elo with a short fixed movetime.
- **`engine_controller` extension:** new `PlaySession` keeping one UCI process
  alive for the whole game (`position startpos moves ...` + `go movetime N`),
  with strength options set at session start. Existing one-shot analysis API
  is unchanged.
- **Post-game summary:** plies in book, where each side left book, and each
  user deviation with the move(s) the repertoire expected. Each deviation is
  recorded as a **failed review (grade 1)** on the corresponding edge, so
  game-condition fumbles re-enter tomorrow's drill queue. A "Review this game"
  button feeds the finished game into the existing import/analysis pipeline.
- If Stockfish is absent, the screen still allows book-only sparring and shows
  a clear notice; out-of-book bot turns end the game with the summary.

## 5. Error handling

- Network/API failures: background threads post status strings to the UI
  (existing pattern), never block the UI thread, and are safe to re-run —
  `game_sync` makes syncs idempotent.
- Malformed PGN games are skipped and counted, never abort a whole import.
- Engine missing/crashed: bot screen degrades to book-only with a message;
  analysis features already handle absence.
- Migration runs in a transaction; on failure the DB is left untouched and the
  app continues on the legacy table with an error status.

## 6. Testing

Extend the existing suite (26 tests) with:

- Tree: node/edge insert, transposition dedup, FEN normalization round-trip,
  mid-game roots.
- Migration: fixture DB with `opening_lines` rows in -> expected nodes/edges
  out, SM-2 state preserved, maturity conflict rule, idempotency (second run
  is a no-op).
- PGN import: nested-RAV fixture -> expected edge set; malformed-game skip
  count.
- `chesscom_client`: canned archive JSON -> parsed games + dedup behavior
  (no live network in tests).
- Book lookup: FEN -> opponent-edge selection; branch acceptance of any
  my-move edge.
- SM-2 per-edge grading transitions (pass/fail intervals).
- `PlaySession` against the existing `mock-stockfish` binary: scripted
  position/go/bestmove exchange, strength options sent once.

## Out of scope (this design)

- Aimchess-style dashboards beyond the existing weakness profiler.
- Puzzle-system changes (existing puzzles remain as-is).
- Chess.com *lessons/review* data (only games are public API).
- Opponent-specific scouting reports.
- Weighted (non-uniform) book move selection — revisit after real use.
