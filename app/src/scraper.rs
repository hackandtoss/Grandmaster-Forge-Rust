use db_manager::OpeningNode;
use lichess_client::LichessClient;
use std::collections::{HashSet, VecDeque};

/// BFS-expand the opening tree from starting position up to `max_depth` half-moves.
/// Only expands moves with >= `min_games` master games.
/// Returns all nodes to insert — does NOT hold a DB lock during HTTP calls.
pub async fn collect_opening_nodes(
    client: &LichessClient,
    max_depth: u32,
    min_games: i64,
) -> Result<Vec<OpeningNode>, String> {
    let start_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

    let mut queue: VecDeque<(String, Option<String>, u32)> = VecDeque::new();
    queue.push_back((start_fen.to_string(), None, 0));

    let mut seen: HashSet<String> = HashSet::new();
    let mut all_nodes: Vec<OpeningNode> = Vec::new();

    while let Some((fen, parent_id, depth)) = queue.pop_front() {
        if depth >= max_depth || seen.contains(&fen) {
            continue;
        }
        seen.insert(fen.clone());

        let resp = match client.explorer_masters(&fen).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("explorer error at depth {depth}: {e}");
                continue;
            }
        };

        for mv in &resp.moves {
            let total = mv.white + mv.draws + mv.black;
            if total < min_games {
                continue;
            }

            let next_fen = match apply_uci_to_fen(&fen, &mv.uci) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("move error {}: {e}", mv.uci);
                    continue;
                }
            };

            let node_id = format!(
                "{}_{}",
                parent_id.as_deref().unwrap_or("root"),
                mv.uci
            );

            all_nodes.push(OpeningNode {
                id: node_id.clone(),
                parent_id: parent_id.clone(),
                move_uci: mv.uci.clone(),
                move_san: mv.san.clone(),
                fen: next_fen.clone(),
                eco: resp.opening.as_ref().and_then(|o| o.eco.clone()),
                opening_name: resp.opening.as_ref().and_then(|o| o.name.clone()),
                master_games: total,
                white_wins: mv.white,
                draws: mv.draws,
                black_wins: mv.black,
            });

            queue.push_back((next_fen, Some(node_id), depth + 1));
        }

        // Rate-limit: Lichess explorer requests
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    Ok(all_nodes)
}

fn apply_uci_to_fen(fen: &str, uci: &str) -> Result<String, String> {
    use shakmaty::{CastlingMode, Chess, EnPassantMode, Position};
    use shakmaty::fen::Fen;
    use shakmaty::uci::Uci;

    let parsed_fen: Fen = fen.parse().map_err(|e| format!("bad fen: {e}"))?;
    let pos: Chess = parsed_fen
        .into_position(CastlingMode::Standard)
        .map_err(|e| format!("illegal position: {e}"))?;
    let uci_move: Uci = uci.parse().map_err(|e| format!("bad uci: {e}"))?;
    let m = uci_move
        .to_move(&pos)
        .map_err(|e| format!("illegal move: {e}"))?;
    let next_pos = pos.play(&m).map_err(|e| format!("play error: {e}"))?;
    Ok(Fen::from_position(next_pos, EnPassantMode::Always).to_string())
}

/// Pick which explorer moves to adopt at this node.
/// My turn: the single most-played move. Opponent turn: the top_n most-played.
pub fn select_adoption_moves(
    moves: &[lichess_client::explorer::ExplorerMove],
    is_my_turn: bool,
    top_n: usize,
) -> Vec<String> {
    let mut sorted: Vec<&lichess_client::explorer::ExplorerMove> = moves.iter().collect();
    sorted.sort_by_key(|m| std::cmp::Reverse(m.white + m.draws + m.black));
    let take = if is_my_turn { 1 } else { top_n };
    sorted.into_iter().take(take).map(|m| m.uci.clone()).collect()
}

/// Walk the masters explorer from start_fen, returning (parent_fen, uci) pairs to adopt.
pub async fn adopt_explorer_lines(
    client: &lichess_client::LichessClient,
    start_fen: &str,
    my_side: &str,
    depth: u32,
    top_n: usize,
) -> Result<Vec<(String, String)>, String> {
    use shakmaty::{CastlingMode, Position};
    let mut out: Vec<(String, String)> = Vec::new();
    // frontier of (full_fen, remaining_depth)
    let mut frontier: Vec<(String, u32)> = vec![(start_fen.to_string(), depth)];
    while let Some((fen, remaining)) = frontier.pop() {
        if remaining == 0 { continue; }
        let resp = client.explorer_masters(&fen).await?;
        if resp.moves.is_empty() { continue; }
        let parsed: shakmaty::fen::Fen = fen.parse().map_err(|e| format!("{e}"))?;
        let pos: shakmaty::Chess = parsed
            .into_position(CastlingMode::Standard)
            .map_err(|e| format!("{e}"))?;
        let is_my_turn = (pos.turn() == shakmaty::Color::White) == (my_side == "White");
        for uci_str in select_adoption_moves(&resp.moves, is_my_turn, top_n) {
            let uci: shakmaty::uci::Uci = match uci_str.parse() { Ok(u) => u, Err(_) => continue };
            let m = match uci.to_move(&pos) { Ok(m) => m, Err(_) => continue };
            let next = match pos.clone().play(&m) { Ok(p) => p, Err(_) => continue };
            let next_fen = shakmaty::fen::Fen::from_position(next, shakmaty::EnPassantMode::Legal).to_string();
            out.push((fen.clone(), uci_str));
            frontier.push((next_fen, remaining - 1));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adoption_selection_respects_turn() {
        use lichess_client::explorer::ExplorerMove;
        let mk = |uci: &str, games: i64| ExplorerMove {
            uci: uci.into(), san: uci.into(),
            white: games, draws: 0, black: 0, average_rating: None,
        };
        let moves = vec![mk("e2e4", 100), mk("d2d4", 300), mk("c2c4", 50)];
        assert_eq!(select_adoption_moves(&moves, true, 3), vec!["d2d4"]);
        assert_eq!(select_adoption_moves(&moves, false, 2), vec!["d2d4", "e2e4"]);
    }
}
