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
}
