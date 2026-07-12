pub const SQLITE_SCHEMA: &[&str] = &[
    r#"
PRAGMA foreign_keys = ON;
"#,
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
    r#"
CREATE TABLE IF NOT EXISTS games (
    id TEXT PRIMARY KEY,
    source TEXT NOT NULL,
    white TEXT NOT NULL,
    black TEXT NOT NULL,
    result TEXT,
    eco TEXT,
    pgn TEXT NOT NULL,
    played_at TEXT
);
"#,
    r#"
CREATE TABLE IF NOT EXISTS positions (
    id TEXT PRIMARY KEY,
    game_id TEXT NOT NULL,
    ply INTEGER NOT NULL,
    fen TEXT NOT NULL,
    played_move TEXT NOT NULL,
    centipawn_loss INTEGER,
    mistake_class TEXT,
    FOREIGN KEY(game_id) REFERENCES games(id)
);
"#,
    r#"
CREATE TABLE IF NOT EXISTS opening_lines (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    opening_name TEXT NOT NULL,
    start_fen TEXT NOT NULL,
    line_uci TEXT NOT NULL,
    source TEXT NOT NULL,
    confidence REAL DEFAULT 0.0
);
"#,
    r#"
CREATE TABLE IF NOT EXISTS puzzles (
    id TEXT PRIMARY KEY,
    fen TEXT NOT NULL,
    solution_uci TEXT NOT NULL,
    theme TEXT NOT NULL,
    difficulty INTEGER NOT NULL
);
"#,
    r#"
CREATE TABLE IF NOT EXISTS training_events (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    target_id TEXT NOT NULL,
    outcome TEXT NOT NULL,
    score_delta REAL NOT NULL,
    created_at TEXT NOT NULL
);
"#,
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
CREATE TABLE IF NOT EXISTS courses (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    side TEXT NOT NULL CHECK (side IN ('White','Black','Mixed')),
    source TEXT NOT NULL,
    tags TEXT NOT NULL DEFAULT '',
    thumbnail_fen TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
"#,
    r#"
CREATE TABLE IF NOT EXISTS course_chapters (
    id TEXT PRIMARY KEY,
    course_id TEXT NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    sort_order INTEGER NOT NULL DEFAULT 0
);
"#,
    r#"
CREATE TABLE IF NOT EXISTS course_lines (
    id TEXT PRIMARY KEY,
    chapter_id TEXT NOT NULL REFERENCES course_chapters(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    root_node_id INTEGER NOT NULL REFERENCES repertoire_nodes(id),
    sort_order INTEGER NOT NULL DEFAULT 0,
    pgn TEXT
);
"#,
    r#"
CREATE TABLE IF NOT EXISTS course_line_moves (
    line_id TEXT NOT NULL REFERENCES course_lines(id) ON DELETE CASCADE,
    move_id INTEGER NOT NULL REFERENCES repertoire_moves(id),
    ply_index INTEGER NOT NULL CHECK (ply_index >= 0),
    is_required INTEGER NOT NULL DEFAULT 1 CHECK (is_required IN (0, 1)),
    PRIMARY KEY (line_id, ply_index)
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
];

#[derive(Debug, Clone, PartialEq)]
pub struct OpeningLineRecord {
    pub id: String,
    pub user_id: String,
    pub opening_name: String,
    pub start_fen: String,
    pub line_uci: Vec<String>,
    pub source: String,
    pub confidence: f32,
    pub srs_interval: f32,
    pub srs_ease: f32,
    pub srs_reps: u32,
    pub srs_due_date: String,
    pub side: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionRecord {
    pub id: String,
    pub game_id: String,
    pub ply: u32,
    pub fen: String,
    pub played_move: String,
    pub centipawn_loss: Option<i32>,
    pub mistake_class: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrainingEventRecord {
    pub id: String,
    pub user_id: String,
    pub kind: String,
    pub target_id: String,
    pub outcome: String,
    pub score_delta: f32,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameRecord {
    pub id: String,
    pub source: String,
    pub white: String,
    pub black: String,
    pub result: Option<String>,
    pub eco: Option<String>,
    pub pgn: String,
    pub played_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PuzzleRecord {
    pub id: String,
    pub fen: String,
    pub solution_uci: String,
    pub theme: String,
    pub difficulty: i32,
}

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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CourseProgress {
    pub total_lines: u32,
    pub discovered_lines: u32,
    pub learned_lines: u32,
    pub mastered_lines: u32,
    pub due_lines: u32,
    pub weak_lines: u32,
}

/// One normalized learning event: any reviewable target (repertoire move,
/// puzzle, tactic, endgame, bot deviation) graded from any source, with
/// optional course/line/move context. Shared input shape for future FSRS.
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

pub trait TrainingStore {
    fn bootstrap(&mut self) -> Result<(), String>;
    fn insert_opening_line(&mut self, line: &OpeningLineRecord) -> Result<(), String>;
    fn insert_position(&mut self, position: &PositionRecord) -> Result<(), String>;
    fn insert_training_event(&mut self, event: &TrainingEventRecord) -> Result<(), String>;
}

pub struct SqliteStore {
    pub conn: rusqlite::Connection,
}

impl SqliteStore {
    pub fn new(path: &str) -> Result<Self, String> {
        let conn = rusqlite::Connection::open(path).map_err(|e| e.to_string())?;
        Ok(Self { conn })
    }

    pub fn new_in_memory() -> Result<Self, String> {
        let conn = rusqlite::Connection::open_in_memory().map_err(|e| e.to_string())?;
        Ok(Self { conn })
    }

    pub fn insert_game(&mut self, game: &GameRecord) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO games (id, source, white, black, result, eco, pgn, played_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO NOTHING",
                rusqlite::params![
                    game.id,
                    game.source,
                    game.white,
                    game.black,
                    game.result,
                    game.eco,
                    game.pgn,
                    game.played_at
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_games(&self) -> Result<Vec<GameRecord>, String> {
        let mut stmt = self.conn.prepare("SELECT id, source, white, black, result, eco, pgn, played_at FROM games ORDER BY played_at DESC")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok(GameRecord {
                    id: row.get(0)?,
                    source: row.get(1)?,
                    white: row.get(2)?,
                    black: row.get(3)?,
                    result: row.get(4)?,
                    eco: row.get(5)?,
                    pgn: row.get(6)?,
                    played_at: row.get(7)?,
                })
            })
            .map_err(|e| e.to_string())?;
        let mut games = Vec::new();
        for r in rows {
            games.push(r.map_err(|e| e.to_string())?);
        }
        Ok(games)
    }

    pub fn get_positions_for_game(&self, game_id: &str) -> Result<Vec<PositionRecord>, String> {
        let mut stmt = self.conn.prepare("SELECT id, game_id, ply, fen, played_move, centipawn_loss, mistake_class FROM positions WHERE game_id = ?1 ORDER BY ply ASC")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([game_id], |row| {
                Ok(PositionRecord {
                    id: row.get(0)?,
                    game_id: row.get(1)?,
                    ply: row.get(2)?,
                    fen: row.get(3)?,
                    played_move: row.get(4)?,
                    centipawn_loss: row.get(5)?,
                    mistake_class: row.get(6)?,
                })
            })
            .map_err(|e| e.to_string())?;
        let mut positions = Vec::new();
        for r in rows {
            positions.push(r.map_err(|e| e.to_string())?);
        }
        Ok(positions)
    }

    pub fn get_opening_lines(&self, user_id: &str) -> Result<Vec<OpeningLineRecord>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, user_id, opening_name, start_fen, line_uci, source, confidence,
                    COALESCE(srs_interval, 1.0), COALESCE(srs_ease, 2.5),
                    COALESCE(srs_reps, 0), COALESCE(srs_due_date, ''),
                    COALESCE(side, 'White')
             FROM opening_lines WHERE user_id = ?1",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([user_id], |row| {
                let line_uci_str: String = row.get(4)?;
                Ok(OpeningLineRecord {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    opening_name: row.get(2)?,
                    start_fen: row.get(3)?,
                    line_uci: parse_line_uci(&line_uci_str),
                    source: row.get(5)?,
                    confidence: row.get(6)?,
                    srs_interval: row.get::<_, f64>(7).unwrap_or(1.0) as f32,
                    srs_ease: row.get::<_, f64>(8).unwrap_or(2.5) as f32,
                    srs_reps: row.get::<_, u32>(9).unwrap_or(0),
                    srs_due_date: row.get::<_, String>(10).unwrap_or_default(),
                    side: row
                        .get::<_, String>(11)
                        .unwrap_or_else(|_| "White".to_string()),
                })
            })
            .map_err(|e| e.to_string())?;
        let mut lines = Vec::new();
        for r in rows {
            lines.push(r.map_err(|e| e.to_string())?);
        }
        Ok(lines)
    }

    pub fn get_lines_from_position(
        &self,
        start_fen: &str,
        user_id: &str,
    ) -> Result<Vec<OpeningLineRecord>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, user_id, opening_name, start_fen, line_uci, source, confidence,
                    COALESCE(srs_interval, 1.0), COALESCE(srs_ease, 2.5),
                    COALESCE(srs_reps, 0), COALESCE(srs_due_date, ''),
                    COALESCE(side, 'White')
             FROM opening_lines WHERE start_fen = ?1 AND user_id = ?2",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params![start_fen, user_id], |row| {
                let line_uci_str: String = row.get(4)?;
                Ok(OpeningLineRecord {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    opening_name: row.get(2)?,
                    start_fen: row.get(3)?,
                    line_uci: parse_line_uci(&line_uci_str),
                    source: row.get(5)?,
                    confidence: row.get(6)?,
                    srs_interval: row.get::<_, f64>(7).unwrap_or(1.0) as f32,
                    srs_ease: row.get::<_, f64>(8).unwrap_or(2.5) as f32,
                    srs_reps: row.get::<_, u32>(9).unwrap_or(0),
                    srs_due_date: row.get::<_, String>(10).unwrap_or_default(),
                    side: row
                        .get::<_, String>(11)
                        .unwrap_or_else(|_| "White".to_string()),
                })
            })
            .map_err(|e| e.to_string())?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn get_due_opening_lines(
        &self,
        user_id: &str,
        today: &str,
    ) -> Result<Vec<OpeningLineRecord>, String> {
        let mut stmt = self.conn.prepare(
            "SELECT id, user_id, opening_name, start_fen, line_uci, source, confidence,
                    COALESCE(srs_interval, 1.0), COALESCE(srs_ease, 2.5),
                    COALESCE(srs_reps, 0), COALESCE(srs_due_date, ''),
                    COALESCE(side, 'White')
             FROM opening_lines
             WHERE user_id = ?1 AND (srs_due_date IS NULL OR srs_due_date = '' OR srs_due_date <= ?2)
             ORDER BY srs_due_date ASC",
        ).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params![user_id, today], |row| {
                let line_uci_str: String = row.get(4)?;
                Ok(OpeningLineRecord {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    opening_name: row.get(2)?,
                    start_fen: row.get(3)?,
                    line_uci: parse_line_uci(&line_uci_str),
                    source: row.get(5)?,
                    confidence: row.get(6)?,
                    srs_interval: row.get::<_, f64>(7).unwrap_or(1.0) as f32,
                    srs_ease: row.get::<_, f64>(8).unwrap_or(2.5) as f32,
                    srs_reps: row.get::<_, u32>(9).unwrap_or(0),
                    srs_due_date: row.get::<_, String>(10).unwrap_or_default(),
                    side: row
                        .get::<_, String>(11)
                        .unwrap_or_else(|_| "White".to_string()),
                })
            })
            .map_err(|e| e.to_string())?;
        let mut lines = Vec::new();
        for r in rows {
            lines.push(r.map_err(|e| e.to_string())?);
        }
        Ok(lines)
    }

    pub fn update_game_accuracy(
        &mut self,
        game_id: &str,
        overall: f32,
        opening: f32,
        middlegame: f32,
        endgame: f32,
    ) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE games SET accuracy_overall = ?1, accuracy_opening = ?2,
                 accuracy_middlegame = ?3, accuracy_endgame = ?4 WHERE id = ?5",
                rusqlite::params![overall, opening, middlegame, endgame, game_id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_unreviewed_games_count(&self) -> Result<u32, String> {
        let count: u32 = self
            .conn
            .query_row(
                "SELECT COUNT(DISTINCT game_id) FROM positions WHERE centipawn_loss IS NULL",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        Ok(count)
    }

    pub fn get_blunder_hotspots_count(&self) -> Result<u32, String> {
        let count: u32 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM positions WHERE mistake_class IN ('Mistake', 'Blunder')",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        Ok(count)
    }

    pub fn get_opening_due_count(&self, user_id: &str) -> Result<u32, String> {
        let count: u32 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM opening_lines WHERE user_id = ?1 AND confidence < 0.8",
                rusqlite::params![user_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        Ok(count)
    }

    pub fn get_puzzle_rating(&self, user_id: &str) -> Result<i32, String> {
        let rating: i32 = self.conn.query_row(
            "SELECT score_delta FROM training_events WHERE user_id = ?1 AND kind = 'puzzle' ORDER BY created_at DESC LIMIT 1",
            rusqlite::params![user_id],
            |row| row.get(0),
        ).unwrap_or(1500);
        Ok(rating)
    }

    pub fn is_game_synced(&self, game_id: &str) -> bool {
        self.is_synced("lichess", game_id)
    }

    pub fn mark_game_synced(&mut self, game_id: &str, synced_at: &str) -> Result<(), String> {
        self.mark_synced("lichess", game_id, synced_at)
    }

    pub fn insert_puzzle(&mut self, puzzle: &PuzzleRecord) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT OR IGNORE INTO puzzles (id, fen, solution_uci, theme, difficulty)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    puzzle.id,
                    puzzle.fen,
                    puzzle.solution_uci,
                    puzzle.theme,
                    puzzle.difficulty
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_next_puzzle(&self, max_difficulty: i32) -> Result<Option<PuzzleRecord>, String> {
        let result = self.conn.query_row(
            "SELECT id, fen, solution_uci, theme, difficulty FROM puzzles
             WHERE difficulty <= ?1
             ORDER BY RANDOM() LIMIT 1",
            rusqlite::params![max_difficulty],
            |row| {
                Ok(PuzzleRecord {
                    id: row.get(0)?,
                    fen: row.get(1)?,
                    solution_uci: row.get(2)?,
                    theme: row.get(3)?,
                    difficulty: row.get(4)?,
                })
            },
        );
        match result {
            Ok(p) => Ok(Some(p)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn insert_opening_node(&mut self, node: &OpeningNode) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO opening_tree
                 (id, parent_id, move_uci, move_san, fen, eco, opening_name,
                  master_games, white_wins, draws, black_wins)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                rusqlite::params![
                    node.id,
                    node.parent_id,
                    node.move_uci,
                    node.move_san,
                    node.fen,
                    node.eco,
                    node.opening_name,
                    node.master_games,
                    node.white_wins,
                    node.draws,
                    node.black_wins
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_children(&self, parent_id: Option<&str>) -> Result<Vec<OpeningNode>, String> {
        let map_row = |row: &rusqlite::Row| {
            Ok(OpeningNode {
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
            })
        };

        let col = "id, parent_id, move_uci, move_san, fen, eco, opening_name, \
                   master_games, white_wins, draws, black_wins";

        let nodes = if let Some(pid) = parent_id {
            let mut stmt = self
                .conn
                .prepare(&format!(
                "SELECT {col} FROM opening_tree WHERE parent_id = ?1 ORDER BY master_games DESC"
            ))
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map(rusqlite::params![pid], map_row)
                .map_err(|e| e.to_string())?;
            rows.filter_map(|r| r.ok()).collect()
        } else {
            let mut stmt = self
                .conn
                .prepare(&format!(
                "SELECT {col} FROM opening_tree WHERE parent_id IS NULL ORDER BY master_games DESC"
            ))
                .map_err(|e| e.to_string())?;
            let rows = stmt.query_map([], map_row).map_err(|e| e.to_string())?;
            rows.filter_map(|r| r.ok()).collect()
        };

        Ok(nodes)
    }

    pub fn run_migrations(&mut self) -> Result<(), String> {
        let migrations = [
            "ALTER TABLE opening_lines ADD COLUMN srs_interval REAL DEFAULT 1.0",
            "ALTER TABLE opening_lines ADD COLUMN srs_ease REAL DEFAULT 2.5",
            "ALTER TABLE opening_lines ADD COLUMN srs_reps INTEGER DEFAULT 0",
            "ALTER TABLE opening_lines ADD COLUMN srs_due_date TEXT DEFAULT ''",
            "ALTER TABLE opening_lines ADD COLUMN side TEXT DEFAULT 'White'",
            "ALTER TABLE games ADD COLUMN accuracy_overall REAL",
            "ALTER TABLE games ADD COLUMN accuracy_opening REAL",
            "ALTER TABLE games ADD COLUMN accuracy_middlegame REAL",
            "ALTER TABLE games ADD COLUMN accuracy_endgame REAL",
        ];
        for sql in &migrations {
            // Ignore "duplicate column" errors — migration already applied
            let _ = self.conn.execute(sql, []);
        }

        // Move lichess_sync rows into the per-source game_sync ledger, then drop it.
        let has_old: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='lichess_sync'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        if has_old > 0 {
            let _ = self.conn.execute(
                "INSERT OR IGNORE INTO game_sync (source, external_id, synced_at)
                 SELECT 'lichess', game_id, synced_at FROM lichess_sync",
                [],
            );
            let _ = self.conn.execute("DROP TABLE lichess_sync", []);
        }

        Ok(())
    }

    pub fn opening_lines_table_exists(&self) -> bool {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='opening_lines'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0)
            > 0
    }

    pub fn retire_opening_lines(&mut self) -> Result<(), String> {
        if self.opening_lines_table_exists() {
            self.conn
                .execute(
                    "ALTER TABLE opening_lines RENAME TO opening_lines_legacy",
                    [],
                )
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn repertoire_move_count(&self) -> Result<u32, String> {
        self.conn
            .query_row("SELECT COUNT(*) FROM repertoire_moves", [], |row| {
                row.get(0)
            })
            .map_err(|e| e.to_string())
    }

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

    pub fn get_due_repertoire_moves(
        &self,
        today: &str,
    ) -> Result<Vec<RepertoireMoveRecord>, String> {
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

    pub fn get_repertoire_move(
        &self,
        move_id: i64,
    ) -> Result<Option<RepertoireMoveRecord>, String> {
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

    /// Look up a repertoire node's side and (normalized) FEN by id.
    pub fn get_node_side_and_fen(&self, node_id: i64) -> Result<Option<(String, String)>, String> {
        match self.conn.query_row(
            "SELECT side, fen FROM repertoire_nodes WHERE id = ?1",
            rusqlite::params![node_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        ) {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn map_course(row: &rusqlite::Row) -> rusqlite::Result<CourseRecord> {
        Ok(CourseRecord {
            id: row.get(0)?,
            title: row.get(1)?,
            description: row.get(2)?,
            side: row.get(3)?,
            source: row.get(4)?,
            tags: row.get(5)?,
            thumbnail_fen: row.get(6)?,
        })
    }

    pub fn insert_course(&mut self, course: &CourseRecord) -> Result<String, String> {
        self.conn
            .execute(
                "INSERT INTO courses (id, title, description, side, source, tags, thumbnail_fen)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(id) DO UPDATE SET
                    title = excluded.title,
                    description = excluded.description,
                    side = excluded.side,
                    source = excluded.source,
                    tags = excluded.tags,
                    thumbnail_fen = excluded.thumbnail_fen,
                    updated_at = CURRENT_TIMESTAMP",
                rusqlite::params![
                    course.id,
                    course.title,
                    course.description,
                    course.side,
                    course.source,
                    course.tags,
                    course.thumbnail_fen
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(course.id.clone())
    }

    pub fn get_course(&self, course_id: &str) -> Result<Option<CourseRecord>, String> {
        match self.conn.query_row(
            "SELECT id, title, description, side, source, tags, thumbnail_fen
             FROM courses WHERE id = ?1",
            [course_id],
            Self::map_course,
        ) {
            Ok(course) => Ok(Some(course)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn get_courses(&self) -> Result<Vec<CourseRecord>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, title, description, side, source, tags, thumbnail_fen
                 FROM courses ORDER BY title COLLATE NOCASE ASC, id ASC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], Self::map_course)
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    pub fn ensure_uncategorized_chapter(&mut self, side: &str) -> Result<String, String> {
        let side_id = match side {
            "White" => "white",
            "Black" => "black",
            _ => return Err("course side must be White or Black".to_string()),
        };
        let course_id = format!("uncategorized-{side_id}");
        let chapter_id = format!("{course_id}-main");
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

    pub fn delete_course(&mut self, course_id: &str) -> Result<bool, String> {
        self.conn
            .execute("DELETE FROM courses WHERE id = ?1", [course_id])
            .map(|count| count > 0)
            .map_err(|e| e.to_string())
    }

    fn map_course_chapter(row: &rusqlite::Row) -> rusqlite::Result<CourseChapterRecord> {
        Ok(CourseChapterRecord {
            id: row.get(0)?,
            course_id: row.get(1)?,
            title: row.get(2)?,
            description: row.get(3)?,
            sort_order: row.get(4)?,
        })
    }

    pub fn insert_course_chapter(
        &mut self,
        chapter: &CourseChapterRecord,
    ) -> Result<String, String> {
        self.conn
            .execute(
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
            )
            .map_err(|e| e.to_string())?;
        Ok(chapter.id.clone())
    }

    pub fn get_course_chapter(
        &self,
        chapter_id: &str,
    ) -> Result<Option<CourseChapterRecord>, String> {
        match self.conn.query_row(
            "SELECT id, course_id, title, description, sort_order
             FROM course_chapters WHERE id = ?1",
            [chapter_id],
            Self::map_course_chapter,
        ) {
            Ok(chapter) => Ok(Some(chapter)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn get_course_chapters(&self, course_id: &str) -> Result<Vec<CourseChapterRecord>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, course_id, title, description, sort_order
                 FROM course_chapters WHERE course_id = ?1
                 ORDER BY sort_order ASC, title COLLATE NOCASE ASC, id ASC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([course_id], Self::map_course_chapter)
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    pub fn delete_course_chapter(&mut self, chapter_id: &str) -> Result<bool, String> {
        self.conn
            .execute("DELETE FROM course_chapters WHERE id = ?1", [chapter_id])
            .map(|count| count > 0)
            .map_err(|e| e.to_string())
    }

    fn map_course_line(row: &rusqlite::Row) -> rusqlite::Result<CourseLineRecord> {
        Ok(CourseLineRecord {
            id: row.get(0)?,
            chapter_id: row.get(1)?,
            title: row.get(2)?,
            description: row.get(3)?,
            root_node_id: row.get(4)?,
            sort_order: row.get(5)?,
            pgn: row.get(6)?,
        })
    }

    pub fn insert_course_line(&mut self, line: &CourseLineRecord) -> Result<String, String> {
        self.conn
            .execute(
                "INSERT INTO course_lines
                    (id, chapter_id, title, description, root_node_id, sort_order, pgn)
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
            )
            .map_err(|e| e.to_string())?;
        Ok(line.id.clone())
    }

    pub fn get_course_line(&self, line_id: &str) -> Result<Option<CourseLineRecord>, String> {
        match self.conn.query_row(
            "SELECT id, chapter_id, title, description, root_node_id, sort_order, pgn
             FROM course_lines WHERE id = ?1",
            [line_id],
            Self::map_course_line,
        ) {
            Ok(line) => Ok(Some(line)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn get_course_lines(&self, chapter_id: &str) -> Result<Vec<CourseLineRecord>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, chapter_id, title, description, root_node_id, sort_order, pgn
                 FROM course_lines WHERE chapter_id = ?1
                 ORDER BY sort_order ASC, title COLLATE NOCASE ASC, id ASC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([chapter_id], Self::map_course_line)
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    pub fn delete_course_line(&mut self, line_id: &str) -> Result<bool, String> {
        self.conn
            .execute("DELETE FROM course_lines WHERE id = ?1", [line_id])
            .map(|count| count > 0)
            .map_err(|e| e.to_string())
    }

    pub fn set_course_line_moves(
        &mut self,
        line_id: &str,
        moves: &[(i64, i32, bool)],
    ) -> Result<(), String> {
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;
        let root_node_id = tx
            .query_row(
                "SELECT root_node_id FROM course_lines WHERE id = ?1",
                [line_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    format!("course line not found: {line_id}")
                }
                _ => e.to_string(),
            })?;
        let mut ordered = moves.to_vec();
        ordered.sort_unstable_by_key(|(_, ply_index, _)| *ply_index);
        let mut expected_parent = root_node_id;
        for (move_id, _, _) in &ordered {
            let (parent_id, child_id) = tx
                .query_row(
                    "SELECT parent_id, child_id FROM repertoire_moves WHERE id = ?1",
                    [move_id],
                    |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
                )
                .map_err(|e| e.to_string())?;
            if parent_id != expected_parent {
                return Err(format!(
                    "move {move_id} does not continue course line {line_id}"
                ));
            }
            expected_parent = child_id;
        }
        tx.execute(
            "DELETE FROM course_line_moves WHERE line_id = ?1",
            [line_id],
        )
        .map_err(|e| e.to_string())?;
        for (move_id, ply_index, is_required) in moves {
            tx.execute(
                "INSERT INTO course_line_moves (line_id, move_id, ply_index, is_required)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![line_id, move_id, ply_index, *is_required as i64],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())
    }

    pub fn get_course_line_moves(&self, line_id: &str) -> Result<Vec<(i64, i32, bool)>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT move_id, ply_index, is_required
                 FROM course_line_moves WHERE line_id = ?1 ORDER BY ply_index ASC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([line_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i32>(1)?,
                    row.get::<_, i64>(2)? != 0,
                ))
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    /// A move is weak when its 3 most recent review events contain at least
    /// 2 failures (rating <= 2) or any bot-play deviation. Failures age out of
    /// the window as newer successful reviews land.
    fn move_is_weak(&self, move_id: i64) -> Result<bool, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT rating, source FROM review_events
                 WHERE target_type = 'repertoire_move' AND target_id = ?1
                 ORDER BY created_at DESC, id DESC LIMIT 3",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([move_id.to_string()], |row| {
                Ok((row.get::<_, i32>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;
        let mut failures = 0;
        for row in rows {
            let (rating, source) = row.map_err(|e| e.to_string())?;
            if source == "bot_deviation" {
                return Ok(true);
            }
            failures += i32::from(rating <= 2);
        }
        Ok(failures >= 2)
    }

    pub fn course_line_status(&self, line_id: &str, today: &str) -> Result<String, String> {
        let line_exists = self
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM course_lines WHERE id = ?1)",
                [line_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|e| e.to_string())?
            != 0;
        if !line_exists {
            return Err(format!("course line not found: {line_id}"));
        }

        let mut stmt = self
            .conn
            .prepare(
                "SELECT m.id, COALESCE(m.srs_reps, 0), COALESCE(m.srs_due_date, '')
                 FROM course_line_moves clm
                 JOIN repertoire_moves m ON m.id = clm.move_id
                 WHERE clm.line_id = ?1 AND clm.is_required = 1 AND m.is_my_move = 1",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([line_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, u32>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;

        let mut required = 0;
        let mut attempted = 0;
        let mut passed = 0;
        let mut mastered = 0;
        let mut due = false;
        let mut move_ids = Vec::new();
        for row in rows {
            let (move_id, reps, due_date) = row.map_err(|e| e.to_string())?;
            required += 1;
            attempted += u32::from(reps > 0 || !due_date.is_empty());
            passed += u32::from(reps > 0);
            // ponytail: SM-2 reps >= 2 is the temporary stability proxy; replace it with FSRS stability.
            mastered += u32::from(reps >= 2);
            due |= due_date.is_empty() || due_date.as_str() <= today;
            move_ids.push(move_id);
        }

        // Weak outranks the SRS-derived statuses (except Undiscovered):
        // repeated recent failure is the more actionable signal.
        let mut weak = false;
        if required > 0 && attempted > 0 {
            for move_id in &move_ids {
                if self.move_is_weak(*move_id)? {
                    weak = true;
                    break;
                }
            }
        }

        let status = if required == 0 || attempted == 0 {
            "Undiscovered"
        } else if weak {
            "Weak"
        } else if passed < required {
            "Learning"
        } else if due {
            "Review Due"
        } else if mastered == required {
            "Mastered"
        } else {
            "Learned"
        };
        Ok(status.to_string())
    }

    pub fn course_progress(&self, course_id: &str, today: &str) -> Result<CourseProgress, String> {
        if self.get_course(course_id)?.is_none() {
            return Err(format!("course not found: {course_id}"));
        }

        let mut progress = CourseProgress::default();
        // ponytail: reuse the canonical per-line status; aggregate SQL can replace this if catalogs measure slow.
        for chapter in self.get_course_chapters(course_id)? {
            for line in self.get_course_lines(&chapter.id)? {
                progress.total_lines += 1;
                match self.course_line_status(&line.id, today)?.as_str() {
                    "Undiscovered" => {}
                    "Learning" => progress.discovered_lines += 1,
                    "Learned" => {
                        progress.discovered_lines += 1;
                        progress.learned_lines += 1;
                    }
                    "Mastered" => {
                        progress.discovered_lines += 1;
                        progress.learned_lines += 1;
                        progress.mastered_lines += 1;
                    }
                    "Review Due" => {
                        progress.discovered_lines += 1;
                        progress.learned_lines += 1;
                        progress.due_lines += 1;
                    }
                    "Weak" => {
                        progress.discovered_lines += 1;
                        progress.weak_lines += 1;
                    }
                    status => return Err(format!("unknown course line status: {status}")),
                }
            }
        }
        Ok(progress)
    }

    pub fn insert_review_event(&mut self, event: &ReviewEventRecord) -> Result<(), String> {
        self.conn
            .execute(
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
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_review_events_for_target(
        &self,
        target_type: &str,
        target_id: &str,
    ) -> Result<Vec<ReviewEventRecord>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, user_id, target_type, target_id, rating, source,
                        course_id, line_id, move_id, created_at
                 FROM review_events
                 WHERE target_type = ?1 AND target_id = ?2
                 ORDER BY created_at DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params![target_type, target_id], |row| {
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
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
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

    pub fn mark_synced(
        &mut self,
        source: &str,
        external_id: &str,
        synced_at: &str,
    ) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT OR IGNORE INTO game_sync (source, external_id, synced_at) VALUES (?1, ?2, ?3)",
                rusqlite::params![source, external_id, synced_at],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

impl TrainingStore for SqliteStore {
    fn bootstrap(&mut self) -> Result<(), String> {
        for stmt in schema_bootstrap_statements() {
            self.conn.execute(stmt, []).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn insert_opening_line(&mut self, line: &OpeningLineRecord) -> Result<(), String> {
        let line_uci_str = serialize_line_uci(&line.line_uci);
        self.conn.execute(
            "INSERT INTO opening_lines (id, user_id, opening_name, start_fen, line_uci, source, confidence,
                                        srs_interval, srs_ease, srs_reps, srs_due_date, side)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(id) DO UPDATE SET
                confidence = excluded.confidence,
                line_uci = excluded.line_uci,
                opening_name = excluded.opening_name,
                srs_interval = excluded.srs_interval,
                srs_ease = excluded.srs_ease,
                srs_reps = excluded.srs_reps,
                srs_due_date = excluded.srs_due_date,
                side = excluded.side",
            rusqlite::params![
                line.id,
                line.user_id,
                line.opening_name,
                line.start_fen,
                line_uci_str,
                line.source,
                line.confidence,
                line.srs_interval,
                line.srs_ease,
                line.srs_reps,
                line.srs_due_date,
                line.side
            ],
        ).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn insert_position(&mut self, position: &PositionRecord) -> Result<(), String> {
        self.conn.execute(
            "INSERT INTO positions (id, game_id, ply, fen, played_move, centipawn_loss, mistake_class)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
                centipawn_loss = excluded.centipawn_loss,
                mistake_class = excluded.mistake_class",
            rusqlite::params![
                position.id,
                position.game_id,
                position.ply,
                position.fen,
                position.played_move,
                position.centipawn_loss,
                position.mistake_class
            ],
        ).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn insert_training_event(&mut self, event: &TrainingEventRecord) -> Result<(), String> {
        self.conn.execute(
            "INSERT INTO training_events (id, user_id, kind, target_id, outcome, score_delta, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                event.id,
                event.user_id,
                event.kind,
                event.target_id,
                event.outcome,
                event.score_delta,
                event.created_at
            ],
        ).map_err(|e| e.to_string())?;
        Ok(())
    }
}

pub fn schema_bootstrap_statements() -> Vec<&'static str> {
    SQLITE_SCHEMA.to_vec()
}

pub fn serialize_line_uci(moves: &[String]) -> String {
    moves.join(" ")
}

pub fn parse_line_uci(line: &str) -> Vec<String> {
    line.split_whitespace().map(|m| m.to_string()).collect()
}

/// Normalize a FEN for transposition keys: keep board/side/castling/ep, zero the clocks.
pub fn normalize_fen(fen: &str) -> String {
    let fields: Vec<&str> = fen.split_whitespace().take(4).collect();
    format!("{} 0 1", fields.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_and_parses_line_uci() {
        let line = vec!["e2e4".to_string(), "c7c5".to_string(), "g1f3".to_string()];
        let serialized = serialize_line_uci(&line);
        assert_eq!(serialized, "e2e4 c7c5 g1f3");
        assert_eq!(parse_line_uci(&serialized), line);
    }

    #[test]
    fn exposes_non_empty_schema() {
        assert!(!schema_bootstrap_statements().is_empty());
    }

    fn make_opening_line(id: &str) -> OpeningLineRecord {
        OpeningLineRecord {
            id: id.to_string(),
            user_id: "u1".to_string(),
            opening_name: "Ruy Lopez".to_string(),
            start_fen: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1".to_string(),
            line_uci: vec!["e7e5".to_string()],
            source: "manual".to_string(),
            confidence: 0.0,
            srs_interval: 1.0,
            srs_ease: 2.5,
            srs_reps: 0,
            srs_due_date: String::new(),
            side: "White".to_string(),
        }
    }

    #[test]
    fn sqlite_store_flow() {
        let mut store = SqliteStore::new_in_memory().unwrap();
        store.bootstrap().unwrap();
        store.run_migrations().unwrap();

        let game = GameRecord {
            id: "game1".to_string(),
            source: "lichess".to_string(),
            white: "player1".to_string(),
            black: "player2".to_string(),
            result: Some("1-0".to_string()),
            eco: Some("B01".to_string()),
            pgn: "1. e4 d5".to_string(),
            played_at: Some("2026-06-12".to_string()),
        };
        store.insert_game(&game).unwrap();

        let pos = PositionRecord {
            id: "pos1".to_string(),
            game_id: "game1".to_string(),
            ply: 1,
            fen: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1".to_string(),
            played_move: "d5".to_string(),
            centipawn_loss: None,
            mistake_class: None,
        };
        store.insert_position(&pos).unwrap();

        let games = store.get_games().unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].id, "game1");

        let positions = store.get_positions_for_game("game1").unwrap();
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].id, "pos1");
        assert_eq!(positions[0].centipawn_loss, None);

        let unreviewed = store.get_unreviewed_games_count().unwrap();
        assert_eq!(unreviewed, 1);
    }

    #[test]
    fn dedup_lichess_sync() {
        let mut store = SqliteStore::new_in_memory().unwrap();
        store.bootstrap().unwrap();

        assert!(!store.is_game_synced("abc123"));
        store.mark_game_synced("abc123", "2026-06-12").unwrap();
        assert!(store.is_game_synced("abc123"));
        store.mark_game_synced("abc123", "2026-06-12").unwrap(); // idempotent
    }

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

        let fetched = store
            .get_next_puzzle(1600)
            .unwrap()
            .expect("should have puzzle");
        assert_eq!(fetched.id, "puzzle1");
        assert!(store.get_next_puzzle(1000).unwrap().is_none());
    }

    #[test]
    fn opening_tree_parent_child() {
        let mut store = SqliteStore::new_in_memory().unwrap();
        store.bootstrap().unwrap();

        let node = OpeningNode {
            id: "root_e4".to_string(),
            parent_id: None,
            move_uci: "e2e4".to_string(),
            move_san: "e4".to_string(),
            fen: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1".to_string(),
            eco: Some("B00".to_string()),
            opening_name: Some("King's Pawn".to_string()),
            master_games: 10000,
            white_wins: 4200,
            draws: 2800,
            black_wins: 3000,
        };
        store.insert_opening_node(&node).unwrap();

        let children = store.get_children(None).unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].move_san, "e4");
    }

    #[test]
    fn srs_due_lines_query() {
        let mut store = SqliteStore::new_in_memory().unwrap();
        store.bootstrap().unwrap();
        store.run_migrations().unwrap();

        let mut line = make_opening_line("line1");
        line.srs_due_date = "2026-06-10".to_string();
        store.insert_opening_line(&line).unwrap();

        let due = store.get_due_opening_lines("u1", "2026-06-12").unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].id, "line1");
    }

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
        assert!(store
            .get_repertoire_moves_from(after_e4, "White")
            .unwrap()
            .is_empty());

        // node side + fen round-trips; missing id yields None
        assert_eq!(
            store.get_node_side_and_fen(root).unwrap(),
            Some(("White".to_string(), normalize_fen(start)))
        );
        assert_eq!(store.get_node_side_and_fen(999_999).unwrap(), None);
    }

    #[test]
    fn due_moves_and_srs_update() {
        let mut store = SqliteStore::new_in_memory().unwrap();
        store.bootstrap().unwrap();
        let a = store
            .ensure_repertoire_node("fenA w KQkq - 0 1", "White")
            .unwrap();
        let b = store
            .ensure_repertoire_node("fenB b KQkq - 0 1", "White")
            .unwrap();
        let c = store
            .ensure_repertoire_node("fenC w KQkq - 0 1", "White")
            .unwrap();
        let (mine, _) = store
            .insert_repertoire_move(a, b, "e2e4", "e4", true, "manual")
            .unwrap();
        let (_theirs, _) = store
            .insert_repertoire_move(b, c, "e7e5", "e5", false, "manual")
            .unwrap();

        // '' due date counts as due; opponent edges never appear
        let due = store.get_due_repertoire_moves("2026-07-01").unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].id, mine);
        assert_eq!(
            store.get_due_repertoire_move_count("2026-07-01").unwrap(),
            1
        );

        store
            .update_repertoire_move_srs(mine, 6.0, 2.6, 2, "2026-07-10")
            .unwrap();
        assert!(store
            .get_due_repertoire_moves("2026-07-01")
            .unwrap()
            .is_empty());
        let rec = store.get_repertoire_move(mine).unwrap().unwrap();
        assert!((rec.srs_interval - 6.0).abs() < 0.01);
        assert_eq!(rec.srs_reps, 2);
        assert_eq!(rec.srs_due_date, "2026-07-10");
        // due again on its due date
        assert_eq!(
            store.get_due_repertoire_moves("2026-07-10").unwrap().len(),
            1
        );
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

    #[test]
    fn lichess_sync_migrates_to_game_sync() {
        let mut store = SqliteStore::new_in_memory().unwrap();
        store.bootstrap().unwrap();
        // Simulate a database created by an older binary with lichess_sync table
        store
            .conn
            .execute(
                "CREATE TABLE lichess_sync (game_id TEXT PRIMARY KEY, synced_at TEXT NOT NULL)",
                [],
            )
            .unwrap();
        store
            .conn
            .execute(
                "INSERT INTO lichess_sync (game_id, synced_at) VALUES ('oldgame', '2026-06-01')",
                [],
            )
            .unwrap();
        store.run_migrations().unwrap();
        assert!(store.is_synced("lichess", "oldgame"));
        // lichess_sync table is gone
        let gone: bool = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='lichess_sync'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap()
            == 0;
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

    fn add_test_course(
        store: &mut SqliteStore,
        root_node_id: i64,
    ) -> (CourseRecord, CourseChapterRecord, CourseLineRecord) {
        let course = CourseRecord {
            id: "course".to_string(),
            title: "Course".to_string(),
            description: "Description".to_string(),
            side: "White".to_string(),
            source: "personal".to_string(),
            tags: "e4".to_string(),
            thumbnail_fen: "startpos".to_string(),
        };
        let chapter = CourseChapterRecord {
            id: "chapter".to_string(),
            course_id: course.id.clone(),
            title: "Chapter".to_string(),
            description: String::new(),
            sort_order: 10,
        };
        let line = CourseLineRecord {
            id: "line".to_string(),
            chapter_id: chapter.id.clone(),
            title: "Line".to_string(),
            description: String::new(),
            root_node_id,
            sort_order: 20,
            pgn: Some("1. e4 e5".to_string()),
        };
        store.insert_course(&course).unwrap();
        store.insert_course_chapter(&chapter).unwrap();
        store.insert_course_line(&line).unwrap();
        (course, chapter, line)
    }

    fn add_review_event(
        store: &mut SqliteStore,
        id: &str,
        move_id: i64,
        rating: i32,
        source: &str,
        created_at: &str,
    ) {
        store
            .insert_review_event(&ReviewEventRecord {
                id: id.to_string(),
                user_id: "default_user".to_string(),
                target_type: "repertoire_move".to_string(),
                target_id: move_id.to_string(),
                rating,
                source: source.to_string(),
                course_id: None,
                line_id: None,
                move_id: Some(move_id),
                created_at: created_at.to_string(),
            })
            .unwrap();
    }

    #[test]
    fn course_line_status_derives_weak_from_recent_event_history() {
        let mut store = SqliteStore::new_in_memory().unwrap();
        store.bootstrap().unwrap();
        let root = store
            .ensure_repertoire_node("a w - - 0 1", "White")
            .unwrap();
        let child = store
            .ensure_repertoire_node("b b - - 0 1", "White")
            .unwrap();
        let (move_id, _) = store
            .insert_repertoire_move(root, child, "e2e4", "e4", true, "manual")
            .unwrap();
        add_test_course(&mut store, root);
        store
            .set_course_line_moves("line", &[(move_id, 0, true)])
            .unwrap();
        store
            .update_repertoire_move_srs(move_id, 1.0, 2.5, 1, "2026-07-20")
            .unwrap();

        // Passed once, not due: Learned, and no events means never Weak.
        assert_eq!(
            store.course_line_status("line", "2026-07-10").unwrap(),
            "Learned"
        );

        // Two failures inside the 3-event window: Weak.
        add_review_event(&mut store, "e1", move_id, 1, "drill", "2026-07-08");
        add_review_event(&mut store, "e2", move_id, 1, "drill", "2026-07-09");
        assert_eq!(
            store.course_line_status("line", "2026-07-10").unwrap(),
            "Weak"
        );

        // Two later passes push the failures out of the window: recovers.
        add_review_event(&mut store, "e3", move_id, 5, "drill", "2026-07-11");
        add_review_event(&mut store, "e4", move_id, 5, "drill", "2026-07-12");
        assert_eq!(
            store.course_line_status("line", "2026-07-10").unwrap(),
            "Learned"
        );

        // A bot deviation inside the window marks the line Weak on its own.
        add_review_event(
            &mut store,
            "e5",
            move_id,
            1,
            "bot_deviation",
            "2026-07-13",
        );
        assert_eq!(
            store.course_line_status("line", "2026-07-10").unwrap(),
            "Weak"
        );

        // An undiscovered line stays Undiscovered even with failure events.
        let child2 = store
            .ensure_repertoire_node("c w - - 0 1", "White")
            .unwrap();
        let (move2, _) = store
            .insert_repertoire_move(child, child2, "e7e5", "e5", true, "manual")
            .unwrap();
        store
            .insert_course_line(&CourseLineRecord {
                id: "line2".to_string(),
                chapter_id: "chapter".to_string(),
                title: "Line 2".to_string(),
                description: String::new(),
                root_node_id: child,
                sort_order: 30,
                pgn: None,
            })
            .unwrap();
        store
            .set_course_line_moves("line2", &[(move2, 0, true)])
            .unwrap();
        add_review_event(&mut store, "f1", move2, 1, "drill", "2026-07-08");
        add_review_event(&mut store, "f2", move2, 1, "drill", "2026-07-09");
        assert_eq!(
            store.course_line_status("line2", "2026-07-10").unwrap(),
            "Undiscovered"
        );
    }

    #[test]
    fn ensure_uncategorized_chapter_creates_white_and_black_fallbacks() {
        let mut store = SqliteStore::new_in_memory().unwrap();
        store.bootstrap().unwrap();

        for (side, course_id, chapter_id) in [
            ("White", "uncategorized-white", "uncategorized-white-main"),
            ("Black", "uncategorized-black", "uncategorized-black-main"),
        ] {
            assert_eq!(
                store.ensure_uncategorized_chapter(side).unwrap(),
                chapter_id
            );
            assert_eq!(
                store.ensure_uncategorized_chapter(side).unwrap(),
                chapter_id
            );
            assert_eq!(store.get_course(course_id).unwrap().unwrap().side, side);
            let chapters = store.get_course_chapters(course_id).unwrap();
            assert_eq!(chapters.len(), 1);
            assert_eq!(chapters[0].id, chapter_id);
        }

        assert_eq!(
            store.ensure_uncategorized_chapter("Mixed").unwrap_err(),
            "course side must be White or Black"
        );
        assert!(store.ensure_uncategorized_chapter("white").is_err());
        assert_eq!(store.get_courses().unwrap().len(), 2);
    }

    #[test]
    fn course_chapter_line_crud_round_trip() {
        let mut store = SqliteStore::new_in_memory().unwrap();
        store.bootstrap().unwrap();
        let root = store
            .ensure_repertoire_node("root w - - 0 1", "White")
            .unwrap();
        let (mut course, mut chapter, mut line) = add_test_course(&mut store, root);

        assert_eq!(store.get_courses().unwrap(), vec![course.clone()]);
        assert_eq!(
            store.get_course_chapters(&course.id).unwrap(),
            vec![chapter.clone()]
        );
        assert_eq!(
            store.get_course_lines(&chapter.id).unwrap(),
            vec![line.clone()]
        );

        course.title = "Updated Course".to_string();
        chapter.title = "Updated Chapter".to_string();
        line.title = "Updated Line".to_string();
        store.insert_course(&course).unwrap();
        store.insert_course_chapter(&chapter).unwrap();
        store.insert_course_line(&line).unwrap();
        assert_eq!(store.get_course(&course.id).unwrap(), Some(course.clone()));
        assert_eq!(
            store.get_course_chapter(&chapter.id).unwrap(),
            Some(chapter.clone())
        );
        assert_eq!(store.get_course_line(&line.id).unwrap(), Some(line.clone()));

        assert!(store.delete_course_line(&line.id).unwrap());
        assert!(store.get_course_lines(&chapter.id).unwrap().is_empty());
        store.insert_course_line(&line).unwrap();
        assert!(store.delete_course_chapter(&chapter.id).unwrap());
        assert!(store.get_course_lines(&chapter.id).unwrap().is_empty());
        store.insert_course_chapter(&chapter).unwrap();
        store.insert_course_line(&line).unwrap();
        assert!(store.delete_course(&course.id).unwrap());
        assert!(store.get_course_chapters(&course.id).unwrap().is_empty());
        assert_eq!(store.get_course(&course.id).unwrap(), None);
    }

    #[test]
    fn course_line_moves_keep_ordered_required_edges() {
        let mut store = SqliteStore::new_in_memory().unwrap();
        store.bootstrap().unwrap();
        let a = store
            .ensure_repertoire_node("a w - - 0 1", "White")
            .unwrap();
        let b = store
            .ensure_repertoire_node("b b - - 0 1", "White")
            .unwrap();
        let c = store
            .ensure_repertoire_node("c w - - 0 1", "White")
            .unwrap();
        let (first, _) = store
            .insert_repertoire_move(a, b, "e2e4", "e4", true, "manual")
            .unwrap();
        let (second, _) = store
            .insert_repertoire_move(b, c, "e7e5", "e5", false, "manual")
            .unwrap();
        add_test_course(&mut store, a);

        store
            .set_course_line_moves("line", &[(second, 1, false), (first, 0, true)])
            .unwrap();
        assert_eq!(
            store.get_course_line_moves("line").unwrap(),
            vec![(first, 0, true), (second, 1, false)]
        );
        assert_eq!(store.repertoire_move_count().unwrap(), 2);

        assert!(store
            .set_course_line_moves("line", &[(i64::MAX, 0, true)])
            .is_err());
        let disconnected_root = store
            .ensure_repertoire_node("x w - - 0 1", "White")
            .unwrap();
        let (disconnected, _) = store
            .insert_repertoire_move(disconnected_root, c, "a2a3", "a3", true, "manual")
            .unwrap();
        assert!(store
            .set_course_line_moves("line", &[(first, 0, true), (disconnected, 1, true)])
            .is_err());
        assert_eq!(
            store.get_course_line_moves("line").unwrap(),
            vec![(first, 0, true), (second, 1, false)]
        );
    }

    #[test]
    fn line_status_is_review_due_when_any_required_move_is_due() {
        let mut store = SqliteStore::new_in_memory().unwrap();
        store.bootstrap().unwrap();
        let a = store
            .ensure_repertoire_node("a w - - 0 1", "White")
            .unwrap();
        let b = store
            .ensure_repertoire_node("b b - - 0 1", "White")
            .unwrap();
        let c = store
            .ensure_repertoire_node("c w - - 0 1", "White")
            .unwrap();
        let d = store
            .ensure_repertoire_node("d b - - 0 1", "White")
            .unwrap();
        let e = store
            .ensure_repertoire_node("e w - - 0 1", "White")
            .unwrap();
        let (first, _) = store
            .insert_repertoire_move(a, b, "e2e4", "e4", true, "manual")
            .unwrap();
        let (opponent, _) = store
            .insert_repertoire_move(b, c, "e7e5", "e5", false, "manual")
            .unwrap();
        let (second, _) = store
            .insert_repertoire_move(c, d, "g1f3", "Nf3", true, "manual")
            .unwrap();
        let (optional, _) = store
            .insert_repertoire_move(d, e, "b8c6", "Nc6", true, "manual")
            .unwrap();
        add_test_course(&mut store, a);
        store
            .set_course_line_moves(
                "line",
                &[
                    (first, 0, true),
                    (opponent, 1, true),
                    (second, 2, true),
                    (optional, 3, false),
                ],
            )
            .unwrap();

        assert_eq!(
            store.course_line_status("line", "2026-07-07").unwrap(),
            "Undiscovered"
        );
        store
            .update_repertoire_move_srs(first, 1.0, 2.5, 1, "2026-07-08")
            .unwrap();
        assert_eq!(
            store.course_line_status("line", "2026-07-07").unwrap(),
            "Learning"
        );
        store
            .update_repertoire_move_srs(second, 1.0, 2.5, 1, "2026-07-08")
            .unwrap();
        assert_eq!(
            store.course_line_status("line", "2026-07-07").unwrap(),
            "Learned"
        );
        store
            .update_repertoire_move_srs(first, 1.0, 2.5, 1, "2026-07-07")
            .unwrap();
        assert_eq!(
            store.course_line_status("line", "2026-07-07").unwrap(),
            "Review Due"
        );
        store
            .update_repertoire_move_srs(first, 6.0, 2.5, 2, "2026-08-01")
            .unwrap();
        store
            .update_repertoire_move_srs(second, 6.0, 2.5, 2, "2026-08-01")
            .unwrap();
        assert_eq!(
            store.course_line_status("line", "2026-07-07").unwrap(),
            "Mastered"
        );
    }

    #[test]
    fn course_progress_follows_child_line_status() {
        let mut store = SqliteStore::new_in_memory().unwrap();
        store.bootstrap().unwrap();
        let root = store
            .ensure_repertoire_node("a w - - 0 1", "White")
            .unwrap();
        let child = store
            .ensure_repertoire_node("b b - - 0 1", "White")
            .unwrap();
        let (move_id, _) = store
            .insert_repertoire_move(root, child, "e2e4", "e4", true, "manual")
            .unwrap();
        add_test_course(&mut store, root);
        store
            .set_course_line_moves("line", &[(move_id, 0, true)])
            .unwrap();

        assert_eq!(
            store.course_progress("course", "2026-07-10").unwrap(),
            CourseProgress {
                total_lines: 1,
                ..CourseProgress::default()
            }
        );

        store
            .update_repertoire_move_srs(move_id, 1.0, 2.5, 1, "2026-07-11")
            .unwrap();
        assert_eq!(
            store.course_progress("course", "2026-07-10").unwrap(),
            CourseProgress {
                total_lines: 1,
                discovered_lines: 1,
                learned_lines: 1,
                ..CourseProgress::default()
            }
        );

        store
            .update_repertoire_move_srs(move_id, 6.0, 2.5, 2, "2026-07-11")
            .unwrap();
        assert_eq!(
            store.course_progress("course", "2026-07-10").unwrap(),
            CourseProgress {
                total_lines: 1,
                discovered_lines: 1,
                learned_lines: 1,
                mastered_lines: 1,
                ..CourseProgress::default()
            }
        );

        store
            .update_repertoire_move_srs(move_id, 6.0, 2.5, 2, "2026-07-10")
            .unwrap();
        assert_eq!(
            store.course_progress("course", "2026-07-10").unwrap(),
            CourseProgress {
                total_lines: 1,
                discovered_lines: 1,
                learned_lines: 1,
                mastered_lines: 0,
                due_lines: 1,
                ..CourseProgress::default()
            }
        );
        assert!(store.course_progress("missing", "2026-07-10").is_err());

        // Two recent failures make the line Weak, which counts as discovered
        // and weak but no longer due/mastered.
        add_review_event(&mut store, "w1", move_id, 1, "drill", "2026-07-08");
        add_review_event(&mut store, "w2", move_id, 1, "drill", "2026-07-09");
        assert_eq!(
            store.course_progress("course", "2026-07-10").unwrap(),
            CourseProgress {
                total_lines: 1,
                discovered_lines: 1,
                weak_lines: 1,
                ..CourseProgress::default()
            }
        );
    }

    #[test]
    fn review_events_record_target_rating_and_source() {
        let mut store = SqliteStore::new_in_memory().unwrap();
        store.bootstrap().unwrap();

        store
            .insert_review_event(&ReviewEventRecord {
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
            })
            .unwrap();
        store
            .insert_review_event(&ReviewEventRecord {
                id: "evt2".to_string(),
                user_id: "default_user".to_string(),
                target_type: "puzzle".to_string(),
                target_id: "p1".to_string(),
                rating: 5,
                source: "puzzle_trainer".to_string(),
                course_id: None,
                line_id: None,
                move_id: None,
                created_at: "2026-07-08".to_string(),
            })
            .unwrap();

        let events = store
            .get_review_events_for_target("repertoire_move", "42")
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].rating, 1);
        assert_eq!(events[0].source, "drill");
        assert_eq!(events[0].course_id.as_deref(), Some("vienna"));
        assert_eq!(events[0].line_id.as_deref(), Some("vienna-main"));
        assert_eq!(events[0].move_id, Some(42));

        let puzzle_events = store.get_review_events_for_target("puzzle", "p1").unwrap();
        assert_eq!(puzzle_events.len(), 1);
        assert_eq!(puzzle_events[0].course_id, None);
        assert_eq!(puzzle_events[0].move_id, None);
    }
}
