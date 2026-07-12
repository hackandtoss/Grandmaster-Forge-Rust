# Review Event Spine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a normalized review-event spine that openings, puzzles, tactics, endgames, bot deviations, and future FSRS can all consume.

**Architecture:** Keep existing `training_events` for legacy dashboard/history. Add a richer `review_events` table with target type/id, rating, source, and optional context fields. Write through to both where current UI expects `training_events`.

**Tech Stack:** Rust 2021, rusqlite 0.31, existing `db_manager`, existing `app/src/tree.rs` grading path.

---

## Dependencies

This can start after course metadata exists, but before FSRS or tactical motif work. Do not implement FSRS in this plan.

## Implementation Notes

- Implemented 2026-07-11 following this plan task-by-task.
- One divergence from the Task 2 sketch: review-event ids append a process-local atomic sequence to the nanosecond suffix, so grading the same edge twice inside one clock tick cannot collide on the `review_events` primary key.
- Tests are stronger than the sketches: the CRUD test round-trips the optional course/line/move context fields, and the grading test asserts the SM-2 schedule matches plain `grade_edge` plus a second event from a `bot_deviation` source.
- The code samples below remain the original TDD sketches; the merged implementation and tests are authoritative where they differ.

## Task 1: Review Event Schema And CRUD

**Files:**
- Modify: `db_manager/src/lib.rs`

- [x] **Step 1: Add failing CRUD test**

Add to `db_manager/src/lib.rs` tests:

```rust
#[test]
fn review_events_record_target_rating_and_source() {
    let mut store = SqliteStore::new_in_memory().unwrap();
    store.bootstrap().unwrap();

    store.insert_review_event(&ReviewEventRecord {
        id: "evt1".to_string(),
        user_id: "default_user".to_string(),
        target_type: "repertoire_move".to_string(),
        target_id: "42".to_string(),
        rating: 1,
        source: "drill".to_string(),
        course_id: Some("vienna".to_string()),
        line_id: Some("vienna-main".to_string()),
        move_id: Some(42),
        created_at: "2026-07-07".to_string(),
    }).unwrap();

    let events = store.get_review_events_for_target("repertoire_move", "42").unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].rating, 1);
    assert_eq!(events[0].source, "drill");
}
```

- [x] **Step 2: Run failing test**

Run: `cargo test -p db_manager review_events_record_target_rating_and_source`

Expected: compile failure because `ReviewEventRecord` and helpers do not exist.

- [x] **Step 3: Add schema**

Append to `SQLITE_SCHEMA`:

```rust
r#"
CREATE TABLE IF NOT EXISTS review_events (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    target_type TEXT NOT NULL,
    target_id TEXT NOT NULL,
    rating INTEGER NOT NULL,
    source TEXT NOT NULL,
    course_id TEXT,
    line_id TEXT,
    move_id INTEGER,
    created_at TEXT NOT NULL
);
"#,
```

- [x] **Step 4: Add record and helpers**

Add near other records:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewEventRecord {
    pub id: String,
    pub user_id: String,
    pub target_type: String,
    pub target_id: String,
    pub rating: i32,
    pub source: String,
    pub course_id: Option<String>,
    pub line_id: Option<String>,
    pub move_id: Option<i64>,
    pub created_at: String,
}
```

Add to `impl SqliteStore`:

```rust
pub fn insert_review_event(&mut self, event: &ReviewEventRecord) -> Result<(), String> {
    self.conn.execute(
        "INSERT INTO review_events
         (id, user_id, target_type, target_id, rating, source, course_id, line_id, move_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![
            event.id,
            event.user_id,
            event.target_type,
            event.target_id,
            event.rating,
            event.source,
            event.course_id,
            event.line_id,
            event.move_id,
            event.created_at
        ],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_review_events_for_target(
    &self,
    target_type: &str,
    target_id: &str,
) -> Result<Vec<ReviewEventRecord>, String> {
    let mut stmt = self.conn.prepare(
        "SELECT id, user_id, target_type, target_id, rating, source,
                course_id, line_id, move_id, created_at
         FROM review_events
         WHERE target_type = ?1 AND target_id = ?2
         ORDER BY created_at DESC",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(rusqlite::params![target_type, target_id], |row| {
        Ok(ReviewEventRecord {
            id: row.get(0)?,
            user_id: row.get(1)?,
            target_type: row.get(2)?,
            target_id: row.get(3)?,
            rating: row.get(4)?,
            source: row.get(5)?,
            course_id: row.get(6)?,
            line_id: row.get(7)?,
            move_id: row.get(8)?,
            created_at: row.get(9)?,
        })
    }).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}
```

- [x] **Step 5: Run test**

Run: `cargo test -p db_manager review_events_record_target_rating_and_source`

Expected: pass.

- [x] **Step 6: Commit**

```bash
git add db_manager/src/lib.rs
git commit -m "feat(db): add normalized review events"
```

## Task 2: Write Review Events When Grading Repertoire Moves

**Files:**
- Modify: `app/src/tree.rs`
- Test: `app/src/tree.rs`

- [x] **Step 1: Add failing grading-event test**

Add to `app/src/tree.rs` tests:

```rust
#[test]
fn grade_edge_records_review_event() {
    let mut db = mem_store();
    let (id, _, _) = add_move_edge(&mut db, START, "e2e4", "White", "manual").unwrap();
    let edge = db.get_repertoire_move(id).unwrap().unwrap();

    grade_edge_with_event(&mut db, &edge, 1, 20635, "drill").unwrap();

    let events = db
        .get_review_events_for_target("repertoire_move", &id.to_string())
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].rating, 1);
    assert_eq!(events[0].source, "drill");
}
```

- [x] **Step 2: Run failing test**

Run: `cargo test -p app grade_edge_records_review_event`

Expected: compile failure because `grade_edge_with_event` does not exist.

- [x] **Step 3: Add wrapper without breaking existing callers**

Add to `app/src/tree.rs`:

```rust
pub fn grade_edge_with_event(
    db: &mut SqliteStore,
    edge: &db_manager::RepertoireMoveRecord,
    grade: u32,
    today_days: u64,
    source: &str,
) -> Result<(), String> {
    grade_edge(db, edge, grade, today_days)?;
    let (y, m, d) = days_to_ymd(today_days);
    let created_at = format!("{:04}-{:02}-{:02}", y, m, d);
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    db.insert_review_event(&db_manager::ReviewEventRecord {
        id: format!("repertoire_move-{}-{}", edge.id, suffix),
        user_id: "default_user".to_string(),
        target_type: "repertoire_move".to_string(),
        target_id: edge.id.to_string(),
        rating: grade.min(5) as i32,
        source: source.to_string(),
        course_id: None,
        line_id: None,
        move_id: Some(edge.id),
        created_at,
    })
}
```

- [x] **Step 4: Run test**

Run: `cargo test -p app grade_edge_records_review_event`

Expected: pass.

- [x] **Step 5: Replace two grading call sites**

In `app/src/main.rs`, replace drill and bot-deviation calls to:

```rust
tree::grade_edge_with_event(&mut state.db, &edge, 5, today_days, "drill")
tree::grade_edge_with_event(&mut state.db, &edge, 1, today_days, "drill")
tree::grade_edge_with_event(&mut st.db, e, 1, today_days, "bot_deviation")
```

Keep existing `training_events` inserts for now so the current dashboard/history does not lose data.

- [x] **Step 6: Run app tests**

Run: `cargo test -p app`

Expected: pass.

- [x] **Step 7: Commit**

```bash
git add db_manager/src/lib.rs app/src/tree.rs app/src/main.rs
git commit -m "feat(training): record normalized review events for repertoire grading"
```

## Self-Review

- Spec coverage: normalized review events and repertoire grading write path are covered.
- Deferred by design: FSRS, tactical motif fields, puzzle/endgame event migration.
- Placeholder scan: no placeholders.
- Type consistency: `ReviewEventRecord` is defined in `db_manager` and used by `app/src/tree.rs`.
