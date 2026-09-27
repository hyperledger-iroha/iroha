//! Read-only export of retained outgoing State proofs for testnet observation.
//!
//! The existing hardware-anchored Core snapshot retains each live outgoing candidate's
//! original paired proof and every field needed to reconstruct its exact public inputs.
//! This projection does not authorize a transition or replace qualified snapshot recovery.

use super::*;
use crate::zk::kagemusha_v1_recursion::kagemusha_candidate_envelope_digest_v1;

/// Maximum canonical State public-input archive accepted by the native testnet observer.
pub const KAGEMUSHA_OUTGOING_STATE_PUBLIC_INPUT_ARCHIVE_MAX_BYTES_V1: usize = 4 * 1024;

/// Observer-ready canonical archives from one still-retained Core outgoing operation.
///
/// These bytes are public proof material. Copying them confers no monetary, hardware, or
/// coordinator authority; the independently pinned testnet observer verifies the pair again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaOutgoingStateProofArchivePairV1 {
    /// Original caller operation ID selected from Core's authenticated index.
    pub operation_id: DigestV1,
    /// Exact model-canonical `KagemushaStateRelationPublicInputsV1` archive.
    pub public_inputs_archive: Vec<u8>,
    /// Exact original model-canonical `KagemushaPairedProofV1` archive.
    pub paired_proof_archive: Vec<u8>,
}

impl<R, G, H> KagemushaStateMachineV1<R, G, H>
where
    R: KagemushaRecursiveVerifierV1,
    G: KagemushaGuardBundleVerifierV1,
    H: KagemushaAuthenticatedHistoryStoreV1,
{
    /// Export the original State proof and its exact public inputs for a live outgoing operation.
    ///
    /// Candidate, committed, and installed operations retain the proof inside the hardware-anchored
    /// snapshot. Released operations deliberately drop the candidate and cannot be exported.
    /// The caller must supply a Core-indexed operation ID, never proof or public-input bytes.
    /// This method rechecks the retained record, proof and release before encoding either archive.
    ///
    /// # Errors
    ///
    /// Rejects missing, released, stale, foreign, mismatched, unverified or oversized material.
    pub fn export_outgoing_state_proof_archives(
        &self,
        operation_id: DigestV1,
    ) -> Result<KagemushaOutgoingStateProofArchivePairV1, KagemushaStateErrorV1> {
        let journal = &self.outgoing_candidate_journal;
        let record = journal
            .operation_index()
            .lookup(operation_id)
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        record
            .context
            .validate_retained_against_state(&self.state)
            .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
        let candidate = match record.phase {
            KagemushaOutgoingOperationPhaseV1::CandidatePersisted => {
                let KagemushaOutgoingJournalStageV1::Candidate(candidate) = journal.stage() else {
                    return Err(KagemushaStateErrorV1::InvalidCandidateStage);
                };
                if candidate.prepared.predecessor_state != self.state
                    || candidate.prepared.proof_statement.journal_revision_before
                        != self.journal_revision
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                candidate
            }
            KagemushaOutgoingOperationPhaseV1::Committed => {
                let KagemushaOutgoingJournalStageV1::Committed(committed) = journal.stage() else {
                    return Err(KagemushaStateErrorV1::InvalidCandidateStage);
                };
                if committed.candidate.prepared.successor_state != self.state
                    || committed
                        .candidate
                        .prepared
                        .proof_statement
                        .journal_revision_after
                        != self.journal_revision
                    || record.commit_certificate_digest != Some(committed.commit_certificate_digest)
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                &committed.candidate
            }
            KagemushaOutgoingOperationPhaseV1::Installed => {
                let finalized = journal
                    .finalized_envelope(record.outbox_reservation_id)
                    .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
                candidate_lifecycle::validate_installed_successor_not_ahead(
                    &self.state,
                    finalized.successor_state(),
                )?;
                if record.commit_certificate_digest
                    != Some(finalized.committed.commit_certificate_digest)
                    || record.envelope_digest != Some(finalized.envelope_digest)
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                &finalized.committed.candidate
            }
            KagemushaOutgoingOperationPhaseV1::Prepared
            | KagemushaOutgoingOperationPhaseV1::Released => {
                return Err(KagemushaStateErrorV1::InvalidCandidateStage);
            }
        };
        record
            .validate_against_prepared(&candidate.prepared)
            .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
        candidate
            .prepared
            .validate_recipient_against_release(&self.proof_release)?;
        if record.candidate_digest != Some(candidate.candidate_envelope_digest) {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let proof = candidate.recovery_view()?.candidate_proof;
        let artifacts = self.proof_release.artifacts;
        let public_inputs = candidate
            .prepared
            .candidate_public_inputs(artifacts, proof)
            .map_err(KagemushaStateErrorV1::ProofRejected)?;
        let digest = kagemusha_candidate_envelope_digest_v1(&public_inputs)
            .map_err(KagemushaStateErrorV1::ProofRejected)?;
        if digest != candidate.candidate_envelope_digest {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        verify_kagemusha_state_proof_v1(&self.recursive_verifier, artifacts, &public_inputs, proof)
            .map_err(|error| KagemushaStateErrorV1::ProofRejected(error.to_string()))?;
        let public_inputs_archive = norito::encode_canonical(&public_inputs)
            .map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)?;
        let paired_proof_archive = norito::encode_canonical(proof)
            .map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)?;
        if public_inputs_archive.is_empty()
            || public_inputs_archive.len()
                > KAGEMUSHA_OUTGOING_STATE_PUBLIC_INPUT_ARCHIVE_MAX_BYTES_V1
            || paired_proof_archive.is_empty()
            || paired_proof_archive.len() > KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1
        {
            return Err(KagemushaStateErrorV1::InvalidProofBundle);
        }
        Ok(KagemushaOutgoingStateProofArchivePairV1 {
            operation_id,
            public_inputs_archive,
            paired_proof_archive,
        })
    }
}
