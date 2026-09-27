//! Publication evidence anchored in an independently selected native finality checkpoint.

use super::pin_registry::{PinManifestRecord, ReplicationOrderRecord};
use crate::{
    NetworkId,
    block::{
        consensus_v2::finality::{V2FinalityArtifact, verify_finality_successor},
        decode_framed_signed_block,
        proofs::TrustedBlockProofAnchor,
    },
    isi::sorafs::AssertSorafsPublicationV1,
    transaction::{Executable, SignedTransaction, TransactionEntrypoint},
};
use norito::{Decode, Encode};

/// Maximum canonical publication response, including its authenticated executed block carrier.
pub const PUBLICATION_PROOF_MAX_BYTES_V1: usize = 32 * 1024 * 1024;
/// Maximum consecutive finality artifacts accepted in one publication proof.
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
    /// Consecutive exact native artifacts, including the independently pinned floor.
    pub lineage: Vec<V2FinalityArtifact>,
    /// Canonical `SignedBlockWire` containing the exact successful signed assertion.
    pub executed_block: Vec<u8>,
}

/// Opaque successful verification of the client's original challenged assertion.
#[derive(Debug)]
pub struct VerifiedSorafsPublicationV1 {
    finality: V2FinalityArtifact,
    completed: bool,
}
impl VerifiedSorafsPublicationV1 {
    /// Verified artifact that can be retained as the next independent checkpoint.
    pub fn finality(&self) -> &V2FinalityArtifact {
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
/// `checkpoint` must come from an independently trusted local configuration or a preceding call's
/// verified output. Never pass the proof's first artifact as the checkpoint. The caller must start
/// a monotonic deadline before generating its challenge and reject verification after that deadline.
pub fn verify_sorafs_publication_v1(
    network: &NetworkId,
    checkpoint: &V2FinalityArtifact,
    expected: &SignedTransaction,
    proof: &SorafsPublicationProofV1,
) -> Result<VerifiedSorafsPublicationV1, SorafsPublicationProofErrorV1> {
    let rejected = SorafsPublicationProofErrorV1;
    if proof.lineage.is_empty()
        || proof.lineage.len() > PUBLICATION_PROOF_MAX_BLOCKS_V1
        || proof.executed_block.len() > PUBLICATION_PROOF_MAX_BYTES_V1
        || norito::core::encoded_frame_len(proof).map_err(|_| rejected)?
            > PUBLICATION_PROOF_MAX_BYTES_V1
        || checkpoint.height_context.network_id != *network
        || expected.network_id() != Some(network)
        || proof.lineage.first() != Some(checkpoint)
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
        || assertion.minimum_height != checkpoint.height
        || assertion.minimum_block_hash != *checkpoint.block_hash.as_ref()
    {
        return Err(rejected);
    }
    checkpoint.verify().map_err(|_| rejected)?;
    for edge in proof.lineage.windows(2) {
        verify_finality_successor(&edge[0], &edge[1]).map_err(|_| rejected)?;
    }
    let last = proof.lineage.last().ok_or(rejected)?;
    if last.height <= checkpoint.height {
        return Err(rejected);
    }
    let block = decode_framed_signed_block(&proof.executed_block).map_err(|_| rejected)?;
    let entry_hash = expected.hash_as_entrypoint();
    let anchor = TrustedBlockProofAnchor::from_untrusted_finality_artifact(
        &block,
        last,
        last.context_id(),
        &entry_hash,
    )
    .map_err(|_| rejected)?;
    let actual = block
        .network_entrypoint_at(anchor.entry_index() as usize)
        .ok_or(rejected)?;
    if actual != &TransactionEntrypoint::External(expected.clone()) {
        return Err(rejected);
    }
    let inclusion = block.network_execution_proof(&entry_hash).ok_or(rejected)?;
    let (_, output) = block
        .network_output_at(anchor.entry_index())
        .ok_or(rejected)?;
    if !inclusion.verify(&anchor) || !output.result.is_ok() {
        return Err(rejected);
    }
    Ok(VerifiedSorafsPublicationV1 {
        finality: last.clone(),
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
