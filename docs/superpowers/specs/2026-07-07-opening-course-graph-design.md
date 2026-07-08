# Opening Course Graph Design

**Date:** 2026-07-07
**Status:** Approved direction, draft spec for review

## Goal

Build the opening trainer as a ChessReps-style course catalog backed by Grandmaster Forge's existing transposition-aware repertoire graph. The user should see organized courses, chapters, named lines, progress bars, and a focused practice view. The app should store positions once, merge transpositions correctly, and let FSRS/move mastery drive honest line progress.

## Design Choice

Use a **Course-First Graph** architecture.

- The UI is course-first: cards, chapters, named lines, progress, practice modes.
- The model is graph-first underneath: named lines are curated paths through `repertoire_nodes` and `repertoire_moves`.
- Transpositions dedupe at the position level, so two human lines can share the same position without creating duplicate learning state.
- This avoids a flat list of openings while keeping the current DAG-style repertoire model.

Rejected alternatives:

- Graph-first explorer: powerful, but not friendly enough for learning and less like the desired ChessReps flow.
- Flat course lines: visually quick, but breaks transpositions, book-bot lookup, and analytics.

## User-Facing Structure

### Catalog

The catalog shows cards similar to the screenshots:

- board thumbnail from a representative course position;
- title, short description, and side tags such as White, Black, vs e4, vs d4;
- badges such as Personal, Imported, Community, Mastery, New, Weak;
- line count and progress bar;
- primary action: Start Learning, Continue, Review Due, or Start Mastering.

Special catalog entry:

- **Dojo**: mixed training across every learned repertoire move, weighted by due items and weak lines.

Filters should include side, opening family, source, status, due review, and weak lines. Search should match course names, chapter names, line names, ECO/opening tags, and descriptions.

### Course Detail

Opening a course shows chapters first. This is the approved inside-course entry point.

Example course:

- Vienna Gambit
  - Main Line
  - Accepted Gambit
  - Declined Lines
  - Early Queen Responses
  - Common Mistakes
  - Mastery Extension

Each chapter shows named lines, line status, move count, due count, and last practiced date. The user can start the whole course, one chapter, or one named line.

### Practice View

The practice view follows the ChessReps feel:

- large board;
- prompt panel: "What's the best move?";
- course name, current line name, and line number;
- course progress and line progress;
- line selector with PGN preview;
- settings menu.

V1 settings:

- show evaluation bar;
- show arrows / hints;
- select line;
- copy PGN;
- copy FEN;
- open in Lichess;
- reset course progress.

Skip sounds, confetti, haptics, piece themes, and board themes in V1. They are nice, but not core to learning.

## Data Model

Conceptual model:

```text
Course Pack -> Chapter -> Named Line -> Graph Path
Graph Path -> repertoire_moves / repertoire_nodes
```

Course packs are user-visible containers. Chapters group lines by learning purpose. Named lines are human-curated paths through the graph. The graph remains the source of truth for legal positions and move ownership.

Minimum course fields:

- id
- title
- description
- side: White, Black, or Mixed
- source: personal, imported, study, explorer, community, mistake, built-in
- opening tags / ECO tags
- thumbnail position FEN
- created / updated timestamps

Minimum chapter fields:

- id
- course id
- title
- description
- sort order

Minimum named-line fields:

- id
- chapter id
- title
- description
- root node id
- ordered move-edge ids for the curated path
- sort order
- optional PGN text for export/display

Ponytail constraint: do not introduce a separate graph model. Course lines reference the existing repertoire graph.

## Progress And Mastery

Progress is strict and honest.

- Individual user-owned move edges drive scheduling and mastery.
- FSRS is the target scheduler. Until FSRS lands, the current SM-2 state can stand in as the backing due signal.
- Line progress is derived from the required user moves in that line.
- If any required user move is due, the whole line shows as needing review.

Line statuses:

- `Undiscovered`: line exists but user has not entered it.
- `Learning`: user has started but has not passed every required move.
- `Learned`: every required user move has been passed at least once.
- `Mastered`: every required user move has stable scheduler state.
- `Review Due`: line was learned/mastered, but one or more required moves are due.
- `Weak`: repeated failures, recent game mistake, or bot-play deviation occurred in the line.

Course progress derives from line statuses:

- discovered lines / total lines;
- learned lines / total lines;
- mastered lines / total lines;
- due lines count;
- weak lines count.

The catalog can show `57/57 mastered`, but that number drops as soon as any line has a due required move.

## Training Modes

- **Learn Course**: introduces undiscovered or learning lines in course order.
- **Review Due**: drills due moves from lines already learned or mastered.
- **Master Course**: drills all required moves until every line reaches Mastered.
- **Select Line**: user picks a specific named line from the line selector.
- **Weak Lines**: focuses on lines with failures, blunders, or bot deviations.
- **Dojo**: mixes due and weak moves across all learned courses.

Accepted answers follow current branch-quizzing behavior: if multiple course lines share a position and several user moves are valid repertoire moves, any matching prepared move is accepted and the session follows that branch.

## Imports And Course Creation

Course content can come from:

- manual builder;
- pasted PGN;
- PGN files;
- Lichess studies;
- Lichess explorer adoption;
- game-review mistake trees;
- future built-in/community-style packs.

Import should ask whether the content becomes:

- a new course;
- a new chapter in an existing course;
- new named lines inside an existing chapter;
- uncategorized repertoire material to organize later.

Imported lines should preserve names when available from PGN headers, study chapters, or variation comments. If no name exists, generate a boring name from the first few moves.

## Bot Integration

Book bot lookup uses the same graph:

- in book, bot chooses opponent-owned edges from the current course/repertoire context;
- if practicing a specific course, prefer moves from that course's named lines;
- when course book ends, Stockfish takes over;
- user deviations inside a course mark the matching line as Weak or Review Due and create scheduler pressure.

## Analytics

Course analytics should answer:

- which courses are due today;
- which lines are weak;
- which openings produce the most game-review mistakes;
- which lines fail most often in drills;
- where transpositions connect courses;
- how much of the repertoire is learned, mastered, due, or weak.

The analytics dashboard should consume derived course/line status rather than maintaining a second progress system.

## Phasing

### Phase 1: Course Shell

- Add course, chapter, and named-line metadata around the existing graph.
- Show catalog cards and course detail chapter list.
- Support manual assignment of existing repertoire edges to a course line.

### Phase 2: Practice View

- Build the course practice view: big board, prompt panel, line selector, progress, and minimal settings.
- Drill a course, chapter, or line using existing branch-quizzing behavior.
- Derive line status from current scheduler state.

### Phase 3: Imports Into Courses

- Let PGN, study, explorer, and manual builder flows target courses/chapters/lines.
- Preserve or generate named-line labels.
- Add copy PGN, copy FEN, and open-in-Lichess actions.

### Phase 4: Mastery And Review

- Replace SM-2 with FSRS for move scheduling.
- Make line/course status derive from FSRS state.
- Add Review Due, Weak Lines, Master Course, and Dojo modes.

### Phase 5: Built-In / Community-Style Library

- Add seedable local course packs such as Vienna Gambit, Traxler Counterattack, common e4 responses, and Black repertoires.
- Treat them as local content packs first; no remote community marketplace until the local format is stable.

## Testing

- A named line references existing graph edges without duplicating positions.
- Two named lines that transpose share node/move state where appropriate.
- A line becomes Review Due when any required user move is due.
- A line returns to Mastered when due required moves are passed.
- Course progress changes when a child line changes state.
- Course-specific bot play prefers course edges while in book and hands off to Stockfish when out of book.
- Import creates courses, chapters, and named lines without breaking existing repertoire graph invariants.

## Open Questions

- Which seed course should be first: Vienna Gambit, Traxler Counterattack, or common responses to e4?
- Should user-created courses and built-in packs use the same visual badges, or should built-ins feel separate?
- Should mastery require a minimum number of successful reviews, or is stable FSRS state enough?
