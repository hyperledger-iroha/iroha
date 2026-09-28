//! Publication evidence anchored in an independently selected native finality checkpoint.

use super::pin_registry::{PinManifestRecord, ReplicationOrderRecord};
use crate::{
    NetworkId,
    isi::sorafs::AssertSorafsPublicationV1,
    query::CommittedTransaction,
    sumeragi_finality::{
        SumeragiFinalityCheckpoint, SumeragiFinalityProof, SumeragiFinalityVerifier,
    },
    transaction::{Executable, SignedTransaction, TransactionEntrypoint},
};
use norito::{Decode, Encode};

/// Maximum canonical publication response, including its authenticated executed block carrier.
pub const PUBLICATION_PROOF_MAX_BYTES_V1: usize = 32 * 1024 * 1024;
/// Maximum consecutive current-certificate proofs accepted in one publication response.
pub const PUBLICATION_PROOF_MAX_BLOCKS_V1: usize = 1024;

/// Bounded request for metadata or one chunk under an exact live native replication assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sorafs::publication::SorafsAssignedSourceRequestV1")]
pub struct SorafsAssignedSourceRequestV1 {
    /// Assigned provider whose owner authenticates the request.
    pub target_provider: [u8; 32],
    /// Admitted provider expected to serve its verified stored replica.
    pub source_provider: [u8; 32],
    /// Exact live replication order.
    pub order_id: [u8; 32],
    /// Exact assignment revision retained by the worker.
    pub assignment_revision: u64,
    /// Exact approved manifest.
    pub manifest_digest: [u8; 32],
    /// None requests canonical source metadata; Some requests one indexed chunk.
    pub chunk_index: Option<u32>,
    /// Independently retained finalized worker floor.
    pub floor_height: u64,
    /// Exact finalized worker floor hash.
    pub floor_block_hash: [u8; 32],
}

/// Informational preparation read; it becomes evidence only after the signed assertion succeeds.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sorafs::publication::SorafsPublicationPreparationV1")]
pub struct SorafsPublicationPreparationV1 {
    /// Exact currently approved pin and its paid registration record.
    pub pin: PinManifestRecord,
    /// Exact automatic replication assignment and completion records.
    pub order: ReplicationOrderRecord,
}

/// Selector for native publication execution evidence, authenticated as an account request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sorafs::publication::SorafsPublicationProofRequestV1")]
pub struct SorafsPublicationProofRequestV1 {
    /// Exact signed assertion entrypoint hash independently retained by the client.
    pub entry_hash: [u8; 32],
    /// Independently trusted checkpoint height.
    pub floor_height: u64,
    /// Independently trusted checkpoint block hash.
    pub floor_block_hash: [u8; 32],
}

/// Untrusted canonical carrier for a challenged native publication assertion.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sorafs::publication::SorafsPublicationProofV1")]
pub struct SorafsPublicationProofV1 {
    /// Consecutive canonical certified blocks, including the independently pinned floor.
    /// The last frame contains the exact successful signed assertion.
    pub lineage: Vec<SumeragiFinalityProof>,
}

/// Opaque successful verification of the client's original challenged assertion.
#[derive(Debug)]
pub struct VerifiedSorafsPublicationV1 {
    finality: SumeragiFinalityCheckpoint,
    completed: bool,
}
impl VerifiedSorafsPublicationV1 {
    /// Verified compact prefix that can be retained as the next independent checkpoint.
    pub fn finality(&self) -> &SumeragiFinalityCheckpoint {
        &self.finality
    }
    /// Whether the proven instruction required every assigned provider's finalized completion.
    pub const fn completed(&self) -> bool {
        self.completed
    }
}

/// Fixed publication proof rejection; raw state reads and HTTP success cannot create evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("native SoraFS publication proof rejected")]
pub struct SorafsPublicationProofErrorV1;

/// Verify the exact signed assertion against independently pinned network and predecessor state.
///
/// `checkpoint` must come from an independently authenticated local selection or a preceding
/// call's verified output. The caller must first compare its chain label with the configured
/// chain using `SumeragiFinalityVerifier::from_trusted_checkpoint`. A proof response cannot supply
/// its own trust root. The caller must start a monotonic deadline before generating its challenge
/// and reject verification after that deadline.
///
/// # Errors
/// Rejects a proof that fails finality, binding or freshness verification.
pub fn verify_sorafs_publication_v1(
    network: &NetworkId,
    checkpoint: &SumeragiFinalityCheckpoint,
    expected: &SignedTransaction,
    proof: &SorafsPublicationProofV1,
) -> Result<VerifiedSorafsPublicationV1, SorafsPublicationProofErrorV1> {
    let rejected = SorafsPublicationProofErrorV1;
    if proof.lineage.len() < 2
        || proof.lineage.len() > PUBLICATION_PROOF_MAX_BLOCKS_V1
        || norito::canonical_frame_len(proof).map_err(|_| rejected)?
            > PUBLICATION_PROOF_MAX_BYTES_V1
        || expected.network_id() != Some(network)
    {
        return Err(rejected);
    }
    let Executable::Instructions(instructions) = expected.instructions() else {
        return Err(rejected);
    };
    if instructions.len() != 1 {
        return Err(rejected);
    }
    let assertion = instructions[0]
        .as_any()
        .downcast_ref::<AssertSorafsPublicationV1>()
        .ok_or(rejected)?;
    if assertion.challenge == [0; 32]
        || assertion.minimum_height != checkpoint.height()
        || assertion.minimum_block_hash != *checkpoint.block_hash().as_ref()
    {
        return Err(rejected);
    }
    let mut verifier = SumeragiFinalityVerifier::from_trusted_checkpoint(
        checkpoint,
        network,
        checkpoint.chain_id(),
    )
    .map_err(|_| rejected)?;
    // Certificate witnesses may differ between honest replicas; authenticate the same decision.
    verifier
        .verify_same_decision(checkpoint.tip(), &proof.lineage[0])
        .map_err(|_| rejected)?;
    let mut verified_tip = None;
    for edge in &proof.lineage[1..] {
        verified_tip = Some(verifier.verify(edge).map_err(|_| rejected)?);
    }
    let verified_tip = verified_tip.ok_or(rejected)?;
    let block = verified_tip.block();
    let entry_hash = expected.hash_as_entrypoint();
    let input_index = block
        .network_entrypoints()
        .position(|entry| entry.hash() == entry_hash)
        .and_then(|index| u32::try_from(index).ok())
        .ok_or(rejected)?;
    let entrypoint = block
        .network_entrypoint_at(input_index as usize)
        .ok_or(rejected)?;
    if entrypoint != &TransactionEntrypoint::External(expected.clone()) {
        return Err(rejected);
    }
    let (output_index, _) = block.network_output_at(input_index).ok_or(rejected)?;
    let output = block
        .execution_outputs()
        .get(output_index as usize)
        .ok_or(rejected)?
        .clone();
    let committed = CommittedTransaction {
        block_hash: block.hash(),
        entrypoint_hash: entrypoint.hash(),
        entrypoint_proof: block.network_input_proof(input_index).ok_or(rejected)?,
        entrypoint: entrypoint.clone(),
        output_hash: iroha_crypto::HashOf::new(&output),
        output_proof: block.output_proof(output_index).ok_or(rejected)?,
        output,
    };
    verified_tip
        .verify_committed_transaction(network, &committed)
        .map_err(|_| rejected)?;
    Ok(VerifiedSorafsPublicationV1 {
        finality: verifier
            .export_checkpoint(proof.lineage.last().ok_or(rejected)?)
            .map_err(|_| rejected)?,
        completed: assertion.require_complete,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assigned_source_request_roundtrips_canonical_metadata_and_chunk_selectors() {
        for chunk_index in [None, Some(0), Some(u32::MAX)] {
            let request = SorafsAssignedSourceRequestV1 {
                target_provider: [1; 32],
                source_provider: [2; 32],
                order_id: [3; 32],
                assignment_revision: 4,
                manifest_digest: [5; 32],
                chunk_index,
                floor_height: 6,
                floor_block_hash: [7; 32],
            };
            let mut bytes = norito::encode_canonical(&request).unwrap();
            assert!(bytes.len() < 4096);
            assert_eq!(
                norito::decode_canonical::<SorafsAssignedSourceRequestV1>(&bytes).unwrap(),
                request
            );
            bytes.push(0);
            assert!(norito::decode_canonical::<SorafsAssignedSourceRequestV1>(&bytes).is_err());
        }
    }
}
