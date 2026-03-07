# Grandmaster Forge Rust

A high-performance chess analysis and training suite targeting modern AMD hardware.

## Project Identity
- **Name**: Grandmaster-Forge-Rust
- **Primary language**: Rust 2021 Edition
- **Optimization flags**: BMI2, AVX2, target-cpu=native

## Architecture Goals
1. Engine integration with Stockfish 18 via UCI using NNUE.
2. Store 10M+ master games in SQLite/DuckDB for analytics.
3. UI built using Iced or Slint with hardware acceleration.
4. Support multi-PV analysis.

## Crates
- `engine_controller`
- `pgn_processor`
- `mastery_ui`
- `db_manager`

## Data Directories
- `/tablebases`
- `/pgn_storage`
- `/user_stats`
