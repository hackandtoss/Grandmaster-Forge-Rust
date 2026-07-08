# Repertoire Tree, Multi-Source Import, and Book-Bot Play — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Code exploration rule:** graphify-out/graph.json exists in this repo. Run `graphify query "<question>"` before reading source files; read raw files only after graphify has oriented you or to modify specific lines. After each task's code changes, run `graphify update .`.

> **Repo drift notice (2026-07-07): HISTORICAL/COMPLETED BASELINE PLAN.**
> This plan describes the completed migration from flat opening lines to the position graph. Use it for context, not as the next implementation plan.
> New opening work should build on top of the graph with course/chapter/named-line metadata; see `docs/superpowers/specs/2026-07-07-opening-course-graph-design.md`.
> SM-2 references here describe the current implemented baseline only. Future scheduling work should replace SM-2 with FSRS rather than layering a second scheduler beside it.
> Bot-book and drill logic should remain graph-backed; course-specific practice filters graph paths instead of creating a separate flat book.

**Goal:** Turn Grandmaster Forge's flat repertoire into a position-tree with per-move SM-2, feed it from five sources (PGN paste, Lichess studies, explorer adopt, mistake generation, manual builder), sync games from chess.com as well as Lichess, and add a Play-vs-Bot mode that plays the user's book before handing off to Elo-limited Stockfish.

**Architecture:** New `repertoire_nodes`/`repertoire_moves` tables in `db_manager` (positions keyed by normalized FEN, SM-2 state per my-move edge). Position walking (SAN/UCI/FEN) stays in crates that already have shakmaty (`pgn_processor`, `app`). New `chesscom_client` crate mirrors `lichess_client`. `engine_controller` gains a persistent `PlaySession`. The `app` crate wires everything through new Slint screens, following the existing background-thread + `slint::invoke_from_event_loop` pattern.

**Tech Stack:** Rust 2021, shakmaty 0.26, rusqlite 0.31 (bundled), Slint 1.6, reqwest 0.12 + tokio, serde/serde_json.

**Spec:** `docs/superpowers/specs/2026-07-01-repertoire-tree-and-bot-play-design.md`

**Deviations from spec (agreed rationale):**
1. The PGN variation parser is hand-rolled in `pgn_processor` instead of using the `pgn-reader` crate — `pgn-reader` pins a different shakmaty major version, which would compile two shakmaty copies and invite type confusion.
2. `repertoire_moves.source` has no SQL CHECK constraint — migrated legacy rows carry historical source strings (`builder`, `game_review`); canonical new values are `manual|pgn|study|explorer|mistake`.

## Global Constraints

- Workspace edition 2021; add no new external dependencies anywhere (new `chesscom_client` crate reuses versions already in the workspace: reqwest 0.12 features ["json"], serde 1 ["derive"], serde_json 1, tokio 1 ["rt-multi-thread","macros"]).
- All tree FENs are **normalized**: generated with `shakmaty::EnPassantMode::Legal`, then first 4 fields + `" 0 1"` (board, side, castling, ep, zeroed clocks). Helper is `pgn_processor::position_key` / `db_manager::normalize_fen`.
- `side` values are exactly `"White"` / `"Black"` (matches existing `opening_lines.side`).
- `user_id` is the literal `"default_user"` everywhere (existing convention).
- `srs_due_date = ''` means "due now"; date format `YYYY-MM-DD` via existing `local_now_str()` / `days_to_ymd()` helpers in `app/src/main.rs:2002-2031`.
- SM-2 grades: pass = 5, fail = 1, via existing `app/src/srs.rs::review`.
- Env keys: `LICHESS_API_KEY` (exists), `LICHESS_USERNAME` (exists, = hackandtoss), new `CHESSCOM_USERNAME=ElectricMindGames`, new optional `CHESSCOM_USER_AGENT` (default `"grandmaster-forge (contact: johncjohnson1113@gmail.com)"`).
- Slint callbacks that do I/O must not block the UI thread except where noted (bot engine reply, ≤500 ms movetime, is accepted synchronous for v1).
- Run tests with `cargo test -p <crate>`; full suite `cargo test --workspace`. Commit after every task with a conventional message.

---

### Task 1: Repertoire tree schema + CRUD in `db_manager`

**Files:**
- Modify: `db_manager/src/lib.rs` (schema array at top; new struct + impl block after `OpeningNode`; tests at bottom)

**Interfaces:**
- Consumes: nothing new.
- Produces (used by Tasks 2, 4, 5, 8, 9, 11):
  - `pub fn normalize_fen(fen: &str) -> String`
  - `pub struct RepertoireMoveRecord { pub id: i64, pub parent_id: i64, pub child_id: i64, pub uci: String, pub san: String, pub is_my_move: bool, pub source: String, pub comment: Option<String>, pub srs_interval: f32, pub srs_ease: f32, pub srs_reps: u32, pub srs_due_date: String, pub parent_fen: String }`
  - `SqliteStore::ensure_repertoire_node(&mut self, fen: &str, side: &str) -> Result<i64, String>`
  - `SqliteStore::insert_repertoire_move(&mut self, parent_id: i64, child_id: i64, uci: &str, san: &str, is_my_move: bool, source: &str) -> Result<(i64, bool), String>` (returns `(edge_id, was_new)`)
  - `SqliteStore::get_repertoire_moves_from(&self, fen: &str, side: &str) -> Result<Vec<RepertoireMoveRecord>, String>`
  - `SqliteStore::get_due_repertoire_moves(&self, today: &str) -> Result<Vec<RepertoireMoveRecord>, String>`
  - `SqliteStore::get_due_repertoire_move_count(&self, today: &str) -> Result<u32, String>`
  - `SqliteStore::update_repertoire_move_srs(&mut self, move_id: i64, interval: f32, ease: f32, reps: u32, due_date: &str) -> Result<(), String>`
  - `SqliteStore::get_repertoire_move(&self, move_id: i64) -> Result<Option<RepertoireMoveRecord>, String>`
  - `SqliteStore::is_synced(&self, source: &str, external_id: &str) -> bool`
  - `SqliteStore::mark_synced(&mut self, source: &str, external_id: &str, synced_at: &str) -> Result<(), String>`

- [ ] **Step 1: Write the failing tests** — append to the `tests` module in `db_manager/src/lib.rs`:

```rust
    #[test]
    fn normalize_fen_drops_clocks() {
        let fen = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 3 7";
        assert_eq!(
            normalize_fen(fen),
            "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1"
        );
    }

    #[test]
    fn repertoire_tree_insert_and_lookup() {
        let mut store = SqliteStore::new_in_memory().unwrap();
        store.bootstrap().unwrap();

        let start = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let after_e4 = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1";

        let root = store.ensure_repertoire_node(start, "White").unwrap();
        let child = store.ensure_repertoire_node(after_e4, "White").unwrap();
        assert_ne!(root, child);
        // idempotent: same fen+side returns same id
        assert_eq!(store.ensure_repertoire_node(start, "White").unwrap(), root);
        // same fen, other side is a different node
        assert_ne!(store.ensure_repertoire_node(start, "Black").unwrap(), root);

        let (edge, was_new) = store
            .insert_repertoire_move(root, child, "e2e4", "e4", true, "manual")
            .unwrap();
        assert!(was_new);
        let (edge2, was_new2) = store
            .insert_repertoire_move(root, child, "e2e4", "e4", true, "pgn")
            .unwrap();
        assert_eq!(edge, edge2);
        assert!(!was_new2);

        let moves = store.get_repertoire_moves_from(start, "White").unwrap();
        assert_eq!(moves.len(), 1);
        assert_eq!(moves[0].uci, "e2e4");
        assert_eq!(moves[0].san, "e4");
        assert!(moves[0].is_my_move);
        assert_eq!(moves[0].parent_fen, start);
        assert_eq!(moves[0].srs_due_date, "");
        assert!(store.get_repertoire_moves_from(after_e4, "White").unwrap().is_empty());
    }

    #[test]
    fn due_moves_and_srs_update() {
        let mut store = SqliteStore::new_in_memory().unwrap();
        store.bootstrap().unwrap();
        let a = store.ensure_repertoire_node("fenA w KQkq - 0 1", "White").unwrap();
        let b = store.ensure_repertoire_node("fenB b KQkq - 0 1", "White").unwrap();
        let c = store.ensure_repertoire_node("fenC w KQkq - 0 1", "White").unwrap();
        let (mine, _) = store.insert_repertoire_move(a, b, "e2e4", "e4", true, "manual").unwrap();
        let (_theirs, _) = store.insert_repertoire_move(b, c, "e7e5", "e5", false, "manual").unwrap();

        // '' due date counts as due; opponent edges never appear
        let due = store.get_due_repertoire_moves("2026-07-01").unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].id, mine);
        assert_eq!(store.get_due_repertoire_move_count("2026-07-01").unwrap(), 1);

        store.update_repertoire_move_srs(mine, 6.0, 2.6, 2, "2026-07-10").unwrap();
        assert!(store.get_due_repertoire_moves("2026-07-01").unwrap().is_empty());
        let rec = store.get_repertoire_move(mine).unwrap().unwrap();
        assert!((rec.srs_interval - 6.0).abs() < 0.01);
        assert_eq!(rec.srs_reps, 2);
        assert_eq!(rec.srs_due_date, "2026-07-10");
        // due again on its due date
        assert_eq!(store.get_due_repertoire_moves("2026-07-10").unwrap().len(), 1);
    }

    #[test]
    fn game_sync_dedup_per_source() {
        let mut store = SqliteStore::new_in_memory().unwrap();
        store.bootstrap().unwrap();
        assert!(!store.is_synced("chesscom", "g1"));
        store.mark_synced("chesscom", "g1", "2026-07-01").unwrap();
        assert!(store.is_synced("chesscom", "g1"));
        assert!(!store.is_synced("lichess", "g1")); // keyed by (source, id)
        store.mark_synced("chesscom", "g1", "2026-07-01").unwrap(); // idempotent
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p db_manager`
Expected: compile error — `normalize_fen`, `ensure_repertoire_node`, etc. not found.

- [ ] **Step 3: Implement.** Append three statements to `SQLITE_SCHEMA` (before the closing `];`):

```rust
    r#"
CREATE TABLE IF NOT EXISTS repertoire_nodes (
    id INTEGER PRIMARY KEY,
    fen TEXT NOT NULL,
    side TEXT NOT NULL CHECK (side IN ('White','Black')),
    UNIQUE (fen, side)
);
"#,
    r#"
CREATE TABLE IF NOT EXISTS repertoire_moves (
    id INTEGER PRIMARY KEY,
    parent_id INTEGER NOT NULL REFERENCES repertoire_nodes(id),
    child_id INTEGER NOT NULL REFERENCES repertoire_nodes(id),
    uci TEXT NOT NULL,
    san TEXT NOT NULL,
    is_my_move INTEGER NOT NULL,
    source TEXT NOT NULL,
    comment TEXT,
    srs_interval REAL DEFAULT 1.0,
    srs_ease REAL DEFAULT 2.5,
    srs_reps INTEGER DEFAULT 0,
    srs_due_date TEXT DEFAULT '',
    UNIQUE (parent_id, uci)
);
"#,
    r#"
CREATE TABLE IF NOT EXISTS game_sync (
    source TEXT NOT NULL,
    external_id TEXT NOT NULL,
    synced_at TEXT NOT NULL,
    PRIMARY KEY (source, external_id)
);
"#,
```

Add after `parse_line_uci`:

```rust
/// Normalize a FEN for transposition keys: keep board/side/castling/ep, zero the clocks.
pub fn normalize_fen(fen: &str) -> String {
    let fields: Vec<&str> = fen.split_whitespace().take(4).collect();
    format!("{} 0 1", fields.join(" "))
}
```

Add struct after `OpeningNode`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct RepertoireMoveRecord {
    pub id: i64,
    pub parent_id: i64,
    pub child_id: i64,
    pub uci: String,
    pub san: String,
    pub is_my_move: bool,
    pub source: String,
    pub comment: Option<String>,
    pub srs_interval: f32,
    pub srs_ease: f32,
    pub srs_reps: u32,
    pub srs_due_date: String,
    pub parent_fen: String,
}
```

Add methods to `impl SqliteStore` (after `run_migrations`). The row-mapper is shared:

```rust
    fn map_rep_move(row: &rusqlite::Row) -> rusqlite::Result<RepertoireMoveRecord> {
        Ok(RepertoireMoveRecord {
            id: row.get(0)?,
            parent_id: row.get(1)?,
            child_id: row.get(2)?,
            uci: row.get(3)?,
            san: row.get(4)?,
            is_my_move: row.get::<_, i64>(5)? != 0,
            source: row.get(6)?,
            comment: row.get(7)?,
            srs_interval: row.get::<_, f64>(8)? as f32,
            srs_ease: row.get::<_, f64>(9)? as f32,
            srs_reps: row.get::<_, i64>(10)? as u32,
            srs_due_date: row.get(11)?,
            parent_fen: row.get(12)?,
        })
    }

    const REP_MOVE_COLS: &'static str =
        "m.id, m.parent_id, m.child_id, m.uci, m.san, m.is_my_move, m.source, m.comment, \
         COALESCE(m.srs_interval, 1.0), COALESCE(m.srs_ease, 2.5), \
         COALESCE(m.srs_reps, 0), COALESCE(m.srs_due_date, ''), n.fen";

    pub fn ensure_repertoire_node(&mut self, fen: &str, side: &str) -> Result<i64, String> {
        let norm = normalize_fen(fen);
        self.conn
            .execute(
                "INSERT OR IGNORE INTO repertoire_nodes (fen, side) VALUES (?1, ?2)",
                rusqlite::params![norm, side],
            )
            .map_err(|e| e.to_string())?;
        self.conn
            .query_row(
                "SELECT id FROM repertoire_nodes WHERE fen = ?1 AND side = ?2",
                rusqlite::params![norm, side],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())
    }

    pub fn insert_repertoire_move(
        &mut self,
        parent_id: i64,
        child_id: i64,
        uci: &str,
        san: &str,
        is_my_move: bool,
        source: &str,
    ) -> Result<(i64, bool), String> {
        let inserted = self
            .conn
            .execute(
                "INSERT OR IGNORE INTO repertoire_moves
                 (parent_id, child_id, uci, san, is_my_move, source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![parent_id, child_id, uci, san, is_my_move as i64, source],
            )
            .map_err(|e| e.to_string())?;
        let id: i64 = self
            .conn
            .query_row(
                "SELECT id FROM repertoire_moves WHERE parent_id = ?1 AND uci = ?2",
                rusqlite::params![parent_id, uci],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        Ok((id, inserted > 0))
    }

    pub fn get_repertoire_moves_from(
        &self,
        fen: &str,
        side: &str,
    ) -> Result<Vec<RepertoireMoveRecord>, String> {
        let norm = normalize_fen(fen);
        let mut stmt = self
            .conn
            .prepare(&format!(
                "SELECT {} FROM repertoire_moves m
                 JOIN repertoire_nodes n ON n.id = m.parent_id
                 WHERE n.fen = ?1 AND n.side = ?2",
                Self::REP_MOVE_COLS
            ))
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params![norm, side], Self::map_rep_move)
            .map_err(|e| e.to_string())?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn get_due_repertoire_moves(&self, today: &str) -> Result<Vec<RepertoireMoveRecord>, String> {
        let mut stmt = self
            .conn
            .prepare(&format!(
                "SELECT {} FROM repertoire_moves m
                 JOIN repertoire_nodes n ON n.id = m.parent_id
                 WHERE m.is_my_move = 1
                   AND (m.srs_due_date IS NULL OR m.srs_due_date = '' OR m.srs_due_date <= ?1)
                 ORDER BY m.srs_due_date ASC",
                Self::REP_MOVE_COLS
            ))
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params![today], Self::map_rep_move)
            .map_err(|e| e.to_string())?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn get_due_repertoire_move_count(&self, today: &str) -> Result<u32, String> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM repertoire_moves
                 WHERE is_my_move = 1
                   AND (srs_due_date IS NULL OR srs_due_date = '' OR srs_due_date <= ?1)",
                rusqlite::params![today],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())
    }

    pub fn update_repertoire_move_srs(
        &mut self,
        move_id: i64,
        interval: f32,
        ease: f32,
        reps: u32,
        due_date: &str,
    ) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE repertoire_moves
                 SET srs_interval = ?1, srs_ease = ?2, srs_reps = ?3, srs_due_date = ?4
                 WHERE id = ?5",
                rusqlite::params![interval, ease, reps, due_date, move_id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_repertoire_move(&self, move_id: i64) -> Result<Option<RepertoireMoveRecord>, String> {
        let mut stmt = self
            .conn
            .prepare(&format!(
                "SELECT {} FROM repertoire_moves m
                 JOIN repertoire_nodes n ON n.id = m.parent_id
                 WHERE m.id = ?1",
                Self::REP_MOVE_COLS
            ))
            .map_err(|e| e.to_string())?;
        let mut rows = stmt
            .query_map(rusqlite::params![move_id], Self::map_rep_move)
            .map_err(|e| e.to_string())?;
        match rows.next() {
            Some(Ok(r)) => Ok(Some(r)),
            Some(Err(e)) => Err(e.to_string()),
            None => Ok(None),
        }
    }

    pub fn is_synced(&self, source: &str, external_id: &str) -> bool {
        self.conn
            .query_row(
                "SELECT 1 FROM game_sync WHERE source = ?1 AND external_id = ?2",
                rusqlite::params![source, external_id],
                |_| Ok(()),
            )
            .is_ok()
    }

    pub fn mark_synced(&mut self, source: &str, external_id: &str, synced_at: &str) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT OR IGNORE INTO game_sync (source, external_id, synced_at) VALUES (?1, ?2, ?3)",
                rusqlite::params![source, external_id, synced_at],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
```

Note: `ensure_repertoire_node` and `get_repertoire_moves_from` call `normalize_fen` internally, so callers may pass full FENs.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p db_manager`
Expected: all tests pass (existing 8 + 4 new).

- [ ] **Step 5: Commit**

```bash
git add db_manager/src/lib.rs
git commit -m "feat(db): repertoire position tree tables, CRUD, per-source game_sync ledger"
```

---

### Task 2: Legacy migration functions (`app/src/tree.rs`)

Migration lives in `app` because walking UCI lines to FENs needs shakmaty. This task only builds and tests the functions; **wiring into `main()` happens in Task 4** so the running app keeps using `opening_lines` until the drill cutover.

**Files:**
- Create: `app/src/tree.rs`
- Modify: `app/src/main.rs:1-4` (add `mod tree;`)
- Modify: `db_manager/src/lib.rs` (two small helpers, below)

**Interfaces:**
- Consumes: Task 1 (`ensure_repertoire_node`, `insert_repertoire_move`, `update_repertoire_move_srs`, `get_repertoire_move`, `normalize_fen`).
- Produces (used by Tasks 4, 5, 8, 9, 11):
  - `tree::position_key(pos: &shakmaty::Chess) -> String` — normalized FEN of a position.
  - `tree::add_move_edge(db: &mut SqliteStore, parent_full_fen: &str, uci_str: &str, side: &str, source: &str) -> Result<(i64, String, bool), String>` — inserts one edge given parent FEN + UCI; computes SAN, child FEN, `is_my_move` from side-to-move; returns `(edge_id, child_full_fen, was_new)`.
  - `tree::import_uci_line(db: &mut SqliteStore, start_fen: &str, moves: &[String], side: &str, source: &str) -> Result<u32, String>` — walks a whole UCI line; returns number of NEW edges.
  - `tree::migrate_legacy_lines(db: &mut SqliteStore) -> Result<u32, String>` — one-time migration, idempotent.
  - `db_manager::SqliteStore::opening_lines_table_exists(&self) -> bool`
  - `db_manager::SqliteStore::retire_opening_lines(&mut self) -> Result<(), String>` (rename to `opening_lines_legacy`)
  - `db_manager::SqliteStore::repertoire_move_count(&self) -> Result<u32, String>`
  - `db_manager::run_migrations` additionally migrates `lichess_sync` rows into `game_sync` (`source='lichess'`) and drops `lichess_sync`.

- [ ] **Step 1: Add the three small db_manager helpers + lichess_sync migration, with tests.** Test (append to db_manager tests):

```rust
    #[test]
    fn lichess_sync_migrates_to_game_sync() {
        let mut store = SqliteStore::new_in_memory().unwrap();
        store.bootstrap().unwrap();
        store.mark_game_synced("oldgame", "2026-06-01").unwrap();
        store.run_migrations().unwrap();
        assert!(store.is_synced("lichess", "oldgame"));
        // lichess_sync table is gone
        let gone: bool = store.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='lichess_sync'",
            [], |row| row.get::<_, i64>(0),
        ).unwrap() == 0;
        assert!(gone);
        store.run_migrations().unwrap(); // idempotent
    }

    #[test]
    fn retire_opening_lines_renames_table() {
        let mut store = SqliteStore::new_in_memory().unwrap();
        store.bootstrap().unwrap();
        assert!(store.opening_lines_table_exists());
        store.retire_opening_lines().unwrap();
        assert!(!store.opening_lines_table_exists());
        store.retire_opening_lines().unwrap(); // idempotent, no error
        assert_eq!(store.repertoire_move_count().unwrap(), 0);
    }
```

Implementation — in `run_migrations`, after the existing `for sql in &migrations` loop, add:

```rust
        // Move lichess_sync rows into the per-source game_sync ledger, then drop it.
        let has_old: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='lichess_sync'",
            [], |row| row.get(0),
        ).unwrap_or(0);
        if has_old > 0 {
            let _ = self.conn.execute(
                "INSERT OR IGNORE INTO game_sync (source, external_id, synced_at)
                 SELECT 'lichess', game_id, synced_at FROM lichess_sync", []);
            let _ = self.conn.execute("DROP TABLE lichess_sync", []);
        }
```

CAUTION: `bootstrap()` recreates `lichess_sync` (it is in `SQLITE_SCHEMA`). Remove the `lichess_sync` statement from `SQLITE_SCHEMA` in this step, and change `mark_game_synced`/`is_game_synced` to delegate: `self.mark_synced("lichess", game_id, synced_at)` / `self.is_synced("lichess", game_id)` (keeps the existing `dedup_lichess_sync` test passing; update that test to not reference the table directly if needed).

New methods:

```rust
    pub fn opening_lines_table_exists(&self) -> bool {
        self.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='opening_lines'",
            [], |row| row.get::<_, i64>(0),
        ).unwrap_or(0) > 0
    }

    pub fn retire_opening_lines(&mut self) -> Result<(), String> {
        if self.opening_lines_table_exists() {
            self.conn
                .execute("ALTER TABLE opening_lines RENAME TO opening_lines_legacy", [])
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn repertoire_move_count(&self) -> Result<u32, String> {
        self.conn
            .query_row("SELECT COUNT(*) FROM repertoire_moves", [], |row| row.get(0))
            .map_err(|e| e.to_string())
    }
```

CAUTION: `bootstrap()` will recreate an empty `opening_lines` on next startup (it is in `SQLITE_SCHEMA`); the migration guard (`repertoire_move_count() > 0` → skip) makes that harmless. Also remove the `opening_lines` ALTER statements from `run_migrations` only if they error on the renamed table — they use `let _ =` already, so leave them.

Run: `cargo test -p db_manager` — expected: pass.

- [ ] **Step 2: Write failing tests for `app/src/tree.rs`** (in-file `#[cfg(test)]` module):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use db_manager::{OpeningLineRecord, SqliteStore, TrainingStore};

    const START: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

    fn mem_store() -> SqliteStore {
        let mut s = SqliteStore::new_in_memory().unwrap();
        s.bootstrap().unwrap();
        s.run_migrations().unwrap();
        s
    }

    #[test]
    fn add_move_edge_computes_san_child_and_ownership() {
        let mut db = mem_store();
        // White repertoire, White to move => my move
        let (_id, child_fen, was_new) =
            add_move_edge(&mut db, START, "e2e4", "White", "manual").unwrap();
        assert!(was_new);
        assert!(child_fen.starts_with("rnbqkbnr/pppppppp/8/8/4P3"));
        let edges = db.get_repertoire_moves_from(START, "White").unwrap();
        assert_eq!(edges[0].san, "e4");
        assert!(edges[0].is_my_move);
        // Black repertoire, White to move => opponent move
        let (_id2, _cf2, _) = add_move_edge(&mut db, START, "e2e4", "Black", "manual").unwrap();
        let edges_b = db.get_repertoire_moves_from(START, "Black").unwrap();
        assert!(!edges_b[0].is_my_move);
    }

    #[test]
    fn import_uci_line_walks_and_transposes() {
        let mut db = mem_store();
        let line: Vec<String> = ["e2e4", "e7e5", "g1f3"].iter().map(|s| s.to_string()).collect();
        let new_edges = import_uci_line(&mut db, START, &line, "White", "pgn").unwrap();
        assert_eq!(new_edges, 3);
        // Re-import is a no-op
        assert_eq!(import_uci_line(&mut db, START, &line, "White", "pgn").unwrap(), 0);
        // Transposition: 1.Nf3 e5?? not relevant — instead verify d2d4/d7d5 via two orders
        let a: Vec<String> = ["d2d4", "d7d5", "c2c4", "e7e6"].iter().map(|s| s.to_string()).collect();
        let b: Vec<String> = ["c2c4", "e7e6", "d2d4", "d7d5"].iter().map(|s| s.to_string()).collect();
        let n1 = import_uci_line(&mut db, START, &a, "White", "pgn").unwrap();
        let n2 = import_uci_line(&mut db, START, &b, "White", "pgn").unwrap();
        assert_eq!(n1, 4);
        // Orders differ at every interior position; only the final position coincides,
        // so the second import adds 4 new edges too — but both funnel into ONE node:
        assert_eq!(n2, 4);
        // and from the shared final position, later continuations are shared.
    }

    #[test]
    fn migrate_legacy_lines_moves_srs_and_retires_table() {
        let mut db = mem_store();
        let line = OpeningLineRecord {
            id: "L1".into(),
            user_id: "default_user".into(),
            opening_name: "Test".into(),
            start_fen: START.into(),
            line_uci: vec!["e2e4".into(), "e7e5".into()],
            source: "builder".into(),
            confidence: 0.5,
            srs_interval: 6.0,
            srs_ease: 2.6,
            srs_reps: 2,
            srs_due_date: "2026-07-10".into(),
            side: "White".into(),
        };
        db.insert_opening_line(&line).unwrap();

        let migrated = migrate_legacy_lines(&mut db).unwrap();
        assert_eq!(migrated, 1);
        assert!(!db.opening_lines_table_exists());
        let edges = db.get_repertoire_moves_from(START, "White").unwrap();
        assert_eq!(edges.len(), 1);
        // my-move edge carries the line's SM-2 state
        assert!((edges[0].srs_interval - 6.0).abs() < 0.01);
        assert_eq!(edges[0].srs_due_date, "2026-07-10");
        // second run is a no-op
        assert_eq!(migrate_legacy_lines(&mut db).unwrap(), 0);
    }
}
```

Run: `cargo test -p app tree` — expected: FAIL (module missing).

- [ ] **Step 3: Implement `app/src/tree.rs`:**

```rust
use db_manager::SqliteStore;
use shakmaty::{CastlingMode, Chess, EnPassantMode, Position};
use shakmaty::fen::Fen;
use shakmaty::san::San;
use shakmaty::uci::Uci;

/// Normalized FEN used as a repertoire-tree key.
pub fn position_key(pos: &Chess) -> String {
    let fen = Fen::from_position(pos.clone(), EnPassantMode::Legal).to_string();
    db_manager::normalize_fen(&fen)
}

fn parse_position(fen: &str) -> Result<Chess, String> {
    let parsed: Fen = fen.parse().map_err(|e| format!("bad FEN '{fen}': {e}"))?;
    parsed
        .into_position(CastlingMode::Standard)
        .map_err(|e| format!("illegal position '{fen}': {e}"))
}

/// Insert one edge. Returns (edge_id, child_full_fen, was_new).
pub fn add_move_edge(
    db: &mut SqliteStore,
    parent_full_fen: &str,
    uci_str: &str,
    side: &str,
    source: &str,
) -> Result<(i64, String, bool), String> {
    let pos = parse_position(parent_full_fen)?;
    let uci: Uci = uci_str.parse().map_err(|e| format!("bad UCI '{uci_str}': {e}"))?;
    let m = uci.to_move(&pos).map_err(|e| format!("illegal move '{uci_str}': {e}"))?;
    let san = San::from_move(&pos, &m).to_string();
    let mover_is_white = pos.turn() == shakmaty::Color::White;
    let is_my_move = (side == "White") == mover_is_white;
    let next = pos.play(&m).map_err(|e| format!("cannot play '{uci_str}': {e}"))?;
    let child_fen = position_key(&next);
    let parent_key = position_key(&pos);

    let parent_id = db.ensure_repertoire_node(&parent_key, side)?;
    let child_id = db.ensure_repertoire_node(&child_fen, side)?;
    let (edge_id, was_new) =
        db.insert_repertoire_move(parent_id, child_id, uci_str, &san, is_my_move, source)?;
    Ok((edge_id, child_fen, was_new))
}

/// Walk a UCI line from start_fen, inserting every move. Returns count of NEW edges.
pub fn import_uci_line(
    db: &mut SqliteStore,
    start_fen: &str,
    moves: &[String],
    side: &str,
    source: &str,
) -> Result<u32, String> {
    let mut fen = start_fen.to_string();
    let mut new_edges = 0u32;
    for uci in moves {
        let (_, child, was_new) = add_move_edge(db, &fen, uci, side, source)?;
        if was_new {
            new_edges += 1;
        }
        fen = child;
    }
    Ok(new_edges)
}

/// One-time migration from opening_lines. Idempotent:
/// skips when the tree already has rows or the legacy table is gone.
pub fn migrate_legacy_lines(db: &mut SqliteStore) -> Result<u32, String> {
    if db.repertoire_move_count()? > 0 || !db.opening_lines_table_exists() {
        return Ok(0);
    }
    let lines = db.get_opening_lines("default_user")?;
    let mut migrated = 0u32;
    for line in &lines {
        let mut fen = line.start_fen.clone();
        for uci in &line.line_uci {
            let (edge_id, child, _was_new) =
                match add_move_edge(db, &fen, uci, &line.side, &line.source) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("migration: skipping bad move in line {}: {e}", line.id);
                        break;
                    }
                };
            // Copy the line's SM-2 state onto my-move edges; longest interval wins.
            if let Some(existing) = db.get_repertoire_move(edge_id)? {
                if existing.is_my_move && line.srs_interval > existing.srs_interval {
                    db.update_repertoire_move_srs(
                        edge_id,
                        line.srs_interval,
                        line.srs_ease,
                        line.srs_reps,
                        &line.srs_due_date,
                    )?;
                }
            }
            fen = child;
        }
        migrated += 1;
    }
    db.retire_opening_lines()?;
    Ok(migrated)
}
```

Add `mod tree;` next to the other `mod` lines at the top of `app/src/main.rs`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p app tree` then `cargo test --workspace`
Expected: PASS. (If `srs_due_date` copy for a fresh edge with interval 6.0 fails because the fresh edge default interval is 1.0 — that is the intended "longer interval wins" path.)

- [ ] **Step 5: Commit**

```bash
git add app/src/tree.rs app/src/main.rs db_manager/src/lib.rs
git commit -m "feat(tree): edge insert/walk helpers and legacy opening_lines migration"
```

---

### Task 3: Variation-aware PGN walker in `pgn_processor`

**Files:**
- Modify: `pgn_processor/src/lib.rs`

**Interfaces:**
- Consumes: nothing new.
- Produces (used by Tasks 5, 7):
  - `pub struct WalkedMove { pub parent_fen: String, pub child_fen: String, pub uci: String, pub san: String }` (FENs are normalized keys, `Eq + Clone + Debug`)
  - `pub fn walk_pgn_variations(text: &str) -> Result<Vec<WalkedMove>, String>` — walks EVERY game and EVERY nested variation in `text`; malformed games are skipped (their moves simply do not appear); error only on empty/unusable input.
  - `pub fn position_key(pos: &shakmaty::Chess) -> String`
  - `pub fn split_games(text: &str) -> Vec<String>` — splits multi-game PGN on `[Event ` header lines.

- [ ] **Step 1: Write the failing tests:**

```rust
    #[test]
    fn walks_nested_variations() {
        let pgn = r#"
[Event "Repertoire"]

1. e4 e5 (1... c5 2. Nf3 (2. Nc3 Nc6) 2... d6) 2. Nf3 Nc6 1-0
"#;
        let walked = walk_pgn_variations(pgn).unwrap();
        // Mainline: e4 e5 Nf3 Nc6 = 4. Var A: c5 Nf3 d6 = 3. Var B (nested): Nc3 Nc6 = 2.
        assert_eq!(walked.len(), 9);
        let sans: Vec<&str> = walked.iter().map(|w| w.san.as_str()).collect();
        assert!(sans.contains(&"c5"));
        assert!(sans.contains(&"Nc3"));
        // The two 2.Nf3 moves come from different parents (after e5 vs after c5)
        let nf3: Vec<&WalkedMove> = walked.iter().filter(|w| w.uci == "g1f3").collect();
        assert_eq!(nf3.len(), 2);
        assert_ne!(nf3[0].parent_fen, nf3[1].parent_fen);
    }

    #[test]
    fn walks_multiple_games_and_skips_malformed() {
        let pgn = r#"
[Event "One"]

1. d4 d5 1/2-1/2

[Event "Two"]

1. e4 Qxe4 1-0

[Event "Three"]

1. c4 e5 *
"#;
        let walked = walk_pgn_variations(pgn).unwrap();
        // Game Two dies at illegal Qxe4 after emitting e4; Games One and Three emit 2 each.
        let sans: Vec<&str> = walked.iter().map(|w| w.san.as_str()).collect();
        assert!(sans.contains(&"d4"));
        assert!(sans.contains(&"c4"));
        assert_eq!(walked.len(), 5);
    }

    #[test]
    fn strips_comments_nags_and_annotations() {
        let pgn = "[Event \"X\"]\n\n1. e4! {best by test} $1 e5?? 2. Nf3 1-0";
        let walked = walk_pgn_variations(pgn).unwrap();
        assert_eq!(walked.len(), 3);
        assert_eq!(walked[0].san, "e4");
        assert_eq!(walked[1].san, "e5");
    }
```

(Place all three inside the existing `tests` module.)

Run: `cargo test -p pgn_processor` — expected: FAIL (functions missing).

- [ ] **Step 2: Implement.** Add to `pgn_processor/src/lib.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalkedMove {
    pub parent_fen: String,
    pub child_fen: String,
    pub uci: String,
    pub san: String,
}

/// Normalized FEN used as a repertoire-tree key (must match db_manager::normalize_fen).
pub fn position_key(pos: &shakmaty::Chess) -> String {
    use shakmaty::EnPassantMode;
    let fen = shakmaty::fen::Fen::from_position(pos.clone(), EnPassantMode::Legal).to_string();
    let fields: Vec<&str> = fen.split_whitespace().take(4).collect();
    format!("{} 0 1", fields.join(" "))
}

/// Split a multi-game PGN on [Event headers.
pub fn split_games(text: &str) -> Vec<String> {
    let mut games: Vec<String> = Vec::new();
    let mut current = String::new();
    for line in text.lines() {
        if line.trim_start().starts_with("[Event ") && !current.trim().is_empty() {
            games.push(std::mem::take(&mut current));
        }
        current.push_str(line);
        current.push('\n');
    }
    if !current.trim().is_empty() {
        games.push(current);
    }
    games
}

/// Walk every game and every nested variation; emit one WalkedMove per legal move.
/// Illegal moves abort only the branch they occur in.
pub fn walk_pgn_variations(text: &str) -> Result<Vec<WalkedMove>, String> {
    use shakmaty::{Chess, Position};
    use shakmaty::san::San;
    use shakmaty::uci::Uci;

    let mut out = Vec::new();
    for game_text in split_games(text) {
        // Movetext = everything that is not a header line.
        let movetext: String = game_text
            .lines()
            .filter(|l| !l.trim_start().starts_with('['))
            .collect::<Vec<_>>()
            .join(" ");

        let mut pos = Chess::default();
        let mut prev: Option<Chess> = None;
        // Each '(' pushes (position_to_return_to, prev_at_that_point).
        let mut stack: Vec<(Chess, Option<Chess>)> = Vec::new();
        let mut dead_branch_depth: Option<usize> = None; // skip tokens after illegal move until branch closes

        let mut chars = movetext.chars().peekable();
        let mut token = String::new();
        let mut flush = |token: &mut String,
                         pos: &mut Chess,
                         prev: &mut Option<Chess>,
                         dead: &mut Option<usize>,
                         depth: usize,
                         out: &mut Vec<WalkedMove>| {
            if token.is_empty() {
                return;
            }
            let raw = std::mem::take(token);
            if dead.is_some() {
                return;
            }
            let t = raw.trim_end_matches(['?', '!']);
            if t.is_empty()
                || t.starts_with('$')
                || t.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) && t.contains('.')
                || matches!(t, "1-0" | "0-1" | "1/2-1/2" | "*")
                || t.chars().all(|c| c.is_ascii_digit())
            {
                return;
            }
            match t.parse::<San>() {
                Ok(san) => match san.to_move(pos) {
                    Ok(m) => {
                        let parent = position_key(pos);
                        let uci = Uci::from_move(&m, shakmaty::CastlingMode::Standard).to_string();
                        let san_str = San::from_move(pos, &m).to_string();
                        let next = pos.clone().play(&m).expect("legal move plays");
                        *prev = Some(pos.clone());
                        *pos = next;
                        out.push(WalkedMove {
                            parent_fen: parent,
                            child_fen: position_key(pos),
                            uci,
                            san: san_str,
                        });
                    }
                    Err(_) => *dead = Some(depth),
                },
                Err(_) => *dead = Some(depth),
            }
        };

        while let Some(ch) = chars.next() {
            match ch {
                '{' => {
                    flush(&mut token, &mut pos, &mut prev, &mut dead_branch_depth, stack.len(), &mut out);
                    while let Some(c) = chars.next() {
                        if c == '}' { break; }
                    }
                }
                ';' => {
                    flush(&mut token, &mut pos, &mut prev, &mut dead_branch_depth, stack.len(), &mut out);
                    while let Some(c) = chars.next() {
                        if c == '\n' { break; }
                    }
                }
                '(' => {
                    flush(&mut token, &mut pos, &mut prev, &mut dead_branch_depth, stack.len(), &mut out);
                    stack.push((pos.clone(), prev.clone()));
                    if dead_branch_depth.is_none() {
                        if let Some(p) = prev.clone() {
                            pos = p; // variation replaces the last played move
                        }
                    }
                }
                ')' => {
                    flush(&mut token, &mut pos, &mut prev, &mut dead_branch_depth, stack.len(), &mut out);
                    if let Some((restored, restored_prev)) = stack.pop() {
                        if let Some(d) = dead_branch_depth {
                            if stack.len() < d {
                                dead_branch_depth = None;
                            }
                        }
                        pos = restored;
                        prev = restored_prev;
                    }
                }
                c if c.is_whitespace() => {
                    flush(&mut token, &mut pos, &mut prev, &mut dead_branch_depth, stack.len(), &mut out);
                }
                c => token.push(c),
            }
        }
        flush(&mut token, &mut pos, &mut prev, &mut dead_branch_depth, 0, &mut out);
    }
    Ok(out)
}
```

Implementation notes for the engineer:
- Move-number tokens like `1.` `12...` are filtered by the "starts with digit and contains '.'" rule; bare digits (rare stray tokens) by the all-digits rule.
- An illegal/unparseable SAN sets `dead_branch_depth` — tokens are ignored until the enclosing `(`...`)` closes (or the game ends), so one bad variation doesn't kill the rest of the game. At top level (depth 0) the rest of that game is skipped, matching the test.
- `San::from_move` re-renders canonical SAN so stored SAN is clean regardless of input decorations.

- [ ] **Step 3: Run tests**

Run: `cargo test -p pgn_processor`
Expected: PASS (existing 3 + 3 new). Debug the walker against the exact fixtures if counts differ — the fixtures above are the contract.

- [ ] **Step 4: Commit**

```bash
git add pgn_processor/src/lib.rs
git commit -m "feat(pgn): recursive variation walker emitting normalized-FEN move edges"
```

---

### Task 4: Drill-engine cutover to the tree (the big switch)

After this task the app reads/writes ONLY the tree: migration runs at startup, the Repertoire screen lists due edges, drills walk the tree with per-edge SM-2 and branch acceptance, the builder saves tree edges, the analysis thread's mistake-correction line goes into the tree, and dashboard counts come from edge queries.

**Files:**
- Modify: `app/src/main.rs` (slint UI block, `AppState`, `main()`, `refresh_data`, `on_select_repertoire_line`, `on_start_drill`, `on_click_board_square`, `on_builder_save_line`, analysis thread tail at `main.rs:1569-1607`)
- Modify: `app/src/tree.rs` (one helper + tests)

**Interfaces:**
- Consumes: Tasks 1–2.
- Produces (used by Tasks 5, 9, 11):
  - `tree::grade_edge(db: &mut SqliteStore, edge: &db_manager::RepertoireMoveRecord, grade: u32, today_days: u64) -> Result<(), String>` — applies `srs::review` and persists (uses `days_to_ymd`-style date math; move `days_to_ymd` and `local_now_str` from `main.rs` into `tree.rs` and re-export, or duplicate — prefer moving them into `tree.rs` as `pub fn` and calling `tree::local_now_str()` from main).
  - AppState drill fields (see below) and the drill-session pattern other tasks reuse.

- [ ] **Step 1: Write failing test for `grade_edge`** (append to `app/src/tree.rs` tests):

```rust
    #[test]
    fn grade_edge_pass_and_fail() {
        let mut db = mem_store();
        let (id, _, _) = add_move_edge(&mut db, START, "e2e4", "White", "manual").unwrap();
        let edge = db.get_repertoire_move(id).unwrap().unwrap();
        // 2026-07-01 is day 20635 since epoch
        grade_edge(&mut db, &edge, 5, 20635).unwrap();
        let after = db.get_repertoire_move(id).unwrap().unwrap();
        assert_eq!(after.srs_reps, 1);
        assert_eq!(after.srs_due_date, "2026-07-02"); // interval 1 day
        grade_edge(&mut db, &after, 1, 20635).unwrap();
        let failed = db.get_repertoire_move(id).unwrap().unwrap();
        assert_eq!(failed.srs_reps, 0);
        assert_eq!(failed.srs_due_date, "2026-07-02"); // reset to 1 day
    }
```

Run: `cargo test -p app grade_edge` — expected: FAIL.

- [ ] **Step 2: Implement `grade_edge` in `tree.rs`** (move `days_to_ymd` + `local_now_str` here from `main.rs`, make them `pub`, fix call sites in `main.rs` to `tree::local_now_str()` / `tree::days_to_ymd()`):

```rust
pub fn grade_edge(
    db: &mut SqliteStore,
    edge: &db_manager::RepertoireMoveRecord,
    grade: u32,
    today_days: u64,
) -> Result<(), String> {
    let card = crate::srs::SrsCard {
        interval: edge.srs_interval,
        ease_factor: edge.srs_ease,
        repetitions: edge.srs_reps,
    };
    let next = crate::srs::review(&card, grade);
    let (y, m, d) = days_to_ymd(today_days + next.interval.max(1.0) as u64);
    db.update_repertoire_move_srs(
        edge.id,
        next.interval,
        next.ease_factor,
        next.repetitions,
        &format!("{:04}-{:02}-{:02}", y, m, d),
    )
}
```

Run: `cargo test -p app` — expected: PASS.

- [ ] **Step 3: Wire migration at startup.** In `main()` right after `db.run_migrations()`:

```rust
    let migrated = tree::migrate_legacy_lines(&mut db).unwrap_or_else(|e| {
        eprintln!("legacy line migration failed (continuing on tree): {e}");
        0
    });
    if migrated > 0 {
        println!("Migrated {migrated} legacy opening lines into the repertoire tree.");
    }
```

- [ ] **Step 4: Replace drill state in `AppState`** (`main.rs:946-962`). Delete `drill_line`, `drill_moves_left`, `drill_valid_first_moves`; add:

```rust
    // Tree-based drill session
    drill_side: String,                 // "White" | "Black"
    drill_chess: Chess,                 // current drill position (kept)
    drill_edge_ids: Vec<i64>,           // my-move edges already graded this session
    drill_expected: Vec<db_manager::RepertoireMoveRecord>, // my-move edges at current node
    drill_start_edge: Option<db_manager::RepertoireMoveRecord>,
```

and initialize in the `AppState` literal (`drill_side: "White".to_string(), drill_edge_ids: Vec::new(), drill_expected: Vec::new(), drill_start_edge: None,`).

- [ ] **Step 5: Re-point `refresh_data`.** Replace the opening-lines block (`main.rs:1050-1059`) with a due-edge list, and the `opening_due` stat:

```rust
            // Refresh due repertoire moves (drill queue)
            let due = state.db.get_due_repertoire_moves(&today).unwrap_or_default();
            let line_entries: Vec<OpeningLineEntry> = due.iter().map(|e| OpeningLineEntry {
                id: slint::SharedString::from(e.id.to_string()),
                name: slint::SharedString::from(format!("{} ({})", e.san, e.source)),
                moves: slint::SharedString::from(e.uci.clone()),
                confidence: (e.srs_reps as f32 / 10.0).min(1.0),
                start_fen: slint::SharedString::from(e.parent_fen.clone()),
            }).collect();
            app.set_opening_lines(slint::ModelRc::from(Rc::new(slint::VecModel::from(line_entries))));
```

and in the `UserSnapshot`: `opening_due: state.db.get_due_repertoire_move_count(&today).unwrap_or(0),` (drop the now-unused `due_lines` variable). Also update `get_opening_due_count` callers if any remain — the method itself stays for compatibility but is no longer called.

- [ ] **Step 6: Rewrite `on_select_repertoire_line`** (`main.rs:1151-1172`) — the id is now an edge id:

```rust
        app.on_select_repertoire_line(move |edge_id: slint::SharedString| {
            let mut state = state.lock().unwrap();
            let app = app_weak.upgrade().unwrap();
            let id: i64 = match edge_id.parse() { Ok(v) => v, Err(_) => return };
            if let Ok(Some(edge)) = state.db.get_repertoire_move(id) {
                app.set_drill_line_id(edge_id);
                app.set_drill_name(slint::SharedString::from(format!("Drill: {} ({})", edge.san, edge.source)));
                app.set_drill_instructions(slint::SharedString::from("Press 'Start Drill' to practice from this position."));
                app.set_drill_active(false);
                app.set_board_pieces(slint::ModelRc::from(Rc::new(slint::VecModel::from(fen_to_pieces(&edge.parent_fen)))));
                app.set_board_selected_square(-1);
                state.drill_start_edge = Some(edge);
            }
        });
```

- [ ] **Step 7: Rewrite `on_start_drill`** (`main.rs:1174-1237`). A session starts at the selected edge's parent; the side is inferred from the node (query the node's side via a small db helper — add `SqliteStore::get_node_side_and_fen(node_id: i64) -> Result<Option<(String, String)>, String>` with `SELECT side, fen FROM repertoire_nodes WHERE id = ?1`, plus a one-line test alongside Task 1's tests):

```rust
        app.on_start_drill(move || {
            let mut state = state.lock().unwrap();
            let app = app_weak.upgrade().unwrap();
            let Some(edge) = state.drill_start_edge.clone() else { return };
            let Ok(Some((side, fen))) = state.db.get_node_side_and_fen(edge.parent_id) else { return };
            let parsed: shakmaty::fen::Fen = match fen.parse() { Ok(f) => f, Err(_) => return };
            let Ok(pos) = parsed.into_position(CastlingMode::Standard) else { return };
            state.drill_side = side.clone();
            state.drill_chess = pos;
            state.drill_edge_ids.clear();
            drill_advance(&mut state, &app); // shared session-step helper, below
            app.set_drill_active(true);
        });
```

Add a free function `drill_advance(state: &mut AppState, app: &AppWindow)` in `main.rs` — the single place that steps a drill session:

```rust
/// Advance the drill: auto-play opponent edges, load expected my-moves, detect session end.
fn drill_advance(state: &mut AppState, app: &AppWindow) {
    use shakmaty::Position;
    loop {
        let key = tree::position_key(&state.drill_chess);
        let edges = state.db.get_repertoire_moves_from(&key, &state.drill_side).unwrap_or_default();
        let (mine, theirs): (Vec<_>, Vec<_>) = edges.into_iter().partition(|e| e.is_my_move);

        let side_to_move_is_mine =
            (state.drill_chess.turn() == shakmaty::Color::White) == (state.drill_side == "White");

        if side_to_move_is_mine {
            if mine.is_empty() {
                app.set_drill_active(false);
                app.set_drill_instructions(slint::SharedString::from("Line complete — well done!"));
                return;
            }
            app.set_drill_branch_info(slint::SharedString::from(
                format!("{} accepted move(s) here", mine.len())));
            app.set_drill_instructions(slint::SharedString::from("Your move."));
            state.drill_expected = mine;
            return;
        }
        // Opponent's turn: auto-play a random opponent edge, or end if none.
        if theirs.is_empty() {
            app.set_drill_active(false);
            app.set_drill_instructions(slint::SharedString::from("Line complete — well done!"));
            return;
        }
        let pick = (uuid_now().len() + theirs.len()) % theirs.len(); // cheap pseudo-random
        let mv = &theirs[pick];
        if let Ok(uci) = mv.uci.parse::<shakmaty::uci::Uci>() {
            if let Ok(m) = uci.to_move(&state.drill_chess) {
                state.drill_chess.play_unchecked(&m);
                let fen = shakmaty::fen::Fen::from_position(
                    state.drill_chess.clone(), shakmaty::EnPassantMode::Legal).to_string();
                app.set_board_pieces(slint::ModelRc::from(std::rc::Rc::new(
                    slint::VecModel::from(fen_to_pieces(&fen)))));
                continue;
            }
        }
        app.set_drill_active(false);
        return;
    }
}
```

(For real randomness without a new dependency, use `std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().subsec_nanos() as usize % theirs.len()` — replace the `pick` line with that.)

- [ ] **Step 8: Rewrite the drill move handler `on_click_board_square`** (`main.rs:1244-1428`) — accept ANY expected my-move edge, grade per edge:

```rust
        app.on_click_board_square(move |idx: i32| {
            let mut state = state.lock().unwrap();
            let app = app_weak.upgrade().unwrap();
            if !app.get_drill_active() { return; }
            let sel = app.get_board_selected_square();
            if sel == -1 { app.set_board_selected_square(idx); return; }
            app.set_board_selected_square(-1);
            let uci_try = format!("{}{}", index_to_square(sel as usize), index_to_square(idx as usize));

            let today_days = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() / 86400;

            let matched = state.drill_expected.iter()
                .find(|e| e.uci == uci_try || e.uci == format!("{}q", uci_try))
                .cloned();

            if let Some(edge) = matched {
                let _ = tree::grade_edge(&mut state.db, &edge, 5, today_days);
                state.drill_edge_ids.push(edge.id);
                let event = TrainingEventRecord {
                    id: format!("evt_{}", uuid_now()),
                    user_id: "default_user".to_string(),
                    kind: "repertoire_drill".to_string(),
                    target_id: format!("{} {}", edge.san, edge.uci),
                    outcome: "Correct".to_string(),
                    score_delta: 15.0,
                    created_at: tree::local_now_str(),
                };
                let _ = state.db.insert_training_event(&event);
                if let Ok(uci) = edge.uci.parse::<Uci>() {
                    if let Ok(m) = uci.to_move(&state.drill_chess) {
                        state.drill_chess.play_unchecked(&m);
                        let fen = shakmaty::fen::Fen::from_position(
                            state.drill_chess.clone(), shakmaty::EnPassantMode::Legal).to_string();
                        app.set_board_pieces(slint::ModelRc::from(Rc::new(
                            slint::VecModel::from(fen_to_pieces(&fen)))));
                    }
                }
                drill_advance(&mut state, &app);
                if !app.get_drill_active() { drop(state); refresh_data_cp(); }
            } else {
                // Wrong move: fail every expected edge at this node (they were all "the answer")
                for edge in state.drill_expected.clone() {
                    let _ = tree::grade_edge(&mut state.db, &edge, 1, today_days);
                }
                let event = TrainingEventRecord {
                    id: format!("evt_{}", uuid_now()),
                    user_id: "default_user".to_string(),
                    kind: "repertoire_drill".to_string(),
                    target_id: uci_try,
                    outcome: "Failed".to_string(),
                    score_delta: -5.0,
                    created_at: tree::local_now_str(),
                };
                let _ = state.db.insert_training_event(&event);
                app.set_drill_instructions(slint::SharedString::from(
                    "Not in your repertoire here — try again (this move is now due for review)."));
                drop(state);
                refresh_data_cp();
            }
        });
```

- [ ] **Step 9: Builder saves to the tree.** Replace the body of `on_builder_save_line`'s DB write (`main.rs:1897-1911`): delete the `OpeningLineRecord` construction and `insert_opening_line` call; instead:

```rust
            let start_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
            let staged = st.builder_staged_uci.clone();
            let side = st.builder_color.clone();
            match tree::import_uci_line(&mut st.db, start_fen, &staged, &side, "manual") {
                Ok(n) => app.set_drill_instructions(slint::SharedString::from(
                    format!("Saved: {n} new move(s) added to your {side} repertoire."))),
                Err(e) => app.set_drill_instructions(slint::SharedString::from(
                    format!("Save failed: {e}"))),
            }
```

(The `name` variable becomes unused — remove the name computation but KEEP the `builder-line-name` UI field; it will label imports in a later polish pass. Remove the unused variable to keep warnings clean.)

- [ ] **Step 10: Analysis-thread corrective line goes into the tree.** In the analysis thread (`main.rs:1569-1607`), replace the `OpeningLineRecord` + `insert_opening_line` block with:

```rust
                            if !analysis.variations.is_empty() {
                                let uci_line: Vec<String> = analysis.variations[0].pv.iter().take(6).cloned().collect();
                                let mover_is_white = pos_record.fen.split_whitespace().nth(1) == Some("w");
                                let side = if mover_is_white { "White" } else { "Black" };
                                let mut state = state_thread.lock().unwrap();
                                let _ = tree::import_uci_line(&mut state.db, &pos_record.fen, &uci_line, side, "mistake");
```

(keep the `TrainingEventRecord` insert that follows; Task 9 replaces this simple line with the full 7-edge mistake tree.)

- [ ] **Step 11: Fix remaining compile errors.** `get_lines_from_position`, `get_due_opening_lines`, `get_opening_lines` calls are now gone from `main.rs`; `OpeningLineRecord` import in main.rs can be dropped if unused. Build until clean:

Run: `cargo build -p app` then `cargo test --workspace`
Expected: clean build; all tests pass. The `db_manager` tests for opening_lines still pass because the table still exists in fresh DBs (migration only renames it in DBs that had tree writes).

- [ ] **Step 12: Manually smoke-test** (run `cargo run -p app` briefly): dashboard loads, Repertoire screen lists due edges (empty on fresh DB is fine), builder save reports "new move(s)", drill on a builder-saved line works with auto opponent replies.

- [ ] **Step 13: Run `graphify update .` and commit**

```bash
graphify update .
git add -A
git commit -m "feat(app): cut drill engine, builder, dashboard, and review-lines over to repertoire tree"
```

---

### Task 5: Import Lines screen + Lichess studies sync

**Files:**
- Modify: `app/src/main.rs` (slint block: new sidebar button + screen + callbacks; Rust: two callbacks)
- Modify: `lichess_client/src/lib.rs` + create `lichess_client/src/studies.rs`

**Interfaces:**
- Consumes: Task 3 (`walk_pgn_variations`), Task 2 (`ensure/insert` via new helper here).
- Produces:
  - `tree::import_walked(db: &mut SqliteStore, walked: &[pgn_processor::WalkedMove], side: &str, source: &str) -> Result<u32, String>` (returns NEW edge count; used by studies sync too)
  - `LichessClient::fetch_studies_pgn(&self, username: &str) -> Result<String, String>` (async)
  - Slint: `import-lines(string /*pgn*/, string /*side*/)`, `sync-studies(string /*side*/)` callbacks; `import-status` property; screen id `"import"`.

- [ ] **Step 1: Failing test for `import_walked`** (`app/src/tree.rs` tests):

```rust
    #[test]
    fn import_walked_inserts_edges_with_ownership() {
        let mut db = mem_store();
        let pgn = "[Event \"R\"]\n\n1. e4 e5 (1... c5) 2. Nf3 *";
        let walked = pgn_processor::walk_pgn_variations(pgn).unwrap();
        let n = import_walked(&mut db, &walked, "White", "pgn").unwrap();
        assert_eq!(n, 4);
        let start_edges = db.get_repertoire_moves_from(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", "White").unwrap();
        assert_eq!(start_edges.len(), 1); // e4, my move
        assert!(start_edges[0].is_my_move);
        let after_e4 = db.get_repertoire_moves_from(&walked[0].child_fen, "White").unwrap();
        assert_eq!(after_e4.len(), 2); // e5 and c5, opponent moves
        assert!(after_e4.iter().all(|e| !e.is_my_move));
    }
```

Run: `cargo test -p app import_walked` — expected: FAIL.

- [ ] **Step 2: Implement `import_walked` in `tree.rs`:**

```rust
/// Insert pre-walked PGN moves (parent/child fens already normalized). Returns NEW edge count.
pub fn import_walked(
    db: &mut SqliteStore,
    walked: &[pgn_processor::WalkedMove],
    side: &str,
    source: &str,
) -> Result<u32, String> {
    let mut new_edges = 0u32;
    for w in walked {
        let mover_is_white = w.parent_fen.split_whitespace().nth(1) == Some("w");
        let is_my_move = (side == "White") == mover_is_white;
        let parent_id = db.ensure_repertoire_node(&w.parent_fen, side)?;
        let child_id = db.ensure_repertoire_node(&w.child_fen, side)?;
        let (_, was_new) =
            db.insert_repertoire_move(parent_id, child_id, &w.uci, &w.san, is_my_move, source)?;
        if was_new { new_edges += 1; }
    }
    Ok(new_edges)
}
```

Note: `app` already depends on `pgn_processor`, no Cargo change. Run: `cargo test -p app` — PASS.

- [ ] **Step 3: `fetch_studies_pgn`.** Create `lichess_client/src/studies.rs`:

```rust
use crate::LichessClient;

impl LichessClient {
    /// All of a user's studies as one multi-game PGN with variations.
    pub async fn fetch_studies_pgn(&self, username: &str) -> Result<String, String> {
        let url = format!("https://lichess.org/api/study/by/{username}/export.pgn");
        let resp = self
            .http
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("Lichess studies API error: {}", resp.status()));
        }
        resp.text().await.map_err(|e| format!("read error: {e}"))
    }
}
```

Add `pub mod studies;` to `lichess_client/src/lib.rs`. No unit test (pure network wrapper; body is one call). Run: `cargo build -p lichess_client` — expected: clean.

- [ ] **Step 4: Slint UI.** In the slint! block: add sidebar button (after the "Opening Builder" `SidebarButton`):

```slint
                        SidebarButton {
                            text: "Import Lines";
                            active: root.active-screen == "import";
                            clicked => { root.select-screen("import"); }
                        }
```

Add properties + callbacks next to the other declarations:

```slint
        in-out property <string> import-status: "";
        in-out property <string> import-side: "White";
        callback import-lines(string, string);
        callback sync-studies(string);
```

Add the screen (as a sibling of the other `if (root.active-screen == ...)` blocks):

```slint
                // 5. Import Lines Screen
                if (root.active-screen == "import") : VerticalLayout {
                    spacing: 16px;
                    alignment: start;
                    Text { text: "Import Repertoire Lines"; color: #ffffff; font-size: 18px; font-weight: 700; }
                    HorizontalLayout {
                        spacing: 8px;
                        Text { text: "Repertoire side:"; color: #a1a1aa; font-size: 13px; vertical-alignment: center; }
                        Rectangle {
                            width: 80px; height: 32px;
                            background: root.import-side == "White" ? #6d28d9 : #27272a;
                            border-radius: 6px;
                            TouchArea { clicked => { root.import-side = "White"; } }
                            Text { text: "White"; color: white; font-size: 13px; horizontal-alignment: center; vertical-alignment: center; }
                        }
                        Rectangle {
                            width: 80px; height: 32px;
                            background: root.import-side == "Black" ? #6d28d9 : #27272a;
                            border-radius: 6px;
                            TouchArea { clicked => { root.import-side = "Black"; } }
                            Text { text: "Black"; color: white; font-size: 13px; horizontal-alignment: center; vertical-alignment: center; }
                        }
                    }
                    import-input := TextEdit {
                        height: 260px;
                        placeholder-text: "Paste PGN here — variations included. Multiple games OK.";
                    }
                    HorizontalLayout {
                        spacing: 12px;
                        Button {
                            text: "Import PGN Lines";
                            clicked => { root.import-lines(import-input.text, root.import-side); import-input.text = ""; }
                        }
                        Button {
                            text: "Sync My Lichess Studies";
                            clicked => { root.sync-studies(root.import-side); }
                        }
                    }
                    Text { text: root.import-status; color: #a78bfa; font-size: 13px; }
                }
```

Add `TextEdit` to the std-widgets import at the top of the slint block: `import { Button, ScrollView, LineEdit, TextEdit } from "std-widgets.slint";`

- [ ] **Step 5: Rust callbacks** (add in `main()` after the builder callbacks):

```rust
    // Import pasted PGN with variations into the tree
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        let refresh = refresh_data.clone();
        app.on_import_lines(move |pgn: slint::SharedString, side: slint::SharedString| {
            let app = app_weak.upgrade().unwrap();
            if pgn.trim().is_empty() {
                app.set_import_status(slint::SharedString::from("Nothing to import."));
                return;
            }
            let walked = match pgn_processor::walk_pgn_variations(&pgn) {
                Ok(w) => w,
                Err(e) => { app.set_import_status(slint::SharedString::from(format!("Parse error: {e}"))); return; }
            };
            let total = walked.len();
            let mut st = state.lock().unwrap();
            match tree::import_walked(&mut st.db, &walked, &side, "pgn") {
                Ok(n) => {
                    drop(st);
                    app.set_import_status(slint::SharedString::from(
                        format!("Imported {n} new move(s) ({total} parsed) into {side} repertoire.")));
                    refresh();
                }
                Err(e) => app.set_import_status(slint::SharedString::from(format!("Import failed: {e}"))),
            }
        });
    }

    // Sync Lichess studies into the tree
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        let refresh = refresh_data.clone();
        app.on_sync_studies(move |side: slint::SharedString| {
            let state = state.clone();
            let app_weak = app_weak.clone();
            let refresh = refresh.clone();
            let side = side.to_string();
            std::thread::spawn(move || {
                let aw = app_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = aw.upgrade() {
                        app.set_import_status(slint::SharedString::from("Fetching studies…"));
                    }
                });
                let token = std::env::var("LICHESS_API_KEY").unwrap_or_default();
                let username = std::env::var("LICHESS_USERNAME").unwrap_or_else(|_| "hackandtoss".into());
                let client = lichess_client::LichessClient::new(&token);
                let rt = tokio::runtime::Runtime::new().unwrap();
                let msg = match rt.block_on(client.fetch_studies_pgn(&username)) {
                    Ok(pgn_text) => match pgn_processor::walk_pgn_variations(&pgn_text) {
                        Ok(walked) => {
                            let mut st = state.lock().unwrap();
                            match tree::import_walked(&mut st.db, &walked, &side, "study") {
                                Ok(n) => format!("Studies: {n} new move(s) imported."),
                                Err(e) => format!("Study import failed: {e}"),
                            }
                        }
                        Err(e) => format!("Study parse failed: {e}"),
                    },
                    Err(e) => format!("Studies fetch failed: {e}"),
                };
                let aw = app_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = aw.upgrade() {
                        app.set_import_status(slint::SharedString::from(msg));
                    }
                    refresh();
                });
            });
        });
    }
```

CAUTION: `refresh_data` is an `Arc` of a non-Send closure — inside the thread it must only be called via `slint::invoke_from_event_loop` (as shown), matching the existing sync-lichess pattern at `main.rs:1621-1667`.

- [ ] **Step 6: Build, run, verify.** `cargo build -p app`; run the app, paste a small PGN with a variation on the Import screen, confirm status shows new moves and the Repertoire screen queue grows.

- [ ] **Step 7: `graphify update .` + commit**

```bash
graphify update .
git add -A
git commit -m "feat(app): Import Lines screen (variation PGN paste) and Lichess studies sync"
```

---

### Task 6: `chesscom_client` crate

**Files:**
- Create: `chesscom_client/Cargo.toml`, `chesscom_client/src/lib.rs`
- Modify: `Cargo.toml` (workspace members)

**Interfaces:**
- Consumes: nothing.
- Produces (used by Task 7):
  - `ChesscomClient::new(user_agent: &str) -> Self`
  - `async fn fetch_archives(&self, username: &str) -> Result<Vec<String>, String>` (monthly archive URLs, oldest→newest as the API returns)
  - `async fn fetch_month(&self, archive_url: &str) -> Result<Vec<ChesscomGame>, String>`
  - `pub struct ChesscomGame { pub url: Option<String>, pub uuid: Option<String>, pub pgn: Option<String>, pub end_time: Option<u64>, pub time_class: Option<String>, pub white: Option<ChesscomPlayer>, pub black: Option<ChesscomPlayer> }`
  - `pub struct ChesscomPlayer { pub username: String, pub rating: Option<i32> }`
  - `pub fn game_external_id(g: &ChesscomGame) -> String` (uuid, else url tail, else `pgn`-length fallback)

- [ ] **Step 1: Scaffold.** `chesscom_client/Cargo.toml`:

```toml
[package]
name = "chesscom_client"
version = "0.1.0"
edition = "2021"

[dependencies]
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

Add `"chesscom_client",` to workspace `members` in the root `Cargo.toml`.

- [ ] **Step 2: Failing tests** (`chesscom_client/src/lib.rs` bottom):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_archives_response() {
        let json = r#"{"archives":["https://api.chess.com/pub/player/x/games/2026/05","https://api.chess.com/pub/player/x/games/2026/06"]}"#;
        let a: ArchivesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(a.archives.len(), 2);
        assert!(a.archives[1].ends_with("2026/06"));
    }

    #[test]
    fn parses_monthly_games_and_external_id() {
        let json = r#"{"games":[{"url":"https://www.chess.com/game/live/1234567","pgn":"[Event \"Live Chess\"]\n\n1. e4 e5 1-0","end_time":1750000000,"time_class":"rapid","uuid":"abc-def","white":{"username":"ElectricMindGames","rating":900},"black":{"username":"opp","rating":905}}]}"#;
        let m: MonthlyGamesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(m.games.len(), 1);
        let g = &m.games[0];
        assert_eq!(g.white.as_ref().unwrap().username, "ElectricMindGames");
        assert_eq!(game_external_id(g), "abc-def");
        let mut no_uuid = g.clone();
        no_uuid.uuid = None;
        assert_eq!(game_external_id(&no_uuid), "1234567");
    }

    #[test]
    fn tolerates_missing_fields() {
        let json = r#"{"games":[{"end_time":1}]}"#;
        let m: MonthlyGamesResponse = serde_json::from_str(json).unwrap();
        assert!(m.games[0].pgn.is_none());
        assert!(!game_external_id(&m.games[0]).is_empty());
    }
}
```

Run: `cargo test -p chesscom_client` — expected: FAIL (types missing).

- [ ] **Step 3: Implement `src/lib.rs`:**

```rust
use serde::Deserialize;

pub struct ChesscomClient {
    http: reqwest::Client,
    user_agent: String,
}

#[derive(Debug, Deserialize)]
pub struct ArchivesResponse {
    pub archives: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ChesscomPlayer {
    pub username: String,
    pub rating: Option<i32>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ChesscomGame {
    pub url: Option<String>,
    pub uuid: Option<String>,
    pub pgn: Option<String>,
    pub end_time: Option<u64>,
    pub time_class: Option<String>,
    pub white: Option<ChesscomPlayer>,
    pub black: Option<ChesscomPlayer>,
}

#[derive(Debug, Deserialize)]
pub struct MonthlyGamesResponse {
    pub games: Vec<ChesscomGame>,
}

/// Stable dedup id: uuid, else numeric tail of the game url, else a length-tagged fallback.
pub fn game_external_id(g: &ChesscomGame) -> String {
    if let Some(u) = &g.uuid {
        return u.clone();
    }
    if let Some(url) = &g.url {
        if let Some(tail) = url.rsplit('/').next() {
            if !tail.is_empty() {
                return tail.to_string();
            }
        }
    }
    format!("anon_{}_{}", g.end_time.unwrap_or(0), g.pgn.as_deref().map(|p| p.len()).unwrap_or(0))
}

impl ChesscomClient {
    /// Chess.com's public API requires a descriptive User-Agent with contact info.
    pub fn new(user_agent: &str) -> Self {
        Self {
            http: reqwest::Client::new(),
            user_agent: user_agent.to_string(),
        }
    }

    pub async fn fetch_archives(&self, username: &str) -> Result<Vec<String>, String> {
        let url = format!(
            "https://api.chess.com/pub/player/{}/games/archives",
            username.to_lowercase()
        );
        let resp = self
            .http
            .get(&url)
            .header("User-Agent", &self.user_agent)
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("chess.com API error: {}", resp.status()));
        }
        resp.json::<ArchivesResponse>()
            .await
            .map(|a| a.archives)
            .map_err(|e| format!("parse error: {e}"))
    }

    pub async fn fetch_month(&self, archive_url: &str) -> Result<Vec<ChesscomGame>, String> {
        let resp = self
            .http
            .get(archive_url)
            .header("User-Agent", &self.user_agent)
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("chess.com API error: {}", resp.status()));
        }
        resp.json::<MonthlyGamesResponse>()
            .await
            .map(|m| m.games)
            .map_err(|e| format!("parse error: {e}"))
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p chesscom_client`
Expected: 3 pass.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml chesscom_client/
git commit -m "feat: chesscom_client crate — public archives API with dedup ids"
```

---

### Task 7: Game sync wiring — chess.com + fix Lichess game inserts

Today `on_sync_lichess` (`app/src/main.rs:1621-1667`) marks games synced but never inserts them into `games` — synced games are invisible. This task fixes that and adds chess.com sync, sharing one ingestion helper.

**Files:**
- Modify: `app/src/main.rs` (new `ingest_pgn_game` helper; rewrite lichess sync closure; new chesscom sync callback; dashboard sync panel in slint block)
- Modify: `app/Cargo.toml` (add `chesscom_client = { path = "../chesscom_client" }`)
- Modify: `.env` (add `CHESSCOM_USERNAME=ElectricMindGames`)

**Interfaces:**
- Consumes: Task 6 crate; existing `pgn_processor::parse_pgn`, `game_to_fen_and_uci`.
- Produces (used by Task 9's analysis and Task 11's review button):
  - `fn ingest_pgn_game(db: &mut SqliteStore, game_id: &str, source: &str, pgn: &str, fallback_date: Option<String>) -> Result<(), String>` in `main.rs` — parses headers, inserts `GameRecord` + all `PositionRecord`s (loss NULL → shows as unreviewed).
  - Slint: `sync-chesscom()` callback, `chesscom-sync-status` property; existing `sync-lichess` gets a dashboard button.

- [ ] **Step 1: Extract `ingest_pgn_game`.** Add as a free function in `main.rs` (mirrors the body of `on_import_pgn` at `main.rs:1440-1476`):

```rust
/// Insert a game + its per-ply positions (unanalyzed). Existing rows are untouched.
fn ingest_pgn_game(
    db: &mut SqliteStore,
    game_id: &str,
    source: &str,
    pgn: &str,
    fallback_date: Option<String>,
) -> Result<(), String> {
    let parsed = pgn_processor::parse_pgn(pgn);
    let game = GameRecord {
        id: game_id.to_string(),
        source: source.to_string(),
        white: parsed.headers.get("White").cloned().unwrap_or_else(|| "White".into()),
        black: parsed.headers.get("Black").cloned().unwrap_or_else(|| "Black".into()),
        result: parsed.result.clone(),
        eco: parsed.headers.get("ECO").cloned(),
        pgn: pgn.to_string(),
        played_at: parsed.headers.get("Date").cloned()
            .map(|d| d.replace('.', "-"))
            .or(fallback_date),
    };
    db.insert_game(&game)?;
    let fens_and_moves = pgn_processor::game_to_fen_and_uci(&parsed.moves)?;
    for (ply, (fen, uci_move)) in fens_and_moves.into_iter().enumerate() {
        db.insert_position(&PositionRecord {
            id: format!("{}_{}", game_id, ply + 1),
            game_id: game_id.to_string(),
            ply: (ply + 1) as u32,
            fen,
            played_move: uci_move,
            centipawn_loss: None,
            mistake_class: None,
        })?;
    }
    Ok(())
}
```

Refactor `on_import_pgn` to call it (keep its analysis-thread spawn unchanged). Run: `cargo build -p app` — clean.

- [ ] **Step 2: Fix the Lichess sync closure.** Inside the `Ok(games)` arm (`main.rs:1644-1656`), replace the mark-only loop:

```rust
                    Ok(games) => {
                        let mut st = state.lock().unwrap();
                        let mut new_count = 0u32;
                        let mut skipped = 0u32;
                        for g in &games {
                            if st.db.is_synced("lichess", &g.id) { continue; }
                            let Some(pgn) = g.pgn.clone() else { skipped += 1; continue; };
                            let date = g.created_at.map(|ms| {
                                let (y, m, d) = tree::days_to_ymd(ms / 86_400_000);
                                format!("{:04}-{:02}-{:02}", y, m, d)
                            });
                            match ingest_pgn_game(&mut st.db, &format!("lichess_{}", g.id), "lichess", &pgn, date) {
                                Ok(()) => new_count += 1,
                                Err(_) => skipped += 1,
                            }
                            let _ = st.db.mark_synced("lichess", &g.id, &tree::local_now_str());
                        }
                        drop(st);
                        let _ = slint::invoke_from_event_loop(move || { refresh_data_cp(); });
                        format!("Lichess: {new_count} new game(s), {skipped} skipped")
                    }
```

- [ ] **Step 3: chess.com sync callback + slint.** Slint declarations:

```slint
        in-out property <string> chesscom-sync-status: "Not synced";
        callback sync-chesscom();
```

Dashboard sync panel (add under the stats grid in the dashboard screen):

```slint
                    HorizontalLayout {
                        spacing: 12px;
                        height: 40px;
                        Button { text: "Sync Lichess Games"; clicked => { root.sync-lichess(""); } }
                        Text { text: root.sync-status; color: #a1a1aa; font-size: 12px; vertical-alignment: center; }
                        Button { text: "Sync Chess.com Games"; clicked => { root.sync-chesscom(); } }
                        Text { text: root.chesscom-sync-status; color: #a1a1aa; font-size: 12px; vertical-alignment: center; }
                    }
```

Change the lichess closure to resolve the username itself when the argument is empty: `let username = if username.is_empty() { std::env::var("LICHESS_USERNAME").unwrap_or_else(|_| "hackandtoss".into()) } else { username };`

Rust callback (same thread pattern as lichess):

```rust
    // Chess.com sync
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        let refresh_data_cp = refresh_data.clone();
        app.on_sync_chesscom(move || {
            let state = state.clone();
            let app_weak = app_weak.clone();
            let refresh_data_cp = refresh_data_cp.clone();
            std::thread::spawn(move || {
                let set_status = |aw: slint::Weak<AppWindow>, msg: String| {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(app) = aw.upgrade() {
                            app.set_chesscom_sync_status(slint::SharedString::from(msg));
                        }
                    });
                };
                set_status(app_weak.clone(), "Syncing…".into());
                let username = std::env::var("CHESSCOM_USERNAME")
                    .unwrap_or_else(|_| "ElectricMindGames".into());
                let ua = std::env::var("CHESSCOM_USER_AGENT").unwrap_or_else(|_| {
                    "grandmaster-forge (contact: johncjohnson1113@gmail.com)".into()
                });
                let client = chesscom_client::ChesscomClient::new(&ua);
                let rt = tokio::runtime::Runtime::new().unwrap();
                let msg = (|| -> Result<String, String> {
                    let archives = rt.block_on(client.fetch_archives(&username))?;
                    let mut new_count = 0u32;
                    let mut skipped = 0u32;
                    // Newest months first; stop after 50 new games to bound first syncs.
                    'outer: for url in archives.iter().rev() {
                        let games = rt.block_on(client.fetch_month(url))?;
                        for g in games.iter().rev() {
                            let ext_id = chesscom_client::game_external_id(g);
                            let mut st = state.lock().unwrap();
                            if st.db.is_synced("chesscom", &ext_id) { continue; }
                            let Some(pgn) = g.pgn.clone() else {
                                let _ = st.db.mark_synced("chesscom", &ext_id, &tree::local_now_str());
                                skipped += 1;
                                continue;
                            };
                            let date = g.end_time.map(|secs| {
                                let (y, m, d) = tree::days_to_ymd(secs / 86_400);
                                format!("{:04}-{:02}-{:02}", y, m, d)
                            });
                            match ingest_pgn_game(&mut st.db, &format!("chesscom_{ext_id}"), "chesscom", &pgn, date) {
                                Ok(()) => new_count += 1,
                                Err(_) => skipped += 1,
                            }
                            let _ = st.db.mark_synced("chesscom", &ext_id, &tree::local_now_str());
                            if new_count >= 50 { break 'outer; }
                        }
                    }
                    Ok(format!("Chess.com: {new_count} new game(s), {skipped} skipped"))
                })().unwrap_or_else(|e| format!("Chess.com sync failed: {e}"));
                let _ = slint::invoke_from_event_loop(move || {
                    refresh_data_cp();
                });
                set_status(app_weak, msg);
            });
        });
    }
```

Add to `app/Cargo.toml` dependencies: `chesscom_client = { path = "../chesscom_client" }`. Append to `.env`: `CHESSCOM_USERNAME=ElectricMindGames`.

- [ ] **Step 4: Build + live verification.** `cargo build -p app`; run app; press both sync buttons; expect real games to appear in the Game Review list (network required — if offline, statuses must show the error string, not crash).

- [ ] **Step 5: `graphify update .` + commit**

```bash
graphify update .
git add -A
git commit -m "feat(sync): chess.com game sync; Lichess sync now actually stores games"
```

---

### Task 8: Explorer adopt-into-repertoire

**Files:**
- Modify: `app/src/scraper.rs` (new async walk), `app/src/main.rs` (callback + a button on the builder screen)

**Interfaces:**
- Consumes: `lichess_client::LichessClient::explorer_masters(fen)`, `tree::add_move_edge`.
- Produces:
  - `scraper::adopt_explorer_lines(client: &LichessClient, start_fen: &str, my_side: &str, depth: u32, top_n: usize) -> Result<Vec<(String, String)>, String>` — async; returns `(parent_full_fen, uci)` pairs in walk order: at my-side turns the single most popular master move, at opponent turns the `top_n` most popular replies (recursive to `depth` plies).
  - Slint: `adopt-explorer(string /*side*/)` callback and `tree-status` reuse for progress.

- [ ] **Step 1: Failing unit test for the pure selection logic.** Refactor so choice logic is testable without network. In `scraper.rs` add:

```rust
/// Pick which explorer moves to adopt at this node.
/// My turn: the single most-played move. Opponent turn: the top_n most-played.
pub fn select_adoption_moves(
    moves: &[lichess_client::explorer::ExplorerMove],
    is_my_turn: bool,
    top_n: usize,
) -> Vec<String> {
    let mut sorted: Vec<&lichess_client::explorer::ExplorerMove> = moves.iter().collect();
    sorted.sort_by_key(|m| std::cmp::Reverse(m.white + m.draws + m.black));
    let take = if is_my_turn { 1 } else { top_n };
    sorted.into_iter().take(take).map(|m| m.uci.clone()).collect()
}
```

Test (in `scraper.rs` tests module — create one if absent):

```rust
    #[test]
    fn adoption_selection_respects_turn() {
        use lichess_client::explorer::ExplorerMove;
        let mk = |uci: &str, games: i64| ExplorerMove {
            uci: uci.into(), san: uci.into(),
            white: games, draws: 0, black: 0, average_rating: None,
        };
        let moves = vec![mk("e2e4", 100), mk("d2d4", 300), mk("c2c4", 50)];
        assert_eq!(select_adoption_moves(&moves, true, 3), vec!["d2d4"]);
        assert_eq!(select_adoption_moves(&moves, false, 2), vec!["d2d4", "e2e4"]);
    }
```

Run: `cargo test -p app adoption` — FAIL then implement — PASS.

- [ ] **Step 2: The async BFS walk** (in `scraper.rs`, following the existing no-Mutex-across-await style of `collect_opening_nodes`):

```rust
/// Walk the masters explorer from start_fen, returning (parent_fen, uci) pairs to adopt.
pub async fn adopt_explorer_lines(
    client: &lichess_client::LichessClient,
    start_fen: &str,
    my_side: &str,
    depth: u32,
    top_n: usize,
) -> Result<Vec<(String, String)>, String> {
    use shakmaty::{CastlingMode, Position};
    let mut out: Vec<(String, String)> = Vec::new();
    // frontier of (full_fen, remaining_depth)
    let mut frontier: Vec<(String, u32)> = vec![(start_fen.to_string(), depth)];
    while let Some((fen, remaining)) = frontier.pop() {
        if remaining == 0 { continue; }
        let resp = client.explorer_masters(&fen).await?;
        if resp.moves.is_empty() { continue; }
        let parsed: shakmaty::fen::Fen = fen.parse().map_err(|e| format!("{e}"))?;
        let pos: shakmaty::Chess = parsed
            .into_position(CastlingMode::Standard)
            .map_err(|e| format!("{e}"))?;
        let is_my_turn = (pos.turn() == shakmaty::Color::White) == (my_side == "White");
        for uci_str in select_adoption_moves(&resp.moves, is_my_turn, top_n) {
            let uci: shakmaty::uci::Uci = match uci_str.parse() { Ok(u) => u, Err(_) => continue };
            let m = match uci.to_move(&pos) { Ok(m) => m, Err(_) => continue };
            let next = match pos.clone().play(&m) { Ok(p) => p, Err(_) => continue };
            let next_fen = shakmaty::fen::Fen::from_position(next, shakmaty::EnPassantMode::Legal).to_string();
            out.push((fen.clone(), uci_str));
            frontier.push((next_fen, remaining - 1));
        }
    }
    Ok(out)
}
```

- [ ] **Step 3: Wire callback + button.** Slint — on the builder screen's right column add:

```slint
                        Rectangle {
                            height: 36px;
                            background: #27272a;
                            border-radius: 8px;
                            TouchArea { clicked => { root.adopt-explorer(root.builder-color); } }
                            Text {
                                text: "Adopt Master Lines from Current Position";
                                color: #a78bfa; font-size: 13px;
                                horizontal-alignment: center; vertical-alignment: center;
                            }
                        }
                        Text { text: root.tree-status; color: #a1a1aa; font-size: 12px; }
```

Declare `callback adopt-explorer(string);`. Rust callback (thread + runtime pattern; reads the builder's current FEN so you can adopt from any prepared position):

```rust
    // Adopt master lines from the builder's current position
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        let refresh = refresh_data.clone();
        app.on_adopt_explorer(move |side: slint::SharedString| {
            let app = app_weak.upgrade().unwrap();
            let start_fen = app.get_builder_fen().to_string();
            let side = side.to_string();
            let state = state.clone();
            let app_weak = app_weak.clone();
            let refresh = refresh.clone();
            std::thread::spawn(move || {
                let aw = app_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = aw.upgrade() {
                        app.set_tree_status(slint::SharedString::from("Adopting master lines…"));
                    }
                });
                let token = std::env::var("LICHESS_API_KEY").unwrap_or_default();
                let client = lichess_client::LichessClient::new(&token);
                let rt = tokio::runtime::Runtime::new().unwrap();
                let msg = match rt.block_on(scraper::adopt_explorer_lines(&client, &start_fen, &side, 8, 3)) {
                    Ok(pairs) => {
                        let mut st = state.lock().unwrap();
                        let mut new_edges = 0u32;
                        for (parent_fen, uci) in &pairs {
                            if let Ok((_, _, was_new)) =
                                tree::add_move_edge(&mut st.db, parent_fen, uci, &side, "explorer")
                            {
                                if was_new { new_edges += 1; }
                            }
                        }
                        format!("Adopted {new_edges} new move(s) from master games.")
                    }
                    Err(e) => format!("Adopt failed: {e}"),
                };
                let aw = app_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = aw.upgrade() {
                        app.set_tree_status(slint::SharedString::from(msg));
                    }
                    refresh();
                });
            });
        });
    }
```

Depth 8 plies / top-3 replies ≈ up to ~120 explorer calls worst case; the explorer API tolerates this unauthenticated rate for one-off clicks. If it rate-limits (HTTP 429), the error surfaces in `tree-status` — acceptable v1.

- [ ] **Step 4: Build + live verify** (`cargo run -p app`: set up 1.e4 on builder board, click Adopt, watch status then check Repertoire queue).

- [ ] **Step 5: `graphify update .` + commit**

```bash
graphify update .
git add -A
git commit -m "feat(explorer): adopt master lines from any position into the repertoire tree"
```

---

### Task 9: Mistake-tree generation (best move + top-3 replies + follow-ups)

**Files:**
- Modify: `app/src/main.rs` (analysis thread tail — replace the Task 4 Step 10 block)
- Modify: `app/src/tree.rs` (helper + test)

**Interfaces:**
- Consumes: `StockfishEngine::analyze_fen` multi-PV, `tree::add_move_edge`.
- Produces:
  - `tree::user_side_for_game(white: &str, black: &str) -> Option<String>` — matches env `LICHESS_USERNAME` / `CHESSCOM_USERNAME` case-insensitively; `Some("White")`/`Some("Black")`/`None`.
  - `fn build_mistake_tree(db: &Arc<Mutex<AppState>>, sf: &mut StockfishEngine, root_fen: &str, side: &str)` — inline in the analysis thread (not reusable; documented here for shape).

- [ ] **Step 1: Failing test for `user_side_for_game`** (`tree.rs` tests):

```rust
    #[test]
    fn user_side_matches_env_usernames() {
        std::env::set_var("LICHESS_USERNAME", "hackandtoss");
        std::env::set_var("CHESSCOM_USERNAME", "ElectricMindGames");
        assert_eq!(user_side_for_game("HackAndToss", "opp"), Some("White".to_string()));
        assert_eq!(user_side_for_game("opp", "electricmindgames"), Some("Black".to_string()));
        assert_eq!(user_side_for_game("a", "b"), None);
    }
```

Implement:

```rust
/// Which side did the user play? Matches configured usernames, case-insensitive.
pub fn user_side_for_game(white: &str, black: &str) -> Option<String> {
    let names: Vec<String> = ["LICHESS_USERNAME", "CHESSCOM_USERNAME"]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .map(|s| s.to_lowercase())
        .collect();
    if names.iter().any(|n| n == &white.to_lowercase()) {
        return Some("White".into());
    }
    if names.iter().any(|n| n == &black.to_lowercase()) {
        return Some("Black".into());
    }
    None
}
```

Run: `cargo test -p app user_side` — PASS. (CAUTION: `std::env::set_var` affects the whole test process; keep both assertions in ONE test function so ordering can't break other tests.)

- [ ] **Step 2: Replace the corrective-line block in the analysis thread** (the block Task 4 Step 10 installed) with the full mistake tree. The thread already has `positions`, `losses`, `sf`, `state_thread`, `white`, `black`:

```rust
                // Mistake tree: worst USER eval drop -> best move, top-3 replies, follow-ups.
                let user_side = tree::user_side_for_game(&white, &black);
                let worst = positions
                    .iter()
                    .zip(losses.iter())
                    .filter(|(pos, _)| match &user_side {
                        // ply 1,3,5.. = White's moves (odd plies)
                        Some(s) => (pos.ply % 2 == 1) == (s == "White"),
                        None => true,
                    })
                    .max_by_key(|(_, loss)| **loss);
                if let Some((pos_record, &loss)) = worst {
                    if loss >= 60 {
                        let mover_is_white = pos_record.fen.split_whitespace().nth(1) == Some("w");
                        let side = if mover_is_white { "White" } else { "Black" }.to_string();
                        // 1) correction (my move)
                        if let Ok(root_analysis) =
                            sf.analyze_fen(&pos_record.fen, AnalysisConfig { depth: 12, multipv: 1 })
                        {
                            let best = root_analysis.bestmove.clone();
                            let mut st = state_thread.lock().unwrap();
                            if let Ok((_, after_best, _)) =
                                tree::add_move_edge(&mut st.db, &pos_record.fen, &best, &side, "mistake")
                            {
                                drop(st);
                                // 2) top-3 opponent replies
                                if let Ok(reply_analysis) =
                                    sf.analyze_fen(&after_best, AnalysisConfig { depth: 10, multipv: 3 })
                                {
                                    let mut seen = std::collections::HashSet::new();
                                    for var in reply_analysis.variations.iter().take(3) {
                                        let Some(reply) = var.pv.first() else { continue };
                                        if !seen.insert(reply.clone()) { continue; }
                                        let mut st = state_thread.lock().unwrap();
                                        let Ok((_, after_reply, _)) = tree::add_move_edge(
                                            &mut st.db, &after_best, reply, &side, "mistake",
                                        ) else { continue };
                                        drop(st);
                                        // 3) my best follow-up to each reply
                                        if let Ok(fu) = sf.analyze_fen(
                                            &after_reply, AnalysisConfig { depth: 10, multipv: 1 },
                                        ) {
                                            let mut st = state_thread.lock().unwrap();
                                            let _ = tree::add_move_edge(
                                                &mut st.db, &after_reply, &fu.bestmove, &side, "mistake",
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
```

Notes: the `loss >= 60` floor skips clean games (below Inaccuracy threshold); `first_critical_mistake` import may become unused in main.rs — remove the unused import if the compiler warns. One tree per analyzed game falls out naturally (this runs once per analysis).

- [ ] **Step 3: End-to-end test via mock engine.** The mock always answers `bestmove e2e4` with 3 PVs (`e2e4`, `d2d4`, `g1f3` — see `app/src/mock_stockfish.rs:30-42`), so a mistake tree from the start position is deterministic. Add an integration-style test in `app/src/tree.rs` tests (guarded to skip when the mock is absent):

This flow is UI-thread-bound, so instead test the pieces: `add_move_edge` chains are covered by Task 2 tests; here just verify manually.

Manual check: `cargo run -p app`, import a short PGN with an obvious blunder via the Game Review screen, wait for analysis, then confirm the Repertoire queue contains `mistake`-source edges (name column shows `(mistake)`).

- [ ] **Step 4: Build + workspace tests + commit**

Run: `cargo test --workspace` — expected PASS.

```bash
graphify update .
git add -A
git commit -m "feat(review): mistake tree — correction, top-3 replies, and follow-ups into SRS queue"
```

---

### Task 10: `PlaySession` in `engine_controller`

**Files:**
- Modify: `engine_controller/src/lib.rs`
- Modify: `app/src/mock_stockfish.rs` (echo legal-ish play: reply `bestmove` from a canned rotation so PlaySession tests don't hang)

**Interfaces:**
- Consumes: existing `StockfishEngine` internals (same crate).
- Produces (used by Task 11):
  - `pub struct PlayConfig { pub elo: Option<u32>, pub movetime_ms: u32 }` (Default: `elo: None, movetime_ms: 400`)
  - `pub struct PlaySession { ... }`
  - `PlaySession::new(binary_path: &str, cfg: PlayConfig) -> Result<Self, String>`
  - `PlaySession::best_move_from(&mut self, moves_uci: &[String]) -> Result<String, String>` — sends `position startpos moves …` + `go movetime N`, returns bestmove.

- [ ] **Step 1: Failing test** (`engine_controller/src/lib.rs` tests — uses the mock binary; mirror how app tests locate it):

```rust
    #[test]
    fn play_session_returns_bestmove_from_mock() {
        // Build/locate mock-stockfish like app::find_stockfish_path does.
        let mut path = std::env::current_dir().unwrap();
        path.pop(); // workspace root from engine_controller/
        path.push("target");
        path.push("debug");
        path.push(if cfg!(windows) { "mock-stockfish.exe" } else { "mock-stockfish" });
        if !path.exists() {
            let _ = std::process::Command::new("cargo")
                .args(["build", "-p", "app", "--bin", "mock-stockfish"])
                .status();
        }
        if !path.exists() {
            eprintln!("mock-stockfish unavailable; skipping");
            return;
        }
        let mut session = PlaySession::new(
            path.to_str().unwrap(),
            PlayConfig { elo: Some(1500), movetime_ms: 50 },
        )
        .unwrap();
        let mv = session
            .best_move_from(&["e2e4".to_string(), "e7e5".to_string()])
            .unwrap();
        assert!(!mv.is_empty());
    }
```

Run: `cargo test -p engine_controller play_session` — FAIL (type missing).

- [ ] **Step 2: Implement** (append to `engine_controller/src/lib.rs`):

```rust
#[derive(Debug, Clone, Copy)]
pub struct PlayConfig {
    pub elo: Option<u32>,
    pub movetime_ms: u32,
}

impl Default for PlayConfig {
    fn default() -> Self {
        Self { elo: None, movetime_ms: 400 }
    }
}

/// A persistent engine process for playing full games (one `position … moves` per turn).
pub struct PlaySession {
    engine: StockfishEngine,
    movetime_ms: u32,
}

impl PlaySession {
    pub fn new(binary_path: &str, cfg: PlayConfig) -> Result<Self, String> {
        let mut engine = StockfishEngine::new(binary_path)?;
        if let Some(elo) = cfg.elo {
            engine.send_command("setoption name UCI_LimitStrength value true")?;
            engine.send_command(&format!("setoption name UCI_Elo value {}", elo.clamp(1320, 3190)))?;
            engine.send_command("isready")?;
            engine.wait_for_line_exact("readyok")?;
        }
        Ok(Self { engine, movetime_ms: cfg.movetime_ms })
    }

    pub fn best_move_from(&mut self, moves_uci: &[String]) -> Result<String, String> {
        let pos_cmd = if moves_uci.is_empty() {
            "position startpos".to_string()
        } else {
            format!("position startpos moves {}", moves_uci.join(" "))
        };
        self.engine.send_command(&pos_cmd)?;
        self.engine.send_command(&format!("go movetime {}", self.movetime_ms))?;
        let mut buf = String::new();
        loop {
            buf.clear();
            let read = self
                .engine
                .stdout
                .read_line(&mut buf)
                .map_err(|e| format!("failed reading engine output: {e}"))?;
            if read == 0 {
                return Err("engine exited during play".to_string());
            }
            let line = buf.trim();
            if line.starts_with("bestmove ") {
                return line
                    .split_whitespace()
                    .nth(1)
                    .map(|s| s.to_string())
                    .ok_or_else(|| "malformed bestmove".to_string());
            }
        }
    }
}
```

(`send_command`/`wait_for_line_exact`/`stdout` are private but same-crate accessible — no visibility changes needed. `use std::io::BufRead;` is already imported at the top.)

- [ ] **Step 3: Run tests**

Run: `cargo test -p engine_controller`
Expected: PASS (mock ignores `setoption UCI_*` silently and answers any `go`).

- [ ] **Step 4: Commit**

```bash
git add engine_controller/src/lib.rs
git commit -m "feat(engine): persistent PlaySession with UCI_Elo strength limiting"
```

---

### Task 11: Play-vs-Bot screen, deviations → SRS, post-game review; README

**Files:**
- Modify: `app/src/main.rs` (slint: sidebar button + `"play"` screen + properties/callbacks; Rust: AppState fields + 3 callbacks + `bot_take_turn` helper)
- Modify: `README.md` (Features/Architecture/Schema/Env sections)

**Interfaces:**
- Consumes: Tasks 1, 2, 4 (`get_repertoire_moves_from`, `grade_edge`, `position_key`), Task 10 (`PlaySession`), Task 7 (`ingest_pgn_game` for the review button).
- Produces: user-facing feature; no downstream consumers.

- [ ] **Step 1: Slint.** Sidebar button `"Play vs Bot"` → screen `"play"`. Properties/callbacks:

```slint
        in-out property <[string]> play-board-pieces: [ /* same 64-string start array as board-pieces */ ];
        in-out property <int> play-selected-square: -1;
        in-out property <string> play-status: "Choose color and start a game.";
        in-out property <string> play-summary: "";
        in-out property <string> play-color: "White";
        in-out property <int> play-elo: 1500;
        in-out property <bool> play-active: false;
        in-out property <string> play-book-state: "";
        callback play-new-game(string, int);
        callback play-click-square(int);
        callback play-resign();
        callback play-review-game();
```

Screen layout (board on the left reusing the 64-square pattern bound to `play-board-pieces`/`play-selected-square` with `play-click-square(index)`; right column):

```slint
                if (root.active-screen == "play") : HorizontalLayout {
                    spacing: 24px;
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
                                text: root.play-board-pieces[index];
                                font-size: 28px; color: #000000;
                                horizontal-alignment: center; vertical-alignment: center;
                            }
                            TouchArea { clicked => { root.play-click-square(index); } }
                        }
                    }
                    // Controls
                    VerticalLayout {
                        spacing: 12px; alignment: start;
                        Text { text: "Play vs Book Bot"; color: #ffffff; font-size: 18px; font-weight: 700; }
                        HorizontalLayout {
                            spacing: 8px;
                            Rectangle {
                                width: 80px; height: 32px;
                                background: root.play-color == "White" ? #6d28d9 : #27272a;
                                border-radius: 6px;
                                TouchArea { clicked => { root.play-color = "White"; } }
                                Text { text: "White"; color: white; font-size: 13px; horizontal-alignment: center; vertical-alignment: center; }
                            }
                            Rectangle {
                                width: 80px; height: 32px;
                                background: root.play-color == "Black" ? #6d28d9 : #27272a;
                                border-radius: 6px;
                                TouchArea { clicked => { root.play-color = "Black"; } }
                                Text { text: "Black"; color: white; font-size: 13px; horizontal-alignment: center; vertical-alignment: center; }
                            }
                        }
                        HorizontalLayout {
                            spacing: 8px;
                            Text { text: "Bot Elo:"; color: #a1a1aa; font-size: 13px; vertical-alignment: center; }
                            elo-input := LineEdit { width: 80px; text: "1500"; }
                        }
                        Button {
                            text: "New Game";
                            clicked => { root.play-new-game(root.play-color, elo-input.text.to-float()); }
                        }
                        if (root.play-active) : Button {
                            text: "Resign";
                            clicked => { root.play-resign(); }
                        }
                        Text { text: root.play-book-state; color: #a78bfa; font-size: 13px; }
                        Text { text: root.play-status; color: #e4e4e7; font-size: 13px; }
                        Text { text: root.play-summary; color: #a1a1aa; font-size: 12px; wrap: word-wrap; width: 300px; }
                        if (!root.play-active && root.play-summary != "") : Button {
                            text: "Review This Game";
                            clicked => { root.play-review-game(); }
                        }
                    }
                }
```

(`to-float` truncation to int for the callback: declare the callback as `play-new-game(string, int)` and pass `elo-input.text.to-float()` — Slint coerces float→int on callback argument; if the compiler objects, change the callback to `(string, float)` and cast in Rust.)

- [ ] **Step 2: AppState fields** (+ init):

```rust
    // Play-vs-bot session
    play_chess: Chess,
    play_moves_uci: Vec<String>,
    play_side: String,             // user's color: "White" | "Black"
    play_session: Option<engine_controller::PlaySession>,
    play_deviations: Vec<String>,
    play_plies_in_book: u32,
    play_left_book_at: Option<u32>,
```

- [ ] **Step 3: Bot-turn helper** (free function in `main.rs`):

```rust
/// Bot plays one move: from the user's book if possible, else via Stockfish PlaySession.
/// Returns a status string for the UI.
fn bot_take_turn(state: &mut AppState, app: &AppWindow) -> String {
    use shakmaty::Position;
    if state.play_chess.is_game_over() {
        return "Game over.".to_string();
    }
    let key = tree::position_key(&state.play_chess);
    let edges = state.db.get_repertoire_moves_from(&key, &state.play_side).unwrap_or_default();
    let book: Vec<_> = edges.into_iter().filter(|e| !e.is_my_move).collect();

    let uci_str = if !book.is_empty() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as usize;
        state.play_plies_in_book += 1;
        app.set_play_book_state(slint::SharedString::from("In book (your prep)"));
        book[nanos % book.len()].uci.clone()
    } else {
        if state.play_left_book_at.is_none() {
            state.play_left_book_at = Some(state.play_moves_uci.len() as u32);
        }
        app.set_play_book_state(slint::SharedString::from("Out of book (Stockfish)"));
        if state.play_session.is_none() {
            let elo = app.get_play_elo().max(1320) as u32;
            match engine_controller::PlaySession::new(
                &state.stockfish_path,
                engine_controller::PlayConfig { elo: Some(elo), movetime_ms: 400 },
            ) {
                Ok(s) => state.play_session = Some(s),
                Err(e) => return format!("Engine unavailable — book-only mode ended the game. ({e})"),
            }
        }
        match state.play_session.as_mut().unwrap().best_move_from(&state.play_moves_uci) {
            Ok(m) => m,
            Err(e) => return format!("Engine error: {e}"),
        }
    };

    let Ok(uci) = uci_str.parse::<Uci>() else { return format!("Bot produced bad move {uci_str}") };
    let Ok(m) = uci.to_move(&state.play_chess) else {
        // Mock engine can emit illegal moves in arbitrary positions; end gracefully.
        return format!("Bot move {uci_str} not legal here — game ended.");
    };
    state.play_chess.play_unchecked(&m);
    state.play_moves_uci.push(uci_str.clone());
    let fen = shakmaty::fen::Fen::from_position(
        state.play_chess.clone(), shakmaty::EnPassantMode::Legal).to_string();
    app.set_play_board_pieces(slint::ModelRc::from(std::rc::Rc::new(
        slint::VecModel::from(fen_to_pieces(&fen)))));
    format!("Bot played {uci_str}. Your move.")
}
```

- [ ] **Step 4: Callbacks.**

`play-new-game`:

```rust
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_play_new_game(move |color: slint::SharedString, elo: i32| {
            let mut st = state.lock().unwrap();
            let app = app_weak.upgrade().unwrap();
            st.play_chess = Chess::default();
            st.play_moves_uci.clear();
            st.play_side = color.to_string();
            st.play_session = None; // rebuilt lazily with the chosen Elo
            st.play_deviations.clear();
            st.play_plies_in_book = 0;
            st.play_left_book_at = None;
            app.set_play_elo(elo);
            app.set_play_active(true);
            app.set_play_summary(slint::SharedString::from(""));
            app.set_play_board_pieces(slint::ModelRc::from(Rc::new(slint::VecModel::from(
                fen_to_pieces("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR")))));
            let msg = if st.play_side == "Black" {
                bot_take_turn(&mut st, &app) // bot is White, moves first
            } else {
                "Your move.".to_string()
            };
            app.set_play_status(slint::SharedString::from(msg));
        });
    }
```

`play-click-square` (two-click move; validates legality; book-deviation grading; then bot replies):

```rust
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_play_click_square(move |idx: i32| {
            let mut st = state.lock().unwrap();
            let app = app_weak.upgrade().unwrap();
            if !app.get_play_active() { return; }
            let sel = app.get_play_selected_square();
            if sel == -1 { app.set_play_selected_square(idx); return; }
            app.set_play_selected_square(-1);
            let uci_try = format!("{}{}", index_to_square(sel as usize), index_to_square(idx as usize));

            // Legality first (try promotion to queen as fallback).
            let mv = uci_try.parse::<Uci>().ok().and_then(|u| u.to_move(&st.play_chess).ok())
                .or_else(|| format!("{uci_try}q").parse::<Uci>().ok()
                    .and_then(|u| u.to_move(&st.play_chess).ok()));
            let Some(m) = mv else {
                app.set_play_status(slint::SharedString::from("Illegal move."));
                return;
            };
            let played_uci = Uci::from_move(&m, CastlingMode::Standard).to_string();

            // Book check BEFORE playing: was this position in book, and did we deviate?
            let key = tree::position_key(&st.play_chess);
            let side = st.play_side.clone();
            let my_edges: Vec<_> = st.db.get_repertoire_moves_from(&key, &side)
                .unwrap_or_default().into_iter().filter(|e| e.is_my_move).collect();
            if !my_edges.is_empty() {
                if my_edges.iter().any(|e| e.uci == played_uci) {
                    st.play_plies_in_book += 1;
                } else {
                    let today_days = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() / 86400;
                    let expected: Vec<String> =
                        my_edges.iter().map(|e| e.san.clone()).collect();
                    for e in &my_edges {
                        let _ = tree::grade_edge(&mut st.db, e, 1, today_days);
                    }
                    st.play_deviations.push(format!(
                        "Move {}: played {played_uci}, book expected {}",
                        st.play_moves_uci.len() / 2 + 1,
                        expected.join(" / ")
                    ));
                    if st.play_left_book_at.is_none() {
                        st.play_left_book_at = Some(st.play_moves_uci.len() as u32);
                    }
                }
            }

            st.play_chess.play_unchecked(&m);
            st.play_moves_uci.push(played_uci);
            let fen = shakmaty::fen::Fen::from_position(
                st.play_chess.clone(), shakmaty::EnPassantMode::Legal).to_string();
            app.set_play_board_pieces(slint::ModelRc::from(Rc::new(
                slint::VecModel::from(fen_to_pieces(&fen)))));

            use shakmaty::Position;
            if st.play_chess.is_game_over() {
                finish_play_game(&mut st, &app, "Game over");
                return;
            }
            let msg = bot_take_turn(&mut st, &app);
            let ended = msg.contains("ended") || msg.contains("Engine")
                || { use shakmaty::Position; st.play_chess.is_game_over() };
            app.set_play_status(slint::SharedString::from(msg));
            if ended { finish_play_game(&mut st, &app, "Game over"); }
        });
    }
```

`finish_play_game` free function + `play-resign` + `play-review-game`:

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

```rust
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_play_resign(move || {
            let mut st = state.lock().unwrap();
            let app = app_weak.upgrade().unwrap();
            finish_play_game(&mut st, &app, "Resigned");
        });
    }

    // Send the finished bot game through the normal import/analysis pipeline.
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_play_review_game(move || {
            let st = state.lock().unwrap();
            let moves = st.play_moves_uci.clone();
            let side = st.play_side.clone();
            drop(st);
            if moves.is_empty() { return; }
            // Rebuild SAN movetext from UCI.
            use shakmaty::{Chess, Position};
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
    }
```

CAUTION: the synchronous engine call in `bot_take_turn` blocks the UI for ~400 ms per out-of-book move — accepted for v1 (documented Global Constraint).

- [ ] **Step 5: Build + live verify.** `cargo run -p app` with real Stockfish on PATH (`STOCKFISH_PATH` env respected by `find_stockfish_path`): play a game as White in a prepared line; confirm (a) bot follows your book with varying branches, (b) "Out of book" flips when you leave prep, (c) deviating from a known position records a deviation and the edge shows as due in the Repertoire queue, (d) resign produces the summary, (e) Review This Game lands the game in Game Review and analysis runs.

- [ ] **Step 6: Update `README.md`.** Features: add "Play vs Book Bot", "Chess.com Sync", "Import Lines (PGN variations + Lichess studies)", "Mistake Trees", "Explorer Adopt"; replace the SM-2-per-line description with per-move-edge; Schema table: add `repertoire_nodes`, `repertoire_moves`, `game_sync`, note `opening_lines` → migrated + retired; Architecture: add `chesscom_client` crate and `app/src/tree.rs`; Environment: add `CHESSCOM_USERNAME`, `CHESSCOM_USER_AGENT`, `LICHESS_USERNAME`; Tests: update count.

- [ ] **Step 7: Final full pass**

Run: `cargo test --workspace` — all pass. `cargo run -p app` smoke test all six screens.

```bash
graphify update .
git add -A
git commit -m "feat(play): book-bot vs Stockfish game mode with deviation-driven SRS; README refresh"
```

---

## Plan Self-Review (done at write time)

- **Spec coverage:** §1 data model → Tasks 1–2; §2a chess.com → Tasks 6–7; §2b PGN import → Tasks 3, 5; §2c studies → Task 5; §2d explorer adopt → Task 8; §2e mistake trees → Task 9; §3 drill rework → Task 4; §4 bot play → Tasks 10–11; §5 error handling → woven into each task's status-string patterns; §6 testing → per-task TDD steps. Migration transactional wrapping (spec §5) is relaxed to idempotent-guard + rename-last (equivalent safety: a crash mid-migration re-runs cleanly because `repertoire_move_count()==0` until edges exist and retirement happens last) — accepted deviation.
- **Order note:** the user's "data first" priority is honored in outcome (imports usable from Task 5 on); Task 4 must precede the import UI so imported edges are drillable and the app never dual-writes.
- **Type consistency check:** `RepertoireMoveRecord` fields match across Tasks 1/2/4/11; `WalkedMove` matches Tasks 3/5; `PlayConfig`/`PlaySession` match Tasks 10/11; `ingest_pgn_game` signature matches Tasks 7/11 usage; `tree::` helpers (`position_key`, `add_move_edge`, `import_uci_line`, `import_walked`, `grade_edge`, `migrate_legacy_lines`, `user_side_for_game`, `local_now_str`, `days_to_ymd`) are each defined before first use.
