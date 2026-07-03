use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    let mut multipv = 1;

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let trimmed = line.trim();

        if trimmed == "uci" {
            println!("id name MockStockfish");
            println!("id author Antigravity");
            println!("uciok");
        } else if trimmed == "isready" {
            println!("readyok");
        } else if trimmed.starts_with("setoption name MultiPV value") {
            if let Some(val_str) = trimmed.split_whitespace().last() {
                if let Ok(val) = val_str.parse::<u32>() {
                    multipv = val;
                }
            }
        } else if trimmed.starts_with("position") {
            // No action needed for mock
        } else if trimmed.starts_with("go") {
            // Output mock evaluations and PVs based on multipv setting
            for pv in 1..=multipv {
                let score = -10 * (pv as i32 - 1); // Best PV has score 0, next is -10, next is -20, etc.
                let mock_move = match pv {
                    1 => "e2e4",
                    2 => "d2d4",
                    3 => "g1f3",
                    _ => "c2c4",
                };
                println!("info depth 10 multipv {pv} score cp {score} pv {mock_move} g8f6");
            }
            println!("bestmove e2e4");
        } else if trimmed == "quit" {
            break;
        }
        let _ = stdout.flush();
    }
}
