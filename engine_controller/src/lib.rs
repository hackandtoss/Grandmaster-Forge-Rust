use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MistakeClass {
    Inaccuracy,
    Mistake,
    Blunder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Score {
    Centipawns(i32),
    Mate(i32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variation {
    pub multipv: u32,
    pub depth: u32,
    pub score: Score,
    pub pv: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisResult {
    pub bestmove: String,
    pub variations: Vec<Variation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnalysisConfig {
    pub depth: u32,
    pub multipv: u32,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            depth: 20,
            multipv: 3,
        }
    }
}

#[derive(Debug, Clone)]
struct VariationBuilder {
    multipv: u32,
    depth: u32,
    score: Score,
    pv: Vec<String>,
}

pub struct StockfishEngine {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl StockfishEngine {
    pub fn new(binary_path: &str) -> Result<Self, String> {
        let mut child = Command::new(binary_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| format!("failed to start Stockfish: {e}"))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Stockfish stdin was unavailable".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Stockfish stdout was unavailable".to_string())?;

        let mut engine = Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        };

        engine.send_command("uci")?;
        engine.wait_for_line_exact("uciok")?;
        engine.send_command("isready")?;
        engine.wait_for_line_exact("readyok")?;

        Ok(engine)
    }

    pub fn analyze_fen(
        &mut self,
        fen: &str,
        cfg: AnalysisConfig,
    ) -> Result<AnalysisResult, String> {
        self.send_command(&format!("setoption name MultiPV value {}", cfg.multipv))?;
        self.send_command("isready")?;
        self.wait_for_line_exact("readyok")?;

        self.send_command(&format!("position fen {fen}"))?;
        self.send_command(&format!("go depth {}", cfg.depth))?;

        let mut buf = String::new();
        let mut bestmove = String::new();
        let mut map: BTreeMap<u32, VariationBuilder> = BTreeMap::new();

        loop {
            buf.clear();
            let read = self
                .stdout
                .read_line(&mut buf)
                .map_err(|e| format!("failed reading engine output: {e}"))?;

            if read == 0 {
                return Err("engine exited before returning bestmove".to_string());
            }

            let line = buf.trim();
            if line.starts_with("info ") {
                if let Some(v) = parse_info_line(line) {
                    map.insert(v.multipv, v);
                }
                continue;
            }

            if line.starts_with("bestmove ") {
                if let Some(token) = line.split_whitespace().nth(1) {
                    bestmove = token.to_string();
                }
                break;
            }
        }

        if bestmove.is_empty() {
            return Err("engine did not report a valid bestmove".to_string());
        }

        let mut variations = map
            .into_values()
            .map(|v| Variation {
                multipv: v.multipv,
                depth: v.depth,
                score: v.score,
                pv: v.pv,
            })
            .collect::<Vec<_>>();
        variations.sort_by_key(|v| v.multipv);

        Ok(AnalysisResult {
            bestmove,
            variations,
        })
    }

    fn send_command(&mut self, command: &str) -> Result<(), String> {
        writeln!(self.stdin, "{command}")
            .and_then(|_| self.stdin.flush())
            .map_err(|e| format!("failed sending command '{command}': {e}"))
    }

    fn wait_for_line_exact(&mut self, needle: &str) -> Result<(), String> {
        let mut buf = String::new();
        loop {
            buf.clear();
            let read = self
                .stdout
                .read_line(&mut buf)
                .map_err(|e| format!("failed reading engine output: {e}"))?;
            if read == 0 {
                return Err(format!("engine exited before '{needle}'"));
            }
            if buf.trim() == needle {
                return Ok(());
            }
        }
    }
}

impl Drop for StockfishEngine {
    fn drop(&mut self) {
        let _ = self.send_command("quit");
        let _ = self.child.wait();
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PlayConfig {
    pub elo: Option<u32>,
    pub movetime_ms: u32,
}

impl Default for PlayConfig {
    fn default() -> Self {
        Self { elo: None, movetime_ms: 400 }
    }
}

/// A persistent engine process for playing full games (one `position … moves` per turn).
pub struct PlaySession {
    engine: StockfishEngine,
    movetime_ms: u32,
}

impl PlaySession {
    pub fn new(binary_path: &str, cfg: PlayConfig) -> Result<Self, String> {
        let mut engine = StockfishEngine::new(binary_path)?;
        if let Some(elo) = cfg.elo {
            engine.send_command("setoption name UCI_LimitStrength value true")?;
            engine.send_command(&format!("setoption name UCI_Elo value {}", elo.clamp(1320, 3190)))?;
            engine.send_command("isready")?;
            engine.wait_for_line_exact("readyok")?;
        }
        Ok(Self { engine, movetime_ms: cfg.movetime_ms })
    }

    pub fn best_move_from(&mut self, moves_uci: &[String]) -> Result<String, String> {
        let pos_cmd = if moves_uci.is_empty() {
            "position startpos".to_string()
        } else {
            format!("position startpos moves {}", moves_uci.join(" "))
        };
        self.engine.send_command(&pos_cmd)?;
        self.engine.send_command(&format!("go movetime {}", self.movetime_ms))?;
        let mut buf = String::new();
        loop {
            buf.clear();
            let read = self
                .engine
                .stdout
                .read_line(&mut buf)
                .map_err(|e| format!("failed reading engine output: {e}"))?;
            if read == 0 {
                return Err("engine exited during play".to_string());
            }
            let line = buf.trim();
            if line == "bestmove" || line.starts_with("bestmove ") {
                return line
                    .split_whitespace()
                    .nth(1)
                    .map(|s| s.to_string())
                    .ok_or_else(|| "malformed bestmove".to_string());
            }
        }
    }
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

fn parse_info_line(line: &str) -> Option<VariationBuilder> {
    let tokens = line.split_whitespace().collect::<Vec<_>>();
    let mut multipv = 1;
    let mut depth = 0;
    let mut score = None;
    let mut pv = Vec::new();

    let mut i = 0usize;
    while i < tokens.len() {
        match tokens[i] {
            "depth" if i + 1 < tokens.len() => {
                depth = tokens[i + 1].parse().ok()?;
                i += 2;
            }
            "multipv" if i + 1 < tokens.len() => {
                multipv = tokens[i + 1].parse().ok()?;
                i += 2;
            }
            "score" if i + 2 < tokens.len() => {
                score = match tokens[i + 1] {
                    "cp" => Some(Score::Centipawns(tokens[i + 2].parse().ok()?)),
                    "mate" => Some(Score::Mate(tokens[i + 2].parse().ok()?)),
                    _ => None,
                };
                i += 3;
            }
            "pv" => {
                pv.extend(tokens[i + 1..].iter().map(|t| (*t).to_string()));
                break;
            }
            _ => i += 1,
        }
    }

    Some(VariationBuilder {
        multipv,
        depth,
        score: score?,
        pv,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_loss_bands() {
        assert_eq!(classify_centipawn_loss(59), None);
        assert_eq!(classify_centipawn_loss(60), Some(MistakeClass::Inaccuracy));
        assert_eq!(classify_centipawn_loss(120), Some(MistakeClass::Mistake));
        assert_eq!(classify_centipawn_loss(250), Some(MistakeClass::Blunder));
    }

    #[test]
    fn parses_uci_info_lines() {
        let line = "info depth 18 multipv 2 score cp -42 pv e2e4 e7e5 g1f3";
        let parsed = parse_info_line(line).expect("expected parse result");
        assert_eq!(parsed.multipv, 2);
        assert_eq!(parsed.depth, 18);
        assert_eq!(parsed.score, Score::Centipawns(-42));
        assert_eq!(parsed.pv, vec!["e2e4", "e7e5", "g1f3"]);
    }

    #[test]
    fn play_session_returns_bestmove_from_mock() {
        // Build/locate mock-stockfish like app::find_stockfish_path does.
        let mut path = std::env::current_dir().unwrap();
        path.pop(); // workspace root from engine_controller/
        path.push("target");
        path.push("debug");
        path.push(if cfg!(windows) { "mock-stockfish.exe" } else { "mock-stockfish" });
        if !path.exists() {
            let _ = std::process::Command::new("cargo")
                .args(["build", "-p", "app", "--bin", "mock-stockfish"])
                .status();
        }
        if !path.exists() {
            eprintln!("mock-stockfish unavailable; skipping");
            return;
        }
        let mut session = PlaySession::new(
            path.to_str().unwrap(),
            PlayConfig { elo: Some(1500), movetime_ms: 50 },
        )
        .unwrap();
        let mv = session
            .best_move_from(&["e2e4".to_string(), "e7e5".to_string()])
            .unwrap();
        assert!(!mv.is_empty());
    }
}
