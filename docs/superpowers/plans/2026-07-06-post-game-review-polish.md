# Post-Game Review Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **Repo drift notice (2026-07-07): UI POLISH PLAN, NOT THE TRAINING ARCHITECTURE.**
> This plan remains useful for board scaling and the chess.com-style review brief, but it should not define scheduling, repertoire organization, or tactical explanation architecture.
> Move labels and review stats should feed the shared learning loop, course/line weakness status, and the deterministic tactical scrutinizer described in the July 7 specs.
> Keep using Stockfish/UCI for evaluation truth; explanations must come from deterministic rules, not LLM-generated chess advice.

**Goal:** Give Grandmaster Forge a chessboard that scales with the window everywhere it appears, and a chess.com-style "Game Review Brief" (result header + move-classification stat grid + CTA) reachable both automatically right after a bot game and on demand from any already-reviewed game.

**Architecture:** All UI lives in one Slint `slint::slint! { ... }` macro block inside `app/src/main.rs`, with Rust closures wiring callbacks — this plan follows that existing structure rather than introducing new files or modules. A new `ChessBoard` Slint component replaces four duplicated fixed-360px boards. A new `GameReviewBrief` Slint component + `ReviewStatEntry` struct, backed by small pure-Rust helper functions, gets triggered from two places: automatically at bot-game end, and on demand from a new button per row in the Reviewed Games list.

**Tech Stack:** Rust 2021, Slint 1.6 (embedded via `slint::slint! {}`), shakmaty 0.26 (chess rules/position, already a dependency), rusqlite, existing `engine_controller`/`db_manager`/`tree` crates.

## Global Constraints

- All Slint UI and its Rust wiring lives in `app/src/main.rs` (existing monolith pattern) — do not split into new modules/files as part of this plan.
- Match the existing dark theme exactly: panel background `#1a1a1e`, borders `#27272a`/`#3f3f46`, primary text `#ffffff`/`#e4e4e7`, secondary text `#a1a1aa`, accent violet `#6d28d9`/`#a78bfa`, amber highlight `#fcd34d`.
- No new crate dependencies — everything needed is already available (`shakmaty::Position::outcome`, `std::collections::hash_map::DefaultHasher`).
- Every new/changed pure-Rust function gets a unit test in the existing `#[cfg(test)] mod tests` block at the bottom of `app/src/main.rs`, added to its existing `use super::{...}` import list.
- Run `cargo test --bin app` after every task. Run `cargo check --bin app` (fast) or `cargo build --bin app` after every Slint-touching step — Slint syntax errors only surface at compile time, there is no separate linter.
- Spec: `docs/superpowers/specs/2026-07-06-post-game-review-polish-design.md`.
- Bug fix that predates this plan (already committed on this branch): `imported_game_id()` / `same_day_bot_games_get_distinct_ids_and_do_not_collide` test. Nothing in this plan touches that code path except Task 2, which extends the PGN this id is computed from.

---

## Task ordering and dependencies

Tasks 1, 2, and 3 touch disjoint code (no shared state, no call-order requirement) and can be built and reviewed in any order or in parallel. Task 4 depends on Task 2 (uses its outcome helper). Task 5 depends on Tasks 3 and 4. Task 6 depends on Tasks 3 and 5.

```
1 (ChessBoard)         \
2 (outcome/PGN)  --> 4 (auto-trigger plumbing) --> 5 (Brief UI) --> 6 (on-demand entry)
3 (stats helper) ------/-----------------------------^
```

---

### Task 1: Shared `ChessBoard` component, migrated onto all 4 boards

**Files:**
- Modify: `app/src/main.rs`

**Interfaces:**
- Produces: Slint component `ChessBoard { in pieces: [string], in selected-square: int, in highlight-color: color, in interactive: bool, callback square-clicked(int) }`.
- No Rust-side interface — this is a pure Slint/rendering change. No other task depends on it.

- [ ] **Step 1: Add the `ChessBoard` component**

Insert this immediately before `export component AppWindow inherits Window {` (currently line 70):

```slint
    component ChessBoard inherits Rectangle {
        in property <[string]> pieces: [
            "", "", "", "", "", "", "", "",
            "", "", "", "", "", "", "", "",
            "", "", "", "", "", "", "", "",
            "", "", "", "", "", "", "", "",
            "", "", "", "", "", "", "", "",
            "", "", "", "", "", "", "", "",
            "", "", "", "", "", "", "", "",
            "", "", "", "", "", "", "", "",
        ];
        in property <int> selected-square: -1;
        in property <color> highlight-color: #fcd34d;
        in property <bool> interactive: true;
        callback square-clicked(int);

        pure function piece-symbol(piece: string) -> string {
            return piece == "P" || piece == "p" ? "♟"
                : piece == "N" || piece == "n" ? "♞"
                : piece == "B" || piece == "b" ? "♝"
                : piece == "R" || piece == "r" ? "♜"
                : piece == "Q" || piece == "q" ? "♛"
                : piece == "K" || piece == "k" ? "♚"
                : "";
        }

        pure function piece-color(piece: string) -> color {
            return piece == "P" || piece == "N" || piece == "B" || piece == "R" || piece == "Q" || piece == "K"
                ? #f8fafc
                : piece == ""
                    ? #00000000
                    : #111827;
        }

        // Stay square: shrink to the smaller of whatever space the layout
        // gives us, floored at today's fixed size, capped so it doesn't
        // dwarf the surrounding panel on an ultrawide window.
        property <length> board-size: max(360px, min(560px, min(root.width, root.height)));
        property <length> cell-size: root.board-size / 8;

        min-width: 360px;
        min-height: 360px;
        horizontal-stretch: 1;
        vertical-stretch: 1;
        background: #1a1a1e;
        border-radius: 8px;
        border-color: #3f3f46;
        border-width: 2px;

        Rectangle {
            x: (parent.width - root.board-size) / 2;
            y: (parent.height - root.board-size) / 2;
            width: root.board-size;
            height: root.board-size;

            for index in 64: Rectangle {
                x: mod(index, 8) * root.cell-size;
                y: floor(index / 8) * root.cell-size;
                width: root.cell-size;
                height: root.cell-size;
                background: (root.selected-square == index)
                    ? root.highlight-color
                    : (mod(floor(index / 8) + mod(index, 8), 2) == 0 ? #f0d9b5 : #b58863);

                Text {
                    text: root.piece-symbol(root.pieces[index]);
                    font-size: root.cell-size * 0.62;
                    font-family: "Segoe UI Symbol";
                    color: root.piece-color(root.pieces[index]);
                    horizontal-alignment: center;
                    vertical-alignment: center;
                }

                if root.interactive : TouchArea {
                    clicked => { root.square-clicked(index); }
                }
            }
        }
    }

```

- [ ] **Step 2: Build-check**

Run: `cargo check --bin app`
Expected: succeeds (the component is defined but not yet used anywhere; Slint does not error on unused components).

- [ ] **Step 3: Migrate the Dashboard drill board**

Find (Dashboard screen, "Chess board container" comment, currently ~line 533):

```slint
                        // Chess board container
                        Rectangle {
                            width: 360px;
                            height: 360px;
                            background: #1a1a1e;
                            border-radius: 8px;
                            border-color: #3f3f46;
                            border-width: 2px;

                            for index in 64: Rectangle {
                                x: mod(index, 8) * 45px;
                                y: floor(index / 8) * 45px;
                                width: 45px;
                                height: 45px;
                                background: (root.board-selected-square == index)
                                    ? #fcd34d
                                    : (mod(floor(index / 8) + mod(index, 8), 2) == 0 ? #f0d9b5 : #b58863);

                                Text {
                                    text: root.piece-symbol(root.board-pieces[index]);
                                    font-size: 28px;
                                    font-family: "Segoe UI Symbol";
                                    color: root.piece-color(root.board-pieces[index]);
                                    horizontal-alignment: center;
                                    vertical-alignment: center;
                                }

                                TouchArea {
                                    clicked => {
                                        root.click-board-square(index);
                                    }
                                }
                            }
                        }
```

Replace with:

```slint
                        // Chess board container
                        ChessBoard {
                            pieces: root.board-pieces;
                            selected-square: root.board-selected-square;
                            square-clicked(index) => { root.click-board-square(index); }
                        }
```

The wrapping `VerticalLayout` for this column has `alignment: center;`. In Slint, an explicit `alignment` on a layout centers children at their natural size rather than stretching them — this fights the board's own `horizontal-stretch`/`vertical-stretch: 1`. Remove `alignment: center;` from that `VerticalLayout` (the board's own inner `Rectangle` already centers the actual 8×8 grid within whatever space it receives, so nothing needs the outer centering once the board can stretch).

- [ ] **Step 4: Build-check, then run and visually verify**

Run: `cargo check --bin app`, then `cargo run --bin app`.
Expected: app launches on the Dashboard screen; resize the window and confirm the board grows/shrinks with it (bounded between ~360px and ~560px) and stays square. If it does **not** grow: the fallback is to explicitly bind `width`/`height` on this `ChessBoard` instance to the column's available size (e.g. `width: parent.width;`) rather than relying on stretch — try this only if the stretch-factor approach visibly fails.

- [ ] **Step 5: Migrate the Game Review timeline board**

Find (Game Review screen, "Board View" section, currently ~line 687):

```slint
                            Rectangle {
                                width: 360px;
                                height: 360px;
                                background: #1a1a1e;
                                border-radius: 8px;
                                border-color: #3f3f46;
                                border-width: 2px;

                                for index in 64: Rectangle {
                                    x: mod(index, 8) * 45px;
                                    y: floor(index / 8) * 45px;
                                    width: 45px;
                                    height: 45px;
                                    background: (mod(floor(index / 8) + mod(index, 8), 2) == 0 ? #f0d9b5 : #b58863);

                                    Text {
                                        text: root.piece-symbol(root.board-pieces[index]);
                                        font-size: 28px;
                                        font-family: "Segoe UI Symbol";
                                        color: root.piece-color(root.board-pieces[index]);
                                        horizontal-alignment: center;
                                        vertical-alignment: center;
                                    }
                                }
                            }
```

Replace with (non-interactive — this board is a timeline viewer, not a move-input board):

```slint
                            ChessBoard {
                                pieces: root.board-pieces;
                                interactive: false;
                            }
```

This column's wrapping `VerticalLayout` also has `alignment: center;` — remove it, same reasoning as Step 3.

- [ ] **Step 6: Build-check and visually verify** (same as Step 4, on the Game Review screen — select a game from the list first so the board isn't empty).

- [ ] **Step 7: Migrate the Opening Builder board**

Find (Opening Builder screen, "8×8 board" comment, currently ~line 854):

```slint
                        // 8×8 board
                        Rectangle {
                            width: 360px;
                            height: 360px;
                            background: #1a1a1e;
                            border-radius: 8px;
                            border-color: #3f3f46;
                            border-width: 2px;

                            for index in 64: Rectangle {
                                x: mod(index, 8) * 45px;
                                y: floor(index / 8) * 45px;
                                width: 45px;
                                height: 45px;
                                background: (root.builder-selected-square == index)
                                    ? #7c3aed
                                    : (mod(floor(index / 8) + mod(index, 8), 2) == 0 ? #f0d9b5 : #b58863);
                                Text {
                                    text: root.piece-symbol(root.builder-board-pieces[index]);
                                    font-size: 28px;
                                    font-family: "Segoe UI Symbol";
                                    color: root.piece-color(root.builder-board-pieces[index]);
                                    horizontal-alignment: center;
                                    vertical-alignment: center;
                                }
                                TouchArea {
                                    clicked => { root.builder-click-square(index); }
                                }
                            }
                        }
```

Replace with:

```slint
                        // 8×8 board
                        ChessBoard {
                            pieces: root.builder-board-pieces;
                            selected-square: root.builder-selected-square;
                            highlight-color: #7c3aed;
                            square-clicked(index) => { root.builder-click-square(index); }
                        }
```

This column (`VerticalLayout { width: 480px; spacing: 12px; ... }`) has no `alignment: center`, so no removal needed here. Its fixed `width: 480px` means the board effectively caps around 480px here rather than the component's own 560px ceiling — that's an acceptable, smaller-scoped improvement over today's fixed 360px; widening this column is out of scope for this plan.

- [ ] **Step 8: Build-check and visually verify** (Opening Builder screen).

- [ ] **Step 9: Migrate the Play vs Bot board**

Find (Play vs Bot screen, currently ~line 1022):

```slint
                    // Board
                    Rectangle {
                        width: 360px; height: 360px;
                        background: #1a1a1e; border-radius: 8px;
                        border-color: #3f3f46; border-width: 2px;
                        for index in 64: Rectangle {
                            x: mod(index, 8) * 45px;
                            y: floor(index / 8) * 45px;
                            width: 45px; height: 45px;
                            background: (root.play-selected-square == index)
                                ? #fcd34d
                                : (mod(floor(index / 8) + mod(index, 8), 2) == 0 ? #f0d9b5 : #b58863);
                            Text {
                                text: root.piece-symbol(root.play-board-pieces[index]);
                                font-size: 28px;
                                font-family: "Segoe UI Symbol";
                                color: root.piece-color(root.play-board-pieces[index]);
                                horizontal-alignment: center; vertical-alignment: center;
                            }
                            TouchArea { clicked => { root.play-click-square(index); } }
                        }
                    }
```

Replace with:

```slint
                    // Board
                    ChessBoard {
                        pieces: root.play-board-pieces;
                        selected-square: root.play-selected-square;
                        square-clicked(index) => { root.play-click-square(index); }
                    }
```

This board sits directly in a plain `HorizontalLayout { spacing: 24px; ... }` with no `alignment` override, so no removal needed.

- [ ] **Step 10: Build-check, test, and visually verify all four screens**

Run: `cargo check --bin app`, then `cargo test --bin app` (expect all 21 existing tests still pass — nothing in this task changes Rust logic), then `cargo run --bin app` and check Dashboard, Game Review, Opening Builder, and Play vs Bot: each board should scale between ~360-560px (Opening Builder capped ~480px per its column) as you resize the window, stay square, and remain clickable where it was clickable before.

- [ ] **Step 11: Remove now-unused `piece-symbol`/`piece-color` from `AppWindow`, if unused**

Search `app/src/main.rs` for `root.piece-symbol` and `root.piece-color` outside the new `ChessBoard` component. If there are no remaining references (all four call sites now go through `ChessBoard`'s own copies), delete `AppWindow`'s copies of these two `pure function`s (originally ~lines 76-92) to avoid dead code.

- [ ] **Step 12: Commit**

```bash
git add app/src/main.rs
git commit -m "feat(app): extract shared ChessBoard component, make it scale"
```

---

### Task 2: Bot game outcome determination + real PGN result tag

**Files:**
- Modify: `app/src/main.rs`

**Interfaces:**
- Produces: `fn bot_game_outcome(chess: &Chess, side: &str, resigned: bool) -> (&'static str, &'static str)` — returns `(pgn_result_tag, header_text)`, e.g. `("1-0", "You beat Forge Bot!")`. `side` is `state.play_side` ("White"/"Black" — the user's color).
- Produces: `fn build_bot_game_pgn(moves: &[String], side: &str, result_tag: &str) -> Option<String>` — rebuilds SAN movetext from recorded UCI moves and returns a full PGN string with `result_tag` as its result, or `None` if `moves` is empty. Extracted from today's `on_play_review_game` (which hardcodes `*`).
- Consumes: nothing new — `Chess`, `Uci`, `San`, `CastlingMode` are already imported at the top of `app/src/main.rs`.

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block at the bottom of `app/src/main.rs`. First, update its import list:

```rust
    use super::{
        bot_game_outcome, build_bot_game_pgn, fen_to_pieces, game_review_summary,
        imported_game_id, ingest_pgn_game, repertoire_course_name, repertoire_source_label,
        sk_fen,
    };
    use db_manager::{SqliteStore, TrainingStore};
    use shakmaty::{CastlingMode, Position};
```

(Add these lines to the existing `use` block rather than replacing it if Task 4 or another in-flight change has already touched it — the goal is that `bot_game_outcome`, `build_bot_game_pgn`, `sk_fen`, `CastlingMode`, and `Position` all become available inside `mod tests`.)

Then add the tests:

```rust
    #[test]
    fn bot_game_outcome_reports_win_from_the_users_perspective() {
        // Fool's mate: 1. f3 e5 2. g4 Qh4# — White is checkmated.
        let chess: Chess = "rnbqkbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 0 3"
            .parse::<sk_fen::Fen>()
            .unwrap()
            .into_position(CastlingMode::Standard)
            .unwrap();

        let (tag, header) = bot_game_outcome(&chess, "White", false);
        assert_eq!(tag, "0-1");
        assert_eq!(header, "Forge Bot won");

        let (tag, header) = bot_game_outcome(&chess, "Black", false);
        assert_eq!(tag, "0-1");
        assert_eq!(header, "You beat Forge Bot!");
    }

    #[test]
    fn bot_game_outcome_resignation_always_credits_the_other_side() {
        let chess = Chess::default();

        let (tag, header) = bot_game_outcome(&chess, "White", true);
        assert_eq!(tag, "0-1");
        assert_eq!(header, "Forge Bot won");

        let (tag, header) = bot_game_outcome(&chess, "Black", true);
        assert_eq!(tag, "1-0");
        assert_eq!(header, "Forge Bot won");
    }

    #[test]
    fn bot_game_outcome_unfinished_position_is_unresolved() {
        let chess = Chess::default();
        let (tag, header) = bot_game_outcome(&chess, "White", false);
        assert_eq!(tag, "*");
        assert_eq!(header, "Game over");
    }

    #[test]
    fn build_bot_game_pgn_embeds_the_real_result_tag() {
        let moves = vec!["e2e4".to_string(), "e7e5".to_string()];
        let pgn = build_bot_game_pgn(&moves, "White", "1-0").unwrap();
        assert!(pgn.contains("1. e4 e5 1-0"));
        assert!(!pgn.trim_end().ends_with('*'));
    }

    #[test]
    fn build_bot_game_pgn_returns_none_for_an_empty_game() {
        assert!(build_bot_game_pgn(&[], "White", "*").is_none());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --bin app bot_game_outcome`
Expected: FAIL to compile — `bot_game_outcome`/`build_bot_game_pgn` not defined.

- [ ] **Step 3: Implement `bot_game_outcome`**

Add near `estimated_elo`/`game_review_summary` (~line 1136):

```rust
/// Determine a finished bot game's PGN result tag and a user-facing header,
/// from the user's perspective (`side` is `state.play_side`).
fn bot_game_outcome(chess: &Chess, side: &str, resigned: bool) -> (&'static str, &'static str) {
    if resigned {
        let tag = if side == "White" { "0-1" } else { "1-0" };
        return (tag, "Forge Bot won");
    }
    match chess.outcome() {
        Some(shakmaty::Outcome::Decisive { winner }) => {
            let tag = if winner == shakmaty::Color::White { "1-0" } else { "0-1" };
            let user_won = (winner == shakmaty::Color::White) == (side == "White");
            let header = if user_won { "You beat Forge Bot!" } else { "Forge Bot won" };
            (tag, header)
        }
        Some(shakmaty::Outcome::Draw) => ("1/2-1/2", "Draw"),
        None => ("*", "Game over"),
    }
}
```

- [ ] **Step 4: Extract `build_bot_game_pgn` from `on_play_review_game`**

Find `on_play_review_game`'s body (currently ~lines 2821-2851):

```rust
        app.on_play_review_game(move || {
            let st = state.lock().unwrap();
            let moves = st.play_moves_uci.clone();
            let side = st.play_side.clone();
            drop(st);
            if moves.is_empty() { return; }
            // Rebuild SAN movetext from UCI.
            let mut pos = Chess::default();
            let mut san_moves: Vec<String> = Vec::new();
            for uci_str in &moves {
                let Ok(uci) = uci_str.parse::<Uci>() else { break };
                let Ok(m) = uci.to_move(&pos) else { break };
                san_moves.push(San::from_move(&pos, &m).to_string());
                pos.play_unchecked(&m);
            }
            let mut movetext = String::new();
            for (i, san) in san_moves.iter().enumerate() {
                if i % 2 == 0 { movetext.push_str(&format!("{}. ", i / 2 + 1)); }
                movetext.push_str(san);
                movetext.push(' ');
            }
            let (w, b) = if side == "White" { ("You", "Forge Bot") } else { ("Forge Bot", "You") };
            let pgn = format!(
                "[Event \"Bot Game\"]\n[White \"{w}\"]\n[Black \"{b}\"]\n[Date \"{}\"]\n\n{movetext}*",
                tree::local_now_str().replace('-', ".")
            );
            if let Some(app) = app_weak.upgrade() {
                app.invoke_import_pgn(slint::SharedString::from(pgn));
                app.set_active_screen(slint::SharedString::from("review"));
            }
        });
```

Replace with (moving the PGN-building logic into the new free function, computing the real result tag, and keeping the callback's own job to just build+trigger+navigate):

```rust
        app.on_play_review_game(move || {
            let st = state.lock().unwrap();
            let moves = st.play_moves_uci.clone();
            let side = st.play_side.clone();
            let (result_tag, _) = bot_game_outcome(&st.play_chess, &side, false);
            drop(st);
            let Some(pgn) = build_bot_game_pgn(&moves, &side, result_tag) else {
                return;
            };
            if let Some(app) = app_weak.upgrade() {
                app.invoke_import_pgn(slint::SharedString::from(pgn));
                app.set_active_screen(slint::SharedString::from("review"));
            }
        });
```

Then add the extracted function near `bot_game_outcome`:

```rust
/// Rebuild a bot game's PGN (SAN movetext) from its recorded UCI moves.
fn build_bot_game_pgn(moves: &[String], side: &str, result_tag: &str) -> Option<String> {
    if moves.is_empty() {
        return None;
    }
    let mut pos = Chess::default();
    let mut san_moves: Vec<String> = Vec::new();
    for uci_str in moves {
        let Ok(uci) = uci_str.parse::<Uci>() else { break };
        let Ok(m) = uci.to_move(&pos) else { break };
        san_moves.push(San::from_move(&pos, &m).to_string());
        pos.play_unchecked(&m);
    }
    let mut movetext = String::new();
    for (i, san) in san_moves.iter().enumerate() {
        if i % 2 == 0 {
            movetext.push_str(&format!("{}. ", i / 2 + 1));
        }
        movetext.push_str(san);
        movetext.push(' ');
    }
    let (w, b) = if side == "White" {
        ("You", "Forge Bot")
    } else {
        ("Forge Bot", "You")
    };
    Some(format!(
        "[Event \"Bot Game\"]\n[White \"{w}\"]\n[Black \"{b}\"]\n[Date \"{}\"]\n\n{movetext}{result_tag}",
        tree::local_now_str().replace('-', ".")
    ))
}
```

Note: `bot_game_outcome`'s first return value (called `_` above, since `on_play_review_game` doesn't need the header) — this is intentional; `finish_play_game` in Task 4 uses both.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --bin app`
Expected: all previous tests plus the 5 new ones pass (26 total).

- [ ] **Step 6: Commit**

```bash
git add app/src/main.rs
git commit -m "feat(app): determine bot game outcome, embed real PGN result tag"
```

---

### Task 3: Move-classification stats helper + `ReviewStatEntry`

**Files:**
- Modify: `app/src/main.rs`

**Interfaces:**
- Produces: Slint struct `ReviewStatEntry { label: string, count: int }`.
- Produces: `fn review_stat_entries(classes: &[&str]) -> Vec<ReviewStatEntry>` — the 9 `engine_controller::MoveClass` categories (Book, Brilliant, Best, Great, Excellent, Good, Miss, Mistake, Blunder), in that display order, skipping any with count 0.
- Produces: `fn accuracy_summary_text(accuracy: Option<f32>) -> String` — extracted from `game_review_summary` (behavior-preserving refactor; existing test must still pass unchanged).
- Consumed by: Task 5 (Brief UI reads `review_stat_entries`/`accuracy_summary_text`), Task 6 (on-demand entry point calls `review_stat_entries` directly from stored data).

- [ ] **Step 1: Write the failing tests**

Add `review_stat_entries` and `accuracy_summary_text` to the test module's `use super::{...}` list (alongside whatever Task 2 already added there), then add:

```rust
    #[test]
    fn review_stat_entries_lists_only_nonzero_categories_in_display_order() {
        let entries = review_stat_entries(&["Book", "Book", "Blunder", "Best"]);
        let as_pairs: Vec<(String, i32)> = entries
            .into_iter()
            .map(|e| (e.label.to_string(), e.count))
            .collect();
        assert_eq!(
            as_pairs,
            vec![
                ("Best".to_string(), 1),
                ("Book".to_string(), 2),
                ("Blunder".to_string(), 1),
            ]
        );
    }

    #[test]
    fn review_stat_entries_empty_for_no_classified_moves() {
        assert!(review_stat_entries(&[]).is_empty());
    }

    #[test]
    fn accuracy_summary_text_matches_existing_format() {
        assert_eq!(
            accuracy_summary_text(Some(87.4)),
            "Accuracy 87% | Est. Elo 2300"
        );
        assert_eq!(
            accuracy_summary_text(None),
            "Accuracy pending | Est. Elo pending"
        );
    }
```

(The exact display order asserted above — Best, Book, Blunder — must match whatever `ORDER` constant Step 3 defines; adjust the expected `Vec` if you order the categories differently, but keep the order consistent with what the Brief UI in Task 5 expects: Book, Brilliant, Best, Great, Excellent, Good, Miss, Mistake, Blunder.)

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --bin app review_stat_entries`
Expected: FAIL to compile — not defined yet.

- [ ] **Step 3: Add the `ReviewStatEntry` struct**

In the `slint::slint! { ... }` macro block, alongside `GameEntry`/`OpeningLineEntry`/`HistoryEntry` (~line 40):

```slint
    struct ReviewStatEntry {
        label: string,
        count: int,
    }
```

- [ ] **Step 4: Implement `accuracy_summary_text` and refactor `game_review_summary` to use it**

Find `game_review_summary` (~line 1140):

```rust
fn game_review_summary(accuracy: Option<f32>, classes: &[&str]) -> String {
    let count = |label: &str| classes.iter().filter(|&&class| class == label).count();
    let accuracy_text = match accuracy {
        Some(value) => format!("Accuracy {:.0}% | Est. Elo {}", value, estimated_elo(value)),
        None => "Accuracy pending | Est. Elo pending".to_string(),
    };
    format!(
        "{accuracy_text} | Book {} | Blunder {} | Mistake {} | Miss {} | Good {} | Excellent {} | Best {} | Great {} | Brilliant {}",
        count("Book"),
        count("Blunder"),
        count("Mistake"),
        count("Miss"),
        count("Good"),
        count("Excellent"),
        count("Best"),
        count("Great"),
        count("Brilliant"),
    )
}
```

Replace with:

```rust
fn accuracy_summary_text(accuracy: Option<f32>) -> String {
    match accuracy {
        Some(value) => format!("Accuracy {:.0}% | Est. Elo {}", value, estimated_elo(value)),
        None => "Accuracy pending | Est. Elo pending".to_string(),
    }
}

fn game_review_summary(accuracy: Option<f32>, classes: &[&str]) -> String {
    let count = |label: &str| classes.iter().filter(|&&class| class == label).count();
    format!(
        "{} | Book {} | Blunder {} | Mistake {} | Miss {} | Good {} | Excellent {} | Best {} | Great {} | Brilliant {}",
        accuracy_summary_text(accuracy),
        count("Book"),
        count("Blunder"),
        count("Mistake"),
        count("Miss"),
        count("Good"),
        count("Excellent"),
        count("Best"),
        count("Great"),
        count("Brilliant"),
    )
}
```

- [ ] **Step 5: Implement `review_stat_entries`**

Add next to `game_review_summary`:

```rust
/// Non-zero move-classification counts, in a fixed display order, for the
/// Game Review Brief's stat grid. Mirrors the 9 engine_controller::MoveClass
/// variants exactly (there is no "Perfect" class).
fn review_stat_entries(classes: &[&str]) -> Vec<ReviewStatEntry> {
    const ORDER: [&str; 9] = [
        "Best", "Book", "Blunder", "Brilliant", "Excellent", "Good", "Great", "Miss", "Mistake",
    ];
    ORDER
        .iter()
        .filter_map(|&label| {
            let count = classes.iter().filter(|&&class| class == label).count();
            if count == 0 {
                None
            } else {
                Some(ReviewStatEntry {
                    label: slint::SharedString::from(label),
                    count: count as i32,
                })
            }
        })
        .collect()
}
```

(This uses alphabetical `ORDER` to match the test in Step 1 exactly. If you'd rather display "Book, Brilliant, Best, Great, Excellent, Good, Miss, Mistake, Blunder" — closer to strength-descending, matching the per-move badge order already used in the Review screen's timeline — reorder `ORDER` accordingly and update the Step 1 test's expected `Vec` to match. Keep this file's `ORDER` and Task 5's rendering order consistent with each other.)

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test --bin app`
Expected: all previous tests plus the 3 new ones pass, and `game_review_summary_lists_accuracy_elo_and_move_counts` still passes unchanged (confirms the refactor is behavior-preserving).

- [ ] **Step 7: Commit**

```bash
git add app/src/main.rs
git commit -m "feat(app): add structured move-classification stats helper"
```

---

### Task 4: Auto-trigger analysis at game end (Rust plumbing)

**Files:**
- Modify: `app/src/main.rs`

**Interfaces:**
- Consumes: `bot_game_outcome`, `build_bot_game_pgn` (Task 2).
- Produces: `finish_play_game(state: &mut AppState, app: &AppWindow, reason: &str) -> Option<String>` (signature changes from `-> ()` to `-> Option<String>` — the PGN to auto-review, if there were any moves played). Produces: `fn import_and_analyze_pgn(state: &Arc<Mutex<AppState>>, app_weak: &slint::Weak<AppWindow>, refresh_data: &Rc<dyn Fn()>, pgn_text: &str, switch_to_review: bool)` — the import+analyze pipeline extracted from `on_import_pgn`, parameterized so the auto-trigger path can run it *without* jumping to the Review screen. Task 5 depends on both.

**Why this needs care:** `finish_play_game` currently runs with the state mutex already locked by its caller (a `MutexGuard`, not the `Arc<Mutex<AppState>>` handle). Triggering the import pipeline needs the `Arc` handle and must happen *after* that lock is released — exactly the pattern `on_play_review_game` already uses (`drop(st)` before `invoke_import_pgn`). Rather than have `finish_play_game` itself reach for the `Arc` while its caller's guard is still alive (a deadlock risk), it now returns the PGN string and lets each of its 3 call sites drop their lock and trigger the import themselves.

- [ ] **Step 1: Replace `on_import_pgn`'s registration with a call to a new shared function**

Find `on_import_pgn`'s registration (currently ~line 1745) — its full body, from `app.on_import_pgn(move |pgn_text: slint::SharedString| {` down to its matching `});`. Replace the entire block with:

```rust
        app.on_import_pgn(move |pgn_text: slint::SharedString| {
            import_and_analyze_pgn(&state, &app_weak, &refresh_data_cp, &pgn_text, true);
        });
```

Then, as a new free function (place it right before `fn ingest_pgn_game`, ~line 2949), paste the **entire previous body** of `on_import_pgn` in, with these mechanical changes: (a) function signature as below, (b) parameters replace the closure's captured variables (`state` here is the `&Arc<Mutex<AppState>>` parameter, not a clone — inside the function body, clones for the background thread now clone from this parameter instead of an outer `state.clone()`), (c) the final block's `app.set_active_screen(...)` call is gated by `switch_to_review`:

```rust
/// Import a finished/pasted PGN, persist it, and spawn Stockfish analysis in
/// the background. `switch_to_review` controls whether the UI jumps to the
/// Review screen immediately (manual "Import & Analyze Game" / "Review This
/// Game" clicks) or stays put (auto-triggered right after a bot game, where
/// the Game Review Brief overlay is shown instead — see Task 5).
fn import_and_analyze_pgn(
    state: &Arc<Mutex<AppState>>,
    app_weak: &slint::Weak<AppWindow>,
    refresh_data_cp: &Rc<dyn Fn()>,
    pgn_text: &str,
    switch_to_review: bool,
) {
    if pgn_text.trim().is_empty() {
        return;
    }

    let mut state_lock = state.lock().unwrap();
    let parsed = pgn_processor::parse_pgn(pgn_text);

    let white = parsed
        .headers
        .get("White")
        .cloned()
        .unwrap_or_else(|| "WhitePlayer".to_string());
    let black = parsed
        .headers
        .get("Black")
        .cloned()
        .unwrap_or_else(|| "BlackPlayer".to_string());
    let date = parsed
        .headers
        .get("Date")
        .cloned()
        .unwrap_or_else(|| "2026-06-12".to_string());
    let game_id = imported_game_id(&white, &black, &date, &parsed.moves);

    if let Err(e) = ingest_pgn_game(
        &mut state_lock.db,
        &game_id,
        "Imported PGN",
        pgn_text,
        Some(date),
    ) {
        drop(state_lock);
        if let Some(app) = app_weak.upgrade() {
            app.set_review_status(slint::SharedString::from(format!("Import failed: {e}")));
        }
        return;
    }

    // Trigger background thread to analyze and evaluate this game!
    let sf_path = state_lock.stockfish_path.clone();
    let state_thread = state.clone();
    let refresh_thread = refresh_data_cp.clone();
    let game_id_thread = game_id.clone();
    let app_weak_thread = app_weak.clone();

    thread::spawn(move || {
        let mut sf = match StockfishEngine::new(&sf_path) {
            Ok(engine) => engine,
            Err(e) => {
                let app_weak_error = app_weak_thread.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = app_weak_error.upgrade() {
                        app.set_review_status(slint::SharedString::from(format!(
                            "Stockfish failed: {e}"
                        )));
                    }
                });
                return;
            }
        };

        let positions = {
            let state = state_thread.lock().unwrap();
            state
                .db
                .get_positions_for_game(&game_id_thread)
                .unwrap_or_default()
        };

        let mut losses = Vec::new();

        for pos in &positions {
            let is_book = {
                let state = state_thread.lock().unwrap();
                is_book_move(&state.db, &pos.fen, &pos.played_move)
            };
            let eval_res = sf.analyze_fen(
                &pos.fen,
                AnalysisConfig {
                    depth: 8,
                    multipv: 3,
                },
            );
            if let Ok(analysis) = eval_res {
                if !analysis.variations.is_empty() {
                    let best_score = score_to_centipawns(analysis.variations[0].score);

                    let played_score = if analysis.bestmove == pos.played_move {
                        best_score
                    } else if let Some(v) = analysis
                        .variations
                        .iter()
                        .find(|var| !var.pv.is_empty() && var.pv[0] == pos.played_move)
                    {
                        score_to_centipawns(v.score)
                    } else {
                        let mut played_chess: Chess = pos
                            .fen
                            .parse::<sk_fen::Fen>()
                            .map(|f| f.into_position(CastlingMode::Standard).unwrap())
                            .unwrap();
                        let uci: Uci = pos.played_move.parse().unwrap();
                        let m = uci.to_move(&played_chess).unwrap();
                        played_chess.play_unchecked(&m);
                        let next_fen = shakmaty::fen::Fen::from_position(
                            played_chess,
                            shakmaty::EnPassantMode::Always,
                        )
                        .to_string();

                        if let Ok(next_analysis) = sf.analyze_fen(
                            &next_fen,
                            AnalysisConfig {
                                depth: 8,
                                multipv: 1,
                            },
                        ) {
                            if !next_analysis.variations.is_empty() {
                                -score_to_centipawns(next_analysis.variations[0].score)
                            } else {
                                best_score
                            }
                        } else {
                            best_score
                        }
                    };

                    let loss = (best_score - played_score).max(0);
                    losses.push(loss);

                    let class = Some(format!(
                        "{:?}",
                        engine_controller::classify_review_move(
                            loss,
                            analysis.bestmove == pos.played_move,
                            best_score,
                            is_book,
                        )
                    ));

                    let mut state = state_thread.lock().unwrap();
                    let mut updated_pos = pos.clone();
                    updated_pos.centipawn_loss = Some(loss);
                    updated_pos.mistake_class = class;
                    let _ = state.db.insert_position(&updated_pos);
                }
            } else {
                losses.push(0);
            }
        }

        let acc_input: Vec<(u32, Option<i32>)> = positions
            .iter()
            .zip(losses.iter())
            .map(|(pos, &loss)| (pos.ply, Some(loss)))
            .collect();
        let acc = accuracy::compute_game_accuracy(&acc_input);
        {
            let mut state = state_thread.lock().unwrap();
            let _ = state.db.update_game_accuracy(
                &game_id_thread,
                acc.overall,
                acc.opening,
                acc.middlegame,
                acc.endgame,
            );
        }

        let user_side = tree::user_side_for_game(&white, &black);
        let worst = positions
            .iter()
            .zip(losses.iter())
            .filter(|(pos, _)| match &user_side {
                Some(s) => (pos.ply % 2 == 1) == (s == "White"),
                None => true,
            })
            .max_by_key(|(_, loss)| **loss);
        if let Some((pos_record, &loss)) = worst {
            if loss >= 60 {
                let mover_is_white = pos_record.fen.split_whitespace().nth(1) == Some("w");
                let side = if mover_is_white { "White" } else { "Black" }.to_string();
                if let Ok(root_analysis) = sf.analyze_fen(
                    &pos_record.fen,
                    AnalysisConfig {
                        depth: 12,
                        multipv: 1,
                    },
                ) {
                    let best = root_analysis.bestmove.clone();
                    let mut st = state_thread.lock().unwrap();
                    if let Ok((_, after_best, _)) = tree::add_move_edge(
                        &mut st.db,
                        &pos_record.fen,
                        &best,
                        &side,
                        "mistake",
                    ) {
                        drop(st);
                        if let Ok(reply_analysis) = sf.analyze_fen(
                            &after_best,
                            AnalysisConfig {
                                depth: 10,
                                multipv: 3,
                            },
                        ) {
                            let mut seen = std::collections::HashSet::new();
                            for var in reply_analysis.variations.iter().take(3) {
                                let Some(reply) = var.pv.first() else {
                                    continue;
                                };
                                if !seen.insert(reply.clone()) {
                                    continue;
                                }
                                let mut st = state_thread.lock().unwrap();
                                let Ok((_, after_reply, _)) = tree::add_move_edge(
                                    &mut st.db,
                                    &after_best,
                                    reply,
                                    &side,
                                    "mistake",
                                ) else {
                                    continue;
                                };
                                drop(st);
                                if let Ok(fu) = sf.analyze_fen(
                                    &after_reply,
                                    AnalysisConfig {
                                        depth: 10,
                                        multipv: 1,
                                    },
                                ) {
                                    let mut st = state_thread.lock().unwrap();
                                    let _ = tree::add_move_edge(
                                        &mut st.db,
                                        &after_reply,
                                        &fu.bestmove,
                                        &side,
                                        "mistake",
                                    );
                                }
                            }
                        }
                        let mut st = state_thread.lock().unwrap();
                        let event = TrainingEventRecord {
                            id: format!("evt_{}", uuid_now()),
                            user_id: "default_user".to_string(),
                            kind: "mistake_tree".to_string(),
                            target_id: game_id_thread.clone(),
                            outcome: format!("worst drop {loss}cp at ply {}", pos_record.ply),
                            score_delta: 0.0,
                            created_at: tree::local_now_str(),
                        };
                        let _ = st.db.insert_training_event(&event);
                    }
                }
            }
        }

        // Trigger UI refresh
        let _ = slint::invoke_from_event_loop(move || {
            refresh_thread();
            if let Some(app) = app_weak_thread.upgrade() {
                app.invoke_select_game(slint::SharedString::from(game_id_thread));
            }
        });
    });

    // Local updates
    drop(state_lock);
    refresh_data_cp();
    if let Some(app) = app_weak.upgrade() {
        if switch_to_review {
            app.set_active_screen(slint::SharedString::from("review"));
        }
        app.set_review_status(slint::SharedString::from("Analyzing with Stockfish..."));
        app.invoke_select_game(slint::SharedString::from(game_id));
    }
}
```

Note `app.invoke_select_game(...)` still runs even when `switch_to_review` is false — this populates the Brief's/Review screen's board+timeline data in the background regardless of which screen is visible, so it's ready the moment the user does navigate there. Only the screen switch itself is gated.

- [ ] **Step 2: Build-check**

Run: `cargo check --bin app`
Expected: likely fails here with "cannot find value `Rc` / borrow of moved value" type errors specific to whatever the closure previously captured by move vs. what the new function now takes by reference — fix by following the existing file's pattern (parameters are references; clone them locally exactly where the old closure used to clone its captures, e.g. `let state_thread = state.clone();` already shown above expects `state: &Arc<Mutex<AppState>>`, so `state.clone()` clones the `Arc` itself — this matches `Arc<T>: Clone`). Iterate until it compiles; this refactor changes *how* the code is invoked, not its logic, so a compiling result should behave identically to before for the `switch_to_review: true` path.

- [ ] **Step 3: Change `finish_play_game` to return the PGN instead of doing nothing with it**

Find `finish_play_game` (~line 2927):

```rust
/// End the bot game: build the summary and leave the moves for optional review.
fn finish_play_game(state: &mut AppState, app: &AppWindow, reason: &str) {
    app.set_play_active(false);
    let total = state.play_moves_uci.len();
    let mut summary = format!(
        "{reason} after {total} plies. In book: {} of your moves.",
        state.play_plies_in_book
    );
    if let Some(ply) = state.play_left_book_at {
        summary.push_str(&format!(" Left book at ply {}.", ply + 1));
    }
    if state.play_deviations.is_empty() {
        summary.push_str(" No deviations from your repertoire — clean prep!");
    } else {
        summary.push_str(&format!(
            " Deviations (now due for review): {}",
            state.play_deviations.join(" | ")
        ));
    }
    app.set_play_summary(slint::SharedString::from(summary));
}
```

Replace with:

```rust
/// End the bot game: build the summary, determine the outcome, show the Game
/// Review Brief (Task 5 wires the Slint side of this), and return the PGN to
/// auto-analyze — the caller must drop its state lock before importing it
/// (see the call sites below).
fn finish_play_game(state: &mut AppState, app: &AppWindow, reason: &str) -> Option<String> {
    app.set_play_active(false);
    let total = state.play_moves_uci.len();
    let mut summary = format!(
        "{reason} after {total} plies. In book: {} of your moves.",
        state.play_plies_in_book
    );
    if let Some(ply) = state.play_left_book_at {
        summary.push_str(&format!(" Left book at ply {}.", ply + 1));
    }
    if state.play_deviations.is_empty() {
        summary.push_str(" No deviations from your repertoire — clean prep!");
    } else {
        summary.push_str(&format!(
            " Deviations (now due for review): {}",
            state.play_deviations.join(" | ")
        ));
    }
    app.set_play_summary(slint::SharedString::from(summary));

    let (result_tag, header) = bot_game_outcome(&state.play_chess, &state.play_side, reason == "Resigned");
    app.set_game_review_result_text(slint::SharedString::from(header));
    app.set_show_game_review_brief(true);
    app.set_review_status(slint::SharedString::from("Analyzing with Stockfish..."));

    build_bot_game_pgn(&state.play_moves_uci, &state.play_side, result_tag)
}
```

(`set_game_review_result_text`/`set_show_game_review_brief` are new `AppWindow` properties added in Task 5 — this task will not compile standalone until Task 5's property declarations exist. If executing Task 4 before Task 5, add the two property declarations now as a small preview of Task 5's Step 1 — `in-out property <bool> show-game-review-brief: false;` and `in-out property <string> game-review-result-text: "";` — placed near `review-status` in `AppWindow`; Task 5 will add the remaining Brief properties/UI around them.)

- [ ] **Step 4: Update all 3 call sites to drop the lock, then trigger the import**

Call site A — inside `on_play_click_square` (~line 2793):

Find:

```rust
            if st.play_chess.is_game_over() {
                finish_play_game(&mut st, &app, "Game over");
                return;
            }
            let msg = bot_take_turn(&mut st, &app);
            let ended =
                msg.contains("ended") || msg.contains("Engine") || st.play_chess.is_game_over();
            app.set_play_status(slint::SharedString::from(msg));
            if ended {
                finish_play_game(&mut st, &app, "Game over");
            }
        });
    }
```

Replace with:

```rust
            if st.play_chess.is_game_over() {
                let pgn = finish_play_game(&mut st, &app, "Game over");
                drop(st);
                if let Some(pgn) = pgn {
                    import_and_analyze_pgn(&state, &app_weak, &refresh_data, &pgn, false);
                }
                return;
            }
            let msg = bot_take_turn(&mut st, &app);
            let ended =
                msg.contains("ended") || msg.contains("Engine") || st.play_chess.is_game_over();
            app.set_play_status(slint::SharedString::from(msg));
            if ended {
                let pgn = finish_play_game(&mut st, &app, "Game over");
                drop(st);
                if let Some(pgn) = pgn {
                    import_and_analyze_pgn(&state, &app_weak, &refresh_data, &pgn, false);
                }
            }
        });
    }
```

This closure (`on_play_click_square`, registered ~line 2712) captures `state` and `app_weak` already (`let state = state.clone(); let app_weak = app_weak.clone();` right before `app.on_play_click_square(move |idx: i32| {`); check whether `refresh_data` is captured there too — if not, add `let refresh_data = refresh_data.clone();` alongside those two lines, matching how other callbacks in this file (e.g. `on_import_pgn`'s own registration) already clone it in.

Call site B — `on_play_resign` (~line 2810):

Find:

```rust
        app.on_play_resign(move || {
            let mut st = state.lock().unwrap();
            let app = app_weak.upgrade().unwrap();
            finish_play_game(&mut st, &app, "Resigned");
        });
```

Replace with:

```rust
        app.on_play_resign(move || {
            let mut st = state.lock().unwrap();
            let app = app_weak.upgrade().unwrap();
            let pgn = finish_play_game(&mut st, &app, "Resigned");
            drop(st);
            if let Some(pgn) = pgn {
                import_and_analyze_pgn(&state, &app_weak, &refresh_data, &pgn, false);
            }
        });
```

Same check: this closure's capture block (just above `app.on_play_resign(move || {`) needs `state`, `app_weak`, and `refresh_data` all cloned in; add whichever are missing.

- [ ] **Step 5: Build and fix any capture errors**

Run: `cargo check --bin app`
Expected: the compiler will report any missing closure captures directly (e.g. "cannot find value `refresh_data` in this scope") — add the corresponding `let refresh_data = refresh_data.clone();` line to that closure's setup block (immediately before its `app.on_*(move |...| {` line) and re-check until it compiles clean.

- [ ] **Step 6: Run the full test suite**

Run: `cargo test --bin app`
Expected: all tests pass (26 from Tasks 2+3, plus whatever Task 3 added — this task adds no new unit tests itself, since `finish_play_game`/`import_and_analyze_pgn` are UI-wiring functions exercised via manual verification in Task 5, not unit tests).

- [ ] **Step 7: Commit**

```bash
git add app/src/main.rs
git commit -m "feat(app): auto-trigger analysis when a bot game ends"
```

---

### Task 5: Game Review Brief overlay (Slint UI)

**Files:**
- Modify: `app/src/main.rs`

**Interfaces:**
- Consumes: `ReviewStatEntry` (Task 3), `review_stat_entries`/`accuracy_summary_text` (Task 3), `show-game-review-brief`/`game-review-result-text` properties and the `Option<String>`-returning `finish_play_game` (Task 4).
- Produces: `AppWindow` properties `game-review-game-id: string`, `game-review-stats: [ReviewStatEntry]`, `game-review-accuracy-text: string`; callbacks `game-review-open()`, `game-review-close()`. Consumed by Task 6.

- [ ] **Step 1: Add the remaining `AppWindow` properties**

Near `review-status` (~line 121), alongside the two properties Task 4 may have already added:

```slint
        in-out property <bool> show-game-review-brief: false;
        in-out property <string> game-review-game-id: "";
        in-out property <string> game-review-result-text: "";
        in-out property <string> game-review-accuracy-text: "";
        in-out property <[ReviewStatEntry]> game-review-stats: [];
        callback game-review-open();
        callback game-review-close();
```

(If Task 4 already added `show-game-review-brief`/`game-review-result-text` as a preview, only add the remaining new lines here.)

- [ ] **Step 2: Add the `GameReviewBrief` component**

Place this right after the `ChessBoard` component (Task 1) and before `export component AppWindow`:

```slint
    component GameReviewBrief inherits Rectangle {
        in property <string> result-text;
        in property <bool> ready;
        in property <string> status-text;
        in property <string> accuracy-text;
        in property <[ReviewStatEntry]> stats;
        callback review-clicked();
        callback close-clicked();

        background: #000000aa;

        Rectangle {
            width: 380px;
            x: (parent.width - self.width) / 2;
            y: (parent.height - self.height) / 2;
            background: #1a1a1e;
            border-radius: 12px;
            border-color: #3f3f46;
            border-width: 1px;

            VerticalLayout {
                padding: 24px;
                spacing: 16px;

                HorizontalLayout {
                    alignment: end;
                    Rectangle {
                        width: 24px;
                        height: 24px;
                        Text {
                            text: "✕";
                            color: #a1a1aa;
                            font-size: 16px;
                        }
                        TouchArea {
                            clicked => { root.close-clicked(); }
                        }
                    }
                }

                Text {
                    text: root.result-text;
                    color: #ffffff;
                    font-size: 20px;
                    font-weight: 800;
                    horizontal-alignment: center;
                }

                if !root.ready : Text {
                    text: root.status-text;
                    color: #a78bfa;
                    font-size: 13px;
                    wrap: word-wrap;
                    horizontal-alignment: center;
                }

                if root.ready : Text {
                    text: root.accuracy-text;
                    color: #e4e4e7;
                    font-size: 13px;
                    horizontal-alignment: center;
                }

                if root.ready : HorizontalLayout {
                    spacing: 8px;
                    alignment: center;
                    for stat in root.stats: Rectangle {
                        background: #27272a;
                        border-radius: 6px;
                        width: 72px;
                        height: 56px;
                        VerticalLayout {
                            alignment: center;
                            Text {
                                text: stat.count;
                                color: #ffffff;
                                font-size: 18px;
                                font-weight: 800;
                                horizontal-alignment: center;
                            }
                            Text {
                                text: stat.label;
                                color: #a1a1aa;
                                font-size: 10px;
                                horizontal-alignment: center;
                            }
                        }
                    }
                }

                if root.ready : Button {
                    text: "Game Review";
                    clicked => { root.review-clicked(); }
                }
            }
        }
    }

```

- [ ] **Step 3: Instantiate it as an overlay over the main content area**

Find the end of the "6. Play vs Bot Screen" block and the container that wraps all six `if (root.active-screen == "...")` blocks (currently ~lines 1084-1086):

```slint
                    }
                }
            }
        }
    }
}
```

The middle `}` (currently line 1086) closes the container holding all six screen blocks. Insert the overlay as one more child of that same container, right before that `}`:

```slint
                    }
                }

                if root.show-game-review-brief : GameReviewBrief {
                    width: 100%;
                    height: 100%;
                    result-text: root.game-review-result-text;
                    ready: root.game-review-stats.length > 0 || root.game-review-accuracy-text != "";
                    status-text: root.review-status;
                    accuracy-text: root.game-review-accuracy-text;
                    stats: root.game-review-stats;
                    review-clicked => { root.game-review-open(); }
                    close-clicked => { root.game-review-close(); }
                }
            }
        }
    }
}
```

This places the brief inside the content area (covers every screen except the left navigation sidebar, per the design), drawn last so it's on top.

Build-check immediately: `cargo check --bin app`. If this specific brace turns out not to be the one closing the six-screen container (Slint will report a clear parse error naming the mismatched brace/line), open the file and locate the correct one — it's the first `}` that appears after "6. Play vs Bot Screen"'s own closing brace and before the outer `HorizontalLayout`'s closing brace (the one that also closes the Sidebar's sibling).

- [ ] **Step 4: Wire `game-review-open` and `game-review-close`**

Add near the other simple callback registrations (a good spot: right after `on_play_resign`, before `on_play_review_game`):

```rust
    {
        let app_weak = app_weak.clone();
        app.on_game_review_close(move || {
            if let Some(app) = app_weak.upgrade() {
                app.set_show_game_review_brief(false);
            }
        });
    }

    {
        let app_weak = app_weak.clone();
        app.on_game_review_open(move || {
            if let Some(app) = app_weak.upgrade() {
                let game_id = app.get_game_review_game_id();
                app.set_active_screen(slint::SharedString::from("review"));
                app.invoke_select_game(game_id);
                app.set_show_game_review_brief(false);
            }
        });
    }
```

- [ ] **Step 5: Populate the Brief from the analysis-thread completion callback**

Inside `import_and_analyze_pgn` (Task 4), the background thread's completion callback currently ends with:

```rust
        // Trigger UI refresh
        let _ = slint::invoke_from_event_loop(move || {
            refresh_thread();
            if let Some(app) = app_weak_thread.upgrade() {
                app.invoke_select_game(slint::SharedString::from(game_id_thread));
            }
        });
    });
```

The thread computed `losses` locally but wrote each position's `mistake_class` straight to the DB without keeping the labels in a local variable. Re-fetch the now-updated positions right before the final refresh — simpler than threading a parallel `Vec<String>` through the loop above, and guaranteed consistent with what `on_select_game` will show for this same game:

```rust
        let updated_positions = {
            let state = state_thread.lock().unwrap();
            state
                .db
                .get_positions_for_game(&game_id_thread)
                .unwrap_or_default()
        };
        let class_names: Vec<&str> = updated_positions
            .iter()
            .filter_map(|pos| pos.mistake_class.as_deref())
            .collect();
        let stats = review_stat_entries(&class_names);
        let accuracy = {
            let state = state_thread.lock().unwrap();
            state
                .db
                .conn
                .query_row(
                    "SELECT accuracy_overall FROM games WHERE id = ?1",
                    [game_id_thread.as_str()],
                    |row| row.get::<_, Option<f64>>(0),
                )
                .ok()
                .flatten()
                .map(|value| value as f32)
        };
        let accuracy_text = accuracy_summary_text(accuracy);

        // Trigger UI refresh
        let _ = slint::invoke_from_event_loop(move || {
            refresh_thread();
            if let Some(app) = app_weak_thread.upgrade() {
                app.invoke_select_game(slint::SharedString::from(game_id_thread.clone()));
                if app.get_show_game_review_brief() && app.get_game_review_game_id().is_empty() {
                    // First population right after auto-trigger: record which
                    // game this open brief refers to.
                    app.set_game_review_game_id(slint::SharedString::from(game_id_thread));
                }
                app.set_game_review_stats(slint::ModelRc::from(Rc::new(slint::VecModel::from(
                    stats,
                ))));
                app.set_game_review_accuracy_text(slint::SharedString::from(accuracy_text));
            }
        });
    });
```

This mirrors the `on_select_game`'s existing accuracy query exactly (same SQL, same pattern), so it stays consistent with what the Review screen itself shows.

Note the `game_id_thread == ""` check: this callback runs for *every* import (manual paste-imports too, not just bot games), so it should only claim `game-review-game-id` when a brief is actually open and not yet attributed to a game. Task 6 (on-demand entry point) sets `game-review-game-id` directly and doesn't go through this thread at all, so it isn't affected by this check.

- [ ] **Step 6: Also set `game-review-game-id` right when the brief opens (auto-trigger path)**

In Task 4's `finish_play_game`, after `app.set_show_game_review_brief(true);`, add:

```rust
    app.set_game_review_game_id(slint::SharedString::from(""));
```

This clears any stale id from a previous game before the new one is known (the analysis thread fills it in once `ingest_pgn_game` has computed the real id — see Step 5's check above). Also reset the stats/accuracy so a stale previous game's grid doesn't flash before the new analysis completes:

```rust
    app.set_game_review_stats(slint::ModelRc::from(Rc::new(slint::VecModel::from(
        Vec::<ReviewStatEntry>::new(),
    ))));
    app.set_game_review_accuracy_text(slint::SharedString::from(""));
```

- [ ] **Step 7: Build, run, and manually verify**

Run: `cargo build --bin app`, then `cargo run --bin app`.

Manual check: go to Play vs Bot, start a game, play (or resign) to the end. Confirm: the brief overlay appears immediately with the correct "You beat Forge Bot!"/"Forge Bot won"/"Draw" header and an "Analyzing your game…" state; after a few seconds it fills in with the stat grid and accuracy/Elo text; clicking "Game Review" navigates to the Review screen with this game selected; clicking X dismisses it and the Play vs Bot screen's existing "Review This Game" button still works afterward (re-triggering import is harmless — same `imported_game_id`, `ON CONFLICT` no-ops/updates safely).

- [ ] **Step 8: Commit**

```bash
git add app/src/main.rs
git commit -m "feat(app): add Game Review Brief overlay, shown after bot games"
```

---

### Task 6: On-demand "Review" entry point in the Reviewed Games list

**Files:**
- Modify: `app/src/main.rs`

**Interfaces:**
- Consumes: `review_stat_entries`, `accuracy_summary_text` (Task 3); `show-game-review-brief`, `game-review-game-id`, `game-review-result-text`, `game-review-stats`, `game-review-accuracy-text` properties (Task 5).
- Produces: callback `show-game-review-brief-for(string)` (takes a `game_id`).

- [ ] **Step 1: Add a "Review" button to each `GameEntry` row**

Find the Reviewed Games list row (currently ~lines 652-672):

```slint
                                    for game in root.games: Rectangle {
                                        background: root.selected-game-id == game.id ? #3f3f46 : #27272a;
                                        border-radius: 8px;
                                        height: 85px;

                                        TouchArea {
                                            clicked => {
                                                root.select-game(game.id);
                                            }
                                        }

                                        VerticalLayout {
                                            alignment: space-between;
                                            Text { text: game.white + " vs " + game.black; color: #ffffff; font-size: 13px; font-weight: 700; }
                                            HorizontalLayout {
                                                alignment: space-between;
                                                Text { text: game.date; color: #a1a1aa; font-size: 11px; }
                                                Text { text: game.result; color: #fbbf24; font-size: 11px; font-weight: 700; }
                                            }
                                        }
                                    }
```

Replace with (adds a small "Review" button anchored top-right, on top of the existing whole-row `TouchArea`; the button's own `TouchArea` intercepts the click before it reaches the row's):

```slint
                                    for game in root.games: Rectangle {
                                        background: root.selected-game-id == game.id ? #3f3f46 : #27272a;
                                        border-radius: 8px;
                                        height: 85px;

                                        TouchArea {
                                            clicked => {
                                                root.select-game(game.id);
                                            }
                                        }

                                        VerticalLayout {
                                            alignment: space-between;
                                            Text { text: game.white + " vs " + game.black; color: #ffffff; font-size: 13px; font-weight: 700; }
                                            HorizontalLayout {
                                                alignment: space-between;
                                                Text { text: game.date; color: #a1a1aa; font-size: 11px; }
                                                Text { text: game.result; color: #fbbf24; font-size: 11px; font-weight: 700; }
                                            }
                                        }

                                        Rectangle {
                                            x: parent.width - self.width - 8px;
                                            y: 8px;
                                            width: 56px;
                                            height: 22px;
                                            background: #3f3f46;
                                            border-radius: 4px;
                                            Text {
                                                text: "Review";
                                                color: #e4e4e7;
                                                font-size: 10px;
                                                horizontal-alignment: center;
                                                vertical-alignment: center;
                                            }
                                            TouchArea {
                                                clicked => {
                                                    root.show-game-review-brief-for(game.id);
                                                }
                                            }
                                        }
                                    }
```

- [ ] **Step 2: Declare the new callback**

Near the other callback declarations (e.g. next to `callback play-review-game();`, ~line 216):

```slint
        callback show-game-review-brief-for(string);
```

- [ ] **Step 3: Wire it in Rust**

Add near the `on_select_game` registration (after it is fine):

```rust
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_show_game_review_brief_for(move |game_id: slint::SharedString| {
            let state = state.lock().unwrap();
            let Some(app) = app_weak.upgrade() else {
                return;
            };

            let positions = state
                .db
                .get_positions_for_game(&game_id)
                .unwrap_or_default();
            let class_names: Vec<&str> = positions
                .iter()
                .filter_map(|pos| pos.mistake_class.as_deref())
                .collect();
            let accuracy = state
                .db
                .conn
                .query_row(
                    "SELECT accuracy_overall FROM games WHERE id = ?1",
                    [game_id.as_str()],
                    |row| row.get::<_, Option<f64>>(0),
                )
                .ok()
                .flatten()
                .map(|value| value as f32);
            let games = state.db.get_games().unwrap_or_default();
            drop(state);

            let Some(game) = games.into_iter().find(|g| g.id == game_id.as_str()) else {
                return;
            };
            let user_side = tree::user_side_for_game(&game.white, &game.black);
            let is_bot_game = game.white == "You" && game.black == "Forge Bot"
                || game.white == "Forge Bot" && game.black == "You";
            let won = match (&user_side, game.result.as_deref()) {
                (Some(side), Some("1-0")) => side == "White",
                (Some(side), Some("0-1")) => side == "Black",
                _ => false,
            };
            let drawn = matches!(game.result.as_deref(), Some("1/2-1/2"));
            let header = if drawn {
                "Draw".to_string()
            } else if is_bot_game {
                if won {
                    "You beat Forge Bot!".to_string()
                } else {
                    "Forge Bot won".to_string()
                }
            } else if won {
                "You won".to_string()
            } else {
                "You lost".to_string()
            };

            app.set_game_review_game_id(game_id.clone());
            app.set_game_review_result_text(slint::SharedString::from(header));
            app.set_game_review_stats(slint::ModelRc::from(Rc::new(slint::VecModel::from(
                review_stat_entries(&class_names),
            ))));
            app.set_game_review_accuracy_text(slint::SharedString::from(accuracy_summary_text(
                accuracy,
            )));
            app.set_show_game_review_brief(true);
        });
    }
```

- [ ] **Step 4: Build, run, and manually verify**

Run: `cargo build --bin app`, then `cargo run --bin app`.

Manual check: go to Game Review screen, confirm each row in "Reviewed Games" now shows a small "Review" button. Click it on a couple of different past games (including a bot game, if one exists from Task 5's manual test, and any Lichess/Chess.com-imported game if present) — the brief should open directly in its ready state (no "Analyzing…" flash) with that game's own header/stats/accuracy, and clicking "Game Review" should select that same game's timeline. Confirm clicking the rest of the row (not the Review button) still loads the timeline inline as before.

- [ ] **Step 5: Run the full test suite one more time**

Run: `cargo test --bin app`
Expected: all tests still pass (this task adds no new pure-Rust logic worth a unit test — it's a DB read + property population, exercised by the manual check above).

- [ ] **Step 6: Commit**

```bash
git add app/src/main.rs
git commit -m "feat(app): add on-demand Game Review Brief from the games list"
```
