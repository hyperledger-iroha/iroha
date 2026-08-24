//! Public Torii DTOs for Parliament-governed validation-fee policy state.
use iroha_crypto::Hash;
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    block::consensus_v2::{HeightContextId, finality::V2FinalityArtifact},
    bridge::{BridgeFinalityProof, BridgeFinalityVerifier},
    governance::types::{
        GovernanceCertificateV1, ProposalKind, ValidationFeePayoutLifecycleProposal,
        ValidationFeePolicyProposal,
    },
    validation_fee::{
        VALIDATION_FEE_POLICY_ACTIVATION_DELAY_BLOCKS, ValidationFeeChargingMode,
        ValidationFeeParliamentAuthorizationV1, ValidationFeePolicyRegistryEntryV1,
        ValidationFeePolicyRegistryV1, ValidationFeePolicySnapshotStatusV1,
        ValidationFeePolicyV1, ValidationFeePolicyWitnessProofV1,
        ValidationFeeTreasuryPayoutBindingV1,
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
/// Default number of validation-fee proposals returned by one list request.
pub const VALIDATION_FEE_PROPOSAL_PAGE_DEFAULT_LIMIT_V1: u32 = 50;
/// Hard maximum number of validation-fee proposals returned by one list request.
pub const VALIDATION_FEE_PROPOSAL_PAGE_MAX_LIMIT_V1: u32 = 100;
/// Maximum encoded length of a validation-fee proposal continuation cursor.
pub const VALIDATION_FEE_PROPOSAL_CURSOR_MAX_ENCODED_LEN_V1: usize = 96;
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
    /// Canonical exact genesis-derived network identity proven by finality and every policy.
    pub network_id: String,
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
    /// Canonical transaction authority bound into the proposal preimage.
    pub proposal_operator: AccountId,
    /// Raw native proposal fingerprint in canonical lowercase hexadecimal.
    pub proposal_id: String,
    /// Fingerprint of the exact typed proposal preimage.
    pub payload_hash: String,
    /// Canonical identifier of the complete Parliament certificate.
    pub governance_certificate_id: String,
    /// Complete certificate required for independent structural validation.
    pub governance_certificate: GovernanceCertificateV1,
    /// Height at which the complete certificate was finalized.
    pub certified_at_height: String,
    /// Exact certified height at which enactment was due and occurred.
    pub enacted_at_height: String,
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
    /// Canonical public DS asset-definition address.
    #[norito(rename = "dsAssetDefinitionId")]
    pub ds_asset_definition_id: String,
    /// Canonical public XOR asset-definition address.
    #[norito(rename = "xorAssetDefinitionId")]
    pub xor_asset_definition_id: String,
    /// Canonical non-signable contract treasury account.
    #[norito(rename = "treasuryAccountId")]
    pub treasury_account_id: String,
    /// Canonical pool vault account.
    #[norito(rename = "vaultAccountId")]
    pub vault_account_id: String,
    /// Exact DS payout batch in fee-asset minor units.
    #[norito(rename = "batchDsMinorUnits")]
    pub batch_ds_minor_units: String,
    /// DS asset scale.
    #[norito(rename = "dsScale")]
    pub ds_scale: u8,
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
    if let Some(reason) = authorization.invariant_error() {
        return Err(format!(
            "validation-fee Parliament authorization is invalid: {reason}"
        ));
    }
    let proposal_id = hex::encode(authorization.proposal_fingerprint);
    Ok(ValidationFeeVerifiedParliamentProposalV1 {
        proposal_kind: proposal_kind.to_owned(),
        proposal_operator: authorization.proposal_operator.clone(),
        proposal_id: proposal_id.clone(),
        payload_hash: hex::encode(authorization.proposal_fingerprint),
        governance_certificate_id: hex::encode(authorization.governance_certificate_id.as_bytes()),
        governance_certificate: authorization.governance_certificate.clone(),
        certified_at_height: authorization
            .governance_certificate
            .certified_at_height
            .to_string(),
        enacted_at_height: authorization.enacted_at_height.to_string(),
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
    if entry
        .parliament_authorization
        .enacted_at_height
        .checked_add(VALIDATION_FEE_POLICY_ACTIVATION_DELAY_BLOCKS)
        != Some(entry.policy.effective_from_height)
    {
        return Err("validation-fee policy activation delay is invalid".into());
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
        // The protected enabled policy invariant fixes exactly 0.10 DS at scale two.
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
            ds_asset_definition_id: payout.ds_asset_id.to_string(),
            xor_asset_definition_id: payout.xor_asset_id.to_string(),
            treasury_account_id: payout.treasury_account_id.to_string(),
            vault_account_id: payout.pool_vault_account_id.to_string(),
            // The protected lifecycle invariant fixes exactly 10 DS at scale two.
            batch_ds_minor_units: "1000".to_owned(),
            ds_scale: entry.policy.ds_scale,
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
    pub fn verify_against(
        &self,
        network_id: NetworkId,
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
        require_canonical_iroha_hash("validation-fee network id", network_id.as_bytes())?;
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
        // The exact caller-pinned context authenticates its complete HeightContext. It must still
        // agree with the independently configured NetworkId before any finality proof is accepted.
        let trusted_network_id = first.finality_artifact.height_context.network_id;
        if trusted_network_id != network_id {
            return Err("validation-fee finality chain targets a different network".into());
        }
        let mut verifier = BridgeFinalityVerifier::with_context(network_id, trusted_context);
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
                    .any(|entry| entry.policy.network_id != network_id)
                {
                    return Err("validation-fee registry targets a different network".into());
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
    /// This is deliberately stricter than [`Self::verify_against`]: an unconfigured registry is
    /// rejected because it cannot authenticate the caller-pinned policy-chain genesis hash.
    ///
    /// # Errors
    ///
    /// Returns an error for any proof failure, absent registry, deployment
    /// network mismatch, or policy-chain genesis mismatch.
    pub fn verify_with_immutable_binding(
        &self,
        network_id: NetworkId,
        policy_chain_genesis_hash: [u8; 32],
        trusted_checkpoint_height: u64,
        trusted_checkpoint_context_id: [u8; 32],
    ) -> Result<ValidationFeeVerifiedPolicyProjectionV1, String> {
        self.verify_against(
            network_id,
            trusted_checkpoint_height,
            trusted_checkpoint_context_id,
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
            network_id: network_id.to_string(),
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
    /// Parliament processing is still in progress.
    Proposed,
    /// Parliament certified approval.
    Approved,
    /// Parliament reached a terminal rejection.
    Rejected,
    /// The proposal payload was enacted.
    Enacted,
    /// A concurrently enacted successor made this policy predecessor stale.
    Superseded,
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
    /// Canonical certificate identifier after successful certification.
    pub governance_certificate_id: Option<String>,
    /// Height at which the successful certificate was finalized.
    pub certified_at_height: Option<String>,
    /// Exact certified enactment-due height.
    pub enact_at_height: Option<String>,
    /// Height at which enactment completed.
    pub enacted_at_height: Option<String>,
}
fn validation_fee_proposal_default_page_limit() -> u32 {
    VALIDATION_FEE_PROPOSAL_PAGE_DEFAULT_LIMIT_V1
}
/// Bounded query for one canonical validation-fee proposal page.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct ValidationFeeProposalListQueryV1 {
    /// Opaque continuation token returned by the preceding page.
    #[norito(default)]
    pub cursor: Option<String>,
    /// Requested record limit. Values outside `1..=100` are rejected.
    #[norito(default = "validation_fee_proposal_default_page_limit")]
    pub limit: u32,
}
impl Default for ValidationFeeProposalListQueryV1 {
    fn default() -> Self {
        Self {
            cursor: None,
            limit: VALIDATION_FEE_PROPOSAL_PAGE_DEFAULT_LIMIT_V1,
        }
    }
}
const VALIDATION_FEE_PROPOSAL_CURSOR_MAGIC_V1: [u8; 8] = *b"vfprop01";
const VALIDATION_FEE_PROPOSAL_CURSOR_BYTES_V1: usize = 8 + 8 + 32;
/// Encode one proposal-order key as a collection-bound canonical cursor.
#[must_use]
pub fn encode_validation_fee_proposal_cursor_v1(
    created_height: u64,
    proposal_id: [u8; 32],
) -> String {
    let mut frame = [0_u8; VALIDATION_FEE_PROPOSAL_CURSOR_BYTES_V1];
    frame[..8].copy_from_slice(&VALIDATION_FEE_PROPOSAL_CURSOR_MAGIC_V1);
    frame[8..16].copy_from_slice(&created_height.to_be_bytes());
    frame[16..].copy_from_slice(&proposal_id);
    hex::encode(frame)
}
/// Decode and validate one canonical validation-fee proposal cursor.
///
/// # Errors
///
/// Returns a stable validation message when the cursor is oversized, non-canonical, belongs to
/// another collection/version, or has the wrong fixed-width payload.
pub fn decode_validation_fee_proposal_cursor_v1(encoded: &str) -> Result<(u64, [u8; 32]), String> {
    if encoded.is_empty() || encoded.len() > VALIDATION_FEE_PROPOSAL_CURSOR_MAX_ENCODED_LEN_V1 {
        return Err(
            "cursor must be a non-empty canonical lowercase-hex token within 96 bytes".into(),
        );
    }
    if !encoded
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("cursor must use canonical lowercase hexadecimal".to_owned());
    }
    let frame = hex::decode(encoded)
        .map_err(|_| "cursor must use canonical lowercase hexadecimal".to_owned())?;
    if frame.len() != VALIDATION_FEE_PROPOSAL_CURSOR_BYTES_V1 || hex::encode(&frame) != encoded {
        return Err("cursor has a non-canonical or invalid fixed-width frame".into());
    }
    if frame[..8] != VALIDATION_FEE_PROPOSAL_CURSOR_MAGIC_V1 {
        return Err("cursor belongs to another collection or API version".into());
    }
    let created_height = u64::from_be_bytes(
        frame[8..16]
            .try_into()
            .expect("validated cursor height has fixed width"),
    );
    let proposal_id = frame[16..]
        .try_into()
        .expect("validated proposal id has fixed width");
    Ok((created_height, proposal_id))
}
/// Canonically ordered bounded page of validation-fee proposals.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct ValidationFeeProposalListV1 {
    /// Response layout version.
    pub version: u16,
    /// Effective bounded page size requested by the caller.
    pub limit: u32,
    /// Records ordered by creation height then proposal id.
    pub proposals: Vec<ValidationFeeProposalRecordV1>,
    /// Opaque continuation token, or `None` after the final page.
    pub next_cursor: Option<String>,
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
    /// Complete successful Parliament certificate, when certified.
    pub governance_certificate: Option<GovernanceCertificateV1>,
}
/// Strict empty query for one validation-fee proposal detail projection.
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
pub struct ValidationFeeProposalDetailQueryV1 {}
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
    pub fn proposal_kind(&self, proposal_operator: &AccountId) -> ProposalKind {
        match self {
            Self::Policy {
                policy,
                payout_lifecycle_proposal_id,
            } => ProposalKind::ValidationFeePolicy(ValidationFeePolicyProposal {
                proposal_operator: proposal_operator.clone(),
                policy: policy.clone(),
                payout_lifecycle_proposal_id: *payout_lifecycle_proposal_id,
            }),
            Self::PayoutLifecycle { payout_binding } => {
                ProposalKind::ValidationFeePayoutLifecycle(ValidationFeePayoutLifecycleProposal {
                    proposal_operator: proposal_operator.clone(),
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
    /// Canonical transaction authority that will execute the drafted instruction.
    ///
    /// The signed transaction must use this exact authority or Core will derive
    /// a different operator-bound proposal fingerprint.
    pub proposal_operator: AccountId,
    /// Exact proposal payload.
    pub proposal: ValidationFeeProposalDraftPayloadV1,
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
    /// Canonical transaction authority bound into `proposal_kind` and `proposal_id`.
    pub proposal_operator: AccountId,
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
    use iroha_data_model::governance::types::{
        BallotAttemptId, BeaconPulseId, BeaconSessionId, BodyElectionAttemptId, BodyInstanceId,
        GovernanceAttemptId, GovernanceCertificateId, GovernanceExpectedHeadPresentV1,
        GovernanceExpectedHeadV1, ParliamentAggregateOutcomeV1, ParliamentAggregateTallyV1,
        ParliamentBallotCertificateBindingV1, ParliamentBody,
        ParliamentBodyCertificateBindingV1, ProposalContentId, RiskTierV1, SortitionRequestV1,
        TleKeySessionId, TleSessionId,
    };
    fn fixture_account(seed: u8) -> AccountId {
        let key_pair =
            iroha_crypto::KeyPair::try_from_seed(vec![seed; 32], iroha_crypto::Algorithm::Ed25519)
                .expect("derive deterministic validation-fee fixture account");
        AccountId::new(key_pair.public_key().clone())
    }
    fn parliament_authorization(
        proposal_fingerprint: [u8; 32],
        enacted_at_height: u64,
    ) -> ValidationFeeParliamentAuthorizationV1 {
        let base = enacted_at_height.checked_sub(7).expect("certificate lifecycle");
        let root = |marker: u8| [marker; 32];
        let proposal_content_id = ProposalContentId::new(proposal_fingerprint);
        let governance_attempt_sequence = 0;
        let governance_attempt_id =
            GovernanceAttemptId::derive_v1(proposal_content_id, governance_attempt_sequence);
        let election_attempt_sequence = 0;
        let election_attempt_id = BodyElectionAttemptId::derive_v1(
            governance_attempt_id,
            ParliamentBody::PolicyJury,
            election_attempt_sequence,
        );
        let beacon_session_id = BeaconSessionId::new(root(2));
        let sortition_request = SortitionRequestV1::try_new_canonical(
            governance_attempt_id,
            election_attempt_id,
            ParliamentBody::PolicyJury,
            root(1),
            500,
            500,
            base + 1,
            base + 2,
            beacon_session_id,
            None,
        )
        .expect("canonical Policy Jury request");
        let roster_root = root(4);
        let body_instance_id = BodyInstanceId::derive_v1(election_attempt_id, roster_root);
        let ballot_attempt_sequence = 0;
        let ballot_attempt_id =
            BallotAttemptId::derive_v1(body_instance_id, ballot_attempt_sequence);
        let release_beacon_session_id = BeaconSessionId::new(root(7));
        let tle_key_session_id = TleKeySessionId::new(root(8));
        let release_height = base + 4;
        let tle_session_id = TleSessionId::derive_v1(
            ballot_attempt_id,
            tle_key_session_id,
            release_beacon_session_id,
            release_height,
        );
        let governance_certificate = GovernanceCertificateV1 {
            proposal_content_id,
            governance_attempt_id,
            governance_attempt_sequence,
            risk_tier: RiskTierV1::Standard,
            body_bindings: vec![ParliamentBodyCertificateBindingV1 {
                body_instance_id,
                election_attempt_id,
                election_attempt_sequence,
                sortition_request_id: sortition_request.id,
                sortition_request,
                body: ParliamentBody::PolicyJury,
                beacon_session_id,
                beacon_pulse_id: BeaconPulseId::new(root(3)),
                roster_root,
                assignment_root: root(5),
                result_root: root(6),
                result_height: base + 5,
                ballot: Some(ParliamentBallotCertificateBindingV1 {
                    ballot_attempt_id,
                    ballot_attempt_sequence,
                    tle_session_id,
                    tle_key_session_id,
                    registration_root: root(9),
                    dropout_root: root(10),
                    survivor_root: root(11),
                    corpus_root: root(12),
                    no_recovery_root: root(13),
                    timed_commitment_root: root(14),
                    release_beacon_session_id,
                    registered_at_height: base + 3,
                    release_height,
                    release_pulse_id: BeaconPulseId::new(root(15)),
                    opening_height: release_height,
                    opening_root: root(16),
                    tally: ParliamentAggregateTallyV1 {
                        original_seats: 500,
                        accepted_ballots: 334,
                        aye: 200,
                        nay: 100,
                        abstain: 34,
                    },
                    outcome: ParliamentAggregateOutcomeV1::Approved,
                }),
            }],
            policy_version: 1,
            effect_preimage_hash: root(19),
            expected_head: GovernanceExpectedHeadV1::Present(
                GovernanceExpectedHeadPresentV1 {
                    subject_id: root(17),
                    version: 1,
                    head_root: root(18),
                },
            ),
            certified_at_height: base + 6,
            enact_at_height: enacted_at_height,
        };
        let governance_certificate_id = GovernanceCertificateId::derive_v1(&governance_certificate);
        ValidationFeeParliamentAuthorizationV1 {
            proposal_operator: fixture_account(3),
            proposal_fingerprint,
            governance_certificate_id,
            governance_certificate,
            enacted_at_height,
        }
    }
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
    fn verified_parliament_projection_retains_full_canonical_certificate() {
        let proposal_id = [0x02; 32];
        let authorization = parliament_authorization(proposal_id, u64::MAX);
        let projected = verified_parliament_proposal("ValidationFeePolicyV1", &authorization)
            .expect("project verified Parliament authorization");
        assert_eq!(projected.proposal_id, projected.payload_hash);
        assert_eq!(
            projected.governance_certificate_id,
            hex::encode(authorization.governance_certificate_id.as_bytes())
        );
        assert_eq!(
            projected.governance_certificate,
            authorization.governance_certificate
        );
        assert_eq!(projected.enacted_at_height, u64::MAX.to_string());
        let json = norito::json::to_string(&projected).expect("serialize JSON-safe projection");
        assert!(json.contains(r#""proposal_kind":"ValidationFeePolicyV1""#));
        assert!(json.contains(r#""governance_certificate_id":"#));
        assert!(json.contains(r#""governance_certificate":"#));
        assert!(!json.contains("plainElectorate"));
        assert!(!json.contains("referendum"));
        let roundtrip: ValidationFeeVerifiedParliamentProposalV1 =
            norito::json::from_str(&json).expect("roundtrip JSON-safe projection");
        assert_eq!(roundtrip, projected);
    }
    #[test]
    fn verified_parliament_projection_rejects_noncanonical_certificate_identity() {
        let mut authorization = parliament_authorization([0x02; 32], 107);
        verified_parliament_proposal("ValidationFeePolicyV1", &authorization)
            .expect("canonical certificate authorization");
        authorization.governance_certificate_id = GovernanceCertificateId::new([0xAA; 32]);
        assert!(
            verified_parliament_proposal("ValidationFeePolicyV1", &authorization).is_err(),
            "a certificate identifier not derived from the retained certificate must reject"
        );
    }
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "the focused schema test keeps every required and retired mobile policy key auditable together"
    )]
    fn verified_current_policy_shape_has_exact_mobile_keys_and_recipient_evidence() {
        let authorization = parliament_authorization([0x02; 32], 107);
        let proposal = verified_parliament_proposal("ValidationFeePolicyV1", &authorization)
            .expect("project canonical certificate authorization");
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
                ds_asset_definition_id: "asset".to_owned(),
                xor_asset_definition_id: "xor".to_owned(),
                treasury_account_id: "treasury".to_owned(),
                vault_account_id: "vault".to_owned(),
                batch_ds_minor_units: "1000".to_owned(),
                ds_scale: 2,
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
            "governance_certificate_id",
            "governance_certificate",
            "certified_at_height",
            "enacted_at_height",
            "contractAddress",
            "dsAssetDefinitionId",
            "batchDsMinorUnits",
            "dsScale",
            "account_id",
            "share_basis_points",
        ] {
            assert!(json.contains(&format!(r#""{key}":"#)), "missing {key}");
        }
        for retired_key in ["sbdAssetDefinitionId", "batchSbdMinorUnits", "sbdScale"] {
            assert!(
                !json.contains(retired_key),
                "retired first-release key leaked: {retired_key}"
            );
        }
        assert!(!json.contains("validator_id"));
        assert!(!json.contains("plainElectorate"));
        assert!(!json.contains("referendum"));
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
    fn proposal_cursor_roundtrip_is_canonical_and_collection_bound() {
        let proposal_id = [0xA5; 32];
        let encoded = encode_validation_fee_proposal_cursor_v1(42, proposal_id);
        assert_eq!(
            encoded.len(),
            VALIDATION_FEE_PROPOSAL_CURSOR_MAX_ENCODED_LEN_V1
        );
        assert_eq!(
            decode_validation_fee_proposal_cursor_v1(&encoded),
            Ok((42, proposal_id))
        );
        let mut wrong_collection = hex::decode(&encoded).expect("decode valid cursor fixture");
        wrong_collection[0] ^= 0x01;
        let wrong_collection = hex::encode(wrong_collection);
        assert!(
            decode_validation_fee_proposal_cursor_v1(&wrong_collection)
                .expect_err("collection marker mismatch must fail")
                .contains("another collection")
        );
        assert!(decode_validation_fee_proposal_cursor_v1(&encoded.to_uppercase()).is_err());
        assert!(decode_validation_fee_proposal_cursor_v1("").is_err());
    }
    #[test]
    fn proposal_list_query_defaults_to_a_bounded_page() {
        let query: ValidationFeeProposalListQueryV1 =
            norito::json::from_str("{}").expect("decode default proposal page query");
        assert_eq!(query, ValidationFeeProposalListQueryV1::default());
        assert_eq!(query.limit, VALIDATION_FEE_PROPOSAL_PAGE_DEFAULT_LIMIT_V1);
    }
}
