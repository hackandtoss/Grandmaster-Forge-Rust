#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrainingMode {
    OpeningRepetition,
    BlunderPrevention,
    PuzzleRush,
    ReviewRecentGame,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UserSnapshot {
    pub opening_due: u32,
    pub blunder_hotspots: u32,
    pub puzzle_rating: i32,
    pub has_unreviewed_games: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NextAction {
    pub mode: TrainingMode,
    pub reason: String,
}

pub fn recommend_next_action(snapshot: &UserSnapshot) -> NextAction {
    if snapshot.has_unreviewed_games {
        return NextAction {
            mode: TrainingMode::ReviewRecentGame,
            reason: "Review your latest game to generate new improvement lines.".to_string(),
        };
    }

    if snapshot.blunder_hotspots > 0 {
        return NextAction {
            mode: TrainingMode::BlunderPrevention,
            reason: "Reinforce known tactical and strategic blunder patterns.".to_string(),
        };
    }

    if snapshot.opening_due > 0 {
        return NextAction {
            mode: TrainingMode::OpeningRepetition,
            reason: "You have opening lines due for spaced repetition.".to_string(),
        };
    }

    NextAction {
        mode: TrainingMode::PuzzleRush,
        reason: format!(
            "No urgent weaknesses found. Keep sharp with puzzles at rating {}.",
            snapshot.puzzle_rating
        ),
    }
}

pub fn launch() -> String {
    "UI shell initialized. Wire this to Iced/Slint app runtime.".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recommends_game_review_first() {
        let snapshot = UserSnapshot {
            opening_due: 10,
            blunder_hotspots: 2,
            puzzle_rating: 1850,
            has_unreviewed_games: true,
        };

        let next = recommend_next_action(&snapshot);
        assert_eq!(next.mode, TrainingMode::ReviewRecentGame);
    }

    #[test]
    fn recommends_puzzles_when_no_urgent_work() {
        let snapshot = UserSnapshot {
            opening_due: 0,
            blunder_hotspots: 0,
            puzzle_rating: 2100,
            has_unreviewed_games: false,
        };

        let next = recommend_next_action(&snapshot);
        assert_eq!(next.mode, TrainingMode::PuzzleRush);
    }
}
