//! Bounded proof proposals from original, independently verified native decisions.
//!
//! The native capability remains borrowed while these bytes are copied. No decoded
//! certificate, arbitrary committee, checkpoint or success flag can enter this adapter.
//! The resulting witnesses still require the installed compact source graph to prove them.

use iroha_crypto::{Algorithm, MerkleProof};
use iroha_data_model::{
    events::EventBox,
    isi::kagemusha_wallet::load_finality::{
        KagemushaWalletLoadFinalityErrorV1, KagemushaWalletLoadReceiptV1,
        verify_finalized_kagemusha_wallet_load_event_v1,
    },
    kagemusha::KagemushaWalletValidationErrorV1,
    sumeragi_finality::{FinalityError, MAX_RESULT_PREIMAGE_BYTES, VerifiedSumeragiBlock},
};
use iroha_kagemusha_proof::finality::{
    history::HistoryAnchor,
    native::{BlockWitnessInput, LoadWitnessInput},
    result::MAX_RESULT_BYTES,
};
use iroha_sumeragi::message::{Qc, VoteKind};

// The compact source admits at most 31 seats, so its canonical QC is under 1 KiB.
const MAX_QC_BYTES: usize = 1024;

// The adapter must never admit a native result outside the compiled proof bound.
const _: () = assert!(MAX_RESULT_BYTES as usize == MAX_RESULT_PREIMAGE_BYTES);

/// Refusal to convert retained native evidence into bounded proof witnesses.
#[derive(Debug, thiserror::Error)]
pub enum FinalityInputError {
    /// Original data exceeds a compiled source limit or is absent.
    #[error("native finality input exceeds the supported {0} bound")]
    Bound(&'static str),
    /// A native decision differs from the independently selected policy.
    #[error("native finality input differs from the selected history policy")]
    Binding,
    /// The original native verifier refuses global scope or context.
    #[error(transparent)]
    Finality(#[from] FinalityError),
    /// The original native event proof does not authenticate the selected receipt.
    #[error(transparent)]
    Event(#[from] KagemushaWalletLoadFinalityErrorV1),
    /// Original canonical QC decoding failed under finite limits.
    #[error(transparent)]
    Codec(#[from] norito::Error),
    /// Canonical receipt shape or transcript construction failed.
    #[error(transparent)]
    Receipt(#[from] KagemushaWalletValidationErrorV1),
    /// Complete native epoch context validation or canonical identity failed.
    #[error("invalid original epoch context: {0}")]
    Context(String),
}

fn original_certificate<'a>(
    anchor: &HistoryAnchor,
    chain: &str,
    block: &'a VerifiedSumeragiBlock,
) -> Result<(&'a [u8], Qc), FinalityInputError> {
    let schedule = &block.commitment().schedule;
    if schedule.current.network_id.as_bytes() != &anchor.network {
        return Err(FinalityInputError::Binding);
    }
    block.verify_global_scope(schedule.current.network_id, chain)?;
    let certificate = block
        .block()
        .commit_certificate()
        .ok_or(FinalityInputError::Bound("certificate"))?;
    let frame = certificate.result_preimage();
    if frame.len() < 40 || frame.len() > MAX_RESULT_BYTES as usize {
        return Err(FinalityInputError::Bound("result frame"));
    }
    let encoded_qc = certificate.commit_qc();
    if encoded_qc.is_empty() || encoded_qc.len() > MAX_QC_BYTES {
        return Err(FinalityInputError::Bound("CommitQC"));
    }
    let qc: Qc = norito::decode_canonical_with_limits(
        encoded_qc,
        norito::canonical_decode_limits(encoded_qc.len()),
    )?;
    if qc.kind != VoteKind::Commit
        || qc.instance.0 != anchor.instance
        || qc.height != block.height()
        || schedule.height != block.height()
        || qc.block_hash != block.core_hash()
        || qc.result != block.result()
    {
        return Err(FinalityInputError::Binding);
    }
    Ok((frame, qc))
}

/// Copy one exact native block into the installed compact graph's witness inputs.
///
/// `anchor` and `chain` must come from the independently selected installation.
/// `block` must remain owned by the original native verifier/envelope. The roster
/// retains its authenticated seat order, and R is copied from the retained frame;
/// re-encoding a projected commitment never substitutes for those original bytes.
/// These proposals do not authenticate a source key or grant Load authority.
///
/// # Errors
/// Refuses foreign/private scope, another anchor instance, oversized originals,
/// unsupported committee geometry, malformed QC/key bytes or changed context IDs.
pub fn block_witness(
    anchor: &HistoryAnchor,
    chain: &str,
    block: &VerifiedSumeragiBlock,
) -> Result<BlockWitnessInput, FinalityInputError> {
    let (frame, qc) = original_certificate(anchor, chain, block)?;
    let schedule = &block.commitment().schedule;
    let current = &schedule.current;
    let members = current.committee.len();
    if !(4..=31).contains(&members) || !(members - 1).is_multiple_of(3) {
        return Err(FinalityInputError::Bound("committee"));
    }
    let current_context = current.context_id().map_err(FinalityInputError::Context)?;
    if qc.epoch.epoch != current.authorization.epoch || qc.epoch.context.0 != current_context {
        return Err(FinalityInputError::Binding);
    }
    let authorized = schedule
        .boundary
        .as_ref()
        .map_or(current, |boundary| &boundary.next);
    let next_members = authorized.committee.len();
    if !(4..=31).contains(&next_members) || !(next_members - 1).is_multiple_of(3) {
        return Err(FinalityInputError::Bound("authorized committee"));
    }
    let authorized_context = authorized
        .context_id()
        .map_err(FinalityInputError::Context)?;
    let roster = current
        .committee
        .iter()
        .map(|member| {
            let (algorithm, bytes) = member
                .validator
                .public_key()
                .try_to_bytes()
                .map_err(|_| FinalityInputError::Binding)?;
            if algorithm != Algorithm::BlsNormal {
                return Err(FinalityInputError::Binding);
            }
            bytes.try_into().map_err(|_| FinalityInputError::Binding)
        })
        .collect::<Result<Vec<[u8; 48]>, _>>()?;
    let message = qc
        .preimage()
        .try_into()
        .map_err(|_| FinalityInputError::Bound("Commit signing preimage"))?;
    Ok(BlockWitnessInput {
        result_frame: frame.to_vec(),
        message,
        roster,
        bitmap: qc.signers.as_bytes().to_vec(),
        signature: qc.agg_sig.0,
        current_context,
        authorized_context,
    })
}

/// Convert an exact native receipt/event path for the same retained result.
///
/// A Core caller obtains `block` from its native cursor and `proof` from the
/// committed event-evidence owner. The ordinary native verifier authenticates
/// the complete expected receipt before any path becomes a compact witness.
/// Missing siblings encode genuine counted-tree promotions or inactive levels.
/// The compact Load source independently constrains every use of that encoding.
///
/// # Errors
/// Refuses unsupported source bounds, wrong policy/receipt/result/height, invalid
/// counted Merkle geometry, or noncanonical receipt fields.
pub fn load_witness(
    anchor: &HistoryAnchor,
    chain: &str,
    block: &VerifiedSumeragiBlock,
    receipt: &KagemushaWalletLoadReceiptV1,
    proof: &MerkleProof<EventBox>,
) -> Result<LoadWitnessInput, FinalityInputError> {
    let (frame, _) = original_certificate(anchor, chain, block)?;
    let commitment = block
        .execution()
        .event_commitment
        .ok_or(FinalityInputError::Binding)?;
    if commitment.leaf_count().get() > 1_u64 << 32 || proof.audit_path().len() > 32 {
        return Err(FinalityInputError::Bound("event path"));
    }
    let native = verify_finalized_kagemusha_wallet_load_event_v1(
        block,
        proof,
        block.commitment().schedule.current.network_id,
        chain,
        receipt,
    )?;
    let mut siblings = [[0; 32]; 32];
    for (output, original) in siblings.iter_mut().zip(proof.audit_path()) {
        if let Some(original) = original {
            *output = *original.as_ref();
        }
    }
    Ok(LoadWitnessInput {
        result_frame: frame.to_vec(),
        receipt: native.receipt().transcript()?,
        event_root: *commitment.root().as_ref(),
        event_count: commitment.leaf_count().get(),
        event_index: native.event_index(),
        siblings,
    })
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
