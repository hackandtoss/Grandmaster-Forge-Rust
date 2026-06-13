pub const SQLITE_SCHEMA: &[&str] = &[
    r#"
CREATE TABLE IF NOT EXISTS lichess_sync (
    game_id TEXT PRIMARY KEY,
    synced_at TEXT NOT NULL
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
                    COALESCE(srs_reps, 0), COALESCE(srs_due_date, '')
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
            })
        }).map_err(|e| e.to_string())?;
        let mut lines = Vec::new();
        for r in rows {
            lines.push(r.map_err(|e| e.to_string())?);
        }
        Ok(lines)
    }

    pub fn get_due_opening_lines(&self, user_id: &str, today: &str) -> Result<Vec<OpeningLineRecord>, String> {
        let mut stmt = self.conn.prepare(
            "SELECT id, user_id, opening_name, start_fen, line_uci, source, confidence,
                    COALESCE(srs_interval, 1.0), COALESCE(srs_ease, 2.5),
                    COALESCE(srs_reps, 0), COALESCE(srs_due_date, '')
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
        self.conn
            .query_row(
                "SELECT 1 FROM lichess_sync WHERE game_id = ?1",
                rusqlite::params![game_id],
                |_| Ok(()),
            )
            .is_ok()
    }

    pub fn mark_game_synced(&mut self, game_id: &str, synced_at: &str) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT OR IGNORE INTO lichess_sync (game_id, synced_at) VALUES (?1, ?2)",
                rusqlite::params![game_id, synced_at],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
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

    pub fn run_migrations(&mut self) -> Result<(), String> {
        let migrations = [
            "ALTER TABLE opening_lines ADD COLUMN srs_interval REAL DEFAULT 1.0",
            "ALTER TABLE opening_lines ADD COLUMN srs_ease REAL DEFAULT 2.5",
            "ALTER TABLE opening_lines ADD COLUMN srs_reps INTEGER DEFAULT 0",
            "ALTER TABLE opening_lines ADD COLUMN srs_due_date TEXT DEFAULT ''",
            "ALTER TABLE games ADD COLUMN accuracy_overall REAL",
            "ALTER TABLE games ADD COLUMN accuracy_opening REAL",
            "ALTER TABLE games ADD COLUMN accuracy_middlegame REAL",
            "ALTER TABLE games ADD COLUMN accuracy_endgame REAL",
        ];
        for sql in &migrations {
            // Ignore "duplicate column" errors — migration already applied
            let _ = self.conn.execute(sql, []);
        }
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
                                        srs_interval, srs_ease, srs_reps, srs_due_date)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(id) DO UPDATE SET
                confidence = excluded.confidence,
                line_uci = excluded.line_uci,
                opening_name = excluded.opening_name,
                srs_interval = excluded.srs_interval,
                srs_ease = excluded.srs_ease,
                srs_reps = excluded.srs_reps,
                srs_due_date = excluded.srs_due_date",
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
                line.srs_due_date
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
}
