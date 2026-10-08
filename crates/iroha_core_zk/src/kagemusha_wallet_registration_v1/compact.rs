//! Candidate compact terminal-result intake. Installed transport selection remains native
//! until genuine native/recursive differential qualification; there is no fallback decoder.
//! TODO: Run the pinned terminal Register differential gate before selecting this format
//! in installed application intake, then retire the superseded prefix transport there.
use super::*;
use crate::{
    kagemusha_wallet_artifacts_v1::producer_inventory::QualifiedReceiptSourceV1,
    kagemusha_wallet_finality_v1::{
        HISTORY_ORIGINAL_MAX_BYTES_V1, HistoryOriginalV1, derive_history_anchor,
    },
};
use iroha_data_model::{
    block::decode_framed_signed_block,
    sumeragi_finality::{ExecutionResultCommitment, result_of_preimage},
};
use iroha_kagemusha_proof::finality::{
    history::HistoryState, result::MAX_RESULT_BYTES, schedule::tape::ResultTapeWitness,
};
use iroha_pasta::{CancellationToken, msm::MemoryBudget};
use iroha_plonk::frontend::Value;

/// Whole original bound: one executed block, one committed transaction, one history proof,
/// one native result and conservative framing overhead. This is not a phone-memory claim.
pub const COMPACT_REGISTRATION_MAX_BYTES_V1: usize = MAX_FINALITY_BLOCK_BYTES
    + REGISTRATION_PROOF_MAX_BYTES_V1
    + HISTORY_ORIGINAL_MAX_BYTES_V1
    + MAX_RESULT_BYTES as usize
    + 4096;

/// Bounded canonical original DATA for the exact terminal Register result.
/// No field is a trusted checkpoint, accepted asset or caller verdict.
#[derive(Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha.kagemusha.wallet.compact_registration.v1")]
pub struct CompactRegistrationOriginalV1 {
    /// Exactly one.
    pub version: u16,
    /// Requested exact UUID/incarnation/scale digest.
    pub asset_digest: [u8; 32],
    /// Direct Register instruction position.
    pub instruction_index: u32,
    /// Canonical shared HistoryOriginalV1 frame; restoration fully verifies it.
    pub history: Vec<u8>,
    /// Exact terminal native R preimage, excluding its external hash domain.
    pub result: Vec<u8>,
    /// Exact canonical result-bearing SignedBlockWire.
    pub block: Vec<u8>,
    /// Exact canonical CommittedTransaction with both counted membership paths.
    pub committed: Vec<u8>,
}
impl CompactRegistrationOriginalV1 {
    /// Assemble deterministic transport DATA from an already proved terminal prefix and
    /// the original executed block/transaction. This does not select an installed graph
    /// or grant registration authority; the receiving owner must verify the complete frame.
    /// No Load event or receipt is used to package a Register.
    /// # Errors
    /// Missing result originals, inconsistent terminal joins, failed Register semantics,
    /// excessive frames, encoding failure or cooperative cancellation.
    pub fn from_terminal(
        prefix: &iroha_kagemusha_proof::finality::native::HistoryPrefix,
        block: &iroha_data_model::block::SignedBlock,
        committed: &CommittedTransaction,
        scheme: &KagemushaWalletSchemeV1,
        asset_digest: [u8; 32],
        instruction_index: u32,
        cancellation: Option<&CancellationToken>,
    ) -> Result<Self, RegistrationErrorV1> {
        checkpoint(cancellation)?;
        let result = block
            .commit_certificate()
            .ok_or(RegistrationErrorV1::Invalid(
                "compact missing native result",
            ))?
            .result_preimage()
            .to_vec();
        let original = Self {
            version: 1,
            asset_digest,
            instruction_index,
            history: HistoryOriginalV1::from_prefix(prefix)
                .encode_canonical()
                .map_err(|_| RegistrationErrorV1::Invalid("compact history original"))?,
            result,
            block: block
                .encode_wire()
                .map_err(|_| RegistrationErrorV1::Invalid("compact block encoding"))?,
            committed: norito::encode_canonical(committed)
                .map_err(|_| RegistrationErrorV1::Invalid("compact committed encoding"))?,
        };
        terminal_data(&original, prefix.state(), scheme, cancellation)?;
        original.validate_bounds()?;
        Ok(original)
    }

    /// Check original extents before inner allocation or verification.
    /// # Errors
    /// Invalid version/selector or any empty/overbound original.
    pub fn validate_bounds(&self) -> Result<(), RegistrationErrorV1> {
        if self.version != 1 || self.asset_digest == [0; 32] {
            return Err(RegistrationErrorV1::Invalid("compact selection"));
        }
        for (bytes, maximum) in [
            (&self.history, HISTORY_ORIGINAL_MAX_BYTES_V1),
            (&self.result, MAX_RESULT_BYTES as usize),
            (&self.block, MAX_FINALITY_BLOCK_BYTES),
            (&self.committed, REGISTRATION_PROOF_MAX_BYTES_V1),
        ] {
            if bytes.is_empty() || bytes.len() > maximum {
                return Err(RegistrationErrorV1::Invalid("compact original extent"));
            }
        }
        Ok(())
    }
    /// Encode bounded canonical DATA without authenticating any field.
    /// # Errors
    /// Invalid extents or encoding failure.
    pub fn encode_canonical(&self) -> Result<Vec<u8>, RegistrationErrorV1> {
        self.validate_bounds()?;
        let bytes = norito::encode_canonical(self)
            .map_err(|_| RegistrationErrorV1::Invalid("compact encoding"))?;
        if bytes.len() > COMPACT_REGISTRATION_MAX_BYTES_V1 {
            return Err(RegistrationErrorV1::Invalid("compact frame extent"));
        }
        Ok(bytes)
    }
    /// Decode exact bounded canonical DATA; this grants no authority.
    /// # Errors
    /// Empty/overbound/trailing/noncanonical frame or invalid inner extents.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, RegistrationErrorV1> {
        if bytes.is_empty() || bytes.len() > COMPACT_REGISTRATION_MAX_BYTES_V1 {
            return Err(RegistrationErrorV1::Invalid("compact frame extent"));
        }
        let value: Self = decode(bytes)?;
        value.validate_bounds()?;
        Ok(value)
    }
}

/// Verify the complete qualified history and exact terminal Register execution.
/// The independently selected native genesis must match every shared history anchor field.
/// A later valid prefix cannot authenticate an earlier registration. Only descriptor/VK
/// qualification is needed; no proving key or caller checkpoint enters this path.
///
/// Installed application intake does not select this candidate format until genuine
/// native-versus-compact differential qualification is complete.
/// # Errors
/// Any foreign source/root, invalid proof/claims, altered terminal bytes or Register term.
pub fn verify_compact_registration_v1(
    original: &CompactRegistrationOriginalV1,
    graph: &QualifiedReceiptSourceV1,
    genesis: &SumeragiFinalityVerifier,
    scheme: &KagemushaWalletSchemeV1,
    budget: MemoryBudget,
    cancellation: Option<&CancellationToken>,
) -> Result<FinalizedKagemushaWalletRegistrationV1, RegistrationErrorV1> {
    original.validate_bounds()?;
    checkpoint(cancellation)?;
    let anchor = derive_history_anchor(genesis)
        .map_err(|_| RegistrationErrorV1::Invalid("compact native genesis"))?;
    if &anchor != graph.anchor()
        || graph.installation().0 != scheme.scheme_id()
        || anchor.network != scheme.network_id
    {
        return Err(RegistrationErrorV1::Invalid("compact installed source"));
    }
    let history = HistoryOriginalV1::decode_canonical(&original.history)
        .map_err(|_| RegistrationErrorV1::Invalid("compact history original"))?
        .restore_qualified(graph, budget, cancellation)
        .map_err(|error| match error {
            crate::kagemusha_wallet_finality_v1::HistoryOriginalErrorV1::Proof(error)
                if error.is_cancelled() =>
            {
                RegistrationErrorV1::Cancelled
            }
            _ => RegistrationErrorV1::Invalid("compact complete history proof"),
        })?;
    let (data, block_hash, height) =
        terminal_data(original, history.state(), scheme, cancellation)?;
    checkpoint(cancellation)?;
    Ok(FinalizedKagemushaWalletRegistrationV1::from_authenticated(
        data, block_hash, height,
    ))
}
fn checkpoint(cancellation: Option<&CancellationToken>) -> Result<(), RegistrationErrorV1> {
    CancellationToken::checkpoint(cancellation).map_err(|_| RegistrationErrorV1::Cancelled)
}

// This helper only projects DATA. The sole public authority path calls it after actual
// complete history verification. Component mutations can exercise these joins independently.
fn terminal_data(
    original: &CompactRegistrationOriginalV1,
    state: &HistoryState,
    scheme: &KagemushaWalletSchemeV1,
    cancellation: Option<&CancellationToken>,
) -> Result<(KagemushaWalletRegistrationDataV1, [u8; 32], u64), RegistrationErrorV1> {
    original.validate_bounds()?;
    checkpoint(cancellation)?;
    if state.frame_len as usize != original.result.len()
        || state.result != result_of_preimage(&original.result).0
    {
        return Err(RegistrationErrorV1::Invalid("compact terminal result"));
    }
    let tape = ResultTapeWitness::from_frame(&Value::known(original.result.clone()))
        .map_err(|_| RegistrationErrorV1::Invalid("compact result tape"))?;
    let root = tape.root();
    if !root.is_known()
        || root
            .error_if_known_and(|root| *root != state.tape_root)
            .is_err()
    {
        return Err(RegistrationErrorV1::Invalid("compact result tape root"));
    }
    checkpoint(cancellation)?;
    let result = ExecutionResultCommitment::decode(&original.result)
        .map_err(|_| RegistrationErrorV1::Invalid("compact canonical result"))?;
    if result.height < 2
        || result.height.checked_add(1) != Some(state.next_height)
        || result.schedule.current.network_id.as_bytes() != &scheme.network_id
    {
        return Err(RegistrationErrorV1::Invalid(
            "compact terminal height or network",
        ));
    }
    let block = norito::core::with_decode_limits_scope(
        norito::canonical_decode_limits(original.block.len()),
        || decode_framed_signed_block(&original.block),
    )
    .map_err(|_| RegistrationErrorV1::Invalid("compact block frame"))?;
    if block
        .encode_wire()
        .map_err(|_| RegistrationErrorV1::Invalid("compact block encoding"))?
        != original.block
        || !block.has_results()
        || block.header().height().get() != result.height
        || block.validate_proposal_commitments().is_err()
        || block.validate_output_merkle_cache().is_err()
    {
        return Err(RegistrationErrorV1::Invalid("compact executed block"));
    }
    let (length, hash) = block
        .executed_block_wire_identity()
        .map_err(|_| RegistrationErrorV1::Invalid("compact executed wire identity"))?;
    if length != result.execution.executed_block_wire_len
        || hash != result.execution.executed_block_wire_hash
        || block.network_input_merkle_commitment() != result.execution.transaction_input_commitment
        || block.output_merkle_commitment() != result.execution.transaction_output_commitment
    {
        return Err(RegistrationErrorV1::Invalid("compact result block binding"));
    }
    checkpoint(cancellation)?;
    let committed: CommittedTransaction = decode(&original.committed)?;
    if !committed.verify_inclusion_in_block(&block) {
        return Err(RegistrationErrorV1::Invalid("compact Register membership"));
    }
    let data = project_kagemusha_wallet_registration_v1(
        &committed,
        result.schedule.current.network_id,
        scheme,
        original.asset_digest,
        usize::try_from(original.instruction_index)
            .map_err(|_| RegistrationErrorV1::Invalid("compact instruction index"))?,
    )?;
    Ok((data, *block.hash().as_ref(), result.height))
}

#[cfg(test)]
mod genuine;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod generator;
