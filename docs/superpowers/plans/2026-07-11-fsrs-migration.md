# FSRS Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace SM-2 with FSRS for repertoire-move scheduling, graded automatically from drill behavior (wrong → Again, slow-correct → Hard, correct → Good), migrating existing SM-2 state without losing any due work.

**Architecture:** Per `docs/superpowers/specs/2026-07-11-fsrs-migration-design.md`. `rs-fsrs` 1.2 with `enable_short_term: false` (LongtermScheduler — every interval ≥ 1 day, matching our date-string scheduling) and `enable_fuzz: false` (deterministic tests). Three nullable FSRS columns on `repertoire_moves`; `srs_due_date`/`srs_reps` stay the shared columns every reader already uses, so the due queue, course statuses, Weak, and progress logic don't change. `app/src/srs.rs` (SM-2) is deleted; `app/src/fsrs.rs` is the new adapter.

**Tech Stack:** Rust 2021, `rs-fsrs = "1.2"` (+ `chrono = "0.4"` for date conversion), rusqlite 0.31.

## Global Constraints

- Zero clippy warnings, `cargo fmt --all -- --check` clean (baseline since PR #12).
- `cargo test --workspace` is 84 now; deleting `srs.rs` removes its 4 tests, the two old grading tests merge into one, and this plan adds 6 — end state 85 (verified). No network, no real Stockfish.
- `review_events.rating` stores FSRS ratings (1–3) from cutover; the Weak rule becomes `rating <= 1`.
- rs-fsrs API facts (verified from source, 2026-07-11): `Rating { Again=1, Hard=2, Good=3, Easy=4 }`; `Card { due: DateTime<Utc>, stability: f64, difficulty: f64, elapsed_days: i64, scheduled_days: i64, reps: i32, lapses: i32, state: State, last_review: DateTime<Utc> }`; `State { New=0, Learning, Review, Relearning }`; `FSRS::new(Parameters)`; `fsrs.repeat(card, now) -> HashMap<Rating, SchedulingInfo>` where `SchedulingInfo.card` is the post-review card.

---

### Task 1: FSRS columns, update helper, and conservative migration (db_manager)

**Files:**
- Modify: `db_manager/src/lib.rs`

**Interfaces:**
- Produces: `RepertoireMoveRecord` gains `pub fsrs_stability: Option<f64>`, `pub fsrs_difficulty: Option<f64>`, `pub fsrs_last_review: Option<String>`; `pub fn update_repertoire_move_fsrs(&mut self, move_id: i64, stability: f64, difficulty: f64, last_review: &str, reps: u32, due_date: &str) -> Result<(), String>`; `pub fn ease_to_difficulty(ease: f32) -> f64`; migration runs inside `bootstrap()`.

- [x] **Step 1: Add failing round-trip + migration test**

```rust
#[test]
fn fsrs_columns_round_trip_and_migration_preserves_due_queue() {
    let mut store = SqliteStore::new_in_memory().unwrap();
    store.bootstrap().unwrap();
    let root = store.ensure_repertoire_node("a w - - 0 1", "White").unwrap();
    let child = store.ensure_repertoire_node("b b - - 0 1", "White").unwrap();
    let (move_id, _) = store
        .insert_repertoire_move(root, child, "e2e4", "e4", true, "manual")
        .unwrap();

    // New edge: no FSRS state yet.
    let edge = store.get_repertoire_move(move_id).unwrap().unwrap();
    assert_eq!(edge.fsrs_stability, None);

    // Simulate SM-2 history (interval 6, ease 2.5, 2 reps, due 2026-07-15)...
    store
        .update_repertoire_move_srs(move_id, 6.0, 2.5, 2, "2026-07-15")
        .unwrap();
    // ...then re-run bootstrap: the migration must seed FSRS state
    // without touching the due date, and be idempotent.
    store.bootstrap().unwrap();
    let migrated = store.get_repertoire_move(move_id).unwrap().unwrap();
    assert_eq!(migrated.srs_due_date, "2026-07-15");
    assert_eq!(migrated.fsrs_stability, Some(6.0));
    let d = migrated.fsrs_difficulty.unwrap();
    assert!((1.0..=10.0).contains(&d));
    // last_review backdated by the interval: 2026-07-15 - 6 days.
    assert_eq!(migrated.fsrs_last_review.as_deref(), Some("2026-07-09"));

    let before = migrated.clone();
    store.bootstrap().unwrap();
    assert_eq!(store.get_repertoire_move(move_id).unwrap().unwrap(), before);

    // Direct FSRS write updates both FSRS state and shared columns.
    store
        .update_repertoire_move_fsrs(move_id, 9.5, 4.2, "2026-07-16", 3, "2026-07-26")
        .unwrap();
    let updated = store.get_repertoire_move(move_id).unwrap().unwrap();
    assert_eq!(updated.fsrs_stability, Some(9.5));
    assert_eq!(updated.fsrs_last_review.as_deref(), Some("2026-07-16"));
    assert_eq!(updated.srs_reps, 3);
    assert_eq!(updated.srs_due_date, "2026-07-26");
}

#[test]
fn ease_to_difficulty_is_monotone_and_clamped() {
    assert!((ease_to_difficulty(1.3) - 10.0).abs() < 1e-6);
    assert!((ease_to_difficulty(3.0) - 1.0).abs() < 1e-6);
    assert!(ease_to_difficulty(2.0) > ease_to_difficulty(2.5));
    assert_eq!(ease_to_difficulty(0.5), 10.0); // clamped low ease
    assert_eq!(ease_to_difficulty(9.9), 1.0); // clamped high ease
}
```

- [x] **Step 2: Run** `cargo test -p db_manager fsrs` — expected: compile failure (missing fields/functions).

- [x] **Step 3: Implement**

1. In the `repertoire_moves` CREATE TABLE inside `SQLITE_SCHEMA`, append after `srs_due_date`:
   `fsrs_stability REAL, fsrs_difficulty REAL, fsrs_last_review TEXT,` (fresh databases).
2. Existing databases: in `bootstrap()` (the `TrainingStore` impl), after the schema loop, run each of the three `ALTER TABLE repertoire_moves ADD COLUMN ...` statements, ignoring only the "duplicate column name" error:

```rust
for stmt in [
    "ALTER TABLE repertoire_moves ADD COLUMN fsrs_stability REAL",
    "ALTER TABLE repertoire_moves ADD COLUMN fsrs_difficulty REAL",
    "ALTER TABLE repertoire_moves ADD COLUMN fsrs_last_review TEXT",
] {
    if let Err(e) = self.conn.execute(stmt, []) {
        if !e.to_string().contains("duplicate column name") {
            return Err(e.to_string());
        }
    }
}
self.migrate_sm2_to_fsrs()?;
```

3. Add the three `Option` fields to `RepertoireMoveRecord` and extend **every** SELECT that constructs it (grep `RepertoireMoveRecord {` in `db_manager/src/lib.rs` — at least `get_repertoire_move`, `get_repertoire_moves_from`, and `get_due_repertoire_moves`) with the three columns.
4. Pure helper + migration + update fn in `impl SqliteStore`:

```rust
/// SM-2 ease (~1.3 hard .. 3.0 easy) -> FSRS difficulty (10 hard .. 1 easy), monotone, clamped.
pub fn ease_to_difficulty(ease: f32) -> f64 {
    (10.0 - (ease as f64 - 1.3) * (9.0 / 1.7)).clamp(1.0, 10.0)
}

/// Seed FSRS state for edges that have SM-2 history but no FSRS state yet.
/// Conservative: stability = old interval, due dates untouched, last review
/// backdated by the interval. Idempotent (WHERE fsrs_stability IS NULL).
fn migrate_sm2_to_fsrs(&mut self) -> Result<(), String> {
    self.conn
        .execute(
            "UPDATE repertoire_moves SET
                fsrs_stability = MAX(srs_interval, 0.1),
                fsrs_difficulty = MIN(MAX(10.0 - (srs_ease - 1.3) * (9.0 / 1.7), 1.0), 10.0),
                fsrs_last_review = date(
                    CASE WHEN srs_due_date <> '' THEN srs_due_date ELSE date('now') END,
                    '-' || CAST(ROUND(MAX(srs_interval, 1.0)) AS INTEGER) || ' days')
             WHERE is_my_move = 1 AND fsrs_stability IS NULL
               AND (srs_reps > 0 OR srs_due_date <> '')",
            [],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn update_repertoire_move_fsrs(
    &mut self,
    move_id: i64,
    stability: f64,
    difficulty: f64,
    last_review: &str,
    reps: u32,
    due_date: &str,
) -> Result<(), String> {
    self.conn
        .execute(
            "UPDATE repertoire_moves
             SET fsrs_stability = ?1, fsrs_difficulty = ?2, fsrs_last_review = ?3,
                 srs_reps = ?4, srs_due_date = ?5
             WHERE id = ?6",
            rusqlite::params![stability, difficulty, last_review, reps, due_date, move_id],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}
```

The SQL difficulty formula intentionally mirrors `ease_to_difficulty`; the unit test pins the Rust helper and the round-trip test pins the SQL against it (interval 6/ease 2.5 case).

- [x] **Step 4: Run** `cargo test -p db_manager` — expected: all pass (23: 21 existing + 2 new).

- [x] **Step 5: Weak threshold under the FSRS scale**

In `move_is_weak`, change `failures += i32::from(rating <= 2);` to `failures += i32::from(rating <= 1);` and update its doc comment (failure = Again; Hard is a success). In the `course_line_status_derives_weak_from_recent_event_history` test, add after the bot-deviation assertion: two `rating: 2` (`"drill"`) events dated `2026-07-14`/`2026-07-15` on `move_id` — the line must NOT be Weak (window is `[2, 2, bot_dev... ]` — wait, the bot_deviation from `2026-07-13` is still in the last-3 window and alone marks it weak; use `move2`'s line instead: give `line2`'s move an SRS update so it's discovered, then two Hard events → expect its SRS-derived status, not `"Weak"`).

- [x] **Step 6: Run** `cargo test -p db_manager` (all green), then commit:

```bash
git add db_manager/src/lib.rs
git commit -m "feat(db): FSRS state columns, conservative SM-2 migration, Weak threshold on FSRS scale"
```

### Task 2: FSRS adapter module (app)

**Files:**
- Modify: `app/Cargo.toml`
- Create: `app/src/fsrs.rs`
- Modify: `app/src/main.rs` (add `mod fsrs;` next to `mod srs;` — `srs` is removed in Task 3)

**Interfaces:**
- Consumes: `RepertoireMoveRecord` FSRS fields (Task 1), `crate::tree::days_to_ymd`.
- Produces: `pub use rs_fsrs::Rating;` re-export; `pub const HESITATION_SECS: u64 = 15;` `pub fn drill_rating(correct: bool, elapsed_secs: u64) -> Rating`; `pub struct EdgeSchedule { pub stability: f64, pub difficulty: f64, pub last_review: String, pub reps: u32, pub due_date: String }`; `pub fn next_schedule(edge: &db_manager::RepertoireMoveRecord, rating: Rating, today_days: u64) -> EdgeSchedule`.

- [x] **Step 1: Add dependencies**

In `app/Cargo.toml` under `[dependencies]`:

```toml
rs-fsrs = "1.2"
chrono = "0.4"
```

- [x] **Step 2: Write failing tests** (inside `app/src/fsrs.rs`, `#[cfg(test)] mod tests`)

```rust
use super::*;

fn edge_with(stability: Option<f64>, difficulty: Option<f64>, last_review: Option<&str>, due: &str, reps: u32) -> db_manager::RepertoireMoveRecord {
    db_manager::RepertoireMoveRecord {
        fsrs_stability: stability,
        fsrs_difficulty: difficulty,
        fsrs_last_review: last_review.map(str::to_string),
        srs_due_date: due.to_string(),
        srs_reps: reps,
        // fill every remaining field with its Default/zero value; id 1,
        // parent/child 1/2, uci "e2e4", san "e4", is_my_move true, source "manual",
        // srs_interval 1.0, srs_ease 2.5 (copy the struct literal from a
        // db_manager test if field names drift).
        ..fresh_edge()
    }
}

// 2026-07-01 is epoch day 20635 (same anchor as tree.rs tests).
const TODAY: u64 = 20635;

#[test]
fn drill_rating_maps_correctness_and_hesitation() {
    assert_eq!(drill_rating(false, 0), Rating::Again);
    assert_eq!(drill_rating(false, 99), Rating::Again);
    assert_eq!(drill_rating(true, HESITATION_SECS), Rating::Good); // boundary: exactly 15s is Good
    assert_eq!(drill_rating(true, HESITATION_SECS + 1), Rating::Hard);
    assert_eq!(drill_rating(true, 3), Rating::Good);
}

#[test]
fn again_comes_due_within_a_day_and_resets_reps() {
    let edge = edge_with(Some(20.0), Some(5.0), Some("2026-06-21"), "2026-07-11", 4);
    let s = next_schedule(&edge, Rating::Again, TODAY);
    assert_eq!(s.reps, 0);
    assert!(s.due_date.as_str() <= "2026-07-02", "failed edges return fast, got {}", s.due_date);
    assert_eq!(s.last_review, "2026-07-01");
}

#[test]
fn repeated_good_moves_due_dates_out_monotonically() {
    let new_edge = edge_with(None, None, None, "", 0);
    let first = next_schedule(&new_edge, Rating::Good, TODAY);
    assert_eq!(first.reps, 1);
    assert!(first.due_date.as_str() > "2026-07-01");

    let edge2 = edge_with(Some(first.stability), Some(first.difficulty), Some(first.last_review.as_str()), &first.due_date, first.reps);
    let second = next_schedule(&edge2, Rating::Good, TODAY + 10);
    assert_eq!(second.reps, 2);
    assert!(second.due_date > first.due_date);
    assert!(second.stability > first.stability);
}

#[test]
fn hard_grows_less_than_good_from_identical_state() {
    let edge = edge_with(Some(10.0), Some(5.0), Some("2026-06-21"), "2026-07-01", 2);
    let hard = next_schedule(&edge, Rating::Hard, TODAY);
    let good = next_schedule(&edge, Rating::Good, TODAY);
    assert!(hard.due_date < good.due_date);
    assert!(hard.difficulty > good.difficulty);
    assert_eq!(hard.reps, 3); // Hard is a success: reps advance
}
```

(`fresh_edge()` is a small test helper constructing a fully-populated `RepertoireMoveRecord`; write it in the tests module.)

- [x] **Step 3: Run** `cargo test -p app fsrs` — expected: compile failure.

- [x] **Step 4: Implement `app/src/fsrs.rs`**

```rust
//! FSRS scheduling adapter: date-string persistence <-> rs-fsrs cards.
//! Long-term scheduler only (enable_short_term = false): every interval is
//! >= 1 day, matching the app's calendar-date scheduling. Fuzz disabled so
//! schedules (and tests) are deterministic.

use chrono::{DateTime, NaiveDate, Utc};
use rs_fsrs::{Card, Parameters, State, FSRS};

pub use rs_fsrs::Rating;

/// Correct but slower than this many seconds grades Hard instead of Good.
pub const HESITATION_SECS: u64 = 15;

pub fn drill_rating(correct: bool, elapsed_secs: u64) -> Rating {
    if !correct {
        Rating::Again
    } else if elapsed_secs > HESITATION_SECS {
        Rating::Hard
    } else {
        Rating::Good
    }
}

pub struct EdgeSchedule {
    pub stability: f64,
    pub difficulty: f64,
    pub last_review: String,
    pub reps: u32,
    pub due_date: String,
}

fn scheduler() -> FSRS {
    FSRS::new(Parameters {
        enable_short_term: false,
        enable_fuzz: false,
        ..Parameters::default()
    })
}

fn days_to_datetime(days: u64) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(days as i64 * 86_400, 0).expect("valid epoch day")
}

fn date_str_to_datetime(date: &str, fallback_days: u64) -> DateTime<Utc> {
    NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map(|d| d.and_hms_opt(0, 0, 0).expect("midnight").and_utc())
        .unwrap_or_else(|_| days_to_datetime(fallback_days))
}

fn datetime_to_date_str(dt: DateTime<Utc>) -> String {
    let (y, m, d) = crate::tree::days_to_ymd((dt.timestamp() / 86_400) as u64);
    format!("{:04}-{:02}-{:02}", y, m, d)
}

/// Apply one FSRS review to an edge's persisted state. `srs_reps` keeps the
/// app-level semantics every status reader depends on: Again resets to 0,
/// any success increments.
pub fn next_schedule(
    edge: &db_manager::RepertoireMoveRecord,
    rating: Rating,
    today_days: u64,
) -> EdgeSchedule {
    let now = days_to_datetime(today_days);
    let card = match (edge.fsrs_stability, edge.fsrs_difficulty) {
        (Some(stability), Some(difficulty)) => {
            let last_review = date_str_to_datetime(
                edge.fsrs_last_review.as_deref().unwrap_or(""),
                today_days.saturating_sub(1),
            );
            let due = date_str_to_datetime(&edge.srs_due_date, today_days);
            Card {
                due,
                stability,
                difficulty,
                elapsed_days: (now - last_review).num_days().max(0),
                scheduled_days: (due - last_review).num_days().max(0),
                reps: edge.srs_reps as i32,
                lapses: 0,
                state: State::Review,
                last_review,
            }
        }
        _ => Card::new(),
    };
    let next = scheduler().repeat(card, now)[&rating].card.clone();
    EdgeSchedule {
        stability: next.stability,
        difficulty: next.difficulty,
        last_review: datetime_to_date_str(now),
        reps: if rating == Rating::Again {
            0
        } else {
            edge.srs_reps + 1
        },
        due_date: datetime_to_date_str(next.due),
    }
}
```

Add `mod fsrs;` in `app/src/main.rs` beside the other module declarations.

- [x] **Step 5: Run** `cargo test -p app fsrs` — expected: 4 pass. Then commit:

```bash
git add app/Cargo.toml app/src/fsrs.rs app/src/main.rs Cargo.lock
git commit -m "feat(app): rs-fsrs adapter with automatic drill ratings"
```

### Task 3: Cut grading over to FSRS; delete SM-2 (tree.rs, srs.rs)

**Files:**
- Modify: `app/src/tree.rs`
- Delete: `app/src/srs.rs`
- Modify: `app/src/main.rs` (remove `mod srs;`)

**Interfaces:**
- Consumes: `crate::fsrs::{next_schedule, Rating}` (Task 2), `update_repertoire_move_fsrs` (Task 1).
- Produces: `pub fn grade_edge_with_event(db, edge, rating: crate::fsrs::Rating, today_days: u64, source: &str) -> Result<(), String>` — the only grading entry point; `grade_edge` and `crate::srs` no longer exist.

- [x] **Step 1: Rewrite the grading tests in `app/src/tree.rs`**

Replace `grade_edge_pass_and_fail` and adjust `grade_edge_records_review_event`:

```rust
#[test]
fn grading_schedules_via_fsrs_and_records_events() {
    let mut db = mem_store();
    let (id, _, _) = add_move_edge(&mut db, START, "e2e4", "White", "manual").unwrap();
    let edge = db.get_repertoire_move(id).unwrap().unwrap();

    // 2026-07-01 is day 20635 since epoch. First success schedules out.
    grade_edge_with_event(&mut db, &edge, crate::fsrs::Rating::Good, 20635, "drill").unwrap();
    let after = db.get_repertoire_move(id).unwrap().unwrap();
    assert_eq!(after.srs_reps, 1);
    assert!(after.fsrs_stability.is_some());
    assert!(after.srs_due_date.as_str() > "2026-07-01");
    assert_eq!(after.fsrs_last_review.as_deref(), Some("2026-07-01"));

    // Failure resets reps and comes back within a day.
    grade_edge_with_event(&mut db, &after, crate::fsrs::Rating::Again, 20640, "drill").unwrap();
    let failed = db.get_repertoire_move(id).unwrap().unwrap();
    assert_eq!(failed.srs_reps, 0);
    assert!(failed.srs_due_date.as_str() <= "2026-07-07");

    // Events carry the FSRS rating values.
    let events = db
        .get_review_events_for_target("repertoire_move", &id.to_string())
        .unwrap();
    assert_eq!(events.len(), 2);
    assert!(events.iter().any(|e| e.rating == 3));
    assert!(events.iter().any(|e| e.rating == 1 && e.source == "drill"));
}
```

- [x] **Step 2: Run** `cargo test -p app grading_schedules` — expected: compile failure (signature).

- [x] **Step 3: Implement**

In `grade_edge_with_event`: parameter `grade: u32` becomes `rating: crate::fsrs::Rating`; replace the `grade_edge(...)` call with:

```rust
let schedule = crate::fsrs::next_schedule(edge, rating, today_days);
db.update_repertoire_move_fsrs(
    edge.id,
    schedule.stability,
    schedule.difficulty,
    &schedule.last_review,
    schedule.reps,
    &schedule.due_date,
)?;
```

and in the event record set `rating: rating as i32` (drop the `.min(5)`). Delete `pub fn grade_edge`, delete `app/src/srs.rs`, remove `mod srs;` from `main.rs`. `use` items: drop `crate::srs::...` references in tree.rs.

- [x] **Step 4: Fix main.rs call sites** (they don't compile yet — that's Task 4's scope, but the workspace must build to run tests; do the minimal edit now): drill pass site passes `crate::fsrs::drill_rating(true, 0)` placeholder-free equivalent — **no**: do Task 4's real wiring in the same commit if splitting breaks the build. Preferred order: make all three call sites pass `crate::fsrs::Rating` values (drill pass → `Rating::Good` temporarily inline, fail/bot → `Rating::Again`), run the suite, commit Tasks 3+4 hesitation wiring separately only if green in between; otherwise combine.

- [x] **Step 5: Run** `cargo test -p app` — all green. Commit:

```bash
git add app/src/tree.rs app/src/main.rs
git rm app/src/srs.rs
git commit -m "feat(app): FSRS replaces SM-2 grading; delete srs.rs"
```

### Task 4: Drill hesitation timing (main.rs)

**Files:**
- Modify: `app/src/main.rs`

**Interfaces:**
- Consumes: `crate::fsrs::{drill_rating, Rating}`.
- Produces: `AppState.drill_prompt_at: Option<std::time::Instant>`.

- [x] **Step 1: Add state field**

`AppState` gains `drill_prompt_at: Option<std::time::Instant>,` (init `None` in the constructor literal).

- [x] **Step 2: Stamp the prompt time**

In `drill_advance`, in the `side_to_move_is_mine` branch right after `state.drill_expected = mine;`, add `state.drill_prompt_at = Some(std::time::Instant::now());`. Also stamp it at the end of the start-drill callback that first presents a position awaiting the user's move (find `on_start_drill`; it calls `drill_advance`, which covers it — verify by reading the callback; if it presents without `drill_advance`, stamp there too).

- [x] **Step 3: Use it at the grading call sites**

Drill success site (the `if let Some(edge) = matched` branch):

```rust
let elapsed = state
    .drill_prompt_at
    .take()
    .map(|t| t.elapsed().as_secs())
    .unwrap_or(0);
let _ = tree::grade_edge_with_event(
    &mut state.db,
    &edge,
    fsrs::drill_rating(true, elapsed),
    today_days,
    "drill",
);
```

Drill fail loop: `fsrs::Rating::Again`. Bot deviation: `fsrs::Rating::Again`.

- [x] **Step 4: Full verification**

Run: `cargo fmt --all && cargo fmt --all -- --check && cargo clippy --workspace && cargo test --workspace`
Expected: fmt clean, zero clippy warnings, 87 tests pass.

- [x] **Step 5: Commit**

```bash
git add app/src/main.rs
git commit -m "feat(app): hesitation-aware automatic FSRS ratings in drills"
```

### Task 5: Docs

**Files:**
- Modify: `README.md`, `docs/superpowers/specs/2026-07-07-learning-scheduler-and-position-identity-design.md`

- [x] **Step 1: README** — replace scheduler wording: "per-my-move-edge SM-2" → "per-my-move-edge FSRS (rs-fsrs, automatic ratings from drill behavior)"; the `repertoire_moves` schema-table row's "SM-2 state" → "shared due/reps plus FSRS memory state (stability/difficulty/last review)"; test count `# 84 tests` → `# 87 tests`; mention automatic Again/Hard/Good grading in the drill section.
- [x] **Step 2: Scheduler spec** — under "FSRS Migration Rule" add: `**IMPLEMENTED 2026-07-11** — see docs/superpowers/specs/2026-07-11-fsrs-migration-design.md; SM-2 removed, conservative migration at bootstrap, automatic ratings (no self-report UI).` Answer its open questions inline: edges only; bot deviations = Again (FSRS has no rating below Again; deviations additionally drive the Weak status).
- [x] **Step 3: Run the full gate once more, tick this plan's checkboxes, commit**

```bash
git add README.md docs/
git commit -m "docs: FSRS migration recorded in README and scheduler spec"
```

## Self-Review

- Spec coverage: engine/dep choice (T2), automatic ratings incl. boundary (T2/T4), state columns + shared reads (T1), migration invariants + idempotency (T1), Weak threshold (T1), SM-2 deletion (T3), day granularity via LongtermScheduler (T2), out-of-scope untouched.
- Placeholder scan: Task 3 Step 4 explicitly resolves its own ordering concern (combine with Task 4 wiring if the intermediate build is broken) — acceptable as a stated decision rule, not a TBD.
- Type consistency: `next_schedule(edge, Rating, u64) -> EdgeSchedule` and `update_repertoire_move_fsrs(id, f64, f64, &str, u32, &str)` used identically across T1–T4; `Rating` re-exported from `crate::fsrs` everywhere.
- Risk noted: exact rs-fsrs `Parameters`/`Card` field set was verified against the crate source on 2026-07-11; if 1.2.x differs at compile time, adjust the adapter only — no other task depends on rs-fsrs types.
