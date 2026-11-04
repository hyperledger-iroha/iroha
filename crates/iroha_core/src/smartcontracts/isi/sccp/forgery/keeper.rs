//! Bounded reconstruction of completed anchor witnesses from original Kura frames.

use std::num::NonZeroUsize;

use iroha_data_model::sccp::native::SccpAnchorWitnessV1;
use iroha_sccp::v1::evidence::{
    ANCHOR_PATH_LEN, SCCP_ANCHOR_CHUNK, anchor_chunk_first_height, anchor_leaf, anchor_node,
};

use super::*;
/// Original witness reconstruction refusal, without collapsing local source custody.
#[derive(Debug, thiserror::Error)]
pub enum AnchorWitnessReadError {
    /// The committed anchor relation, cancellation or original deadline refused.
    #[error(transparent)]
    Verification(#[from] ForgeryVerificationError),
    /// The exact canonical source/decoder refusal and its retained native owner.
    #[error(transparent)]
    Source(#[from] crate::state::CommittedOutcomeReadError),
}

/// Reconstruct one path against an original retained completed chunk root.
///
/// Each leaf comes from the exact original successor's parent hash and result;
/// this also covers genesis without inventing a genesis `CommitQC`. Acquisition and bounded
/// canonical decoding use the caller's pool. Only original header windows are read, so
/// transaction/output bytes do not exhaust the unchanged cumulative decoder owner.
/// A fixed logarithmic stack replaces a copied
/// 4096-leaf graph. No DATA becomes an anchor unless the completed root matches the
/// original committed state. The caller must still verify its complete forgery certificate.
///
/// # Errors
/// Refuses missing or expired original roots, unavailable Kura bytes, cancellation,
/// deadlines, allocation refusal and any reconstructed root mismatch.
pub fn anchor_witness(
    view: &StateView<'_>,
    height: u64,
    budget: &AllocationBudget,
    control: SccpNativeReadControl<'_>,
) -> core::result::Result<SccpAnchorWitnessV1, AnchorWitnessReadError> {
    check_control(control)?;
    let (parent, instance) = original_source(view)?;
    let (chunk, position) =
        anchor_chunk_position(height).ok_or(ForgeryVerificationError::NotEvidence)?;
    let (open, _) = anchor_chunk_position(parent.height())
        .ok_or(ForgeryVerificationError::Unavailable("zero native tip"))?;
    if chunk >= open {
        return Err(ForgeryVerificationError::NotEvidence.into());
    }
    let expected = store::anchor_chunks::get(view.world(), &chunk)
        .ok_or(ForgeryVerificationError::EvidenceExpired)?;
    let first = anchor_chunk_first_height(chunk).ok_or(ForgeryVerificationError::NotEvidence)?;
    let history = view.canonical_history();
    let witness =
        stream_witness::<AnchorWitnessReadError>(first, position, expected, |anchor_height| {
            check_control(control)?;
            let successor = anchor_height
                .checked_add(1)
                .and_then(|height| usize::try_from(height).ok())
                .and_then(NonZeroUsize::new)
                .ok_or(ForgeryVerificationError::NotEvidence)?;
            let original = history.original_consensus_header(successor, budget)?;
            let header = original.header();
            if header.height != successor.get() as u64 || header.instance != instance {
                return Err(ForgeryVerificationError::Unavailable(
                    "original successor identity differs",
                )
                .into());
            }
            check_control(control)?;
            Ok(SccpAnchorV1 {
                core_hash: header.parent_hash.0,
                result: header.parent_result.0,
            })
        })?;
    check_control(control)?;
    Ok(witness)
}

fn stream_witness<E: From<ForgeryVerificationError>>(
    first: u64,
    selected: usize,
    expected: &[u8; 32],
    mut read: impl FnMut(u64) -> core::result::Result<SccpAnchorV1, E>,
) -> core::result::Result<SccpAnchorWitnessV1, E> {
    if selected >= usize::try_from(SCCP_ANCHOR_CHUNK).expect("4096-entry anchor chunk fits usize") {
        return Err(ForgeryVerificationError::NotEvidence.into());
    }
    let mut stack = [[0; 32]; ANCHOR_PATH_LEN + 1];
    let mut path = [[0; 32]; ANCHOR_PATH_LEN];
    let mut selected_anchor = SccpAnchorV1 {
        core_hash: [0; 32],
        result: [0; 32],
    };
    for index in 0..usize::try_from(SCCP_ANCHOR_CHUNK).expect("4096-entry anchor chunk fits usize")
    {
        let height = first
            .checked_add(index as u64)
            .ok_or(ForgeryVerificationError::NotEvidence)?;
        let anchor = read(height)?;
        if index == selected {
            selected_anchor = anchor;
        }
        let mut hash = anchor_leaf(height, &anchor.core_hash, &anchor.result);
        let mut level = 0;
        let mut node = index;
        loop {
            if level < ANCHOR_PATH_LEN && node == ((selected >> level) ^ 1) {
                path[level] = hash;
            }
            if node & 1 == 0 {
                stack[level] = hash;
                break;
            }
            hash = anchor_node(&stack[level], &hash);
            node >>= 1;
            level += 1;
        }
    }
    if &stack[ANCHOR_PATH_LEN] != expected {
        return Err(ForgeryVerificationError::BadWitness.into());
    }
    Ok(SccpAnchorWitnessV1 {
        core_hash: selected_anchor.core_hash,
        result: selected_anchor.result,
        path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_sccp::v1::evidence::{anchor_chunk_root, verify_anchor_path};

    fn anchor(height: u64) -> SccpAnchorV1 {
        SccpAnchorV1 {
            core_hash: Hash::new(height.to_be_bytes()).into(),
            result: [0x81; 32],
        }
    }

    #[test]
    fn streaming_witness_matches_original_root_at_both_edges_and_internal_positions() {
        let first = 4097;
        let leaves = (first..first + SCCP_ANCHOR_CHUNK)
            .map(|height| {
                let a = anchor(height);
                anchor_leaf(height, &a.core_hash, &a.result)
            })
            .collect::<Vec<_>>();
        let root = anchor_chunk_root(&leaves).unwrap();
        for position in [0, 1, 63, 1024, 2047, 4095] {
            let witness =
                stream_witness::<ForgeryVerificationError>(first, position, &root, |height| {
                    Ok(anchor(height))
                })
                .unwrap();
            assert!(verify_anchor_path(
                &leaves[position],
                position,
                &witness.path,
                &root
            ));
            assert_eq!(witness.core_hash, anchor(first + position as u64).core_hash);
        }
        for changed in [first, first + 2047, first + SCCP_ANCHOR_CHUNK - 1] {
            assert!(matches!(
                stream_witness(first, 0, &root, |height| {
                    let mut original = anchor(height);
                    if height == changed {
                        original.core_hash[0] ^= 1;
                    }
                    Ok(original)
                }),
                Err(ForgeryVerificationError::BadWitness)
            ));
        }
        assert!(matches!(
            stream_witness(first, 0, &[0; 32], |height| Ok(anchor(height))),
            Err(ForgeryVerificationError::BadWitness)
        ));
        let mut reads = 0;
        assert!(matches!(
            stream_witness(first, 0, &root, |_| {
                reads += 1;
                Err(ForgeryVerificationError::Deadline)
            }),
            Err(ForgeryVerificationError::Deadline)
        ));
        assert_eq!(reads, 1);
    }

    #[test]
    fn witness_acquisition_preserves_missing_original_source_and_cancellation() {
        use std::sync::atomic::AtomicBool;
        let state = crate::smartcontracts::isi::sccp::test_support::blank_state();
        let view = state.view();
        let stop = AtomicBool::new(false);
        let control = SccpNativeReadControl {
            deadline: Instant::now() + std::time::Duration::from_secs(5),
            stop: &stop,
        };
        assert!(matches!(
            anchor_witness(&view, 1, &AllocationBudget::new(0), control),
            Err(AnchorWitnessReadError::Verification(
                ForgeryVerificationError::Unavailable(_)
            ))
        ));
        stop.store(true, Ordering::Relaxed);
        assert!(matches!(
            anchor_witness(&view, 1, &AllocationBudget::new(0), control),
            Err(AnchorWitnessReadError::Verification(
                ForgeryVerificationError::Cancelled
            ))
        ));
    }
}
