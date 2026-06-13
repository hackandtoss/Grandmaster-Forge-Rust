/// Chess.com-style accuracy from average centipawn loss. Returns [0.0, 100.0].
pub fn accuracy_from_acpl(avg_centipawn_loss: f32) -> f32 {
    let raw = 103.1668 * (-0.04354 * avg_centipawn_loss).exp() - 3.1669;
    raw.clamp(0.0, 100.0)
}

#[derive(Debug, Clone, PartialEq)]
pub struct GameAccuracy {
    pub overall: f32,
    pub opening: f32,    // plies 1–30 (moves 1–15)
    pub middlegame: f32, // plies 31–70
    pub endgame: f32,    // plies 71+
}

/// Compute per-phase accuracy from (ply, centipawn_loss) pairs.
/// Plies with None loss (unanalyzed) are excluded from the average.
pub fn compute_game_accuracy(moves: &[(u32, Option<i32>)]) -> GameAccuracy {
    let phase_acpl = |range: std::ops::RangeInclusive<u32>| -> f32 {
        let losses: Vec<f32> = moves
            .iter()
            .filter(|(ply, loss)| range.contains(ply) && loss.is_some())
            .map(|(_, loss)| loss.unwrap() as f32)
            .collect();
        if losses.is_empty() {
            return 100.0;
        }
        accuracy_from_acpl(losses.iter().sum::<f32>() / losses.len() as f32)
    };

    let all_losses: Vec<f32> = moves
        .iter()
        .filter_map(|(_, l)| l.map(|v| v as f32))
        .collect();

    let overall = if all_losses.is_empty() {
        100.0
    } else {
        accuracy_from_acpl(all_losses.iter().sum::<f32>() / all_losses.len() as f32)
    };

    GameAccuracy {
        overall,
        opening: phase_acpl(1..=30),
        middlegame: phase_acpl(31..=70),
        endgame: phase_acpl(71..=999),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_game_is_100_percent() {
        assert!((accuracy_from_acpl(0.0) - 100.0).abs() < 1.0);
    }

    #[test]
    fn high_loss_is_low_accuracy() {
        assert!(accuracy_from_acpl(200.0) < 20.0);
    }

    #[test]
    fn phase_accuracy_excludes_unanalyzed_moves() {
        let moves = vec![(1, Some(10)), (2, Some(5)), (3, None)];
        let acc = compute_game_accuracy(&moves);
        assert!(acc.overall > 0.0);
    }

    #[test]
    fn empty_game_gives_100() {
        let acc = compute_game_accuracy(&[]);
        assert!((acc.overall - 100.0).abs() < 0.01);
    }
}
