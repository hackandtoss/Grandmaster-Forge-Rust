use db_manager::{GameRecord, OpeningLineRecord, PositionRecord, SqliteStore, TrainingEventRecord, TrainingStore};
use engine_controller::{AnalysisConfig, StockfishEngine};
use shakmaty::{CastlingMode, Chess, Position};
use shakmaty::san::San;
use shakmaty::uci::Uci;
use slint::Model;
use std::sync::{Arc, Mutex};
use std::thread;

slint::slint! {
    import { Button, ScrollView, LineEdit } from "std-widgets.slint";

    struct GameEntry {
        id: string,
        white: string,
        black: string,
        result: string,
        date: string,
        pgn: string,
    }

    struct OpeningLineEntry {
        id: string,
        name: string,
        moves: string,
        confidence: float,
        start_fen: string,
    }

    struct HistoryEntry {
        kind: string,
        target: string,
        outcome: string,
        delta: float,
        date: string,
    }

    component SidebarButton inherits Rectangle {
        in property <string> text;
        in property <bool> active;
        callback clicked;
        height: 40px;
        background: active ? #2d2d35 : #00000000;
        border-radius: 8px;
        animate background { duration: 150ms; }
        
        TouchArea {
            clicked => { root.clicked(); }
        }
        Text {
            text: root.text;
            color: active ? #ffffff : #a0a0aa;
            font-size: 14px;
            font-weight: active ? 700 : 500;
            x: 16px;
            vertical-alignment: center;
        }
    }

    export component AppWindow inherits Window {
        title: "Grandmaster Forge";
        min-width: 1000px;
        min-height: 700px;
        background: #121214;

        // Properties for dashboard
        in-out property <string> rec-mode: "ReviewRecentGame";
        in-out property <string> rec-reason: "Review your latest game to generate new improvement lines.";
        in-out property <int> stat-unreviewed: 0;
        in-out property <int> stat-blunders: 0;
        in-out property <int> stat-openings: 0;
        in-out property <int> stat-rating: 1500;

        // Lists
        in-out property <[GameEntry]> games: [];
        in-out property <[OpeningLineEntry]> opening-lines: [];
        in-out property <[HistoryEntry]> history: [];

        // Active screen: "dashboard", "repertoire", "review"
        in-out property <string> active-screen: "dashboard";

        // Active game review details
        in-out property <string> selected-game-id: "";
        in-out property <string> selected-game-title: "No Game Selected";
        in-out property <[string]> selected-game-moves: [];
        in-out property <[string]> selected-game-losses: [];
        in-out property <[string]> selected-game-classes: [];
        in-out property <[string]> selected-game-fens: [];
        in-out property <int> selected-move-index: -1;
        in-out property <string> selected-move-eval: "";
        in-out property <string> selected-move-best: "";

        // Interactive board pieces (64 strings)
        in-out property <[string]> board-pieces: [
            "♜", "♞", "♝", "♛", "♚", "♝", "♞", "♜",
            "♟", "♟", "♟", "♟", "♟", "♟", "♟", "♟",
            "", "", "", "", "", "", "", "",
            "", "", "", "", "", "", "", "",
            "", "", "", "", "", "", "", "",
            "", "", "", "", "", "", "", "",
            "♙", "♙", "♙", "♙", "♙", "♙", "♙", "♙",
            "♖", "♘", "♗", "♕", "♔", "♗", "♘", "♖"
        ];
        in-out property <int> board-selected-square: -1;

        // Drill state
        in-out property <string> drill-line-id: "";
        in-out property <string> drill-name: "";
        in-out property <string> drill-instructions: "Select a line to drill.";
        in-out property <bool> drill-active: false;

        // Callbacks
        callback select-screen(string);
        callback import-pgn(string);
        callback select-game(string);
        callback select-game-move(int);
        callback select-repertoire-line(string);
        callback click-board-square(int);
        callback start-drill();

        HorizontalLayout {
            // Sidebar
            Rectangle {
                width: 220px;
                background: #1a1a1e;
                border-color: #27272a;
                border-width: 1px;
                
                VerticalLayout {
                    padding: 16px;
                    spacing: 24px;
                    alignment: start;

                    // Brand Title
                    Text {
                        text: "FORGE MASTER";
                        color: #a78bfa; // light violet
                        font-size: 18px;
                        font-weight: 800;
                        horizontal-alignment: center;
                    }

                    // Navigation Menu
                    VerticalLayout {
                        spacing: 8px;
                        SidebarButton {
                            text: "Dashboard";
                            active: root.active-screen == "dashboard";
                            clicked => { root.select-screen("dashboard"); }
                        }
                        SidebarButton {
                            text: "Repertoire Browser";
                            active: root.active-screen == "repertoire";
                            clicked => { root.select-screen("repertoire"); }
                        }
                        SidebarButton {
                            text: "Game Review";
                            active: root.active-screen == "review";
                            clicked => { root.select-screen("review"); }
                        }
                    }
                }
            }

            // Main Area
            Rectangle {
                background: #121214;
                padding: 24px;

                // 1. Dashboard Screen
                if (root.active-screen == "dashboard") : VerticalLayout {
                    spacing: 24px;
                    alignment: start;

                    // Header Action Card with linear gradient
                    Rectangle {
                        height: 160px;
                        background: @linear-gradient(135deg, #6d28d9 0%, #4f46e5 100%);
                        border-radius: 12px;
                        padding: 20px;

                        VerticalLayout {
                            alignment: space-between;
                            VerticalLayout {
                                spacing: 4px;
                                Text {
                                    text: "DAILY TRAINING RECOMMENDATION";
                                    color: #c084fc;
                                    font-size: 11px;
                                    font-weight: 700;
                                }
                                Text {
                                    text: root.rec-mode == "ReviewRecentGame" ? "Review Your Games" :
                                          root.rec-mode == "BlunderPrevention" ? "Blunder Prevention Session" :
                                          root.rec-mode == "OpeningRepetition" ? "Drill Openings" : "Solve Chess Puzzles";
                                    color: #ffffff;
                                    font-size: 20px;
                                    font-weight: 800;
                                }
                                Text {
                                    text: root.rec-reason;
                                    color: #e4e4e7;
                                    font-size: 13px;
                                }
                            }
                            HorizontalLayout {
                                alignment: start;
                                Button {
                                    text: "Start Activity";
                                    clicked => {
                                        if (root.rec-mode == "ReviewRecentGame") {
                                            root.select-screen("review");
                                        } else if (root.rec-mode == "OpeningRepetition") {
                                            root.select-screen("repertoire");
                                        } else {
                                            root.select-screen("repertoire");
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Stats Grid (4 Cards)
                    HorizontalLayout {
                        spacing: 16px;
                        height: 100px;
                        
                        // Card 1
                        Rectangle {
                            background: #1a1a1e;
                            border-radius: 8px;
                            border-color: #27272a;
                            border-width: 1px;
                            padding: 16px;
                            VerticalLayout {
                                alignment: center;
                                Text { text: "Unreviewed Games"; color: #a1a1aa; font-size: 12px; }
                                Text { text: root.stat-unreviewed; color: #ffffff; font-size: 24px; font-weight: 700; }
                            }
                        }
                        // Card 2
                        Rectangle {
                            background: #1a1a1e;
                            border-radius: 8px;
                            border-color: #27272a;
                            border-width: 1px;
                            padding: 16px;
                            VerticalLayout {
                                alignment: center;
                                Text { text: "Blunder Hotspots"; color: #a1a1aa; font-size: 12px; }
                                Text { text: root.stat-blunders; color: root.stat-blunders > 0 ? #f43f5e : #10b981; font-size: 24px; font-weight: 700; }
                            }
                        }
                        // Card 3
                        Rectangle {
                            background: #1a1a1e;
                            border-radius: 8px;
                            border-color: #27272a;
                            border-width: 1px;
                            padding: 16px;
                            VerticalLayout {
                                alignment: center;
                                Text { text: "Due Openings"; color: #a1a1aa; font-size: 12px; }
                                Text { text: root.stat-openings; color: root.stat-openings > 0 ? #fbbf24 : #10b981; font-size: 24px; font-weight: 700; }
                            }
                        }
                        // Card 4
                        Rectangle {
                            background: #1a1a1e;
                            border-radius: 8px;
                            border-color: #27272a;
                            border-width: 1px;
                            padding: 16px;
                            VerticalLayout {
                                alignment: center;
                                Text { text: "Training Rating"; color: #a1a1aa; font-size: 12px; }
                                Text { text: root.stat-rating; color: #fbbf24; font-size: 24px; font-weight: 700; }
                            }
                        }
                    }

                    // Activity History
                    VerticalLayout {
                        spacing: 12px;
                        Text {
                            text: "Recent Training Log";
                            color: #ffffff;
                            font-size: 16px;
                            font-weight: 700;
                        }

                        Rectangle {
                            background: #1a1a1e;
                            border-radius: 8px;
                            border-color: #27272a;
                            border-width: 1px;
                            height: 200px;
                            padding: 12px;

                            ScrollView {
                                VerticalLayout {
                                    spacing: 8px;
                                    alignment: start;
                                    for entry in root.history: Rectangle {
                                        height: 45px;
                                        background: #27272a;
                                        border-radius: 6px;
                                        padding: 8px;
                                        HorizontalLayout {
                                            alignment: space-between;
                                            VerticalLayout {
                                                alignment: center;
                                                Text { text: entry.target; color: #ffffff; font-size: 13px; font-weight: 700; }
                                                Text { text: entry.date + " • " + entry.kind; color: #a1a1aa; font-size: 11px; }
                                            }
                                            HorizontalLayout {
                                                spacing: 12px;
                                                Text {
                                                    text: entry.outcome;
                                                    color: entry.outcome == "Correct" ? #10b981 : #f43f5e;
                                                    font-size: 13px;
                                                    font-weight: 700;
                                                }
                                                Text {
                                                    text: (entry.delta >= 0.0 ? "+" : "") + entry.delta;
                                                    color: entry.delta >= 0.0 ? #10b981 : #f43f5e;
                                                    font-size: 13px;
                                                    font-weight: 700;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // 2. Repertoire Browser Screen
                if (root.active-screen == "repertoire") : HorizontalLayout {
                    spacing: 24px;

                    // Repertoire Line List (Left Column)
                    VerticalLayout {
                        width: 380px;
                        spacing: 12px;
                        Text {
                            text: "Active Repertoires";
                            color: #ffffff;
                            font-size: 18px;
                            font-weight: 700;
                        }

                        Rectangle {
                            background: #1a1a1e;
                            border-radius: 8px;
                            border-color: #27272a;
                            border-width: 1px;
                            padding: 12px;

                            ScrollView {
                                VerticalLayout {
                                    spacing: 12px;
                                    alignment: start;
                                    for line in root.opening-lines: Rectangle {
                                        background: root.drill-line-id == line.id ? #3f3f46 : #27272a;
                                        border-radius: 8px;
                                        padding: 12px;
                                        height: 90px;
                                        
                                        TouchArea {
                                            clicked => {
                                                root.select-repertoire-line(line.id);
                                            }
                                        }

                                        VerticalLayout {
                                            alignment: space-between;
                                            HorizontalLayout {
                                                alignment: space-between;
                                                Text { text: line.name; color: #ffffff; font-size: 14px; font-weight: 700; }
                                                Text { text: "Conf: " + floor(line.confidence * 100.0) + "%"; color: #a78bfa; font-size: 12px; }
                                            }
                                            Text { text: line.moves; color: #a1a1aa; font-size: 12px; height: 16px; overflow: elide; }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Interactive Chess Board and Drill View (Right Column)
                    VerticalLayout {
                        spacing: 16px;
                        alignment: center;

                        // Chess board container
                        Rectangle {
                            width: 360px;
                            height: 360px;
                            background: #1a1a1e;
                            border-radius: 8px;
                            border-color: #3f3f46;
                            border-width: 2px;

                            for index in 64: Rectangle {
                                x: mod(index, 8) * 45px;
                                y: floor(index / 8) * 45px;
                                width: 45px;
                                height: 45px;
                                background: (root.board-selected-square == index)
                                    ? #fcd34d
                                    : (mod(floor(index / 8) + mod(index, 8), 2) == 0 ? #f0d9b5 : #b58863);

                                Text {
                                    text: root.board-pieces[index];
                                    font-size: 28px;
                                    color: #000000;
                                    horizontal-alignment: center;
                                    vertical-alignment: center;
                                }

                                TouchArea {
                                    clicked => {
                                        root.click-board-square(index);
                                    }
                                }
                            }
                        }

                        // Drill Controls
                        Rectangle {
                            background: #1a1a1e;
                            border-radius: 8px;
                            padding: 16px;
                            width: 360px;

                            VerticalLayout {
                                spacing: 12px;
                                Text {
                                    text: root.drill-name == "" ? "No Line Selected" : root.drill-name;
                                    color: #ffffff;
                                    font-size: 15px;
                                    font-weight: 700;
                                    horizontal-alignment: center;
                                }
                                Text {
                                    text: root.drill-instructions;
                                    color: #e4e4e7;
                                    font-size: 13px;
                                    horizontal-alignment: center;
                                }
                                HorizontalLayout {
                                    alignment: center;
                                    if (!root.drill-active && root.drill-line-id != "") : Button {
                                        text: "Start Drill";
                                        clicked => { root.start-drill(); }
                                    }
                                }
                            }
                        }
                    }
                }

                // 3. Game Review Screen
                if (root.active-screen == "review") : HorizontalLayout {
                    spacing: 24px;

                    // Game Selection & PGN Import (Left Column)
                    VerticalLayout {
                        width: 360px;
                        spacing: 16px;

                        // Import PGN Panel
                        Rectangle {
                            background: #1a1a1e;
                            border-radius: 8px;
                            border-color: #27272a;
                            border-width: 1px;
                            padding: 16px;
                            height: 180px;

                            VerticalLayout {
                                spacing: 12px;
                                Text { text: "Import PGN Archive"; color: #ffffff; font-size: 14px; font-weight: 700; }
                                pgn-input := LineEdit {
                                    placeholder-text: "Paste PGN string here...";
                                }
                                Button {
                                    text: "Import & Analyze Game";
                                    clicked => {
                                        root.import-pgn(pgn-input.text);
                                        pgn-input.text = "";
                                    }
                                }
                            }
                        }

                        // Game List
                        Text {
                            text: "Reviewed Games";
                            color: #ffffff;
                            font-size: 16px;
                            font-weight: 700;
                        }

                        Rectangle {
                            background: #1a1a1e;
                            border-radius: 8px;
                            border-color: #27272a;
                            border-width: 1px;
                            padding: 12px;

                            ScrollView {
                                VerticalLayout {
                                    spacing: 12px;
                                    alignment: start;
                                    for game in root.games: Rectangle {
                                        background: root.selected-game-id == game.id ? #3f3f46 : #27272a;
                                        border-radius: 8px;
                                        padding: 12px;
                                        height: 85px;
                                        
                                        TouchArea {
                                            clicked => {
                                                root.select-game(game.id);
                                            }
                                        }

                                        VerticalLayout {
                                            alignment: space-between;
                                            Text { text: game.white + " vs " + game.black; color: #ffffff; font-size: 13px; font-weight: 700; }
                                            HorizontalLayout {
                                                alignment: space-between;
                                                Text { text: game.date; color: #a1a1aa; font-size: 11px; }
                                                Text { text: game.result; color: #fbbf24; font-size: 11px; font-weight: 700; }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Game Evaluation and Timeline Board (Right Column)
                    if (root.selected-game-id != "") : HorizontalLayout {
                        spacing: 20px;
                        
                        // Board View
                        VerticalLayout {
                            spacing: 12px;
                            alignment: center;
                            
                            Rectangle {
                                width: 360px;
                                height: 360px;
                                background: #1a1a1e;
                                border-radius: 8px;
                                border-color: #3f3f46;
                                border-width: 2px;

                                for index in 64: Rectangle {
                                    x: mod(index, 8) * 45px;
                                    y: floor(index / 8) * 45px;
                                    width: 45px;
                                    height: 45px;
                                    background: (mod(floor(index / 8) + mod(index, 8), 2) == 0 ? #f0d9b5 : #b58863);

                                    Text {
                                        text: root.board-pieces[index];
                                        font-size: 28px;
                                        color: #000000;
                                        horizontal-alignment: center;
                                        vertical-alignment: center;
                                    }
                                }
                            }

                            // Engine Eval Block
                            Rectangle {
                                background: #1a1a1e;
                                border-radius: 8px;
                                padding: 12px;
                                width: 360px;

                                VerticalLayout {
                                    spacing: 4px;
                                    Text { text: "POSITION ANALYSIS"; color: #a1a1aa; font-size: 11px; font-weight: 700; }
                                    Text { text: "Evaluation: " + root.selected-move-eval; color: #ffffff; font-size: 14px; font-weight: 700; }
                                    Text { text: "Best Continuation: " + root.selected-move-best; color: #10b981; font-size: 12px; }
                                }
                            }
                        }

                        // Move Timeline
                        VerticalLayout {
                            width: 240px;
                            spacing: 12px;

                            Text {
                                text: root.selected-game-title;
                                color: #ffffff;
                                font-size: 14px;
                                font-weight: 700;
                                height: 20px;
                                overflow: elide;
                            }

                            Rectangle {
                                background: #1a1a1e;
                                border-radius: 8px;
                                border-color: #27272a;
                                border-width: 1px;
                                padding: 8px;

                                ScrollView {
                                    VerticalLayout {
                                        spacing: 6px;
                                        alignment: start;
                                        for move-str[index] in root.selected-game-moves: Rectangle {
                                            background: root.selected-move-index == index ? #3f3f46 : #27272a;
                                            border-radius: 6px;
                                            height: 40px;
                                            padding: 8px;

                                            TouchArea {
                                                clicked => {
                                                    root.select-game-move(index);
                                                }
                                            }

                                            HorizontalLayout {
                                                alignment: space-between;
                                                Text {
                                                    text: (floor(index / 2) + 1) + ". " + (mod(index, 2) == 0 ? "" : "... ") + move-str;
                                                    color: #ffffff;
                                                    font-size: 13px;
                                                    font-weight: 700;
                                                }
                                                // Blunder indicators
                                                if (root.selected-game-losses[index] != "") : Rectangle {
                                                    background: root.selected-game-classes[index] == "Blunder" ? #f43f5e :
                                                                root.selected-game-classes[index] == "Mistake" ? #f59e0b : #eab308;
                                                    border-radius: 4px;
                                                    padding: 4px;
                                                    height: 20px;
                                                    HorizontalLayout {
                                                        Text {
                                                            text: root.selected-game-classes[index] + " (-" + root.selected-game-losses[index] + ")";
                                                            color: #ffffff;
                                                            font-size: 10px;
                                                            font-weight: 700;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if (root.selected-game-id == "") : VerticalLayout {
                        alignment: center;
                        Text {
                            text: "Select a game from the list to review move evaluations.";
                            color: #a1a1aa;
                            font-size: 14px;
                            horizontal-alignment: center;
                        }
                    }
                }
            }
        }
    }
}

// Convert FEN string to a board setup of 64 characters
fn fen_to_pieces(fen: &str) -> Vec<slint::SharedString> {
    let board_part = fen.split_whitespace().next().unwrap_or(fen);
    let mut pieces = vec![slint::SharedString::from(""); 64];
    let mut square_idx = 0usize;

    for ch in board_part.chars() {
        if ch == '/' {
            continue;
        }
        if let Some(digit) = ch.to_digit(10) {
            square_idx += digit as usize;
        } else {
            let symbol = match ch {
                'P' => "♙", 'N' => "♘", 'B' => "♗", 'R' => "♖", 'Q' => "♕", 'K' => "♔",
                'p' => "♟", 'n' => "♞", 'b' => "♝", 'r' => "♜", 'q' => "♛", 'k' => "♚",
                _ => "",
            };
            if square_idx < 64 {
                pieces[square_idx] = slint::SharedString::from(symbol);
            }
            square_idx += 1;
        }
    }
    pieces
}

// Helper to convert board index (0..63) to chess square name (e.g. e2)
fn index_to_square(idx: usize) -> String {
    let col = idx % 8;
    let row = idx / 8;
    let file = (b'a' + col as u8) as char;
    let rank = (b'8' - row as u8) as char;
    format!("{}{}", file, rank)
}

fn square_to_index(sq: &str) -> Option<usize> {
    if sq.len() < 2 {
        return None;
    }
    let chars: Vec<char> = sq.chars().collect();
    let col = (chars[0] as u8).checked_sub(b'a')? as usize;
    let row = (b'8').checked_sub(chars[1] as u8)? as usize;
    if col < 8 && row < 8 {
        Some(row * 8 + col)
    } else {
        None
    }
}

// Compute dynamic centipawn loss and mistake class
fn score_to_centipawns(score: engine_controller::Score) -> i32 {
    match score {
        engine_controller::Score::Centipawns(cp) => cp,
        engine_controller::Score::Mate(m) => {
            if m > 0 {
                10000 - m * 100
            } else {
                -10000 - m * 100
            }
        }
    }
}

fn find_stockfish_path() -> String {
    if let Ok(path) = std::env::var("STOCKFISH_PATH") {
        if std::path::Path::new(&path).exists() {
            return path;
        }
    }

    // Try target/debug/mock-stockfish.exe or mock-stockfish
    let mut path = std::env::current_dir().unwrap_or_default();
    path.push("target");
    path.push("debug");
    #[cfg(windows)]
    path.push("mock-stockfish.exe");
    #[cfg(not(windows))]
    path.push("mock-stockfish");

    if path.exists() {
        return path.to_string_lossy().to_string();
    }

    // Dynamic compilation trigger
    println!("Mock Stockfish executable not found. Compiling mock Stockfish dynamically...");
    let _ = std::process::Command::new("cargo")
        .args(["build", "--bin", "mock-stockfish"])
        .status();

    if path.exists() {
        return path.to_string_lossy().to_string();
    }

    "mock-stockfish".to_string()
}

struct AppState {
    db: SqliteStore,
    stockfish_path: String,
    // Active drill moves & position
    drill_line: Option<OpeningLineRecord>,
    drill_moves_left: Vec<String>,
    drill_chess: Chess,
}

fn main() {
    dotenvy::dotenv().ok();
    let stockfish_path = find_stockfish_path();

    // Initialize database
    let mut db = SqliteStore::new("forge.db").expect("failed to open database file forge.db");
    db.bootstrap().expect("failed to bootstrap database tables");

    // Load initial data
    let state = Arc::new(Mutex::new(AppState {
        db,
        stockfish_path,
        drill_line: None,
        drill_moves_left: Vec::new(),
        drill_chess: Chess::default(),
    }));

    let app = AppWindow::new().expect("failed to create Slint window");
    let app_weak = app.as_weak();

    // Setup initial data refresh helper
    let refresh_data = {
        let state = state.clone();
        let app_weak = app_weak.clone();
        move || {
            let state = state.lock().unwrap();
            let app = app_weak.upgrade().unwrap();

            // Refresh dashboard
            let snapshot = mastery_ui::UserSnapshot {
                opening_due: state.db.get_opening_due_count("default_user").unwrap_or(0),
                blunder_hotspots: state.db.get_blunder_hotspots_count().unwrap_or(0),
                puzzle_rating: state.db.get_puzzle_rating("default_user").unwrap_or(1500),
                has_unreviewed_games: state.db.get_unreviewed_games_count().unwrap_or(0) > 0,
            };
            let rec = mastery_ui::recommend_next_action(&snapshot);
            app.set_rec_mode(slint::SharedString::from(format!("{:?}", rec.mode)));
            app.set_rec_reason(slint::SharedString::from(rec.reason));
            app.set_stat_unreviewed(snapshot.has_unreviewed_games as i32);
            app.set_stat_blunders(snapshot.blunder_hotspots as i32);
            app.set_stat_openings(snapshot.opening_due as i32);
            app.set_stat_rating(snapshot.puzzle_rating);

            // Refresh game list
            let games = state.db.get_games().unwrap_or_default();
            let game_entries: Vec<GameEntry> = games.into_iter().map(|g| GameEntry {
                id: slint::SharedString::from(g.id),
                white: slint::SharedString::from(g.white),
                black: slint::SharedString::from(g.black),
                result: slint::SharedString::from(g.result.unwrap_or_else(|| "*".to_string())),
                date: slint::SharedString::from(g.played_at.unwrap_or_else(|| "2026-06-12".to_string())),
                pgn: slint::SharedString::from(g.pgn),
            }).collect();
            app.set_games(slint::ModelRc::from(Rc::new(slint::VecModel::from(game_entries))));

            // Refresh opening lines
            let lines = state.db.get_opening_lines("default_user").unwrap_or_default();
            let line_entries: Vec<OpeningLineEntry> = lines.into_iter().map(|l| OpeningLineEntry {
                id: slint::SharedString::from(l.id),
                name: slint::SharedString::from(l.opening_name),
                moves: slint::SharedString::from(db_manager::serialize_line_uci(&l.line_uci)),
                confidence: l.confidence,
                start_fen: slint::SharedString::from(l.start_fen),
            }).collect();
            app.set_opening_lines(slint::ModelRc::from(Rc::new(slint::VecModel::from(line_entries))));

            // Refresh training logs
            let mut stmt = state.db.conn.prepare("SELECT kind, target_id, outcome, score_delta, created_at FROM training_events ORDER BY created_at DESC").unwrap();
            let logs = stmt.query_map([], |row| {
                Ok(HistoryEntry {
                    kind: slint::SharedString::from(row.get::<_, String>(0)?),
                    target: slint::SharedString::from(row.get::<_, String>(1)?),
                    outcome: slint::SharedString::from(row.get::<_, String>(2)?),
                    delta: row.get::<_, f64>(3)? as f32,
                    date: slint::SharedString::from(row.get::<_, String>(4)?),
                })
            }).unwrap();
            let mut log_list = Vec::new();
            for log in logs {
                if let Ok(l) = log { log_list.push(l); }
            }
            app.set_history(slint::ModelRc::from(Rc::new(slint::VecModel::from(log_list))));
        }
    };

    use std::rc::Rc;
    refresh_data();

    // Wiring screen selection
    {
        let app_weak = app_weak.clone();
        app.on_select_screen(move |screen| {
            let app = app_weak.upgrade().unwrap();
            app.set_active_screen(screen);
        });
    }

    // Wiring game selection for Game Review timeline
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_select_game(move |game_id: slint::SharedString| {
            let state = state.lock().unwrap();
            let app = app_weak.upgrade().unwrap();

            let positions = state.db.get_positions_for_game(&game_id).unwrap_or_default();
            let mut moves = Vec::new();
            let mut losses = Vec::new();
            let mut classes = Vec::new();
            let mut fens = Vec::new();

            for pos in &positions {
                moves.push(slint::SharedString::from(pos.played_move.clone()));
                losses.push(slint::SharedString::from(pos.centipawn_loss.map(|l| l.to_string()).unwrap_or_default()));
                classes.push(slint::SharedString::from(pos.mistake_class.clone().unwrap_or_default()));
                fens.push(slint::SharedString::from(pos.fen.clone()));
            }

            app.set_selected_game_id(game_id.clone());
            app.set_selected_game_title(slint::SharedString::from("Timeline"));
            app.set_selected_game_moves(slint::ModelRc::from(Rc::new(slint::VecModel::from(moves))));
            app.set_selected_game_losses(slint::ModelRc::from(Rc::new(slint::VecModel::from(losses))));
            app.set_selected_game_classes(slint::ModelRc::from(Rc::new(slint::VecModel::from(classes))));
            app.set_selected_game_fens(slint::ModelRc::from(Rc::new(slint::VecModel::from(fens))));
            app.set_selected_move_index(-1);

            // Reset board to starting pos
            app.set_board_pieces(slint::ModelRc::from(Rc::new(slint::VecModel::from(fen_to_pieces("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR")))));
        });
    }

    // Wiring move selection inside a game timeline
    {
        let app_weak = app_weak.clone();
        app.on_select_game_move(move |idx: i32| {
            let app = app_weak.upgrade().unwrap();
            app.set_selected_move_index(idx);

            let fens = app.get_selected_game_fens();
            if let Some(fen) = fens.row_data(idx as usize) {
                app.set_board_pieces(slint::ModelRc::from(Rc::new(slint::VecModel::from(fen_to_pieces(&fen)))));

                let class = app.get_selected_game_classes().row_data(idx as usize).unwrap_or_default();
                let loss = app.get_selected_game_losses().row_data(idx as usize).unwrap_or_default();

                if class != "" {
                    app.set_selected_move_eval(slint::SharedString::from(format!("{} (Lost {} cp)", class, loss)));
                } else {
                    app.set_selected_move_eval(slint::SharedString::from("Optimal or Incomplete"));
                }
                app.set_selected_move_best(slint::SharedString::from("Check bestmove in timeline."));
            }
        });
    }

    // Wire repertoire line selection for drilling
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_select_repertoire_line(move |line_id: slint::SharedString| {
            let mut state = state.lock().unwrap();
            let app = app_weak.upgrade().unwrap();

            let lines = state.db.get_opening_lines("default_user").unwrap_or_default();
            if let Some(line) = lines.into_iter().find(|l| line_id == l.id) {
                app.set_drill_line_id(line_id);
                app.set_drill_name(slint::SharedString::from(line.opening_name.clone()));
                app.set_drill_instructions(slint::SharedString::from("Press 'Start Drill' to practice this line."));
                app.set_drill_active(false);
                app.set_board_pieces(slint::ModelRc::from(Rc::new(slint::VecModel::from(fen_to_pieces(&line.start_fen)))));
                app.set_board_selected_square(-1);

                state.drill_line = Some(line);
                state.drill_moves_left = Vec::new();
            }
        });
    }

    // Drill execution callbacks
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_start_drill(move || {
            let mut state = state.lock().unwrap();
            let app = app_weak.upgrade().unwrap();

            if let Some(line) = state.drill_line.clone() {
                app.set_drill_active(true);
                app.set_drill_instructions(slint::SharedString::from("Make the first move of the repertoire line!"));
                
                state.drill_moves_left = line.line_uci.clone();
                let fen = &line.start_fen;
                let parsed_fen: shakmaty::fen::Fen = fen.parse().unwrap();
                state.drill_chess = parsed_fen.into_position(CastlingMode::Standard).unwrap();
                app.set_board_pieces(slint::ModelRc::from(Rc::new(slint::VecModel::from(fen_to_pieces(fen)))));
            }
        });
    }

    // Click square on interactive board (drills)
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        let refresh_data_cp = refresh_data.clone();
        app.on_click_board_square(move |idx: i32| {
            let mut state = state.lock().unwrap();
            let app = app_weak.upgrade().unwrap();

            if !app.get_drill_active() {
                return;
            }

            let sel = app.get_board_selected_square();
            if sel == -1 {
                // First click: select a piece
                app.set_board_selected_square(idx);
            } else {
                // Second click: play move
                app.set_board_selected_square(-1);
                let from_sq = index_to_square(sel as usize);
                let to_sq = index_to_square(idx as usize);
                let uci_try = format!("{}{}", from_sq, to_sq);

                if state.drill_moves_left.is_empty() {
                    return;
                }

                let next_correct_move = state.drill_moves_left[0].clone();

                if uci_try == next_correct_move || format!("{}q", uci_try) == next_correct_move {
                    // Correct! Play the move
                    let san: San = next_correct_move.parse().unwrap_or_else(|_| {
                        // fallback to parse uci as move directly via Uci
                        let uci: Uci = next_correct_move.parse().unwrap();
                        let m = uci.to_move(&state.drill_chess).unwrap();
                        state.drill_chess.play_unchecked(&m);
                        "".parse().unwrap()
                    });
                    
                    if san != "".parse().unwrap() {
                        let m = san.to_move(&state.drill_chess).unwrap();
                        state.drill_chess.play_unchecked(&m);
                    }

                    state.drill_moves_left.remove(0);

                    // Update FEN rendering
                    let new_fen = shakmaty::fen::Fen::from_position(state.drill_chess.clone(), shakmaty::EnPassantMode::Always).to_string();
                    app.set_board_pieces(slint::ModelRc::from(Rc::new(slint::VecModel::from(fen_to_pieces(&new_fen)))));

                    if state.drill_moves_left.is_empty() {
                        // Drill completed!
                        app.set_drill_active(false);
                        app.set_drill_instructions(slint::SharedString::from("Drill completed successfully! Confidence increased."));

                        // Increase opening line confidence
                        if let Some(line) = state.drill_line.clone() {
                            let mut updated = line.clone();
                            updated.confidence = (updated.confidence + 0.2).min(1.0);
                            let _ = state.db.insert_opening_line(&updated);

                            // Insert success log
                            let event = TrainingEventRecord {
                                id: format!("evt_{}", uuid_now()),
                                user_id: "default_user".to_string(),
                                kind: "repertoire_drill".to_string(),
                                target_id: line.opening_name.clone(),
                                outcome: "Correct".to_string(),
                                score_delta: 15.0,
                                created_at: local_now_str(),
                            };
                            let _ = state.db.insert_training_event(&event);
                        }
                        drop(state);
                        refresh_data_cp();
                    } else {
                        // Play opponent's response automatically!
                        let opp_move = state.drill_moves_left.remove(0);
                        
                        let uci: Uci = opp_move.parse().unwrap();
                        let m = uci.to_move(&state.drill_chess).unwrap();
                        state.drill_chess.play_unchecked(&m);

                        let opp_fen = shakmaty::fen::Fen::from_position(state.drill_chess.clone(), shakmaty::EnPassantMode::Always).to_string();
                        app.set_board_pieces(slint::ModelRc::from(Rc::new(slint::VecModel::from(fen_to_pieces(&opp_fen)))));

                        if state.drill_moves_left.is_empty() {
                            // Completed on opponent move
                            app.set_drill_active(false);
                            app.set_drill_instructions(slint::SharedString::from("Drill completed successfully!"));
                            if let Some(line) = state.drill_line.clone() {
                                let mut updated = line.clone();
                                updated.confidence = (updated.confidence + 0.2).min(1.0);
                                let _ = state.db.insert_opening_line(&updated);

                                let event = TrainingEventRecord {
                                    id: format!("evt_{}", uuid_now()),
                                    user_id: "default_user".to_string(),
                                    kind: "repertoire_drill".to_string(),
                                    target_id: line.opening_name.clone(),
                                    outcome: "Correct".to_string(),
                                    score_delta: 15.0,
                                    created_at: local_now_str(),
                                };
                                let _ = state.db.insert_training_event(&event);
                            }
                            drop(state);
                            refresh_data_cp();
                        } else {
                            app.set_drill_instructions(slint::SharedString::from("Opponent responded. Make your next move!"));
                        }
                    }
                } else {
                    app.set_drill_instructions(slint::SharedString::from("Incorrect move! Try again."));
                    
                    // Log fail event
                    if let Some(line) = state.drill_line.clone() {
                        let event = TrainingEventRecord {
                            id: format!("evt_{}", uuid_now()),
                            user_id: "default_user".to_string(),
                            kind: "repertoire_drill".to_string(),
                            target_id: line.opening_name.clone(),
                            outcome: "Failed".to_string(),
                            score_delta: -5.0,
                            created_at: local_now_str(),
                        };
                        let _ = state.db.insert_training_event(&event);
                    }
                    drop(state);
                    refresh_data_cp();
                }
            }
        });
    }

    // Wiring PGN Ingestion & Background analysis
    {
        let state = state.clone();
        let refresh_data_cp = refresh_data.clone();
        app.on_import_pgn(move |pgn_text: slint::SharedString| {
            if pgn_text.trim().is_empty() {
                return;
            }

            let mut state_lock = state.lock().unwrap();
            let parsed = pgn_processor::parse_pgn(&pgn_text);

            let white = parsed.headers.get("White").cloned().unwrap_or_else(|| "WhitePlayer".to_string());
            let black = parsed.headers.get("Black").cloned().unwrap_or_else(|| "BlackPlayer".to_string());
            let date = parsed.headers.get("Date").cloned().unwrap_or_else(|| "2026-06-12".to_string());
            let result_str = parsed.headers.get("Result").cloned().unwrap_or_else(|| "*".to_string());
            let game_id = format!("{}_{}_{}", white, black, date).replace(" ", "_");

            let game = GameRecord {
                id: game_id.clone(),
                source: "Imported PGN".to_string(),
                white: white.clone(),
                black: black.clone(),
                result: Some(result_str),
                eco: parsed.headers.get("ECO").cloned(),
                pgn: pgn_text.to_string(),
                played_at: Some(date),
            };

            let _ = state_lock.db.insert_game(&game);

            // Map moves to FENs and insert
            if let Ok(fens_and_moves) = pgn_processor::game_to_fen_and_uci(&parsed.moves) {
                for (ply, (fen, uci_move)) in fens_and_moves.into_iter().enumerate() {
                    let pos = PositionRecord {
                        id: format!("{}_{}", game_id, ply + 1),
                        game_id: game_id.clone(),
                        ply: (ply + 1) as u32,
                        fen,
                        played_move: uci_move,
                        centipawn_loss: None,
                        mistake_class: None,
                    };
                    let _ = state_lock.db.insert_position(&pos);
                }
            }

            // Trigger background thread to analyze and evaluate this game!
            let sf_path = state_lock.stockfish_path.clone();
            let state_thread = state.clone();
            let refresh_thread = refresh_data_cp.clone();
            let game_id_thread = game_id.clone();
            let parsed_moves = parsed.moves.clone();

            thread::spawn(move || {
                let mut sf = match StockfishEngine::new(&sf_path) {
                    Ok(engine) => engine,
                    Err(e) => {
                        println!("Failed to launch engine: {e}");
                        return;
                    }
                };

                let positions = {
                    let state = state_thread.lock().unwrap();
                    state.db.get_positions_for_game(&game_id_thread).unwrap_or_default()
                };

                let mut losses = Vec::new();

                for pos in &positions {
                    // Analyze position
                    let eval_res = sf.analyze_fen(&pos.fen, AnalysisConfig { depth: 8, multipv: 3 });
                    if let Ok(analysis) = eval_res {
                        if !analysis.variations.is_empty() {
                            let best_score = score_to_centipawns(analysis.variations[0].score);
                            
                            // Find played move score
                            let played_score = if analysis.bestmove == pos.played_move {
                                best_score
                            } else if let Some(v) = analysis.variations.iter().find(|var| !var.pv.is_empty() && var.pv[0] == pos.played_move) {
                                score_to_centipawns(v.score)
                            } else {
                                // Evaluate position after played move
                                let mut played_chess: Chess = pos.fen.parse::<sk_fen::Fen>().map(|f| f.into_position(CastlingMode::Standard).unwrap()).unwrap();
                                let uci: Uci = pos.played_move.parse().unwrap();
                                let m = uci.to_move(&played_chess).unwrap();
                                played_chess.play_unchecked(&m);
                                let next_fen = shakmaty::fen::Fen::from_position(played_chess, shakmaty::EnPassantMode::Always).to_string();

                                if let Ok(next_analysis) = sf.analyze_fen(&next_fen, AnalysisConfig { depth: 8, multipv: 1 }) {
                                    if !next_analysis.variations.is_empty() {
                                        // It's the opponent's turn to move now, so invert evaluation
                                        -score_to_centipawns(next_analysis.variations[0].score)
                                    } else {
                                        best_score
                                    }
                                } else {
                                    best_score
                                }
                            };

                            let loss = (best_score - played_score).max(0);
                            losses.push(loss);

                            let class = engine_controller::classify_centipawn_loss(loss).map(|c| format!("{:?}", c));
                            
                            // Write result back
                            let mut state = state_thread.lock().unwrap();
                            let mut updated_pos = pos.clone();
                            updated_pos.centipawn_loss = Some(loss);
                            updated_pos.mistake_class = class;
                            let _ = state.db.insert_position(&updated_pos);
                        }
                    } else {
                        losses.push(0);
                    }
                }

                // Identify first critical mistake
                if let Some(critical) = pgn_processor::first_critical_mistake(&parsed_moves, &losses) {
                    // Look up FEN of the blunder position
                    if let Some(pos_record) = positions.get((critical.ply as usize).saturating_sub(1)) {
                        // Re-evaluate to get best line
                        if let Ok(analysis) = sf.analyze_fen(&pos_record.fen, AnalysisConfig { depth: 10, multipv: 1 }) {
                            if !analysis.variations.is_empty() {
                                let uci_line = analysis.variations[0].pv.clone();
                                let line_record = OpeningLineRecord {
                                    id: format!("line_{}", game_id_thread),
                                    user_id: "default_user".to_string(),
                                    opening_name: format!("Mistake Correction ({} vs {})", white, black),
                                    start_fen: pos_record.fen.clone(),
                                    line_uci: uci_line,
                                    source: "game_review".to_string(),
                                    confidence: 0.0,
                                };
                                let mut state = state_thread.lock().unwrap();
                                let _ = state.db.insert_opening_line(&line_record);

                                // Add a notification log
                                let event = TrainingEventRecord {
                                    id: format!("evt_{}", uuid_now()),
                                    user_id: "default_user".to_string(),
                                    kind: "game_review".to_string(),
                                    target_id: game_id_thread.clone(),
                                    outcome: format!("{:?}", critical.class),
                                    score_delta: 0.0,
                                    created_at: local_now_str(),
                                };
                                let _ = state.db.insert_training_event(&event);
                            }
                        }
                    }
                }

                // Trigger UI refresh
                let _ = slint::invoke_from_event_loop(move || {
                    refresh_thread();
                });
            });

            // Local updates
            drop(state_lock);
            refresh_data();
        });
    }

    app.run().expect("failed to run Slint application");
}

// Simple unique id generator helper
fn uuid_now() -> String {
    use std::time::SystemTime;
    let since_the_epoch = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros();
    format!("{:x}", since_the_epoch)
}

fn local_now_str() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = secs / 86400;
    let (y, m, d) = days_to_ymd(days);
    format!("{:04}-{:02}-{:02}", y, m, d)
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970u64;
    loop {
        let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let days_in_year = if leap { 366 } else { 365 };
        if days < days_in_year { break; }
        days -= days_in_year;
        year += 1;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let month_days: [u64; 12] = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 1u64;
    for md in &month_days {
        if days < *md { break; }
        days -= *md;
        month += 1;
    }
    (year, month, days + 1)
}

// Temporary alias to bridge name discrepancy in shakmaty FEN parsing
mod sk_fen {
    pub use shakmaty::fen::Fen;
}
