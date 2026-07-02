pub const SQLITE_SCHEMA: &[&str] = &[
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
CREATE TABLE IF NOT EXISTS game_sync (
    source TEXT NOT NULL,
    external_id TEXT NOT NULL,
    synced_at TEXT NOT NULL,
    PRIMARY KEY (source, external_id)
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
        self.conn.execute(
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
        ).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_games(&self) -> Result<Vec<GameRecord>, String> {
        let mut stmt = self.conn.prepare("SELECT id, source, white, black, result, eco, pgn, played_at FROM games ORDER BY played_at DESC")
            .map_err(|e| e.to_string())?;
        let rows = stmt.query_map([], |row| {
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
        }).map_err(|e| e.to_string())?;
        let mut games = Vec::new();
        for r in rows {
            games.push(r.map_err(|e| e.to_string())?);
        }
        Ok(games)
    }

    pub fn get_positions_for_game(&self, game_id: &str) -> Result<Vec<PositionRecord>, String> {
        let mut stmt = self.conn.prepare("SELECT id, game_id, ply, fen, played_move, centipawn_loss, mistake_class FROM positions WHERE game_id = ?1 ORDER BY ply ASC")
            .map_err(|e| e.to_string())?;
        let rows = stmt.query_map([game_id], |row| {
            Ok(PositionRecord {
                id: row.get(0)?,
                game_id: row.get(1)?,
                ply: row.get(2)?,
                fen: row.get(3)?,
                played_move: row.get(4)?,
                centipawn_loss: row.get(5)?,
                mistake_class: row.get(6)?,
            })
        }).map_err(|e| e.to_string())?;
        let mut positions = Vec::new();
        for r in rows {
            positions.push(r.map_err(|e| e.to_string())?);
        }
        Ok(positions)
    }

    pub fn get_opening_lines(&self, user_id: &str) -> Result<Vec<OpeningLineRecord>, String> {
        let mut stmt = self.conn.prepare(
            "SELECT id, user_id, opening_name, start_fen, line_uci, source, confidence,
                    COALESCE(srs_interval, 1.0), COALESCE(srs_ease, 2.5),
                    COALESCE(srs_reps, 0), COALESCE(srs_due_date, ''),
                    COALESCE(side, 'White')
             FROM opening_lines WHERE user_id = ?1",
        ).map_err(|e| e.to_string())?;
        let rows = stmt.query_map([user_id], |row| {
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
                side: row.get::<_, String>(11).unwrap_or_else(|_| "White".to_string()),
            })
        }).map_err(|e| e.to_string())?;
        let mut lines = Vec::new();
        for r in rows {
            lines.push(r.map_err(|e| e.to_string())?);
        }
        Ok(lines)
    }

    pub fn get_lines_from_position(&self, start_fen: &str, user_id: &str) -> Result<Vec<OpeningLineRecord>, String> {
        let mut stmt = self.conn.prepare(
            "SELECT id, user_id, opening_name, start_fen, line_uci, source, confidence,
                    COALESCE(srs_interval, 1.0), COALESCE(srs_ease, 2.5),
                    COALESCE(srs_reps, 0), COALESCE(srs_due_date, ''),
                    COALESCE(side, 'White')
             FROM opening_lines WHERE start_fen = ?1 AND user_id = ?2",
        ).map_err(|e| e.to_string())?;
        let rows = stmt.query_map(rusqlite::params![start_fen, user_id], |row| {
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
                side: row.get::<_, String>(11).unwrap_or_else(|_| "White".to_string()),
            })
        }).map_err(|e| e.to_string())?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn get_due_opening_lines(&self, user_id: &str, today: &str) -> Result<Vec<OpeningLineRecord>, String> {
        let mut stmt = self.conn.prepare(
            "SELECT id, user_id, opening_name, start_fen, line_uci, source, confidence,
                    COALESCE(srs_interval, 1.0), COALESCE(srs_ease, 2.5),
                    COALESCE(srs_reps, 0), COALESCE(srs_due_date, ''),
                    COALESCE(side, 'White')
             FROM opening_lines
             WHERE user_id = ?1 AND (srs_due_date IS NULL OR srs_due_date = '' OR srs_due_date <= ?2)
             ORDER BY srs_due_date ASC",
        ).map_err(|e| e.to_string())?;
        let rows = stmt.query_map(rusqlite::params![user_id, today], |row| {
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
                side: row.get::<_, String>(11).unwrap_or_else(|_| "White".to_string()),
            })
        }).map_err(|e| e.to_string())?;
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
        let count: u32 = self.conn.query_row(
            "SELECT COUNT(DISTINCT game_id) FROM positions WHERE centipawn_loss IS NULL",
            [],
            |row| row.get(0),
        ).unwrap_or(0);
        Ok(count)
    }

    pub fn get_blunder_hotspots_count(&self) -> Result<u32, String> {
        let count: u32 = self.conn.query_row(
            "SELECT COUNT(*) FROM positions WHERE mistake_class IN ('Mistake', 'Blunder')",
            [],
            |row| row.get(0),
        ).unwrap_or(0);
        Ok(count)
    }

    pub fn get_opening_due_count(&self, user_id: &str) -> Result<u32, String> {
        let count: u32 = self.conn.query_row(
            "SELECT COUNT(*) FROM opening_lines WHERE user_id = ?1 AND confidence < 0.8",
            rusqlite::params![user_id],
            |row| row.get(0),
        ).unwrap_or(0);
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
                    node.id, node.parent_id, node.move_uci, node.move_san, node.fen,
                    node.eco, node.opening_name, node.master_games, node.white_wins,
                    node.draws, node.black_wins
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
            let mut stmt = self.conn.prepare(&format!(
                "SELECT {col} FROM opening_tree WHERE parent_id = ?1 ORDER BY master_games DESC"
            )).map_err(|e| e.to_string())?;
            let rows = stmt.query_map(rusqlite::params![pid], map_row)
                .map_err(|e| e.to_string())?;
            rows.filter_map(|r| r.ok()).collect()
        } else {
            let mut stmt = self.conn.prepare(&format!(
                "SELECT {col} FROM opening_tree WHERE parent_id IS NULL ORDER BY master_games DESC"
            )).map_err(|e| e.to_string())?;
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

        Ok(())
    }

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

        let fetched = store.get_next_puzzle(1600).unwrap().expect("should have puzzle");
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

    #[test]
    fn lichess_sync_migrates_to_game_sync() {
        let mut store = SqliteStore::new_in_memory().unwrap();
        store.bootstrap().unwrap();
        // Simulate a database created by an older binary with lichess_sync table
        store.conn.execute(
            "CREATE TABLE lichess_sync (game_id TEXT PRIMARY KEY, synced_at TEXT NOT NULL)",
            [],
        ).unwrap();
        store.conn.execute(
            "INSERT INTO lichess_sync (game_id, synced_at) VALUES ('oldgame', '2026-06-01')",
            [],
        ).unwrap();
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
}
