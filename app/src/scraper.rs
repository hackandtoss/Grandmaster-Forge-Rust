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
