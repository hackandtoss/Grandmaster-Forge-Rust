# Board Flip Design

Approved 2026-07-11. Shared board-flip option so Black-side positions render with the player's side at the bottom, on the Puzzle Trainer and Play vs Bot screens.

## Problem

The shared `ChessBoard` Slint component always renders White at the bottom (model index 0 = a8, row-major). Black-to-move puzzles and Play-vs-Bot games as Black force the user to read the board upside down. Recorded as a gap in `docs/superpowers/specs/2026-07-10-puzzle-trainer-v1-notes.md` (board orientation).

## Decision: view-level flip inside the shared component

`ChessBoard` gains one input property:

- `in property <bool> flipped: false`

Rendering: each of the 64 cells keeps its **model index** (a8 = 0, White's perspective) for piece lookup, selection highlight, and the `square-clicked(int)` callback. Only the cell's screen position changes: when `flipped`, cell `i` draws at the coordinates of `63 - i` (180° rotation — files and ranks both reversed). Square colors are parity-invariant under `63 - i`, so the checkerboard formula is untouched.

Consequences:

- Zero changes to Rust click handlers, `fen_to_pieces`, `index_to_square`, or any FEN/move logic — they already speak model coordinates and continue to.
- Every screen shares the mechanism; screens not wired up simply leave `flipped` at its default.

Rejected alternatives:

- **Data-level flip in Rust** (reverse the pieces array, translate indices in handlers): touches every board call site and its click handler; easy to get selection/highlight wrong; wrong altitude for a pure view concern.
- **Separate flipped component**: copy-paste of `ChessBoard`.

## Control: auto-flip + manual toggle

Two new `AppWindow` properties, each bound to its screen's board, each with a small "Flip Board" button that negates it:

- `puzzle-board-flipped` — set on `load-puzzle` from the puzzle FEN's side-to-move: Black to move → `true` (stored puzzle FENs always have the solver to move).
- `play-board-flipped` — set when a Play vs Bot game starts: `play_side == "Black"` → `true`.

Manual toggle simply flips the property afterward; the next puzzle load or new game re-asserts the auto value. No separate override state.

Out of scope: builder/drill/review screens stay White-at-bottom (they can bind `flipped` later); rank/file coordinate labels (the board has none today); persistence of a manual flip across sessions.

## Error handling

`fen_black_to_move(fen: &str) -> bool` returns `false` (White at bottom) for malformed FEN or a missing side-to-move field. A wrong orientation is cosmetic and recoverable via the toggle; it must never block puzzle loading.

## Testing

- New pure helper `fen_black_to_move` unit-tested: `"... w ..."` → false, `"... b ..."` → true, malformed/empty FEN → false.
- The Slint-side `63 - i` mapping is a single expression with no Rust test surface; verified by `cargo check -p app` and manual smoke.
- Play-side comparison is a direct equality on existing state; no helper.
- Existing tests must stay green: the flip changes no model-coordinate behavior.
