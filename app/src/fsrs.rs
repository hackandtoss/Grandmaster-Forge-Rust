//! FSRS scheduling adapter: date-string persistence <-> rs-fsrs cards.
//! Long-term scheduler only (`enable_short_term: false`): every interval is
//! at least one day, matching the app's calendar-date scheduling. Fuzz is
//! disabled so schedules (and tests) are deterministic.

use chrono::{DateTime, NaiveDate, Utc};
use rs_fsrs::{Card, Parameters, State, FSRS};

pub use rs_fsrs::Rating;

/// Correct but slower than this many seconds grades Hard instead of Good.
pub const HESITATION_SECS: u64 = 15;

/// Automatic FSRS rating from observed drill behavior — no self-report.
pub fn drill_rating(correct: bool, elapsed_secs: u64) -> Rating {
    if !correct {
        Rating::Again
    } else if elapsed_secs > HESITATION_SECS {
        Rating::Hard
    } else {
        Rating::Good
    }
}

/// The persisted outcome of one FSRS review.
pub struct EdgeSchedule {
    pub stability: f64,
    pub difficulty: f64,
    pub last_review: String,
    pub reps: u32,
    pub due_date: String,
}

fn scheduler() -> FSRS {
    FSRS::new(Parameters {
        enable_short_term: false,
        enable_fuzz: false,
        ..Parameters::default()
    })
}

fn days_to_datetime(days: u64) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(days as i64 * 86_400, 0).expect("valid epoch day")
}

fn date_str_to_datetime(date: &str, fallback_days: u64) -> DateTime<Utc> {
    NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map(|d| d.and_hms_opt(0, 0, 0).expect("midnight").and_utc())
        .unwrap_or_else(|_| days_to_datetime(fallback_days))
}

fn datetime_to_date_str(dt: DateTime<Utc>) -> String {
    let (y, m, d) = crate::tree::days_to_ymd((dt.timestamp() / 86_400) as u64);
    format!("{:04}-{:02}-{:02}", y, m, d)
}

/// Apply one FSRS review to an edge's persisted state. `srs_reps` keeps the
/// app-level semantics every status reader depends on: Again resets to 0,
/// any success increments.
pub fn next_schedule(
    edge: &db_manager::RepertoireMoveRecord,
    rating: Rating,
    today_days: u64,
) -> EdgeSchedule {
    let now = days_to_datetime(today_days);
    let card = match (edge.fsrs_stability, edge.fsrs_difficulty) {
        (Some(stability), Some(difficulty)) => {
            let last_review = date_str_to_datetime(
                edge.fsrs_last_review.as_deref().unwrap_or(""),
                today_days.saturating_sub(1),
            );
            let due = date_str_to_datetime(&edge.srs_due_date, today_days);
            Card {
                due,
                stability,
                difficulty,
                elapsed_days: (now - last_review).num_days().max(0),
                scheduled_days: (due - last_review).num_days().max(0),
                reps: edge.srs_reps as i32,
                lapses: 0,
                state: State::Review,
                last_review,
            }
        }
        _ => Card::new(),
    };
    let next = scheduler().repeat(card, now)[&rating].card.clone();
    EdgeSchedule {
        stability: next.stability,
        difficulty: next.difficulty,
        last_review: datetime_to_date_str(now),
        reps: if rating == Rating::Again {
            0
        } else {
            edge.srs_reps + 1
        },
        due_date: datetime_to_date_str(next.due),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_edge() -> db_manager::RepertoireMoveRecord {
        db_manager::RepertoireMoveRecord {
            id: 1,
            parent_id: 1,
            child_id: 2,
            uci: "e2e4".to_string(),
            san: "e4".to_string(),
            is_my_move: true,
            source: "manual".to_string(),
            comment: None,
            srs_interval: 1.0,
            srs_ease: 2.5,
            srs_reps: 0,
            srs_due_date: String::new(),
            fsrs_stability: None,
            fsrs_difficulty: None,
            fsrs_last_review: None,
            parent_fen: "start w - - 0 1".to_string(),
        }
    }

    fn edge_with(
        stability: Option<f64>,
        difficulty: Option<f64>,
        last_review: Option<&str>,
        due: &str,
        reps: u32,
    ) -> db_manager::RepertoireMoveRecord {
        db_manager::RepertoireMoveRecord {
            fsrs_stability: stability,
            fsrs_difficulty: difficulty,
            fsrs_last_review: last_review.map(str::to_string),
            srs_due_date: due.to_string(),
            srs_reps: reps,
            ..fresh_edge()
        }
    }

    // 2026-07-01 is epoch day 20635 (same anchor as tree.rs tests).
    const TODAY: u64 = 20635;

    #[test]
    fn drill_rating_maps_correctness_and_hesitation() {
        assert_eq!(drill_rating(false, 0), Rating::Again);
        assert_eq!(drill_rating(false, 99), Rating::Again);
        // Boundary: exactly HESITATION_SECS is still Good.
        assert_eq!(drill_rating(true, HESITATION_SECS), Rating::Good);
        assert_eq!(drill_rating(true, HESITATION_SECS + 1), Rating::Hard);
        assert_eq!(drill_rating(true, 3), Rating::Good);
    }

    #[test]
    fn again_returns_soon_and_resets_reps() {
        let edge = edge_with(Some(20.0), Some(5.0), Some("2026-06-21"), "2026-07-11", 4);
        let again = next_schedule(&edge, Rating::Again, TODAY);
        let good = next_schedule(&edge, Rating::Good, TODAY);
        assert_eq!(again.reps, 0);
        assert_eq!(again.last_review, "2026-07-01");
        // "Due soon": strictly sooner than a success, within a week, in the future.
        assert!(again.due_date > "2026-07-01".to_string());
        assert!(
            again.due_date.as_str() <= "2026-07-08",
            "got {}",
            again.due_date
        );
        assert!(again.due_date < good.due_date);
        assert!(again.stability < edge.fsrs_stability.unwrap());
    }

    #[test]
    fn repeated_good_moves_due_dates_out_monotonically() {
        let new_edge = edge_with(None, None, None, "", 0);
        let first = next_schedule(&new_edge, Rating::Good, TODAY);
        assert_eq!(first.reps, 1);
        assert!(first.due_date.as_str() > "2026-07-01");

        let edge2 = edge_with(
            Some(first.stability),
            Some(first.difficulty),
            Some(first.last_review.as_str()),
            &first.due_date,
            first.reps,
        );
        let second = next_schedule(&edge2, Rating::Good, TODAY + 10);
        assert_eq!(second.reps, 2);
        assert!(second.due_date > first.due_date);
        assert!(second.stability > first.stability);
    }

    #[test]
    fn hard_grows_less_than_good_from_identical_state() {
        let edge = edge_with(Some(10.0), Some(5.0), Some("2026-06-21"), "2026-07-01", 2);
        let hard = next_schedule(&edge, Rating::Hard, TODAY);
        let good = next_schedule(&edge, Rating::Good, TODAY);
        assert!(hard.due_date < good.due_date);
        assert!(hard.difficulty > good.difficulty);
        // Hard is a success: reps advance.
        assert_eq!(hard.reps, 3);
    }
}
