use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct WeaknessReport {
    pub worst_opening_eco: Option<String>,
    pub blunder_move_numbers: Vec<u32>,
    pub endgame_needs_work: bool,
    pub top_training_priority: String,
}

/// One analyzed position: (ply, centipawn_loss, mistake_class, eco).
pub type PositionSummary = (u32, Option<i32>, Option<String>, Option<String>);

/// Build a weakness report from aggregated position data.
pub fn build_weakness_report(
    positions: &[PositionSummary],
    endgame_accuracy: f32,
) -> WeaknessReport {
    let mut blunder_plies: Vec<u32> = positions
        .iter()
        .filter(|(_, _, class, _)| matches!(class.as_deref(), Some("Blunder") | Some("Mistake")))
        .map(|(ply, _, _, _)| *ply)
        .collect();
    blunder_plies.dedup();

    let mut eco_losses: HashMap<String, Vec<i32>> = HashMap::new();
    for (_, loss, _, eco) in positions {
        if let (Some(loss), Some(eco)) = (loss, eco) {
            eco_losses.entry(eco.clone()).or_default().push(*loss);
        }
    }

    let worst_opening_eco = eco_losses
        .iter()
        .map(|(eco, losses)| {
            let avg = losses.iter().sum::<i32>() as f32 / losses.len() as f32;
            (eco.clone(), avg)
        })
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(eco, _)| eco);

    let endgame_needs_work = endgame_accuracy < 70.0;

    let top_training_priority = if !blunder_plies.is_empty() {
        format!(
            "Recurring mistakes around move {}",
            blunder_plies[0] / 2 + 1
        )
    } else if endgame_needs_work {
        "Endgame technique needs improvement".to_string()
    } else if let Some(eco) = &worst_opening_eco {
        format!("Opening {} has high average loss", eco)
    } else {
        "No major weaknesses detected".to_string()
    };

    WeaknessReport {
        worst_opening_eco,
        blunder_move_numbers: blunder_plies,
        endgame_needs_work,
        top_training_priority,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_blunder_cluster() {
        let positions = vec![
            (
                10,
                Some(300),
                Some("Blunder".to_string()),
                Some("B01".to_string()),
            ),
            (
                10,
                Some(150),
                Some("Mistake".to_string()),
                Some("B01".to_string()),
            ),
            (20, Some(10), None, Some("B01".to_string())),
        ];
        let report = build_weakness_report(&positions, 85.0);
        assert!(report.blunder_move_numbers.contains(&10));
        assert_eq!(report.worst_opening_eco, Some("B01".to_string()));
    }

    #[test]
    fn flags_endgame_weakness() {
        let report = build_weakness_report(&[], 55.0);
        assert!(report.endgame_needs_work);
    }
}
