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

        let resp = self
            .http
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
