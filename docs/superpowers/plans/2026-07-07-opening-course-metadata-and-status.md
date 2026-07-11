# Opening Course Metadata And Status Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add real course/chapter/named-line metadata over the existing repertoire graph and derive honest line/course status from required user-owned moves.

**Architecture:** Keep `repertoire_nodes` and `repertoire_moves` as the only chess graph. Add small SQLite metadata tables that point at existing graph nodes/moves. Add query helpers in `db_manager`; do not add a new crate or scheduler.

**Tech Stack:** Rust 2021, rusqlite 0.31, existing `db_manager`, existing `app/src/tree.rs`, current SM-2 fields as temporary due signal.

---

## Scope

This plan implements the first prerequisite for the opening-course spec. It does not redesign the Slint UI and does not route imports into courses yet; that is the next plan.

## Implementation Notes

- Implemented together on 2026-07-10 in `db_manager` with SQLite foreign-key enforcement and cascading metadata deletes.
- The final CRUD surface includes singular reads and deletes in addition to the upsert/list sketches below.
- The final status helper makes every specified state reachable: `Undiscovered`, `Learning`, `Learned`, `Mastered`, and `Review Due`.
- Until FSRS replaces SM-2, two successful repetitions is the `Mastered` stability proxy because that is the existing transition from a one-day to a six-day interval.
- The code samples below remain the original TDD sketches; the checked implementation and tests are authoritative where they differ.

## Files

- Modify: `db_manager/src/lib.rs`
- Optionally modify later: `app/src/tree.rs` only if a tiny helper is useful for attaching imported graph edges to a course line.

## Task 1: Course Metadata Schema And Records

**Files:**
- Modify: `db_manager/src/lib.rs`

- [x] **Step 1: Add failing schema/CRUD tests**

Add to `#[cfg(test)] mod tests` in `db_manager/src/lib.rs`:

```rust
#[test]
fn course_chapter_line_crud_round_trip() {
    let mut store = SqliteStore::new_in_memory().unwrap();
    store.bootstrap().unwrap();

    let course_id = store
        .insert_course(&CourseRecord {
            id: "vienna-gambit".to_string(),
            title: "Vienna Gambit".to_string(),
            description: "Aggressive 1.e4 repertoire.".to_string(),
            side: "White".to_string(),
            source: "built-in".to_string(),
            tags: "vienna,e4,gambit".to_string(),
            thumbnail_fen: "startpos".to_string(),
        })
        .unwrap();
    assert_eq!(course_id, "vienna-gambit");

    let chapter_id = store
        .insert_course_chapter(&CourseChapterRecord {
            id: "vienna-main".to_string(),
            course_id: course_id.clone(),
            title: "Main Line".to_string(),
            description: "Accepted gambit lines.".to_string(),
            sort_order: 10,
        })
        .unwrap();

    let line_id = store
        .insert_course_line(&CourseLineRecord {
            id: "vienna-bishop-bump".to_string(),
            chapter_id: chapter_id.clone(),
            title: "Bishop Bump".to_string(),
            description: "Named path through the graph.".to_string(),
            root_node_id: 1,
            sort_order: 20,
            pgn: Some("1. e4 e5 2. Nc3".to_string()),
        })
        .unwrap();

    let courses = store.get_courses().unwrap();
    assert_eq!(courses.len(), 1);
    assert_eq!(courses[0].title, "Vienna Gambit");

    let chapters = store.get_course_chapters(&course_id).unwrap();
    assert_eq!(chapters.len(), 1);
    assert_eq!(chapters[0].id, chapter_id);

    let lines = store.get_course_lines(&chapter_id).unwrap();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].id, line_id);
}
```

- [x] **Step 2: Run failing test**

Run: `cargo test -p db_manager course_chapter_line_crud_round_trip`

Expected: compile failure because `CourseRecord`, `insert_course`, and related helpers do not exist.

- [x] **Step 3: Add schema**

Append these statements to `SQLITE_SCHEMA` in `db_manager/src/lib.rs`:

```rust
r#"
CREATE TABLE IF NOT EXISTS courses (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    side TEXT NOT NULL CHECK (side IN ('White','Black','Mixed')),
    source TEXT NOT NULL,
    tags TEXT NOT NULL DEFAULT '',
    thumbnail_fen TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT '',
    updated_at TEXT NOT NULL DEFAULT ''
);
"#,
r#"
CREATE TABLE IF NOT EXISTS course_chapters (
    id TEXT PRIMARY KEY,
    course_id TEXT NOT NULL REFERENCES courses(id),
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    sort_order INTEGER NOT NULL DEFAULT 0
);
"#,
r#"
CREATE TABLE IF NOT EXISTS course_lines (
    id TEXT PRIMARY KEY,
    chapter_id TEXT NOT NULL REFERENCES course_chapters(id),
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    root_node_id INTEGER NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    pgn TEXT,
    FOREIGN KEY(root_node_id) REFERENCES repertoire_nodes(id)
);
"#,
r#"
CREATE TABLE IF NOT EXISTS course_line_moves (
    line_id TEXT NOT NULL REFERENCES course_lines(id),
    move_id INTEGER NOT NULL REFERENCES repertoire_moves(id),
    ply_index INTEGER NOT NULL,
    is_required INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (line_id, move_id)
);
"#,
```

- [x] **Step 4: Add records and CRUD**

Add near the other record structs:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CourseRecord {
    pub id: String,
    pub title: String,
    pub description: String,
    pub side: String,
    pub source: String,
    pub tags: String,
    pub thumbnail_fen: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CourseChapterRecord {
    pub id: String,
    pub course_id: String,
    pub title: String,
    pub description: String,
    pub sort_order: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CourseLineRecord {
    pub id: String,
    pub chapter_id: String,
    pub title: String,
    pub description: String,
    pub root_node_id: i64,
    pub sort_order: i32,
    pub pgn: Option<String>,
}
```

Add methods on `impl SqliteStore`:

```rust
pub fn insert_course(&mut self, course: &CourseRecord) -> Result<String, String> {
    self.conn.execute(
        "INSERT INTO courses (id, title, description, side, source, tags, thumbnail_fen)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(id) DO UPDATE SET
            title = excluded.title,
            description = excluded.description,
            side = excluded.side,
            source = excluded.source,
            tags = excluded.tags,
            thumbnail_fen = excluded.thumbnail_fen",
        rusqlite::params![
            course.id,
            course.title,
            course.description,
            course.side,
            course.source,
            course.tags,
            course.thumbnail_fen
        ],
    ).map_err(|e| e.to_string())?;
    Ok(course.id.clone())
}

pub fn get_courses(&self) -> Result<Vec<CourseRecord>, String> {
    let mut stmt = self.conn.prepare(
        "SELECT id, title, description, side, source, tags, thumbnail_fen
         FROM courses ORDER BY title ASC",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| {
        Ok(CourseRecord {
            id: row.get(0)?,
            title: row.get(1)?,
            description: row.get(2)?,
            side: row.get(3)?,
            source: row.get(4)?,
            tags: row.get(5)?,
            thumbnail_fen: row.get(6)?,
        })
    }).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn insert_course_chapter(&mut self, chapter: &CourseChapterRecord) -> Result<String, String> {
    self.conn.execute(
        "INSERT INTO course_chapters (id, course_id, title, description, sort_order)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(id) DO UPDATE SET
            course_id = excluded.course_id,
            title = excluded.title,
            description = excluded.description,
            sort_order = excluded.sort_order",
        rusqlite::params![
            chapter.id,
            chapter.course_id,
            chapter.title,
            chapter.description,
            chapter.sort_order
        ],
    ).map_err(|e| e.to_string())?;
    Ok(chapter.id.clone())
}

pub fn get_course_chapters(&self, course_id: &str) -> Result<Vec<CourseChapterRecord>, String> {
    let mut stmt = self.conn.prepare(
        "SELECT id, course_id, title, description, sort_order
         FROM course_chapters WHERE course_id = ?1 ORDER BY sort_order ASC, title ASC",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([course_id], |row| {
        Ok(CourseChapterRecord {
            id: row.get(0)?,
            course_id: row.get(1)?,
            title: row.get(2)?,
            description: row.get(3)?,
            sort_order: row.get(4)?,
        })
    }).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn insert_course_line(&mut self, line: &CourseLineRecord) -> Result<String, String> {
    self.conn.execute(
        "INSERT INTO course_lines (id, chapter_id, title, description, root_node_id, sort_order, pgn)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(id) DO UPDATE SET
            chapter_id = excluded.chapter_id,
            title = excluded.title,
            description = excluded.description,
            root_node_id = excluded.root_node_id,
            sort_order = excluded.sort_order,
            pgn = excluded.pgn",
        rusqlite::params![
            line.id,
            line.chapter_id,
            line.title,
            line.description,
            line.root_node_id,
            line.sort_order,
            line.pgn
        ],
    ).map_err(|e| e.to_string())?;
    Ok(line.id.clone())
}

pub fn get_course_lines(&self, chapter_id: &str) -> Result<Vec<CourseLineRecord>, String> {
    let mut stmt = self.conn.prepare(
        "SELECT id, chapter_id, title, description, root_node_id, sort_order, pgn
         FROM course_lines WHERE chapter_id = ?1 ORDER BY sort_order ASC, title ASC",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([chapter_id], |row| {
        Ok(CourseLineRecord {
            id: row.get(0)?,
            chapter_id: row.get(1)?,
            title: row.get(2)?,
            description: row.get(3)?,
            root_node_id: row.get(4)?,
            sort_order: row.get(5)?,
            pgn: row.get(6)?,
        })
    }).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}
```

- [x] **Step 5: Run test**

Run: `cargo test -p db_manager course_chapter_line_crud_round_trip`

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add db_manager/src/lib.rs
git commit -m "feat(db): add opening course metadata tables"
```

## Task 2: Attach Graph Moves To Named Lines

**Files:**
- Modify: `db_manager/src/lib.rs`

- [x] **Step 1: Add failing attachment test**

Add to `db_manager/src/lib.rs` tests:

```rust
#[test]
fn course_line_moves_keep_ordered_required_edges() {
    let mut store = SqliteStore::new_in_memory().unwrap();
    store.bootstrap().unwrap();

    let start = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    let after = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1";
    let root = store.ensure_repertoire_node(start, "White").unwrap();
    let child = store.ensure_repertoire_node(after, "White").unwrap();
    let (edge_id, _) = store
        .insert_repertoire_move(root, child, "e2e4", "e4", true, "manual")
        .unwrap();

    store.insert_course(&CourseRecord {
        id: "c".to_string(),
        title: "Course".to_string(),
        description: "".to_string(),
        side: "White".to_string(),
        source: "personal".to_string(),
        tags: "".to_string(),
        thumbnail_fen: start.to_string(),
    }).unwrap();
    store.insert_course_chapter(&CourseChapterRecord {
        id: "ch".to_string(),
        course_id: "c".to_string(),
        title: "Chapter".to_string(),
        description: "".to_string(),
        sort_order: 0,
    }).unwrap();
    store.insert_course_line(&CourseLineRecord {
        id: "line".to_string(),
        chapter_id: "ch".to_string(),
        title: "Line".to_string(),
        description: "".to_string(),
        root_node_id: root,
        sort_order: 0,
        pgn: None,
    }).unwrap();

    store.set_course_line_moves("line", &[(edge_id, 0, true)]).unwrap();
    let moves = store.get_course_line_moves("line").unwrap();
    assert_eq!(moves, vec![(edge_id, 0, true)]);
}
```

- [ ] **Step 2: Run failing test**

Run: `cargo test -p db_manager course_line_moves_keep_ordered_required_edges`

Expected: compile failure because methods do not exist.

- [x] **Step 3: Add helpers**

Add to `impl SqliteStore`:

```rust
pub fn set_course_line_moves(
    &mut self,
    line_id: &str,
    moves: &[(i64, i32, bool)],
) -> Result<(), String> {
    let tx = self.conn.transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM course_line_moves WHERE line_id = ?1", [line_id])
        .map_err(|e| e.to_string())?;
    for (move_id, ply_index, is_required) in moves {
        tx.execute(
            "INSERT INTO course_line_moves (line_id, move_id, ply_index, is_required)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![line_id, move_id, ply_index, *is_required as i64],
        ).map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())
}

pub fn get_course_line_moves(&self, line_id: &str) -> Result<Vec<(i64, i32, bool)>, String> {
    let mut stmt = self.conn.prepare(
        "SELECT move_id, ply_index, is_required
         FROM course_line_moves WHERE line_id = ?1 ORDER BY ply_index ASC",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([line_id], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i32>(1)?,
            row.get::<_, i64>(2)? != 0,
        ))
    }).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}
```

- [x] **Step 4: Run test**

Run: `cargo test -p db_manager course_line_moves_keep_ordered_required_edges`

Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add db_manager/src/lib.rs
git commit -m "feat(db): attach repertoire moves to named course lines"
```

## Task 3: Derived Line Status

**Files:**
- Modify: `db_manager/src/lib.rs`

- [x] **Step 1: Add failing status test**

Add to `db_manager/src/lib.rs` tests:

```rust
#[test]
fn line_status_is_review_due_when_any_required_move_is_due() {
    let mut store = SqliteStore::new_in_memory().unwrap();
    store.bootstrap().unwrap();

    let start = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    let after = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1";
    let root = store.ensure_repertoire_node(start, "White").unwrap();
    let child = store.ensure_repertoire_node(after, "White").unwrap();
    let (edge_id, _) = store
        .insert_repertoire_move(root, child, "e2e4", "e4", true, "manual")
        .unwrap();

    store.insert_course(&CourseRecord {
        id: "c".to_string(),
        title: "Course".to_string(),
        description: "".to_string(),
        side: "White".to_string(),
        source: "personal".to_string(),
        tags: "".to_string(),
        thumbnail_fen: start.to_string(),
    }).unwrap();
    store.insert_course_chapter(&CourseChapterRecord {
        id: "ch".to_string(),
        course_id: "c".to_string(),
        title: "Chapter".to_string(),
        description: "".to_string(),
        sort_order: 0,
    }).unwrap();
    store.insert_course_line(&CourseLineRecord {
        id: "line".to_string(),
        chapter_id: "ch".to_string(),
        title: "Line".to_string(),
        description: "".to_string(),
        root_node_id: root,
        sort_order: 0,
        pgn: None,
    }).unwrap();
    store.set_course_line_moves("line", &[(edge_id, 0, true)]).unwrap();

    assert_eq!(store.course_line_status("line", "2026-07-07").unwrap(), "Review Due");

    store
        .update_repertoire_move_srs(edge_id, 10.0, 2.5, 3, "2026-08-01")
        .unwrap();
    assert_eq!(store.course_line_status("line", "2026-07-07").unwrap(), "Mastered");
}
```

- [ ] **Step 2: Run failing test**

Run: `cargo test -p db_manager line_status_is_review_due_when_any_required_move_is_due`

Expected: compile failure because `course_line_status` does not exist.

- [x] **Step 3: Add the smallest status helper**

Add to `impl SqliteStore`:

```rust
pub fn course_line_status(&self, line_id: &str, today: &str) -> Result<String, String> {
    let mut stmt = self.conn.prepare(
        "SELECT m.is_my_move,
                COALESCE(m.srs_reps, 0),
                COALESCE(m.srs_due_date, ''),
                clm.is_required
         FROM course_line_moves clm
         JOIN repertoire_moves m ON m.id = clm.move_id
         WHERE clm.line_id = ?1
         ORDER BY clm.ply_index ASC",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([line_id], |row| {
        Ok((
            row.get::<_, i64>(0)? != 0,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)? != 0,
        ))
    }).map_err(|e| e.to_string())?;

    let mut required_user_moves = 0;
    let mut passed_required_user_moves = 0;
    let mut due = false;

    for row in rows {
        let (is_my_move, reps, due_date, required) = row.map_err(|e| e.to_string())?;
        if !required || !is_my_move {
            continue;
        }
        required_user_moves += 1;
        if reps > 0 {
            passed_required_user_moves += 1;
        }
        if due_date.is_empty() || due_date.as_str() <= today {
            due = true;
        }
    }

    let status = if required_user_moves == 0 {
        "Undiscovered"
    } else if due && passed_required_user_moves > 0 {
        "Review Due"
    } else if passed_required_user_moves == 0 {
        "Learning"
    } else if passed_required_user_moves < required_user_moves {
        "Learning"
    } else {
        "Mastered"
    };
    Ok(status.to_string())
}
```

Ponytail note: `Weak` is intentionally not in this first helper because the code does not yet have normalized review events/failure counts. Add it after the review-event spine exists.

- [x] **Step 4: Run test**

Run: `cargo test -p db_manager line_status_is_review_due_when_any_required_move_is_due`

Expected: pass.

- [x] **Step 5: Run db tests**

Run: `cargo test -p db_manager`

Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add db_manager/src/lib.rs
git commit -m "feat(db): derive named line mastery status from move schedule"
```

## Self-Review

- Spec coverage: course/chapter/line metadata, graph-path linkage, and strict due-move status are covered.
- Deferred by design: UI, imports, FSRS, `Weak` status, seed course packs.
- Placeholder scan: no placeholders.
- Type consistency: `CourseRecord`, `CourseChapterRecord`, `CourseLineRecord`, `set_course_line_moves`, and `course_line_status` are defined before use.

## Completion (2026-07-10)

- Implemented the four metadata tables, course/chapter/line CRUD, and ordered contiguous paths referencing existing `repertoire_moves`.
- Derived status uses required user-owned moves: one successful repetition is `Learned`, two or more is the temporary `Mastered` proxy, and a due move takes precedence once the line is learned.
- Deferred: `Weak`, course-level progress aggregation, UI/import routing, FSRS, and Zobrist.
- Verified: all three focused tests, `cargo test -p db_manager`, `cargo test --workspace`, formatting, and `git diff --check` pass.
