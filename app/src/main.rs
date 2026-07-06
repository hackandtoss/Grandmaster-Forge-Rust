mod srs;
mod accuracy;
mod weakness;
mod scraper;
mod tree;

use db_manager::{GameRecord, PositionRecord, SqliteStore, TrainingEventRecord, TrainingStore};
use engine_controller::{AnalysisConfig, StockfishEngine};
use shakmaty::{CastlingMode, Chess, Position};
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

        // Status / analysis properties
        in-out property <string> weakness-summary: "";
        in-out property <string> sync-status: "Not synced";
        in-out property <string> tree-status: "";

        // Repertoire builder state
        in-out property <string> builder-fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        in-out property <string> builder-moves: "";
        in-out property <string> builder-color: "White";
        in-out property <string> builder-line-name: "";
        in-out property <int> builder-selected-square: -1;
        in-out property <[string]> builder-board-pieces: [
            "♜","♞","♝","♛","♚","♝","♞","♜",
            "♟","♟","♟","♟","♟","♟","♟","♟",
            "","","","","","","","",
            "","","","","","","","",
            "","","","","","","","",
            "","","","","","","","",
            "♙","♙","♙","♙","♙","♙","♙","♙",
            "♖","♘","♗","♕","♔","♗","♘","♖"
        ];

        // Drill branch info
        in-out property <string> drill-branch-info: "";

        // Callbacks
        callback select-screen(string);
        callback import-pgn(string);
        callback select-game(string);
        callback select-game-move(int);
        callback select-repertoire-line(string);
        callback click-board-square(int);
        callback start-drill();
        callback sync-lichess(string);
        callback build-opening-tree();
        callback load-puzzle();
        callback explorer-select-move(string);

        // Repertoire builder callbacks
        callback builder-click-square(int);
        callback builder-save-line();
        callback builder-reset();
        callback builder-set-color(string);
        callback builder-undo();
        callback suggest-pgn-lines();

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
                        SidebarButton {
                            text: "Opening Builder";
                            active: root.active-screen == "builder";
                            clicked => { root.select-screen("builder"); }
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

                // 4. Opening Builder Screen
                if (root.active-screen == "builder") : HorizontalLayout {
                    spacing: 16px;
                    padding: 0px;

                    // Left: board + controls
                    VerticalLayout {
                        width: 480px;
                        spacing: 12px;

                        // Color picker
                        HorizontalLayout {
                            spacing: 8px;
                            Text { text: "Train as:"; color: #a1a1aa; font-size: 13px; vertical-alignment: center; }
                            Rectangle {
                                width: 80px; height: 32px;
                                background: root.builder-color == "White" ? #6d28d9 : #27272a;
                                border-radius: 6px;
                                TouchArea { clicked => { root.builder-set-color("White"); } }
                                Text { text: "White"; color: white; font-size: 13px; horizontal-alignment: center; vertical-alignment: center; }
                            }
                            Rectangle {
                                width: 80px; height: 32px;
                                background: root.builder-color == "Black" ? #6d28d9 : #27272a;
                                border-radius: 6px;
                                TouchArea { clicked => { root.builder-set-color("Black"); } }
                                Text { text: "Black"; color: white; font-size: 13px; horizontal-alignment: center; vertical-alignment: center; }
                            }
                        }

                        // 8×8 board
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
                                background: (root.builder-selected-square == index)
                                    ? #7c3aed
                                    : (mod(floor(index / 8) + mod(index, 8), 2) == 0 ? #f0d9b5 : #b58863);
                                Text {
                                    text: root.builder-board-pieces[index];
                                    font-size: 28px;
                                    color: #000000;
                                    horizontal-alignment: center;
                                    vertical-alignment: center;
                                }
                                TouchArea {
                                    clicked => { root.builder-click-square(index); }
                                }
                            }
                        }

                        // Undo / Reset buttons
                        HorizontalLayout {
                            spacing: 8px;
                            Rectangle {
                                height: 36px; border-radius: 6px; background: #374151;
                                TouchArea { clicked => { root.builder-undo(); } }
                                Text { text: "Undo"; color: white; font-size: 13px; horizontal-alignment: center; vertical-alignment: center; }
                            }
                            Rectangle {
                                height: 36px; border-radius: 6px; background: #374151;
                                TouchArea { clicked => { root.builder-reset(); } }
                                Text { text: "Reset"; color: white; font-size: 13px; horizontal-alignment: center; vertical-alignment: center; }
                            }
                        }
                    }

                    // Right: line name + move list + save + PGN suggest
                    VerticalLayout {
                        spacing: 12px;
                        alignment: start;

                        Text { text: "Line Name"; color: #a1a1aa; font-size: 12px; }
                        LineEdit {
                            height: 36px;
                            placeholder-text: "e.g. Ruy Lopez — White";
                            text <=> root.builder-line-name;
                        }

                        Text { text: "Moves Staged"; color: #a1a1aa; font-size: 12px; }
                        Rectangle {
                            height: 120px;
                            background: #1c1c1e;
                            border-radius: 8px;
                            border-color: #3f3f46;
                            border-width: 1px;
                            Text {
                                text: root.builder-moves == "" ? "Play moves on the board…" : root.builder-moves;
                                color: root.builder-moves == "" ? #71717a : #e4e4e7;
                                font-size: 13px;
                                wrap: word-wrap;
                            }
                        }

                        // Save button
                        Rectangle {
                            height: 42px;
                            background: @linear-gradient(90deg, #6d28d9, #4f46e5);
                            border-radius: 8px;
                            TouchArea { clicked => { root.builder-save-line(); } }
                            Text {
                                text: "Save Line to Repertoire";
                                color: white;
                                font-size: 14px;
                                font-weight: 700;
                                horizontal-alignment: center;
                                vertical-alignment: center;
                            }
                        }

                        // Suggest from PGN
                        Rectangle {
                            height: 36px;
                            background: #27272a;
                            border-radius: 8px;
                            TouchArea { clicked => { root.suggest-pgn-lines(); } }
                            Text {
                                text: "Suggest Lines from Last PGN";
                                color: #a78bfa;
                                font-size: 13px;
                                horizontal-alignment: center;
                                vertical-alignment: center;
                            }
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
    // Tree-based drill session
    drill_side: String,                 // "White" | "Black"
    drill_chess: Chess,                 // current drill position
    drill_edge_ids: Vec<i64>,           // my-move edges already graded this session
    drill_expected: Vec<db_manager::RepertoireMoveRecord>, // my-move edges at current node
    drill_start_edge: Option<db_manager::RepertoireMoveRecord>,
    // Current board FEN for explorer
    current_fen: Option<String>,
    // Repertoire builder state
    builder_chess: Chess,
    builder_staged_uci: Vec<String>,
    builder_selected_sq: Option<usize>,
    builder_color: String,
}

fn main() {
    dotenvy::dotenv().ok();
    let stockfish_path = find_stockfish_path();

    // Initialize database
    let mut db = SqliteStore::new("forge.db").expect("failed to open database file forge.db");
    db.bootstrap().expect("failed to bootstrap database tables");
    db.run_migrations().expect("failed to run database migrations");
    let migrated = tree::migrate_legacy_lines(&mut db).unwrap_or_else(|e| {
        eprintln!("legacy line migration failed (continuing on tree): {e}");
        0
    });
    if migrated > 0 {
        println!("Migrated {migrated} legacy opening lines into the repertoire tree.");
    }

    // Load initial data
    let state = Arc::new(Mutex::new(AppState {
        db,
        stockfish_path,
        drill_side: "White".to_string(),
        drill_chess: Chess::default(),
        drill_edge_ids: Vec::new(),
        drill_expected: Vec::new(),
        drill_start_edge: None,
        current_fen: None,
        builder_chess: Chess::default(),
        builder_staged_uci: Vec::new(),
        builder_selected_sq: None,
        builder_color: "White".to_string(),
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
            let today = tree::local_now_str();
            let snapshot = mastery_ui::UserSnapshot {
                opening_due: state.db.get_due_repertoire_move_count(&today).unwrap_or(0),
                blunder_hotspots: state.db.get_blunder_hotspots_count().unwrap_or(0),
                puzzle_rating: state.db.get_puzzle_rating("default_user").unwrap_or(1500),
                has_unreviewed_games: state.db.get_unreviewed_games_count().unwrap_or(0) > 0,
            };

            // Weakness profile
            let all_positions: Vec<(u32, Option<i32>, Option<String>, Option<String>)> = {
                let mut stmt = state.db.conn.prepare(
                    "SELECT p.ply, p.centipawn_loss, p.mistake_class, g.eco
                     FROM positions p JOIN games g ON g.id = p.game_id"
                ).unwrap();
                stmt.query_map([], |row| {
                    Ok((
                        row.get::<_, u32>(0)?,
                        row.get::<_, Option<i32>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                }).unwrap().filter_map(|r| r.ok()).collect()
            };
            let avg_endgame_acc = state.db.conn.query_row(
                "SELECT COALESCE(AVG(accuracy_endgame), 100.0) FROM games WHERE accuracy_endgame IS NOT NULL",
                [], |row| row.get::<_, f64>(0),
            ).unwrap_or(100.0) as f32;
            let wr = weakness::build_weakness_report(&all_positions, avg_endgame_acc);
            app.set_weakness_summary(slint::SharedString::from(wr.top_training_priority));
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

            // Refresh due repertoire moves (drill queue)
            let due = state.db.get_due_repertoire_moves(&today).unwrap_or_default();
            let line_entries: Vec<OpeningLineEntry> = due.iter().map(|e| OpeningLineEntry {
                id: slint::SharedString::from(e.id.to_string()),
                name: slint::SharedString::from(format!("{} ({})", e.san, e.source)),
                moves: slint::SharedString::from(e.uci.clone()),
                confidence: (e.srs_reps as f32 / 10.0).min(1.0),
                start_fen: slint::SharedString::from(e.parent_fen.clone()),
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
    let refresh_data = std::sync::Arc::new(refresh_data);
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

    // Wire repertoire edge selection for drilling (id is a repertoire_moves edge id)
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_select_repertoire_line(move |edge_id: slint::SharedString| {
            let mut state = state.lock().unwrap();
            let app = app_weak.upgrade().unwrap();
            let id: i64 = match edge_id.parse() { Ok(v) => v, Err(_) => return };
            if let Ok(Some(edge)) = state.db.get_repertoire_move(id) {
                app.set_drill_line_id(edge_id);
                app.set_drill_name(slint::SharedString::from(format!("Drill: {} ({})", edge.san, edge.source)));
                app.set_drill_instructions(slint::SharedString::from("Press 'Start Drill' to practice from this position."));
                app.set_drill_active(false);
                app.set_board_pieces(slint::ModelRc::from(Rc::new(slint::VecModel::from(fen_to_pieces(&edge.parent_fen)))));
                app.set_board_selected_square(-1);
                state.drill_start_edge = Some(edge);
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
            let Some(edge) = state.drill_start_edge.clone() else { return };
            let Ok(Some((side, fen))) = state.db.get_node_side_and_fen(edge.parent_id) else { return };
            let parsed: shakmaty::fen::Fen = match fen.parse() { Ok(f) => f, Err(_) => return };
            let Ok(pos) = parsed.into_position(CastlingMode::Standard) else { return };
            state.drill_side = side;
            state.drill_chess = pos;
            state.drill_edge_ids.clear();
            app.set_drill_active(true);
            drill_advance(&mut state, &app); // shared session-step helper
        });
    }

    // Click square on interactive board (drills): accept ANY expected my-move edge
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        let refresh_data_cp = refresh_data.clone();
        app.on_click_board_square(move |idx: i32| {
            let mut state = state.lock().unwrap();
            let app = app_weak.upgrade().unwrap();
            if !app.get_drill_active() { return; }
            let sel = app.get_board_selected_square();
            if sel == -1 { app.set_board_selected_square(idx); return; }
            app.set_board_selected_square(-1);
            let uci_try = format!("{}{}", index_to_square(sel as usize), index_to_square(idx as usize));

            let today_days = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() / 86400;

            let matched = state.drill_expected.iter()
                .find(|e| e.uci == uci_try || e.uci == format!("{}q", uci_try))
                .cloned();

            if let Some(edge) = matched {
                let _ = tree::grade_edge(&mut state.db, &edge, 5, today_days);
                state.drill_edge_ids.push(edge.id);
                let event = TrainingEventRecord {
                    id: format!("evt_{}", uuid_now()),
                    user_id: "default_user".to_string(),
                    kind: "repertoire_drill".to_string(),
                    target_id: format!("{} {}", edge.san, edge.uci),
                    outcome: "Correct".to_string(),
                    score_delta: 15.0,
                    created_at: tree::local_now_str(),
                };
                let _ = state.db.insert_training_event(&event);
                if let Ok(uci) = edge.uci.parse::<Uci>() {
                    if let Ok(m) = uci.to_move(&state.drill_chess) {
                        state.drill_chess.play_unchecked(&m);
                        let fen = shakmaty::fen::Fen::from_position(
                            state.drill_chess.clone(), shakmaty::EnPassantMode::Legal).to_string();
                        app.set_board_pieces(slint::ModelRc::from(Rc::new(
                            slint::VecModel::from(fen_to_pieces(&fen)))));
                    }
                }
                drill_advance(&mut state, &app);
                if !app.get_drill_active() { drop(state); refresh_data_cp(); }
            } else {
                // Wrong move: fail every expected edge at this node (they were all "the answer")
                for edge in state.drill_expected.clone() {
                    let _ = tree::grade_edge(&mut state.db, &edge, 1, today_days);
                }
                let event = TrainingEventRecord {
                    id: format!("evt_{}", uuid_now()),
                    user_id: "default_user".to_string(),
                    kind: "repertoire_drill".to_string(),
                    target_id: uci_try,
                    outcome: "Failed".to_string(),
                    score_delta: -5.0,
                    created_at: tree::local_now_str(),
                };
                let _ = state.db.insert_training_event(&event);
                app.set_drill_instructions(slint::SharedString::from(
                    "Not in your repertoire here — try again (this move is now due for review)."));
                drop(state);
                refresh_data_cp();
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

                // Compute and persist phase-based accuracy
                let acc_input: Vec<(u32, Option<i32>)> = positions
                    .iter()
                    .zip(losses.iter())
                    .map(|(pos, &loss)| (pos.ply, Some(loss)))
                    .collect();
                let acc = accuracy::compute_game_accuracy(&acc_input);
                {
                    let mut state = state_thread.lock().unwrap();
                    let _ = state.db.update_game_accuracy(
                        &game_id_thread,
                        acc.overall,
                        acc.opening,
                        acc.middlegame,
                        acc.endgame,
                    );
                }

                // Identify first critical mistake
                if let Some(critical) = pgn_processor::first_critical_mistake(&parsed_moves, &losses) {
                    // Look up FEN of the blunder position
                    if let Some(pos_record) = positions.get((critical.ply as usize).saturating_sub(1)) {
                        // Re-evaluate to get best line
                        if let Ok(analysis) = sf.analyze_fen(&pos_record.fen, AnalysisConfig { depth: 10, multipv: 1 }) {
                            if !analysis.variations.is_empty() {
                                let uci_line: Vec<String> = analysis.variations[0].pv.iter().take(6).cloned().collect();
                                let mover_is_white = pos_record.fen.split_whitespace().nth(1) == Some("w");
                                let side = if mover_is_white { "White" } else { "Black" };
                                let mut state = state_thread.lock().unwrap();
                                let _ = tree::import_uci_line(&mut state.db, &pos_record.fen, &uci_line, side, "mistake");

                                // Add a notification log
                                let event = TrainingEventRecord {
                                    id: format!("evt_{}", uuid_now()),
                                    user_id: "default_user".to_string(),
                                    kind: "game_review".to_string(),
                                    target_id: game_id_thread.clone(),
                                    outcome: format!("{:?}", critical.class),
                                    score_delta: 0.0,
                                    created_at: tree::local_now_str(),
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
            refresh_data_cp();
        });
    }

    // Lichess sync callback
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        let refresh_data_cp = refresh_data.clone();
        app.on_sync_lichess(move |username: slint::SharedString| {
            let username = username.to_string();
            let state = state.clone();
            let app_weak = app_weak.clone();
            let refresh_data_cp = refresh_data_cp.clone();
            std::thread::spawn(move || {
                // Status: syncing (upgrade happens on event-loop thread)
                let aw = app_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = aw.upgrade() {
                        app.set_sync_status(slint::SharedString::from("Syncing…"));
                    }
                });
                let token = std::env::var("LICHESS_API_KEY").unwrap_or_default();
                let client = lichess_client::LichessClient::new(&token);
                let rt = tokio::runtime::Runtime::new().unwrap();
                let games = rt.block_on(client.fetch_user_games(&username, 50));
                let msg = match games {
                    Ok(games) => {
                        let count = games.len();
                        let mut st = state.lock().unwrap();
                        for g in &games {
                            if !st.db.is_game_synced(&g.id) {
                                let _ = st.db.mark_game_synced(&g.id, &tree::local_now_str());
                            }
                        }
                        drop(st);
                        // refresh_data is Rc (non-Send), invoke on event loop thread
                        let _ = slint::invoke_from_event_loop(move || { refresh_data_cp(); });
                        format!("Synced {} games", count)
                    }
                    Err(e) => format!("Sync failed: {}", e),
                };
                let aw = app_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = aw.upgrade() {
                        app.set_sync_status(slint::SharedString::from(msg));
                    }
                });
            });
        });
    }

    // Build opening tree callback
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_build_opening_tree(move || {
            let state = state.clone();
            let app_weak = app_weak.clone();
            std::thread::spawn(move || {
                let aw = app_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = aw.upgrade() {
                        app.set_tree_status(slint::SharedString::from("Building tree…"));
                    }
                });
                let token = std::env::var("LICHESS_API_KEY").unwrap_or_default();
                let client = lichess_client::LichessClient::new(&token);
                let rt = tokio::runtime::Runtime::new().unwrap();
                let result = rt.block_on(scraper::collect_opening_nodes(&client, 4, 1000));
                let msg = match result {
                    Ok(nodes) => {
                        let count = nodes.len();
                        let mut st = state.lock().unwrap();
                        for node in nodes {
                            let _ = st.db.insert_opening_node(&node);
                        }
                        drop(st);
                        format!("Tree built: {} nodes", count)
                    }
                    Err(e) => format!("Tree error: {}", e),
                };
                let aw = app_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = aw.upgrade() {
                        app.set_tree_status(slint::SharedString::from(msg));
                    }
                });
            });
        });
    }

    // Load puzzle callback
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_load_puzzle(move || {
            let st = state.lock().unwrap();
            if let Ok(Some(puzzle)) = st.db.get_next_puzzle(2000) {
                let fen = puzzle.fen.clone();
                drop(st);
                if let Some(app) = app_weak.upgrade() {
                    let pieces = fen_to_pieces(&fen);
                    app.set_board_pieces(slint::ModelRc::new(slint::VecModel::from(
                        pieces.iter().map(|s| slint::SharedString::from(s.as_str())).collect::<Vec<_>>(),
                    )));
                    app.set_drill_instructions(slint::SharedString::from(
                        format!("Puzzle: find the best move ({})", puzzle.id),
                    ));
                }
            } else {
                drop(st);
            }
        });
    }

    // Opening explorer move selection callback
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_explorer_select_move(move |uci: slint::SharedString| {
            let st = state.lock().unwrap();
            let fen = st.current_fen.clone().unwrap_or_else(|| {
                "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string()
            });
            drop(st);
            use shakmaty::{CastlingMode, Chess, EnPassantMode, Position};
            use shakmaty::fen::Fen;
            use shakmaty::uci::Uci;
            let new_fen = (|| -> Result<String, String> {
                let parsed: Fen = fen.parse().map_err(|e| format!("{e}"))?;
                let pos: Chess = parsed.into_position(CastlingMode::Standard).map_err(|e| format!("{e}"))?;
                let uci_move: Uci = uci.to_string().parse().map_err(|e| format!("{e}"))?;
                let m = uci_move.to_move(&pos).map_err(|e| format!("{e}"))?;
                let next = pos.play(&m).map_err(|e| format!("{e}"))?;
                Ok(Fen::from_position(next, EnPassantMode::Always).to_string())
            })();
            if let Ok(new_fen) = new_fen {
                if let Some(app) = app_weak.upgrade() {
                    let pieces = fen_to_pieces(&new_fen);
                    app.set_board_pieces(slint::ModelRc::new(slint::VecModel::from(
                        pieces.iter().map(|s| slint::SharedString::from(s.as_str())).collect::<Vec<_>>(),
                    )));
                }
            }
        });
    }

    // ── Repertoire Builder callbacks ─────────────────────────────────────────

    // builder-click-square: two-click move entry on the builder board
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_builder_click_square(move |sq_idx: i32| {
            let sq = sq_idx as usize;
            let mut st = state.lock().unwrap();
            let app = app_weak.upgrade().unwrap();

            if let Some(from) = st.builder_selected_sq {
                // Second click: attempt the move
                let from_sq = index_to_square(from);
                let to_sq = index_to_square(sq);
                let uci = format!("{}{}", from_sq, to_sq);
                use shakmaty::{EnPassantMode, Position};
                use shakmaty::uci::Uci;
                let uci_move: Uci = match uci.parse() {
                    Ok(u) => u,
                    Err(_) => { st.builder_selected_sq = None; app.set_builder_selected_square(-1); return; }
                };
                match uci_move.to_move(&st.builder_chess) {
                    Ok(m) => {
                        st.builder_chess.play_unchecked(&m);
                        st.builder_staged_uci.push(uci.clone());
                        let new_fen = shakmaty::fen::Fen::from_position(
                            st.builder_chess.clone(), EnPassantMode::Always,
                        ).to_string();
                        let pieces = fen_to_pieces(&new_fen);
                        app.set_builder_board_pieces(slint::ModelRc::from(
                            Rc::new(slint::VecModel::from(pieces)),
                        ));
                        app.set_builder_fen(slint::SharedString::from(new_fen));
                        app.set_builder_moves(slint::SharedString::from(
                            st.builder_staged_uci.join(" "),
                        ));
                    }
                    Err(_) => {}
                }
                st.builder_selected_sq = None;
                app.set_builder_selected_square(-1);
            } else {
                // First click: select a piece
                st.builder_selected_sq = Some(sq);
                app.set_builder_selected_square(sq_idx);
            }
        });
    }

    // builder-undo: pop last staged move and rebuild position from scratch
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_builder_undo(move || {
            let mut st = state.lock().unwrap();
            if st.builder_staged_uci.pop().is_none() { return; }
            // Rebuild position from start
            use shakmaty::{CastlingMode, Position};
            use shakmaty::uci::Uci;
            let start_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
            let parsed: shakmaty::fen::Fen = start_fen.parse().unwrap();
            let mut pos: shakmaty::Chess = parsed.into_position(CastlingMode::Standard).unwrap();
            for uci_str in &st.builder_staged_uci {
                if let Ok(uci_move) = uci_str.parse::<Uci>() {
                    if let Ok(m) = uci_move.to_move(&pos) {
                        pos.play_unchecked(&m);
                    }
                }
            }
            st.builder_chess = pos;
            let new_fen = shakmaty::fen::Fen::from_position(
                st.builder_chess.clone(), shakmaty::EnPassantMode::Always,
            ).to_string();
            let app = app_weak.upgrade().unwrap();
            let pieces = fen_to_pieces(&new_fen);
            app.set_builder_board_pieces(slint::ModelRc::from(Rc::new(slint::VecModel::from(pieces))));
            app.set_builder_fen(slint::SharedString::from(new_fen));
            app.set_builder_moves(slint::SharedString::from(st.builder_staged_uci.join(" ")));
            app.set_builder_selected_square(-1);
            st.builder_selected_sq = None;
        });
    }

    // builder-reset: clear staged moves and board
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_builder_reset(move || {
            let mut st = state.lock().unwrap();
            st.builder_chess = shakmaty::Chess::default();
            st.builder_staged_uci.clear();
            st.builder_selected_sq = None;
            let start_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
            let app = app_weak.upgrade().unwrap();
            let pieces = fen_to_pieces(start_fen);
            app.set_builder_board_pieces(slint::ModelRc::from(Rc::new(slint::VecModel::from(pieces))));
            app.set_builder_fen(slint::SharedString::from(start_fen));
            app.set_builder_moves(slint::SharedString::from(""));
            app.set_builder_line_name(slint::SharedString::from(""));
            app.set_builder_selected_square(-1);
        });
    }

    // builder-set-color
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_builder_set_color(move |color: slint::SharedString| {
            let mut st = state.lock().unwrap();
            st.builder_color = color.to_string();
            drop(st);
            let app = app_weak.upgrade().unwrap();
            app.set_builder_color(color);
        });
    }

    // builder-save-line: persist current staged line to DB
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        let refresh_data_save = refresh_data.clone();
        app.on_builder_save_line(move || {
            let mut st = state.lock().unwrap();
            if st.builder_staged_uci.is_empty() { return; }
            let app = app_weak.upgrade().unwrap();
            let start_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
            let staged = st.builder_staged_uci.clone();
            let side = st.builder_color.clone();
            match tree::import_uci_line(&mut st.db, start_fen, &staged, &side, "manual") {
                Ok(n) => app.set_drill_instructions(slint::SharedString::from(
                    format!("Saved: {n} new move(s) added to your {side} repertoire."))),
                Err(e) => app.set_drill_instructions(slint::SharedString::from(
                    format!("Save failed: {e}"))),
            }
            // Reset builder after save
            st.builder_chess = shakmaty::Chess::default();
            st.builder_staged_uci.clear();
            st.builder_selected_sq = None;
            drop(st);
            app.set_builder_moves(slint::SharedString::from(""));
            app.set_builder_line_name(slint::SharedString::from(""));
            let pieces = fen_to_pieces("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
            app.set_builder_board_pieces(slint::ModelRc::from(Rc::new(slint::VecModel::from(pieces))));
            refresh_data_save();
        });
    }

    // suggest-pgn-lines: extract opening moves from the most recent imported game
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_suggest_pgn_lines(move || {
            let st = state.lock().unwrap();
            let games = st.db.get_games().unwrap_or_default();
            drop(st);
            let app = app_weak.upgrade().unwrap();
            if let Some(game) = games.first() {
                // Parse PGN, convert first 15 SAN moves to UCI
                let parsed = pgn_processor::parse_pgn(&game.pgn);
                use shakmaty::{CastlingMode, Position};
                use shakmaty::san::San;
                let start_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
                let parsed_start: shakmaty::fen::Fen = start_fen.parse().unwrap();
                let mut suggest_pos: shakmaty::Chess = parsed_start.into_position(CastlingMode::Standard).unwrap();
                let mut first_15: Vec<String> = Vec::new();
                for san_str in parsed.moves.iter().take(15) {
                    let san_str = san_str.trim_end_matches(['?', '!', '+', '#']);
                    if let Ok(san) = san_str.parse::<San>() {
                        if let Ok(m) = san.to_move(&suggest_pos) {
                            let uci = shakmaty::uci::Uci::from_move(&m, shakmaty::CastlingMode::Standard);
                            first_15.push(uci.to_string());
                            suggest_pos.play_unchecked(&m);
                        } else { break; }
                    } else { break; }
                }
                if !first_15.is_empty() {
                    app.set_builder_moves(slint::SharedString::from(first_15.join(" ")));
                    app.set_builder_line_name(slint::SharedString::from(
                        format!("{} vs {} (opening)", game.white, game.black),
                    ));
                    // Load the suggested moves into builder state
                    let mut st2 = state.lock().unwrap();
                    use shakmaty::{CastlingMode, Position};
                    use shakmaty::uci::Uci;
                    let start_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
                    let parsed_fen: shakmaty::fen::Fen = start_fen.parse().unwrap();
                    let mut pos: shakmaty::Chess = parsed_fen.into_position(CastlingMode::Standard).unwrap();
                    let mut valid: Vec<String> = Vec::new();
                    for uci_str in &first_15 {
                        if let Ok(uci_move) = uci_str.parse::<Uci>() {
                            if let Ok(m) = uci_move.to_move(&pos) {
                                pos.play_unchecked(&m);
                                valid.push(uci_str.clone());
                            } else { break; }
                        } else { break; }
                    }
                    st2.builder_chess = pos.clone();
                    st2.builder_staged_uci = valid;
                    drop(st2);
                    let fen = shakmaty::fen::Fen::from_position(pos, shakmaty::EnPassantMode::Always).to_string();
                    let pieces = fen_to_pieces(&fen);
                    app.set_builder_board_pieces(slint::ModelRc::from(Rc::new(slint::VecModel::from(pieces))));
                    app.set_builder_fen(slint::SharedString::from(fen));
                }
            } else {
                app.set_builder_line_name(slint::SharedString::from("No games imported yet"));
            }
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

/// Advance the drill: auto-play opponent edges, load expected my-moves, detect session end.
fn drill_advance(state: &mut AppState, app: &AppWindow) {
    loop {
        let key = tree::position_key(&state.drill_chess);
        let edges = state.db.get_repertoire_moves_from(&key, &state.drill_side).unwrap_or_default();
        let (mine, theirs): (Vec<_>, Vec<_>) = edges.into_iter().partition(|e| e.is_my_move);

        let side_to_move_is_mine =
            (state.drill_chess.turn() == shakmaty::Color::White) == (state.drill_side == "White");

        if side_to_move_is_mine {
            if mine.is_empty() {
                app.set_drill_active(false);
                app.set_drill_instructions(slint::SharedString::from("Line complete — well done!"));
                return;
            }
            app.set_drill_branch_info(slint::SharedString::from(
                format!("{} accepted move(s) here", mine.len())));
            app.set_drill_instructions(slint::SharedString::from("Your move."));
            state.drill_expected = mine;
            return;
        }
        // Opponent's turn: auto-play a random opponent edge, or end if none.
        if theirs.is_empty() {
            app.set_drill_active(false);
            app.set_drill_instructions(slint::SharedString::from("Line complete — well done!"));
            return;
        }
        let pick = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as usize % theirs.len();
        let mv = &theirs[pick];
        if let Ok(uci) = mv.uci.parse::<shakmaty::uci::Uci>() {
            if let Ok(m) = uci.to_move(&state.drill_chess) {
                state.drill_chess.play_unchecked(&m);
                let fen = shakmaty::fen::Fen::from_position(
                    state.drill_chess.clone(), shakmaty::EnPassantMode::Legal).to_string();
                app.set_board_pieces(slint::ModelRc::from(std::rc::Rc::new(
                    slint::VecModel::from(fen_to_pieces(&fen)))));
                continue;
            }
        }
        app.set_drill_active(false);
        return;
    }
}

// Temporary alias to bridge name discrepancy in shakmaty FEN parsing
mod sk_fen {
    pub use shakmaty::fen::Fen;
}
