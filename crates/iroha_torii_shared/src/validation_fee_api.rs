//! Public Torii DTOs for Parliament-governed validation-fee policy state.

use iroha_crypto::Hash;
use iroha_data_model::{
    ChainId,
    account::AccountId,
    block::consensus_v2::{HeightContextId, finality::V2FinalityArtifact},
    bridge::{BridgeFinalityProof, BridgeFinalityVerifier},
    governance::types::{
        AtWindow, GovernanceFinalizationEvidence, ParliamentBodies, ProposalKind,
        ValidationFeePayoutLifecycleProposal, ValidationFeePolicyProposal,
    },
    isi::governance::VotingMode,
    validation_fee::{
        ValidationFeeChargingMode, ValidationFeePolicyRegistryEntryV1,
        ValidationFeePolicyRegistryV1, ValidationFeePolicySnapshotStatusV1, ValidationFeePolicyV1,
        ValidationFeePolicyWitnessProofV1, ValidationFeeTreasuryPayoutBindingV1,
    },
};
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};

/// Current validation-fee proof request/response layout.
pub const VALIDATION_FEE_POLICY_PROOF_VERSION_V1: u16 = 1;
/// Stable public Norito schema name for the proof request.
pub const VALIDATION_FEE_POLICY_PROOF_REQUEST_SCHEMA_NAME: &str =
    "iroha.torii.v1.validation_fee.current_policy_proof.request";
/// Stable public Norito schema name for the proof response.
pub const VALIDATION_FEE_POLICY_PROOF_RESPONSE_SCHEMA_NAME: &str =
    "iroha.torii.v1.validation_fee.current_policy_proof.response";
/// Stable public JSON schema name for a locally verified policy projection.
pub const VALIDATION_FEE_VERIFIED_POLICY_PROJECTION_SCHEMA_NAME: &str =
    "iroha.validation_fee.verified_policy_projection.v1";
/// Current validation-fee proposal read/draft layout.
pub const VALIDATION_FEE_PROPOSAL_API_VERSION_V1: u16 = 1;
/// Maximum number of consecutive finality proofs, including the trusted checkpoint proof.
pub const VALIDATION_FEE_POLICY_PROOF_MAX_FINALITY_PROOFS: usize = 64;
/// Maximum canonical bytes occupied by the bounded finality chain.
pub const VALIDATION_FEE_POLICY_PROOF_MAX_FINALITY_CHAIN_BYTES: usize = 3 * 1024 * 1024;
/// Defensive maximum for a complete proof response.
pub const VALIDATION_FEE_POLICY_PROOF_MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

/// Select the farthest consecutive tip that fits one checkpoint-promotion page.
#[must_use]
pub fn validation_fee_policy_proof_page_tip(
    trusted_checkpoint_height: u64,
    observed_ledger_tip_height: u64,
) -> Option<u64> {
    if trusted_checkpoint_height == 0 || trusted_checkpoint_height > observed_ledger_tip_height {
        return None;
    }
    let span = u64::try_from(VALIDATION_FEE_POLICY_PROOF_MAX_FINALITY_PROOFS - 1)
        .expect("validation-fee finality proof bound fits u64");
    Some(observed_ledger_tip_height.min(trusted_checkpoint_height.saturating_add(span)))
}

/// Request a current validation-fee registry snapshot from a caller-pinned checkpoint.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    JsonDeserialize,
    JsonSerialize,
    NoritoDeserialize,
    NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct ValidationFeeCurrentPolicyProofRequestV1 {
    /// Request layout version.
    pub version: u16,
    /// Height of the externally trusted checkpoint which must begin the returned chain.
    pub trusted_checkpoint_height: u64,
}

/// A complete registry snapshot authenticated by a block execution commitment and finality chain.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct ValidationFeeCurrentPolicyProofV1 {
    /// Response layout version.
    pub version: u16,
    /// Canonical complete protected registry, or `None` before first enactment.
    pub registry: Option<ValidationFeePolicyRegistryV1>,
    /// Fixed synthetic ordinary-write witness for the registry snapshot.
    pub policy_witness: ValidationFeePolicyWitnessProofV1,
    /// Consecutive finality proofs beginning at the caller's checkpoint.
    pub finality_chain: Vec<BridgeFinalityProof>,
    /// Context id at the evaluated tip, suitable for durable checkpoint promotion.
    pub evaluated_context_id: HeightContextId,
    /// Height whose post-execution policy state was evaluated.
    pub evaluated_block_height: u64,
    /// Canonical lowercase hash of the evaluated committed block.
    pub evaluated_block_hash: String,
    /// Ledger tip observed when this bounded page was assembled.
    pub observed_ledger_tip_height: u64,
    /// Whether another checkpoint-promotion request is required to reach the observed tip.
    pub more_available: bool,
}

/// Minimal policy state returned only after local finality, witness, registry,
/// and immutable deployment-binding verification.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct ValidationFeeVerifiedPolicyProjectionV1 {
    /// Stable projection schema name.
    pub schema: String,
    /// Projection layout version.
    pub version: u16,
    /// Canonical chain identifier supplied by the caller and proven by every policy.
    pub chain_id: String,
    /// Lowercase deployment-bound genesis hash proven by every policy.
    pub genesis_hash: String,
    /// Lowercase hash of policy version one.
    pub policy_chain_genesis_hash: String,
    /// Lowercase hash of the complete immutable registry history.
    pub registry_hash: String,
    /// Latest enacted policy version, including a future scheduled successor.
    pub head_policy_version: u64,
    /// Lowercase hash of the latest enacted policy.
    pub head_policy_hash: String,
    /// Complete policy entry effective at the evaluated height, if one exists.
    pub current_policy: Option<ValidationFeePolicyRegistryEntryV1>,
    /// Caller-pinned checkpoint height at which local finality verification began.
    pub trusted_checkpoint_height: u64,
    /// Lowercase caller-pinned checkpoint context id.
    pub trusted_checkpoint_context_id: String,
    /// Finalized evaluated block height.
    pub evaluated_block_height: u64,
    /// Lowercase evaluated block context id suitable for checkpoint promotion.
    pub evaluated_context_id: String,
    /// Lowercase evaluated block hash.
    pub evaluated_block_hash: String,
    /// Ledger tip observed when Torii assembled this page.
    pub observed_ledger_tip_height: u64,
    /// Whether another bounded proof page is required.
    pub more_available: bool,
}

impl ValidationFeeCurrentPolicyProofV1 {
    /// Verify the canonical registry, ordinary-write witness, and checkpoint-to-tip finality chain.
    ///
    /// # Errors
    ///
    /// Returns a stable explanation when any portable binding is malformed or inconsistent.
    pub fn verify_against(
        &self,
        chain_id: ChainId,
        trusted_checkpoint_height: u64,
        trusted_checkpoint_context_id: [u8; 32],
    ) -> Result<HeightContextId, String> {
        if self.version != VALIDATION_FEE_POLICY_PROOF_VERSION_V1
            || trusted_checkpoint_height == 0
            || self.evaluated_block_height == 0
            || self.observed_ledger_tip_height < self.evaluated_block_height
            || self.more_available
                != (self.evaluated_block_height < self.observed_ledger_tip_height)
        {
            return Err("unsupported validation-fee proof version or invalid trust anchor".into());
        }
        require_nonzero_hash(
            "trusted validation-fee checkpoint context id",
            &trusted_checkpoint_context_id,
        )?;
        let evaluated_block_hash =
            exact_lower_hex_32("evaluated_block_hash", &self.evaluated_block_hash)?;
        require_nonzero_hash("evaluated block hash", &evaluated_block_hash)?;
        if self.finality_chain.is_empty()
            || self.finality_chain.len() > VALIDATION_FEE_POLICY_PROOF_MAX_FINALITY_PROOFS
        {
            return Err("validation-fee finality chain is empty or exceeds 64 proofs".into());
        }
        let finality_bytes = norito::to_bytes(&self.finality_chain)
            .map_err(|error| format!("finality chain encoding failed: {error}"))?;
        if finality_bytes.len() > VALIDATION_FEE_POLICY_PROOF_MAX_FINALITY_CHAIN_BYTES {
            return Err("validation-fee finality chain exceeds its byte bound".into());
        }
        if self.finality_chain.windows(2).any(|pair| {
            pair[0].finality_artifact.height.checked_add(1)
                != Some(pair[1].finality_artifact.height)
        }) {
            return Err("validation-fee finality chain skips or reorders a height".into());
        }
        let trusted_context = HeightContextId(iroha_crypto::HashOf::from_untyped_unchecked(
            Hash::prehashed(trusted_checkpoint_context_id),
        ));
        let first = self
            .finality_chain
            .first()
            .expect("non-empty finality chain");
        if first.finality_artifact.height != trusted_checkpoint_height
            || first.finality_artifact.context_id() != trusted_context
        {
            return Err(
                "validation-fee finality chain does not begin at the caller's checkpoint".into(),
            );
        }
        let mut verifier = BridgeFinalityVerifier::with_context(chain_id.clone(), trusted_context);
        for proof in &self.finality_chain {
            verifier
                .verify(proof)
                .map_err(|error| format!("validation-fee finality chain failed: {error}"))?;
        }
        let evaluated = self
            .finality_chain
            .last()
            .expect("non-empty finality chain");
        let artifact: &V2FinalityArtifact = &evaluated.finality_artifact;
        if artifact.height != self.evaluated_block_height
            || artifact.block_hash.as_ref() != &evaluated_block_hash
            || evaluated.block_header.height().get() != artifact.height
            || evaluated.block_header.hash() != artifact.block_hash
            || self.evaluated_context_id != artifact.context_id()
        {
            return Err("finality chain tip does not match the evaluated policy block".into());
        }
        artifact
            .commit_qc
            .execution_commitment
            .validate()
            .map_err(|error| format!("evaluated execution commitment is invalid: {error}"))?;
        if !self
            .policy_witness
            .verify(artifact.commit_qc.execution_commitment.ordinary_writes_root)
        {
            return Err("validation-fee synthetic write proof is invalid".into());
        }
        let commitment = self.policy_witness.commitment()?;
        if commitment.evaluated_height != artifact.height {
            return Err("validation-fee snapshot height differs from finality".into());
        }
        match (&commitment.status, &self.registry) {
            (ValidationFeePolicySnapshotStatusV1::Unconfigured, None) => {}
            (ValidationFeePolicySnapshotStatusV1::Invalid(_), _) => {
                return Err("protected validation-fee registry is invalid".into());
            }
            (ValidationFeePolicySnapshotStatusV1::Available(available), Some(registry)) => {
                registry
                    .validate()
                    .map_err(|error| format!("validation-fee registry is invalid: {error}"))?;
                if registry
                    .registered_policies
                    .iter()
                    .any(|entry| entry.policy.chain_id != chain_id)
                {
                    return Err("validation-fee registry targets a different chain".into());
                }
                if registry
                    .snapshot_hash()
                    .map_err(|error| format!("validation-fee registry hash failed: {error}"))?
                    != available.registry_hash
                    || registry.head().map(|entry| entry.policy_hash)
                        != Some(available.head_policy_hash)
                    || registry
                        .scheduled_entry_at_height(artifact.height)
                        .map(|entry| entry.policy_hash)
                        != available.scheduled_policy_hash
                    || registry
                        .effective_entry_at_height(artifact.height)
                        .map(|entry| entry.policy_hash)
                        != available.effective_policy_hash
                {
                    return Err(
                        "validation-fee registry differs from its finalized snapshot commitment"
                            .into(),
                    );
                }
            }
            _ => {
                return Err(
                    "validation-fee registry presence differs from its snapshot status".into(),
                );
            }
        }
        Ok(self.evaluated_context_id)
    }

    /// Return the policy effective at the finalized evaluation height.
    #[must_use]
    pub fn current_policy(&self) -> Option<&ValidationFeePolicyV1> {
        self.registry
            .as_ref()?
            .effective_entry_at_height(self.evaluated_block_height)
            .map(|entry| &entry.policy)
    }

    /// Verify the proof and project it under an immutable deployment binding.
    ///
    /// This is deliberately stricter than [`Self::verify_against`]: an
    /// unconfigured registry is rejected because it cannot authenticate the
    /// caller-pinned policy-chain genesis hash.
    ///
    /// # Errors
    ///
    /// Returns an error for any proof failure, absent registry, deployment
    /// genesis mismatch, or policy-chain genesis mismatch.
    pub fn verify_with_immutable_binding(
        &self,
        chain_id: ChainId,
        bound_genesis_hash: [u8; 32],
        policy_chain_genesis_hash: [u8; 32],
        trusted_checkpoint_height: u64,
        trusted_checkpoint_context_id: [u8; 32],
    ) -> Result<ValidationFeeVerifiedPolicyProjectionV1, String> {
        self.verify_against(
            chain_id.clone(),
            trusted_checkpoint_height,
            trusted_checkpoint_context_id,
        )?;
        if bound_genesis_hash == [0; 32] || policy_chain_genesis_hash == [0; 32] {
            return Err("validation-fee immutable binding contains a zero hash".into());
        }
        let registry = self
            .registry
            .as_ref()
            .ok_or_else(|| "validation-fee registry is not configured".to_owned())?;
        if registry.registered_policies.iter().any(|entry| {
            entry.policy.charging_mode
                == ValidationFeeChargingMode::PerQualifyingTransferInstruction
                && (entry.policy.treasury_payout_binding.is_none()
                    || entry.payout_lifecycle.is_none())
        }) {
            return Err(
                "enabled validation-fee policy lacks its mandatory Parliament-enacted payout lifecycle"
                    .into(),
            );
        }
        if registry
            .registered_policies
            .iter()
            .any(|entry| entry.policy.genesis_hash != bound_genesis_hash)
        {
            return Err("validation-fee registry targets a different genesis".into());
        }
        let head = registry
            .head()
            .ok_or_else(|| "validation-fee registry is empty".to_owned())?;
        let first = registry
            .registered_policies
            .first()
            .ok_or_else(|| "validation-fee registry is empty".to_owned())?;
        if first.policy_hash != policy_chain_genesis_hash {
            return Err("validation-fee policy-chain genesis hash mismatch".into());
        }
        let registry_hash = registry
            .snapshot_hash()
            .map_err(|error| format!("validation-fee registry hash failed: {error}"))?;
        Ok(ValidationFeeVerifiedPolicyProjectionV1 {
            schema: VALIDATION_FEE_VERIFIED_POLICY_PROJECTION_SCHEMA_NAME.to_owned(),
            version: VALIDATION_FEE_POLICY_PROOF_VERSION_V1,
            chain_id: chain_id.to_string(),
            genesis_hash: hex::encode(bound_genesis_hash),
            policy_chain_genesis_hash: hex::encode(policy_chain_genesis_hash),
            registry_hash: hex::encode(registry_hash),
            head_policy_version: head.policy.policy_version,
            head_policy_hash: hex::encode(head.policy_hash),
            current_policy: registry
                .effective_entry_at_height(self.evaluated_block_height)
                .cloned(),
            trusted_checkpoint_height,
            trusted_checkpoint_context_id: hex::encode(trusted_checkpoint_context_id),
            evaluated_block_height: self.evaluated_block_height,
            evaluated_context_id: hex::encode(self.evaluated_context_id.0.as_ref()),
            evaluated_block_hash: self.evaluated_block_hash.clone(),
            observed_ledger_tip_height: self.observed_ledger_tip_height,
            more_available: self.more_available,
        })
    }
}

/// Validation-fee governance proposal status exposed by the typed read API.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    JsonDeserialize,
    JsonSerialize,
    NoritoDeserialize,
    NoritoSerialize,
)]
#[norito(tag = "status", content = "value", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ValidationFeeProposalStatusV1 {
    /// Parliament or referendum processing is still in progress.
    Proposed,
    /// The referendum finalized with approval.
    Approved,
    /// The referendum finalized without approval.
    Rejected,
    /// The proposal payload was enacted.
    Enacted,
    /// A concurrently enacted successor made this policy predecessor stale.
    Superseded,
}

/// Referendum state retained with a validation-fee proposal.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    JsonDeserialize,
    JsonSerialize,
    NoritoDeserialize,
    NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct ValidationFeeProposalReferendumV1 {
    /// Exact inclusive voting/enactment window.
    pub window: AtWindow,
    /// Voting mode. First-release validation-fee proposals are always `Plain`.
    pub mode: VotingMode,
    /// Whether Parliament has opened the referendum for voting.
    pub opened: bool,
    /// Whether the referendum is closed.
    pub closed: bool,
}

/// Proposal-time Parliament snapshot committed into the proposal record.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct ValidationFeeParliamentSnapshotV1 {
    /// Proposal-local sortition epoch.
    pub selection_epoch: u64,
    /// Proposal-specific sortition beacon.
    pub beacon: [u8; 32],
    /// Commitment to all seven exact body rosters.
    pub roster_root: [u8; 32],
    /// Independently drawn body rosters.
    pub bodies: ParliamentBodies,
}

/// One complete typed validation-fee proposal read from protected governance state.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct ValidationFeeProposalRecordV1 {
    /// Lowercase deterministic proposal fingerprint.
    pub proposal_id: String,
    /// Bonded citizen who created the proposal.
    pub proposer: AccountId,
    /// Exact native validation-fee proposal kind and payload.
    pub proposal_kind: ProposalKind,
    /// Height at which the proposal was created.
    pub created_height: u64,
    /// Current proposal status.
    pub status: ValidationFeeProposalStatusV1,
    /// Exact retained referendum.
    pub referendum: ValidationFeeProposalReferendumV1,
    /// Proposal-time seven-body Parliament snapshot.
    pub parliament_snapshot: ValidationFeeParliamentSnapshotV1,
    /// Deterministic finalized tally, when finalized.
    pub finalization_evidence: Option<GovernanceFinalizationEvidence>,
    /// Height at which enactment completed.
    pub enacted_at_height: Option<u64>,
}

/// Canonically ordered list of all validation-fee proposals.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct ValidationFeeProposalListV1 {
    /// Response layout version.
    pub version: u16,
    /// Records ordered by creation height then proposal id.
    pub proposals: Vec<ValidationFeeProposalRecordV1>,
}

/// Exact validation-fee proposal detail response.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct ValidationFeeProposalDetailV1 {
    /// Response layout version.
    pub version: u16,
    /// Exact proposal record.
    pub proposal: ValidationFeeProposalRecordV1,
}

/// Exact native validation-fee payload requested from the draft endpoint.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(tag = "kind", content = "payload", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ValidationFeeProposalDraftPayloadV1 {
    /// Draft a policy proposal.
    Policy {
        /// Complete policy payload.
        policy: ValidationFeePolicyV1,
        /// Exact enacted lifecycle proposal selected by a payout-enabled policy.
        payout_lifecycle_proposal_id: Option<[u8; 32]>,
    },
    /// Draft an exact payout lifecycle proposal.
    PayoutLifecycle {
        /// Complete immutable payout binding.
        payout_binding: ValidationFeeTreasuryPayoutBindingV1,
    },
}

impl ValidationFeeProposalDraftPayloadV1 {
    /// Convert the public payload into the exact native proposal kind.
    #[must_use]
    pub fn proposal_kind(&self) -> ProposalKind {
        match self {
            Self::Policy {
                policy,
                payout_lifecycle_proposal_id,
            } => ProposalKind::ValidationFeePolicy(ValidationFeePolicyProposal {
                policy: policy.clone(),
                payout_lifecycle_proposal_id: *payout_lifecycle_proposal_id,
            }),
            Self::PayoutLifecycle { payout_binding } => {
                ProposalKind::ValidationFeePayoutLifecycle(ValidationFeePayoutLifecycleProposal {
                    payout_binding: payout_binding.clone(),
                })
            }
        }
    }
}

/// Strict request for one locally signable native validation-fee proposal instruction.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct ValidationFeeProposalDraftRequestV1 {
    /// Request layout version.
    pub version: u16,
    /// Exact proposal payload.
    pub proposal: ValidationFeeProposalDraftPayloadV1,
    /// Optional inclusive referendum window.
    pub referendum_window: Option<AtWindow>,
    /// Optional voting mode. `None` means `Plain`; `Zk` is rejected.
    pub mode: Option<VotingMode>,
}

/// Canonical framed native instruction returned for local signing and submission.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct ValidationFeeProposalInstructionDraftV1 {
    /// Registered instruction wire identifier.
    pub wire_id: String,
    /// Lowercase hexadecimal canonical framed instruction bytes.
    pub payload_hex: String,
}

/// Strict response binding a draft to its exact native proposal and instruction.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct ValidationFeeProposalDraftResponseV1 {
    /// Response layout version.
    pub version: u16,
    /// Lowercase deterministic proposal fingerprint.
    pub proposal_id: String,
    /// Exact native proposal kind produced by this draft.
    pub proposal_kind: ProposalKind,
    /// Exactly one canonical native proposal instruction.
    pub tx_instructions: Vec<ValidationFeeProposalInstructionDraftV1>,
}

fn exact_lower_hex_32(label: &str, value: &str) -> Result<[u8; 32], String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "{label} must be exactly 64 lowercase hexadecimal digits"
        ));
    }
    let bytes = hex::decode(value).map_err(|error| format!("{label} is invalid: {error}"))?;
    bytes
        .try_into()
        .map_err(|_| format!("{label} must decode to exactly 32 bytes"))
}

fn require_nonzero_hash(label: &str, value: &[u8; 32]) -> Result<(), String> {
    if value.iter().all(|byte| *byte == 0) {
        return Err(format!("{label} must be non-zero"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonzero_hash_validation_accepts_even_ending_hashes() {
        require_nonzero_hash("checkpoint", &[0x02; 32])
            .expect("Iroha hashes have no parity validity bit");
        assert_eq!(
            require_nonzero_hash("checkpoint", &[0; 32]),
            Err("checkpoint must be non-zero".to_owned())
        );
    }

    #[test]
    fn checkpoint_promotion_pages_reach_a_distant_tip_without_gaps() {
        let observed_tip = 250;
        let mut checkpoint = 1;
        let mut pages = Vec::new();
        while checkpoint < observed_tip {
            let next = validation_fee_policy_proof_page_tip(checkpoint, observed_tip)
                .expect("valid checkpoint page");
            assert!(next > checkpoint);
            assert!(next - checkpoint < VALIDATION_FEE_POLICY_PROOF_MAX_FINALITY_PROOFS as u64);
            pages.push((checkpoint, next));
            checkpoint = next;
        }
        assert_eq!(pages, vec![(1, 64), (64, 127), (127, 190), (190, 250)]);
    }
}
