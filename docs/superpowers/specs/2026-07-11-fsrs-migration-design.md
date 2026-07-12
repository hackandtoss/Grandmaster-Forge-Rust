# FSRS Migration Design

Approved direction 2026-07-11. Replaces SM-2 with FSRS for repertoire-move scheduling, graded automatically from observed drill behavior — no self-report buttons. Refines `docs/superpowers/specs/2026-07-07-learning-scheduler-and-position-identity-design.md` (FSRS Migration Rule) into an implementable design.

## Problem

SM-2 (1987) uses a fixed one-size-fits-all curve: it does not model per-item memory, handles early/overdue reviews poorly (chess drilling is never on a rigid schedule), and repeated failures trap edges in permanently short intervals. FSRS models each item with explicit Difficulty/Stability/Retrievability state, was parameter-fit on hundreds of millions of real reviews, and can later be re-fit per user from our `review_events` log — which was built as its input (PR #11).

## Decisions

### Engine: `rs-fsrs` crate, scheduler only

- Add `rs-fsrs` 1.2 (MIT, ~1.5K SLoC, deps: `chrono` + optional `serde`) to the `app` crate. Default published FSRS parameters.
- The `fsrs` crate (Anki's, with the ML optimizer) is **deferred**: it drags in the `burn` ML framework. Per-user parameter fitting from `review_events` is a future offline step, not part of this migration.
- `rs-fsrs` implements the FSRS-4.5/5-era formulas (last release 2024-10). Accepted: the SM-2→FSRS jump dwarfs formula-revision deltas.
- Per the scheduler spec's ponytail constraint: no scheduler trait. `app/src/srs.rs` (SM-2) is **deleted** and replaced by a small `app/src/fsrs.rs` adapter around `rs-fsrs` (rating mapping, epoch-days ↔ `DateTime<Utc>` conversion, new-edge initialization via `Card::new()`).

### Automatic rating (no buttons)

| Observed drill behavior | FSRS rating |
|---|---|
| Wrong move | Again (1) |
| Correct, but hesitant — completed move more than `HESITATION_SECS = 15` seconds after the position was presented | Hard (2) |
| Correct | Good (3) |
| — | Easy (4) unused in V1: a mis-assigned Easy over-stretches intervals; revisit with real data |
| Bot-play deviation | Again (1) |

- Hesitation is measured in `AppState`: a timestamp recorded when the drill position is shown/advanced, read when the move completes (second click). The threshold is a named constant; tuning it is cheap and the future optimizer makes intervals self-correcting regardless.
- `review_events.rating` stores the FSRS rating (1–4) from cutover onward. Historical events (fail=1, pass=5) stay valid: 1 still means failure; the future optimizer maps 5→Good.
- **Weak-status threshold change**: `move_is_weak` currently counts `rating <= 2` as failure. Under the FSRS scale Hard=2 is a *success*; the threshold becomes `rating <= 1` (Again only). Historical data is unaffected (old failures are 1, old passes are 5).

### State: two new columns, shared read columns unchanged

- `repertoire_moves` gains `fsrs_stability REAL`, `fsrs_difficulty REAL`, and `fsrs_last_review TEXT` (date of the last FSRS-graded review — required to compute elapsed time when reconstructing the memory state), all nullable; `NULL` = no FSRS memory state yet.
- Day granularity: the app schedules by calendar date (`srs_due_date` is a date string). FSRS's sub-day learning steps collapse to "due today"; the adapter reconstructs cards in Review state whenever stability exists.
- **`srs_due_date` and `srs_reps` remain the shared scheduling columns FSRS writes** (reps: increment on success, reset on Again — same semantics as today). Every reader is therefore untouched: the due-drill queue (`get_due_repertoire_moves`), `course_line_status`, `course_progress`, Weak derivation, and confidence displays.
- `srs_interval` and `srs_ease` become frozen legacy columns after cutover: never read by the scheduler again, retained (not dropped) as migration provenance.

### Grading path

`tree::grade_edge_with_event` keeps its shape (edge, rating, day, source) but its rating parameter becomes an FSRS rating, so call sites change values: the drill pass path calls a new pure helper `drill_rating(correct, elapsed_secs)` (Again/Hard/Good) instead of hardcoding 5, the drill fail loop and bot-deviation path pass Again. Internally it updates the edge via the `fsrs.rs` adapter — load or initialize the Card from (`fsrs_stability`, `fsrs_difficulty`, `fsrs_last_review`, `srs_due_date`), apply the rating, persist new stability/difficulty/last-review/due/reps. `grade_edge` (SM-2) and `srs.rs` are removed in the same change — "FSRS should replace SM-2, not run beside it."

### Migration (idempotent, at bootstrap)

For user-owned edges with SM-2 history (`srs_reps > 0` or a non-empty due date) and `fsrs_stability IS NULL`:

- `fsrs_stability = max(srs_interval, 0.1)` — an edge that was due in N days plausibly has ~N days of stability; conservative and monotone.
- `fsrs_difficulty` = a monotone map from `srs_ease` (≈1.3 hard … 3.0 easy) onto FSRS difficulty (1 easy … 10 hard), clamped; exact constants pinned in the plan with a unit test of monotonicity and clamping.
- **`srs_due_date` is not modified.** The scheduler-spec invariant "no due work disappears" holds by construction; a test proves the due queue is byte-identical before/after migration.
- Never-reviewed edges stay `NULL` and get their FSRS state from `Card::new()` on first grade.

## Out of scope

Puzzle scheduling (puzzles remain random-pick under a difficulty ceiling), the `fsrs` optimizer / per-user parameter fitting, the Easy rating, self-report grading UI, motifs as scheduled skills, Zobrist, any scheduler abstraction layer.

## Testing (per the scheduler spec's mandates)

- Again ⇒ due again within a day; Good repeatedly ⇒ due dates move out monotonically (interval growth).
- Hard grows the interval less than Good from identical state.
- Migration: seeded SM-2 edges keep their exact due dates; due queue identical before/after; conversion is idempotent (second run is a no-op).
- Hesitation mapping: pure function `drill_rating(correct: bool, elapsed_secs: u64) -> Rating` unit-tested at the boundary (15s).
- Weak threshold: Hard (2) events do not mark a move weak; Again (1) events do.
- Full workspace stays green without network or a real Stockfish; existing transposition/status tests unchanged.
