use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MistakeClass {
    Inaccuracy,
    Mistake,
    Blunder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedGame {
    pub headers: HashMap<String, String>,
    pub moves: Vec<String>,
    pub result: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MistakeEvent {
    pub ply: u32,
    pub played_move: String,
    pub centipawn_loss: i32,
    pub class: MistakeClass,
}

pub fn parse_pgn(text: &str) -> ParsedGame {
    let headers = parse_headers(text);
    let moves = parse_san_moves(text);
    let result = headers
        .get("Result")
        .cloned()
        .or_else(|| parse_result_token(text));

    ParsedGame {
        headers,
        moves,
        result,
    }
}

pub fn first_critical_mistake(moves: &[String], losses: &[i32]) -> Option<MistakeEvent> {
    let idx = losses.iter().position(|loss| classify_centipawn_loss(*loss).is_some())?;
    let class = classify_centipawn_loss(losses[idx])?;
    let played_move = moves.get(idx)?.clone();

    Some(MistakeEvent {
        ply: idx as u32 + 1,
        played_move,
        centipawn_loss: losses[idx],
        class,
    })
}

pub fn game_to_fen_and_uci(moves: &[String]) -> Result<Vec<(String, String)>, String> {
    use shakmaty::{Chess, Position, san::San, uci::Uci};
    let mut pos = Chess::default();
    let mut result = Vec::new();
    for san_str in moves {
        let fen_before = shakmaty::fen::Fen::from_position(pos.clone(), shakmaty::EnPassantMode::Always).to_string();
        let san: San = san_str.parse().map_err(|e| format!("invalid SAN move '{san_str}': {e}"))?;
        let m = san.to_move(&pos).map_err(|e| format!("illegal move '{san_str}' in pos {fen_before}: {e}"))?;
        let uci_move = Uci::from_move(&m, shakmaty::CastlingMode::Standard).to_string();
        pos = pos.play(&m).map_err(|e| format!("failed to play move '{san_str}' in pos {fen_before}: {e}"))?;
        result.push((fen_before, uci_move));
    }
    Ok(result)
}

pub fn classify_centipawn_loss(loss: i32) -> Option<MistakeClass> {
    if loss >= 250 {
        Some(MistakeClass::Blunder)
    } else if loss >= 120 {
        Some(MistakeClass::Mistake)
    } else if loss >= 60 {
        Some(MistakeClass::Inaccuracy)
    } else {
        None
    }
}

fn parse_headers(text: &str) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
            continue;
        }

        let body = trimmed.trim_start_matches('[').trim_end_matches(']');
        let mut split = body.splitn(2, ' ');
        let key = split.next().unwrap_or_default().trim();
        let value = split.next().unwrap_or_default().trim().trim_matches('"');

        if !key.is_empty() {
            headers.insert(key.to_string(), value.to_string());
        }
    }
    headers
}

fn parse_san_moves(text: &str) -> Vec<String> {
    let mut in_tag = false;
    let mut in_comment = false;
    let mut in_variation = 0usize;
    let mut current = String::new();
    let mut body = String::new();

    for ch in text.chars() {
        match ch {
            '[' => in_tag = true,
            ']' if in_tag => {
                in_tag = false;
                continue;
            }
            '{' => in_comment = true,
            '}' if in_comment => {
                in_comment = false;
                continue;
            }
            '(' => in_variation += 1,
            ')' if in_variation > 0 => {
                in_variation -= 1;
                continue;
            }
            _ => {}
        }

        if in_tag || in_comment || in_variation > 0 {
            continue;
        }

        body.push(ch);
    }

    let mut moves = Vec::new();
    for token in body.split_whitespace() {
        if token.contains('.') || is_result_token(token) {
            continue;
        }

        current.clear();
        current.push_str(token.trim());
        if !current.is_empty() {
            moves.push(current.clone());
        }
    }

    moves
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalkedMove {
    pub parent_fen: String,
    pub child_fen: String,
    pub uci: String,
    pub san: String,
}

/// Normalized FEN used as a repertoire-tree key (must match db_manager::normalize_fen).
pub fn position_key(pos: &shakmaty::Chess) -> String {
    use shakmaty::EnPassantMode;
    let fen = shakmaty::fen::Fen::from_position(pos.clone(), EnPassantMode::Legal).to_string();
    let fields: Vec<&str> = fen.split_whitespace().take(4).collect();
    format!("{} 0 1", fields.join(" "))
}

/// Split a multi-game PGN on [Event headers.
pub fn split_games(text: &str) -> Vec<String> {
    let mut games: Vec<String> = Vec::new();
    let mut current = String::new();
    for line in text.lines() {
        if line.trim_start().starts_with("[Event ") && !current.trim().is_empty() {
            games.push(std::mem::take(&mut current));
        }
        current.push_str(line);
        current.push('\n');
    }
    if !current.trim().is_empty() {
        games.push(current);
    }
    games
}

/// Walk every game and every nested variation; emit one WalkedMove per legal move.
/// Illegal moves abort only the branch they occur in.
pub fn walk_pgn_variations(text: &str) -> Result<Vec<WalkedMove>, String> {
    use shakmaty::san::San;
    use shakmaty::uci::Uci;
    use shakmaty::{Chess, Position};

    if text.trim().is_empty() {
        return Err("empty PGN input".to_string());
    }

    let mut out = Vec::new();
    for game_text in split_games(text) {
        // Movetext = everything that is not a header line.
        let movetext: String = game_text
            .lines()
            .filter(|l| !l.trim_start().starts_with('['))
            .collect::<Vec<_>>()
            .join(" ");

        let mut pos = Chess::default();
        let mut prev: Option<Chess> = None;
        // Each '(' pushes (position_to_return_to, prev_at_that_point).
        let mut stack: Vec<(Chess, Option<Chess>)> = Vec::new();
        let mut dead_branch_depth: Option<usize> = None; // skip tokens after illegal move until branch closes

        let mut chars = movetext.chars().peekable();
        let mut token = String::new();
        let flush = |token: &mut String,
                     pos: &mut Chess,
                     prev: &mut Option<Chess>,
                     dead: &mut Option<usize>,
                     depth: usize,
                     out: &mut Vec<WalkedMove>| {
            if token.is_empty() {
                return;
            }
            let raw = std::mem::take(token);
            if dead.is_some() {
                return;
            }
            let t = raw.trim_end_matches(['?', '!']);
            if t.is_empty()
                || t.starts_with('$')
                || (t.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) && t.contains('.'))
                || matches!(t, "1-0" | "0-1" | "1/2-1/2" | "*")
                || t.chars().all(|c| c.is_ascii_digit())
            {
                return;
            }
            match t.parse::<San>() {
                Ok(san) => match san.to_move(pos) {
                    Ok(m) => {
                        let parent = position_key(pos);
                        let uci = Uci::from_move(&m, shakmaty::CastlingMode::Standard).to_string();
                        let san_str = San::from_move(pos, &m).to_string();
                        let next = pos.clone().play(&m).expect("legal move plays");
                        *prev = Some(pos.clone());
                        *pos = next;
                        out.push(WalkedMove {
                            parent_fen: parent,
                            child_fen: position_key(pos),
                            uci,
                            san: san_str,
                        });
                    }
                    Err(_) => *dead = Some(depth),
                },
                Err(_) => *dead = Some(depth),
            }
        };

        while let Some(ch) = chars.next() {
            match ch {
                '{' => {
                    flush(&mut token, &mut pos, &mut prev, &mut dead_branch_depth, stack.len(), &mut out);
                    for c in chars.by_ref() {
                        if c == '}' {
                            break;
                        }
                    }
                }
                ';' => {
                    flush(&mut token, &mut pos, &mut prev, &mut dead_branch_depth, stack.len(), &mut out);
                    for c in chars.by_ref() {
                        if c == '\n' {
                            break;
                        }
                    }
                }
                '(' => {
                    flush(&mut token, &mut pos, &mut prev, &mut dead_branch_depth, stack.len(), &mut out);
                    stack.push((pos.clone(), prev.clone()));
                    if dead_branch_depth.is_none() {
                        if let Some(p) = prev.clone() {
                            pos = p; // variation replaces the last played move
                        }
                    }
                }
                ')' => {
                    flush(&mut token, &mut pos, &mut prev, &mut dead_branch_depth, stack.len(), &mut out);
                    if let Some((restored, restored_prev)) = stack.pop() {
                        if let Some(d) = dead_branch_depth {
                            if stack.len() < d {
                                dead_branch_depth = None;
                            }
                        }
                        pos = restored;
                        prev = restored_prev;
                    }
                }
                c if c.is_whitespace() => {
                    flush(&mut token, &mut pos, &mut prev, &mut dead_branch_depth, stack.len(), &mut out);
                }
                c => token.push(c),
            }
        }
        flush(&mut token, &mut pos, &mut prev, &mut dead_branch_depth, 0, &mut out);
    }
    Ok(out)
}

fn is_result_token(token: &str) -> bool {
    matches!(token, "1-0" | "0-1" | "1/2-1/2" | "*")
}

fn parse_result_token(text: &str) -> Option<String> {
    text.split_whitespace()
        .find(|token| is_result_token(token))
        .map(|token| token.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_headers_and_moves() {
        let pgn = r#"
[Event "Rapid"]
[Result "1-0"]

1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 1-0
"#;
        let game = parse_pgn(pgn);
        assert_eq!(game.headers.get("Event"), Some(&"Rapid".to_string()));
        assert_eq!(game.moves, vec!["e4", "e5", "Nf3", "Nc6", "Bb5", "a6"]);
        assert_eq!(game.result, Some("1-0".to_string()));
    }

    #[test]
    fn finds_first_critical_mistake() {
        let moves = vec!["e4".into(), "e5".into(), "Qh5".into()];
        let losses = vec![8, 30, 170];
        let critical = first_critical_mistake(&moves, &losses).expect("expected mistake");
        assert_eq!(critical.ply, 3);
        assert_eq!(critical.class, MistakeClass::Mistake);
    }

    #[test]
    fn converts_game_moves_to_fen_and_uci() {
        let moves = vec!["e4".to_string(), "e5".to_string(), "Nf3".to_string()];
        let fen_and_uci = game_to_fen_and_uci(&moves).unwrap();
        assert_eq!(fen_and_uci.len(), 3);
        assert_eq!(fen_and_uci[0].0, "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        assert_eq!(fen_and_uci[0].1, "e2e4");
        assert_eq!(fen_and_uci[1].1, "e7e5");
        assert_eq!(fen_and_uci[2].1, "g1f3");
    }

    #[test]
    fn walks_nested_variations() {
        let pgn = r#"
[Event "Repertoire"]

1. e4 e5 (1... c5 2. Nf3 (2. Nc3 Nc6) 2... d6) 2. Nf3 Nc6 1-0
"#;
        let walked = walk_pgn_variations(pgn).unwrap();
        // Mainline: e4 e5 Nf3 Nc6 = 4. Var A: c5 Nf3 d6 = 3. Var B (nested): Nc3 Nc6 = 2.
        assert_eq!(walked.len(), 9);
        let sans: Vec<&str> = walked.iter().map(|w| w.san.as_str()).collect();
        assert!(sans.contains(&"c5"));
        assert!(sans.contains(&"Nc3"));
        // The two 2.Nf3 moves come from different parents (after e5 vs after c5)
        let nf3: Vec<&WalkedMove> = walked.iter().filter(|w| w.uci == "g1f3").collect();
        assert_eq!(nf3.len(), 2);
        assert_ne!(nf3[0].parent_fen, nf3[1].parent_fen);
    }

    #[test]
    fn walks_multiple_games_and_skips_malformed() {
        let pgn = r#"
[Event "One"]

1. d4 d5 1/2-1/2

[Event "Two"]

1. e4 Qxe4 1-0

[Event "Three"]

1. c4 e5 *
"#;
        let walked = walk_pgn_variations(pgn).unwrap();
        // Game Two dies at illegal Qxe4 after emitting e4; Games One and Three emit 2 each.
        let sans: Vec<&str> = walked.iter().map(|w| w.san.as_str()).collect();
        assert!(sans.contains(&"d4"));
        assert!(sans.contains(&"c4"));
        assert_eq!(walked.len(), 5);
    }

    #[test]
    fn strips_comments_nags_and_annotations() {
        let pgn = "[Event \"X\"]\n\n1. e4! {best by test} $1 e5?? 2. Nf3 1-0";
        let walked = walk_pgn_variations(pgn).unwrap();
        assert_eq!(walked.len(), 3);
        assert_eq!(walked[0].san, "e4");
        assert_eq!(walked[1].san, "e5");
    }
}
