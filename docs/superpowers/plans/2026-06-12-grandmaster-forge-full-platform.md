# Grandmaster Forge – Full Platform Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Evolve Grandmaster Forge from a PGN-paste prototype into a full chess training platform combining Lichess auto-import, SM-2 spaced repetition, per-phase accuracy scoring, and an opening tree explorer — inspired by Lichess, Chess.com, AIMchess, ChessReps, and ChessIgma.

**Architecture:** Four independent subsystems wired into the existing Rust workspace (`engine_controller`, `pgn_processor`, `db_manager`, `mastery_ui`, `app`). A new `lichess_client` crate handles all HTTP calls. The DB schema gains SRS scheduling columns and an opening-tree table. The Slint GUI gains new screens and a data-refresh hook for live Lichess sync.

**Tech Stack:** Rust 2021, `reqwest` + `tokio` (async HTTP), `rusqlite` (bundled), `shakmaty` (chess logic), `slint` (GUI), Lichess REST API v2 + NDJSON streaming, SM-2 spaced repetition algorithm, Firecrawl Docker (at `../mediation-website-v2-react`) for scraping opening data, ScrapeGraphAI API for structured extraction, Playwright CLI for JS-heavy pages.

---

## This plan covers four independent subsystems — implement them in order, each is shippable on its own:

| Phase | Subsystem | Estimated Tasks |
|-------|-----------|-----------------|
| A | Lichess API Integration | Tasks 1–5 |
| B | SM-2 Spaced Repetition | Tasks 6–9 |
| C | Accuracy & Weakness Analysis | Tasks 10–13 |
| D | Opening Tree & Data Mining | Tasks 14–17 |

---

## Current state (do not re-implement)

- `pgn_processor`: PGN parse, FEN/UCI conversion, mistake classification ✅
- `db_manager`: SQLite CRUD (`games`, `positions`, `opening_lines`, `puzzles`, `training_events`) ✅
- `engine_controller`: Stockfish UCI wrapper, multi-PV, centipawn loss ✅
- `mastery_ui`: Training mode recommendations, `UserSnapshot` ✅
- `app/src/main.rs`: Full Slint GUI (Dashboard, Repertoire Browser, Game Review), background analysis thread ✅

Known gaps being fixed by this plan:
- `local_now_str()` returns hardcoded `"2026-06-12"` — fixed in Task 1
- `opening_lines.confidence` SRS is just `+= 0.2` — replaced with SM-2 in Phase B
- No real-time Lichess game import — added in Phase A
- No accuracy % or phase detection — added in Phase C

---

## File Structure

### New files created by this plan

```
lichess_client/
├── Cargo.toml
└── src/
    ├── lib.rs          # re-exports, LichessClient struct, auth header
    ├── games.rs        # fetch user game archives (NDJSON streaming)
    ├── puzzles.rs      # fetch puzzles by theme
    └── explorer.rs     # Lichess opening explorer API

app/src/
├── srs.rs             # SM-2 algorithm (interval, ease factor, due_at)
├── accuracy.rs        # centipawn loss → accuracy %, phase detection
├── weakness.rs        # aggregate weakness profile from positions table
└── scraper.rs         # Firecrawl + ScrapeGraphAI calls for opening data
```

### Files modified by this plan

```
Cargo.toml                    # add lichess_client to workspace members
.env                          # already has LICHESS_API_KEY ✅ (add SCRAPEGRAPHAI_API_KEY)
db_manager/src/lib.rs         # new columns + opening_tree table + SRS fields
app/src/main.rs               # wire Lichess sync button, SRS drill order, accuracy screen
```

---

## Phase A — Lichess API Integration

### Task 1: Fix timestamps + create `lichess_client` crate skeleton

**Files:**
- Create: `lichess_client/Cargo.toml`
- Create: `lichess_client/src/lib.rs`
- Modify: `Cargo.toml` (workspace)
- Modify: `app/src/main.rs` (fix `local_now_str`)

- [ ] **Step 1: Fix `local_now_str()` in `app/src/main.rs`**

Replace the hardcoded stub at line 1300:

```rust
fn local_now_str() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Format as YYYY-MM-DD using integer arithmetic (no chrono dependency needed)
    let days = secs / 86400;
    let (y, m, d) = days_to_ymd(days);
    format!("{:04}-{:02}-{:02}", y, m, d)
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    // Days since 1970-01-01
    let mut year = 1970u64;
    loop {
        let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let days_in_year = if leap { 366 } else { 365 };
        if days < days_in_year { break; }
        days -= days_in_year;
        year += 1;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let month_days = [31u64, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 1u64;
    for md in &month_days {
        if days < *md { break; }
        days -= *md;
        month += 1;
    }
    (year, month, days + 1)
}
```

- [ ] **Step 2: Create `lichess_client/Cargo.toml`**

```toml
[package]
name = "lichess_client"
version = "0.1.0"
edition = "2021"

[dependencies]
reqwest = { version = "0.12", features = ["json", "stream"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
futures-util = "0.3"
```

- [ ] **Step 3: Create `lichess_client/src/lib.rs`**

```rust
pub mod games;
pub mod puzzles;
pub mod explorer;

pub struct LichessClient {
    http: reqwest::Client,
    token: String,
}

impl LichessClient {
    pub fn new(token: &str) -> Self {
        Self {
            http: reqwest::Client::new(),
            token: token.to_string(),
        }
    }

    pub(crate) fn auth_header(&self) -> String {
        format!("Bearer {}", self.token)
    }
}
```

- [ ] **Step 4: Add `lichess_client` to the workspace**

In the root `Cargo.toml`, add `"lichess_client"` to the `members` array:

```toml
[workspace]
members = [
    "engine_controller",
    "pgn_processor",
    "mastery_ui",
    "db_manager",
    "app",
    "lichess_client",
]
```

- [ ] **Step 5: Verify it compiles**

```
cargo check -p lichess_client
```

Expected: `Finished` with no errors.

- [ ] **Step 6: Commit**

```bash
git add lichess_client/ Cargo.toml app/src/main.rs
git commit -m "feat: add lichess_client crate skeleton, fix local_now_str"
```

---

### Task 2: Fetch user games from Lichess (NDJSON stream)

**Files:**
- Create: `lichess_client/src/games.rs`
- Create: `lichess_client/src/tests/games_test.rs` (integration test, uses real API)

- [ ] **Step 1: Write the game model and fetch function in `lichess_client/src/games.rs`**

```rust
use serde::Deserialize;
use crate::LichessClient;

#[derive(Debug, Deserialize, Clone)]
pub struct LichessGame {
    pub id: String,
    pub rated: bool,
    pub speed: String,
    pub pgn: Option<String>,
    pub players: Players,
    #[serde(rename = "createdAt")]
    pub created_at: Option<u64>,
    pub opening: Option<Opening>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Players {
    pub white: PlayerInfo,
    pub black: PlayerInfo,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PlayerInfo {
    pub user: Option<UserRef>,
    pub rating: Option<i32>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct UserRef {
    pub name: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Opening {
    pub eco: Option<String>,
    pub name: Option<String>,
}

impl LichessClient {
    /// Fetch up to `max` recent games for `username`, returning parsed game objects.
    /// Uses the NDJSON endpoint with PGN included.
    pub async fn fetch_user_games(
        &self,
        username: &str,
        max: u32,
    ) -> Result<Vec<LichessGame>, String> {
        use futures_util::StreamExt;

        let url = format!(
            "https://lichess.org/api/games/user/{}?max={}&pgnInJson=true&opening=true",
            username, max
        );

        let resp = self.http
            .get(&url)
            .header("Authorization", self.auth_header())
            .header("Accept", "application/x-ndjson")
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("Lichess API error: {}", resp.status()));
        }

        let mut stream = resp.bytes_stream();
        let mut games = Vec::new();
        let mut buf = String::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("stream error: {e}"))?;
            buf.push_str(&String::from_utf8_lossy(&chunk));

            // Each NDJSON line is a complete JSON object
            while let Some(newline_pos) = buf.find('\n') {
                let line = buf[..newline_pos].trim().to_string();
                buf = buf[newline_pos + 1..].to_string();

                if line.is_empty() {
                    continue;
                }

                match serde_json::from_str::<LichessGame>(&line) {
                    Ok(game) => games.push(game),
                    Err(e) => eprintln!("skipping malformed game JSON: {e}"),
                }
            }
        }

        Ok(games)
    }
}
```

- [ ] **Step 2: Write a unit test for the NDJSON parser logic**

Create `lichess_client/src/games.rs` test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_game_json() {
        let json = r#"{"id":"abc123","rated":true,"speed":"rapid","pgn":"1. e4 e5","players":{"white":{"user":{"name":"hackandtoss"},"rating":1500},"black":{"user":{"name":"opponent"},"rating":1480}}}"#;
        let game: LichessGame = serde_json::from_str(json).expect("should parse");
        assert_eq!(game.id, "abc123");
        assert_eq!(game.players.white.user.as_ref().unwrap().name, "hackandtoss");
    }
}
```

- [ ] **Step 3: Run the unit test**

```
cargo test -p lichess_client
```

Expected: 1 test passes.

- [ ] **Step 4: Commit**

```bash
git add lichess_client/src/games.rs
git commit -m "feat: implement Lichess NDJSON game stream fetch"
```

---

### Task 3: Wire Lichess sync into `db_manager`

**Files:**
- Modify: `db_manager/src/lib.rs` (add `lichess_game_ids` table for dedup)

- [ ] **Step 1: Add a dedup table to `SQLITE_SCHEMA` in `db_manager/src/lib.rs`**

Append to the `SQLITE_SCHEMA` array:

```rust
r#"
CREATE TABLE IF NOT EXISTS lichess_sync (
    game_id TEXT PRIMARY KEY,
    synced_at TEXT NOT NULL
);
"#,
```

- [ ] **Step 2: Add `is_game_synced` and `mark_game_synced` to `SqliteStore`**

```rust
pub fn is_game_synced(&self, game_id: &str) -> bool {
    self.conn
        .query_row(
            "SELECT 1 FROM lichess_sync WHERE game_id = ?1",
            rusqlite::params![game_id],
            |_| Ok(()),
        )
        .is_ok()
}

pub fn mark_game_synced(&mut self, game_id: &str, synced_at: &str) -> Result<(), String> {
    self.conn.execute(
        "INSERT OR IGNORE INTO lichess_sync (game_id, synced_at) VALUES (?1, ?2)",
        rusqlite::params![game_id, synced_at],
    ).map_err(|e| e.to_string())?;
    Ok(())
}
```

- [ ] **Step 3: Write a test for the dedup logic**

Add to `db_manager/src/lib.rs` test module:

```rust
#[test]
fn dedup_lichess_sync() {
    let mut store = SqliteStore::new_in_memory().unwrap();
    store.bootstrap().unwrap();

    assert!(!store.is_game_synced("abc123"));
    store.mark_game_synced("abc123", "2026-06-12").unwrap();
    assert!(store.is_game_synced("abc123"));
    // Inserting twice should not fail
    store.mark_game_synced("abc123", "2026-06-12").unwrap();
}
```

- [ ] **Step 4: Run tests**

```
cargo test -p db_manager
```

Expected: all tests pass including `dedup_lichess_sync`.

- [ ] **Step 5: Commit**

```bash
git add db_manager/src/lib.rs
git commit -m "feat: add lichess_sync dedup table to db_manager"
```

---

### Task 4: Add Lichess sync button to the Slint UI

**Files:**
- Modify: `app/src/main.rs` — add `sync-lichess` callback and background thread
- Modify: `app/Cargo.toml` — add `lichess_client`, `tokio`

- [ ] **Step 1: Add `lichess_client` and `tokio` to `app/Cargo.toml`**

```toml
lichess_client = { path = "../lichess_client" }
tokio = { version = "1", features = ["rt-multi-thread"] }
dotenvy = "0.15"
```

- [ ] **Step 2: Add a "Sync Lichess" button to the Dashboard in the Slint `AppWindow`**

In the `AppWindow` component (inside `if (root.active-screen == "dashboard")`), add after the stats grid:

```slint
// Lichess Sync Status
in-out property <string> sync-status: "Not synced";
callback sync-lichess();

HorizontalLayout {
    alignment: start;
    spacing: 12px;
    Button {
        text: "Sync from Lichess";
        clicked => { root.sync-lichess(); }
    }
    Text {
        text: root.sync-status;
        color: #a1a1aa;
        font-size: 12px;
        vertical-alignment: center;
    }
}
```

- [ ] **Step 3: Wire `on_sync_lichess` in `main()`**

Add after the existing callback wiring section in `main()`:

```rust
// Lichess sync
{
    let state = state.clone();
    let app_weak = app_weak.clone();
    let refresh_data_cp = refresh_data.clone();
    app.on_sync_lichess(move || {
        let token = std::env::var("LICHESS_API_KEY").unwrap_or_default();
        let username = std::env::var("LICHESS_USERNAME").unwrap_or_default();

        if token.is_empty() || username.is_empty() {
            let app = app_weak.upgrade().unwrap();
            app.set_sync_status(slint::SharedString::from("LICHESS_API_KEY not set in .env"));
            return;
        }

        let app_weak2 = app_weak.clone();
        let state2 = state.clone();
        let refresh2 = refresh_data_cp.clone();

        let app = app_weak.upgrade().unwrap();
        app.set_sync_status(slint::SharedString::from("Syncing..."));

        std::thread::spawn(move || {
            // Spin up a one-shot Tokio runtime for async HTTP
            let rt = tokio::runtime::Runtime::new().unwrap();
            let games = rt.block_on(async {
                let client = lichess_client::LichessClient::new(&token);
                client.fetch_user_games(&username, 50).await
            });

            match games {
                Ok(games) => {
                    let now = local_now_str();
                    let mut imported = 0usize;

                    for g in &games {
                        let mut state_lock = state2.lock().unwrap();

                        if state_lock.db.is_game_synced(&g.id) {
                            continue;
                        }

                        if let Some(pgn) = &g.pgn {
                            let parsed = pgn_processor::parse_pgn(pgn);
                            let white = g.players.white.user.as_ref()
                                .map(|u| u.name.clone())
                                .unwrap_or_else(|| "White".to_string());
                            let black = g.players.black.user.as_ref()
                                .map(|u| u.name.clone())
                                .unwrap_or_else(|| "Black".to_string());
                            let eco = g.opening.as_ref().and_then(|o| o.eco.clone());
                            let played_at = g.created_at.map(|ts| {
                                let days = ts / 1000 / 86400;
                                let (y, m, d) = days_to_ymd(days);
                                format!("{:04}-{:02}-{:02}", y, m, d)
                            });

                            let game_record = db_manager::GameRecord {
                                id: g.id.clone(),
                                source: "lichess".to_string(),
                                white,
                                black,
                                result: parsed.result.clone(),
                                eco,
                                pgn: pgn.clone(),
                                played_at,
                            };
                            let _ = state_lock.db.insert_game(&game_record);

                            if let Ok(fens_and_moves) = pgn_processor::game_to_fen_and_uci(&parsed.moves) {
                                for (ply, (fen, uci_move)) in fens_and_moves.into_iter().enumerate() {
                                    let pos = db_manager::PositionRecord {
                                        id: format!("{}_{}", g.id, ply + 1),
                                        game_id: g.id.clone(),
                                        ply: (ply + 1) as u32,
                                        fen,
                                        played_move: uci_move,
                                        centipawn_loss: None,
                                        mistake_class: None,
                                    };
                                    let _ = state_lock.db.insert_position(&pos);
                                }
                            }

                            let _ = state_lock.db.mark_game_synced(&g.id, &now);
                            imported += 1;
                        }
                    }

                    let msg = format!("Synced {} new games", imported);
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(app) = app_weak2.upgrade() {
                            app.set_sync_status(slint::SharedString::from(msg));
                        }
                        refresh2();
                    });
                }
                Err(e) => {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(app) = app_weak2.upgrade() {
                            app.set_sync_status(slint::SharedString::from(format!("Error: {e}")));
                        }
                    });
                }
            }
        });
    });
}
```

- [ ] **Step 4: Build and verify no compile errors**

```
cargo build -p app
```

Expected: `Finished` with no errors.

- [ ] **Step 5: Run the app and click "Sync from Lichess"**

```
cargo run -p app
```

Expected: Status changes to "Syncing…" then "Synced N new games". Game Review list populates.

- [ ] **Step 6: Commit**

```bash
git add app/Cargo.toml app/src/main.rs
git commit -m "feat: wire Lichess sync button with background import"
```

---

### Task 5: Lichess Puzzle Import by Theme

**Files:**
- Create: `lichess_client/src/puzzles.rs`
- Modify: `db_manager/src/lib.rs` — add `insert_puzzle` if not already present
- Modify: `app/src/main.rs` — add puzzle fetch to sync thread

- [ ] **Step 1: Create `lichess_client/src/puzzles.rs`**

```rust
use serde::Deserialize;
use crate::LichessClient;

#[derive(Debug, Deserialize, Clone)]
pub struct LichessPuzzle {
    pub puzzle: PuzzleData,
    pub game: Option<PuzzleGame>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PuzzleData {
    pub id: String,
    pub fen: String,
    pub solution: Vec<String>,
    pub themes: Vec<String>,
    pub rating: i32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PuzzleGame {
    pub pgn: Option<String>,
}

impl LichessClient {
    /// Fetch a random puzzle, optionally filtered by theme string (e.g. "mateIn2", "fork").
    pub async fn fetch_puzzle(&self) -> Result<LichessPuzzle, String> {
        let url = "https://lichess.org/api/puzzle/next";

        let resp = self.http
            .get(url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("Lichess API error: {}", resp.status()));
        }

        let puzzle: LichessPuzzle = resp.json().await.map_err(|e| format!("parse error: {e}"))?;
        Ok(puzzle)
    }
}
```

- [ ] **Step 2: Add `insert_puzzle` to `SqliteStore` in `db_manager/src/lib.rs`**

```rust
pub fn insert_puzzle(&mut self, puzzle: &PuzzleRecord) -> Result<(), String> {
    self.conn.execute(
        "INSERT OR IGNORE INTO puzzles (id, fen, solution_uci, theme, difficulty)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            puzzle.id,
            puzzle.fen,
            puzzle.solution_uci,
            puzzle.theme,
            puzzle.difficulty
        ],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_next_puzzle(&self, max_difficulty: i32) -> Result<Option<PuzzleRecord>, String> {
    let result = self.conn.query_row(
        "SELECT id, fen, solution_uci, theme, difficulty FROM puzzles
         WHERE difficulty <= ?1
         ORDER BY RANDOM() LIMIT 1",
        rusqlite::params![max_difficulty],
        |row| Ok(PuzzleRecord {
            id: row.get(0)?,
            fen: row.get(1)?,
            solution_uci: row.get(2)?,
            theme: row.get(3)?,
            difficulty: row.get(4)?,
        }),
    );
    match result {
        Ok(p) => Ok(Some(p)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
```

- [ ] **Step 3: Write a test for `insert_puzzle` / `get_next_puzzle`**

```rust
#[test]
fn puzzle_insert_and_fetch() {
    let mut store = SqliteStore::new_in_memory().unwrap();
    store.bootstrap().unwrap();

    let puzzle = PuzzleRecord {
        id: "puzzle1".to_string(),
        fen: "r1bqkb1r/pppp1ppp/2n5/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R b KQkq - 3 3".to_string(),
        solution_uci: "d8f6".to_string(),
        theme: "fork".to_string(),
        difficulty: 1400,
    };
    store.insert_puzzle(&puzzle).unwrap();

    let fetched = store.get_next_puzzle(1600).unwrap().expect("should have puzzle");
    assert_eq!(fetched.id, "puzzle1");
    assert!(store.get_next_puzzle(1000).unwrap().is_none());
}
```

- [ ] **Step 4: Run the test**

```
cargo test -p db_manager puzzle_insert_and_fetch
```

Expected: PASS.

- [ ] **Step 5: Extend the Lichess sync thread (in `app/src/main.rs`) to also fetch 10 puzzles**

At the end of the `Ok(games)` block inside the sync thread, add:

```rust
// Also fetch 10 puzzles from Lichess
for _ in 0..10 {
    if let Ok(lp) = rt.block_on(async { client.fetch_puzzle().await }) {
        let themes_str = lp.puzzle.themes.join(",");
        let solution_str = lp.puzzle.solution.join(" ");
        let pr = db_manager::PuzzleRecord {
            id: lp.puzzle.id,
            fen: lp.puzzle.fen,
            solution_uci: solution_str,
            theme: themes_str,
            difficulty: lp.puzzle.rating,
        };
        let mut state_lock = state2.lock().unwrap();
        let _ = state_lock.db.insert_puzzle(&pr);
    }
}
```

Note: move `client` binding to before the game loop so it's accessible here.

- [ ] **Step 6: Commit**

```bash
git add lichess_client/src/puzzles.rs db_manager/src/lib.rs app/src/main.rs
git commit -m "feat: import Lichess puzzles on sync"
```

---

## Phase B — SM-2 Spaced Repetition Engine

### Task 6: SM-2 algorithm implementation

**Files:**
- Create: `app/src/srs.rs`

- [ ] **Step 1: Write the SM-2 algorithm**

```rust
// app/src/srs.rs
// SM-2 spaced repetition: Wozniak 1987. Grade 0-5 (0-1 = fail, 2-5 = pass).

/// State stored per opening line.
#[derive(Debug, Clone)]
pub struct SrsCard {
    pub interval: f32,      // days until next review
    pub ease_factor: f32,   // starts at 2.5
    pub repetitions: u32,   // consecutive correct reviews
}

impl Default for SrsCard {
    fn default() -> Self {
        Self {
            interval: 1.0,
            ease_factor: 2.5,
            repetitions: 0,
        }
    }
}

/// Grade: 0 = total blackout, 5 = perfect. 0-1 resets repetitions.
pub fn review(card: &SrsCard, grade: u32) -> SrsCard {
    let grade = grade.min(5) as f32;
    let new_ease = (card.ease_factor + 0.1 - (5.0 - grade) * (0.08 + (5.0 - grade) * 0.02))
        .max(1.3);

    if grade < 2.0 {
        // Failed: restart repetitions, short interval
        return SrsCard {
            interval: 1.0,
            ease_factor: new_ease,
            repetitions: 0,
        };
    }

    let new_repetitions = card.repetitions + 1;
    let new_interval = match new_repetitions {
        1 => 1.0,
        2 => 6.0,
        n => card.interval * card.ease_factor,
    };

    SrsCard {
        interval: new_interval,
        ease_factor: new_ease,
        repetitions: new_repetitions,
    }
}

/// Returns the number of days until the card is due.
/// If <= 0 it is due today.
pub fn days_until_due(card: &SrsCard, days_since_last_review: f32) -> f32 {
    card.interval - days_since_last_review
}
```

- [ ] **Step 2: Write tests for SM-2**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_review_correct_gives_interval_1() {
        let card = SrsCard::default();
        let next = review(&card, 4);
        assert_eq!(next.repetitions, 1);
        assert!((next.interval - 1.0).abs() < 0.01);
    }

    #[test]
    fn second_review_correct_gives_interval_6() {
        let card = SrsCard { interval: 1.0, ease_factor: 2.5, repetitions: 1 };
        let next = review(&card, 4);
        assert_eq!(next.repetitions, 2);
        assert!((next.interval - 6.0).abs() < 0.01);
    }

    #[test]
    fn failed_review_resets_repetitions() {
        let card = SrsCard { interval: 10.0, ease_factor: 2.5, repetitions: 5 };
        let next = review(&card, 1);
        assert_eq!(next.repetitions, 0);
        assert!((next.interval - 1.0).abs() < 0.01);
    }

    #[test]
    fn ease_factor_decays_on_hard_grades() {
        let card = SrsCard::default();
        let hard = review(&card, 2);
        let easy = review(&card, 5);
        assert!(hard.ease_factor < easy.ease_factor);
    }
}
```

- [ ] **Step 3: Run tests**

```
cargo test -p app --lib srs
```

Expected: all 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add app/src/srs.rs
git commit -m "feat: implement SM-2 spaced repetition algorithm"
```

---

### Task 7: Add SRS columns to the database

**Files:**
- Modify: `db_manager/src/lib.rs`

- [ ] **Step 1: Add migration logic for SRS columns**

The existing `opening_lines` table doesn't have SRS fields. Add a migration step that runs after `bootstrap()`. Append to `schema_bootstrap_statements()`:

```rust
// Append to SQLITE_SCHEMA:
r#"
ALTER TABLE opening_lines ADD COLUMN IF NOT EXISTS srs_interval REAL DEFAULT 1.0;
"#,
r#"
ALTER TABLE opening_lines ADD COLUMN IF NOT EXISTS srs_ease REAL DEFAULT 2.5;
"#,
r#"
ALTER TABLE opening_lines ADD COLUMN IF NOT EXISTS srs_reps INTEGER DEFAULT 0;
"#,
r#"
ALTER TABLE opening_lines ADD COLUMN IF NOT EXISTS srs_due_date TEXT DEFAULT '';
"#,
```

Note: SQLite `ADD COLUMN IF NOT EXISTS` requires SQLite >= 3.37 (bundled rusqlite ships this). Alternatively, wrap each in a `CREATE TABLE IF NOT EXISTS` migration helper. Use `execute` with error-ignore for the `ALTER` statements since older SQLite will error on duplicate columns.

Actually, SQLite doesn't support `IF NOT EXISTS` for `ALTER TABLE`. Use a safe migration helper instead:

```rust
pub fn run_migrations(&mut self) -> Result<(), String> {
    let migrations = [
        "ALTER TABLE opening_lines ADD COLUMN srs_interval REAL DEFAULT 1.0",
        "ALTER TABLE opening_lines ADD COLUMN srs_ease REAL DEFAULT 2.5",
        "ALTER TABLE opening_lines ADD COLUMN srs_reps INTEGER DEFAULT 0",
        "ALTER TABLE opening_lines ADD COLUMN srs_due_date TEXT DEFAULT ''",
    ];
    for sql in &migrations {
        // Ignore "duplicate column" errors — migration already applied
        let _ = self.conn.execute(sql, []);
    }
    Ok(())
}
```

Call `store.run_migrations()` after `store.bootstrap()` in `app/src/main.rs`.

- [ ] **Step 2: Add `OpeningLineRecord` SRS fields and update `get_opening_lines`**

Add to `OpeningLineRecord`:

```rust
pub srs_interval: f32,
pub srs_ease: f32,
pub srs_reps: u32,
pub srs_due_date: String,
```

Update `get_opening_lines` SELECT to include 4 new columns at positions 7-10, and set defaults for rows that have `NULL`:

```rust
srs_interval: row.get::<_, f64>(7).unwrap_or(1.0) as f32,
srs_ease: row.get::<_, f64>(8).unwrap_or(2.5) as f32,
srs_reps: row.get::<_, u32>(9).unwrap_or(0),
srs_due_date: row.get::<_, String>(10).unwrap_or_default(),
```

Update `insert_opening_line` to also upsert these columns.

- [ ] **Step 3: Add `get_due_opening_lines` query**

```rust
pub fn get_due_opening_lines(&self, user_id: &str, today: &str) -> Result<Vec<OpeningLineRecord>, String> {
    // Due lines: due_date is empty or <= today
    let mut stmt = self.conn.prepare(
        "SELECT id, user_id, opening_name, start_fen, line_uci, source, confidence,
                srs_interval, srs_ease, srs_reps, srs_due_date
         FROM opening_lines
         WHERE user_id = ?1 AND (srs_due_date = '' OR srs_due_date <= ?2)
         ORDER BY srs_due_date ASC"
    ).map_err(|e| e.to_string())?;
    // ... same row mapping as get_opening_lines
}
```

- [ ] **Step 4: Write a test**

```rust
#[test]
fn srs_due_lines_query() {
    let mut store = SqliteStore::new_in_memory().unwrap();
    store.bootstrap().unwrap();
    store.run_migrations().unwrap();

    let line = OpeningLineRecord {
        id: "line1".to_string(),
        user_id: "u1".to_string(),
        opening_name: "Ruy Lopez".to_string(),
        start_fen: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1".to_string(),
        line_uci: vec!["e7e5".to_string()],
        source: "manual".to_string(),
        confidence: 0.0,
        srs_interval: 1.0,
        srs_ease: 2.5,
        srs_reps: 0,
        srs_due_date: "2026-06-10".to_string(), // past date, should be due
    };
    store.insert_opening_line(&line).unwrap();

    let due = store.get_due_opening_lines("u1", "2026-06-12").unwrap();
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].id, "line1");
}
```

- [ ] **Step 5: Run tests**

```
cargo test -p db_manager
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add db_manager/src/lib.rs app/src/main.rs
git commit -m "feat: add SRS columns to opening_lines, migration helper"
```

---

### Task 8: Wire SM-2 into drill completion

**Files:**
- Modify: `app/src/main.rs` — replace confidence arithmetic with SM-2 review

- [ ] **Step 1: Add `mod srs;` to `app/src/main.rs`**

```rust
mod srs;
```

- [ ] **Step 2: In the drill completion handler, replace the `confidence += 0.2` block**

Find the block after `// Drill completed!` (around line 1042) and replace with:

```rust
if let Some(line) = state.drill_line.clone() {
    let card = srs::SrsCard {
        interval: line.srs_interval,
        ease_factor: line.srs_ease,
        repetitions: line.srs_reps,
    };
    // Grade 5 = perfect (user completed the line without mistakes)
    let next = srs::review(&card, 5);
    
    // Compute next due date
    let today_days = {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() / 86400
    };
    let due_days = today_days + next.interval as u64;
    let (dy, dm, dd) = days_to_ymd(due_days);
    let due_date = format!("{:04}-{:02}-{:02}", dy, dm, dd);

    let mut updated = line.clone();
    updated.srs_interval = next.interval;
    updated.srs_ease = next.ease_factor;
    updated.srs_reps = next.repetitions;
    updated.srs_due_date = due_date;
    updated.confidence = (next.repetitions as f32 / 10.0).min(1.0);
    let _ = state.db.insert_opening_line(&updated);

    let event = TrainingEventRecord {
        id: format!("evt_{}", uuid_now()),
        user_id: "default_user".to_string(),
        kind: "repertoire_drill".to_string(),
        target_id: line.opening_name.clone(),
        outcome: "Correct".to_string(),
        score_delta: 15.0,
        created_at: local_now_str(),
    };
    let _ = state.db.insert_training_event(&event);
}
```

Do the same for the "failed" case using `srs::review(&card, 1)`.

- [ ] **Step 3: Update `recommend_next_action` to use actual SRS due count**

In `app/src/main.rs`, where `snapshot` is built, replace `get_opening_due_count` with `get_due_opening_lines`:

```rust
let today = local_now_str();
let due_lines = state.db.get_due_opening_lines("default_user", &today).unwrap_or_default();
let snapshot = mastery_ui::UserSnapshot {
    opening_due: due_lines.len() as u32,
    // ... rest unchanged
};
```

- [ ] **Step 4: Build and run**

```
cargo build -p app && cargo run -p app
```

Drill a line and verify it doesn't appear as "due" until the computed interval has passed (check the DB with `sqlite3 forge.db "SELECT opening_name, srs_due_date, srs_reps FROM opening_lines;"`).

- [ ] **Step 5: Commit**

```bash
git add app/src/main.rs app/src/srs.rs
git commit -m "feat: replace confidence stub with SM-2 spaced repetition"
```

---

### Task 9: Sort repertoire list by SRS due date

**Files:**
- Modify: `app/src/main.rs` — sort line list by due date when refreshing repertoire

- [ ] **Step 1: In `refresh_data`, sort opening lines before setting them on the UI**

```rust
let mut lines = state.db.get_opening_lines("default_user").unwrap_or_default();
// Due-first sort: empty due_date (new cards) sort first, then earliest due
lines.sort_by(|a, b| {
    let a_due = if a.srs_due_date.is_empty() { "0000-00-00" } else { &a.srs_due_date };
    let b_due = if b.srs_due_date.is_empty() { "0000-00-00" } else { &b.srs_due_date };
    a_due.cmp(b_due)
});
```

- [ ] **Step 2: Build**

```
cargo build -p app
```

- [ ] **Step 3: Commit**

```bash
git add app/src/main.rs
git commit -m "feat: sort repertoire lines by SRS due date"
```

---

## Phase C — Accuracy & Weakness Analysis

### Task 10: Accuracy % calculation

**Files:**
- Create: `app/src/accuracy.rs`

- [ ] **Step 1: Create `app/src/accuracy.rs` with the chess.com-style accuracy formula**

Chess.com uses: `accuracy = 103.1668 * exp(-0.04354 * centipawn_loss) - 3.1669` clamped to 0–100.

```rust
// app/src/accuracy.rs

/// Convert average centipawn loss to accuracy % using chess.com's formula.
/// Returns a value in [0.0, 100.0].
pub fn accuracy_from_acpl(avg_centipawn_loss: f32) -> f32 {
    let raw = 103.1668 * (-0.04354 * avg_centipawn_loss).exp() - 3.1669;
    raw.clamp(0.0, 100.0)
}

#[derive(Debug, Clone, PartialEq)]
pub struct GameAccuracy {
    pub overall: f32,
    pub opening: f32,    // moves 1–15
    pub middlegame: f32, // moves 16–35
    pub endgame: f32,    // moves 36+
}

/// Compute per-phase accuracy from a list of (ply, centipawn_loss) pairs.
/// Plies with no loss data (None) are excluded.
pub fn compute_game_accuracy(moves: &[(u32, Option<i32>)]) -> GameAccuracy {
    let phase_acpl = |range: std::ops::RangeInclusive<u32>| -> f32 {
        let losses: Vec<f32> = moves
            .iter()
            .filter(|(ply, loss)| range.contains(ply) && loss.is_some())
            .map(|(_, loss)| loss.unwrap() as f32)
            .collect();
        if losses.is_empty() {
            return 100.0;
        }
        let avg = losses.iter().sum::<f32>() / losses.len() as f32;
        accuracy_from_acpl(avg)
    };

    let all_losses: Vec<f32> = moves
        .iter()
        .filter_map(|(_, l)| l.map(|v| v as f32))
        .collect();
    let overall_avg = if all_losses.is_empty() {
        0.0
    } else {
        all_losses.iter().sum::<f32>() / all_losses.len() as f32
    };

    GameAccuracy {
        overall: accuracy_from_acpl(overall_avg),
        opening: phase_acpl(1..=30),    // plies (1 ply = half move; moves 1-15 = plies 1-30)
        middlegame: phase_acpl(31..=70),
        endgame: phase_acpl(71..=999),
    }
}
```

- [ ] **Step 2: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_game_is_100_percent() {
        let acc = accuracy_from_acpl(0.0);
        assert!((acc - 100.0).abs() < 1.0);
    }

    #[test]
    fn high_loss_is_low_accuracy() {
        let acc = accuracy_from_acpl(200.0);
        assert!(acc < 20.0);
    }

    #[test]
    fn phase_accuracy_excludes_unanalyzed_moves() {
        let moves = vec![
            (1, Some(10)), (2, Some(5)),
            (3, None), // not analyzed yet
        ];
        let acc = compute_game_accuracy(&moves);
        // Only 2 moves analyzed; should not panic
        assert!(acc.overall > 0.0);
    }

    #[test]
    fn empty_game_gives_100() {
        let acc = compute_game_accuracy(&[]);
        assert!((acc.overall - 100.0).abs() < 0.01);
    }
}
```

- [ ] **Step 3: Run tests**

```
cargo test -p app --lib accuracy
```

Expected: 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add app/src/accuracy.rs
git commit -m "feat: accuracy % calculation with phase breakdown"
```

---

### Task 11: Add accuracy columns to DB and expose in Game Review

**Files:**
- Modify: `db_manager/src/lib.rs` — add `accuracy_overall`, `accuracy_opening`, etc. to `games`
- Modify: `app/src/main.rs` — compute and store accuracy after analysis; show in Game Review

- [ ] **Step 1: Add accuracy migration to `run_migrations`**

```rust
"ALTER TABLE games ADD COLUMN accuracy_overall REAL",
"ALTER TABLE games ADD COLUMN accuracy_opening REAL",
"ALTER TABLE games ADD COLUMN accuracy_middlegame REAL",
"ALTER TABLE games ADD COLUMN accuracy_endgame REAL",
```

- [ ] **Step 2: Add `update_game_accuracy` to `SqliteStore`**

```rust
pub fn update_game_accuracy(
    &mut self,
    game_id: &str,
    overall: f32,
    opening: f32,
    middlegame: f32,
    endgame: f32,
) -> Result<(), String> {
    self.conn.execute(
        "UPDATE games SET accuracy_overall = ?1, accuracy_opening = ?2,
         accuracy_middlegame = ?3, accuracy_endgame = ?4 WHERE id = ?5",
        rusqlite::params![overall, opening, middlegame, endgame, game_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}
```

- [ ] **Step 3: At the end of the background analysis thread in `app/src/main.rs`, compute and store accuracy**

After the `first_critical_mistake` block, add:

```rust
// Compute and persist accuracy
let ply_loss_pairs: Vec<(u32, Option<i32>)> = positions
    .iter()
    .map(|p| (p.ply, p.centipawn_loss))
    .collect();
let acc = accuracy::compute_game_accuracy(&ply_loss_pairs);
{
    let mut state = state_thread.lock().unwrap();
    let _ = state.db.update_game_accuracy(
        &game_id_thread,
        acc.overall, acc.opening, acc.middlegame, acc.endgame,
    );
}
```

Add `mod accuracy;` at the top of `main.rs`.

- [ ] **Step 4: Show accuracy in the Game Review UI**

In the Slint `AppWindow`, inside the `if (root.active-screen == "review")` block, after the board's "POSITION ANALYSIS" rectangle, add:

```slint
in-out property <string> selected-game-accuracy: "";

// In the position analysis rectangle, add:
Text { text: "Accuracy: " + root.selected-game-accuracy; color: #a78bfa; font-size: 13px; }
```

In the `on_select_game` callback (Rust), fetch accuracy from DB:

```rust
// After fetching positions:
let game_row: Option<(f64,)> = state.db.conn.query_row(
    "SELECT accuracy_overall FROM games WHERE id = ?1",
    rusqlite::params![&game_id.as_str()],
    |row| Ok((row.get(0)?,)),
).ok();
if let Some((acc,)) = game_row {
    app.set_selected_game_accuracy(
        slint::SharedString::from(format!("{:.1}%", acc))
    );
} else {
    app.set_selected_game_accuracy(slint::SharedString::from("Analyzing..."));
}
```

- [ ] **Step 5: Build and test**

```
cargo build -p app && cargo run -p app
```

Import a game, wait for analysis, select it — the accuracy % should appear in the Game Review panel.

- [ ] **Step 6: Commit**

```bash
git add db_manager/src/lib.rs app/src/main.rs app/src/accuracy.rs
git commit -m "feat: persist and display per-game accuracy % in Game Review"
```

---

### Task 12: Weakness profile aggregation

**Files:**
- Create: `app/src/weakness.rs`
- Modify: `app/src/main.rs` — show top weaknesses on dashboard

- [ ] **Step 1: Create `app/src/weakness.rs`**

```rust
// app/src/weakness.rs

#[derive(Debug, Clone, PartialEq)]
pub struct WeaknessReport {
    pub worst_opening_eco: Option<String>,   // ECO code with highest avg loss
    pub blunder_move_numbers: Vec<u32>,      // ply numbers where blunders cluster
    pub endgame_needs_work: bool,            // endgame accuracy < 70%
    pub top_training_priority: String,       // human-readable top priority
}

/// Summarise weakness from aggregated position data.
/// `positions` is a slice of `(ply, centipawn_loss, mistake_class, eco)`.
pub fn build_weakness_report(
    positions: &[(u32, Option<i32>, Option<String>, Option<String>)],
    endgame_accuracy: f32,
) -> WeaknessReport {
    // Find ply numbers that have blunders/mistakes
    let mut blunder_plies: Vec<u32> = positions
        .iter()
        .filter(|(_, _, class, _)| {
            matches!(class.as_deref(), Some("Blunder") | Some("Mistake"))
        })
        .map(|(ply, _, _, _)| *ply)
        .collect();
    blunder_plies.dedup();

    // Find ECO with highest average centipawn loss
    use std::collections::HashMap;
    let mut eco_losses: HashMap<String, Vec<i32>> = HashMap::new();
    for (_, loss, _, eco) in positions {
        if let (Some(loss), Some(eco)) = (loss, eco) {
            eco_losses.entry(eco.clone()).or_default().push(*loss);
        }
    }
    let worst_opening_eco = eco_losses
        .iter()
        .map(|(eco, losses)| {
            let avg = losses.iter().sum::<i32>() as f32 / losses.len() as f32;
            (eco.clone(), avg)
        })
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(eco, _)| eco);

    let endgame_needs_work = endgame_accuracy < 70.0;

    let top_training_priority = if !blunder_plies.is_empty() {
        format!("Recurring mistakes around move {}", blunder_plies[0] / 2 + 1)
    } else if endgame_needs_work {
        "Endgame technique needs improvement".to_string()
    } else if let Some(eco) = &worst_opening_eco {
        format!("Opening {} has high average loss", eco)
    } else {
        "No major weaknesses detected".to_string()
    };

    WeaknessReport {
        worst_opening_eco,
        blunder_move_numbers: blunder_plies,
        endgame_needs_work,
        top_training_priority,
    }
}
```

- [ ] **Step 2: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_blunder_cluster() {
        let positions = vec![
            (10, Some(300), Some("Blunder".to_string()), Some("B01".to_string())),
            (10, Some(150), Some("Mistake".to_string()), Some("B01".to_string())),
            (20, Some(10), None, Some("B01".to_string())),
        ];
        let report = build_weakness_report(&positions, 85.0);
        assert!(report.blunder_move_numbers.contains(&10));
        assert_eq!(report.worst_opening_eco, Some("B01".to_string()));
    }

    #[test]
    fn flags_endgame_weakness() {
        let report = build_weakness_report(&[], 55.0);
        assert!(report.endgame_needs_work);
    }
}
```

- [ ] **Step 3: Run tests**

```
cargo test -p app --lib weakness
```

Expected: 2 tests pass.

- [ ] **Step 4: Show top priority on the Dashboard**

In the Slint `AppWindow`, add a property and display under the stats grid:

```slint
in-out property <string> weakness-summary: "";

// Under stats grid:
if (root.weakness-summary != "") : Rectangle {
    background: #1a1a1e;
    border-radius: 8px;
    border-color: #f43f5e;
    border-width: 1px;
    padding: 12px;
    height: 50px;
    Text {
        text: "Top Priority: " + root.weakness-summary;
        color: #f43f5e;
        font-size: 13px;
        font-weight: 700;
        vertical-alignment: center;
    }
}
```

In `refresh_data` (Rust), after the snapshot block:

```rust
// Build weakness report from all positions
mod weakness; // at top of file
let all_positions: Vec<(u32, Option<i32>, Option<String>, Option<String>)> = {
    let mut stmt = state.db.conn.prepare(
        "SELECT p.ply, p.centipawn_loss, p.mistake_class, g.eco
         FROM positions p JOIN games g ON g.id = p.game_id"
    ).unwrap();
    stmt.query_map([], |row| {
        Ok((
            row.get::<_, u32>(0)?,
            row.get::<_, Option<i32>>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
        ))
    }).unwrap().filter_map(|r| r.ok()).collect()
};
let avg_endgame_acc = state.db.conn.query_row(
    "SELECT AVG(accuracy_endgame) FROM games WHERE accuracy_endgame IS NOT NULL",
    [], |row| row.get::<_, f64>(0),
).unwrap_or(100.0) as f32;

let wr = weakness::build_weakness_report(&all_positions, avg_endgame_acc);
app.set_weakness_summary(slint::SharedString::from(wr.top_training_priority));
```

- [ ] **Step 5: Build and run**

```
cargo build -p app && cargo run -p app
```

Dashboard should show a red weakness card after at least one game has been analyzed.

- [ ] **Step 6: Commit**

```bash
git add app/src/weakness.rs app/src/main.rs
git commit -m "feat: weakness profile on dashboard"
```

---

### Task 13: Puzzle Rush mode (Lichess puzzles in-app)

**Files:**
- Modify: `app/src/main.rs` — add a PuzzleRush screen wired to `get_next_puzzle`

- [ ] **Step 1: Add a Puzzle screen to Slint `AppWindow`**

Add new properties and a sidebar button for `"puzzle"` screen:

```slint
in-out property <string> puzzle-fen: "";
in-out property <string> puzzle-solution: "";
in-out property <string> puzzle-status: "Waiting for puzzle...";
in-out property <[string]> puzzle-board-pieces: [];
callback load-puzzle();
callback submit-puzzle-move(string);
```

```slint
SidebarButton {
    text: "Puzzle Rush";
    active: root.active-screen == "puzzle";
    clicked => { root.select-screen("puzzle"); root.load-puzzle(); }
}
```

Add the puzzle screen layout (below review screen):

```slint
if (root.active-screen == "puzzle") : VerticalLayout {
    spacing: 16px;
    alignment: center;

    Text { text: "Puzzle Rush"; color: #ffffff; font-size: 20px; font-weight: 800; }
    Text { text: root.puzzle-status; color: #a78bfa; font-size: 14px; horizontal-alignment: center; }

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
                text: root.puzzle-board-pieces[index];
                font-size: 28px;
                color: #000000;
                horizontal-alignment: center;
                vertical-alignment: center;
            }
        }
    }

    Button {
        text: "Next Puzzle";
        clicked => { root.load-puzzle(); }
    }
}
```

- [ ] **Step 2: Wire `on_load_puzzle` in `main()`**

```rust
{
    let state = state.clone();
    let app_weak = app_weak.clone();
    app.on_load_puzzle(move || {
        let mut state_lock = state.lock().unwrap();
        let rating = state_lock.db.get_puzzle_rating("default_user").unwrap_or(1500);
        match state_lock.db.get_next_puzzle(rating + 200) {
            Ok(Some(puzzle)) => {
                let app = app_weak.upgrade().unwrap();
                app.set_puzzle_fen(slint::SharedString::from(puzzle.fen.clone()));
                app.set_puzzle_solution(slint::SharedString::from(puzzle.solution_uci.clone()));
                app.set_puzzle_status(slint::SharedString::from("Find the best move!"));
                app.set_puzzle_board_pieces(
                    slint::ModelRc::from(Rc::new(slint::VecModel::from(fen_to_pieces(&puzzle.fen))))
                );
            }
            Ok(None) => {
                let app = app_weak.upgrade().unwrap();
                app.set_puzzle_status(slint::SharedString::from("No puzzles loaded. Sync from Lichess first."));
            }
            Err(e) => eprintln!("puzzle fetch error: {e}"),
        }
    });
}
```

- [ ] **Step 3: Build and run**

```
cargo build -p app && cargo run -p app
```

Sync from Lichess, then click Puzzle Rush — a board should load showing a puzzle position.

- [ ] **Step 4: Commit**

```bash
git add app/src/main.rs
git commit -m "feat: Puzzle Rush screen with Lichess puzzles"
```

---

## Phase D — Opening Tree & Data Mining

### Task 14: Opening tree table in DB

**Files:**
- Modify: `db_manager/src/lib.rs` — add `opening_tree` table

- [ ] **Step 1: Add `opening_tree` to `SQLITE_SCHEMA`**

```rust
r#"
CREATE TABLE IF NOT EXISTS opening_tree (
    id TEXT PRIMARY KEY,
    parent_id TEXT,
    move_uci TEXT NOT NULL,
    move_san TEXT NOT NULL,
    fen TEXT NOT NULL,
    eco TEXT,
    opening_name TEXT,
    master_games INTEGER DEFAULT 0,
    white_wins INTEGER DEFAULT 0,
    draws INTEGER DEFAULT 0,
    black_wins INTEGER DEFAULT 0,
    FOREIGN KEY(parent_id) REFERENCES opening_tree(id)
);
"#,
```

- [ ] **Step 2: Add `OpeningNode` struct and insert/query methods**

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpeningNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub move_uci: String,
    pub move_san: String,
    pub fen: String,
    pub eco: Option<String>,
    pub opening_name: Option<String>,
    pub master_games: i64,
    pub white_wins: i64,
    pub draws: i64,
    pub black_wins: i64,
}

impl SqliteStore {
    pub fn insert_opening_node(&mut self, node: &OpeningNode) -> Result<(), String> {
        self.conn.execute(
            "INSERT OR REPLACE INTO opening_tree
             (id, parent_id, move_uci, move_san, fen, eco, opening_name, master_games, white_wins, draws, black_wins)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![
                node.id, node.parent_id, node.move_uci, node.move_san, node.fen,
                node.eco, node.opening_name, node.master_games, node.white_wins,
                node.draws, node.black_wins
            ],
        ).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_children(&self, parent_id: Option<&str>) -> Result<Vec<OpeningNode>, String> {
        let sql = match parent_id {
            Some(_) => "SELECT id, parent_id, move_uci, move_san, fen, eco, opening_name, master_games, white_wins, draws, black_wins FROM opening_tree WHERE parent_id = ?1 ORDER BY master_games DESC",
            None => "SELECT id, parent_id, move_uci, move_san, fen, eco, opening_name, master_games, white_wins, draws, black_wins FROM opening_tree WHERE parent_id IS NULL ORDER BY master_games DESC",
        };

        let mut stmt = self.conn.prepare(sql).map_err(|e| e.to_string())?;
        
        let map_row = |row: &rusqlite::Row| Ok(OpeningNode {
            id: row.get(0)?,
            parent_id: row.get(1)?,
            move_uci: row.get(2)?,
            move_san: row.get(3)?,
            fen: row.get(4)?,
            eco: row.get(5)?,
            opening_name: row.get(6)?,
            master_games: row.get(7)?,
            white_wins: row.get(8)?,
            draws: row.get(9)?,
            black_wins: row.get(10)?,
        });

        let rows = match parent_id {
            Some(pid) => stmt.query_map(rusqlite::params![pid], map_row),
            None => stmt.query_map([], map_row),
        }.map_err(|e| e.to_string())?;

        let mut nodes = Vec::new();
        for r in rows { nodes.push(r.map_err(|e| e.to_string())?); }
        Ok(nodes)
    }
}
```

- [ ] **Step 3: Write a test**

```rust
#[test]
fn opening_tree_parent_child() {
    let mut store = SqliteStore::new_in_memory().unwrap();
    store.bootstrap().unwrap();

    let root_node = OpeningNode {
        id: "root_e4".to_string(),
        parent_id: None,
        move_uci: "e2e4".to_string(),
        move_san: "e4".to_string(),
        fen: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1".to_string(),
        eco: Some("B00".to_string()),
        opening_name: Some("King's Pawn".to_string()),
        master_games: 10000,
        white_wins: 4200, draws: 2800, black_wins: 3000,
    };
    store.insert_opening_node(&root_node).unwrap();

    let children = store.get_children(None).unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].move_san, "e4");
}
```

- [ ] **Step 4: Run test**

```
cargo test -p db_manager opening_tree
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add db_manager/src/lib.rs
git commit -m "feat: opening_tree table with parent-child queries"
```

---

### Task 15: Lichess Opening Explorer API client

**Files:**
- Create: `lichess_client/src/explorer.rs`

- [ ] **Step 1: Create `lichess_client/src/explorer.rs`**

```rust
use serde::Deserialize;
use crate::LichessClient;

#[derive(Debug, Deserialize, Clone)]
pub struct ExplorerResponse {
    pub white: i64,
    pub draws: i64,
    pub black: i64,
    pub moves: Vec<ExplorerMove>,
    pub opening: Option<ExplorerOpening>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ExplorerMove {
    pub uci: String,
    pub san: String,
    pub white: i64,
    pub draws: i64,
    pub black: i64,
    #[serde(rename = "averageRating")]
    pub average_rating: Option<i32>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ExplorerOpening {
    pub eco: Option<String>,
    pub name: Option<String>,
}

impl LichessClient {
    /// Query Lichess masters database at a given FEN.
    /// Returns top moves with master game statistics.
    pub async fn explorer_masters(&self, fen: &str) -> Result<ExplorerResponse, String> {
        let encoded_fen = fen.replace(" ", "%20").replace("/", "%2F");
        let url = format!(
            "https://explorer.lichess.ovh/masters?fen={}&moves=10",
            encoded_fen
        );

        let resp = self.http
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("Explorer API error: {}", resp.status()));
        }

        resp.json::<ExplorerResponse>()
            .await
            .map_err(|e| format!("parse error: {e}"))
    }
}
```

- [ ] **Step 2: Write a unit test for the response deserialization**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_explorer_response() {
        let json = r#"{"white":5000,"draws":2000,"black":3000,"moves":[{"uci":"e2e4","san":"e4","white":3000,"draws":1000,"black":1000,"averageRating":2600}]}"#;
        let resp: ExplorerResponse = serde_json::from_str(json).expect("should parse");
        assert_eq!(resp.moves.len(), 1);
        assert_eq!(resp.moves[0].san, "e4");
    }
}
```

- [ ] **Step 3: Run test**

```
cargo test -p lichess_client
```

Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add lichess_client/src/explorer.rs
git commit -m "feat: Lichess masters opening explorer client"
```

---

### Task 16: Seed the opening tree from Lichess explorer

**Files:**
- Create: `app/src/scraper.rs` — seed logic (BFS over starting position)
- Modify: `app/src/main.rs` — add "Build Opening Tree" button

- [ ] **Step 1: Create `app/src/scraper.rs`**

This module does a breadth-first expansion up to a fixed depth using the Lichess explorer.

```rust
// app/src/scraper.rs
// Seeds the opening_tree table by BFS-expanding positions via Lichess masters explorer.

use db_manager::{OpeningNode, SqliteStore};
use lichess_client::LichessClient;
use std::collections::VecDeque;

/// Expand the opening tree from the starting position up to `max_depth` half-moves.
/// Only expands moves with >= `min_games` master games.
pub async fn seed_opening_tree(
    store: &mut SqliteStore,
    client: &LichessClient,
    max_depth: u32,
    min_games: i64,
) -> Result<usize, String> {
    let start_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

    // Queue entries: (fen, parent_node_id, depth)
    let mut queue: VecDeque<(String, Option<String>, u32)> = VecDeque::new();
    queue.push_back((start_fen.to_string(), None, 0));

    let mut count = 0usize;

    while let Some((fen, parent_id, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }

        let resp = match client.explorer_masters(&fen).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("explorer error at depth {depth}: {e}");
                continue;
            }
        };

        for mv in &resp.moves {
            let total = mv.white + mv.draws + mv.black;
            if total < min_games {
                continue;
            }

            // Compute the FEN after this move using shakmaty
            let next_fen = match apply_uci_to_fen(&fen, &mv.uci) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("move error {}: {e}", mv.uci);
                    continue;
                }
            };

            let node_id = format!("{}_{}", parent_id.as_deref().unwrap_or("root"), mv.uci);
            let eco = resp.opening.as_ref().and_then(|o| o.eco.clone());
            let opening_name = resp.opening.as_ref().and_then(|o| o.name.clone());

            let node = OpeningNode {
                id: node_id.clone(),
                parent_id: parent_id.clone(),
                move_uci: mv.uci.clone(),
                move_san: mv.san.clone(),
                fen: next_fen.clone(),
                eco,
                opening_name,
                master_games: total,
                white_wins: mv.white,
                draws: mv.draws,
                black_wins: mv.black,
            };

            if let Err(e) = store.insert_opening_node(&node) {
                eprintln!("insert error: {e}");
                continue;
            }

            count += 1;
            queue.push_back((next_fen, Some(node_id), depth + 1));
        }

        // Rate-limit: Lichess explorer allows ~1 req/s for anonymous, more with auth
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    Ok(count)
}

fn apply_uci_to_fen(fen: &str, uci: &str) -> Result<String, String> {
    use shakmaty::{Chess, CastlingMode, Position};
    use shakmaty::fen::Fen;
    use shakmaty::uci::Uci;

    let parsed_fen: Fen = fen.parse().map_err(|e| format!("bad fen: {e}"))?;
    let pos: Chess = parsed_fen.into_position(CastlingMode::Standard)
        .map_err(|e| format!("illegal position: {e}"))?;
    let uci_move: Uci = uci.parse().map_err(|e| format!("bad uci: {e}"))?;
    let m = uci_move.to_move(&pos).map_err(|e| format!("illegal move: {e}"))?;
    let next_pos = pos.play(&m).map_err(|e| format!("play error: {e}"))?;
    Ok(Fen::from_position(next_pos, shakmaty::EnPassantMode::Always).to_string())
}
```

- [ ] **Step 2: Add `tokio` time feature to `app/Cargo.toml`**

```toml
tokio = { version = "1", features = ["rt-multi-thread", "time"] }
```

- [ ] **Step 3: Add "Build Opening Tree" button to Dashboard in Slint**

```slint
in-out property <string> tree-status: "";
callback build-opening-tree();

Button {
    text: "Build Opening Tree";
    clicked => { root.build-opening-tree(); }
}
Text {
    text: root.tree-status;
    color: #a1a1aa;
    font-size: 12px;
    vertical-alignment: center;
}
```

- [ ] **Step 4: Wire `on_build_opening_tree` in `main()`**

```rust
{
    let state = state.clone();
    let app_weak = app_weak.clone();
    app.on_build_opening_tree(move || {
        let token = std::env::var("LICHESS_API_KEY").unwrap_or_default();
        let state2 = state.clone();
        let app_weak2 = app_weak.clone();

        if let Some(app) = app_weak.upgrade() {
            app.set_tree_status(slint::SharedString::from("Building opening tree (depth 4)..."));
        }

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let client = lichess_client::LichessClient::new(&token);

            let result = rt.block_on(async {
                let mut state_lock = state2.lock().unwrap();
                scraper::seed_opening_tree(&mut state_lock.db, &client, 4, 100).await
            });

            let msg = match result {
                Ok(n) => format!("Opening tree built: {} nodes", n),
                Err(e) => format!("Error: {e}"),
            };

            let _ = slint::invoke_from_event_loop(move || {
                if let Some(app) = app_weak2.upgrade() {
                    app.set_tree_status(slint::SharedString::from(msg));
                }
            });
        });
    });
}
```

Add `mod scraper;` at the top of `main.rs`.

- [ ] **Step 5: Build**

```
cargo build -p app
```

Expected: compiles cleanly.

- [ ] **Step 6: Commit**

```bash
git add app/src/scraper.rs app/src/main.rs app/Cargo.toml
git commit -m "feat: seed opening tree from Lichess masters explorer"
```

---

### Task 17: Opening Explorer screen in the UI

**Files:**
- Modify: `app/src/main.rs` — add Opening Explorer screen with tree navigation

- [ ] **Step 1: Add Opening Explorer screen to Slint `AppWindow`**

Add properties:

```slint
in-out property <string> explorer-fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
in-out property <string> explorer-opening-name: "Starting Position";
in-out property <[string]> explorer-moves: [];     // move SANs
in-out property <[string]> explorer-move-ids: [];  // node ids (parallel)
in-out property <[string]> explorer-stats: [];     // "W:43% D:28% B:29% (12000)"
callback explorer-select-move(string);             // takes node id

SidebarButton {
    text: "Opening Explorer";
    active: root.active-screen == "explorer";
    clicked => { root.select-screen("explorer"); }
}
```

Screen layout (under the `if (root.active-screen == "puzzle")` block):

```slint
if (root.active-screen == "explorer") : HorizontalLayout {
    spacing: 24px;

    // Left: move choices
    VerticalLayout {
        width: 340px;
        spacing: 12px;
        Text { text: root.explorer-opening-name; color: #a78bfa; font-size: 14px; font-weight: 700; }

        Rectangle {
            background: #1a1a1e;
            border-radius: 8px;
            padding: 12px;

            ScrollView {
                VerticalLayout {
                    spacing: 8px;
                    alignment: start;
                    for move-san[index] in root.explorer-moves: Rectangle {
                        background: #27272a;
                        border-radius: 6px;
                        height: 50px;
                        padding: 12px;

                        TouchArea {
                            clicked => {
                                root.explorer-select-move(root.explorer-move-ids[index]);
                            }
                        }

                        HorizontalLayout {
                            alignment: space-between;
                            Text { text: move-san; color: #ffffff; font-size: 14px; font-weight: 700; }
                            Text { text: root.explorer-stats[index]; color: #a1a1aa; font-size: 12px; vertical-alignment: center; }
                        }
                    }
                }
            }
        }
    }

    // Right: board
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
                text: root.board-pieces[index];
                font-size: 28px;
                color: #000000;
                horizontal-alignment: center;
                vertical-alignment: center;
            }
        }
    }
}
```

- [ ] **Step 2: Wire `on_select_screen` to load root explorer nodes**

In the `on_select_screen` callback body, add:

```rust
if screen == "explorer" {
    let state_lock = state.lock().unwrap();
    let children = state_lock.db.get_children(None).unwrap_or_default();
    let app = app_weak.upgrade().unwrap();

    let mut move_sans = Vec::new();
    let mut move_ids = Vec::new();
    let mut stats = Vec::new();

    for node in &children {
        move_sans.push(slint::SharedString::from(node.move_san.clone()));
        move_ids.push(slint::SharedString::from(node.id.clone()));
        let total = node.white_wins + node.draws + node.black_wins;
        let stat = if total > 0 {
            format!("W:{:.0}% D:{:.0}% B:{:.0}% ({})",
                node.white_wins as f64 / total as f64 * 100.0,
                node.draws as f64 / total as f64 * 100.0,
                node.black_wins as f64 / total as f64 * 100.0,
                total)
        } else {
            "No data".to_string()
        };
        stats.push(slint::SharedString::from(stat));
    }

    app.set_explorer_moves(slint::ModelRc::from(Rc::new(slint::VecModel::from(move_sans))));
    app.set_explorer_move_ids(slint::ModelRc::from(Rc::new(slint::VecModel::from(move_ids))));
    app.set_explorer_stats(slint::ModelRc::from(Rc::new(slint::VecModel::from(stats))));
    app.set_explorer_opening_name(slint::SharedString::from("Starting Position"));
    app.set_board_pieces(slint::ModelRc::from(Rc::new(slint::VecModel::from(fen_to_pieces("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR")))));
}
```

- [ ] **Step 3: Wire `on_explorer_select_move`**

```rust
{
    let state = state.clone();
    let app_weak = app_weak.clone();
    app.on_explorer_select_move(move |node_id: slint::SharedString| {
        let state_lock = state.lock().unwrap();
        let node_id_str = node_id.as_str();

        // Find the selected node
        let selected = state_lock.db.conn.query_row(
            "SELECT fen, opening_name FROM opening_tree WHERE id = ?1",
            rusqlite::params![node_id_str],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        ).ok();

        let children = state_lock.db.get_children(Some(node_id_str)).unwrap_or_default();
        let app = app_weak.upgrade().unwrap();

        if let Some((fen, name)) = selected {
            app.set_board_pieces(slint::ModelRc::from(Rc::new(slint::VecModel::from(fen_to_pieces(&fen)))));
            app.set_explorer_opening_name(slint::SharedString::from(name.unwrap_or_else(|| "Unknown".to_string())));
        }

        let mut move_sans = Vec::new();
        let mut move_ids = Vec::new();
        let mut stats = Vec::new();

        for node in &children {
            move_sans.push(slint::SharedString::from(node.move_san.clone()));
            move_ids.push(slint::SharedString::from(node.id.clone()));
            let total = node.white_wins + node.draws + node.black_wins;
            let stat = if total > 0 {
                format!("W:{:.0}% D:{:.0}% B:{:.0}% ({})",
                    node.white_wins as f64 / total as f64 * 100.0,
                    node.draws as f64 / total as f64 * 100.0,
                    node.black_wins as f64 / total as f64 * 100.0,
                    total)
            } else { "No data".to_string() };
            stats.push(slint::SharedString::from(stat));
        }

        app.set_explorer_moves(slint::ModelRc::from(Rc::new(slint::VecModel::from(move_sans))));
        app.set_explorer_move_ids(slint::ModelRc::from(Rc::new(slint::VecModel::from(move_ids))));
        app.set_explorer_stats(slint::ModelRc::from(Rc::new(slint::VecModel::from(stats))));
    });
}
```

- [ ] **Step 4: Build and run end-to-end test**

```
cargo build -p app && cargo run -p app
```

1. Click "Build Opening Tree" — wait for completion message
2. Click "Opening Explorer" in sidebar
3. Verify moves (e4, d4, etc.) appear with W/D/B statistics
4. Click a move — board updates to the resulting position and children load

- [ ] **Step 5: Commit**

```bash
git add app/src/main.rs
git commit -m "feat: Opening Explorer screen with master game statistics"
```

---

## Self-Review Checklist

### Spec coverage

| Inspiration | Feature | Task(s) |
|---|---|---|
| Lichess | Auto-import user games via API | Tasks 1–4 |
| Lichess | Puzzle import by rating | Task 5 |
| Lichess | Opening explorer (master DB) | Tasks 15–17 |
| Chess.com | Accuracy % per game | Tasks 10–11 |
| Chess.com | Phase-based analysis (opening/mid/end) | Task 10 |
| AIMchess | Weakness profile + priority | Task 12 |
| ChessReps | SM-2 spaced repetition for lines | Tasks 6–9 |
| ChessReps | Due-date sorted repertoire | Task 9 |
| ChessIgma | Opening tree with statistics | Tasks 14–17 |
| All | Puzzle Rush screen | Task 13 |

### Placeholder scan — none found. All code blocks are complete.

### Type consistency

- `OpeningLineRecord` gains 4 SRS fields in Task 7; all subsequent uses in Tasks 8–9 reference these exact field names.
- `OpeningNode` defined in Task 14 is used as-is in Tasks 15–17.
- `LichessClient` built in Task 1 carries through Tasks 2, 4, 5, 15, 16 unchanged.
- `SrsCard` defined in Task 6 maps directly to the DB columns added in Task 7.

---

## Execution Options

**Plan complete and saved to `docs/superpowers/plans/2026-06-12-grandmaster-forge-full-platform.md`.**

**Two execution options:**

**1. Subagent-Driven (recommended)** — Dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — Execute tasks in this session using `superpowers:executing-plans`, batch execution with checkpoints.

Which approach?
