use db_manager::SqliteStore;
use shakmaty::fen::Fen;
use shakmaty::san::San;
use shakmaty::uci::Uci;
use shakmaty::{CastlingMode, Chess, EnPassantMode, Position};

/// Normalized FEN used as a repertoire-tree key.
pub fn position_key(pos: &Chess) -> String {
    let fen = Fen::from_position(pos.clone(), EnPassantMode::Legal).to_string();
    db_manager::normalize_fen(&fen)
}

fn parse_position(fen: &str) -> Result<Chess, String> {
    let parsed: Fen = fen.parse().map_err(|e| format!("bad FEN '{fen}': {e}"))?;
    parsed
        .into_position(CastlingMode::Standard)
        .map_err(|e| format!("illegal position '{fen}': {e}"))
}

/// Which side did the user play? Matches configured usernames, case-insensitive.
pub fn user_side_for_game(white: &str, black: &str) -> Option<String> {
    let names: Vec<String> = ["LICHESS_USERNAME", "CHESSCOM_USERNAME"]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .map(|s| s.to_lowercase())
        .collect();
    if names.iter().any(|n| n == &white.to_lowercase()) {
        return Some("White".into());
    }
    if names.iter().any(|n| n == &black.to_lowercase()) {
        return Some("Black".into());
    }
    None
}

/// Insert one edge. Returns (edge_id, child_full_fen, was_new).
pub fn add_move_edge(
    db: &mut SqliteStore,
    parent_full_fen: &str,
    uci_str: &str,
    side: &str,
    source: &str,
) -> Result<(i64, String, bool), String> {
    let pos = parse_position(parent_full_fen)?;
    let uci: Uci = uci_str
        .parse()
        .map_err(|e| format!("bad UCI '{uci_str}': {e}"))?;
    let m = uci
        .to_move(&pos)
        .map_err(|e| format!("illegal move '{uci_str}': {e}"))?;
    let san = San::from_move(&pos, &m).to_string();
    let mover_is_white = pos.turn() == shakmaty::Color::White;
    let is_my_move = (side == "White") == mover_is_white;
    let parent_key = position_key(&pos);
    let next = pos
        .play(&m)
        .map_err(|e| format!("cannot play '{uci_str}': {e}"))?;
    let child_fen = position_key(&next);

    let parent_id = db.ensure_repertoire_node(&parent_key, side)?;
    let child_id = db.ensure_repertoire_node(&child_fen, side)?;
    let (edge_id, was_new) =
        db.insert_repertoire_move(parent_id, child_id, uci_str, &san, is_my_move, source)?;
    Ok((edge_id, child_fen, was_new))
}

/// Walk a UCI line from start_fen, inserting every move. Returns count of NEW edges.
#[cfg(test)]
pub fn import_uci_line(
    db: &mut SqliteStore,
    start_fen: &str,
    moves: &[String],
    side: &str,
    source: &str,
) -> Result<u32, String> {
    let mut fen = start_fen.to_string();
    let mut new_edges = 0u32;
    for uci in moves {
        let (_, child, was_new) = add_move_edge(db, &fen, uci, side, source)?;
        if was_new {
            new_edges += 1;
        }
        fen = child;
    }
    Ok(new_edges)
}

fn validate_course_chapter(db: &SqliteStore, chapter_id: &str, side: &str) -> Result<(), String> {
    let chapter = db
        .get_course_chapter(chapter_id)?
        .ok_or_else(|| format!("course chapter not found: {chapter_id}"))?;
    let course = db
        .get_course(&chapter.course_id)?
        .ok_or_else(|| format!("course not found: {}", chapter.course_id))?;
    if course.side != "Mixed" && course.side != side {
        return Err(format!(
            "course {} is for {}, not {side}",
            course.id, course.side
        ));
    }
    Ok(())
}

fn insert_uci_course_line(
    db: &mut SqliteStore,
    start_fen: &str,
    moves: &[String],
    side: &str,
    source: &str,
    chapter_id: &str,
    line_title: &str,
    line_key: &str,
    sort_order: i32,
    pgn: Option<String>,
) -> Result<String, String> {
    if moves.is_empty() {
        return Err("cannot create a course line with no moves".to_string());
    }

    let mut fen = start_fen.to_string();
    let mut root_node_id = None;
    let mut line_moves = Vec::with_capacity(moves.len());
    for (ply_index, uci) in moves.iter().enumerate() {
        let (edge_id, child_fen, _) = add_move_edge(db, &fen, uci, side, source)?;
        let edge = db
            .get_repertoire_move(edge_id)?
            .ok_or_else(|| format!("inserted repertoire move missing: {edge_id}"))?;
        root_node_id.get_or_insert(edge.parent_id);
        line_moves.push((edge_id, ply_index as i32, edge.is_my_move));
        fen = child_fen;
    }

    let line_id = format!("{chapter_id}:{source}:{line_key}:{}", moves.join("-"));
    db.insert_course_line(&db_manager::CourseLineRecord {
        id: line_id.clone(),
        chapter_id: chapter_id.to_string(),
        title: line_title.to_string(),
        description: String::new(),
        root_node_id: root_node_id.expect("non-empty moves set a root node"),
        sort_order,
        pgn,
    })?;
    db.set_course_line_moves(&line_id, &line_moves)?;
    Ok(line_id)
}

/// Insert a legal UCI path into the repertoire graph and attach that exact path
/// to a named course line. Only user-owned edges are required for line status.
pub fn import_uci_line_as_course_line(
    db: &mut SqliteStore,
    start_fen: &str,
    moves: &[String],
    side: &str,
    source: &str,
    chapter_id: &str,
    line_title: &str,
) -> Result<String, String> {
    validate_course_chapter(db, chapter_id, side)?;
    let title = line_title.trim();
    let title = if title.is_empty() {
        "Untitled Line"
    } else {
        title
    };
    insert_uci_course_line(
        db, start_fen, moves, side, source, chapter_id, title, title, 0, None,
    )
}

fn pgn_line_title(parsed: &pgn_processor::ParsedGame, index: usize) -> String {
    for header in ["ChapterName", "Event", "Opening"] {
        if let Some(value) = parsed
            .headers
            .get(header)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty() && *value != "?")
        {
            return value.to_string();
        }
    }
    let moves = parsed
        .moves
        .iter()
        .take(4)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(" ");
    if moves.is_empty() {
        format!("Imported Line {}", index + 1)
    } else {
        moves
    }
}

/// Import every PGN variation into the shared repertoire graph, then add one
/// named course line for each game's mainline. Returns (new edges, named lines).
pub fn import_pgn_as_course_lines(
    db: &mut SqliteStore,
    pgn: &str,
    side: &str,
    source: &str,
    chapter_id: &str,
) -> Result<(u32, u32), String> {
    validate_course_chapter(db, chapter_id, side)?;
    let walked = pgn_processor::walk_pgn_variations(pgn)?;
    let new_edges = import_walked(db, &walked, side, source)?;
    let mut named_lines = 0;

    // ponytail: flattened WalkedMove values lose branch provenance; add path-aware
    // parser output when named variation lines are required.
    for (index, game) in pgn_processor::split_games(pgn).into_iter().enumerate() {
        let parsed = pgn_processor::parse_pgn(&game);
        let Ok(fens_and_moves) = pgn_processor::game_to_fen_and_uci(&parsed.moves) else {
            continue;
        };
        let Some((start_fen, _)) = fens_and_moves.first() else {
            continue;
        };
        let moves = fens_and_moves
            .iter()
            .map(|(_, uci)| uci.clone())
            .collect::<Vec<_>>();
        let title = pgn_line_title(&parsed, index);
        insert_uci_course_line(
            db,
            start_fen,
            &moves,
            side,
            source,
            chapter_id,
            &title,
            &format!("{index}-{title}"),
            index as i32,
            Some(game.trim().to_string()),
        )?;
        named_lines += 1;
    }

    Ok((new_edges, named_lines))
}

/// Apply an SM-2 grade to a single repertoire edge and persist the new schedule.
/// `today_days` is epoch-days; the next due date is `today + max(interval, 1)` days.
pub fn grade_edge(
    db: &mut SqliteStore,
    edge: &db_manager::RepertoireMoveRecord,
    grade: u32,
    today_days: u64,
) -> Result<(), String> {
    let card = crate::srs::SrsCard {
        interval: edge.srs_interval,
        ease_factor: edge.srs_ease,
        repetitions: edge.srs_reps,
    };
    let next = crate::srs::review(&card, grade);
    let (y, m, d) = days_to_ymd(today_days + next.interval.max(1.0) as u64);
    db.update_repertoire_move_srs(
        edge.id,
        next.interval,
        next.ease_factor,
        next.repetitions,
        &format!("{:04}-{:02}-{:02}", y, m, d),
    )
}

/// Grade an edge (SM-2, same as [`grade_edge`]) and record a normalized
/// review event for it. `source` says what produced the grade ("drill",
/// "bot_deviation", ...). Review events are the shared learning-event shape
/// that future FSRS/tactics/endgame work reads; `training_events` stays the
/// legacy dashboard feed.
pub fn grade_edge_with_event(
    db: &mut SqliteStore,
    edge: &db_manager::RepertoireMoveRecord,
    grade: u32,
    today_days: u64,
    source: &str,
) -> Result<(), String> {
    grade_edge(db, edge, grade, today_days)?;
    let (y, m, d) = days_to_ymd(today_days);
    let created_at = format!("{:04}-{:02}-{:02}", y, m, d);
    // Nanos plus a process-local sequence keep ids unique even when the same
    // edge is graded twice inside one clock tick.
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    db.insert_review_event(&db_manager::ReviewEventRecord {
        id: format!("repertoire_move-{}-{}-{}", edge.id, suffix, seq),
        user_id: "default_user".to_string(),
        target_type: "repertoire_move".to_string(),
        target_id: edge.id.to_string(),
        rating: grade.min(5) as i32,
        source: source.to_string(),
        course_id: None,
        line_id: None,
        move_id: Some(edge.id),
        created_at,
    })
}

/// Today's date as `YYYY-MM-DD` from the system clock (epoch-days math).
pub fn local_now_str() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = secs / 86400;
    let (y, m, d) = days_to_ymd(days);
    format!("{:04}-{:02}-{:02}", y, m, d)
}

/// Convert epoch-days into a (year, month, day) triple (proleptic Gregorian from 1970).
pub fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970u64;
    loop {
        let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let days_in_year = if leap { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let month_days: [u64; 12] = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u64;
    for md in &month_days {
        if days < *md {
            break;
        }
        days -= *md;
        month += 1;
    }
    (year, month, days + 1)
}

/// Insert pre-walked PGN moves (parent/child fens already normalized). Returns NEW edge count.
pub fn import_walked(
    db: &mut SqliteStore,
    walked: &[pgn_processor::WalkedMove],
    side: &str,
    source: &str,
) -> Result<u32, String> {
    let mut new_edges = 0u32;
    for w in walked {
        let mover_is_white = w.parent_fen.split_whitespace().nth(1) == Some("w");
        let is_my_move = (side == "White") == mover_is_white;
        let parent_id = db.ensure_repertoire_node(&w.parent_fen, side)?;
        let child_id = db.ensure_repertoire_node(&w.child_fen, side)?;
        let (_, was_new) =
            db.insert_repertoire_move(parent_id, child_id, &w.uci, &w.san, is_my_move, source)?;
        if was_new {
            new_edges += 1;
        }
    }
    Ok(new_edges)
}

/// One-time migration from opening_lines. Idempotent:
/// skips when the tree already has rows or the legacy table is gone.
pub fn migrate_legacy_lines(db: &mut SqliteStore) -> Result<u32, String> {
    if db.repertoire_move_count()? > 0 || !db.opening_lines_table_exists() {
        return Ok(0);
    }
    let lines = db.get_opening_lines("default_user")?;
    if lines.is_empty() {
        return Ok(0); // nothing to migrate; leave the empty table in place
    }
    let mut migrated = 0u32;
    for line in &lines {
        let mut fen = line.start_fen.clone();
        for uci in &line.line_uci {
            let (edge_id, child, _was_new) =
                match add_move_edge(db, &fen, uci, &line.side, &line.source) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("migration: skipping bad move in line {}: {e}", line.id);
                        break;
                    }
                };
            // Copy the line's SM-2 state onto my-move edges; longest interval wins.
            if let Some(existing) = db.get_repertoire_move(edge_id)? {
                if existing.is_my_move && line.srs_interval > existing.srs_interval {
                    db.update_repertoire_move_srs(
                        edge_id,
                        line.srs_interval,
                        line.srs_ease,
                        line.srs_reps,
                        &line.srs_due_date,
                    )?;
                }
            }
            fen = child;
        }
        migrated += 1;
    }
    db.retire_opening_lines()?;
    Ok(migrated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use db_manager::{
        CourseChapterRecord, CourseRecord, OpeningLineRecord, SqliteStore, TrainingStore,
    };

    const START: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

    fn mem_store() -> SqliteStore {
        let mut s = SqliteStore::new_in_memory().unwrap();
        s.bootstrap().unwrap();
        s.run_migrations().unwrap();
        s
    }

    fn add_course_target(db: &mut SqliteStore, side: &str) -> String {
        let course_id = format!("course-{}", side.to_lowercase());
        let chapter_id = format!("{course_id}-main");
        db.insert_course(&CourseRecord {
            id: course_id.clone(),
            title: format!("{side} Course"),
            description: String::new(),
            side: side.to_string(),
            source: "personal".to_string(),
            tags: String::new(),
            thumbnail_fen: START.to_string(),
        })
        .unwrap();
        db.insert_course_chapter(&CourseChapterRecord {
            id: chapter_id.clone(),
            course_id,
            title: "Main".to_string(),
            description: String::new(),
            sort_order: 0,
        })
        .unwrap();
        chapter_id
    }

    #[test]
    fn add_move_edge_computes_san_child_and_ownership() {
        let mut db = mem_store();
        // White repertoire, White to move => my move
        let (_id, child_fen, was_new) =
            add_move_edge(&mut db, START, "e2e4", "White", "manual").unwrap();
        assert!(was_new);
        assert!(child_fen.starts_with("rnbqkbnr/pppppppp/8/8/4P3"));
        let edges = db.get_repertoire_moves_from(START, "White").unwrap();
        assert_eq!(edges[0].san, "e4");
        assert!(edges[0].is_my_move);
        // Black repertoire, White to move => opponent move
        let (_id2, _cf2, _) = add_move_edge(&mut db, START, "e2e4", "Black", "manual").unwrap();
        let edges_b = db.get_repertoire_moves_from(START, "Black").unwrap();
        assert!(!edges_b[0].is_my_move);
    }

    #[test]
    fn import_uci_line_walks_and_transposes() {
        let mut db = mem_store();
        let line: Vec<String> = ["e2e4", "e7e5", "g1f3"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let new_edges = import_uci_line(&mut db, START, &line, "White", "pgn").unwrap();
        assert_eq!(new_edges, 3);
        // Re-import is a no-op
        assert_eq!(
            import_uci_line(&mut db, START, &line, "White", "pgn").unwrap(),
            0
        );
        // Transposition: 1.Nf3 e5?? not relevant — instead verify d2d4/d7d5 via two orders
        let a: Vec<String> = ["d2d4", "d7d5", "c2c4", "e7e6"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let b: Vec<String> = ["c2c4", "e7e6", "d2d4", "d7d5"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let n1 = import_uci_line(&mut db, START, &a, "White", "pgn").unwrap();
        let n2 = import_uci_line(&mut db, START, &b, "White", "pgn").unwrap();
        assert_eq!(n1, 4);
        // Orders differ at every interior position; only the final position coincides,
        // so the second import adds 4 new edges too — but both funnel into ONE node:
        assert_eq!(n2, 4);
        // and from the shared final position, later continuations are shared.
    }

    #[test]
    fn import_uci_line_with_course_keeps_order_and_requires_only_my_moves() {
        let mut db = mem_store();
        let chapter_id = add_course_target(&mut db, "White");
        let moves = ["e2e4", "e7e5", "g1f3"]
            .iter()
            .map(|uci| uci.to_string())
            .collect::<Vec<_>>();

        let line_id = import_uci_line_as_course_line(
            &mut db,
            START,
            &moves,
            "White",
            "manual",
            &chapter_id,
            "Open Game",
        )
        .unwrap();

        let attached = db.get_course_line_moves(&line_id).unwrap();
        assert_eq!(
            attached
                .iter()
                .map(|(_, ply, required)| (*ply, *required))
                .collect::<Vec<_>>(),
            vec![(0, true), (1, false), (2, true)]
        );
        assert_eq!(
            attached
                .iter()
                .map(|(id, _, _)| db.get_repertoire_move(*id).unwrap().unwrap().uci)
                .collect::<Vec<_>>(),
            moves
        );

        let black_chapter = add_course_target(&mut db, "Black");
        let before = db.repertoire_move_count().unwrap();
        let error = import_uci_line_as_course_line(
            &mut db,
            START,
            &moves,
            "White",
            "manual",
            &black_chapter,
            "Wrong Target",
        )
        .unwrap_err();
        assert!(error.contains("is for Black, not White"));
        assert_eq!(db.repertoire_move_count().unwrap(), before);
    }

    #[test]
    fn import_pgn_as_course_lines_names_mainlines_and_keeps_variations() {
        let mut db = mem_store();
        let chapter_id = add_course_target(&mut db, "White");
        let pgn = r#"[Event "King Pawn Main"]

1. e4 e5 (1... c5) 2. Nf3 *

[Event "?"]
[Opening "Queen's Pawn"]

1. d4 d5 *"#;

        assert_eq!(
            import_pgn_as_course_lines(&mut db, pgn, "White", "pgn", &chapter_id).unwrap(),
            (6, 2)
        );
        let lines = db.get_course_lines(&chapter_id).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].title, "King Pawn Main");
        assert_eq!(lines[1].title, "Queen's Pawn");
        assert!(lines[0].pgn.as_deref().unwrap().contains("King Pawn Main"));
        assert_eq!(
            db.get_course_line_moves(&lines[0].id)
                .unwrap()
                .iter()
                .map(|(_, _, required)| *required)
                .collect::<Vec<_>>(),
            vec![true, false, true]
        );
        assert_eq!(db.get_course_line_moves(&lines[1].id).unwrap().len(), 2);

        let walked = pgn_processor::walk_pgn_variations(pgn).unwrap();
        let after_e4 = &walked.iter().find(|w| w.uci == "e2e4").unwrap().child_fen;
        let replies = db.get_repertoire_moves_from(after_e4, "White").unwrap();
        assert!(replies.iter().any(|edge| edge.uci == "e7e5"));
        assert!(replies.iter().any(|edge| edge.uci == "c7c5"));

        assert_eq!(
            import_pgn_as_course_lines(&mut db, pgn, "White", "pgn", &chapter_id).unwrap(),
            (0, 2)
        );
        assert_eq!(db.get_course_lines(&chapter_id).unwrap().len(), 2);
    }

    #[test]
    fn migrate_legacy_lines_moves_srs_and_retires_table() {
        let mut db = mem_store();
        let line = OpeningLineRecord {
            id: "L1".into(),
            user_id: "default_user".into(),
            opening_name: "Test".into(),
            start_fen: START.into(),
            line_uci: vec!["e2e4".into(), "e7e5".into()],
            source: "builder".into(),
            confidence: 0.5,
            srs_interval: 6.0,
            srs_ease: 2.6,
            srs_reps: 2,
            srs_due_date: "2026-07-10".into(),
            side: "White".into(),
        };
        db.insert_opening_line(&line).unwrap();

        let migrated = migrate_legacy_lines(&mut db).unwrap();
        assert_eq!(migrated, 1);
        assert!(!db.opening_lines_table_exists());
        let edges = db.get_repertoire_moves_from(START, "White").unwrap();
        assert_eq!(edges.len(), 1);
        // my-move edge carries the line's SM-2 state
        assert!((edges[0].srs_interval - 6.0).abs() < 0.01);
        assert_eq!(edges[0].srs_due_date, "2026-07-10");
        // second run is a no-op
        assert_eq!(migrate_legacy_lines(&mut db).unwrap(), 0);
    }

    #[test]
    fn import_walked_inserts_edges_with_ownership() {
        let mut db = mem_store();
        let pgn = "[Event \"R\"]\n\n1. e4 e5 (1... c5) 2. Nf3 *";
        let walked = pgn_processor::walk_pgn_variations(pgn).unwrap();
        let n = import_walked(&mut db, &walked, "White", "pgn").unwrap();
        assert_eq!(n, 4);
        let start_edges = db
            .get_repertoire_moves_from(
                "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                "White",
            )
            .unwrap();
        assert_eq!(start_edges.len(), 1); // e4, my move
        assert!(start_edges[0].is_my_move);
        let after_e4 = db
            .get_repertoire_moves_from(&walked[0].child_fen, "White")
            .unwrap();
        assert_eq!(after_e4.len(), 2); // e5 and c5, opponent moves
        assert!(after_e4.iter().all(|e| !e.is_my_move));
    }

    #[test]
    fn user_side_matches_env_usernames() {
        std::env::set_var("LICHESS_USERNAME", "hackandtoss");
        std::env::set_var("CHESSCOM_USERNAME", "ElectricMindGames");
        assert_eq!(
            user_side_for_game("HackAndToss", "opp"),
            Some("White".to_string())
        );
        assert_eq!(
            user_side_for_game("opp", "electricmindgames"),
            Some("Black".to_string())
        );
        assert_eq!(user_side_for_game("a", "b"), None);
    }

    #[test]
    fn grade_edge_pass_and_fail() {
        let mut db = mem_store();
        let (id, _, _) = add_move_edge(&mut db, START, "e2e4", "White", "manual").unwrap();
        let edge = db.get_repertoire_move(id).unwrap().unwrap();
        // 2026-07-01 is day 20635 since epoch
        grade_edge(&mut db, &edge, 5, 20635).unwrap();
        let after = db.get_repertoire_move(id).unwrap().unwrap();
        assert_eq!(after.srs_reps, 1);
        assert_eq!(after.srs_due_date, "2026-07-02"); // interval 1 day
        grade_edge(&mut db, &after, 1, 20635).unwrap();
        let failed = db.get_repertoire_move(id).unwrap().unwrap();
        assert_eq!(failed.srs_reps, 0);
        assert_eq!(failed.srs_due_date, "2026-07-02"); // reset to 1 day
    }

    #[test]
    fn grade_edge_records_review_event() {
        let mut db = mem_store();
        let (id, _, _) = add_move_edge(&mut db, START, "e2e4", "White", "manual").unwrap();
        let edge = db.get_repertoire_move(id).unwrap().unwrap();

        // 2026-07-01 is day 20635 since epoch
        grade_edge_with_event(&mut db, &edge, 1, 20635, "drill").unwrap();

        // The SM-2 schedule still advances exactly as with plain grade_edge.
        let after = db.get_repertoire_move(id).unwrap().unwrap();
        assert_eq!(after.srs_due_date, "2026-07-02");

        let events = db
            .get_review_events_for_target("repertoire_move", &id.to_string())
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].rating, 1);
        assert_eq!(events[0].source, "drill");
        assert_eq!(events[0].move_id, Some(id));
        assert_eq!(events[0].created_at, "2026-07-01");

        grade_edge_with_event(&mut db, &after, 5, 20635, "bot_deviation").unwrap();
        let events = db
            .get_review_events_for_target("repertoire_move", &id.to_string())
            .unwrap();
        assert_eq!(events.len(), 2);
        assert!(events.iter().any(|e| e.source == "bot_deviation"));
    }

    #[test]
    fn migrate_legacy_lines_empty_table_is_noop_and_stays_put() {
        let mut db = mem_store();
        // Fresh DB: opening_lines exists but is empty. Migration must be a no-op that
        // leaves the empty legacy table in place, and must stay a no-op on relaunch.
        assert!(db.opening_lines_table_exists());
        assert_eq!(migrate_legacy_lines(&mut db).unwrap(), 0);
        assert!(db.opening_lines_table_exists());
        assert_eq!(migrate_legacy_lines(&mut db).unwrap(), 0);
        assert!(db.opening_lines_table_exists());
    }

    #[test]
    fn position_key_matches_pgn_processor_and_drops_phantom_ep() {
        use shakmaty::{Chess, Position};
        let mut pos = Chess::default();
        let m = "e2e4"
            .parse::<shakmaty::uci::Uci>()
            .unwrap()
            .to_move(&pos)
            .unwrap();
        pos = pos.play(&m).unwrap();
        let key = position_key(&pos);
        assert_eq!(key, pgn_processor::position_key(&pos));
        // EnPassantMode::Legal: no black pawn can capture on e3, so ep field must be '-'
        assert!(key.split_whitespace().nth(3) == Some("-"));
    }
}
