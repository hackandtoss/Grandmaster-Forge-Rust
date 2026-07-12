# Puzzle Review Events And Weak Lines Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Route puzzle pass/fail results through normalized `review_events` (ending the `score_delta`-as-absolute-rating overload as the only record), and derive the `Weak` course-line status from review-event history.

**Architecture:** Build on the review-event spine (`2026-07-07-review-event-spine.md`, PR #11). Puzzle grading writes through to both `training_events` (legacy dashboard feed — `get_puzzle_rating` still reads it) and `review_events` (normalized). `Weak` is derived inside `course_line_status` from each required move's recent event window; no new tables, no schema change, no FSRS.

**Weak rule (deterministic):** a required user move is *weak* when, among its 3 most recent `review_events` (ordered `created_at DESC, id DESC`), at least 2 have `rating <= 2`, **or** any of the 3 has `source = 'bot_deviation'`. A line is `Weak` when it is not `Undiscovered` and any required move is weak. `Weak` outranks `Learning`/`Review Due`/`Mastered`/`Learned` because repeated failure is the more actionable signal; failures age out of the window after subsequent successes. ("Recent game mistake" from the spec is deferred until game review writes review events.)

**Tech Stack:** Rust 2021, rusqlite 0.31, existing `db_manager`, `app/src/main.rs` puzzle path.

---

## Dependencies

Requires `review_events` (PR #11). Do not implement FSRS, motif labels, or game-mistake events in this plan.

## Task 1: Derive Weak In course_line_status

**Files:**
- Modify: `db_manager/src/lib.rs`

- [x] **Step 1: Add failing status test**

Extend the existing course-status tests with a `Weak` scenario: a line whose single required move has events `[fail, fail, pass]` (most recent first) is `Weak`; after two more passes (`[pass, pass, fail]` window) it is not; a required move whose last-3 window contains a `bot_deviation` event is `Weak`; a line with no events keeps its current status; an `Undiscovered` line is never `Weak`.

- [x] **Step 2: Run failing test** — `cargo test -p db_manager weak`

- [x] **Step 3: Implement**

Add a private `move_is_weak(&self, move_id: i64)` that selects `rating, source` from `review_events WHERE target_type='repertoire_move' AND target_id=?1 ORDER BY created_at DESC, id DESC LIMIT 3` and applies the rule. In `course_line_status`, also select `m.id` for required moves; when the pre-Weak status is not `Undiscovered`, return `Weak` if any required move is weak (checked before the `Learning` branch).

- [x] **Step 4: Add weak_lines to CourseProgress**

`CourseProgress` gains `weak_lines: u32`; `course_progress` handles the `Weak` arm (counts as discovered + weak). Extend the progress test.

- [x] **Step 5: Run tests, commit**

## Task 2: Puzzle Results As Review Events

**Files:**
- Modify: `app/src/main.rs`

- [x] **Step 1: Add failing pure-helper test**

`puzzle_review_event(puzzle_id: &str, solved: bool, created_at: &str) -> db_manager::ReviewEventRecord` — asserts: `target_type == "puzzle"`, `target_id` is the puzzle id, rating 5 on pass / 1 on fail, `source == "puzzle_trainer"`, no course/line/move context, unique ids across two calls.

- [x] **Step 2: Run failing test** — `cargo test -p app puzzle_review_event`

- [x] **Step 3: Implement helper and wire**

Helper builds the record (id: `puzzle-<id>-<uuid_now()>`). In `record_puzzle_event`, insert it alongside the existing `training_events` write (which `get_puzzle_rating` still needs).

- [x] **Step 4: Run tests, commit**

## Task 3: Surface Weak Counts In Course Progress Text

**Files:**
- Modify: `app/src/main.rs`

- [x] **Step 1: Extend course_progress_text test**

`course_progress_text(total, mastered, due, weak)`: `(3, 2, 1, 0)` → `"2/3 mastered - 1 due"`; `(3, 2, 1, 1)` → `"2/3 mastered - 1 due - 1 weak"`; `(3, 3, 0, 0)` → `"3/3 mastered"`.

- [x] **Step 2: Update helper and both call sites** (course cards refresh + selected-course header) to pass `progress.weak_lines`. Line detail rows already render the status string, so `Weak` appears there with no change.

- [x] **Step 3: Run app tests, build, commit**

## Self-Review

- Spec coverage: `Weak` from repeated failures and bot deviations (course-graph design), puzzle events normalized (puzzle-trainer notes gap 2).
- Deferred by design: game-mistake-driven Weak, FSRS, Weak-lines training mode, dashboard migration off `training_events`.
- Type consistency: uses `ReviewEventRecord` and `get_review_events_for_target` from PR #11.
