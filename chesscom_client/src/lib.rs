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
