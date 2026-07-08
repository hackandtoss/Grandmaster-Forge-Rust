# Tactical Scrutinizer Design

**Date:** 2026-07-07
**Status:** Draft for discussion; no code changes requested yet

## Goal

Turn analyzed mistakes from the user's own games into tactical training material with short, deterministic explanations. Stockfish decides whether a move was bad. The scrutinizer labels why it was likely bad using chess geometry, then feeds that label into the existing mistake-tree, drill, and weakness-priority loops.

## Non-goals

- No LLM-generated chess advice.
- No custom board representation; use `shakmaty`.
- No new UI-heavy coaching system in the first pass.
- No clustering dependency until there are enough labeled mistakes to cluster.

## Current Repo Baseline

- Game review already stores per-ply positions and mistake classes in `positions`.
- Stockfish analysis already identifies centipawn loss and move classes.
- Mistake trees already create due-now study material from the user's worst eval drop in a game.
- The repertoire is already a transposition-aware graph in `repertoire_nodes` / `repertoire_moves`.

## Trigger

Run the tactical scrutinizer only after engine analysis confirms a meaningful user mistake:

- Source position: position before the user's move.
- Played move: user's move.
- Engine correction: best move from Stockfish.
- Threshold: reuse the existing mistake/blunder classification first; tune later only with real false positives.

## Motif Set For V1

Start with labels that can be checked cheaply and explained without prose generation:

- Hanging piece: the played move leaves a piece attacked more than defended or fails to recapture a loose piece.
- Fork: the engine correction creates a simultaneous attack on two valuable targets.
- Pin: a line piece attacks through a piece to a king, queen, or rook.
- Skewer: a high-value piece is attacked in line with a lower-value piece behind it.
- Overloaded defender: one defender was responsible for multiple tactical duties and the move breaks one.
- Back-rank weakness: king has limited escape squares and a rook/queen file or rank tactic is available.
- Missed forcing move: engine correction is check, capture, or direct mate threat and the played move is quiet.

Anything uncertain should fall back to `tactical_theme = "unclear"` rather than inventing an explanation.

## Data Shape

Keep this as metadata on review/study positions, not a new chess model.

Minimum useful fields:

- game id
- ply
- side
- FEN before the mistake
- played UCI
- engine best UCI
- centipawn loss / mate swing if available
- primary motif label
- optional secondary motif labels
- source = `game_review`

Ponytail constraint: do not add a many-table taxonomy system for seven labels. One compact table or JSON/text metadata column is enough until the UI needs more.

## Study Integration

When a motif is detected:

1. Attach the motif to the mistake-tree root or generated correction edge.
2. Make the correction due immediately, same as current mistake-tree behavior.
3. Show the motif in review/history surfaces where a mistake is already visible.
4. Aggregate motif counts and average loss for the weakness dashboard.

The first version can display labels only. Human-readable explanations can be fixed templates such as "Your move left the knight undefended" once the detector can name the piece and square.

## Testing

Use a few tiny FEN fixtures per motif. Each fixture should assert:

- the legal position parses through `shakmaty`;
- the played move and engine correction are legal;
- the detector returns the expected primary motif;
- uncertain positions return `unclear` instead of a false confident label.

## Open Questions

- Which labels do you actually want to see first in the UI: motif names only, or one-sentence templates?
- Should the app prioritize repeated motif frequency, total centipawn loss, or recent failures when deciding the next training priority?
- Should missed tactics from winning positions be treated as urgently as blunders from equal positions?
