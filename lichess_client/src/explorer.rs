use crate::LichessClient;
use serde::Deserialize;

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
    pub async fn explorer_masters(&self, fen: &str) -> Result<ExplorerResponse, String> {
        let url = format!(
            "https://explorer.lichess.ovh/masters?fen={}&moves=10",
            urlencoding::encode(fen)
        );

        let resp = self
            .http
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
