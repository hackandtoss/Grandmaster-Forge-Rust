# Grandmaster Forge Rust

A chess training platform focused on mastery through opening depth, game review, error-driven learning, and personalization.

## Product targets
1. Opening library with deep, drillable lines (ChessReps-style plus engine and master-game coverage).
2. Automated game review that detects first inaccuracy/mistake/blunder and generates corrective repertoires.
3. Master-game mining at scale and mistake-derived training line generation.
4. Personalized drills: opening repetition, blunder prevention, and adaptive puzzles.

## Workspace crates
- `engine_controller`: UCI/Stockfish integration, multi-PV analysis, centipawn-loss classification.
- `pgn_processor`: PGN parsing, move extraction, first-critical-mistake detection.
- `db_manager`: normalized data contracts and SQL schema bootstrap for games, positions, lines, puzzles, events.
- `mastery_ui`: training-mode routing logic and UI shell hooks for Iced/Slint.

## Suggested architecture
1. Ingestion pipeline:
   - Import PGN archives into `pgn_storage`.
   - Parse games and normalize into `games` + `positions` tables.
2. Analysis pipeline:
   - Run Stockfish multi-PV over user-game critical positions.
   - Compute centipawn loss and classify inaccuracies/mistakes/blunders.
3. Line generation:
   - For each first critical mistake, store engine best line plus follow-up moves.
   - Merge with master-game continuations and opening names/ECO tags.
4. Personalization loop:
   - Track outcomes in `training_events`.
   - Prioritize sessions by: unreviewed games -> blunder hotspots -> due openings -> puzzles.

## Current status
- Core APIs and data models are scaffolded across all crates.
- Unit tests are included for parsers/classifiers/selection logic.
- External runtime wiring (real DB driver, GUI runtime, stockfish process integration in app flow) is next.

## Next implementation steps
1. Add a concrete SQLite backend in `db_manager` (`rusqlite`) and run schema bootstrap.
2. Wire `pgn_processor` to engine analysis for mistake-triggered line insertion.
3. Add one executable crate (`app`) to orchestrate import -> analyze -> train recommendations.
4. Add an actual Iced/Slint front-end screen for:
   - Repertoire browser
   - Game review timeline
   - Daily training queue
