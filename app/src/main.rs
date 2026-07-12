mod accuracy;
mod scraper;
mod srs;
mod tree;
mod weakness;

use db_manager::{
    GameRecord, PositionRecord, PuzzleRecord, SqliteStore, TrainingEventRecord, TrainingStore,
};
use engine_controller::{AnalysisConfig, StockfishEngine};
use shakmaty::san::San;
use shakmaty::uci::Uci;
use shakmaty::{CastlingMode, Chess, Position};
use slint::Model;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread;

slint::slint! {
    import { Button, ScrollView, LineEdit, TextEdit } from "std-widgets.slint";

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
        side: string,
        source: string,
        line_count: int,
    }

    struct CourseDetailEntry {
        id: string,
        title: string,
        meta: string,
        is_chapter: bool,
    }

    struct CourseTargetEntry {
        id: string,
        label: string,
        side: string,
    }

    struct HistoryEntry {
        kind: string,
        target: string,
        outcome: string,
        delta: float,
        date: string,
    }

    struct ReviewStatEntry {
        label: string,
        count: int,
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

    component ChessBoard inherits Rectangle {
        in property <[string]> pieces: [
            "", "", "", "", "", "", "", "",
            "", "", "", "", "", "", "", "",
            "", "", "", "", "", "", "", "",
            "", "", "", "", "", "", "", "",
            "", "", "", "", "", "", "", "",
            "", "", "", "", "", "", "", "",
            "", "", "", "", "", "", "", "",
            "", "", "", "", "", "", "", "",
        ];
        in property <int> selected-square: -1;
        in property <color> highlight-color: #fcd34d;
        in property <bool> interactive: true;
        callback square-clicked(int);

        pure function piece-symbol(piece: string) -> string {
            return piece == "P" || piece == "p" ? "♟"
                : piece == "N" || piece == "n" ? "♞"
                : piece == "B" || piece == "b" ? "♝"
                : piece == "R" || piece == "r" ? "♜"
                : piece == "Q" || piece == "q" ? "♛"
                : piece == "K" || piece == "k" ? "♚"
                : "";
        }

        pure function piece-color(piece: string) -> color {
            return piece == "P" || piece == "N" || piece == "B" || piece == "R" || piece == "Q" || piece == "K"
                ? #f8fafc
                : piece == ""
                    ? #00000000
                    : #111827;
        }

        // Stay square: shrink to the smaller of whatever space the layout
        // gives us, floored at today's fixed size, capped so it doesn't
        // dwarf the surrounding panel on an ultrawide window.
        property <length> board-size: max(360px, min(560px, min(root.width, root.height)));
        property <length> cell-size: root.board-size / 8;

        // max-width/max-height mirror the 560px soft cap in board-size above.
        // Without these, this root Rectangle (not just the inner checkerboard)
        // has no upper bound, so a call site that places ChessBoard directly in
        // a HorizontalLayout with a non-stretchy sibling (e.g. Play vs Bot) would
        // let this panel claim all surplus width on an ultrawide window, growing
        // into a mostly-empty box far past the visible 560px grid.
        min-width: 360px;
        min-height: 360px;
        max-width: 560px;
        max-height: 560px;
        horizontal-stretch: 1;
        vertical-stretch: 1;
        background: #1a1a1e;
        border-radius: 8px;
        border-color: #3f3f46;
        border-width: 2px;

        Rectangle {
            x: (parent.width - root.board-size) / 2;
            y: (parent.height - root.board-size) / 2;
            width: root.board-size;
            height: root.board-size;

            for index in 64: Rectangle {
                x: mod(index, 8) * root.cell-size;
                y: floor(index / 8) * root.cell-size;
                width: root.cell-size;
                height: root.cell-size;
                background: (root.selected-square == index)
                    ? root.highlight-color
                    : (mod(floor(index / 8) + mod(index, 8), 2) == 0 ? #f0d9b5 : #b58863);

                Text {
                    text: root.piece-symbol(root.pieces[index]);
                    font-size: root.cell-size * 0.62;
                    font-family: "Segoe UI Symbol";
                    color: root.piece-color(root.pieces[index]);
                    horizontal-alignment: center;
                    vertical-alignment: center;
                }

                if root.interactive : TouchArea {
                    clicked => { root.square-clicked(index); }
                }
            }
        }
    }

    component GameReviewBrief inherits Rectangle {
        in property <string> result-text;
        in property <bool> ready;
        in property <string> status-text;
        in property <string> accuracy-text;
        in property <[ReviewStatEntry]> stats;
        callback review-clicked();
        callback close-clicked();

        background: #000000aa;

        Rectangle {
            width: 380px;
            x: (parent.width - self.width) / 2;
            y: (parent.height - self.height) / 2;
            background: #1a1a1e;
            border-radius: 12px;
            border-color: #3f3f46;
            border-width: 1px;

            VerticalLayout {
                padding: 24px;
                spacing: 16px;

                HorizontalLayout {
                    alignment: end;
                    Rectangle {
                        width: 24px;
                        height: 24px;
                        Text {
                            text: "✕";
                            color: #a1a1aa;
                            font-size: 16px;
                        }
                        TouchArea {
                            clicked => { root.close-clicked(); }
                        }
                    }
                }

                Text {
                    text: root.result-text;
                    color: #ffffff;
                    font-size: 20px;
                    font-weight: 800;
                    horizontal-alignment: center;
                }

                if !root.ready : Text {
                    text: root.status-text;
                    color: #a78bfa;
                    font-size: 13px;
                    wrap: word-wrap;
                    horizontal-alignment: center;
                }

                if root.ready : Text {
                    text: root.accuracy-text;
                    color: #e4e4e7;
                    font-size: 13px;
                    horizontal-alignment: center;
                }

                if root.ready : HorizontalLayout {
                    spacing: 8px;
                    alignment: center;
                    for stat in root.stats: Rectangle {
                        background: #27272a;
                        border-radius: 6px;
                        width: 72px;
                        height: 56px;
                        VerticalLayout {
                            alignment: center;
                            Text {
                                text: stat.count;
                                color: #ffffff;
                                font-size: 18px;
                                font-weight: 800;
                                horizontal-alignment: center;
                            }
                            Text {
                                text: stat.label;
                                color: #a1a1aa;
                                font-size: 10px;
                                horizontal-alignment: center;
                            }
                        }
                    }
                }

                if root.ready : Button {
                    text: "Game Review";
                    clicked => { root.review-clicked(); }
                }
            }
        }
    }

    component CourseTargetPicker inherits Rectangle {
        in property <[CourseTargetEntry]> targets;
        in property <string> selected-id;
        in property <string> side;
        callback choose(string);

        height: 132px;
        background: #1a1a1e;
        border-radius: 8px;
        border-color: #27272a;
        border-width: 1px;

        VerticalLayout {
            padding: 6px;
            spacing: 6px;
            ScrollView {
                VerticalLayout {
                    spacing: 6px;
                    alignment: start;
                    Button {
                        height: 34px;
                        text: (root.selected-id == "" ? "Selected: " : "")
                            + "Uncategorized " + root.side + " / Main";
                        clicked => { root.choose(""); }
                    }
                    for target in root.targets: Button {
                        visible: target.side == root.side || target.side == "Mixed";
                        height: 34px;
                        text: (root.selected-id == target.id ? "Selected: " : "") + target.label;
                        clicked => { root.choose(target.id); }
                    }
                }
            }
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
        in-out property <[CourseDetailEntry]> course-detail: [];
        in-out property <[CourseTargetEntry]> course-targets: [];
        in-out property <[HistoryEntry]> history: [];

        // Course catalog/detail and shared builder/import target
        in-out property <string> selected-course-id: "";
        in-out property <string> selected-course-title: "";
        in-out property <string> selected-course-progress: "";
        in-out property <string> builder-target-chapter-id: "";
        in-out property <string> import-target-chapter-id: "";

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
        in-out property <string> selected-game-summary: "Import or select a game to review.";
        in-out property <string> review-status: "";
        in-out property <bool> show-game-review-brief: false;
        in-out property <string> game-review-game-id: "";
        in-out property <string> game-review-result-text: "";
        in-out property <string> game-review-accuracy-text: "";
        in-out property <[ReviewStatEntry]> game-review-stats: [];
        callback game-review-open();
        callback game-review-close();
        callback show-game-review-brief-for(string);

        // Interactive board pieces (64 strings)
        in-out property <[string]> board-pieces: [
            "r", "n", "b", "q", "k", "b", "n", "r",
            "p", "p", "p", "p", "p", "p", "p", "p",
            "", "", "", "", "", "", "", "",
            "", "", "", "", "", "", "", "",
            "", "", "", "", "", "", "", "",
            "", "", "", "", "", "", "", "",
            "P", "P", "P", "P", "P", "P", "P", "P",
            "R", "N", "B", "Q", "K", "B", "N", "R"
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
        in-out property <string> chesscom-sync-status: "Not synced";
        in-out property <string> tree-status: "";

        // Repertoire builder state
        in-out property <string> builder-fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        in-out property <string> builder-moves: "";
        in-out property <string> builder-color: "White";
        in-out property <string> builder-line-name: "";
        in-out property <int> builder-selected-square: -1;
        in-out property <[string]> builder-board-pieces: [
            "r","n","b","q","k","b","n","r",
            "p","p","p","p","p","p","p","p",
            "","","","","","","","",
            "","","","","","","","",
            "","","","","","","","",
            "","","","","","","","",
            "P","P","P","P","P","P","P","P",
            "R","N","B","Q","K","B","N","R"
        ];

        // Drill branch info
        in-out property <string> drill-branch-info: "";

        // Import Lines screen state
        in-out property <string> import-status: "";
        in-out property <string> import-side: "White";

        // Play-vs-bot screen state
        in-out property <[string]> play-board-pieces: [
            "r", "n", "b", "q", "k", "b", "n", "r",
            "p", "p", "p", "p", "p", "p", "p", "p",
            "", "", "", "", "", "", "", "",
            "", "", "", "", "", "", "", "",
            "", "", "", "", "", "", "", "",
            "", "", "", "", "", "", "", "",
            "P", "P", "P", "P", "P", "P", "P", "P",
            "R", "N", "B", "Q", "K", "B", "N", "R"
        ];
        in-out property <int> play-selected-square: -1;
        in-out property <string> play-status: "Choose color and start a game.";
        in-out property <string> play-summary: "";
        in-out property <string> play-color: "White";
        in-out property <int> play-elo: 1500;
        in-out property <bool> play-active: false;
        in-out property <string> play-book-state: "";

        // Puzzle Trainer screen state
        in-out property <[string]> puzzle-board-pieces: [
            "r", "n", "b", "q", "k", "b", "n", "r",
            "p", "p", "p", "p", "p", "p", "p", "p",
            "", "", "", "", "", "", "", "",
            "", "", "", "", "", "", "", "",
            "", "", "", "", "", "", "", "",
            "", "", "", "", "", "", "", "",
            "P", "P", "P", "P", "P", "P", "P", "P",
            "R", "N", "B", "Q", "K", "B", "N", "R"
        ];
        in-out property <int> puzzle-selected-square: -1;
        in-out property <string> puzzle-status: "Press 'New Puzzle' to start.";
        in-out property <string> puzzle-meta: "";
        in-out property <string> puzzle-progress: "";
        in-out property <string> puzzle-solution-text: "";
        in-out property <bool> puzzle-active: false;
        in-out property <string> puzzle-fetch-status: "";

        // Callbacks
        callback select-screen(string);
        callback import-lines(string, string);
        callback sync-studies(string);
        callback import-pgn(string);
        callback select-game(string);
        callback select-game-move(int);
        callback select-course(string);
        callback select-repertoire-line(string);
        callback click-board-square(int);
        callback start-drill();
        callback sync-lichess(string);
        callback sync-chesscom();
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
        callback adopt-explorer(string);

        // Play-vs-bot callbacks
        callback play-new-game(string, int);
        callback play-click-square(int);
        callback play-resign();
        callback play-review-game();

        // Puzzle trainer callbacks (load-puzzle above loads a random puzzle)
        callback puzzle-click-square(int);
        callback puzzle-show-solution();
        callback fetch-lichess-puzzles();

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
                        SidebarButton {
                            text: "Import Lines";
                            active: root.active-screen == "import";
                            clicked => { root.select-screen("import"); }
                        }
                        SidebarButton {
                            text: "Play vs Bot";
                            active: root.active-screen == "play";
                            clicked => { root.select-screen("play"); }
                        }
                        SidebarButton {
                            text: "Puzzle Trainer";
                            active: root.active-screen == "puzzles";
                            clicked => { root.select-screen("puzzles"); }
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
                                        } else if (root.rec-mode == "PuzzleRush") {
                                            root.select-screen("puzzles");
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

                    HorizontalLayout {
                        spacing: 12px;
                        height: 40px;
                        Button { text: "Sync Lichess Games"; clicked => { root.sync-lichess(""); } }
                        Text { text: root.sync-status; color: #a1a1aa; font-size: 12px; vertical-alignment: center; }
                        Button { text: "Sync Chess.com Games"; clicked => { root.sync-chesscom(); } }
                        Text { text: root.chesscom-sync-status; color: #a1a1aa; font-size: 12px; vertical-alignment: center; }
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

                    // Course catalog / selected course detail (Left Column)
                    VerticalLayout {
                        width: 340px;
                        spacing: 12px;

                        if (root.selected-course-id == "") : Text {
                            text: "Opening Courses";
                            color: #ffffff;
                            font-size: 18px;
                            font-weight: 700;
                        }

                        if (root.selected-course-id == "") : Rectangle {
                            background: #1a1a1e;
                            border-radius: 8px;
                            border-color: #27272a;
                            border-width: 1px;

                            ScrollView {
                                VerticalLayout {
                                    padding: 12px;
                                    spacing: 12px;
                                    alignment: start;
                                    for course in root.opening-lines: Button {
                                        height: 112px;
                                        text: course.name + " | " + course.side + "\n"
                                            + course.moves + "\n"
                                            + course.line_count + " lines | " + course.source;
                                        clicked => { root.select-course(course.id); }
                                    }
                                }
                            }
                        }

                        if (root.selected-course-id != "") : Button {
                            height: 34px;
                            text: "< All Courses";
                            clicked => { root.select-course(""); }
                        }

                        if (root.selected-course-id != "") : Text {
                            text: root.selected-course-title;
                            color: #ffffff;
                            font-size: 18px;
                            font-weight: 700;
                        }

                        if (root.selected-course-id != "") : Text {
                            text: root.selected-course-progress;
                            color: #a78bfa;
                            font-size: 12px;
                        }

                        if (root.selected-course-id != "") : Rectangle {
                            background: #1a1a1e;
                            border-radius: 8px;
                            border-color: #27272a;
                            border-width: 1px;

                            ScrollView {
                                VerticalLayout {
                                    padding: 12px;
                                    spacing: 8px;
                                    alignment: start;
                                    for entry in root.course-detail: Rectangle {
                                        height: entry.is_chapter ? 34px : 64px;
                                        background: entry.is_chapter
                                            ? #00000000
                                            : #00000000;
                                        border-radius: 6px;

                                        if (!entry.is_chapter) : Button {
                                            text: (root.drill-line-id == entry.id ? "Selected: " : "")
                                                + entry.title + "\n" + entry.meta;
                                            clicked => { root.select-repertoire-line(entry.id); }
                                        }

                                        if (entry.is_chapter) : VerticalLayout {
                                            padding: 4px;
                                            alignment: center;
                                            Text {
                                                text: entry.title;
                                                color: #a78bfa;
                                                font-size: 13px;
                                                font-weight: 700;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Interactive Chess Board and Drill View (Right Column)
                    VerticalLayout {
                        spacing: 16px;
                        horizontal-stretch: 1;

                        // Chess board container
                        ChessBoard {
                            pieces: root.board-pieces;
                            selected-square: root.board-selected-square;
                            square-clicked(index) => { root.click-board-square(index); }
                        }

                        // Drill Controls
                        Rectangle {
                            background: #1a1a1e;
                            border-radius: 8px;
                            padding: 16px;
                            // Bounded to match ChessBoard's own soft cap so this
                            // column can't claim unbounded width on an ultrawide
                            // window (and, in turn, so the outer HorizontalLayout
                            // can't starve its sibling column of surplus space).
                            min-width: 360px;
                            max-width: 560px;

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

                                        Rectangle {
                                            x: parent.width - self.width - 8px;
                                            y: 8px;
                                            width: 56px;
                                            height: 22px;
                                            background: #3f3f46;
                                            border-radius: 4px;
                                            Text {
                                                text: "Review";
                                                color: #e4e4e7;
                                                font-size: 10px;
                                                horizontal-alignment: center;
                                                vertical-alignment: center;
                                            }
                                            TouchArea {
                                                clicked => {
                                                    root.show-game-review-brief-for(game.id);
                                                }
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
                            horizontal-stretch: 1;

                            ChessBoard {
                                pieces: root.board-pieces;
                                interactive: false;
                            }

                            // Engine Eval Block
                            Rectangle {
                                background: #1a1a1e;
                                border-radius: 8px;
                                padding: 12px;
                                // Bounded to match ChessBoard's own soft cap so this
                                // column can't claim unbounded width on an ultrawide
                                // window (and, in turn, so the outer HorizontalLayout
                                // can't starve its sibling column of surplus space).
                                min-width: 360px;
                                max-width: 560px;

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
                        ChessBoard {
                            pieces: root.builder-board-pieces;
                            selected-square: root.builder-selected-square;
                            highlight-color: #7c3aed;
                            square-clicked(index) => { root.builder-click-square(index); }
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

                        Text { text: "Course / Chapter"; color: #a1a1aa; font-size: 12px; }
                        CourseTargetPicker {
                            targets: root.course-targets;
                            selected-id: root.builder-target-chapter-id;
                            side: root.builder-color;
                            choose(id) => { root.builder-target-chapter-id = id; }
                        }

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

                        Rectangle {
                            height: 36px;
                            background: #27272a;
                            border-radius: 8px;
                            TouchArea { clicked => { root.adopt-explorer(root.builder-color); } }
                            Text {
                                text: "Adopt Master Lines from Current Position";
                                color: #a78bfa; font-size: 13px;
                                horizontal-alignment: center; vertical-alignment: center;
                            }
                        }
                        Text { text: root.tree-status; color: #a1a1aa; font-size: 12px; }
                    }
                }

                // 5. Import Lines Screen
                if (root.active-screen == "import") : VerticalLayout {
                    spacing: 16px;
                    alignment: start;
                    Text { text: "Import Repertoire Lines"; color: #ffffff; font-size: 18px; font-weight: 700; }
                    HorizontalLayout {
                        spacing: 8px;
                        Text { text: "Repertoire side:"; color: #a1a1aa; font-size: 13px; vertical-alignment: center; }
                        Rectangle {
                            width: 80px; height: 32px;
                            background: root.import-side == "White" ? #6d28d9 : #27272a;
                            border-radius: 6px;
                            TouchArea { clicked => { root.import-side = "White"; root.import-target-chapter-id = ""; } }
                            Text { text: "White"; color: white; font-size: 13px; horizontal-alignment: center; vertical-alignment: center; }
                        }
                        Rectangle {
                            width: 80px; height: 32px;
                            background: root.import-side == "Black" ? #6d28d9 : #27272a;
                            border-radius: 6px;
                            TouchArea { clicked => { root.import-side = "Black"; root.import-target-chapter-id = ""; } }
                            Text { text: "Black"; color: white; font-size: 13px; horizontal-alignment: center; vertical-alignment: center; }
                        }
                    }
                    Text { text: "Course / Chapter"; color: #a1a1aa; font-size: 12px; }
                    CourseTargetPicker {
                        width: 520px;
                        targets: root.course-targets;
                        selected-id: root.import-target-chapter-id;
                        side: root.import-side;
                        choose(id) => { root.import-target-chapter-id = id; }
                    }
                    import-input := TextEdit {
                        height: 180px;
                        placeholder-text: "Paste PGN here — variations included. Multiple games OK.";
                    }
                    HorizontalLayout {
                        spacing: 12px;
                        Button {
                            text: "Import PGN Lines";
                            clicked => { root.import-lines(import-input.text, root.import-side); }
                        }
                        Button {
                            text: "Sync My Lichess Studies";
                            clicked => { root.sync-studies(root.import-side); }
                        }
                    }
                    Text { text: root.import-status; color: #a78bfa; font-size: 13px; }
                }

                // 6. Play vs Bot Screen
                if (root.active-screen == "play") : HorizontalLayout {
                    spacing: 24px;
                    // Board
                    ChessBoard {
                        pieces: root.play-board-pieces;
                        selected-square: root.play-selected-square;
                        square-clicked(index) => { root.play-click-square(index); }
                    }
                    // Controls
                    VerticalLayout {
                        spacing: 12px; alignment: start;
                        Text { text: "Play vs Book Bot"; color: #ffffff; font-size: 18px; font-weight: 700; }
                        HorizontalLayout {
                            spacing: 8px;
                            Rectangle {
                                width: 80px; height: 32px;
                                background: root.play-color == "White" ? #6d28d9 : #27272a;
                                border-radius: 6px;
                                TouchArea { clicked => { root.play-color = "White"; } }
                                Text { text: "White"; color: white; font-size: 13px; horizontal-alignment: center; vertical-alignment: center; }
                            }
                            Rectangle {
                                width: 80px; height: 32px;
                                background: root.play-color == "Black" ? #6d28d9 : #27272a;
                                border-radius: 6px;
                                TouchArea { clicked => { root.play-color = "Black"; } }
                                Text { text: "Black"; color: white; font-size: 13px; horizontal-alignment: center; vertical-alignment: center; }
                            }
                        }
                        HorizontalLayout {
                            spacing: 8px;
                            Text { text: "Bot Elo:"; color: #a1a1aa; font-size: 13px; vertical-alignment: center; }
                            elo-input := LineEdit { width: 80px; text: "1500"; }
                        }
                        Button {
                            text: "New Game";
                            clicked => { root.play-new-game(root.play-color, elo-input.text.to-float()); }
                        }
                        if (root.play-active) : Button {
                            text: "Resign";
                            clicked => { root.play-resign(); }
                        }
                        Text { text: root.play-book-state; color: #a78bfa; font-size: 13px; }
                        Text { text: root.play-status; color: #e4e4e7; font-size: 13px; }
                        Text { text: root.play-summary; color: #a1a1aa; font-size: 12px; wrap: word-wrap; width: 300px; }
                        if (!root.play-active && root.play-summary != "") : Button {
                            text: "Review This Game";
                            clicked => { root.play-review-game(); }
                        }
                    }
                }

                // 7. Puzzle Trainer Screen
                if (root.active-screen == "puzzles") : HorizontalLayout {
                    spacing: 24px;
                    // Board
                    ChessBoard {
                        pieces: root.puzzle-board-pieces;
                        selected-square: root.puzzle-selected-square;
                        square-clicked(index) => { root.puzzle-click-square(index); }
                    }
                    // Controls
                    VerticalLayout {
                        spacing: 12px; alignment: start;
                        Text { text: "Puzzle Trainer"; color: #ffffff; font-size: 18px; font-weight: 700; }
                        Button {
                            text: root.puzzle-active ? "Skip / New Puzzle" : "New Puzzle";
                            clicked => { root.load-puzzle(); }
                        }
                        Text { text: root.puzzle-meta; color: #a78bfa; font-size: 13px; wrap: word-wrap; width: 300px; }
                        Text { text: root.puzzle-progress; color: #a1a1aa; font-size: 12px; }
                        Text { text: root.puzzle-status; color: #e4e4e7; font-size: 13px; wrap: word-wrap; width: 300px; }
                        if (root.puzzle-active) : Button {
                            text: "Show Solution";
                            clicked => { root.puzzle-show-solution(); }
                        }
                        Text { text: root.puzzle-solution-text; color: #fbbf24; font-size: 13px; wrap: word-wrap; width: 300px; }
                        Button {
                            text: "Fetch Puzzles from Lichess";
                            clicked => { root.fetch-lichess-puzzles(); }
                        }
                        Text { text: root.puzzle-fetch-status; color: #a1a1aa; font-size: 12px; wrap: word-wrap; width: 300px; }
                    }
                }

                if root.show-game-review-brief : GameReviewBrief {
                    width: 100%;
                    height: 100%;
                    result-text: root.game-review-result-text;
                    ready: root.game-review-stats.length > 0 || root.game-review-accuracy-text != "";
                    status-text: root.review-status;
                    accuracy-text: root.game-review-accuracy-text;
                    stats: root.game-review-stats;
                    review-clicked => { root.game-review-open(); }
                    close-clicked => { root.game-review-close(); }
                }
            }
        }
    }
}

// Convert FEN string to 64 side-aware piece codes so the UI can pick glyphs/colors.
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
            if square_idx < 64 {
                pieces[square_idx] = slint::SharedString::from(ch.to_string());
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

fn estimated_elo(accuracy: f32) -> i32 {
    (((800.0 + accuracy * 17.2) / 50.0).round() * 50.0) as i32
}

fn accuracy_summary_text(accuracy: Option<f32>) -> String {
    match accuracy {
        Some(value) => format!("Accuracy {:.0}% | Est. Elo {}", value, estimated_elo(value)),
        None => "Accuracy pending | Est. Elo pending".to_string(),
    }
}

fn course_progress_text(total_lines: i32, mastered_lines: i32, due_lines: i32) -> String {
    if due_lines > 0 {
        format!("{mastered_lines}/{total_lines} mastered - {due_lines} due")
    } else {
        format!("{mastered_lines}/{total_lines} mastered")
    }
}

fn course_detail_entries(db: &SqliteStore, course_id: &str, today: &str) -> Vec<CourseDetailEntry> {
    let mut entries = Vec::new();
    for chapter in db.get_course_chapters(course_id).unwrap_or_default() {
        entries.push(CourseDetailEntry {
            id: slint::SharedString::from(""),
            title: slint::SharedString::from(chapter.title.clone()),
            meta: slint::SharedString::from(""),
            is_chapter: true,
        });
        for line in db.get_course_lines(&chapter.id).unwrap_or_default() {
            let status = db
                .course_line_status(&line.id, today)
                .unwrap_or_else(|_| "Status unavailable".to_string());
            let move_count = db
                .get_course_line_moves(&line.id)
                .map_or(0, |moves| moves.len());
            entries.push(CourseDetailEntry {
                id: slint::SharedString::from(line.id),
                title: slint::SharedString::from(line.title),
                meta: slint::SharedString::from(format!("{status} | {move_count} moves")),
                is_chapter: false,
            });
        }
    }
    entries
}

fn game_review_summary(accuracy: Option<f32>, classes: &[&str]) -> String {
    let count = |label: &str| classes.iter().filter(|&&class| class == label).count();
    format!(
        "{} | Book {} | Blunder {} | Mistake {} | Miss {} | Good {} | Excellent {} | Best {} | Great {} | Brilliant {}",
        accuracy_summary_text(accuracy),
        count("Book"),
        count("Blunder"),
        count("Mistake"),
        count("Miss"),
        count("Good"),
        count("Excellent"),
        count("Best"),
        count("Great"),
        count("Brilliant"),
    )
}

/// Non-zero move-classification counts, in a fixed display order, for the
/// Game Review Brief's stat grid. Mirrors the 9 engine_controller::MoveClass
/// variants exactly (there is no "Perfect" class).
fn review_stat_entries(classes: &[&str]) -> Vec<ReviewStatEntry> {
    const ORDER: [&str; 9] = [
        "Best",
        "Book",
        "Blunder",
        "Brilliant",
        "Excellent",
        "Good",
        "Great",
        "Miss",
        "Mistake",
    ];
    ORDER
        .iter()
        .filter_map(|&label| {
            let count = classes.iter().filter(|&&class| class == label).count();
            if count == 0 {
                None
            } else {
                Some(ReviewStatEntry {
                    label: slint::SharedString::from(label),
                    count: count as i32,
                })
            }
        })
        .collect()
}

/// Determine a finished bot game's PGN result tag and a user-facing header,
/// from the user's perspective (`side` is `state.play_side`).
fn bot_game_outcome(chess: &Chess, side: &str, resigned: bool) -> (&'static str, &'static str) {
    if resigned {
        let tag = if side == "White" { "0-1" } else { "1-0" };
        return (tag, "Forge Bot won");
    }
    match chess.outcome() {
        Some(shakmaty::Outcome::Decisive { winner }) => {
            let tag = if winner == shakmaty::Color::White {
                "1-0"
            } else {
                "0-1"
            };
            let user_won = (winner == shakmaty::Color::White) == (side == "White");
            let header = if user_won {
                "You beat Forge Bot!"
            } else {
                "Forge Bot won"
            };
            (tag, header)
        }
        Some(shakmaty::Outcome::Draw) => ("1/2-1/2", "Draw"),
        None => ("*", "Game over"),
    }
}

/// Rebuild a bot game's PGN (SAN movetext) from its recorded UCI moves.
fn build_bot_game_pgn(moves: &[String], side: &str, result_tag: &str) -> Option<String> {
    if moves.is_empty() {
        return None;
    }
    let mut pos = Chess::default();
    let mut san_moves: Vec<String> = Vec::new();
    for uci_str in moves {
        let Ok(uci) = uci_str.parse::<Uci>() else {
            break;
        };
        let Ok(m) = uci.to_move(&pos) else { break };
        san_moves.push(San::from_move(&pos, &m).to_string());
        pos.play_unchecked(&m);
    }
    let mut movetext = String::new();
    for (i, san) in san_moves.iter().enumerate() {
        if i % 2 == 0 {
            movetext.push_str(&format!("{}. ", i / 2 + 1));
        }
        movetext.push_str(san);
        movetext.push(' ');
    }
    let user_name = std::env::var("LICHESS_USERNAME")
        .or_else(|_| std::env::var("CHESSCOM_USERNAME"))
        .unwrap_or_else(|_| "You".to_string());
    let (w, b) = if side == "White" {
        (user_name.as_str(), "Forge Bot")
    } else {
        ("Forge Bot", user_name.as_str())
    };
    Some(format!(
        "[Event \"Bot Game\"]\n[White \"{w}\"]\n[Black \"{b}\"]\n[Date \"{}\"]\n[Round \"{}\"]\n\n{movetext}{result_tag}",
        tree::local_now_str().replace('-', "."),
        uuid_now()
    ))
}

fn side_to_move_label(fen: &str) -> &'static str {
    if fen.split_whitespace().nth(1) == Some("b") {
        "Black"
    } else {
        "White"
    }
}

/// Built-in starter tactics so the Puzzle Trainer has content before any
/// external puzzle import exists. FEN is the position to solve (solver to
/// move); `solution_uci` is the full line from the solver's side, alternating
/// solver moves and scripted opponent replies.
fn starter_puzzles() -> Vec<PuzzleRecord> {
    let puzzle = |id: &str, fen: &str, solution: &str, theme: &str, difficulty: i32| PuzzleRecord {
        id: id.to_string(),
        fen: fen.to_string(),
        solution_uci: solution.to_string(),
        theme: theme.to_string(),
        difficulty,
    };
    vec![
        puzzle(
            "seed_scholars_mate",
            "r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 4 4",
            "h5f7",
            "mateIn1",
            600,
        ),
        puzzle(
            "seed_back_rank",
            "6k1/5ppp/8/8/8/8/5PPP/3R2K1 w - - 0 1",
            "d1d8",
            "backRankMate,mateIn1",
            800,
        ),
        puzzle(
            "seed_smothered",
            "6rk/6pp/8/6N1/8/8/8/6K1 w - - 0 1",
            "g5f7",
            "smotheredMate,mateIn1",
            900,
        ),
        puzzle(
            "seed_anastasia",
            "r7/4N1pk/8/8/8/R7/5PPP/6K1 w - - 0 1",
            "a3h3",
            "anastasiaMate,mateIn1",
            1100,
        ),
        puzzle(
            "seed_philidor",
            "5r1k/6pp/7N/8/8/1Q6/5PP1/6K1 w - - 0 1",
            "b3g8 f8g8 h6f7",
            "smotheredMate,mateIn2",
            1200,
        ),
    ]
}

/// Convert a fetched Lichess puzzle into our stored convention: FEN is the
/// position to solve (solver to move, derived from the game PGN and
/// `initialPly`), `solution_uci` runs from the solver's side, themes are
/// comma-joined, and the Lichess rating maps to difficulty.
fn lichess_puzzle_to_record(
    fetched: &lichess_client::puzzles::LichessPuzzle,
) -> Result<PuzzleRecord, String> {
    if fetched.puzzle.solution.is_empty() {
        return Err("empty solution".to_string());
    }
    let fen = lichess_client::puzzles::puzzle_fen_from_pgn(
        &fetched.game.pgn,
        fetched.puzzle.initial_ply,
    )?;
    Ok(PuzzleRecord {
        id: format!("lichess_{}", fetched.puzzle.id),
        fen,
        solution_uci: fetched.puzzle.solution.join(" "),
        theme: fetched.puzzle.themes.join(","),
        difficulty: fetched.puzzle.rating,
    })
}

/// "Move X of Y" counting only the solver's moves (even indices of the
/// solution line); `index` is the next expected solution index.
fn puzzle_progress_text(index: usize, total: usize) -> String {
    format!("Move {} of {}", index / 2 + 1, (total + 1) / 2)
}

/// `get_puzzle_rating` reads the latest kind='puzzle' `score_delta` as the
/// current rating, so puzzle events store the absolute updated rating.
fn updated_puzzle_rating(current: i32, solved: bool) -> i32 {
    if solved {
        (current + 10).min(3200)
    } else {
        (current - 10).max(400)
    }
}

/// Record a pass/fail training event for the current puzzle and fold the
/// result into the puzzle rating shown on the dashboard.
fn record_puzzle_event(state: &mut AppState, solved: bool) {
    let Some(puzzle) = state.puzzle_current.as_ref() else {
        return;
    };
    let current = state.db.get_puzzle_rating("default_user").unwrap_or(1500);
    let event = TrainingEventRecord {
        id: format!("evt_{}", uuid_now()),
        user_id: "default_user".to_string(),
        kind: "puzzle".to_string(),
        target_id: puzzle.id.clone(),
        outcome: if solved { "Correct" } else { "Failed" }.to_string(),
        score_delta: updated_puzzle_rating(current, solved) as f32,
        created_at: tree::local_now_str(),
    };
    let _ = state.db.insert_training_event(&event);
}

/// Replay a puzzle's UCI solution from its FEN and render it as SAN.
/// Returns None if the FEN or any move does not replay legally.
fn solution_to_san(fen: &str, ucis: &[String]) -> Option<String> {
    let parsed: sk_fen::Fen = fen.parse().ok()?;
    let mut pos: Chess = parsed.into_position(CastlingMode::Standard).ok()?;
    let mut out: Vec<String> = Vec::with_capacity(ucis.len());
    for uci_str in ucis {
        let uci: Uci = uci_str.parse().ok()?;
        let m = uci.to_move(&pos).ok()?;
        out.push(San::from_move(&pos, &m).to_string());
        pos.play_unchecked(&m);
    }
    Some(out.join(" "))
}

fn is_book_move(db: &SqliteStore, fen: &str, played_move: &str) -> bool {
    db.get_repertoire_moves_from(fen, side_to_move_label(fen))
        .unwrap_or_default()
        .iter()
        .any(|edge| edge.uci == played_move)
}

fn find_stockfish_path() -> String {
    if let Ok(path) = std::env::var("STOCKFISH_PATH") {
        if std::path::Path::new(&path).exists() {
            return path;
        }
    }

    // Prefer a real stockfish from PATH before falling back to the mock engine
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(if cfg!(windows) {
                "stockfish.exe"
            } else {
                "stockfish"
            });
            if candidate.exists() {
                return candidate.to_string_lossy().to_string();
            }
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

/// Insert a game + its per-ply positions (unanalyzed). Existing rows are untouched.
fn ingest_pgn_game(
    db: &mut SqliteStore,
    game_id: &str,
    source: &str,
    pgn: &str,
    fallback_date: Option<String>,
) -> Result<(), String> {
    let parsed = pgn_processor::parse_pgn(pgn);
    let game = GameRecord {
        id: game_id.to_string(),
        source: source.to_string(),
        white: parsed
            .headers
            .get("White")
            .cloned()
            .unwrap_or_else(|| "White".into()),
        black: parsed
            .headers
            .get("Black")
            .cloned()
            .unwrap_or_else(|| "Black".into()),
        result: parsed.result.clone(),
        eco: parsed.headers.get("ECO").cloned(),
        pgn: pgn.to_string(),
        played_at: parsed
            .headers
            .get("Date")
            .cloned()
            .map(|d| d.replace('.', "-"))
            .or(fallback_date),
    };
    db.insert_game(&game)?;
    let fens_and_moves = pgn_processor::game_to_fen_and_uci(&parsed.moves)?;
    for (ply, (fen, uci_move)) in fens_and_moves.into_iter().enumerate() {
        db.insert_position(&PositionRecord {
            id: format!("{}_{}", game_id, ply + 1),
            game_id: game_id.to_string(),
            ply: (ply + 1) as u32,
            fen,
            played_move: uci_move,
            centipawn_loss: None,
            mistake_class: None,
        })?;
    }
    Ok(())
}

struct AppState {
    db: SqliteStore,
    stockfish_path: String,
    // Tree-based drill session
    drill_side: String,                                    // "White" | "Black"
    drill_chess: Chess,                                    // current drill position (kept)
    drill_expected: Vec<db_manager::RepertoireMoveRecord>, // my-move edges at current node
    drill_start_edge: Option<db_manager::RepertoireMoveRecord>,
    // Current board FEN for explorer
    current_fen: Option<String>,
    // Repertoire builder state
    builder_chess: Chess,
    builder_staged_uci: Vec<String>,
    builder_selected_sq: Option<usize>,
    builder_color: String,
    // Play-vs-bot session
    play_chess: Chess,
    play_moves_uci: Vec<String>,
    play_side: String, // user's color: "White" | "Black"
    play_session: Option<engine_controller::PlaySession>,
    play_deviations: Vec<String>,
    play_plies_in_book: u32,
    play_left_book_at: Option<u32>,
    // Puzzle trainer session
    puzzle_current: Option<PuzzleRecord>,
    puzzle_chess: Chess, // current puzzle position (kept in sync with plays)
    puzzle_solution: Vec<String>, // solution UCI line: solver move, reply, solver move, …
    puzzle_index: usize, // next expected index into puzzle_solution
    puzzle_failed: bool, // first wrong move / reveal already recorded
}

fn main() {
    dotenvy::dotenv().ok();
    let stockfish_path = find_stockfish_path();

    // Initialize database
    let mut db = SqliteStore::new("forge.db").expect("failed to open database file forge.db");
    db.bootstrap().expect("failed to bootstrap database tables");
    db.run_migrations()
        .expect("failed to run database migrations");

    let migrated = tree::migrate_legacy_lines(&mut db).unwrap_or_else(|e| {
        eprintln!("legacy line migration failed (continuing on tree): {e}");
        0
    });
    if migrated > 0 {
        println!("Migrated {migrated} legacy opening lines into the repertoire tree.");
    }

    // Seed built-in starter puzzles so the Puzzle Trainer has content before
    // any external puzzle import exists. INSERT OR IGNORE keeps this idempotent.
    for puzzle in starter_puzzles() {
        if let Err(e) = db.insert_puzzle(&puzzle) {
            eprintln!("failed to seed starter puzzle {}: {e}", puzzle.id);
        }
    }

    // Load initial data
    let state = Arc::new(Mutex::new(AppState {
        db,
        stockfish_path,
        drill_side: "White".to_string(),
        drill_chess: Chess::default(),
        drill_expected: Vec::new(),
        drill_start_edge: None,
        current_fen: None,
        builder_chess: Chess::default(),
        builder_staged_uci: Vec::new(),
        builder_selected_sq: None,
        builder_color: "White".to_string(),
        play_chess: Chess::default(),
        play_moves_uci: Vec::new(),
        play_side: "White".to_string(),
        play_session: None,
        play_deviations: Vec::new(),
        play_plies_in_book: 0,
        play_left_book_at: None,
        puzzle_current: None,
        puzzle_chess: Chess::default(),
        puzzle_solution: Vec::new(),
        puzzle_index: 0,
        puzzle_failed: false,
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
                let mut stmt = state
                    .db
                    .conn
                    .prepare(
                        "SELECT p.ply, p.centipawn_loss, p.mistake_class, g.eco
                     FROM positions p JOIN games g ON g.id = p.game_id",
                    )
                    .unwrap();
                stmt.query_map([], |row| {
                    Ok((
                        row.get::<_, u32>(0)?,
                        row.get::<_, Option<i32>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                })
                .unwrap()
                .filter_map(|r| r.ok())
                .collect()
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
            let game_entries: Vec<GameEntry> = games
                .into_iter()
                .map(|g| GameEntry {
                    id: slint::SharedString::from(g.id),
                    white: slint::SharedString::from(g.white),
                    black: slint::SharedString::from(g.black),
                    result: slint::SharedString::from(g.result.unwrap_or_else(|| "*".to_string())),
                    date: slint::SharedString::from(
                        g.played_at.unwrap_or_else(|| "2026-06-12".to_string()),
                    ),
                    pgn: slint::SharedString::from(g.pgn),
                })
                .collect();
            app.set_games(slint::ModelRc::from(Rc::new(slint::VecModel::from(
                game_entries,
            ))));

            // Refresh real course cards and selected course detail.
            let courses = state.db.get_courses().unwrap_or_default();
            let mut line_entries = Vec::new();
            for course in &courses {
                let Ok(progress) = state.db.course_progress(&course.id, &today) else {
                    continue;
                };
                let total = progress.total_lines as i32;
                let mastered = progress.mastered_lines as i32;
                let due = progress.due_lines as i32;
                line_entries.push(OpeningLineEntry {
                    id: slint::SharedString::from(course.id.clone()),
                    name: slint::SharedString::from(course.title.clone()),
                    moves: slint::SharedString::from(course_progress_text(total, mastered, due)),
                    confidence: if total == 0 {
                        0.0
                    } else {
                        mastered as f32 / total as f32
                    },
                    start_fen: slint::SharedString::from(course.thumbnail_fen.clone()),
                    side: slint::SharedString::from(course.side.clone()),
                    source: slint::SharedString::from(course.source.clone()),
                    line_count: total,
                });
            }
            app.set_opening_lines(slint::ModelRc::from(Rc::new(slint::VecModel::from(
                line_entries,
            ))));

            let mut course_targets = Vec::new();
            for course in &courses {
                if matches!(
                    course.id.as_str(),
                    "uncategorized-white" | "uncategorized-black"
                ) {
                    continue;
                }
                for chapter in state.db.get_course_chapters(&course.id).unwrap_or_default() {
                    course_targets.push(CourseTargetEntry {
                        id: slint::SharedString::from(chapter.id),
                        label: slint::SharedString::from(format!(
                            "{} / {}",
                            course.title, chapter.title
                        )),
                        side: slint::SharedString::from(course.side.clone()),
                    });
                }
            }
            app.set_course_targets(slint::ModelRc::from(Rc::new(slint::VecModel::from(
                course_targets,
            ))));

            let selected_course_id = app.get_selected_course_id().to_string();
            if let Some(course) = courses
                .iter()
                .find(|course| course.id == selected_course_id)
            {
                let progress = state.db.course_progress(&course.id, &today).ok();
                let progress_text = progress.map_or_else(
                    || "0/0 mastered".to_string(),
                    |progress| {
                        course_progress_text(
                            progress.total_lines as i32,
                            progress.mastered_lines as i32,
                            progress.due_lines as i32,
                        )
                    },
                );
                app.set_selected_course_title(slint::SharedString::from(course.title.clone()));
                app.set_selected_course_progress(slint::SharedString::from(progress_text));
                app.set_course_detail(slint::ModelRc::from(Rc::new(slint::VecModel::from(
                    course_detail_entries(&state.db, &course.id, &today),
                ))));
            } else if !selected_course_id.is_empty() {
                app.set_selected_course_id(slint::SharedString::from(""));
                app.set_selected_course_title(slint::SharedString::from(""));
                app.set_selected_course_progress(slint::SharedString::from(""));
                app.set_course_detail(slint::ModelRc::from(Rc::new(slint::VecModel::from(Vec::<
                    CourseDetailEntry,
                >::new(
                )))));
            }

            // Refresh training logs
            let mut stmt = state.db.conn.prepare("SELECT kind, target_id, outcome, score_delta, created_at FROM training_events ORDER BY created_at DESC").unwrap();
            let logs = stmt
                .query_map([], |row| {
                    Ok(HistoryEntry {
                        kind: slint::SharedString::from(row.get::<_, String>(0)?),
                        target: slint::SharedString::from(row.get::<_, String>(1)?),
                        outcome: slint::SharedString::from(row.get::<_, String>(2)?),
                        delta: row.get::<_, f64>(3)? as f32,
                        date: slint::SharedString::from(row.get::<_, String>(4)?),
                    })
                })
                .unwrap();
            let mut log_list = Vec::new();
            for log in logs {
                if let Ok(l) = log {
                    log_list.push(l);
                }
            }
            app.set_history(slint::ModelRc::from(Rc::new(slint::VecModel::from(
                log_list,
            ))));
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

    // Course catalog/detail selection.
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        let refresh = refresh_data.clone();
        app.on_select_course(move |course_id| {
            let mut state = state.lock().unwrap();
            let app = app_weak.upgrade().unwrap();
            state.drill_start_edge = None;
            state.drill_expected.clear();
            drop(state);
            app.set_drill_line_id(slint::SharedString::default());
            app.set_drill_name(slint::SharedString::default());
            app.set_drill_instructions(slint::SharedString::from("Select a named line to drill."));
            app.set_drill_branch_info(slint::SharedString::default());
            app.set_drill_active(false);
            app.set_board_selected_square(-1);
            app.set_board_pieces(slint::ModelRc::from(Rc::new(slint::VecModel::from(
                fen_to_pieces("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR"),
            ))));
            app.set_selected_course_id(course_id);
            refresh();
        });
    }

    // Wiring game selection for Game Review timeline
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_select_game(move |game_id: slint::SharedString| {
            let state = state.lock().unwrap();
            let app = app_weak.upgrade().unwrap();

            let positions = state
                .db
                .get_positions_for_game(&game_id)
                .unwrap_or_default();
            let mut moves = Vec::new();
            let mut losses = Vec::new();
            let mut classes = Vec::new();
            let mut fens = Vec::new();

            for pos in &positions {
                moves.push(slint::SharedString::from(pos.played_move.clone()));
                losses.push(slint::SharedString::from(
                    pos.centipawn_loss
                        .map(|l| l.to_string())
                        .unwrap_or_default(),
                ));
                classes.push(slint::SharedString::from(
                    pos.mistake_class.clone().unwrap_or_default(),
                ));
                fens.push(slint::SharedString::from(pos.fen.clone()));
            }

            app.set_selected_game_id(game_id.clone());
            app.set_selected_game_title(slint::SharedString::from("Timeline"));
            app.set_selected_game_moves(slint::ModelRc::from(Rc::new(slint::VecModel::from(
                moves,
            ))));
            app.set_selected_game_losses(slint::ModelRc::from(Rc::new(slint::VecModel::from(
                losses,
            ))));
            app.set_selected_game_classes(slint::ModelRc::from(Rc::new(slint::VecModel::from(
                classes,
            ))));
            app.set_selected_game_fens(slint::ModelRc::from(Rc::new(slint::VecModel::from(fens))));
            app.set_selected_move_index(-1);

            // Reset board to starting pos
            app.set_board_pieces(slint::ModelRc::from(Rc::new(slint::VecModel::from(
                fen_to_pieces("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR"),
            ))));
        });
    }

    // Wiring on-demand Game Review Brief from the Reviewed Games list
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_show_game_review_brief_for(move |game_id: slint::SharedString| {
            let state = state.lock().unwrap();
            let Some(app) = app_weak.upgrade() else {
                return;
            };

            let positions = state
                .db
                .get_positions_for_game(&game_id)
                .unwrap_or_default();
            let class_names: Vec<&str> = positions
                .iter()
                .filter_map(|pos| pos.mistake_class.as_deref())
                .collect();
            let accuracy = state
                .db
                .conn
                .query_row(
                    "SELECT accuracy_overall FROM games WHERE id = ?1",
                    [game_id.as_str()],
                    |row| row.get::<_, Option<f64>>(0),
                )
                .ok()
                .flatten()
                .map(|value| value as f32);
            let games = state.db.get_games().unwrap_or_default();
            drop(state);

            let Some(game) = games.into_iter().find(|g| g.id == game_id.as_str()) else {
                return;
            };
            let is_bot_game = game.white == "You" && game.black == "Forge Bot"
                || game.white == "Forge Bot" && game.black == "You";
            // Bot games always store the literal placeholder names "You"/"Forge
            // Bot" rather than a configured Lichess/Chess.com username, so
            // `tree::user_side_for_game` (which matches against those env-var
            // usernames) can never resolve a side for them — it would leave
            // `won` permanently false and make "You beat Forge Bot!" dead code.
            // Resolve the bot-game side directly from the same literal check
            // used for `is_bot_game`; fall back to the username-based lookup
            // for real imported games.
            let user_side = if is_bot_game {
                if game.white == "You" {
                    Some("White".to_string())
                } else {
                    Some("Black".to_string())
                }
            } else {
                tree::user_side_for_game(&game.white, &game.black)
            };
            let won = match (&user_side, game.result.as_deref()) {
                (Some(side), Some("1-0")) => side == "White",
                (Some(side), Some("0-1")) => side == "Black",
                _ => false,
            };
            let drawn = matches!(game.result.as_deref(), Some("1/2-1/2"));
            let header = if drawn {
                "Draw".to_string()
            } else if is_bot_game {
                if won {
                    "You beat Forge Bot!".to_string()
                } else {
                    "Forge Bot won".to_string()
                }
            } else if won {
                "You won".to_string()
            } else {
                "You lost".to_string()
            };

            app.set_game_review_game_id(game_id.clone());
            app.set_game_review_result_text(slint::SharedString::from(header));
            app.set_game_review_stats(slint::ModelRc::from(Rc::new(slint::VecModel::from(
                review_stat_entries(&class_names),
            ))));
            app.set_game_review_accuracy_text(slint::SharedString::from(accuracy_summary_text(
                accuracy,
            )));
            app.set_show_game_review_brief(true);
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
                app.set_board_pieces(slint::ModelRc::from(Rc::new(slint::VecModel::from(
                    fen_to_pieces(&fen),
                ))));

                let class = app
                    .get_selected_game_classes()
                    .row_data(idx as usize)
                    .unwrap_or_default();
                let loss = app
                    .get_selected_game_losses()
                    .row_data(idx as usize)
                    .unwrap_or_default();

                if class != "" {
                    app.set_selected_move_eval(slint::SharedString::from(format!(
                        "{} (Lost {} cp)",
                        class, loss
                    )));
                } else {
                    app.set_selected_move_eval(slint::SharedString::from("Optimal or Incomplete"));
                }
                app.set_selected_move_best(slint::SharedString::from(
                    "Check bestmove in timeline.",
                ));
            }
        });
    }

    // Resolve a named course line to its first existing repertoire edge for drilling.
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_select_repertoire_line(move |line_id: slint::SharedString| {
            let mut state = state.lock().unwrap();
            let app = app_weak.upgrade().unwrap();
            let Ok(Some(line)) = state.db.get_course_line(&line_id) else {
                return;
            };
            let Ok(moves) = state.db.get_course_line_moves(&line_id) else {
                return;
            };
            let Some((first_move_id, _, _)) = moves.first() else {
                return;
            };
            let Ok(Some(edge)) = state.db.get_repertoire_move(*first_move_id) else {
                return;
            };
            app.set_drill_line_id(line_id);
            app.set_drill_name(slint::SharedString::from(format!("Drill: {}", line.title)));
            app.set_drill_instructions(slint::SharedString::from(
                "Press 'Start Drill' to practice from this course line's opening position.",
            ));
            app.set_drill_active(false);
            app.set_board_pieces(slint::ModelRc::from(Rc::new(slint::VecModel::from(
                fen_to_pieces(&edge.parent_fen),
            ))));
            app.set_board_selected_square(-1);
            state.drill_start_edge = Some(edge);
        });
    }

    // Drill execution callbacks
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_start_drill(move || {
            let mut state = state.lock().unwrap();
            let app = app_weak.upgrade().unwrap();
            let Some(edge) = state.drill_start_edge.clone() else {
                return;
            };
            let Ok(Some((side, fen))) = state.db.get_node_side_and_fen(edge.parent_id) else {
                return;
            };
            let parsed: shakmaty::fen::Fen = match fen.parse() {
                Ok(f) => f,
                Err(_) => return,
            };
            let Ok(pos) = parsed.into_position(CastlingMode::Standard) else {
                return;
            };
            state.drill_side = side.clone();
            state.drill_chess = pos;
            state.drill_expected.clear();
            drill_advance(&mut state, &app); // shared session-step helper, below
            app.set_drill_active(true);
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
                app.set_board_selected_square(idx);
                return;
            }
            app.set_board_selected_square(-1);
            let uci_try = format!(
                "{}{}",
                index_to_square(sel as usize),
                index_to_square(idx as usize)
            );

            let today_days = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                / 86400;

            let matched = state
                .drill_expected
                .iter()
                .find(|e| e.uci == uci_try || e.uci == format!("{}q", uci_try))
                .cloned();

            if let Some(edge) = matched {
                let _ = tree::grade_edge_with_event(&mut state.db, &edge, 5, today_days, "drill");
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
                            state.drill_chess.clone(),
                            shakmaty::EnPassantMode::Legal,
                        )
                        .to_string();
                        app.set_board_pieces(slint::ModelRc::from(Rc::new(slint::VecModel::from(
                            fen_to_pieces(&fen),
                        ))));
                    }
                }
                drill_advance(&mut state, &app);
                if !app.get_drill_active() {
                    drop(state);
                    refresh_data_cp();
                }
            } else {
                // Wrong move: fail every expected edge at this node (they were all "the answer")
                for edge in state.drill_expected.clone() {
                    let _ =
                        tree::grade_edge_with_event(&mut state.db, &edge, 1, today_days, "drill");
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
                    "Not in your repertoire here — try again (this move is now due for review).",
                ));
                drop(state);
                refresh_data_cp();
            }
        });
    }

    // Wiring PGN Ingestion & Background analysis
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        let refresh_data_cp = refresh_data.clone();
        app.on_import_pgn(move |pgn_text: slint::SharedString| {
            import_and_analyze_pgn(&state, &app_weak, &refresh_data_cp, &pgn_text, true);
        });
    }

    // Lichess sync callback
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        let refresh_data_cp = refresh_data.clone();
        app.on_sync_lichess(move |username: slint::SharedString| {
            let username = username.to_string();
            let username = if username.is_empty() {
                std::env::var("LICHESS_USERNAME").unwrap_or_else(|_| "hackandtoss".into())
            } else {
                username
            };
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
                        let mut st = state.lock().unwrap();
                        let mut new_count = 0u32;
                        let mut skipped = 0u32;
                        for g in &games {
                            if st.db.is_synced("lichess", &g.id) {
                                continue;
                            }
                            let Some(pgn) = g.pgn.clone() else {
                                skipped += 1;
                                continue;
                            };
                            let date = g.created_at.map(|ms| {
                                let (y, m, d) = tree::days_to_ymd(ms / 86_400_000);
                                format!("{:04}-{:02}-{:02}", y, m, d)
                            });
                            match ingest_pgn_game(
                                &mut st.db,
                                &format!("lichess_{}", g.id),
                                "lichess",
                                &pgn,
                                date,
                            ) {
                                Ok(()) => new_count += 1,
                                Err(e) => {
                                    eprintln!("sync: failed to ingest lichess game {}: {e}", g.id);
                                    skipped += 1;
                                }
                            }
                            let _ = st.db.mark_synced("lichess", &g.id, &tree::local_now_str());
                        }
                        drop(st);
                        // refresh_data is Rc (non-Send), invoke on event loop thread
                        let _ = slint::invoke_from_event_loop(move || {
                            refresh_data_cp();
                        });
                        format!("Lichess: {new_count} new game(s), {skipped} skipped")
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

    // Chess.com sync
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        let refresh_data_cp = refresh_data.clone();
        app.on_sync_chesscom(move || {
            let state = state.clone();
            let app_weak = app_weak.clone();
            let refresh_data_cp = refresh_data_cp.clone();
            std::thread::spawn(move || {
                let set_status = |aw: slint::Weak<AppWindow>, msg: String| {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(app) = aw.upgrade() {
                            app.set_chesscom_sync_status(slint::SharedString::from(msg));
                        }
                    });
                };
                set_status(app_weak.clone(), "Syncing…".into());
                let username = std::env::var("CHESSCOM_USERNAME")
                    .unwrap_or_else(|_| "ElectricMindGames".into());
                let ua = std::env::var("CHESSCOM_USER_AGENT").unwrap_or_else(|_| {
                    "grandmaster-forge (contact: johncjohnson1113@gmail.com)".into()
                });
                let client = chesscom_client::ChesscomClient::new(&ua);
                let rt = tokio::runtime::Runtime::new().unwrap();
                let msg = (|| -> Result<String, String> {
                    let archives = rt.block_on(client.fetch_archives(&username))?;
                    let mut new_count = 0u32;
                    let mut skipped = 0u32;
                    // Newest months first; stop after 50 new games to bound first syncs.
                    'outer: for url in archives.iter().rev() {
                        let games = rt.block_on(client.fetch_month(url))?;
                        for g in games.iter().rev() {
                            let ext_id = chesscom_client::game_external_id(g);
                            let mut st = state.lock().unwrap();
                            if st.db.is_synced("chesscom", &ext_id) {
                                continue;
                            }
                            let Some(pgn) = g.pgn.clone() else {
                                let _ =
                                    st.db
                                        .mark_synced("chesscom", &ext_id, &tree::local_now_str());
                                skipped += 1;
                                continue;
                            };
                            let date = g.end_time.map(|secs| {
                                let (y, m, d) = tree::days_to_ymd(secs / 86_400);
                                format!("{:04}-{:02}-{:02}", y, m, d)
                            });
                            match ingest_pgn_game(
                                &mut st.db,
                                &format!("chesscom_{ext_id}"),
                                "chesscom",
                                &pgn,
                                date,
                            ) {
                                Ok(()) => new_count += 1,
                                Err(e) => {
                                    eprintln!("sync: failed to ingest chesscom game {ext_id}: {e}");
                                    skipped += 1;
                                }
                            }
                            let _ = st
                                .db
                                .mark_synced("chesscom", &ext_id, &tree::local_now_str());
                            if new_count >= 50 {
                                break 'outer;
                            }
                        }
                    }
                    Ok(format!(
                        "Chess.com: {new_count} new game(s), {skipped} skipped"
                    ))
                })()
                .unwrap_or_else(|e| format!("Chess.com sync failed: {e}"));
                let _ = slint::invoke_from_event_loop(move || {
                    refresh_data_cp();
                });
                set_status(app_weak, msg);
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

    // Load puzzle callback: picks a random puzzle for the Puzzle Trainer screen.
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_load_puzzle(move || {
            let mut st = state.lock().unwrap();
            let app = app_weak.upgrade().unwrap();
            match st.db.get_next_puzzle(2000) {
                Ok(Some(puzzle)) => {
                    let pos = puzzle
                        .fen
                        .parse::<sk_fen::Fen>()
                        .ok()
                        .and_then(|f| f.into_position::<Chess>(CastlingMode::Standard).ok());
                    let Some(pos) = pos else {
                        app.set_puzzle_status(slint::SharedString::from(format!(
                            "Puzzle {} has an invalid FEN — press New Puzzle again.",
                            puzzle.id
                        )));
                        return;
                    };
                    let solution: Vec<String> = puzzle
                        .solution_uci
                        .split_whitespace()
                        .map(str::to_string)
                        .collect();
                    if solution.is_empty() {
                        app.set_puzzle_status(slint::SharedString::from(format!(
                            "Puzzle {} has no stored solution — press New Puzzle again.",
                            puzzle.id
                        )));
                        return;
                    }
                    app.set_puzzle_board_pieces(slint::ModelRc::from(Rc::new(
                        slint::VecModel::from(fen_to_pieces(&puzzle.fen)),
                    )));
                    app.set_puzzle_selected_square(-1);
                    app.set_puzzle_meta(slint::SharedString::from(format!(
                        "{} • rating {}",
                        puzzle.theme.replace(',', ", "),
                        puzzle.difficulty
                    )));
                    app.set_puzzle_progress(slint::SharedString::from(puzzle_progress_text(
                        0,
                        solution.len(),
                    )));
                    app.set_puzzle_status(slint::SharedString::from(format!(
                        "{} to move — find the best move.",
                        side_to_move_label(&puzzle.fen)
                    )));
                    app.set_puzzle_solution_text(slint::SharedString::from(""));
                    app.set_puzzle_active(true);
                    st.puzzle_chess = pos;
                    st.puzzle_solution = solution;
                    st.puzzle_index = 0;
                    st.puzzle_failed = false;
                    st.puzzle_current = Some(puzzle);
                }
                Ok(None) => app.set_puzzle_status(slint::SharedString::from(
                    "No puzzles stored yet — starter puzzles are seeded on launch.",
                )),
                Err(e) => app.set_puzzle_status(slint::SharedString::from(format!(
                    "Failed to load a puzzle: {e}"
                ))),
            }
        });
    }

    // Fetch a small batch of real Lichess puzzles into the puzzles table.
    // Same background-thread pattern as the sync callbacks: network + DB work
    // happens off the UI thread, status text is set from the event loop.
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_fetch_lichess_puzzles(move || {
            let state = state.clone();
            let app_weak = app_weak.clone();
            std::thread::spawn(move || {
                let set_status = |aw: &slint::Weak<AppWindow>, msg: String| {
                    let aw = aw.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(app) = aw.upgrade() {
                            app.set_puzzle_fetch_status(slint::SharedString::from(msg));
                        }
                    });
                };
                let token = std::env::var("LICHESS_API_KEY").unwrap_or_default();
                if token.is_empty() {
                    set_status(
                        &app_weak,
                        "LICHESS_API_KEY is not set — add it to .env to fetch puzzles.".into(),
                    );
                    return;
                }
                set_status(&app_weak, "Fetching puzzles from Lichess…".into());
                let client = lichess_client::LichessClient::new(&token);
                let rt = tokio::runtime::Runtime::new().unwrap();
                let mut saved = 0u32;
                let mut repeats = 0u32;
                let mut skipped = 0u32;
                let mut failure: Option<String> = None;
                let mut seen_ids: Vec<String> = Vec::new();
                for _ in 0..10 {
                    match rt.block_on(client.fetch_puzzle()) {
                        Ok(fetched) => {
                            // The API may hand back a puzzle already seen in
                            // this batch; count it instead of re-storing.
                            if seen_ids.contains(&fetched.puzzle.id) {
                                repeats += 1;
                                continue;
                            }
                            seen_ids.push(fetched.puzzle.id.clone());
                            match lichess_puzzle_to_record(&fetched) {
                                Ok(record) => {
                                    let mut st = state.lock().unwrap();
                                    match st.db.insert_puzzle(&record) {
                                        Ok(()) => saved += 1,
                                        Err(e) => {
                                            eprintln!(
                                                "puzzle fetch: failed to store {}: {e}",
                                                record.id
                                            );
                                            skipped += 1;
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!(
                                        "puzzle fetch: cannot derive FEN for {}: {e}",
                                        fetched.puzzle.id
                                    );
                                    skipped += 1;
                                }
                            }
                        }
                        Err(e) => {
                            failure = Some(e);
                            break;
                        }
                    }
                }
                let msg = match failure {
                    Some(e) if saved == 0 => format!("Lichess puzzle fetch failed: {e}"),
                    Some(e) => format!("Saved {saved} puzzle(s), then fetch failed: {e}"),
                    None => format!(
                        "Lichess: {saved} puzzle(s) saved, {repeats} repeat(s), {skipped} skipped."
                    ),
                };
                set_status(&app_weak, msg);
            });
        });
    }

    // Puzzle trainer: two-click move entry checked against the stored solution.
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        let refresh_data_cp = refresh_data.clone();
        app.on_puzzle_click_square(move |idx: i32| {
            let mut st = state.lock().unwrap();
            let app = app_weak.upgrade().unwrap();
            if !app.get_puzzle_active() {
                return;
            }
            let sel = app.get_puzzle_selected_square();
            if sel == -1 {
                app.set_puzzle_selected_square(idx);
                return;
            }
            app.set_puzzle_selected_square(-1);
            let uci_try = format!(
                "{}{}",
                index_to_square(sel as usize),
                index_to_square(idx as usize)
            );
            // Legality first (queen-promotion fallback, same as drills/bot play).
            let mv = uci_try
                .parse::<Uci>()
                .ok()
                .and_then(|u| u.to_move(&st.puzzle_chess).ok())
                .or_else(|| {
                    format!("{uci_try}q")
                        .parse::<Uci>()
                        .ok()
                        .and_then(|u| u.to_move(&st.puzzle_chess).ok())
                });
            let Some(m) = mv else {
                app.set_puzzle_status(slint::SharedString::from(
                    "Illegal move — click one of your pieces, then its destination.",
                ));
                return;
            };
            let played = Uci::from_move(&m, CastlingMode::Standard).to_string();
            let Some(expected) = st.puzzle_solution.get(st.puzzle_index).cloned() else {
                return;
            };
            let mut needs_refresh = false;
            if played != expected {
                // Legal but wrong: the first miss marks the puzzle failed (once).
                if !st.puzzle_failed {
                    st.puzzle_failed = true;
                    record_puzzle_event(&mut st, false);
                    needs_refresh = true;
                }
                app.set_puzzle_status(slint::SharedString::from(
                    "Not the best move — try again, or press Show Solution.",
                ));
            } else {
                st.puzzle_chess.play_unchecked(&m);
                st.puzzle_index += 1;
                // Auto-play the opponent's scripted reply, if any.
                if st.puzzle_index < st.puzzle_solution.len() {
                    let reply = st.puzzle_solution[st.puzzle_index].clone();
                    let reply_move = reply
                        .parse::<Uci>()
                        .ok()
                        .and_then(|u| u.to_move(&st.puzzle_chess).ok());
                    let Some(rm) = reply_move else {
                        // Bad stored data — end the puzzle rather than guessing.
                        app.set_puzzle_active(false);
                        app.set_puzzle_status(slint::SharedString::from(format!(
                            "Puzzle data error: stored reply {reply} is not legal here."
                        )));
                        return;
                    };
                    st.puzzle_chess.play_unchecked(&rm);
                    st.puzzle_index += 1;
                }
                let fen = shakmaty::fen::Fen::from_position(
                    st.puzzle_chess.clone(),
                    shakmaty::EnPassantMode::Legal,
                )
                .to_string();
                app.set_puzzle_board_pieces(slint::ModelRc::from(Rc::new(slint::VecModel::from(
                    fen_to_pieces(&fen),
                ))));
                if st.puzzle_index >= st.puzzle_solution.len() {
                    app.set_puzzle_active(false);
                    app.set_puzzle_progress(slint::SharedString::from(""));
                    app.set_puzzle_status(slint::SharedString::from(if st.puzzle_failed {
                        "Solved (after a miss) — load the next one!"
                    } else {
                        "Solved! Load the next one."
                    }));
                    if !st.puzzle_failed {
                        record_puzzle_event(&mut st, true);
                        needs_refresh = true;
                    }
                } else {
                    app.set_puzzle_status(slint::SharedString::from("Correct — keep going."));
                    app.set_puzzle_progress(slint::SharedString::from(puzzle_progress_text(
                        st.puzzle_index,
                        st.puzzle_solution.len(),
                    )));
                }
            }
            drop(st);
            if needs_refresh {
                refresh_data_cp();
            }
        });
    }

    // Puzzle trainer: reveal the solution (counts as a fail).
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        let refresh_data_cp = refresh_data.clone();
        app.on_puzzle_show_solution(move || {
            let mut st = state.lock().unwrap();
            let app = app_weak.upgrade().unwrap();
            if !app.get_puzzle_active() {
                return;
            }
            let Some(puzzle) = st.puzzle_current.clone() else {
                return;
            };
            let line = solution_to_san(&puzzle.fen, &st.puzzle_solution)
                .unwrap_or_else(|| puzzle.solution_uci.clone());
            app.set_puzzle_solution_text(slint::SharedString::from(format!("Solution: {line}")));
            app.set_puzzle_active(false);
            app.set_puzzle_progress(slint::SharedString::from(""));
            app.set_puzzle_status(slint::SharedString::from(
                "Solution revealed — recorded as failed. Load a new puzzle when ready.",
            ));
            let mut needs_refresh = false;
            if !st.puzzle_failed {
                st.puzzle_failed = true;
                record_puzzle_event(&mut st, false);
                needs_refresh = true;
            }
            drop(st);
            if needs_refresh {
                refresh_data_cp();
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
            use shakmaty::fen::Fen;
            use shakmaty::uci::Uci;
            use shakmaty::{CastlingMode, Chess, EnPassantMode, Position};
            let new_fen = (|| -> Result<String, String> {
                let parsed: Fen = fen.parse().map_err(|e| format!("{e}"))?;
                let pos: Chess = parsed
                    .into_position(CastlingMode::Standard)
                    .map_err(|e| format!("{e}"))?;
                let uci_move: Uci = uci.to_string().parse().map_err(|e| format!("{e}"))?;
                let m = uci_move.to_move(&pos).map_err(|e| format!("{e}"))?;
                let next = pos.play(&m).map_err(|e| format!("{e}"))?;
                Ok(Fen::from_position(next, EnPassantMode::Always).to_string())
            })();
            if let Ok(new_fen) = new_fen {
                if let Some(app) = app_weak.upgrade() {
                    let pieces = fen_to_pieces(&new_fen);
                    app.set_board_pieces(slint::ModelRc::new(slint::VecModel::from(
                        pieces
                            .iter()
                            .map(|s| slint::SharedString::from(s.as_str()))
                            .collect::<Vec<_>>(),
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
                use shakmaty::uci::Uci;
                use shakmaty::{EnPassantMode, Position};
                let uci_move: Uci = match uci.parse() {
                    Ok(u) => u,
                    Err(_) => {
                        st.builder_selected_sq = None;
                        app.set_builder_selected_square(-1);
                        return;
                    }
                };
                match uci_move.to_move(&st.builder_chess) {
                    Ok(m) => {
                        st.builder_chess.play_unchecked(&m);
                        st.builder_staged_uci.push(uci.clone());
                        let new_fen = shakmaty::fen::Fen::from_position(
                            st.builder_chess.clone(),
                            EnPassantMode::Always,
                        )
                        .to_string();
                        let pieces = fen_to_pieces(&new_fen);
                        app.set_builder_board_pieces(slint::ModelRc::from(Rc::new(
                            slint::VecModel::from(pieces),
                        )));
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
            if st.builder_staged_uci.pop().is_none() {
                return;
            }
            // Rebuild position from start
            use shakmaty::uci::Uci;
            use shakmaty::{CastlingMode, Position};
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
                st.builder_chess.clone(),
                shakmaty::EnPassantMode::Always,
            )
            .to_string();
            let app = app_weak.upgrade().unwrap();
            let pieces = fen_to_pieces(&new_fen);
            app.set_builder_board_pieces(slint::ModelRc::from(Rc::new(slint::VecModel::from(
                pieces,
            ))));
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
            app.set_builder_board_pieces(slint::ModelRc::from(Rc::new(slint::VecModel::from(
                pieces,
            ))));
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
            app.set_builder_target_chapter_id(slint::SharedString::from(""));
        });
    }

    // builder-save-line: persist current staged line to DB
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        let refresh_data_save = refresh_data.clone();
        app.on_builder_save_line(move || {
            let mut st = state.lock().unwrap();
            if st.builder_staged_uci.is_empty() {
                return;
            }
            let app = app_weak.upgrade().unwrap();
            let start_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
            let staged = st.builder_staged_uci.clone();
            let side = st.builder_color.clone();
            let requested_chapter = app.get_builder_target_chapter_id().to_string();
            let line_title = app.get_builder_line_name().to_string();
            let result = if requested_chapter.is_empty() {
                st.db
                    .ensure_uncategorized_chapter(&side)
                    .and_then(|chapter_id| {
                        tree::import_uci_line_as_course_line(
                            &mut st.db,
                            start_fen,
                            &staged,
                            &side,
                            "manual",
                            &chapter_id,
                            if line_title.trim().is_empty() {
                                "Manual Line"
                            } else {
                                line_title.trim()
                            },
                        )
                    })
            } else {
                tree::import_uci_line_as_course_line(
                    &mut st.db,
                    start_fen,
                    &staged,
                    &side,
                    "manual",
                    &requested_chapter,
                    if line_title.trim().is_empty() {
                        "Manual Line"
                    } else {
                        line_title.trim()
                    },
                )
            };
            match result {
                Ok(_) => app.set_tree_status(slint::SharedString::from(format!(
                    "Saved line into a {side} course chapter."
                ))),
                Err(e) => {
                    app.set_tree_status(slint::SharedString::from(format!("Save failed: {e}")));
                    return;
                }
            }
            // Reset builder after save
            st.builder_chess = shakmaty::Chess::default();
            st.builder_staged_uci.clear();
            st.builder_selected_sq = None;
            drop(st);
            app.set_builder_moves(slint::SharedString::from(""));
            app.set_builder_line_name(slint::SharedString::from(""));
            let pieces = fen_to_pieces("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
            app.set_builder_board_pieces(slint::ModelRc::from(Rc::new(slint::VecModel::from(
                pieces,
            ))));
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
                use shakmaty::san::San;
                use shakmaty::{CastlingMode, Position};
                let start_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
                let parsed_start: shakmaty::fen::Fen = start_fen.parse().unwrap();
                let mut suggest_pos: shakmaty::Chess =
                    parsed_start.into_position(CastlingMode::Standard).unwrap();
                let mut first_15: Vec<String> = Vec::new();
                for san_str in parsed.moves.iter().take(15) {
                    let san_str = san_str.trim_end_matches(['?', '!', '+', '#']);
                    if let Ok(san) = san_str.parse::<San>() {
                        if let Ok(m) = san.to_move(&suggest_pos) {
                            let uci =
                                shakmaty::uci::Uci::from_move(&m, shakmaty::CastlingMode::Standard);
                            first_15.push(uci.to_string());
                            suggest_pos.play_unchecked(&m);
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                if !first_15.is_empty() {
                    app.set_builder_moves(slint::SharedString::from(first_15.join(" ")));
                    app.set_builder_line_name(slint::SharedString::from(format!(
                        "{} vs {} (opening)",
                        game.white, game.black
                    )));
                    // Load the suggested moves into builder state
                    let mut st2 = state.lock().unwrap();
                    use shakmaty::uci::Uci;
                    use shakmaty::{CastlingMode, Position};
                    let start_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
                    let parsed_fen: shakmaty::fen::Fen = start_fen.parse().unwrap();
                    let mut pos: shakmaty::Chess =
                        parsed_fen.into_position(CastlingMode::Standard).unwrap();
                    let mut valid: Vec<String> = Vec::new();
                    for uci_str in &first_15 {
                        if let Ok(uci_move) = uci_str.parse::<Uci>() {
                            if let Ok(m) = uci_move.to_move(&pos) {
                                pos.play_unchecked(&m);
                                valid.push(uci_str.clone());
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                    st2.builder_chess = pos.clone();
                    st2.builder_staged_uci = valid;
                    drop(st2);
                    let fen =
                        shakmaty::fen::Fen::from_position(pos, shakmaty::EnPassantMode::Always)
                            .to_string();
                    let pieces = fen_to_pieces(&fen);
                    app.set_builder_board_pieces(slint::ModelRc::from(Rc::new(
                        slint::VecModel::from(pieces),
                    )));
                    app.set_builder_fen(slint::SharedString::from(fen));
                }
            } else {
                app.set_builder_line_name(slint::SharedString::from("No games imported yet"));
            }
        });
    }

    // Import pasted PGN with variations into the tree
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        let refresh = refresh_data.clone();
        app.on_import_lines(move |pgn: slint::SharedString, side: slint::SharedString| {
            let app = app_weak.upgrade().unwrap();
            if pgn.trim().is_empty() {
                app.set_import_status(slint::SharedString::from("Nothing to import."));
                return;
            }
            let requested_chapter = app.get_import_target_chapter_id().to_string();
            let side = side.to_string();
            let mut st = state.lock().unwrap();
            let chapter = if requested_chapter.is_empty() {
                st.db.ensure_uncategorized_chapter(&side)
            } else {
                Ok(requested_chapter)
            };
            let result = chapter.and_then(|chapter_id| {
                tree::import_pgn_as_course_lines(&mut st.db, &pgn, &side, "pgn", &chapter_id)
            });
            match result {
                Ok((new_edges, named_lines)) => {
                    drop(st);
                    app.set_import_status(slint::SharedString::from(format!(
                        "Imported {new_edges} new move(s) and {named_lines} named line(s) into {side} courses."
                    )));
                    refresh();
                }
                Err(e) => {
                    app.set_import_status(slint::SharedString::from(format!("Import failed: {e}")))
                }
            }
        });
    }

    // Sync Lichess studies into the tree
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        let refresh = refresh_data.clone();
        app.on_sync_studies(move |side: slint::SharedString| {
            let requested_chapter = app_weak
                .upgrade()
                .map(|app| app.get_import_target_chapter_id().to_string())
                .unwrap_or_default();
            let state = state.clone();
            let app_weak = app_weak.clone();
            let refresh = refresh.clone();
            let side = side.to_string();
            std::thread::spawn(move || {
                let aw = app_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = aw.upgrade() {
                        app.set_import_status(slint::SharedString::from("Fetching studies…"));
                    }
                });
                let token = std::env::var("LICHESS_API_KEY").unwrap_or_default();
                let username =
                    std::env::var("LICHESS_USERNAME").unwrap_or_else(|_| "hackandtoss".into());
                let client = lichess_client::LichessClient::new(&token);
                let rt = tokio::runtime::Runtime::new().unwrap();
                let msg = match rt.block_on(client.fetch_studies_pgn(&username)) {
                    Ok(pgn_text) => {
                        let mut st = state.lock().unwrap();
                        let chapter = if requested_chapter.is_empty() {
                            st.db.ensure_uncategorized_chapter(&side)
                        } else {
                            Ok(requested_chapter.clone())
                        };
                        match chapter.and_then(|chapter_id| {
                            tree::import_pgn_as_course_lines(
                                &mut st.db,
                                &pgn_text,
                                &side,
                                "study",
                                &chapter_id,
                            )
                        }) {
                            Ok((new_edges, named_lines)) => format!(
                                "Studies: {new_edges} new move(s), {named_lines} named line(s)."
                            ),
                            Err(e) => format!("Study import failed: {e}"),
                        }
                    }
                    Err(e) => format!("Studies fetch failed: {e}"),
                };
                let aw = app_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = aw.upgrade() {
                        app.set_import_status(slint::SharedString::from(msg));
                    }
                    refresh();
                });
            });
        });
    }

    // Adopt master lines from the builder's current position
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        let refresh = refresh_data.clone();
        app.on_adopt_explorer(move |side: slint::SharedString| {
            let app = app_weak.upgrade().unwrap();
            let start_fen = app.get_builder_fen().to_string();
            let side = side.to_string();
            let state = state.clone();
            let app_weak = app_weak.clone();
            let refresh = refresh.clone();
            std::thread::spawn(move || {
                let aw = app_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = aw.upgrade() {
                        app.set_tree_status(slint::SharedString::from("Adopting master lines…"));
                    }
                });
                let token = std::env::var("LICHESS_API_KEY").unwrap_or_default();
                let client = lichess_client::LichessClient::new(&token);
                let rt = tokio::runtime::Runtime::new().unwrap();
                let (pairs, stop_reason) = rt.block_on(scraper::adopt_explorer_lines(
                    &client, &start_fen, &side, 8, 3,
                ));
                let msg = {
                    let mut st = state.lock().unwrap();
                    let mut new_edges = 0u32;
                    for (parent_fen, uci) in &pairs {
                        match tree::add_move_edge(&mut st.db, parent_fen, uci, &side, "explorer") {
                            Ok((_, _, was_new)) => {
                                if was_new {
                                    new_edges += 1;
                                }
                            }
                            Err(e) => {
                                eprintln!("adopt: failed to insert edge {parent_fen} {uci}: {e}");
                            }
                        }
                    }
                    match stop_reason {
                        None => format!("Adopted {new_edges} new move(s) from master games."),
                        Some(reason) => {
                            format!("Adopted {new_edges} new move(s) — stopped early: {reason}")
                        }
                    }
                };
                let aw = app_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = aw.upgrade() {
                        app.set_tree_status(slint::SharedString::from(msg));
                    }
                    refresh();
                });
            });
        });
    }

    // Play-vs-bot: start a new game.
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_play_new_game(move |color: slint::SharedString, elo: i32| {
            let mut st = state.lock().unwrap();
            let app = app_weak.upgrade().unwrap();
            st.play_chess = Chess::default();
            st.play_moves_uci.clear();
            st.play_side = color.to_string();
            st.play_session = None; // rebuilt lazily with the chosen Elo
            st.play_deviations.clear();
            st.play_plies_in_book = 0;
            st.play_left_book_at = None;
            app.set_play_elo(elo);
            app.set_play_active(true);
            app.set_play_summary(slint::SharedString::from(""));
            app.set_play_board_pieces(slint::ModelRc::from(Rc::new(slint::VecModel::from(
                fen_to_pieces("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR"),
            ))));
            let msg = if st.play_side == "Black" {
                bot_take_turn(&mut st, &app) // bot is White, moves first
            } else {
                "Your move.".to_string()
            };
            app.set_play_status(slint::SharedString::from(msg));
        });
    }

    // Play-vs-bot: two-click move; validates legality; book-deviation grading; then bot replies.
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        let refresh_data = refresh_data.clone();
        app.on_play_click_square(move |idx: i32| {
            let mut st = state.lock().unwrap();
            let app = app_weak.upgrade().unwrap();
            if !app.get_play_active() {
                return;
            }
            let sel = app.get_play_selected_square();
            if sel == -1 {
                app.set_play_selected_square(idx);
                return;
            }
            app.set_play_selected_square(-1);
            let uci_try = format!(
                "{}{}",
                index_to_square(sel as usize),
                index_to_square(idx as usize)
            );

            // Legality first (try promotion to queen as fallback).
            let mv = uci_try
                .parse::<Uci>()
                .ok()
                .and_then(|u| u.to_move(&st.play_chess).ok())
                .or_else(|| {
                    format!("{uci_try}q")
                        .parse::<Uci>()
                        .ok()
                        .and_then(|u| u.to_move(&st.play_chess).ok())
                });
            let Some(m) = mv else {
                app.set_play_status(slint::SharedString::from("Illegal move."));
                return;
            };
            let played_uci = Uci::from_move(&m, CastlingMode::Standard).to_string();

            // Book check BEFORE playing: was this position in book, and did we deviate?
            let key = tree::position_key(&st.play_chess);
            let side = st.play_side.clone();
            let my_edges: Vec<_> = st
                .db
                .get_repertoire_moves_from(&key, &side)
                .unwrap_or_default()
                .into_iter()
                .filter(|e| e.is_my_move)
                .collect();
            if !my_edges.is_empty() {
                if my_edges.iter().any(|e| e.uci == played_uci) {
                    st.play_plies_in_book += 1;
                } else {
                    let today_days = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                        / 86400;
                    let expected: Vec<String> = my_edges.iter().map(|e| e.san.clone()).collect();
                    for e in &my_edges {
                        let _ = tree::grade_edge_with_event(
                            &mut st.db,
                            e,
                            1,
                            today_days,
                            "bot_deviation",
                        );
                    }
                    let move_no = st.play_moves_uci.len() / 2 + 1;
                    st.play_deviations.push(format!(
                        "Move {}: played {played_uci}, book expected {}",
                        move_no,
                        expected.join(" / ")
                    ));
                    if st.play_left_book_at.is_none() {
                        st.play_left_book_at = Some(st.play_moves_uci.len() as u32);
                    }
                }
            }

            st.play_chess.play_unchecked(&m);
            st.play_moves_uci.push(played_uci);
            let fen = shakmaty::fen::Fen::from_position(
                st.play_chess.clone(),
                shakmaty::EnPassantMode::Legal,
            )
            .to_string();
            app.set_play_board_pieces(slint::ModelRc::from(Rc::new(slint::VecModel::from(
                fen_to_pieces(&fen),
            ))));

            use shakmaty::Position;
            if st.play_chess.is_game_over() {
                let pgn = finish_play_game(&mut st, &app, "Game over");
                drop(st);
                if let Some(pgn) = pgn {
                    import_and_analyze_pgn(&state, &app_weak, &refresh_data, &pgn, false);
                }
                return;
            }
            let msg = bot_take_turn(&mut st, &app);
            let ended = msg.contains("ended") || msg.contains("Engine") || {
                use shakmaty::Position;
                st.play_chess.is_game_over()
            };
            app.set_play_status(slint::SharedString::from(msg));
            if ended {
                let pgn = finish_play_game(&mut st, &app, "Game over");
                drop(st);
                if let Some(pgn) = pgn {
                    import_and_analyze_pgn(&state, &app_weak, &refresh_data, &pgn, false);
                }
            }
        });
    }

    // Play-vs-bot: resign.
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        let refresh_data = refresh_data.clone();
        app.on_play_resign(move || {
            let mut st = state.lock().unwrap();
            let app = app_weak.upgrade().unwrap();
            let pgn = finish_play_game(&mut st, &app, "Resigned");
            drop(st);
            if let Some(pgn) = pgn {
                import_and_analyze_pgn(&state, &app_weak, &refresh_data, &pgn, false);
            }
        });
    }

    {
        let app_weak = app_weak.clone();
        app.on_game_review_close(move || {
            if let Some(app) = app_weak.upgrade() {
                app.set_show_game_review_brief(false);
            }
        });
    }

    {
        let app_weak = app_weak.clone();
        app.on_game_review_open(move || {
            if let Some(app) = app_weak.upgrade() {
                let game_id = app.get_game_review_game_id();
                app.set_active_screen(slint::SharedString::from("review"));
                app.invoke_select_game(game_id);
                app.set_show_game_review_brief(false);
            }
        });
    }

    // Send the finished bot game through the normal import/analysis pipeline.
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_play_review_game(move || {
            let st = state.lock().unwrap();
            let moves = st.play_moves_uci.clone();
            let side = st.play_side.clone();
            let (result_tag, _) = bot_game_outcome(&st.play_chess, &side, false);
            drop(st);
            let Some(pgn) = build_bot_game_pgn(&moves, &side, result_tag) else {
                return;
            };
            if let Some(app) = app_weak.upgrade() {
                app.invoke_import_pgn(slint::SharedString::from(pgn));
                app.set_active_screen(slint::SharedString::from("review"));
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

/// Bot plays one move: from the user's book if possible, else via Stockfish PlaySession.
/// Returns a status string for the UI.
fn bot_take_turn(state: &mut AppState, app: &AppWindow) -> String {
    use shakmaty::Position;
    if state.play_chess.is_game_over() {
        return "Game over.".to_string();
    }
    let key = tree::position_key(&state.play_chess);
    let edges = state
        .db
        .get_repertoire_moves_from(&key, &state.play_side)
        .unwrap_or_default();
    let book: Vec<_> = edges.into_iter().filter(|e| !e.is_my_move).collect();

    let uci_str = if !book.is_empty() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as usize;
        state.play_plies_in_book += 1;
        app.set_play_book_state(slint::SharedString::from("In book (your prep)"));
        book[nanos % book.len()].uci.clone()
    } else {
        if state.play_left_book_at.is_none() {
            state.play_left_book_at = Some(state.play_moves_uci.len() as u32);
        }
        app.set_play_book_state(slint::SharedString::from("Out of book (Stockfish)"));
        if state.play_session.is_none() {
            let elo = app.get_play_elo().max(1320) as u32;
            match engine_controller::PlaySession::new(
                &state.stockfish_path,
                engine_controller::PlayConfig {
                    elo: Some(elo),
                    movetime_ms: 400,
                },
            ) {
                Ok(s) => state.play_session = Some(s),
                Err(e) => {
                    return format!("Engine unavailable — book-only mode ended the game. ({e})")
                }
            }
        }
        match state
            .play_session
            .as_mut()
            .unwrap()
            .best_move_from(&state.play_moves_uci)
        {
            Ok(m) => m,
            Err(e) => return format!("Engine error: {e}"),
        }
    };

    let Ok(uci) = uci_str.parse::<Uci>() else {
        return format!("Bot produced bad move {uci_str} — game ended.");
    };
    let Ok(m) = uci.to_move(&state.play_chess) else {
        // Mock engine can emit illegal moves in arbitrary positions; end gracefully.
        return format!("Bot move {uci_str} not legal here — game ended.");
    };
    state.play_chess.play_unchecked(&m);
    state.play_moves_uci.push(uci_str.clone());
    let fen =
        shakmaty::fen::Fen::from_position(state.play_chess.clone(), shakmaty::EnPassantMode::Legal)
            .to_string();
    app.set_play_board_pieces(slint::ModelRc::from(std::rc::Rc::new(
        slint::VecModel::from(fen_to_pieces(&fen)),
    )));
    format!("Bot played {uci_str}. Your move.")
}

/// End the bot game: build the summary, determine the outcome, show the Game
/// Review Brief (Task 5 wires the Slint side of this), and return the PGN to
/// auto-analyze — the caller must drop its state lock before importing it
/// (see the call sites below).
fn finish_play_game(state: &mut AppState, app: &AppWindow, reason: &str) -> Option<String> {
    app.set_play_active(false);
    let total = state.play_moves_uci.len();
    let mut summary = format!(
        "{reason} after {total} plies. Book moves played: {}.",
        state.play_plies_in_book
    );
    if let Some(ply) = state.play_left_book_at {
        summary.push_str(&format!(" Left book at ply {}.", ply + 1));
    }
    if state.play_deviations.is_empty() {
        summary.push_str(" No deviations from your repertoire — clean prep!");
    } else {
        summary.push_str(&format!(
            " Deviations (now due for review): {}",
            state.play_deviations.join(" | ")
        ));
    }
    app.set_play_summary(slint::SharedString::from(summary));

    let (result_tag, header) =
        bot_game_outcome(&state.play_chess, &state.play_side, reason == "Resigned");
    // A 0-move game has nothing for `build_bot_game_pgn` to return (see below)
    // and thus nothing for the background analysis thread to ever populate —
    // opening the Brief here would leave it permanently stuck on "Analyzing…".
    // Only open it when there's an actual game to analyze.
    if !state.play_moves_uci.is_empty() {
        app.set_game_review_result_text(slint::SharedString::from(header));
        app.set_show_game_review_brief(true);
        app.set_game_review_game_id(slint::SharedString::from(""));
        app.set_game_review_stats(slint::ModelRc::from(Rc::new(slint::VecModel::from(Vec::<
            ReviewStatEntry,
        >::new(
        )))));
        app.set_game_review_accuracy_text(slint::SharedString::from(""));
        app.set_review_status(slint::SharedString::from("Analyzing with Stockfish..."));
    }

    build_bot_game_pgn(&state.play_moves_uci, &state.play_side, result_tag)
}

/// Import a finished/pasted PGN, persist it, and spawn Stockfish analysis in
/// the background. `switch_to_review` controls whether the UI jumps to the
/// Review screen immediately (manual "Import & Analyze Game" / "Review This
/// Game" clicks) or stays put (auto-triggered right after a bot game, where
/// the Game Review Brief overlay is shown instead — see Task 5).
fn import_and_analyze_pgn(
    state: &Arc<Mutex<AppState>>,
    app_weak: &slint::Weak<AppWindow>,
    refresh_data_cp: &Arc<impl Fn() + Send + Sync + 'static>,
    pgn_text: &str,
    switch_to_review: bool,
) {
    if pgn_text.trim().is_empty() {
        return;
    }

    let mut state_lock = state.lock().unwrap();
    let parsed = pgn_processor::parse_pgn(pgn_text);

    let white = parsed
        .headers
        .get("White")
        .cloned()
        .unwrap_or_else(|| "WhitePlayer".to_string());
    let black = parsed
        .headers
        .get("Black")
        .cloned()
        .unwrap_or_else(|| "BlackPlayer".to_string());
    let date = parsed
        .headers
        .get("Date")
        .cloned()
        .unwrap_or_else(|| "2026-06-12".to_string());
    let round = parsed
        .headers
        .get("Round")
        .filter(|r| !r.is_empty() && *r != "?" && *r != "-");
    let game_id = match round {
        Some(r) => format!("{}_{}_{}_{}", white, black, date, r).replace(" ", "_"),
        None => format!("{}_{}_{}", white, black, date).replace(" ", "_"),
    };

    if let Err(e) = ingest_pgn_game(
        &mut state_lock.db,
        &game_id,
        "Imported PGN",
        pgn_text,
        Some(date),
    ) {
        drop(state_lock);
        if let Some(app) = app_weak.upgrade() {
            app.set_review_status(slint::SharedString::from(format!("Import failed: {e}")));
        }
        return;
    }

    // Trigger background thread to analyze and evaluate this game!
    let sf_path = state_lock.stockfish_path.clone();
    let state_thread = state.clone();
    let refresh_thread = refresh_data_cp.clone();
    let game_id_thread = game_id.clone();
    let app_weak_thread = app_weak.clone();

    thread::spawn(move || {
        let mut sf = match StockfishEngine::new(&sf_path) {
            Ok(engine) => engine,
            Err(e) => {
                let app_weak_error = app_weak_thread.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = app_weak_error.upgrade() {
                        app.set_review_status(slint::SharedString::from(format!(
                            "Stockfish failed: {e}"
                        )));
                    }
                });
                return;
            }
        };

        let positions = {
            let state = state_thread.lock().unwrap();
            state
                .db
                .get_positions_for_game(&game_id_thread)
                .unwrap_or_default()
        };

        let mut losses = Vec::new();

        for pos in &positions {
            let is_book = {
                let state = state_thread.lock().unwrap();
                is_book_move(&state.db, &pos.fen, &pos.played_move)
            };
            // Analyze position
            let eval_res = sf.analyze_fen(
                &pos.fen,
                AnalysisConfig {
                    depth: 8,
                    multipv: 3,
                },
            );
            if let Ok(analysis) = eval_res {
                if !analysis.variations.is_empty() {
                    let best_score = score_to_centipawns(analysis.variations[0].score);

                    // Find played move score
                    let played_score = if analysis.bestmove == pos.played_move {
                        best_score
                    } else if let Some(v) = analysis
                        .variations
                        .iter()
                        .find(|var| !var.pv.is_empty() && var.pv[0] == pos.played_move)
                    {
                        score_to_centipawns(v.score)
                    } else {
                        // Evaluate position after played move
                        let mut played_chess: Chess = pos
                            .fen
                            .parse::<sk_fen::Fen>()
                            .map(|f| f.into_position(CastlingMode::Standard).unwrap())
                            .unwrap();
                        let uci: Uci = pos.played_move.parse().unwrap();
                        let m = uci.to_move(&played_chess).unwrap();
                        played_chess.play_unchecked(&m);
                        let next_fen = shakmaty::fen::Fen::from_position(
                            played_chess,
                            shakmaty::EnPassantMode::Always,
                        )
                        .to_string();

                        if let Ok(next_analysis) = sf.analyze_fen(
                            &next_fen,
                            AnalysisConfig {
                                depth: 8,
                                multipv: 1,
                            },
                        ) {
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

                    let class = Some(format!(
                        "{:?}",
                        engine_controller::classify_review_move(
                            loss,
                            analysis.bestmove == pos.played_move,
                            best_score,
                            is_book,
                        )
                    ));

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

        // Mistake tree: worst USER eval drop -> best move, top-3 replies, follow-ups.
        let user_side = tree::user_side_for_game(&white, &black);
        let worst = positions
            .iter()
            .zip(losses.iter())
            .filter(|(pos, _)| match &user_side {
                // ply 1,3,5.. = White's moves (odd plies)
                Some(s) => (pos.ply % 2 == 1) == (s == "White"),
                None => true,
            })
            .max_by_key(|(_, loss)| **loss);
        if let Some((pos_record, &loss)) = worst {
            if loss >= 60 {
                let mover_is_white = pos_record.fen.split_whitespace().nth(1) == Some("w");
                let side = if mover_is_white { "White" } else { "Black" }.to_string();
                // 1) correction (my move)
                if let Ok(root_analysis) = sf.analyze_fen(
                    &pos_record.fen,
                    AnalysisConfig {
                        depth: 12,
                        multipv: 1,
                    },
                ) {
                    let best = root_analysis.bestmove.clone();
                    let mut st = state_thread.lock().unwrap();
                    match tree::add_move_edge(&mut st.db, &pos_record.fen, &best, &side, "mistake")
                    {
                        Ok((_, after_best, _)) => {
                            drop(st);
                            // 2) top-3 opponent replies
                            if let Ok(reply_analysis) = sf.analyze_fen(
                                &after_best,
                                AnalysisConfig {
                                    depth: 10,
                                    multipv: 3,
                                },
                            ) {
                                let mut seen = std::collections::HashSet::new();
                                for var in reply_analysis.variations.iter().take(3) {
                                    let Some(reply) = var.pv.first() else {
                                        continue;
                                    };
                                    if !seen.insert(reply.clone()) {
                                        continue;
                                    }
                                    let mut st = state_thread.lock().unwrap();
                                    let Ok((_, after_reply, _)) = tree::add_move_edge(
                                        &mut st.db,
                                        &after_best,
                                        reply,
                                        &side,
                                        "mistake",
                                    ) else {
                                        continue;
                                    };
                                    drop(st);
                                    // 3) my best follow-up to each reply
                                    if let Ok(fu) = sf.analyze_fen(
                                        &after_reply,
                                        AnalysisConfig {
                                            depth: 10,
                                            multipv: 1,
                                        },
                                    ) {
                                        let mut st = state_thread.lock().unwrap();
                                        let _ = tree::add_move_edge(
                                            &mut st.db,
                                            &after_reply,
                                            &fu.bestmove,
                                            &side,
                                            "mistake",
                                        );
                                    }
                                }
                            }
                            let mut st = state_thread.lock().unwrap();
                            let event = TrainingEventRecord {
                                id: format!("evt_{}", uuid_now()),
                                user_id: "default_user".to_string(),
                                kind: "mistake_tree".to_string(),
                                target_id: game_id_thread.clone(),
                                outcome: format!("worst drop {loss}cp at ply {}", pos_record.ply),
                                score_delta: 0.0,
                                created_at: tree::local_now_str(),
                            };
                            let _ = st.db.insert_training_event(&event);
                        }
                        Err(e) => {
                            eprintln!(
                                "mistake-tree: failed to insert correction edge for game {}: {}",
                                game_id_thread, e
                            );
                        }
                    }
                }
            }
        }

        let updated_positions = {
            let state = state_thread.lock().unwrap();
            state
                .db
                .get_positions_for_game(&game_id_thread)
                .unwrap_or_default()
        };
        let class_names: Vec<&str> = updated_positions
            .iter()
            .filter_map(|pos| pos.mistake_class.as_deref())
            .collect();
        let stats = review_stat_entries(&class_names);
        let accuracy = {
            let state = state_thread.lock().unwrap();
            state
                .db
                .conn
                .query_row(
                    "SELECT accuracy_overall FROM games WHERE id = ?1",
                    [game_id_thread.as_str()],
                    |row| row.get::<_, Option<f64>>(0),
                )
                .ok()
                .flatten()
                .map(|value| value as f32)
        };
        let accuracy_text = accuracy_summary_text(accuracy);

        // Trigger UI refresh
        let _ = slint::invoke_from_event_loop(move || {
            refresh_thread();
            if let Some(app) = app_weak_thread.upgrade() {
                app.invoke_select_game(slint::SharedString::from(game_id_thread.clone()));
                let brief_open = app.get_show_game_review_brief();
                let brief_id = app.get_game_review_game_id();
                if brief_open && brief_id.is_empty() {
                    // First population right after auto-trigger: record which
                    // game this open brief refers to.
                    app.set_game_review_game_id(slint::SharedString::from(game_id_thread.clone()));
                }
                // Only overwrite the stat grid/accuracy text if the open brief is
                // either unattributed yet or already attributed to *this* game —
                // otherwise a slow background analysis for game A can clobber a
                // Brief that a Task 6 "Review" click (or another import) has since
                // repointed at a different game B.
                if brief_open
                    && (brief_id.is_empty() || brief_id.as_str() == game_id_thread.as_str())
                {
                    app.set_game_review_stats(slint::ModelRc::from(Rc::new(
                        slint::VecModel::from(stats),
                    )));
                    app.set_game_review_accuracy_text(slint::SharedString::from(accuracy_text));
                }
            }
        });
    });

    // Local updates
    drop(state_lock);
    refresh_data_cp();
    if let Some(app) = app_weak.upgrade() {
        if switch_to_review {
            app.set_active_screen(slint::SharedString::from("review"));
        }
        app.set_review_status(slint::SharedString::from("Analyzing with Stockfish..."));
        app.invoke_select_game(slint::SharedString::from(game_id));
    }
}

/// Advance the drill: auto-play opponent edges, load expected my-moves, detect session end.
fn drill_advance(state: &mut AppState, app: &AppWindow) {
    loop {
        let key = tree::position_key(&state.drill_chess);
        let edges = state
            .db
            .get_repertoire_moves_from(&key, &state.drill_side)
            .unwrap_or_default();
        let (mine, theirs): (Vec<_>, Vec<_>) = edges.into_iter().partition(|e| e.is_my_move);

        let side_to_move_is_mine =
            (state.drill_chess.turn() == shakmaty::Color::White) == (state.drill_side == "White");

        if side_to_move_is_mine {
            if mine.is_empty() {
                app.set_drill_active(false);
                app.set_drill_instructions(slint::SharedString::from("Line complete — well done!"));
                return;
            }
            app.set_drill_branch_info(slint::SharedString::from(format!(
                "{} accepted move(s) here",
                mine.len()
            )));
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
            .subsec_nanos() as usize
            % theirs.len();
        let mv = &theirs[pick];
        if let Ok(uci) = mv.uci.parse::<shakmaty::uci::Uci>() {
            if let Ok(m) = uci.to_move(&state.drill_chess) {
                state.drill_chess.play_unchecked(&m);
                let fen = shakmaty::fen::Fen::from_position(
                    state.drill_chess.clone(),
                    shakmaty::EnPassantMode::Legal,
                )
                .to_string();
                app.set_board_pieces(slint::ModelRc::from(std::rc::Rc::new(
                    slint::VecModel::from(fen_to_pieces(&fen)),
                )));
                continue;
            }
        }
        app.set_drill_active(false);
        return;
    }
}

#[cfg(test)]
mod tests {
    use super::{
        accuracy_summary_text, bot_game_outcome, build_bot_game_pgn, course_progress_text,
        fen_to_pieces, game_review_summary, lichess_puzzle_to_record, puzzle_progress_text,
        review_stat_entries, sk_fen, solution_to_san, starter_puzzles, updated_puzzle_rating,
    };
    use shakmaty::uci::Uci;
    use shakmaty::{CastlingMode, Chess, Position};

    #[test]
    fn fen_to_pieces_preserves_piece_side_codes() {
        let pieces = fen_to_pieces("r7/8/8/8/8/8/P7/K7 w - - 0 1");
        let as_strings: Vec<String> = pieces.iter().map(|piece| piece.to_string()).collect();

        assert_eq!(as_strings[0], "r");
        assert_eq!(as_strings[48], "P");
        assert_eq!(as_strings[56], "K");
        assert_eq!(as_strings[63], "");
    }

    #[test]
    fn game_review_summary_lists_accuracy_elo_and_move_counts() {
        let summary = game_review_summary(
            Some(87.4),
            &[
                "Book",
                "Blunder",
                "Mistake",
                "Miss",
                "Good",
                "Excellent",
                "Best",
                "Great",
                "Brilliant",
            ],
        );

        assert!(summary.contains("Accuracy 87%"));
        assert!(summary.contains("Est. Elo 2300"));
        assert!(summary.contains("Book 1"));
        assert!(summary.contains("Blunder 1"));
        assert!(summary.contains("Mistake 1"));
        assert!(summary.contains("Miss 1"));
        assert!(summary.contains("Good 1"));
        assert!(summary.contains("Excellent 1"));
        assert!(summary.contains("Best 1"));
        assert!(summary.contains("Great 1"));
        assert!(summary.contains("Brilliant 1"));
    }

    #[test]
    fn bot_game_outcome_reports_win_from_the_users_perspective() {
        // Fool's mate: 1. f3 e5 2. g4 Qh4# — White is checkmated.
        let chess: Chess = "rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 0 3"
            .parse::<sk_fen::Fen>()
            .unwrap()
            .into_position(CastlingMode::Standard)
            .unwrap();

        let (tag, header) = bot_game_outcome(&chess, "White", false);
        assert_eq!(tag, "0-1");
        assert_eq!(header, "Forge Bot won");

        let (tag, header) = bot_game_outcome(&chess, "Black", false);
        assert_eq!(tag, "0-1");
        assert_eq!(header, "You beat Forge Bot!");
    }

    #[test]
    fn bot_game_outcome_resignation_always_credits_the_other_side() {
        let chess = Chess::default();

        let (tag, header) = bot_game_outcome(&chess, "White", true);
        assert_eq!(tag, "0-1");
        assert_eq!(header, "Forge Bot won");

        let (tag, header) = bot_game_outcome(&chess, "Black", true);
        assert_eq!(tag, "1-0");
        assert_eq!(header, "Forge Bot won");
    }

    #[test]
    fn bot_game_outcome_unfinished_position_is_unresolved() {
        let chess = Chess::default();
        let (tag, header) = bot_game_outcome(&chess, "White", false);
        assert_eq!(tag, "*");
        assert_eq!(header, "Game over");
    }

    #[test]
    fn build_bot_game_pgn_embeds_the_real_result_tag() {
        let moves = vec!["e2e4".to_string(), "e7e5".to_string()];
        let pgn = build_bot_game_pgn(&moves, "White", "1-0").unwrap();
        assert!(pgn.contains("1. e4 e5 1-0"));
        assert!(!pgn.trim_end().ends_with('*'));
    }

    #[test]
    fn build_bot_game_pgn_returns_none_for_an_empty_game() {
        assert!(build_bot_game_pgn(&[], "White", "*").is_none());
    }

    #[test]
    fn review_stat_entries_lists_only_nonzero_categories_in_display_order() {
        let entries = review_stat_entries(&["Book", "Book", "Blunder", "Best"]);
        let as_pairs: Vec<(String, i32)> = entries
            .into_iter()
            .map(|e| (e.label.to_string(), e.count))
            .collect();
        assert_eq!(
            as_pairs,
            vec![
                ("Best".to_string(), 1),
                ("Book".to_string(), 2),
                ("Blunder".to_string(), 1),
            ]
        );
    }

    #[test]
    fn review_stat_entries_empty_for_no_classified_moves() {
        assert!(review_stat_entries(&[]).is_empty());
    }

    #[test]
    fn accuracy_summary_text_matches_existing_format() {
        assert_eq!(
            accuracy_summary_text(Some(87.4)),
            "Accuracy 87% | Est. Elo 2300"
        );
        assert_eq!(
            accuracy_summary_text(None),
            "Accuracy pending | Est. Elo pending"
        );
    }

    #[test]
    fn course_progress_text_reports_mastered_and_due_lines() {
        assert_eq!(course_progress_text(3, 2, 1), "2/3 mastered - 1 due");
        assert_eq!(course_progress_text(3, 3, 0), "3/3 mastered");
    }

    #[test]
    fn starter_puzzles_replay_legally_and_end_in_checkmate() {
        let puzzles = starter_puzzles();
        assert!(!puzzles.is_empty());
        for puzzle in puzzles {
            let parsed: sk_fen::Fen = puzzle
                .fen
                .parse()
                .unwrap_or_else(|e| panic!("{}: bad FEN: {e}", puzzle.id));
            let mut pos: Chess = parsed
                .into_position(CastlingMode::Standard)
                .unwrap_or_else(|e| panic!("{}: illegal position: {e}", puzzle.id));
            let moves: Vec<&str> = puzzle.solution_uci.split_whitespace().collect();
            assert!(!moves.is_empty(), "{}: empty solution", puzzle.id);
            for uci_str in &moves {
                let uci: Uci = uci_str
                    .parse()
                    .unwrap_or_else(|e| panic!("{}: bad UCI {uci_str}: {e}", puzzle.id));
                let m = uci
                    .to_move(&pos)
                    .unwrap_or_else(|e| panic!("{}: illegal move {uci_str}: {e}", puzzle.id));
                pos = pos
                    .play(&m)
                    .unwrap_or_else(|e| panic!("{}: cannot play {uci_str}: {e}", puzzle.id));
            }
            assert!(
                pos.is_checkmate(),
                "{}: solution should end in checkmate",
                puzzle.id
            );
        }
    }

    #[test]
    fn solution_to_san_renders_the_philidor_finish() {
        let san = solution_to_san(
            "5r1k/6pp/7N/8/8/1Q6/5PP1/6K1 w - - 0 1",
            &["b3g8".to_string(), "f8g8".to_string(), "h6f7".to_string()],
        )
        .expect("line should replay");
        assert_eq!(san, "Qg8 Rxg8 Nf7");
    }

    #[test]
    fn solution_to_san_rejects_illegal_lines() {
        assert!(solution_to_san(
            "6k1/5ppp/8/8/8/8/5PPP/3R2K1 w - - 0 1",
            &["d1e3".to_string()]
        )
        .is_none());
        assert!(solution_to_san("not a fen", &["e2e4".to_string()]).is_none());
    }

    #[test]
    fn lichess_puzzle_to_record_follows_stored_convention() {
        let fetched = lichess_client::puzzles::LichessPuzzle {
            puzzle: lichess_client::puzzles::PuzzleData {
                id: "abc12".to_string(),
                solution: vec!["h5f7".to_string()],
                themes: vec!["mate".to_string(), "mateIn1".to_string()],
                rating: 1420,
                initial_ply: 5,
            },
            game: lichess_client::puzzles::PuzzleGame {
                pgn: "e4 e5 Bc4 Nc6 Qh5 Nf6".to_string(),
            },
        };
        let record = lichess_puzzle_to_record(&fetched).unwrap();
        assert_eq!(record.id, "lichess_abc12");
        // Same position as seed_scholars_mate: FEN is the position to solve
        // with the solver (White) to move.
        assert_eq!(
            record.fen,
            "r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 4 4"
        );
        assert_eq!(record.solution_uci, "h5f7");
        assert_eq!(record.theme, "mate,mateIn1");
        assert_eq!(record.difficulty, 1420);

        let mut truncated = fetched.clone();
        truncated.puzzle.solution.clear();
        assert!(lichess_puzzle_to_record(&truncated).is_err());
    }

    #[test]
    fn updated_puzzle_rating_moves_up_and_down_with_bounds() {
        assert_eq!(updated_puzzle_rating(1500, true), 1510);
        assert_eq!(updated_puzzle_rating(1500, false), 1490);
        assert_eq!(updated_puzzle_rating(400, false), 400);
        assert_eq!(updated_puzzle_rating(3200, true), 3200);
    }

    #[test]
    fn puzzle_progress_text_counts_solver_moves_only() {
        assert_eq!(puzzle_progress_text(0, 1), "Move 1 of 1");
        assert_eq!(puzzle_progress_text(0, 3), "Move 1 of 2");
        assert_eq!(puzzle_progress_text(2, 3), "Move 2 of 2");
    }
}

// Temporary alias to bridge name discrepancy in shakmaty FEN parsing
mod sk_fen {
    pub use shakmaty::fen::Fen;
}
