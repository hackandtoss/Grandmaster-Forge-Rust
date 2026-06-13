/// SM-2 spaced repetition card state.
#[derive(Debug, Clone)]
pub struct SrsCard {
    pub interval: f32,
    pub ease_factor: f32,
    pub repetitions: u32,
}

impl Default for SrsCard {
    fn default() -> Self {
        Self {
            interval: 1.0,
            ease_factor: 2.5,
            repetitions: 0,
        }
    }
}

/// Grade: 0 = total blackout, 5 = perfect recall. Below 2 resets repetitions.
pub fn review(card: &SrsCard, grade: u32) -> SrsCard {
    let grade = grade.min(5) as f32;
    let new_ease = (card.ease_factor + 0.1 - (5.0 - grade) * (0.08 + (5.0 - grade) * 0.02))
        .max(1.3);

    if grade < 2.0 {
        return SrsCard {
            interval: 1.0,
            ease_factor: new_ease,
            repetitions: 0,
        };
    }

    let new_repetitions = card.repetitions + 1;
    let new_interval = match new_repetitions {
        1 => 1.0,
        2 => 6.0,
        _ => card.interval * card.ease_factor,
    };

    SrsCard {
        interval: new_interval,
        ease_factor: new_ease,
        repetitions: new_repetitions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_review_correct_gives_interval_1() {
        let card = SrsCard::default();
        let next = review(&card, 4);
        assert_eq!(next.repetitions, 1);
        assert!((next.interval - 1.0).abs() < 0.01);
    }

    #[test]
    fn second_review_correct_gives_interval_6() {
        let card = SrsCard { interval: 1.0, ease_factor: 2.5, repetitions: 1 };
        let next = review(&card, 4);
        assert_eq!(next.repetitions, 2);
        assert!((next.interval - 6.0).abs() < 0.01);
    }

    #[test]
    fn failed_review_resets_repetitions() {
        let card = SrsCard { interval: 10.0, ease_factor: 2.5, repetitions: 5 };
        let next = review(&card, 1);
        assert_eq!(next.repetitions, 0);
        assert!((next.interval - 1.0).abs() < 0.01);
    }

    #[test]
    fn ease_factor_decays_on_hard_grades() {
        let card = SrsCard::default();
        let hard = review(&card, 2);
        let easy = review(&card, 5);
        assert!(hard.ease_factor < easy.ease_factor);
    }
}
