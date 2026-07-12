# Board Flip Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Auto-flip the board (player's side at the bottom) on the Puzzle Trainer and Play vs Bot screens, with a manual "Flip Board" toggle on both.

**Architecture:** Per `docs/superpowers/specs/2026-07-11-board-flip-design.md`: one `flipped` input property on the shared `ChessBoard` Slint component remaps each cell's screen position to `63 - index` (180° rotation) while piece lookup, selection highlight, and `square-clicked(int)` stay in model coordinates (a8 = 0) — zero changes to Rust click handlers or FEN logic. Two `AppWindow` booleans carry per-screen flip state; auto values are asserted on `load-puzzle` (Black to move → flipped) and `play-new-game` (playing Black → flipped).

**Tech Stack:** Rust 2021, Slint 1.6 (`slint!` macro in `app/src/main.rs`), existing `side_to_move_label` helper.

## Global Constraints

- Workspace must stay at zero clippy warnings and `cargo fmt --all -- --check` clean (baseline since PR #12).
- `cargo test --workspace` currently 83 passing; this plan takes it to 84 — never fewer.
- Tests must not need a network or a real Stockfish binary.
- Divergence from spec, recorded here: the spec names a new helper `fen_black_to_move`; the codebase already has `side_to_move_label` (`app/src/main.rs:1584`) with identical semantics (returns `"Black"` iff FEN field 2 is `b`, `"White"` otherwise, including malformed FEN). Reuse it instead of adding a duplicate; Task 1 adds its missing unit test.

---

### Task 1: Characterization test for the auto-flip decision helper

**Files:**
- Modify: `app/src/main.rs` (tests module, near `puzzle_progress_text_counts_solver_moves_only`)

**Interfaces:**
- Consumes: `fn side_to_move_label(fen: &str) -> &'static str` (exists at `app/src/main.rs:1584`).
- Produces: guaranteed contract for Tasks 3–4: `side_to_move_label(fen) == "Black"` is the auto-flip condition, `"White"` for malformed/empty FEN.

- [x] **Step 1: Add the test**

Add to the `mod tests` block in `app/src/main.rs`, and add `side_to_move_label` to the `use super::{...}` list:

```rust
#[test]
fn side_to_move_label_reads_fen_and_defaults_to_white() {
    assert_eq!(
        side_to_move_label("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"),
        "White"
    );
    assert_eq!(
        side_to_move_label("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR b KQkq - 0 1"),
        "Black"
    );
    // Malformed FEN must fail toward White-at-bottom (spec: never block loading).
    assert_eq!(side_to_move_label(""), "White");
    assert_eq!(side_to_move_label("not a fen"), "White");
}
```

- [x] **Step 2: Run it**

Run: `cargo test -p app side_to_move_label_reads_fen`
Expected: PASS (characterization of existing behavior — this pins the contract the flip relies on; if it fails, stop and re-read the helper).

- [x] **Step 3: Commit**

```bash
git add app/src/main.rs
git commit -m "test(app): pin side_to_move_label contract for board flip"
```

### Task 2: `flipped` property on ChessBoard

**Files:**
- Modify: `app/src/main.rs` (the `component ChessBoard` block, ~line 90)

**Interfaces:**
- Produces: `in property <bool> flipped` on `ChessBoard`; all existing bindings keep working (default `false`).

- [x] **Step 1: Add the property and remap cell positions**

In `component ChessBoard`, add below `in property <bool> interactive: true;`:

```slint
// View-level 180° rotation: cell `index` keeps its model meaning (a8 = 0,
// White's perspective) for pieces/selection/clicks; only where it is DRAWN
// changes. 63 - index reverses both file and rank, and preserves square
// color parity, so the checker formula below needs no change.
in property <bool> flipped: false;
```

Then in the `for index in 64: Rectangle` loop, replace the two position lines:

```slint
x: mod(root.flipped ? 63 - index : index, 8) * root.cell-size;
y: floor((root.flipped ? 63 - index : index) / 8) * root.cell-size;
```

Everything else in the cell (background/selection check, `piece-symbol(root.pieces[index])`, `square-clicked(index)`) stays on `index` — do not touch it.

- [x] **Step 2: Build**

Run: `cargo check -p app`
Expected: clean. (The mapping has no Rust test surface; the spec accepts `cargo check` + smoke here.)

- [x] **Step 3: Commit**

```bash
git add app/src/main.rs
git commit -m "feat(app): view-level flipped rendering on shared ChessBoard"
```

### Task 3: Puzzle Trainer wiring (auto + toggle)

**Files:**
- Modify: `app/src/main.rs` (AppWindow properties ~line 390; puzzle screen ~line 1326; `on_load_puzzle` ~line 2693)

**Interfaces:**
- Consumes: `ChessBoard.flipped` (Task 2), `side_to_move_label` (Task 1).
- Produces: `in-out property <bool> puzzle-board-flipped` on AppWindow.

- [x] **Step 1: Add the AppWindow property**

Next to the other puzzle properties (`puzzle-fetch-status` etc.):

```slint
in-out property <bool> puzzle-board-flipped: false;
```

- [x] **Step 2: Bind the board and add the toggle**

The puzzle screen's `ChessBoard` (~line 1326) becomes:

```slint
ChessBoard {
    pieces: root.puzzle-board-pieces;
    selected-square: root.puzzle-selected-square;
    flipped: root.puzzle-board-flipped;
    square-clicked(index) => { root.puzzle-click-square(index); }
}
```

In the puzzle controls column, directly under the `New Puzzle` button, add:

```slint
Button {
    text: "Flip Board";
    clicked => { root.puzzle-board-flipped = !root.puzzle-board-flipped; }
}
```

- [x] **Step 3: Auto-flip on load**

In `on_load_puzzle`'s success path, right after `app.set_puzzle_selected_square(-1);`:

```rust
app.set_puzzle_board_flipped(side_to_move_label(&puzzle.fen) == "Black");
```

- [x] **Step 4: Build and test**

Run: `cargo check -p app && cargo test -p app`
Expected: clean build, 41 tests pass (40 + Task 1's).

- [x] **Step 5: Commit**

```bash
git add app/src/main.rs
git commit -m "feat(app): auto-flip + manual flip toggle on Puzzle Trainer"
```

### Task 4: Play vs Bot wiring (auto + toggle)

**Files:**
- Modify: `app/src/main.rs` (AppWindow properties; play screen ~line 1273; `on_play_new_game` ~line 3466)

**Interfaces:**
- Consumes: `ChessBoard.flipped` (Task 2).
- Produces: `in-out property <bool> play-board-flipped` on AppWindow.

- [x] **Step 1: Add the AppWindow property**

Next to the other play properties:

```slint
in-out property <bool> play-board-flipped: false;
```

- [x] **Step 2: Bind the board and add the toggle**

The play screen's `ChessBoard` (~line 1273) becomes:

```slint
ChessBoard {
    pieces: root.play-board-pieces;
    selected-square: root.play-selected-square;
    flipped: root.play-board-flipped;
    square-clicked(index) => { root.play-click-square(index); }
}
```

In the play controls column, directly under the `New Game` button, add:

```slint
Button {
    text: "Flip Board";
    clicked => { root.play-board-flipped = !root.play-board-flipped; }
}
```

- [x] **Step 3: Auto-flip on new game**

In `on_play_new_game`, right after `app.set_play_active(true);`:

```rust
app.set_play_board_flipped(st.play_side == "Black");
```

- [x] **Step 4: Full verification**

Run: `cargo fmt --all && cargo fmt --all -- --check && cargo clippy --workspace && cargo test --workspace`
Expected: fmt clean, zero clippy warnings, 84 tests pass.

- [x] **Step 5: Commit**

```bash
git add app/src/main.rs
git commit -m "feat(app): auto-flip + manual flip toggle on Play vs Bot"
```

### Task 5: Docs

**Files:**
- Modify: `README.md`
- Modify: `docs/superpowers/specs/2026-07-10-puzzle-trainer-v1-notes.md` (gap 3, board orientation)

- [x] **Step 1: README**

In the Puzzle Training section add a bullet:

```markdown
- Board auto-flips so the solver's side is at the bottom (Black-to-move puzzles render Black-down); Play vs Bot flips when you play Black — both screens have a manual "Flip Board" toggle
```

Update the test-count line from `# 83 tests` to `# 84 tests` (same wording otherwise).

- [x] **Step 2: Mark the notes-doc gap addressed**

Retitle gap 3 heading with ` — ADDRESSED 2026-07-11` and append one resolution sentence pointing at `docs/superpowers/specs/2026-07-11-board-flip-design.md`.

- [x] **Step 3: Commit**

```bash
git add README.md docs/superpowers/specs/2026-07-10-puzzle-trainer-v1-notes.md
git commit -m "docs: board flip in README and puzzle notes"
```

## Self-Review

- Spec coverage: view-level flip (Task 2), auto+toggle on both screens (Tasks 3–4), malformed-FEN default (Task 1), out-of-scope items untouched.
- Placeholder scan: none; every code step shows the code.
- Type consistency: `flipped`, `puzzle-board-flipped`, `play-board-flipped`, `side_to_move_label` used identically across tasks.
- Divergence from spec (recorded in Global Constraints): reuse `side_to_move_label` instead of new `fen_black_to_move`.
