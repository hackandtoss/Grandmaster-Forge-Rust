# Post-Game Review Polish — Design

**Date:** 2026-07-06
**Status:** Approved by user (brainstorming dialogue, all sections)

> **Repo drift notice (2026-07-07): STILL VALID, BUT NARROW.**
> This spec covers review presentation polish. The broader product direction now requires review labels, accuracy, eval swings, and deterministic coach notes to feed course/line weakness, FSRS scheduling, and the tactical scrutinizer.
> Do not use this spec to infer a flat repertoire model or standalone review-only analytics.

## Context

While play-testing the bot-play feature from
[2026-07-01-repertoire-tree-and-bot-play-design.md](2026-07-01-repertoire-tree-and-bot-play-design.md),
three problems surfaced:

1. **Bug (found and fixed during this session, before this design):** bot games
   build their `game_id` from the static labels `White="You"`/`Black="Forge Bot"`
   plus a date-only string. Two bot games played on the same calendar day
   collided on id; `ON CONFLICT DO NOTHING` (games table) and a partial
   `ON CONFLICT DO UPDATE` that never touches `fen`/`played_move` (positions
   table) meant the second game's "Review This Game" silently showed the first
   game's stale moves — i.e. looked like it "did nothing." Fixed by folding a
   hash of the move list into the id (`imported_game_id()` in `app/src/main.rs`);
   regression test `same_day_bot_games_get_distinct_ids_and_do_not_collide`.
2. The post-game summary is plain text (plies in book, deviations) per the
   original design's scope — never designed to be the richer
   Great/Best/Miss-style brief chess.com shows. This is a scope gap, not a bug.
3. The chessboard is a fixed 360×360px grid, identically duplicated across all
   four screens that render one (Dashboard drill, Game Review, Opening
   Builder, Play vs Bot), so it never grows to use available window space.

This design covers items 2 and 3.

## Goal

Make the review experience closer to chess.com's: a board that scales with
the window on every screen, and a real "Game Review" moment (result banner +
move-classification brief + CTA) — reachable both automatically right after a
bot game finishes and on demand for any already-reviewed game — instead of a
quiet screen switch with a line of gray text.

## 1. Shared `ChessBoard` component

**Problem:** `app/src/main.rs` has the same 64-square loop, fixed at
`width: 360px; height: 360px;` with `45px` cells, copy-pasted at four call
sites (Dashboard drill view, Game Review timeline, Opening Builder, Play vs
Bot). Fixing scaling in only one or two places would leave the rest visibly
inconsistent.

**Design:** extract one reusable Slint component:

```
component ChessBoard inherits Rectangle {
    in property <[string]> pieces;
    in property <int> selected-square: -1;
    in property <color> highlight-color: #fcd34d;
    in property <bool> interactive: true;
    callback square-clicked(int);

    // Stay square: shrink to the smaller of whatever space the layout gives us.
    property <length> board-size: max(360px, min(560px, min(root.width, root.height)));
    property <length> cell-size: board-size / 8;
    ...
    // centered inner Rectangle holding the 64-square `for` loop, cell-size
    // and glyph font-size both derive from `cell-size` so nothing stretches
}
```

- `pieces`, `selected-square`, `highlight-color`, and `square-clicked` are
  passed through from each screen's existing state properties/callbacks
  unchanged — this is a rendering-layer extraction, not a data-model change.
- The Game Review timeline board stays non-interactive (`interactive: false`,
  no `square-clicked` wiring) since it's driven by timeline selection, not
  direct clicks, today.
- `piece-symbol`/`piece-color` (currently `pure function`s on `AppWindow`)
  move onto `ChessBoard` itself so the component is self-contained; removed
  from `AppWindow` if nothing else still needs them there.
- Sizing: fills whatever space its containing layout cell gives it, floored
  at 360px (today's fixed size) and soft-capped at 560px so it doesn't dwarf
  the surrounding panel on an ultrawide window. Stays perfectly square and
  centered within its allocated box; cell size and piece glyph `font-size`
  both scale off the same `cell-size` computation.
- Each of the four call sites drops its `width: 360px; height: 360px;`
  wrapper so the surrounding layout can actually give the board room to grow.

## 2. Game Review Brief (auto post-game + on demand per game)

The brief is one reusable component with two entry points: automatically
right after a bot game ends, or on demand for any individual game already in
the "Reviewed Games" list. Both show the same header/stat-grid/CTA; they only
differ in how they're triggered and whether an analysis wait is involved.

**Entry point A — auto post-game:** today, Stockfish analysis of a finished
bot game only starts when the user clicks "Review This Game"
(`on_play_review_game` -> `invoke_import_pgn` -> the analysis thread in
`on_import_pgn`). To show the brief immediately, `finish_play_game` will kick
off that same import-and-analyze pipeline automatically as soon as the game
ends, at the same depth/multipv as today's manual path (depth 8, multipv 3,
unchanged mistake-tree follow-up) — no throttling for the auto-triggered case.
The brief opens right away in its **analyzing** state (see below) since the
background thread has only just started.

**Entry point B — on demand from the game list:** each `GameEntry` row in the
"Reviewed Games" list gains a small "Review" button/icon alongside its
existing whole-row click (which keeps loading the full timeline inline,
unchanged). Clicking "Review" fires a new `on_show_game_review_brief(game_id)`
callback that reads that game's already-computed classification counts and
accuracy straight from the DB (the same query `on_select_game` already runs)
and opens the brief directly in its **ready** state — no analysis wait, since
every game in this list has already been through the pipeline.

**The brief component**, shown as an overlay over whichever screen triggered
it:

- **Header:** derived from the game's stored `white`/`black`/`result` plus
  `tree::user_side_for_game(white, black)` (already used by the mistake-tree
  pipeline) to phrase "You won" / "You lost" / "Draw". When the opponent is
  the book bot (`white`/`black` matching the `"You"`/`"Forge Bot"` pattern),
  use the friendlier "You beat Forge Bot!" / "Forge Bot won" / "Draw" instead
  — same underlying data, nicer phrasing when we know the opponent's
  identity. `reason: &str` (`finish_play_game`'s existing parameter, e.g.
  `"Game over"`/`"Resigned"`) does not itself carry who won, so
  `finish_play_game` gains an explicit outcome determination —
  `state.play_chess.outcome()` (shakmaty) mapped against `state.play_side`,
  plus the resign case (resigning side always loses).

  This outcome also needs to reach the **stored** game, not just the live
  brief: today `on_play_review_game` hardcodes the PGN's result tag as `*`
  (`{movetext}*`), so a bot game's `result` column is never anything but
  `"*"` once persisted — entry point B's header logic (reading `white`/
  `black`/`result` back from a `GameEntry` row later) would have nothing
  usable to work with. Fixing the auto-trigger path means building that PGN
  with the real `1-0`/`0-1`/`1/2-1/2` tag from the same outcome
  determination, so the stored `result` is correct for both entry points,
  not just the live one.
- **Analyzing state** (entry point A only): while the background thread is
  still running, shows an "Analyzing your game…" status (reusing the
  existing `review-status` string/pattern).
- **Ready state:** a compact grid of the non-zero move-classification counts
  — Book / Brilliant / Best / Great / Excellent / Good / Miss / Mistake /
  Blunder, exactly the 9 `engine_controller::MoveClass` variants (there is no
  "Perfect" — the Review screen's per-move badge color-mapping checks for one
  anyway, a pre-existing harmless dead branch this design doesn't touch) —
  plus accuracy and estimated Elo (already computed by
  `accuracy::compute_game_accuracy` / `estimated_elo`).
- **"Game Review" button:** navigates to the existing Review screen with this
  game selected — same destination `on_play_review_game` already reaches
  today (`set_active_screen("review")` + `invoke_select_game`).
- **Close (X):** dismisses without navigating. After dismissal (or after
  navigating away and back), entry point A's screen (Play vs Bot) keeps a
  persistent, non-modal "Game Review" affordance pointing at the same
  already-analyzed game, and entry point B's "Review" button remains on its
  row — the brief is a one-time notification, not the only way in.

**Data plumbing:** a new small Rust helper alongside the existing
`game_review_summary()` (`app/src/main.rs`) that returns the per-category
counts as an ordered list of `(label, count)` pairs, skipping zero-count
categories, instead of one flattened string. Bound to a new Slint struct
(`ReviewStatEntry { label: string, count: int }`), following the same
list-of-struct pattern already used for `GameEntry`/`OpeningLineEntry`/
`HistoryEntry`, so the brief can render it with a `for stat in
root.game-review-stats: ...` grid.

New `AppWindow` properties: `show-game-review-brief: bool`,
`game-review-game-id: string` (which game the open brief refers to, so its
CTA knows what to select), `game-review-result-text: string`,
`game-review-stats: [ReviewStatEntry]`. Both entry points populate the same
properties. Existing `review-status`, `selected-game-summary`, and the
accuracy query already used by `on_select_game` are reused rather than
duplicated.

## 3. Data flow

**Entry point A (auto post-game):**

1. Bot game ends (checkmate/resign/etc.) -> `finish_play_game` builds the
   existing plies-in-book/deviations summary (unchanged), determines the
   win/loss/draw outcome, sets `show-game-review-brief = true` /
   `game-review-game-id` with a result-based header and the "Analyzing…"
   state, and spawns the same import+analyze background thread
   `on_import_pgn` uses today.
2. Thread finishes (per-move classification + accuracy, same as today) ->
   `invoke_from_event_loop` populates `game-review-stats` and flips the brief
   to its ready state, exactly as `on_select_game`/`refresh_thread` already
   refresh other screens today.

**Entry point B (on demand from the list):**

1. User clicks "Review" on a `GameEntry` row -> `on_show_game_review_brief`
   queries that game's stored classification counts + accuracy (already
   computed, same query `on_select_game` runs) and sets
   `show-game-review-brief = true` / `game-review-game-id` / `game-review-stats`
   directly in the ready state — no background thread, no analyzing wait.

**Shared from here:**

3. User clicks "Game Review" -> same navigation `on_play_review_game` already
   performs (`set_active_screen("review")` + `invoke_select_game` with
   `game-review-game-id`) -> brief hides.
4. User clicks X instead -> brief hides, `show-game-review-brief` resets;
   entry point A's Play vs Bot screen keeps its persistent, non-modal
   "Game Review" affordance, entry point B's row keeps its "Review" button —
   either way the game remains reachable after the brief closes.

## 4. Error handling

Follows the existing pattern (background threads post status strings, never
block the UI thread):

- Stockfish unavailable (entry point A only): the brief's "Analyzing…" state
  is replaced by the existing `"Stockfish failed: {e}"`-style status text;
  the close X still works, and the Play screen's fallback affordance
  reflects the failure rather than silently retrying.
- Entry point B always reads already-persisted data, so it has no analysis
  failure mode of its own; if a listed game somehow has no accuracy yet
  (analysis still mid-flight from elsewhere), it shows the same "Analyzing
  with Stockfish…" status `on_select_game` already falls back to today.
- Everything else (malformed PGN, engine crash mid-analysis) already has
  handling in the reused `on_import_pgn` pipeline; this design adds no new
  failure modes there since it calls the same code, just earlier.

## 5. Testing

- **Stats helper (Rust):** unit test analogous to the existing
  `game_review_summary_lists_accuracy_elo_and_move_counts`, asserting the
  structured list contains the expected `(label, count)` pairs and omits
  zero-count categories.
- **ChessBoard (Slint):** no new Rust logic beyond the existing pure
  `piece-symbol`/`piece-color` functions moved onto the component, so no new
  Rust unit tests; verify visually (`cargo run`, resize the window) on all
  four screens.
- **Manual verification checklist:** play a bot game to completion; confirm
  the brief appears in an analyzing state, then fills in with real counts;
  confirm "Game Review" navigates correctly; confirm X dismisses and the Play
  screen still offers a way back into the same game. Separately, from the
  Reviewed Games list, click "Review" on a few different past games and
  confirm the brief opens directly in its ready state (no analyzing wait)
  with that row's own stats and header, and that its "Game Review" button
  selects the right game. Resize the window and confirm the board scales
  (with its floor/ceiling) on all four screens.

## Out of scope

- Visually matching chess.com's full chrome (avatars, animated reveal,
  per-move coach commentary bubbles) — static header + stat grid only.
- Making the Game Review timeline board directly clickable/interactive.
- Any change to SM-2 grading, deviation detection, or the mistake-tree
  pipeline — this design only changes *when* the existing analysis runs and
  *how* its results are surfaced.
- Weighted/animated board square highlighting beyond the existing
  selected-square color swap.
- A "Review" entry point anywhere other than the Reviewed Games list (e.g.
  Dashboard recent-activity widgets) — can extend the same callback later if
  wanted.
