# Agent Notes

- Use `shakmaty` for board legality, FEN, SAN, and UCI. Do not add a custom board model.
- Repertoire data is a transposition-aware graph: `repertoire_nodes` plus `repertoire_moves`. Do not rebuild it as a tree.
- Course work should add metadata over the graph: Course -> Chapter -> Named Line -> existing graph path.
- Use Stockfish/UCI for evaluation truth. Do not use LLMs for chess evaluation or move explanations.
- Coach text should be short deterministic templates from engine results and chess geometry.
- Current scheduler is SM-2 on user-owned move edges. FSRS is the target replacement; do not run two schedulers indefinitely.
- Keep normalized FEN as canonical position identity. Add Zobrist only as a measured cache/index.
- First implementation order:
  1. course metadata and derived line status;
  2. course UI/import routing;
  3. normalized review events.
- Keep `PROJECT_TRACKER.md`, `.env`, runtime DBs, Graphify output, and AI tool folders local-only.
