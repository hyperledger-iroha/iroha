//! Direct quorum-certified execution results for light clients.
//!
//! A signed genesis fixes the first epoch's complete authority and height interval.
//! Each incumbent boundary quorum certifies the next interval and ordered roster.
//! Intermediate ordinary blocks are unnecessary: honest Commit voters already validate
//! execution, availability and the lag-two schedule before signing the result. This reader
//! authenticates that consensus decision; it does not manufacture full-block custody.

use super::*;
use crate::sumeragi::epoch::ValidatorEpochContextV1;

const MAX_COMMIT_HEADER_BYTES: usize = 64 * 1024;
const MAX_COMMIT_QC_BYTES: usize = 4 * 1024;

/// Original native header, CommitQC and signed execution-result preimage, without a block body.
/// The ordered signing roster comes exclusively from independently authenticated epochs.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, iroha_schema::IntoSchema, norito::NoritoSchema)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::sumeragi_finality::SumeragiCommitCertificateV1")]
pub struct SumeragiCommitCertificateV1 {
    /// Canonical native consensus header named by the quorum certificate.
    pub consensus_header: Vec<u8>,
    /// Canonical exact-quorum BLS Commit certificate.
    pub commit_qc: Vec<u8>,
    /// Complete canonical native execution result, including its counted event root.
    pub result_preimage: Vec<u8>,
}

impl SumeragiCommitCertificateV1 {
    /// Retain the original certificate from an independently verified non-genesis block.
    ///
    /// # Errors
    /// Genesis has no CommitQC; malformed or oversized original components are rejected.
    pub fn from_verified(block: &VerifiedSumeragiBlock) -> Result<Self, FinalityError> {
        need(block.height() >= 2, "genesis has no Commit certificate")?;
        let certificate = block.block().commit_certificate().ok_or_else(|| {
            FinalityError("verified block has no embedded Commit certificate".into())
        })?;
        let value = Self {
            consensus_header: certificate.consensus_header().to_vec(),
            commit_qc: certificate.commit_qc().to_vec(),
            result_preimage: certificate.result_preimage().to_vec(),
        };
        value.validate_shape()?;
        Ok(value)
    }

    /// Check finite component lengths without authenticating any signed statement.
    ///
    /// # Errors
    /// Missing or oversized components are rejected before native decoding.
    pub fn validate_shape(&self) -> Result<(), FinalityError> {
        for (bytes, limit) in [
            (&self.consensus_header, MAX_COMMIT_HEADER_BYTES),
            (&self.commit_qc, MAX_COMMIT_QC_BYTES),
            (&self.result_preimage, MAX_RESULT_PREIMAGE_BYTES),
        ] {
            need(!bytes.is_empty() && bytes.len() <= limit, "compact certificate component exceeds its bound")?;
        }
        Ok(())
    }
}

/// Quorum-certified execution under a genesis-rooted, height-authorized committee.
/// No public constructor or decoder can manufacture this capability.
#[derive(Debug, Clone)]
pub struct VerifiedSumeragiCommitV1 {
    network: NetworkId,
    chain: String,
    core_hash: Hash32,
    commitment: ExecutionResultCommitment,
}

impl VerifiedSumeragiCommitV1 {
    /// Original certified execution height.
    #[must_use]
    pub const fn height(&self) -> u64 { self.commitment.height }

    /// Native consensus block identity authenticated by the BLS certificate.
    #[must_use]
    pub const fn core_hash(&self) -> Hash32 { self.core_hash }

    /// Certified execution, including its original counted event commitment.
    #[must_use]
    pub const fn execution(&self) -> &ExecutionCommitment { &self.commitment.execution }

    /// Complete certified schedule and execution result.
    #[must_use]
    pub const fn commitment(&self) -> &ExecutionResultCommitment { &self.commitment }

    /// Match the independently selected global network and chain.
    ///
    /// # Errors
    /// A different network or chain is rejected.
    pub fn verify_global_scope(&self, network: NetworkId, chain: &str) -> Result<(), FinalityError> {
        need(self.network == network && self.chain == chain, "compact certificate global scope differs")
    }
}

/// Native BLS light reader with authenticated epoch authority, never caller-selected rosters.
///
/// Ordinary Commit results need no history replay within their signed epoch interval.
/// Crossing an interval requires its exact final-height Commit result under the incumbent
/// quorum. Certified parameters and selection outputs rely on ordinary honest-validator
/// execution, just like the successful Load event; this is not VM reexecution.
#[derive(Debug, Clone)]
pub struct SumeragiCommitVerifierV1 {
    network: NetworkId,
    chain: String,
    instance: Hash32,
    epochs: BTreeMap<u64, ValidatorEpochContextV1>,
}

impl SumeragiCommitVerifierV1 {
    /// Select the original signed global genesis and any independently verified epoch decisions.
    /// The supplied owner must already have authenticated and selected its genesis.
    ///
    /// # Errors
    /// Private roots or an inconsistent signed genesis context are rejected.
    pub fn new(native: &SumeragiFinalityVerifier) -> Result<Self, FinalityError> {
        need(
            matches!(native.root_scope().map_err(malformed)?, crate::block::consensus::SumeragiRootScope::Global),
            "compact global finality requires global signed genesis",
        )?;
        let mut epochs = BTreeMap::new();
        let initial = native.initial_epoch();
        initial.validate().map_err(malformed)?;
        epochs.insert(initial.authorization.epoch, initial.clone());
        for decision in native.decisions.values() {
            epochs.insert(decision.schedule.current.authorization.epoch, decision.schedule.current.clone());
            if let Some(boundary) = &decision.schedule.boundary {
                epochs.insert(boundary.next.authorization.epoch, boundary.next.clone());
            }
        }
        Ok(Self {
            network: initial.network_id,
            chain: native.chain_id().into(),
            instance: native.instance(),
            epochs,
        })
    }

    /// Independently selected genesis-derived network.
    #[must_use]
    pub const fn network(&self) -> NetworkId { self.network }

    /// Independently selected chain label.
    #[must_use]
    pub fn chain_id(&self) -> &str { &self.chain }

    /// Authenticate one exact CommitQC and install only its certified boundary successor.
    ///
    /// # Errors
    /// Missing epoch transitions, stale or substituted rosters, wrong scope, height/result
    /// bindings, noncanonical bytes, malformed schedules and invalid exact-quorum BLS fail.
    /// A failure leaves all retained authority unchanged.
    pub fn verify(&mut self, proof: &SumeragiCommitCertificateV1) -> Result<VerifiedSumeragiCommitV1, FinalityError> {
        proof.validate_shape()?;
        let header: CoreHeader = norito::decode_canonical(&proof.consensus_header).map_err(malformed)?;
        let qc: Qc = norito::decode_canonical(&proof.commit_qc).map_err(malformed)?;
        let commitment = ExecutionResultCommitment::decode(&proof.result_preimage).map_err(malformed)?;
        let offered = &commitment.schedule.current;
        let selected = self.epochs.get(&offered.authorization.epoch)
            .ok_or_else(|| FinalityError("missing certified predecessor epoch boundary".into()))?;
        need(selected == offered, "certificate roster differs from authenticated epoch authority")?;
        need(header.height >= 2 && header.height >= selected.authorization.first_height
            && header.height <= selected.authorization.last_height,
            "certificate height is outside authenticated epoch authority")?;
        let epoch = core_epoch(selected).map_err(malformed)?;
        let validators: Vec<_> = selected.committee.iter().map(|member| FinalityValidator {
            public_key: member.validator.public_key().clone(),
            proof_of_possession: member.proof_of_possession.clone(),
        }).collect();
        let (crypto, committee) = ProofCrypto::new(&validators)?;
        let core_hash = header.hash(&crypto);
        need(header.instance == self.instance && header.epoch == epoch.id
            && qc.instance == self.instance && qc.epoch == epoch.id
            && qc.kind == VoteKind::Commit && qc.height == header.height
            && qc.block_hash == core_hash && qc.result == result_of_preimage(&proof.result_preimage)
            && commitment.height == header.height,
            "compact certificate does not bind the selected instance, epoch and execution")?;
        iroha_sumeragi::crypto::Verifier::new(&crypto, &self.instance, &epoch.id, &committee)
            .verify_qc(&qc).map_err(|error| FinalityError(format!("commit certificate: {error:?}")))?;
        if let Some(boundary) = &commitment.schedule.boundary {
            if let Some(previous) = self.epochs.get(&boundary.next.authorization.epoch) {
                need(previous == &boundary.next, "conflicting certified epoch successor")?;
            }
            self.epochs.insert(boundary.next.authorization.epoch, boundary.next.clone());
        }
        Ok(VerifiedSumeragiCommitV1 {
            network: self.network,
            chain: self.chain.clone(),
            core_hash,
            commitment,
        })
    }
}

#[cfg(all(test, feature = "transparent_api"))]
mod tests;
