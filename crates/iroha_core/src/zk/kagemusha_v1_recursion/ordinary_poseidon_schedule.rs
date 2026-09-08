//! Exact ordinary PLONK/BGH19 squeeze schedules for native Claim transcripts.
//!
//! TODO: Complete actual-reader equivalence, fixed-queue capacity and full Claim artifact
//! qualification before enabling the monetary profile. Release resource limits remain unchanged.

use super::*;

/// Count absorbed native fields at every ordinary-proof challenge, including the final binding.
///
/// This follows the authenticated protocol shape. Witness values and proof bytes cannot choose
/// the schedule. Committed instance columns contribute two fields each, including the Claim's
/// two supplied hybrid carriers; uncommitted instance columns contribute their scalar counts.
pub(super) fn ordinary_poseidon_squeeze_inputs_v1<C, L>(
    protocol: &PlonkProtocol<C, L>,
    expected_k: usize,
) -> Result<Vec<usize>, String>
where
    C: CurveAffine,
    L: Loader<C>,
{
    let profile = ordinary_ipa_proof_profile_at_k_v1(protocol, expected_k)?;
    proof_bytes::validate_ordinary_challenge_profile_v1(&protocol.num_challenge)
        .map_err(|error| format!("ordinary Poseidon challenge profile: {error:?}"))?;
    let overflow = || "ordinary Poseidon squeeze inventory overflow".to_owned();
    let common_instances = if protocol.instance_committing_key.is_some() {
        protocol
            .num_instance
            .len()
            .checked_mul(2)
            .ok_or_else(overflow)?
    } else {
        protocol
            .num_instance
            .iter()
            .try_fold(0_usize, |sum, count| {
                sum.checked_add(*count).ok_or_else(overflow)
            })?
    };
    let mut pending = common_instances
        .checked_add(usize::from(protocol.transcript_initial_state.is_some()))
        .ok_or_else(overflow)?;
    let mut schedule = Vec::new();
    for (&witnesses, &challenges) in protocol.num_witness.iter().zip(&protocol.num_challenge) {
        pending = witnesses
            .checked_mul(2)
            .and_then(|fields| pending.checked_add(fields))
            .ok_or_else(overflow)?;
        for _ in 0..challenges {
            schedule.push(std::mem::take(&mut pending));
        }
    }
    // PLONK quotient commitments followed by z. Zero-challenge phases retain their inputs.
    pending = profile
        .quotient_commitments
        .checked_mul(2)
        .and_then(|fields| pending.checked_add(fields))
        .ok_or_else(overflow)?;
    schedule.push(pending);
    // The actual BGH19 reader squeezes x1/x2, reads F, squeezes x3, then reads one scalar per
    // signed-rotation set and squeezes x4. Its ordinary IPA is always zero knowledge here.
    schedule.extend([profile.evaluations, 0, 2, profile.bgh19_rotation_sets, 2, 0]);
    // Each IPA round reads L and R before squeezing. The final proof items are c, blind, G.
    schedule.extend(std::iter::repeat_n(4, expected_k));
    schedule.push(4);
    Ok(schedule)
}

/// Whole ordinary transcripts and folds selected from authenticated protocol inventory.
#[derive(Clone, Debug)]
pub(in crate::zk::kagemusha_v1_recursion) struct ClaimProofTranscriptPlanV1 {
    parent: Option<Vec<usize>>,
    shard: Option<Vec<usize>>,
    folds: ClaimFoldTranscriptPlanV1,
    #[cfg(test)]
    ordinary_permutations: usize,
}

impl ClaimProofTranscriptPlanV1 {
    /// Fill the available native capacity with complete transcripts and folds.
    /// Ties prefer the predecessor, then the shard; values never influence the selection.
    pub(in crate::zk::kagemusha_v1_recursion) fn new<C: CurveAffine>(
        parent: &PlonkProtocol<C>,
        shard: &PlonkProtocol<C>,
    ) -> Result<Self, String> {
        use super::super::mint_hash_shard::KAGEMUSHA_MINT_HASH_SHARD_K_V1;
        let parent_preprocessed = parent.preprocessed.len();
        let shard_preprocessed = shard.preprocessed.len();
        let parent_schedule =
            ordinary_poseidon_squeeze_inputs_v1(parent, KAGEMUSHA_RECURSION_IPA_K_V1 as usize)?;
        let shard_schedule =
            ordinary_poseidon_squeeze_inputs_v1(shard, KAGEMUSHA_MINT_HASH_SHARD_K_V1 as usize)?;
        let capacity = ClaimFoldTranscriptPlanV1::capacity_after_mandatory(
            parent_preprocessed,
            shard_preprocessed,
        )?;
        let parent_permutations = schedule_permutations(&parent_schedule)?;
        let shard_permutations = schedule_permutations(&shard_schedule)?;
        let mask = select_ordinary_native_mask(capacity, parent_permutations, shard_permutations);
        let parent = (mask & 2 != 0).then_some(parent_schedule);
        let shard = (mask & 1 != 0).then_some(shard_schedule);
        let reserved = if parent.is_some() {
            parent_permutations
        } else {
            0
        } + if shard.is_some() {
            shard_permutations
        } else {
            0
        };
        let folds = ClaimFoldTranscriptPlanV1::with_reserved_ordinary(
            parent_preprocessed,
            shard_preprocessed,
            reserved,
        )?;
        Ok(Self {
            parent,
            shard,
            folds,
            #[cfg(test)]
            ordinary_permutations: reserved,
        })
    }

    /// Complete predecessor squeeze schedule, or the existing Base path.
    pub(in crate::zk::kagemusha_v1_recursion) fn parent(&self) -> Option<&[usize]> {
        self.parent.as_deref()
    }

    /// Complete shard squeeze schedule, or the existing Base path.
    pub(in crate::zk::kagemusha_v1_recursion) fn shard(&self) -> Option<&[usize]> {
        self.shard.as_deref()
    }

    /// Whole-fold reservations after both optional ordinary transcripts.
    pub(in crate::zk::kagemusha_v1_recursion) fn folds(&self) -> ClaimFoldTranscriptPlanV1 {
        self.folds
    }

    /// All optional native permutations for the actual emitted-graph regression.
    #[cfg(test)]
    pub(in crate::zk::kagemusha_v1_recursion) fn native_permutation_count(&self) -> usize {
        self.ordinary_permutations + self.folds.native_permutation_count()
    }
}

fn select_ordinary_native_mask(capacity: usize, parent: usize, shard: usize) -> u8 {
    use super::claim_fold_transcript::FOLD_PERMUTATIONS;
    let mut selected = 0;
    let mut most_permutations = 0;
    // There are only four complete ordinary-transcript subsets. For each subset, the two
    // equal-sized folds fill the remainder optimally. No transcript is split at a squeeze.
    for mask in 0_u8..4 {
        let parent_count = if mask & 2 != 0 { parent } else { 0 };
        let shard_count = if mask & 1 != 0 { shard } else { 0 };
        let Some(ordinary) = parent_count
            .checked_add(shard_count)
            .filter(|count| *count <= capacity)
        else {
            continue;
        };
        let folds = ((capacity - ordinary) / FOLD_PERMUTATIONS).min(2);
        let total = ordinary + folds * FOLD_PERMUTATIONS;
        if total > most_permutations || (total == most_permutations && mask > selected) {
            selected = mask;
            most_permutations = total;
        }
    }
    selected
}

fn schedule_permutations(schedule: &[usize]) -> Result<usize, String> {
    schedule.iter().try_fold(0_usize, |sum, &count| {
        sum.checked_add(count / 2 + 1)
            .ok_or_else(|| "ordinary Poseidon permutation inventory overflow".to_owned())
    })
}

#[cfg(test)]
#[path = "ordinary_poseidon_schedule_tests.rs"]
mod tests;
