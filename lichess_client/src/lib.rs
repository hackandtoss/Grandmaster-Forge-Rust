pub mod explorer;
pub mod games;
pub mod puzzles;
pub mod studies;

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
