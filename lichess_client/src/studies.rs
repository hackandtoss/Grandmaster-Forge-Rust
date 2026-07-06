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
