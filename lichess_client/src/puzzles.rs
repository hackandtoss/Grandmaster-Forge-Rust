use crate::LichessClient;
use serde::Deserialize;
use shakmaty::fen::Fen;
use shakmaty::san::San;
use shakmaty::{Chess, EnPassantMode, Position};

/// One puzzle from `GET /api/puzzle/next`. The live response carries no FEN:
/// the position to solve is reached by replaying `game.pgn` through
/// `puzzle.initialPly` — see [`puzzle_fen_from_pgn`].
#[derive(Debug, Deserialize, Clone)]
pub struct LichessPuzzle {
    pub puzzle: PuzzleData,
    pub game: PuzzleGame,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PuzzleData {
    pub id: String,
    /// UCI moves starting with the solver's move (the side to move in the
    /// derived puzzle position), alternating with scripted opponent replies.
    pub solution: Vec<String>,
    pub themes: Vec<String>,
    pub rating: i32,
    /// 0-based index of the last pre-puzzle move in `game.pgn`.
    pub initial_ply: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PuzzleGame {
    /// Bare space-separated SAN tokens (no move numbers, headers, or result),
    /// ending on the move that creates the puzzle position.
    pub pgn: String,
}

/// Derive the FEN of the position to solve by replaying `initial_ply + 1` SAN
/// tokens of `pgn` from the standard start. The side to move in the returned
/// FEN is the solver, matching the stored puzzle convention (FEN = position
/// to solve, `solution_uci` from the solver's side).
pub fn puzzle_fen_from_pgn(pgn: &str, initial_ply: u32) -> Result<String, String> {
    let need = initial_ply as usize + 1;
    let mut pos = Chess::default();
    let mut played = 0usize;
    for token in pgn.split_whitespace().take(need) {
        let san: San = token
            .parse()
            .map_err(|e| format!("ply {played}: bad SAN {token}: {e}"))?;
        let m = san
            .to_move(&pos)
            .map_err(|e| format!("ply {played}: illegal move {token}: {e}"))?;
        pos.play_unchecked(&m);
        played += 1;
    }
    if played < need {
        return Err(format!("PGN ends after {played} plies, need {need}"));
    }
    Ok(Fen::from_position(pos, EnPassantMode::Legal).to_string())
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

        let puzzle: LichessPuzzle = resp.json().await.map_err(|e| format!("parse error: {e}"))?;
        Ok(puzzle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shakmaty::uci::Uci;
    use shakmaty::{CastlingMode, Color};

    /// Real response captured from `GET https://lichess.org/api/puzzle/next`
    /// on 2026-07-10, verbatim including the fields we do not deserialize.
    const NEXT_PUZZLE_FIXTURE: &str = r#"{"game":{"id":"6EG5pJyR","perf":{"key":"classical","name":"Classical"},"rated":true,"players":[{"name":"Pindakees","id":"pindakees","color":"white","rating":1800},{"name":"alaamir","id":"alaamir","color":"black","rating":1724}],"pgn":"d4 d5 c4 c6 Nc3 Nf6 Nf3 Bg4 e3 e6 cxd5 cxd5 Bb5+ Nbd7 h3 Bh5 O-O a6 Be2 Bb4 Bd2 O-O a3 Bxc3 bxc3 b5 Rb1 Ne4 Be1 f5 a4 f4 axb5 fxe3 fxe3 axb5 Bxb5 Bg6 Ra1 Nb6 Bd3 Qc7 Rxa8 Rxa8 Qc2 Na4 c4 Ng3 Bxg3 Qxg3 Bxg6 Qxg6 Qxg6 hxg6 Ne5 dxc4 Nxc4 Nc3 Ne5 Ne2+ Kf2 Ra2 Kf3 Nc3 Nxg6 Ra7 Kf4","clock":"15+15"},"puzzle":{"id":"1CwqP","rating":1292,"plays":24611,"solution":["a7f7","f4e5","f7f1"],"themes":["endgame","short","advantage","skewer"],"initialPly":66}}"#;

    #[test]
    fn derives_known_fen_from_scholars_mate_pgn() {
        let fen = puzzle_fen_from_pgn("e4 e5 Bc4 Nc6 Qh5 Nf6", 5).unwrap();
        assert_eq!(
            fen,
            "r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 4 4"
        );
    }

    #[test]
    fn pgn_shorter_than_initial_ply_is_an_error() {
        let err = puzzle_fen_from_pgn("e4 e5", 5).unwrap_err();
        assert!(err.contains("need 6"), "{err}");
    }

    #[test]
    fn illegal_san_is_an_error() {
        assert!(puzzle_fen_from_pgn("e4 Ke5", 1).is_err());
        assert!(puzzle_fen_from_pgn("not_a_move", 0).is_err());
    }

    #[test]
    fn live_fixture_deserializes_and_solution_replays_from_derived_fen() {
        let parsed: LichessPuzzle = serde_json::from_str(NEXT_PUZZLE_FIXTURE).unwrap();
        assert_eq!(parsed.puzzle.id, "1CwqP");
        assert_eq!(parsed.puzzle.rating, 1292);
        assert_eq!(parsed.puzzle.initial_ply, 66);
        assert_eq!(parsed.puzzle.solution, ["a7f7", "f4e5", "f7f1"]);
        assert!(parsed.puzzle.themes.iter().any(|t| t == "skewer"));

        let fen = puzzle_fen_from_pgn(&parsed.game.pgn, parsed.puzzle.initial_ply).unwrap();
        let mut pos: Chess = fen
            .parse::<Fen>()
            .unwrap()
            .into_position(CastlingMode::Standard)
            .unwrap();
        // The solver is the side to move, and the full solution line must
        // replay legally from the derived position.
        assert_eq!(pos.turn(), Color::Black);
        for uci_str in &parsed.puzzle.solution {
            let m = uci_str
                .parse::<Uci>()
                .unwrap()
                .to_move(&pos)
                .unwrap_or_else(|e| panic!("solution move {uci_str} illegal: {e}"));
            pos.play_unchecked(&m);
        }
    }
}
