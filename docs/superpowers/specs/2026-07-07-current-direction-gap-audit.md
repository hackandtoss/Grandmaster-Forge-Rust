# Current Direction Gap Audit

**Date:** 2026-07-07
**Status:** Working audit for planning

## Direction We Chose

Grandmaster Forge should become a full chess improvement workbench:

- opening courses like ChessReps, but backed by the existing transposition graph;
- named courses, chapters, and lines, not a flat list;
- strict line mastery: any due required move makes the line `Review Due`;
- FSRS as the target scheduler, replacing SM-2 later;
- game review with chess.com-style labels and deterministic coach notes;
- tactical, puzzle, endgame, bot, and analytics features feeding one learning loop.

## Already Present In Code

- `db_manager`: SQLite schema for `repertoire_nodes`, `repertoire_moves`, `games`, `positions`, `puzzles`, `training_events`, `opening_tree`, `game_sync`.
- `app/src/tree.rs`: graph insertion, PGN/study import into graph, per-edge grading, legacy `opening_lines` migration.
- `app/src/srs.rs`: SM-2 scheduler baseline.
- `engine_controller`: Stockfish analysis, multi-PV, move classes, `PlaySession`.
- `app/src/main.rs`: six-screen Slint app, game review, builder, import, book bot, puzzle callback, source-grouped "course" display.

## Missing Before New Specs Can Be Implemented Cleanly

### 1. Real Course Metadata

The app currently fakes courses by grouping repertoire moves by `(side, source)` in `app/src/main.rs`.

Missing:

- `courses`;
- `course_chapters`;
- `course_lines`;
- `course_line_moves`;
- CRUD/query helpers;
- course-aware line import target.

First plan: `docs/superpowers/plans/2026-07-07-opening-course-metadata-and-status.md`.

### 2. Derived Line Status

The app has per-move due state, but no line-level status.

Missing:

- `Undiscovered`;
- `Learning`;
- `Learned`;
- `Mastered`;
- `Review Due`;
- `Weak`;
- course progress derived from child lines.

First plan: `docs/superpowers/plans/2026-07-07-opening-course-metadata-and-status.md`.

### 3. Course UI And Import Routing

The visible repertoire browser still shows source buckets instead of real course cards/chapters/lines.

Missing:

- catalog cards from `courses`;
- course detail screen/state;
- course/chapter/line target for imports;
- manual builder save target;
- "Uncategorized" fallback course.

Second plan: `docs/superpowers/plans/2026-07-07-course-ui-and-import-routing.md`.

### 4. Review Event Spine

`training_events` exists but is too generic for FSRS, tactics, puzzles, endgames, and bot deviations to share one learning model.

Missing:

- normalized review event ratings;
- target type/id;
- source context;
- optional course/line/move linkage;
- future FSRS input shape.

Third plan: `docs/superpowers/plans/2026-07-07-review-event-spine.md`.

## Not First

- FSRS implementation: wait until review events exist.
- Zobrist: wait until profiling says normalized-FEN lookup is hot.
- `linfa`: wait until tactical labels exist.
- Community marketplace: wait until local course pack format is stable.
- Board themes/sounds/confetti: not learning-critical.
