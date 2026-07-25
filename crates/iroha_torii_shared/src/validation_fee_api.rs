//! Public Torii DTOs for Parliament-governed validation-fee policy state.

use iroha_crypto::Hash;
use iroha_data_model::{
    ChainId,
    account::AccountId,
    block::consensus_v2::{HeightContextId, finality::V2FinalityArtifact},
    bridge::{BridgeFinalityProof, BridgeFinalityVerifier},
    governance::types::{
        AtWindow, GovernanceFinalizationEvidence, ParliamentBodies, ParliamentBody, ProposalKind,
        ValidationFeePayoutLifecycleProposal, ValidationFeePolicyProposal,
    },
    isi::governance::VotingMode,
    validation_fee::{
        ValidationFeeChargingMode, ValidationFeeGovernanceVotingModeV1,
        ValidationFeeParliamentAuthorizationV1, ValidationFeePolicyRegistryEntryV1,
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
    /// JSON-safe complete policy/evidence projection effective at the evaluated height.
    pub current_policy: Option<ValidationFeeVerifiedCurrentPolicyV1>,
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

/// Exact JSON-safe mobile policy shape derived only from a verified registry entry.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct ValidationFeeVerifiedCurrentPolicyV1 {
    /// Canonical decimal policy version.
    #[norito(rename = "activePolicyVersion")]
    pub active_policy_version: String,
    /// Canonical lowercase Iroha policy hash.
    #[norito(rename = "activePolicyHash")]
    pub active_policy_hash: String,
    /// Canonical public fee-asset definition address.
    #[norito(rename = "feeAssetDefinitionId")]
    pub fee_asset_definition_id: String,
    /// Fee-asset decimal scale.
    #[norito(rename = "feeScale")]
    pub fee_scale: u8,
    /// Exact fee minor units as a canonical decimal string.
    #[norito(rename = "feeMinorUnits")]
    pub fee_minor_units: String,
    /// Exact charging mode.
    #[norito(rename = "chargingMode")]
    pub charging_mode: String,
    /// First active height as a canonical decimal string.
    #[norito(rename = "effectiveFromHeight")]
    pub effective_from_height: String,
    /// Exclusive expiry height as a canonical decimal string, when present.
    #[norito(rename = "expiresAfterHeight")]
    pub expires_after_height: Option<String>,
    /// Complete policy and payout-lifecycle Parliament evidence.
    pub parliament: ValidationFeeVerifiedParliamentV1,
    /// Complete immutable payout binding.
    pub payout: ValidationFeeVerifiedPayoutV1,
}

/// Complete policy and payout-lifecycle Parliament evidence.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct ValidationFeeVerifiedParliamentV1 {
    /// Authorization for the policy proposal.
    #[norito(rename = "validationFeePolicy")]
    pub validation_fee_policy: ValidationFeeVerifiedParliamentProposalV1,
    /// Authorization for the exact payout-lifecycle proposal.
    #[norito(rename = "payoutLifecycle")]
    pub payout_lifecycle: ValidationFeeVerifiedParliamentProposalV1,
    /// Canonical Iroha hash sealing the payout binding.
    #[norito(rename = "payoutLifecycleSealHash")]
    pub payout_lifecycle_seal_hash: String,
}

/// Complete JSON-safe authorization for one enacted Parliament proposal.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct ValidationFeeVerifiedParliamentProposalV1 {
    /// Exact proposal kind expected by mobile runtime configuration.
    pub proposal_kind: String,
    /// Raw native proposal fingerprint in canonical lowercase hexadecimal.
    pub proposal_id: String,
    /// Fingerprint of the exact typed proposal preimage.
    pub payload_hash: String,
    /// Raw Parliament roster commitment in canonical lowercase hexadecimal.
    pub parliament_roster_root: String,
    /// Exact referendum and enactment window.
    pub enactment_window: ValidationFeeVerifiedEnactmentWindowV1,
    /// Complete finalized PLAIN referendum evidence.
    pub finalization: ValidationFeeVerifiedFinalizationV1,
}

/// JSON-safe Parliament referendum and enactment heights.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
#[expect(
    clippy::struct_field_names,
    reason = "the canonical V1 JSON/Norito field names intentionally share the `_at_height` suffix"
)]
pub struct ValidationFeeVerifiedEnactmentWindowV1 {
    /// Inclusive referendum opening height.
    pub opens_at_height: String,
    /// Inclusive referendum closing height.
    pub closes_at_height: String,
    /// Height at which the approved proposal was enacted.
    pub enacted_at_height: String,
}

/// Complete JSON-safe deterministic PLAIN referendum result.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct ValidationFeeVerifiedFinalizationV1 {
    /// Native policy or payout-lifecycle proposal identifier.
    pub proposal_id: String,
    /// Native referendum identifier.
    pub referendum_id: String,
    /// Finalization height as a canonical decimal string.
    pub finalized_at_height: String,
    /// Exact first-release voting mode (`PLAIN`).
    pub mode: String,
    /// Final approve weight as a canonical full-range u128 decimal string.
    pub approve: String,
    /// Final reject weight as a canonical full-range u128 decimal string.
    pub reject: String,
    /// Final abstain weight as a canonical full-range u128 decimal string.
    pub abstain: String,
    /// Minimum turnout as a canonical full-range u128 decimal string.
    pub min_turnout: String,
    /// Approval threshold numerator as a canonical decimal string.
    pub approval_threshold_numerator: String,
    /// Approval threshold denominator as a canonical decimal string.
    pub approval_threshold_denominator: String,
    /// Final deterministic decision.
    pub approved: bool,
}

/// Complete JSON-safe immutable payout binding.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct ValidationFeeVerifiedPayoutV1 {
    /// Canonical deployed contract address.
    #[norito(rename = "contractAddress")]
    pub contract_address: String,
    /// Raw SHA-256 contract artifact digest in lowercase hexadecimal.
    #[norito(rename = "codeHash")]
    pub code_hash: String,
    /// Exact autonomous entrypoint.
    pub entrypoint: String,
    /// Canonical public SBD asset-definition address.
    #[norito(rename = "sbdAssetDefinitionId")]
    pub sbd_asset_definition_id: String,
    /// Canonical public XOR asset-definition address.
    #[norito(rename = "xorAssetDefinitionId")]
    pub xor_asset_definition_id: String,
    /// Canonical non-signable contract treasury account.
    #[norito(rename = "treasuryAccountId")]
    pub treasury_account_id: String,
    /// Canonical pool vault account.
    #[norito(rename = "vaultAccountId")]
    pub vault_account_id: String,
    /// Exact SBD payout batch in fee-asset minor units.
    #[norito(rename = "batchSbdMinorUnits")]
    pub batch_sbd_minor_units: String,
    /// SBD asset scale.
    #[norito(rename = "sbdScale")]
    pub sbd_scale: u8,
    /// Inclusive minimum XOR output as a canonical decimal string.
    #[norito(rename = "xorOutputMin")]
    pub xor_output_min: String,
    /// Inclusive maximum XOR output as a canonical decimal string.
    #[norito(rename = "xorOutputMax")]
    pub xor_output_max: String,
    /// Four ordered validator payout recipients.
    pub recipients: Vec<ValidationFeeVerifiedPayoutRecipientV1>,
}

/// One exact payout recipient; no unproved validator identity is projected.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct ValidationFeeVerifiedPayoutRecipientV1 {
    /// Canonical recipient account.
    pub account_id: String,
    /// Exact payout share in basis points.
    pub share_basis_points: u16,
}

fn verified_parliament_proposal(
    proposal_kind: &str,
    authorization: &ValidationFeeParliamentAuthorizationV1,
) -> Result<ValidationFeeVerifiedParliamentProposalV1, String> {
    if authorization.proposal_id != authorization.proposal_fingerprint {
        return Err(
            "validation-fee Parliament proposal id differs from its typed payload hash".to_owned(),
        );
    }
    if authorization.finalization.mode != ValidationFeeGovernanceVotingModeV1::Plain {
        return Err("validation-fee Parliament projection requires PLAIN finalization".to_owned());
    }
    let proposal_id = hex::encode(authorization.proposal_id);
    let finalization = authorization.finalization;
    Ok(ValidationFeeVerifiedParliamentProposalV1 {
        proposal_kind: proposal_kind.to_owned(),
        proposal_id: proposal_id.clone(),
        payload_hash: hex::encode(authorization.proposal_fingerprint),
        parliament_roster_root: hex::encode(authorization.proposal_time_roster_root),
        enactment_window: ValidationFeeVerifiedEnactmentWindowV1 {
            opens_at_height: authorization.referendum_window.lower.to_string(),
            closes_at_height: authorization.referendum_window.upper.to_string(),
            enacted_at_height: authorization.enacted_at_height.to_string(),
        },
        finalization: ValidationFeeVerifiedFinalizationV1 {
            proposal_id,
            referendum_id: hex::encode(finalization.referendum_id),
            finalized_at_height: finalization.finalized_at_height.to_string(),
            mode: "PLAIN".to_owned(),
            approve: finalization.approve.to_string(),
            reject: finalization.reject.to_string(),
            abstain: finalization.abstain.to_string(),
            min_turnout: finalization.min_turnout.to_string(),
            approval_threshold_numerator: finalization.approval_threshold_numerator.to_string(),
            approval_threshold_denominator: finalization.approval_threshold_denominator.to_string(),
            approved: finalization.approved,
        },
    })
}

fn verified_current_policy(
    entry: &ValidationFeePolicyRegistryEntryV1,
) -> Result<Option<ValidationFeeVerifiedCurrentPolicyV1>, String> {
    if entry.policy.charging_mode == ValidationFeeChargingMode::Disabled {
        return Ok(None);
    }
    let payout = entry
        .policy
        .treasury_payout_binding
        .as_ref()
        .ok_or_else(|| "enabled validation-fee policy has no payout binding".to_owned())?;
    let payout_lifecycle = entry
        .payout_lifecycle
        .as_ref()
        .ok_or_else(|| "enabled validation-fee policy has no payout lifecycle".to_owned())?;
    if payout.invariant_error().is_some() || payout_lifecycle.invariant_error().is_some() {
        return Err("enabled validation-fee payout binding or lifecycle is invalid".into());
    }
    let recipients = payout
        .recipients
        .iter()
        .map(|recipient| ValidationFeeVerifiedPayoutRecipientV1 {
            account_id: recipient.account_id.to_string(),
            // The protected payout invariant fixes exactly four equal 25% shares.
            share_basis_points: 2_500,
        })
        .collect();
    Ok(Some(ValidationFeeVerifiedCurrentPolicyV1 {
        active_policy_version: entry.policy.policy_version.to_string(),
        active_policy_hash: hex::encode(entry.policy_hash),
        fee_asset_definition_id: entry.policy.ds_asset_id.to_string(),
        fee_scale: entry.policy.ds_scale,
        // The protected enabled policy invariant fixes exactly 0.10 SBD at scale two.
        fee_minor_units: "10".to_owned(),
        charging_mode: "PER_QUALIFYING_TRANSFER_INSTRUCTION".to_owned(),
        effective_from_height: entry.policy.effective_from_height.to_string(),
        expires_after_height: entry
            .policy
            .expires_after_height
            .map(|height| height.to_string()),
        parliament: ValidationFeeVerifiedParliamentV1 {
            validation_fee_policy: verified_parliament_proposal(
                "ValidationFeePolicyV1",
                &entry.parliament_authorization,
            )?,
            payout_lifecycle: verified_parliament_proposal(
                "ValidationFeePayoutLifecycleV1",
                &payout_lifecycle.parliament_authorization,
            )?,
            payout_lifecycle_seal_hash: hex::encode(payout_lifecycle.lifecycle_seal),
        },
        payout: ValidationFeeVerifiedPayoutV1 {
            contract_address: payout.contract_address.to_string(),
            code_hash: hex::encode(payout.code_hash),
            entrypoint: payout.entrypoint.to_string(),
            sbd_asset_definition_id: payout.sbd_asset_id.to_string(),
            xor_asset_definition_id: payout.xor_asset_id.to_string(),
            treasury_account_id: payout.treasury_account_id.to_string(),
            vault_account_id: payout.pool_vault_account_id.to_string(),
            // The protected lifecycle invariant fixes exactly 10 SBD at scale two.
            batch_sbd_minor_units: "1000".to_owned(),
            sbd_scale: entry.policy.ds_scale,
            xor_output_min: payout.min_xor_out.to_string(),
            xor_output_max: payout.max_xor_out.to_string(),
            recipients,
        },
    }))
}

impl ValidationFeeCurrentPolicyProofV1 {
    /// Verify the canonical registry, ordinary-write witness, and checkpoint-to-tip finality chain.
    ///
    /// # Errors
    ///
    /// Returns a stable explanation when any portable binding is malformed or inconsistent.
    #[expect(
        clippy::too_many_lines,
        reason = "the ordered V1 proof checks preserve fail-closed validation and stable error precedence"
    )]
    #[expect(
        clippy::needless_pass_by_value,
        reason = "the public V1 verifier owns one ChainId across finality verification and registry binding"
    )]
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
        require_canonical_iroha_hash(
            "trusted validation-fee checkpoint context id",
            &trusted_checkpoint_context_id,
        )?;
        let evaluated_block_hash =
            exact_lower_hex_32("evaluated_block_hash", &self.evaluated_block_hash)?;
        require_canonical_iroha_hash("evaluated block hash", &evaluated_block_hash)?;
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
    #[expect(
        clippy::needless_pass_by_value,
        reason = "the public V1 verifier owns one ChainId across finality verification and immutable deployment binding"
    )]
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
        require_canonical_iroha_hash(
            "validation-fee immutable binding genesis hash",
            &bound_genesis_hash,
        )?;
        require_canonical_iroha_hash(
            "validation-fee immutable binding policy-chain genesis hash",
            &policy_chain_genesis_hash,
        )?;
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
        let current_policy = registry
            .effective_entry_at_height(self.evaluated_block_height)
            .map(verified_current_policy)
            .transpose()?
            .flatten();
        Ok(ValidationFeeVerifiedPolicyProjectionV1 {
            schema: VALIDATION_FEE_VERIFIED_POLICY_PROJECTION_SCHEMA_NAME.to_owned(),
            version: VALIDATION_FEE_POLICY_PROOF_VERSION_V1,
            chain_id: chain_id.to_string(),
            genesis_hash: hex::encode(bound_genesis_hash),
            policy_chain_genesis_hash: hex::encode(policy_chain_genesis_hash),
            registry_hash: hex::encode(registry_hash),
            head_policy_version: head.policy.policy_version,
            head_policy_hash: hex::encode(head.policy_hash),
            current_policy,
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
    pub selection_epoch: String,
    /// Proposal-specific sortition beacon.
    pub beacon: [u8; 32],
    /// Commitment to all seven exact body rosters.
    pub roster_root: [u8; 32],
    /// Independently drawn body rosters.
    pub bodies: ParliamentBodies,
}

/// JSON-safe governance pipeline retained with a proposal.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct ValidationFeeProposalPipelineV1 {
    /// Ordered stage records.
    pub stages: Vec<ValidationFeeProposalPipelineStageV1>,
}

/// One JSON-safe proposal pipeline stage.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct ValidationFeeProposalPipelineStageV1 {
    /// Stable governance stage name.
    pub stage: String,
    /// Block height at which the stage began.
    pub started_at: String,
    /// Inclusive stage deadline, when configured.
    pub deadline: Option<String>,
    /// Completion height, when completed or failed.
    pub completed_at: Option<String>,
    /// Stable failure description, when the stage failed.
    pub failure: Option<String>,
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
    pub created_height: String,
    /// Current proposal status.
    pub status: ValidationFeeProposalStatusV1,
    /// Exact governance pipeline with JSON-safe heights.
    pub pipeline: ValidationFeeProposalPipelineV1,
    /// Exact retained referendum.
    pub referendum: ValidationFeeProposalReferendumV1,
    /// Proposal-time seven-body Parliament snapshot.
    pub parliament_snapshot: ValidationFeeParliamentSnapshotV1,
    /// Deterministic finalized tally, when finalized.
    pub finalization_evidence: Option<GovernanceFinalizationEvidence>,
    /// Height at which enactment completed.
    pub enacted_at_height: Option<String>,
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
    /// Latest committed height used for this projection.
    pub current_height: String,
    /// Exact progress for every required proposal-local Parliament body.
    pub body_progress: Vec<ValidationFeeParliamentBodyProgressV1>,
    /// Live or finalized citizen tally.
    pub tally: ValidationFeeProposalTallyV1,
    /// Current retained citizen locks for this referendum.
    pub locks: ValidationFeeProposalLocksV1,
}

/// Optional account selector for proposal-local Parliament decision projection.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Default,
    JsonDeserialize,
    JsonSerialize,
    NoritoDeserialize,
    NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct ValidationFeeProposalDetailQueryV1 {
    /// Canonical account whose exact seated-body decision should be projected.
    #[norito(default)]
    pub account_id: Option<AccountId>,
}

/// Exact progress for one proposal-local Parliament body.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct ValidationFeeParliamentBodyProgressV1 {
    /// Parliament body.
    pub body: ParliamentBody,
    /// Exact immutable seated members.
    pub members: Vec<AccountId>,
    /// Exact immutable alternates, who are not eligible to vote.
    pub alternates: Vec<AccountId>,
    /// Number of approvals required for this actual roster.
    pub required: String,
    /// Recorded approvals.
    pub approve: String,
    /// Recorded rejections.
    pub reject: String,
    /// Recorded abstentions.
    pub abstain: String,
    /// Whether the approval quorum has been reached.
    pub approval_quorum_met: bool,
    /// Whether the rejection quorum has been reached.
    pub rejection_quorum_met: bool,
    /// Selected account's exact decision, encoded as `APPROVE`, `REJECT`, or `ABSTAIN`.
    pub current_account_decision: Option<String>,
}

/// Citizen referendum tally projected from retained locks or finalization evidence.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct ValidationFeeProposalTallyV1 {
    /// Aye weight.
    pub approve: String,
    /// Nay weight.
    pub reject: String,
    /// Abstain weight.
    pub abstain: String,
    /// Complete turnout.
    pub turnout: String,
    /// Configured minimum turnout.
    pub min_turnout: String,
    /// Approval fraction numerator.
    pub approval_threshold_numerator: String,
    /// Approval fraction denominator.
    pub approval_threshold_denominator: String,
    /// Final decision, or `None` while the referendum is not finalized.
    pub approved: Option<bool>,
}

/// JSON-safe retained citizen locks keyed by canonical voter account.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct ValidationFeeProposalLocksV1 {
    /// Current voter lock records.
    pub locks: std::collections::BTreeMap<AccountId, ValidationFeeProposalLockV1>,
}

/// One exact retained citizen ballot lock.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct ValidationFeeProposalLockV1 {
    /// Canonical voter account.
    pub owner: AccountId,
    /// Exact locked quantity.
    pub amount: String,
    /// Exact accumulated slash amount.
    pub slashed: String,
    /// Inclusive lock expiry height.
    pub expiry_height: String,
    /// Canonical ballot direction (`Aye`, `Nay`, or `Abstain`).
    pub direction: String,
    /// Requested conviction duration in blocks.
    pub duration_blocks: String,
}

/// Exact native validation-fee payload requested from the draft endpoint.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(tag = "kind", content = "payload", rename_all = "SCREAMING_SNAKE_CASE")]
#[expect(
    clippy::large_enum_variant,
    reason = "boxing one payload would change the canonical public V1 enum construction and wire shape"
)]
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

fn require_canonical_iroha_hash(label: &str, value: &[u8; 32]) -> Result<(), String> {
    if value[31] & 1 == 0 {
        return Err(format!(
            "{label} must carry the canonical Iroha hash marker"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::validation_fee::{
        ValidationFeeFinalizationEvidenceV1, ValidationFeeGovernanceWindowV1,
    };

    #[test]
    fn iroha_hash_validation_requires_the_canonical_marker() {
        require_canonical_iroha_hash("checkpoint", &[0x03; 32])
            .expect("odd-ending Iroha hash is canonical");
        assert_eq!(
            require_canonical_iroha_hash("checkpoint", &[0x02; 32]),
            Err("checkpoint must carry the canonical Iroha hash marker".to_owned())
        );
        assert_eq!(
            require_canonical_iroha_hash("checkpoint", &[0; 32]),
            Err("checkpoint must carry the canonical Iroha hash marker".to_owned())
        );
    }

    #[test]
    fn verified_parliament_projection_preserves_full_range_integers_as_strings() {
        let proposal_id = [0x02; 32];
        let authorization = ValidationFeeParliamentAuthorizationV1 {
            proposal_id,
            proposal_fingerprint: proposal_id,
            proposal_time_roster_root: [0x04; 32],
            referendum_window: ValidationFeeGovernanceWindowV1 {
                lower: 1,
                upper: u64::MAX,
            },
            finalization: ValidationFeeFinalizationEvidenceV1 {
                referendum_id: proposal_id,
                finalized_at_height: u64::MAX - 1,
                mode: ValidationFeeGovernanceVotingModeV1::Plain,
                approve: u128::MAX,
                reject: 0,
                abstain: 0,
                min_turnout: u128::MAX,
                approval_threshold_numerator: u64::MAX,
                approval_threshold_denominator: u64::MAX,
                approved: true,
            },
            enacted_at_height: u64::MAX,
        };
        let projected = verified_parliament_proposal("ValidationFeePolicyV1", &authorization)
            .expect("project verified Parliament authorization");
        assert_eq!(projected.proposal_id, projected.payload_hash);
        assert_eq!(projected.finalization.approve, u128::MAX.to_string());
        assert_eq!(
            projected.finalization.approval_threshold_numerator,
            u64::MAX.to_string()
        );
        assert_eq!(
            projected.enactment_window.enacted_at_height,
            u64::MAX.to_string()
        );

        let json = norito::json::to_string(&projected).expect("serialize JSON-safe projection");
        assert!(json.contains(r#""proposal_kind":"ValidationFeePolicyV1""#));
        assert!(json.contains(&format!(r#""approve":"{}""#, u128::MAX)));
        assert!(json.contains(&format!(r#""approval_threshold_numerator":"{}""#, u64::MAX)));
        assert!(!json.contains("validator_id"));
        let roundtrip: ValidationFeeVerifiedParliamentProposalV1 =
            norito::json::from_str(&json).expect("roundtrip JSON-safe projection");
        assert_eq!(roundtrip, projected);
    }

    #[test]
    fn verified_current_policy_shape_has_exact_mobile_keys_and_recipient_evidence() {
        let proposal = ValidationFeeVerifiedParliamentProposalV1 {
            proposal_kind: "ValidationFeePolicyV1".to_owned(),
            proposal_id: "02".repeat(32),
            payload_hash: "02".repeat(32),
            parliament_roster_root: "04".repeat(32),
            enactment_window: ValidationFeeVerifiedEnactmentWindowV1 {
                opens_at_height: "1".to_owned(),
                closes_at_height: "2".to_owned(),
                enacted_at_height: "2".to_owned(),
            },
            finalization: ValidationFeeVerifiedFinalizationV1 {
                proposal_id: "02".repeat(32),
                referendum_id: "02".repeat(32),
                finalized_at_height: "2".to_owned(),
                mode: "PLAIN".to_owned(),
                approve: u128::MAX.to_string(),
                reject: "0".to_owned(),
                abstain: "0".to_owned(),
                min_turnout: u128::MAX.to_string(),
                approval_threshold_numerator: "1".to_owned(),
                approval_threshold_denominator: "2".to_owned(),
                approved: true,
            },
        };
        let current = ValidationFeeVerifiedCurrentPolicyV1 {
            active_policy_version: "1".to_owned(),
            active_policy_hash: "03".repeat(32),
            fee_asset_definition_id: "asset".to_owned(),
            fee_scale: 2,
            fee_minor_units: "10".to_owned(),
            charging_mode: "PER_QUALIFYING_TRANSFER_INSTRUCTION".to_owned(),
            effective_from_height: "120961".to_owned(),
            expires_after_height: None,
            parliament: ValidationFeeVerifiedParliamentV1 {
                validation_fee_policy: proposal.clone(),
                payout_lifecycle: ValidationFeeVerifiedParliamentProposalV1 {
                    proposal_kind: "ValidationFeePayoutLifecycleV1".to_owned(),
                    ..proposal
                },
                payout_lifecycle_seal_hash: "05".repeat(32),
            },
            payout: ValidationFeeVerifiedPayoutV1 {
                contract_address: "contract".to_owned(),
                code_hash: "02".repeat(32),
                entrypoint: "autonomous_validation_fee_tick".to_owned(),
                sbd_asset_definition_id: "asset".to_owned(),
                xor_asset_definition_id: "xor".to_owned(),
                treasury_account_id: "treasury".to_owned(),
                vault_account_id: "vault".to_owned(),
                batch_sbd_minor_units: "1000".to_owned(),
                sbd_scale: 2,
                xor_output_min: "4".to_owned(),
                xor_output_max: "100".to_owned(),
                recipients: vec![ValidationFeeVerifiedPayoutRecipientV1 {
                    account_id: "recipient".to_owned(),
                    share_basis_points: 2_500,
                }],
            },
        };
        let json = norito::json::to_string(&current).expect("serialize current policy");
        for key in [
            "activePolicyVersion",
            "activePolicyHash",
            "feeAssetDefinitionId",
            "feeScale",
            "feeMinorUnits",
            "chargingMode",
            "effectiveFromHeight",
            "expiresAfterHeight",
            "validationFeePolicy",
            "payoutLifecycle",
            "payoutLifecycleSealHash",
            "contractAddress",
            "batchSbdMinorUnits",
            "account_id",
            "share_basis_points",
        ] {
            assert!(json.contains(&format!(r#""{key}":"#)), "missing {key}");
        }
        assert!(!json.contains("validator_id"));
        assert!(!json.contains(r#""policy":"#));
        let roundtrip: ValidationFeeVerifiedCurrentPolicyV1 =
            norito::json::from_str(&json).expect("roundtrip current policy");
        assert_eq!(roundtrip, current);
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

    #[test]
    fn governance_windows_preserve_full_u64_decimal_strings() {
        let window: AtWindow = norito::json::from_str(
            r#"{"lower":"9007199254740993","upper":"18446744073709551615"}"#,
        )
        .expect("parse exact heights beyond the JavaScript safe-integer range");
        assert_eq!(window.lower, 9_007_199_254_740_993);
        assert_eq!(window.upper, u64::MAX);
        assert_eq!(
            norito::json::to_json(&window).expect("serialize exact heights"),
            r#"{"lower":"9007199254740993","upper":"18446744073709551615"}"#
        );

        for invalid in [
            r#"{"lower":1,"upper":"2"}"#,
            r#"{"lower":"01","upper":"2"}"#,
            r#"{"lower":"+1","upper":"2"}"#,
            r#"{"lower":"1.0","upper":"2"}"#,
            r#"{"lower":"18446744073709551616","upper":"2"}"#,
        ] {
            assert!(
                norito::json::from_str::<AtWindow>(invalid).is_err(),
                "non-canonical governance integer must be rejected: {invalid}",
            );
        }
    }
}
