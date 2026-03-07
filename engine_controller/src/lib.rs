use shakmaty::{Board, Chess, Position, Move, Square, Piece, Color, Role};
use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader, Write};

pub struct Chessboard {
    board: Board,
}

impl Chessboard {
    pub fn new() -> Self {
        Chessboard {
            board: Board::default(),
        }
    }

    pub fn display(&self) {
        println!("{}", self.board.board_fen(shakmaty::fen::FenOpts::default()));
    }

    pub fn make_move(&mut self, mv: Move) -> Result<(), String> {
        if self.board.is_legal(&mv) {
            self.board.play_unchecked(&mv);
            Ok(())
        } else {
            Err("Illegal move".to_string())
        }
    }

    pub fn get_board(&self) -> &Board {
        &self.board
    }
}

pub struct StockfishEngine {
    process: std::process::Child,
}

impl StockfishEngine {
    pub fn new() -> Result<Self, String> {
        let process = Command::new("stockfish")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start Stockfish: {}", e))?;

        let mut engine = StockfishEngine { process };

        // Initialize UCI
        engine.send_command("uci")?;
        engine.wait_for_ready()?;

        Ok(engine)
    }

    fn send_command(&mut self, cmd: &str) -> Result<(), String> {
        if let Some(ref mut stdin) = self.process.stdin {
            writeln!(stdin, "{}", cmd).map_err(|e| format!("Failed to send command: {}", e))?;
            Ok(())
        } else {
            Err("No stdin".to_string())
        }
    }

    fn wait_for_ready(&mut self) -> Result<(), String> {
        if let Some(ref mut stdout) = self.process.stdout {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let line = line.map_err(|e| format!("Read error: {}", e))?;
                if line == "readyok" {
                    return Ok(());
                }
            }
        }
        Err("Did not receive readyok".to_string())
    }

    pub fn set_position(&mut self, fen: &str) -> Result<(), String> {
        self.send_command(&format!("position fen {}", fen))?;
        self.send_command("isready")?;
        self.wait_for_ready()
    }

    pub fn go(&mut self, time: u32) -> Result<String, String> {
        self.send_command(&format!("go movetime {}", time))?;
        if let Some(ref mut stdout) = self.process.stdout {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let line = line.map_err(|e| format!("Read error: {}", e))?;
                if line.starts_with("bestmove") {
                    return Ok(line);
                }
            }
        }
        Err("No bestmove received".to_string())
    }
}

impl Drop for StockfishEngine {
    fn drop(&mut self) {
        let _ = self.send_command("quit");
        let _ = self.process.wait();
    }
}

pub fn initialize() {
    // TODO: implement engine startup and command handling
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chessboard_creation() {
        let board = Chessboard::new();
        assert_eq!(board.get_board().board_fen(shakmaty::fen::FenOpts::default()), "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR");
    }
}
