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
    pub async fn fetch_puzzle(&self) -> Result<LichessPuzzle, String> {
        let url = "https://lichess.org/api/puzzle/next";

        let resp = self
            .http
            .get(url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("Lichess API error: {}", resp.status()));
        }

        let puzzle: LichessPuzzle = resp
            .json()
            .await
            .map_err(|e| format!("parse error: {e}"))?;
        Ok(puzzle)
    }
}
