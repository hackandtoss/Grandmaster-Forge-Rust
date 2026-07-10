# Puzzle Trainer V1 Notes

**Date:** 2026-07-10
**Status:** Shipped V1; gaps recorded for follow-up planning

## What V1 Is

A dedicated `puzzles` screen in the Slint shell reusing the existing pieces:

- `puzzles` table and `get_next_puzzle(max_difficulty)` for storage/selection (ceiling kept at the pre-existing 2000).
- The `load-puzzle` callback, previously orphaned (no UI element invoked it), now drives the screen.
- The shared `ChessBoard` component with the same two-click move entry as drills and bot play.
- Solution checking walks the stored space-separated `solution_uci` line: solver move, scripted opponent reply, solver move, and so on.
- One training event per puzzle (`kind = "puzzle"`, outcome `Correct`/`Failed`): recorded at the first wrong move or reveal, or at clean completion.
- Five built-in starter mate puzzles are seeded idempotently at startup; a unit test replays each line through `shakmaty` and asserts it ends in checkmate.

## Gaps Found During Implementation

### 1. No real puzzle ingestion path

Nothing populates `puzzles` besides the new starter seeds. `lichess_client::fetch_puzzle` exists but is unwired, and its deserialization expects `puzzle.fen`, while the live Lichess `/api/puzzle/next` response provides the game PGN plus `initialPly` instead of a FEN. Wiring it up needs a small derivation step (replay PGN to `initialPly`, emit FEN) plus a decision on when to fetch (sync-time batch vs on-demand). This is the main blocker between the trainer and real content.

### 2. `score_delta` doubles as the absolute puzzle rating

`get_puzzle_rating` reads the most recent `kind='puzzle'` event's `score_delta` as the current rating, so puzzle events must store the absolute updated rating (V1 does: ±10, clamped to 400–3200). Consequence: the dashboard Training Rating updates correctly, but the Recent Training Log renders the event as a delta (`+1510`). This is exactly the kind of overloading the review-event-spine plan (`docs/superpowers/plans/2026-07-07-review-event-spine.md`) should fix; puzzle events should move to normalized review events when that lands.

### 3. Board orientation

The board always renders White at the bottom. Black-to-move puzzles are playable (the status line states the side to move) but not flipped. Same limitation as Play vs Bot as Black; a shared board-flip option would fix both.

### 4. Deliberately deferred

- No per-puzzle SRS/FSRS scheduling — puzzles are random draws under the difficulty ceiling.
- No theme filtering or motif-based selection (waits on the tactical scrutinizer labels).
- No puzzle generation from the user's own reviewed mistakes (natural follow-up once motif labels exist).
