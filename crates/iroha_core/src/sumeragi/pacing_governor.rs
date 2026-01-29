//! Deterministic pacing governor utilities.

use iroha_config::parameters::actual::SumeragiPacingGovernor;
use iroha_data_model::block::BlockHeader;

/// Sample extracted from committed block headers for pacing evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PacingSample {
    pub(crate) height: u64,
    pub(crate) creation_time_ms: u64,
    pub(crate) view_change_index: u64,
}

impl From<&BlockHeader> for PacingSample {
    fn from(header: &BlockHeader) -> Self {
        Self {
            height: header.height().get(),
            creation_time_ms: header.creation_time_ms,
            view_change_index: header.view_change_index(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PacingGovernorAction {
    Increase,
    Decrease,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PacingGovernorDecision {
    pub(crate) action: PacingGovernorAction,
    pub(crate) new_factor_bps: u32,
    pub(crate) view_change_ratio_permille: u32,
    pub(crate) commit_spacing_ratio_permille: u32,
    pub(crate) avg_spacing_ms: u64,
    pub(crate) target_block_time_ms: u64,
    pub(crate) sample_count: usize,
}

pub(crate) fn evaluate_pacing_governor(
    cfg: SumeragiPacingGovernor,
    samples: &[PacingSample],
    current_factor_bps: u32,
    target_block_time_ms: u64,
) -> Option<PacingGovernorDecision> {
    if !cfg.enabled {
        return None;
    }
    if samples.len() < 2 || target_block_time_ms == 0 {
        return None;
    }

    let mut spacing_sum: u128 = 0;
    let mut view_change_delta: u128 = 0;
    for window in samples.windows(2) {
        let prev = window[0];
        let next = window[1];
        spacing_sum = spacing_sum.saturating_add(u128::from(
            next.creation_time_ms.saturating_sub(prev.creation_time_ms),
        ));
        view_change_delta = view_change_delta.saturating_add(u128::from(
            next.view_change_index
                .saturating_sub(prev.view_change_index),
        ));
    }

    let transitions = (samples.len() - 1) as u128;
    let avg_spacing_ms = u64::try_from(spacing_sum / transitions).unwrap_or(u64::MAX);
    let commit_spacing_ratio_permille =
        ratio_permille(u128::from(avg_spacing_ms), u128::from(target_block_time_ms));
    let view_change_ratio_permille = ratio_permille(view_change_delta, transitions);

    let min_factor = cfg.min_factor_bps.max(10_000);
    let max_factor = cfg.max_factor_bps.max(min_factor);
    let current = current_factor_bps.max(min_factor).min(max_factor);

    let increase = view_change_ratio_permille >= cfg.view_change_pressure_permille
        || commit_spacing_ratio_permille >= cfg.commit_spacing_pressure_permille;
    let decrease = view_change_ratio_permille <= cfg.view_change_clear_permille
        && commit_spacing_ratio_permille <= cfg.commit_spacing_clear_permille;

    let (action, next_factor) = if increase && current < max_factor {
        (
            PacingGovernorAction::Increase,
            current.saturating_add(cfg.step_up_bps).min(max_factor),
        )
    } else if decrease && current > min_factor {
        (
            PacingGovernorAction::Decrease,
            current.saturating_sub(cfg.step_down_bps).max(min_factor),
        )
    } else {
        return None;
    };

    if next_factor == current_factor_bps {
        return None;
    }

    Some(PacingGovernorDecision {
        action,
        new_factor_bps: next_factor,
        view_change_ratio_permille,
        commit_spacing_ratio_permille,
        avg_spacing_ms,
        target_block_time_ms,
        sample_count: samples.len(),
    })
}

fn ratio_permille(numerator: u128, denominator: u128) -> u32 {
    if denominator == 0 {
        return u32::MAX;
    }
    let scaled = numerator.saturating_mul(1_000) / denominator;
    u32::try_from(scaled).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(height: u64, creation_time_ms: u64, view_change_index: u64) -> PacingSample {
        PacingSample {
            height,
            creation_time_ms,
            view_change_index,
        }
    }

    fn cfg() -> SumeragiPacingGovernor {
        SumeragiPacingGovernor {
            enabled: true,
            window_blocks: 3,
            view_change_pressure_permille: 100,
            view_change_clear_permille: 10,
            commit_spacing_pressure_permille: 1_200,
            commit_spacing_clear_permille: 1_100,
            step_up_bps: 1_000,
            step_down_bps: 100,
            min_factor_bps: 10_000,
            max_factor_bps: 20_000,
        }
    }

    #[test]
    fn pacing_governor_scales_up_on_view_changes() {
        let samples = vec![
            sample(1, 1_000, 0),
            sample(2, 2_000, 1),
            sample(3, 3_000, 2),
        ];
        let decision = evaluate_pacing_governor(cfg(), &samples, 10_000, 1_000).expect("decision");
        assert_eq!(decision.action, PacingGovernorAction::Increase);
        assert_eq!(decision.new_factor_bps, 11_000);
    }

    #[test]
    fn pacing_governor_scales_down_when_stable() {
        let samples = vec![
            sample(1, 1_000, 0),
            sample(2, 2_200, 0),
            sample(3, 3_400, 0),
        ];
        let decision = evaluate_pacing_governor(cfg(), &samples, 12_000, 1_200).expect("decision");
        assert_eq!(decision.action, PacingGovernorAction::Decrease);
        assert_eq!(decision.new_factor_bps, 11_900);
    }

    #[test]
    fn pacing_governor_respects_bounds() {
        let samples = vec![
            sample(1, 1_000, 0),
            sample(2, 3_000, 5),
            sample(3, 5_000, 10),
        ];
        let decision = evaluate_pacing_governor(cfg(), &samples, 20_000, 1_000);
        assert!(decision.is_none());
    }
}
