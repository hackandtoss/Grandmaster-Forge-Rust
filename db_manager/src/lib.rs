pub const SQLITE_SCHEMA: &[&str] = &[
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

pub trait TrainingStore {
    fn bootstrap(&mut self) -> Result<(), String>;
    fn insert_opening_line(&mut self, line: &OpeningLineRecord) -> Result<(), String>;
    fn insert_position(&mut self, position: &PositionRecord) -> Result<(), String>;
    fn insert_training_event(&mut self, event: &TrainingEventRecord) -> Result<(), String>;
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
}
