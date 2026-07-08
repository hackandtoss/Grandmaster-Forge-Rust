# Learning Scheduler And Position Identity Design

**Date:** 2026-07-07
**Status:** Draft for discussion; no code changes requested yet

## Goal

Keep the app's learning model transposition-safe and local-first while planning two future upgrades: FSRS scheduling and optional hash-based position lookup. The immediate rule is boring: preserve the working SQLite + normalized-FEN graph until real usage proves a need to change it.

## Current Repo Baseline

- Position graph: `repertoire_nodes` and `repertoire_moves`.
- Position key today: normalized FEN with clocks removed.
- Scheduler today: SM-2 fields on each user-owned move edge.
- Database today: bundled SQLite via `rusqlite`.
- Board logic: `shakmaty`.

## Position Identity Rule

The semantic rule is:

> Same legal position for the same repertoire side maps to the same node.

Normalized FEN already provides this. A future Zobrist `u64` should be treated as a cache/index for speed, not as a different source of truth.

V1 rule:

- Keep normalized FEN as the canonical stored position identity.
- Add a Zobrist column/index only if profiling shows FEN lookups are a real bottleneck.
- If added, compute it from a `shakmaty` position or a tiny tested helper, never from a custom board model.
- Keep a collision-safe path: unique FEN remains available to verify or break ties.

## Database Rule

Do not migrate to `sled` or `sqlx` just because they are plausible Rust choices.

- Keep `rusqlite` while the app is local-first and relational queries are useful.
- Revisit storage only if SQLite becomes a measured bottleneck or the schema becomes painful for the training queries.
- If key-value storage is ever added, limit it to cache-like derived data such as engine results or position hashes.

## FSRS Migration Rule

FSRS should replace SM-2, not run beside it indefinitely.

Expected mapping:

- Correct first try: high FSRS rating.
- Correct after hesitation/hint: middle FSRS rating, if the UI captures that signal.
- Wrong move or repertoire deviation: low FSRS rating.
- Blunder repeated in game-condition bot play: low FSRS rating plus motif metadata.

Migration shape:

1. Keep existing SM-2 data until FSRS fields and review events are ready.
2. Add FSRS state to user-owned move edges or a compact linked review-state table.
3. Convert old SM-2 due dates conservatively so nothing disappears from the queue.
4. Remove SM-2 scheduling reads after FSRS is live.

Ponytail constraint: no scheduler trait or plugin system unless the app truly supports multiple schedulers. One scheduler in production is enough.

## Weakness Clustering Rule

Start with deterministic aggregates:

- motif frequency;
- average centipawn loss by motif;
- recent failure rate;
- opening/ECO bucket;
- phase bucket.

Add `linfa` only after tactical labels exist and there is enough data for clustering to produce something better than sorted counts.

## Testing

For any future implementation:

- position identity tests must prove transpositions collide intentionally;
- hash/index tests must prove FEN remains the collision-safe fallback;
- FSRS tests must prove failed reviews become due soon and successful reviews move out;
- migration tests must prove existing due SM-2 items remain due after conversion.

## Open Questions

- Do you want FSRS to schedule individual move edges only, or also tactical motifs as separate "skills"?
- Should bot-play deviations count the same as drill failures, or heavier because they happened under game conditions?
- Is a Zobrist key valuable for interoperability/export in your workflow, or only for performance?
