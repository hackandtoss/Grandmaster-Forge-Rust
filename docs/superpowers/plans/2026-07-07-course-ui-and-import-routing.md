# Course UI And Import Routing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace source-grouped fake courses with real course cards and route new repertoire imports into a real course/chapter/line container.

**Architecture:** Build on `2026-07-07-opening-course-metadata-and-status.md`. Keep the current Slint monolith and existing graph import helpers. Add an "Uncategorized" fallback course so old flows can keep working without blocking on full course-picking UI.

**Tech Stack:** Rust 2021, Slint 1.6, rusqlite 0.31, existing `app/src/main.rs`, `app/src/tree.rs`, `db_manager`.

---

## Dependencies

Complete `docs/superpowers/plans/2026-07-07-opening-course-metadata-and-status.md` first.

## Task 1: Add Uncategorized Course Helper

**Files:**
- Modify: `db_manager/src/lib.rs`

- [ ] **Step 1: Add failing helper test**

Add to `db_manager/src/lib.rs` tests:

```rust
#[test]
fn ensure_uncategorized_course_creates_course_and_chapter() {
    let mut store = SqliteStore::new_in_memory().unwrap();
    store.bootstrap().unwrap();

    let chapter = store.ensure_uncategorized_chapter("White").unwrap();
    assert_eq!(chapter, "uncategorized-white-main");

    let courses = store.get_courses().unwrap();
    assert_eq!(courses[0].id, "uncategorized-white");

    let chapters = store.get_course_chapters("uncategorized-white").unwrap();
    assert_eq!(chapters[0].id, chapter);

    assert_eq!(store.ensure_uncategorized_chapter("White").unwrap(), chapter);
}
```

- [ ] **Step 2: Run failing test**

Run: `cargo test -p db_manager ensure_uncategorized_course_creates_course_and_chapter`

Expected: compile failure because `ensure_uncategorized_chapter` does not exist.

- [ ] **Step 3: Implement helper**

Add to `impl SqliteStore`:

```rust
pub fn ensure_uncategorized_chapter(&mut self, side: &str) -> Result<String, String> {
    let side_lc = side.to_lowercase();
    let course_id = format!("uncategorized-{side_lc}");
    let chapter_id = format!("uncategorized-{side_lc}-main");
    self.insert_course(&CourseRecord {
        id: course_id.clone(),
        title: format!("Uncategorized {side} Repertoire"),
        description: "Imported or manually added lines waiting to be organized.".to_string(),
        side: side.to_string(),
        source: "personal".to_string(),
        tags: "uncategorized".to_string(),
        thumbnail_fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string(),
    })?;
    self.insert_course_chapter(&CourseChapterRecord {
        id: chapter_id.clone(),
        course_id,
        title: "Main".to_string(),
        description: "Default holding chapter.".to_string(),
        sort_order: 0,
    })?;
    Ok(chapter_id)
}
```

- [ ] **Step 4: Run test**

Run: `cargo test -p db_manager ensure_uncategorized_course_creates_course_and_chapter`

Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add db_manager/src/lib.rs
git commit -m "feat(db): add uncategorized course fallback"
```

## Task 2: Show Real Courses In Repertoire Browser

**Files:**
- Modify: `app/src/main.rs`

- [ ] **Step 1: Add a pure helper test for course progress labels**

Add near existing tests in `app/src/main.rs`:

```rust
#[test]
fn course_progress_text_reports_mastered_and_due_lines() {
    assert_eq!(course_progress_text(3, 2, 1), "2/3 mastered - 1 due");
    assert_eq!(course_progress_text(3, 3, 0), "3/3 mastered");
}
```

- [ ] **Step 2: Run failing test**

Run: `cargo test -p app course_progress_text_reports_mastered_and_due_lines`

Expected: compile failure because `course_progress_text` does not exist.

- [ ] **Step 3: Add helper**

Add near `repertoire_course_name` in `app/src/main.rs`:

```rust
fn course_progress_text(total_lines: i32, mastered_lines: i32, due_lines: i32) -> String {
    if due_lines > 0 {
        format!("{mastered_lines}/{total_lines} mastered - {due_lines} due")
    } else {
        format!("{mastered_lines}/{total_lines} mastered")
    }
}
```

- [ ] **Step 4: Replace source-grouped refresh query**

In `refresh_data`, replace the current "Refresh repertoire moves as course-style groups" block with a query over real courses:

```rust
let today = tree::local_now_str();
let courses = state.db.get_courses().unwrap_or_default();
let mut line_entries = Vec::new();
for course in courses {
    let chapters = state.db.get_course_chapters(&course.id).unwrap_or_default();
    let mut total_lines = 0;
    let mut mastered_lines = 0;
    let mut due_lines = 0;
    let mut first_line_id = String::new();
    for chapter in chapters {
        for line in state.db.get_course_lines(&chapter.id).unwrap_or_default() {
            if first_line_id.is_empty() {
                first_line_id = line.id.clone();
            }
            total_lines += 1;
            match state.db.course_line_status(&line.id, &today).unwrap_or_else(|_| "Learning".to_string()).as_str() {
                "Mastered" => mastered_lines += 1,
                "Review Due" => due_lines += 1,
                _ => {}
            }
        }
    }
    if total_lines == 0 {
        continue;
    }
    let confidence = if total_lines == 0 {
        0.0
    } else {
        mastered_lines as f32 / total_lines as f32
    };
    line_entries.push(OpeningLineEntry {
        id: slint::SharedString::from(first_line_id),
        name: slint::SharedString::from(course.title),
        moves: slint::SharedString::from(course_progress_text(total_lines, mastered_lines, due_lines)),
        confidence,
        start_fen: slint::SharedString::from(course.thumbnail_fen),
        line_count: total_lines,
        due_count: due_lines,
        category: slint::SharedString::from(course.source),
    });
}
app.set_opening_lines(slint::ModelRc::from(Rc::new(slint::VecModel::from(line_entries))));
```

This intentionally keeps the existing `OpeningLineEntry` Slint struct for the first UI cut.

- [ ] **Step 5: Run tests**

Run: `cargo test -p app course_progress_text_reports_mastered_and_due_lines`

Expected: pass.

- [ ] **Step 6: Build**

Run: `cargo check -p app`

Expected: pass.

- [ ] **Step 7: Commit**

```bash
git add app/src/main.rs
git commit -m "feat(app): show real opening courses in repertoire browser"
```

## Task 3: Attach Manual Builder Saves To Uncategorized Course

**Files:**
- Modify: `app/src/tree.rs`
- Modify: `app/src/main.rs`
- Test: `app/src/tree.rs`

- [ ] **Step 1: Add failing tree helper test**

Add to `app/src/tree.rs` tests:

```rust
#[test]
fn import_uci_line_with_course_creates_named_line_path() {
    let mut db = mem_store();
    let chapter_id = db.ensure_uncategorized_chapter("White").unwrap();
    let moves: Vec<String> = ["e2e4", "e7e5"].iter().map(|s| s.to_string()).collect();

    let line_id = import_uci_line_as_course_line(
        &mut db,
        START,
        &moves,
        "White",
        "manual",
        &chapter_id,
        "Test Line",
    ).unwrap();

    let line_moves = db.get_course_line_moves(&line_id).unwrap();
    assert_eq!(line_moves.len(), 2);
}
```

- [ ] **Step 2: Run failing test**

Run: `cargo test -p app import_uci_line_with_course_creates_named_line_path`

Expected: compile failure because helper does not exist.

- [ ] **Step 3: Add helper**

Add to `app/src/tree.rs`:

```rust
pub fn import_uci_line_as_course_line(
    db: &mut SqliteStore,
    start_fen: &str,
    moves: &[String],
    side: &str,
    source: &str,
    chapter_id: &str,
    line_title: &str,
) -> Result<String, String> {
    let mut fen = start_fen.to_string();
    let mut move_ids = Vec::new();
    let mut root_node_id = None;
    for (idx, uci) in moves.iter().enumerate() {
        let (edge_id, child, _) = add_move_edge(db, &fen, uci, side, source)?;
        let edge = db.get_repertoire_move(edge_id)?.ok_or_else(|| "inserted edge missing".to_string())?;
        if idx == 0 {
            root_node_id = Some(edge.parent_id);
        }
        move_ids.push((edge_id, idx as i32, true));
        fen = child;
    }
    let root_node_id = root_node_id.ok_or_else(|| "cannot create a course line with no moves".to_string())?;
    let line_id = format!("{}-{}", chapter_id, moves.join("-"));
    db.insert_course_line(&db_manager::CourseLineRecord {
        id: line_id.clone(),
        chapter_id: chapter_id.to_string(),
        title: line_title.to_string(),
        description: String::new(),
        root_node_id,
        sort_order: 0,
        pgn: None,
    })?;
    db.set_course_line_moves(&line_id, &move_ids)?;
    Ok(line_id)
}
```

- [ ] **Step 4: Run test**

Run: `cargo test -p app import_uci_line_with_course_creates_named_line_path`

Expected: pass.

- [ ] **Step 5: Wire builder save**

In `app/src/main.rs`, find `on_builder_save_line`. Replace the final `tree::import_uci_line(...)` call with:

```rust
let chapter_id = st.db.ensure_uncategorized_chapter(&side)?;
tree::import_uci_line_as_course_line(
    &mut st.db,
    start_fen,
    &staged,
    &side,
    "manual",
    &chapter_id,
    "Manual Line",
)
```

If the closure currently returns no `Result`, use a local `match`:

```rust
let result = st
    .db
    .ensure_uncategorized_chapter(&side)
    .and_then(|chapter_id| {
        tree::import_uci_line_as_course_line(
            &mut st.db,
            start_fen,
            &staged,
            &side,
            "manual",
            &chapter_id,
            "Manual Line",
        )
    });
match result {
    Ok(_) => app.set_import_status(slint::SharedString::from(format!("Saved line to Uncategorized {side} Repertoire."))),
    Err(e) => app.set_import_status(slint::SharedString::from(format!("Save failed: {e}"))),
}
```

- [ ] **Step 6: Build**

Run: `cargo check -p app`

Expected: pass.

- [ ] **Step 7: Commit**

```bash
git add app/src/tree.rs app/src/main.rs
git commit -m "feat(app): route manual opening lines into course metadata"
```

## Self-Review

- Spec coverage: real course display and first import routing path are covered.
- Deferred by design: full course picker, PGN/study/explorer target UI, seed course packs, visual catalog card redesign.
- Placeholder scan: no placeholders.
- Type consistency: depends on `CourseRecord`, `CourseChapterRecord`, `CourseLineRecord`, `ensure_uncategorized_chapter`, `course_line_status`, `set_course_line_moves`.
