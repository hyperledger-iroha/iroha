//! Public Torii DTOs for Parliament-governed validation-fee policy state.
use iroha_crypto::Hash;
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    asset::AssetDefinitionId,
    block::consensus_v2::{HeightContextId, finality::V2FinalityArtifact},
    bridge::{BridgeFinalityProof, BridgeFinalityVerifier},
    governance::types::{
        GovernanceCertificateV1, ProposalKind, ValidationFeePayoutLifecycleProposal,
        ValidationFeePolicyProposal,
    },
    hijiri::{
        HIJIRI_PARAMETERS_VERSION_V1, HijiriAccountRiskV1, HijiriParametersV1, Q16,
        hijiri_fee_quote_hash_from_digests_v1,
    },
    validation_fee::{
        VALIDATION_FEE_DS_SCALE, VALIDATION_FEE_POLICY_ACTIVATION_DELAY_BLOCKS,
        ValidationFeeChargingMode, ValidationFeeParliamentAuthorizationV1,
        ValidationFeePolicyRegistryEntryV1, ValidationFeePolicyRegistryV1,
        ValidationFeePolicySnapshotStatusV1, ValidationFeePolicyV1,
        ValidationFeePolicyWitnessProofV1, ValidationFeeTreasuryPayoutBindingV1,
    },
};
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};
/// Current validation-fee proof request/response layout.
pub const VALIDATION_FEE_POLICY_PROOF_VERSION_V1: u16 = 1;
/// Current Hijiri validation-fee quote request/response layout.
pub const VALIDATION_FEE_HIJIRI_QUOTE_VERSION_V1: u16 = 1;
/// Stable public Norito schema name for the proof request.
pub const VALIDATION_FEE_POLICY_PROOF_REQUEST_SCHEMA_NAME: &str =
    "iroha.torii.v1.validation_fee.current_policy_proof.request";
/// Stable public Norito schema name for the proof response.
pub const VALIDATION_FEE_POLICY_PROOF_RESPONSE_SCHEMA_NAME: &str =
    "iroha.torii.v1.validation_fee.current_policy_proof.response";
/// Stable public JSON schema name for a locally verified policy projection.
pub const VALIDATION_FEE_VERIFIED_POLICY_PROJECTION_SCHEMA_NAME: &str =
    "iroha.validation_fee.verified_policy_projection.v1";
/// Stable public JSON schema name for an evaluated Hijiri fee quote.
pub const VALIDATION_FEE_HIJIRI_QUOTE_PROJECTION_SCHEMA_NAME: &str =
    "iroha.torii.v1.validation_fee.hijiri_quote.response";
/// Stable public Norito schema name for a Hijiri fee-quote request.
pub const VALIDATION_FEE_HIJIRI_QUOTE_REQUEST_SCHEMA_NAME: &str =
    "iroha.torii.v1.validation_fee.hijiri_quote.request";
/// Honest assurance label for a complete quote without an independent state witness.
pub const VALIDATION_FEE_HIJIRI_QUOTE_EVALUATED_ASSURANCE_V1: &str =
    "EVALUATED_PROJECTION_NOT_INDEPENDENTLY_WITNESS_VERIFIED";
/// Maximum qualifying transfers accepted by one V1 quote request.
pub const VALIDATION_FEE_HIJIRI_QUOTE_MAX_QUALIFYING_TRANSFERS_V1: u32 = 100_000;
/// Maximum canonical request bytes accepted by the live V1 quote route.
pub const VALIDATION_FEE_HIJIRI_QUOTE_MAX_REQUEST_BYTES_V1: usize = 4 * 1024;
/// Maximum response bytes a V1 client should accept from the live quote route.
pub const VALIDATION_FEE_HIJIRI_QUOTE_MAX_RESPONSE_BYTES_V1: usize = 64 * 1024;
/// Protected V1 validation fee expressed in fee-asset minor units.
pub const VALIDATION_FEE_BASE_MINOR_UNITS_V1: u64 = 10;
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
/// Request an evaluated current-state validation-fee quote for one account.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct ValidationFeeHijiriQuoteRequestV1 {
    /// Request layout version.
    pub version: u16,
    /// Canonical universal account whose effective Hijiri risk is priced. Torii permits the
    /// authenticated account itself or a live multisig controller for which it is a direct
    /// signatory.
    #[norito(rename = "accountId")]
    pub account_id: AccountId,
    /// Number of qualifying transfers priced as one aggregate before a single ceiling operation.
    #[norito(rename = "qualifyingTransferCount")]
    pub qualifying_transfer_count: u32,
}
impl ValidationFeeHijiriQuoteRequestV1 {
    /// Validate the request version and bounded nonzero transfer count.
    ///
    /// # Errors
    ///
    /// Returns a stable explanation for an unsupported version, zero count, or count above the V1
    /// quote bound.
    pub fn validate(&self) -> Result<(), String> {
        if self.version != VALIDATION_FEE_HIJIRI_QUOTE_VERSION_V1 {
            return Err("unsupported Hijiri validation-fee quote version".to_owned());
        }
        validate_hijiri_quote_transfer_count(self.qualifying_transfer_count)
    }
}
fn validate_hijiri_quote_transfer_count(qualifying_transfer_count: u32) -> Result<(), String> {
    if qualifying_transfer_count == 0
        || qualifying_transfer_count > VALIDATION_FEE_HIJIRI_QUOTE_MAX_QUALIFYING_TRANSFERS_V1
    {
        return Err(format!(
            "qualifying_transfer_count must be between 1 and {}",
            VALIDATION_FEE_HIJIRI_QUOTE_MAX_QUALIFYING_TRANSFERS_V1
        ));
    }
    Ok(())
}
/// Return the checked 1-based execution height immediately after a committed state tip.
///
/// # Errors
///
/// Returns an error when the committed state height cannot be incremented.
pub fn validation_fee_hijiri_quote_execution_height(
    evaluated_state_height: u64,
) -> Result<u64, String> {
    evaluated_state_height.checked_add(1).ok_or_else(|| {
        "committed state height cannot advance to a Hijiri quote execution height".to_owned()
    })
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
/// Exact fee quote evaluated from one same-snapshot base policy and Hijiri state.
///
/// The live route does not carry a witness for either state input, and the historical
/// validation-fee witness does not commit custom Hijiri parameters. Consequently this DTO's
/// assurance label covers the complete projection. A client using
/// [`ValidationFeeVerifiedPolicyProjectionV1`] can independently verify the base-policy portion,
/// but must still authenticate the Hijiri records through a separate state/query trust path.
/// Later inclusion or intervening state changes can make a live quote stale; policy/Hijiri hash
/// admission binding rejects that transaction so the client can refresh and retry.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct ValidationFeeHijiriQuoteProjectionV1 {
    /// Stable projection schema name.
    pub schema: String,
    /// Projection layout version.
    pub version: u16,
    /// Explicitly states that this complete quote was evaluated, not independently witness-verified.
    pub assurance: String,
    /// Committed state height from which the live route read its same-snapshot inputs.
    #[norito(rename = "evaluatedStateHeight")]
    pub evaluated_state_height: String,
    /// Execution height at which the selected validation-fee policy is scheduled to apply.
    #[norito(rename = "quotedExecutionHeight")]
    pub quoted_execution_height: String,
    /// Canonical universal account being quoted.
    #[norito(rename = "accountId")]
    pub account_id: AccountId,
    /// Evaluated active validation-fee policy version.
    #[norito(rename = "activePolicyVersion")]
    pub active_policy_version: String,
    /// Evaluated active validation-fee policy hash.
    #[norito(rename = "activePolicyHash")]
    pub active_policy_hash: String,
    /// Fee asset charged by the active base policy.
    #[norito(rename = "feeAssetDefinitionId")]
    pub fee_asset_definition_id: String,
    /// Canonical treasury account that must receive the validation fee.
    #[norito(rename = "treasuryAccountId")]
    pub treasury_account_id: String,
    /// Fee-asset decimal scale.
    #[norito(rename = "feeScale")]
    pub fee_scale: u8,
    /// Active global Hijiri schema version.
    #[norito(rename = "hijiriParametersVersion")]
    pub hijiri_parameters_version: u16,
    /// Active global Hijiri revision as an exact decimal string.
    #[norito(rename = "hijiriParametersRevision")]
    pub hijiri_parameters_revision: String,
    /// Domain-separated digest of the complete active global Hijiri parameter.
    #[norito(rename = "hijiriParametersDigest")]
    pub hijiri_parameters_digest: String,
    /// Default account risk as the exact raw Q16.16 integer.
    #[norito(rename = "defaultAccountRiskQ16")]
    pub default_account_risk_q16: u32,
    /// Effective account risk as the exact raw Q16.16 integer.
    #[norito(rename = "effectiveAccountRiskQ16")]
    pub effective_account_risk_q16: u32,
    /// Explicit account-risk revision, or `None` when the global default was used.
    #[norito(rename = "accountRiskRevision")]
    pub account_risk_revision: Option<String>,
    /// Explicit account-risk digest, or `None` when the global default was used.
    #[norito(rename = "accountRiskDigest")]
    pub account_risk_digest: Option<String>,
    /// Applied multiplier as the exact raw Q16.16 integer.
    #[norito(rename = "feeMultiplierQ16")]
    pub fee_multiplier_q16: u32,
    /// Composite hash binding global policy, account identity, and explicit risk presence/value.
    #[norito(rename = "hijiriFeeQuoteHash")]
    pub hijiri_fee_quote_hash: String,
    /// Exact base fee for one qualifying transfer instruction, in minor units.
    #[norito(rename = "basePerTransferFeeMinorUnits")]
    pub base_per_transfer_fee_minor_units: String,
    /// Exact adjusted fee for one qualifying transfer instruction, in minor units.
    ///
    /// For multiple qualifying transfers, apply `feeMultiplierQ16` once to their aggregate base;
    /// multiplying this already-rounded one-transfer value can overcharge by rounding repeatedly.
    #[norito(rename = "adjustedPerTransferFeeMinorUnits")]
    pub adjusted_per_transfer_fee_minor_units: String,
    /// Echoed number of qualifying transfers priced by this quote.
    #[norito(rename = "qualifyingTransferCount")]
    pub qualifying_transfer_count: u32,
    /// Exact aggregate base before applying the Hijiri multiplier.
    #[norito(rename = "aggregateBaseFeeMinorUnits")]
    pub aggregate_base_fee_minor_units: String,
    /// Exact aggregate fee after one multiplier and ceiling operation.
    #[norito(rename = "aggregateAdjustedFeeMinorUnits")]
    pub aggregate_adjusted_fee_minor_units: String,
}
impl ValidationFeeHijiriQuoteProjectionV1 {
    /// Validate the self-contained shape and arithmetic of this quote projection.
    ///
    /// This general validator accepts both historical proof-derived projections, whose quoted
    /// execution height equals their evaluated height, and live projections evaluated for the
    /// checked successor height. Use [`Self::validate_for_request`] for a live route response.
    ///
    /// # Errors
    ///
    /// Returns an error when a schema marker, canonical decimal/hash, height relationship,
    /// optional account-risk pair, protected base-fee invariant, or Q16 fee calculation is
    /// incoherent.
    pub fn validate_coherence(&self) -> Result<(), String> {
        if self.schema != VALIDATION_FEE_HIJIRI_QUOTE_PROJECTION_SCHEMA_NAME
            || self.version != VALIDATION_FEE_HIJIRI_QUOTE_VERSION_V1
            || self.assurance != VALIDATION_FEE_HIJIRI_QUOTE_EVALUATED_ASSURANCE_V1
        {
            return Err("Hijiri quote schema, version, or assurance is invalid".to_owned());
        }

        let evaluated_state_height = exact_decimal_u64(
            "Hijiri quote evaluated state height",
            &self.evaluated_state_height,
        )?;
        let quoted_execution_height = exact_decimal_u64(
            "Hijiri quote execution height",
            &self.quoted_execution_height,
        )?;
        if evaluated_state_height == 0
            || quoted_execution_height == 0
            || (quoted_execution_height != evaluated_state_height
                && evaluated_state_height.checked_add(1) != Some(quoted_execution_height))
        {
            return Err(
                "Hijiri quote execution height must equal its nonzero state height or checked successor"
                    .to_owned(),
            );
        }

        let active_policy_version = exact_decimal_u64(
            "Hijiri quote active policy version",
            &self.active_policy_version,
        )?;
        if active_policy_version == 0 {
            return Err("Hijiri quote active policy version must be positive".to_owned());
        }
        let active_policy_hash =
            exact_lower_hex_32("Hijiri quote active policy hash", &self.active_policy_hash)?;
        require_canonical_iroha_hash("Hijiri quote active policy hash", &active_policy_hash)?;
        let fee_asset_definition_id = self
            .fee_asset_definition_id
            .parse::<AssetDefinitionId>()
            .map_err(|_| "Hijiri quote fee asset definition id is not canonical".to_owned())?;
        if fee_asset_definition_id.to_string() != self.fee_asset_definition_id {
            return Err("Hijiri quote fee asset definition id is not canonical".to_owned());
        }
        let treasury_account_id = AccountId::parse_encoded(&self.treasury_account_id)
            .map_err(|_| "Hijiri quote treasury account id is not canonical".to_owned())?;
        if treasury_account_id.to_string() != self.treasury_account_id {
            return Err("Hijiri quote treasury account id is not canonical".to_owned());
        }
        if self.fee_scale != VALIDATION_FEE_DS_SCALE {
            return Err("Hijiri quote fee scale differs from the protected V1 scale".to_owned());
        }

        if self.hijiri_parameters_version != HIJIRI_PARAMETERS_VERSION_V1 {
            return Err("Hijiri quote global parameter version is unsupported".to_owned());
        }
        let hijiri_revision = exact_decimal_u64(
            "Hijiri quote global parameter revision",
            &self.hijiri_parameters_revision,
        )?;
        if hijiri_revision == 0 {
            return Err("Hijiri quote global parameter revision must be positive".to_owned());
        }
        let parameters_digest = exact_lower_hex_32(
            "Hijiri quote global parameter digest",
            &self.hijiri_parameters_digest,
        )?;
        require_canonical_iroha_hash("Hijiri quote global parameter digest", &parameters_digest)?;
        if self.default_account_risk_q16 > Q16::ONE.raw()
            || self.effective_account_risk_q16 > Q16::ONE.raw()
        {
            return Err("Hijiri quote account risk exceeds Q16 one".to_owned());
        }
        let account_risk_digest = match (
            self.account_risk_revision.as_deref(),
            self.account_risk_digest.as_deref(),
        ) {
            (None, None) => {
                if self.effective_account_risk_q16 != self.default_account_risk_q16 {
                    return Err(
                        "Hijiri quote without an explicit account risk must use the global default"
                            .to_owned(),
                    );
                }
                None
            }
            (Some(revision), Some(digest)) => {
                let revision = exact_decimal_u64("Hijiri quote account-risk revision", revision)?;
                if revision == 0 {
                    return Err("Hijiri quote account-risk revision must be positive".to_owned());
                }
                let digest = exact_lower_hex_32("Hijiri quote account-risk digest", digest)?;
                require_canonical_iroha_hash("Hijiri quote account-risk digest", &digest)?;
                Some(digest)
            }
            _ => {
                return Err(
                    "Hijiri quote account-risk revision and digest must be both present or both absent"
                        .to_owned(),
                );
            }
        };
        let observed_fee_quote_hash = exact_lower_hex_32(
            "Hijiri quote composite fee hash",
            &self.hijiri_fee_quote_hash,
        )?;
        require_canonical_iroha_hash("Hijiri quote composite fee hash", &observed_fee_quote_hash)?;
        let expected_fee_quote_hash = hijiri_fee_quote_hash_from_digests_v1(
            parameters_digest,
            &self.account_id,
            account_risk_digest,
        )
        .map_err(|error| format!("Hijiri quote composite fee hash failed: {error}"))?;
        if observed_fee_quote_hash != expected_fee_quote_hash {
            return Err(
                "Hijiri quote composite fee hash does not bind its advertised inputs".to_owned(),
            );
        }

        validate_hijiri_quote_transfer_count(self.qualifying_transfer_count)?;
        let base_per_transfer = exact_decimal_u64(
            "Hijiri quote base per-transfer fee",
            &self.base_per_transfer_fee_minor_units,
        )?;
        if base_per_transfer != VALIDATION_FEE_BASE_MINOR_UNITS_V1 {
            return Err("Hijiri quote base differs from the protected V1 fee invariant".to_owned());
        }
        let aggregate_base = exact_decimal_u64(
            "Hijiri quote aggregate base fee",
            &self.aggregate_base_fee_minor_units,
        )?;
        let expected_aggregate_base = base_per_transfer
            .checked_mul(u64::from(self.qualifying_transfer_count))
            .ok_or_else(|| "Hijiri quote aggregate base fee overflows u64".to_owned())?;
        if aggregate_base != expected_aggregate_base {
            return Err("Hijiri quote aggregate base fee is incoherent".to_owned());
        }
        if self.fee_multiplier_q16 < Q16::ONE.raw() {
            return Err("Hijiri quote fee multiplier must be at least Q16 one".to_owned());
        }
        let multiplier = Q16::from_raw(self.fee_multiplier_q16);
        let expected_per_transfer = multiplier
            .checked_mul_u64_ceil(base_per_transfer)
            .ok_or_else(|| "Hijiri quote adjusted per-transfer fee overflows u64".to_owned())?;
        let adjusted_per_transfer = exact_decimal_u64(
            "Hijiri quote adjusted per-transfer fee",
            &self.adjusted_per_transfer_fee_minor_units,
        )?;
        if adjusted_per_transfer != expected_per_transfer {
            return Err("Hijiri quote adjusted per-transfer fee is incoherent".to_owned());
        }
        let expected_aggregate_adjusted = multiplier
            .checked_mul_u64_ceil(aggregate_base)
            .ok_or_else(|| "Hijiri quote adjusted aggregate fee overflows u64".to_owned())?;
        let aggregate_adjusted = exact_decimal_u64(
            "Hijiri quote adjusted aggregate fee",
            &self.aggregate_adjusted_fee_minor_units,
        )?;
        if aggregate_adjusted != expected_aggregate_adjusted {
            return Err("Hijiri quote adjusted aggregate fee is incoherent".to_owned());
        }
        Ok(())
    }

    /// Validate this projection as the exact live response to `request`.
    ///
    /// In addition to [`Self::validate_coherence`], this binds the echoed account and transfer
    /// count and requires the quoted execution height to be the checked successor of the
    /// evaluated committed-state height.
    ///
    /// # Errors
    ///
    /// Returns an error when the request is invalid, the response is incoherent, an echoed input
    /// differs, or the response uses a historical equal-height projection rather than live
    /// next-height semantics.
    pub fn validate_for_request(
        &self,
        request: &ValidationFeeHijiriQuoteRequestV1,
    ) -> Result<(), String> {
        request.validate()?;
        self.validate_coherence()?;
        if self.account_id != request.account_id
            || self.qualifying_transfer_count != request.qualifying_transfer_count
        {
            return Err(
                "Hijiri quote response does not echo the requested account and count".to_owned(),
            );
        }
        let evaluated_state_height = exact_decimal_u64(
            "Hijiri quote evaluated state height",
            &self.evaluated_state_height,
        )?;
        let quoted_execution_height = exact_decimal_u64(
            "Hijiri quote execution height",
            &self.quoted_execution_height,
        )?;
        if validation_fee_hijiri_quote_execution_height(evaluated_state_height)?
            != quoted_execution_height
        {
            return Err(
                "live Hijiri quote execution height must be the checked state successor".to_owned(),
            );
        }
        Ok(())
    }
}
/// Response returned by the live Hijiri validation-fee quote route.
pub type ValidationFeeHijiriQuoteResponseV1 = ValidationFeeHijiriQuoteProjectionV1;
/// Validated active base-policy facts used to evaluate a Hijiri quote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationFeeHijiriQuoteBaseV1 {
    evaluated_state_height: u64,
    quoted_execution_height: u64,
    active_policy_version: u64,
    active_policy_hash: [u8; 32],
    fee_asset_definition_id: String,
    treasury_account_id: String,
    fee_scale: u8,
    base_fee_minor_units: u64,
}
impl ValidationFeeHijiriQuoteBaseV1 {
    /// Construct bounded V1 base-policy facts for an evaluated quote.
    ///
    /// # Errors
    ///
    /// Returns an error for zero/non-adjacent heights, zero versions, non-canonical hashes or fee
    /// coordinates, or a base amount/scale outside the protected V1 policy invariant.
    pub fn try_new(
        evaluated_state_height: u64,
        quoted_execution_height: u64,
        active_policy_version: u64,
        active_policy_hash: [u8; 32],
        fee_asset_definition_id: String,
        treasury_account_id: String,
        fee_scale: u8,
        base_fee_minor_units: u64,
    ) -> Result<Self, String> {
        if evaluated_state_height == 0
            || quoted_execution_height == 0
            || active_policy_version == 0
            || quoted_execution_height < evaluated_state_height
            || quoted_execution_height > evaluated_state_height.saturating_add(1)
        {
            return Err(
                "Hijiri quote execution height must equal its state height or checked successor, and policy version must be positive"
                    .to_owned(),
            );
        }
        require_canonical_iroha_hash("active validation-fee policy hash", &active_policy_hash)?;
        let parsed_fee_asset_definition_id =
            fee_asset_definition_id
                .parse::<AssetDefinitionId>()
                .map_err(|_| "Hijiri quote fee asset definition id is not canonical".to_owned())?;
        if parsed_fee_asset_definition_id.to_string() != fee_asset_definition_id {
            return Err("Hijiri quote fee asset definition id is not canonical".to_owned());
        }
        let parsed_treasury_account_id = AccountId::parse_encoded(&treasury_account_id)
            .map_err(|_| "Hijiri quote treasury account id is not canonical".to_owned())?;
        if parsed_treasury_account_id.to_string() != treasury_account_id {
            return Err("Hijiri quote treasury account id is not canonical".to_owned());
        }
        if fee_scale != VALIDATION_FEE_DS_SCALE
            || base_fee_minor_units != VALIDATION_FEE_BASE_MINOR_UNITS_V1
        {
            return Err("Hijiri quote base differs from the protected V1 fee invariant".to_owned());
        }
        Ok(Self {
            evaluated_state_height,
            quoted_execution_height,
            active_policy_version,
            active_policy_hash,
            fee_asset_definition_id,
            treasury_account_id,
            fee_scale,
            base_fee_minor_units,
        })
    }
}
impl ValidationFeeVerifiedPolicyProjectionV1 {
    /// Evaluate the active validation fee for `account_id` using separately obtained Hijiri
    /// records.
    ///
    /// This method preserves the proof-verified base-policy facts in this projection, but it does
    /// not elevate the caller-supplied Hijiri records to witness-proven state. The returned
    /// [`ValidationFeeHijiriQuoteProjectionV1::assurance`] field states that limitation explicitly.
    /// `Ok(None)` means the verified projection has no enabled policy at the evaluated height.
    ///
    /// # Errors
    ///
    /// Returns an error when the verified-policy shape is non-canonical, the supplied Hijiri
    /// records are invalid or belong to another account, or adjusted minor-unit arithmetic
    /// overflows.
    pub fn evaluate_hijiri_quote(
        &self,
        account_id: &AccountId,
        parameters: &HijiriParametersV1,
        account_risk: Option<&HijiriAccountRiskV1>,
    ) -> Result<Option<ValidationFeeHijiriQuoteProjectionV1>, String> {
        self.evaluate_hijiri_quote_for_transfer_count(account_id, parameters, account_risk, 1)
    }

    /// Evaluate an exact aggregate quote using a single ceiling operation after the base fees are
    /// summed.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::evaluate_hijiri_quote`], and rejects a zero or excessive
    /// qualifying-transfer count.
    pub fn evaluate_hijiri_quote_for_transfer_count(
        &self,
        account_id: &AccountId,
        parameters: &HijiriParametersV1,
        account_risk: Option<&HijiriAccountRiskV1>,
        qualifying_transfer_count: u32,
    ) -> Result<Option<ValidationFeeHijiriQuoteProjectionV1>, String> {
        let Some(current_policy) = self.current_policy.as_ref() else {
            return Ok(None);
        };
        let base = validated_validation_fee_quote_base(self, current_policy)?;
        evaluate_hijiri_quote_v1(
            base,
            account_id,
            parameters,
            account_risk,
            qualifying_transfer_count,
        )
        .map(Some)
    }
}
fn validated_validation_fee_quote_base(
    projection: &ValidationFeeVerifiedPolicyProjectionV1,
    current_policy: &ValidationFeeVerifiedCurrentPolicyV1,
) -> Result<ValidationFeeHijiriQuoteBaseV1, String> {
    if projection.schema != VALIDATION_FEE_VERIFIED_POLICY_PROJECTION_SCHEMA_NAME
        || projection.version != VALIDATION_FEE_POLICY_PROOF_VERSION_V1
        || projection.evaluated_block_height == 0
    {
        return Err("verified validation-fee projection shape is invalid".to_owned());
    }
    let active_policy_version = exact_decimal_u64(
        "active validation-fee policy version",
        &current_policy.active_policy_version,
    )?;
    if active_policy_version == 0 {
        return Err("active validation-fee policy version must be positive".to_owned());
    }
    let active_policy_hash = exact_lower_hex_32(
        "active validation-fee policy hash",
        &current_policy.active_policy_hash,
    )?;
    require_canonical_iroha_hash("active validation-fee policy hash", &active_policy_hash)?;
    if current_policy.fee_asset_definition_id.is_empty()
        || current_policy.fee_scale != VALIDATION_FEE_DS_SCALE
        || current_policy.charging_mode != "PER_QUALIFYING_TRANSFER_INSTRUCTION"
    {
        return Err("active validation-fee quote base is invalid".to_owned());
    }
    let base_fee_minor_units = exact_decimal_u64(
        "base validation fee minor units",
        &current_policy.fee_minor_units,
    )?;
    if base_fee_minor_units != VALIDATION_FEE_BASE_MINOR_UNITS_V1 {
        return Err("active V1 validation fee must be exactly 10 minor units".to_owned());
    }
    let effective_from_height = exact_decimal_u64(
        "validation-fee effective height",
        &current_policy.effective_from_height,
    )?;
    let expires_after_height = current_policy
        .expires_after_height
        .as_deref()
        .map(|height| exact_decimal_u64("validation-fee expiry height", height))
        .transpose()?;
    if projection.evaluated_block_height < effective_from_height
        || expires_after_height.is_some_and(|height| projection.evaluated_block_height >= height)
    {
        return Err("validation-fee policy is not active at the evaluated height".to_owned());
    }
    ValidationFeeHijiriQuoteBaseV1::try_new(
        projection.evaluated_block_height,
        projection.evaluated_block_height,
        active_policy_version,
        active_policy_hash,
        current_policy.fee_asset_definition_id.clone(),
        current_policy.payout.treasury_account_id.clone(),
        current_policy.fee_scale,
        base_fee_minor_units,
    )
}
/// Evaluate one self-contained Hijiri validation-fee quote from validated base-policy facts.
///
/// The result is intentionally evaluated-only: this pure helper does not claim that either the
/// base or Hijiri inputs carry a finality witness. A Torii handler can preserve a same-snapshot
/// evaluation by constructing `base`, `parameters`, and `account_risk` from one state view.
///
/// # Errors
///
/// Returns an error for a zero or excessive transfer count, invalid/mismatched Hijiri records, or
/// overflowing aggregate/adjusted minor-unit arithmetic.
pub fn evaluate_hijiri_quote_v1(
    base: ValidationFeeHijiriQuoteBaseV1,
    account_id: &AccountId,
    parameters: &HijiriParametersV1,
    account_risk: Option<&HijiriAccountRiskV1>,
    qualifying_transfer_count: u32,
) -> Result<ValidationFeeHijiriQuoteProjectionV1, String> {
    validate_hijiri_quote_transfer_count(qualifying_transfer_count)?;
    let parameters_digest = parameters
        .digest()
        .map_err(|error| format!("global Hijiri parameter is invalid: {error}"))?;
    let account_risk_digest = account_risk
        .map(HijiriAccountRiskV1::digest)
        .transpose()
        .map_err(|error| format!("Hijiri account-risk parameter is invalid: {error}"))?;
    let effective_account_risk = parameters
        .effective_risk(account_id, account_risk)
        .map_err(|error| format!("Hijiri fee quote cannot select account risk: {error}"))?;
    let fee_multiplier = parameters
        .multiplier_for(account_id, account_risk)
        .map_err(|error| format!("Hijiri fee quote cannot select a multiplier: {error}"))?;
    let adjusted_fee_minor_units = parameters
        .apply_fee_minor_units(account_id, account_risk, base.base_fee_minor_units)
        .map_err(|error| format!("Hijiri fee quote cannot adjust the base fee: {error}"))?
        .ok_or_else(|| "Hijiri adjusted validation fee overflows u64 minor units".to_owned())?;
    let aggregate_base_fee_minor_units = base
        .base_fee_minor_units
        .checked_mul(u64::from(qualifying_transfer_count))
        .ok_or_else(|| {
            "Hijiri aggregate validation-fee base overflows u64 minor units".to_owned()
        })?;
    let aggregate_adjusted_fee_minor_units = parameters
        .apply_fee_minor_units(account_id, account_risk, aggregate_base_fee_minor_units)
        .map_err(|error| format!("Hijiri fee quote cannot adjust the aggregate base: {error}"))?
        .ok_or_else(|| "Hijiri adjusted aggregate fee overflows u64 minor units".to_owned())?;
    let hijiri_fee_quote_hash = parameters
        .fee_quote_hash(account_id, account_risk)
        .map_err(|error| format!("Hijiri fee quote hash cannot be derived: {error}"))?;
    Ok(ValidationFeeHijiriQuoteProjectionV1 {
        schema: VALIDATION_FEE_HIJIRI_QUOTE_PROJECTION_SCHEMA_NAME.to_owned(),
        version: VALIDATION_FEE_HIJIRI_QUOTE_VERSION_V1,
        assurance: VALIDATION_FEE_HIJIRI_QUOTE_EVALUATED_ASSURANCE_V1.to_owned(),
        evaluated_state_height: base.evaluated_state_height.to_string(),
        quoted_execution_height: base.quoted_execution_height.to_string(),
        account_id: account_id.clone(),
        active_policy_version: base.active_policy_version.to_string(),
        active_policy_hash: hex::encode(base.active_policy_hash),
        fee_asset_definition_id: base.fee_asset_definition_id,
        treasury_account_id: base.treasury_account_id,
        fee_scale: base.fee_scale,
        hijiri_parameters_version: parameters.version,
        hijiri_parameters_revision: parameters.revision.to_string(),
        hijiri_parameters_digest: hex::encode(parameters_digest),
        default_account_risk_q16: parameters.default_account_risk.raw(),
        effective_account_risk_q16: effective_account_risk.raw(),
        account_risk_revision: account_risk.map(|risk| risk.revision.to_string()),
        account_risk_digest: account_risk_digest.map(hex::encode),
        fee_multiplier_q16: fee_multiplier.raw(),
        hijiri_fee_quote_hash: hex::encode(hijiri_fee_quote_hash),
        base_per_transfer_fee_minor_units: base.base_fee_minor_units.to_string(),
        adjusted_per_transfer_fee_minor_units: adjusted_fee_minor_units.to_string(),
        qualifying_transfer_count,
        aggregate_base_fee_minor_units: aggregate_base_fee_minor_units.to_string(),
        aggregate_adjusted_fee_minor_units: aggregate_adjusted_fee_minor_units.to_string(),
    })
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoDeserialize, NoritoSerialize)]
pub enum ValidationFeeProposalStatusV1 {
    /// Parliament processing is active or certified for future execution.
    Proposed,
    /// Parliament reached a terminal rejection.
    Rejected,
    /// The proposal payload was enacted.
    Enacted,
    /// A concurrently enacted successor made this policy predecessor stale.
    Superseded,
    /// The certified proposal effect failed atomically.
    ExecutionFailed,
}
impl norito::json::FastJsonWrite for ValidationFeeProposalStatusV1 {
    fn write_json(&self, out: &mut String) {
        let label = match self {
            Self::Proposed => "Proposed",
            Self::Rejected => "Rejected",
            Self::Enacted => "Enacted",
            Self::Superseded => "Superseded",
            Self::ExecutionFailed => "ExecutionFailed",
        };
        norito::json::write_json_string(label, out);
    }
}
impl norito::json::JsonDeserialize for ValidationFeeProposalStatusV1 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        match parser.parse_string()?.as_str() {
            "Proposed" => Ok(Self::Proposed),
            "Rejected" => Ok(Self::Rejected),
            "Enacted" => Ok(Self::Enacted),
            "Superseded" => Ok(Self::Superseded),
            "ExecutionFailed" => Ok(Self::ExecutionFailed),
            other => Err(norito::json::Error::InvalidField {
                field: "status".to_owned(),
                message: format!("unknown governance proposal status `{other}`"),
            }),
        }
    }
}
/// One complete typed validation-fee proposal read from protected governance state.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct ValidationFeeProposalRecordV1 {
    /// Bonded citizen who created the proposal.
    pub proposer: AccountId,
    /// Exact native validation-fee proposal kind and payload.
    pub kind: ProposalKind,
    /// Height at which the proposal was created.
    #[norito(json = "first_release_exact_json_u64_number")]
    pub created_height: u64,
    /// Current proposal status.
    pub status: ValidationFeeProposalStatusV1,
}
mod first_release_exact_json_u64_number {
    use norito::json::{
        self, BoundedJsonError, JsonDeserialize, JsonSerialize, JsonWriteSink, Parser,
    };

    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "Norito field serializers receive values by shared reference"
    )]
    pub fn serialize(value: &u64, out: &mut String) {
        value.json_serialize(out);
    }

    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "Norito bounded field serializers receive values by shared reference"
    )]
    pub fn serialize_bounded(
        value: &u64,
        out: &mut dyn JsonWriteSink,
    ) -> Result<(), BoundedJsonError> {
        value.json_serialize_to(out)
    }

    pub fn deserialize(parser: &mut Parser<'_>) -> Result<u64, json::Error> {
        let value = u64::json_deserialize(parser)?;
        if value > iroha_data_model::parliament_types::FIRST_RELEASE_MAX_EXACT_JSON_U64 {
            return Err(json::Error::InvalidField {
                field: "created_height".to_owned(),
                message:
                    "governance proposal creation height exceeds the exact JSON integer maximum"
                        .to_owned(),
            });
        }
        Ok(value)
    }
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, NoritoDeserialize, NoritoSerialize)]
pub struct ValidationFeeProposalDetailQueryV1 {}
impl norito::json::JsonSerialize for ValidationFeeProposalDetailQueryV1 {
    fn json_serialize(&self, out: &mut String) {
        out.push_str("{}");
    }

    fn json_serialize_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        norito::json::write_validated_json_to("{}", out)
    }
}
impl norito::json::JsonDeserialize for ValidationFeeProposalDetailQueryV1 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value =
            <norito::json::Value as norito::json::JsonDeserialize>::json_deserialize(parser)?;
        match value {
            norito::json::Value::Object(fields) if fields.is_empty() => Ok(Self {}),
            norito::json::Value::Object(_) => Err(norito::json::Error::Message(
                "validation-fee proposal detail query rejects unknown fields".to_owned(),
            )),
            _ => Err(norito::json::Error::Message(
                "validation-fee proposal detail query must be an empty object".to_owned(),
            )),
        }
    }
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
fn exact_decimal_u64(label: &str, value: &str) -> Result<u64, String> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| format!("{label} must be a canonical unsigned decimal integer"))?;
    if parsed.to_string() != value {
        return Err(format!(
            "{label} must be a canonical unsigned decimal integer"
        ));
    }
    Ok(parsed)
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
    use iroha_data_model::{
        governance::types::{
            BallotAttemptId, BeaconPulseId, BeaconSessionId, BodyElectionAttemptId, BodyInstanceId,
            GovernanceAttemptId, GovernanceCertificateId, GovernanceExpectedHeadPresentV1,
            GovernanceExpectedHeadV1, ParliamentAggregateOutcomeV1, ParliamentAggregateTallyV1,
            ParliamentBallotCertificateBindingV1, ParliamentBody,
            ParliamentBodyCertificateBindingV1, ProposalContentId, RiskTierV1, SortitionRequestV1,
            TleKeySessionId, TleSessionId, parliament_ballot_result_root_v1,
        },
        hijiri::{FeeMultiplierBand, HijiriFeePolicy, Q16},
    };
    fn fixture_account(seed: u8) -> AccountId {
        let key_pair =
            iroha_crypto::KeyPair::try_from_seed(vec![seed; 32], iroha_crypto::Algorithm::Ed25519)
                .expect("derive deterministic validation-fee fixture account");
        AccountId::new(key_pair.public_key().clone())
    }
    fn fixture_asset_definition() -> AssetDefinitionId {
        AssetDefinitionId::from_uuid_bytes([
            0x2f, 0x17, 0xc7, 0x24, 0x66, 0xf8, 0x4a, 0x4b, 0xb8, 0xa8, 0xe2, 0x48, 0x84, 0xfd,
            0xcd, 0x2f,
        ])
        .expect("valid deterministic validation-fee fixture asset")
    }
    fn hijiri_parameters(default_account_risk: Q16) -> HijiriParametersV1 {
        let fee_policy = HijiriFeePolicy::new(
            vec![
                FeeMultiplierBand::new(Q16::from_parts(0, 0x8000), Q16::ONE)
                    .expect("low-risk Hijiri fee band"),
                FeeMultiplierBand::new(Q16::ONE, Q16::from_parts(1, 0x4000))
                    .expect("high-risk Hijiri fee band"),
            ],
            Q16::from_parts(2, 0),
        )
        .expect("valid Hijiri fee policy");
        HijiriParametersV1::try_new(1, None, fee_policy, default_account_risk)
            .expect("valid global Hijiri parameter")
    }
    fn parliament_authorization(
        proposal_fingerprint: [u8; 32],
        enacted_at_height: u64,
    ) -> ValidationFeeParliamentAuthorizationV1 {
        let base = enacted_at_height
            .checked_sub(1_024)
            .expect("certificate lifecycle");
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
        let release_height = base + 1_021;
        let tle_session_id = TleSessionId::derive_v1(
            ballot_attempt_id,
            tle_key_session_id,
            release_beacon_session_id,
            release_height,
        );
        let opening_root = root(16);
        let tally = ParliamentAggregateTallyV1 {
            original_seats: 500,
            accepted_ballots: 334,
            aye: 200,
            nay: 100,
            abstain: 34,
        };
        let outcome = ParliamentAggregateOutcomeV1::Approved;
        let result_height = base + 1_022;
        let result_root = parliament_ballot_result_root_v1(
            governance_attempt_id,
            body_instance_id,
            ballot_attempt_id,
            opening_root,
            tally,
            outcome,
            result_height,
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
                original_seats: tally.original_seats,
                beacon_session_id,
                beacon_pulse_id: BeaconPulseId::new(root(3)),
                roster_root,
                assignment_root: root(5),
                result_root,
                result_height,
                public_finding: None,
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
                    registration_close_height: base + 504,
                    survivor_freeze_height: base + 1_004,
                    commitment_close_height: base + 1_020,
                    registration_closed_at_height: base + 504,
                    survivors_frozen_at_height: base + 1_004,
                    commitment_closed_at_height: base + 1_020,
                    max_ballot_retries: 3,
                    max_corpus_entries: 500,
                    release_height,
                    opening_deadline_height: result_height,
                    release_pulse_id: BeaconPulseId::new(root(15)),
                    opening_height: release_height,
                    opening_root,
                    tally,
                    outcome,
                }),
            }],
            policy_version: 1,
            effect_preimage_hash: root(19),
            expected_head: GovernanceExpectedHeadV1::Present(GovernanceExpectedHeadPresentV1 {
                subject_id: root(17),
                version: 1,
                head_root: root(18),
            }),
            certified_at_height: base + 1_023,
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
        let mut authorization = parliament_authorization([0x02; 32], 2_048);
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
        let authorization = parliament_authorization([0x02; 32], 2_048);
        let proposal = verified_parliament_proposal("ValidationFeePolicyV1", &authorization)
            .expect("project canonical certificate authorization");
        let fee_asset_definition_id = fixture_asset_definition().to_string();
        let treasury_account_id = fixture_account(44).to_string();
        let current = ValidationFeeVerifiedCurrentPolicyV1 {
            active_policy_version: "1".to_owned(),
            active_policy_hash: "03".repeat(32),
            fee_asset_definition_id: fee_asset_definition_id.clone(),
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
                treasury_account_id: treasury_account_id.clone(),
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
        let verified = ValidationFeeVerifiedPolicyProjectionV1 {
            schema: VALIDATION_FEE_VERIFIED_POLICY_PROJECTION_SCHEMA_NAME.to_owned(),
            version: VALIDATION_FEE_POLICY_PROOF_VERSION_V1,
            network_id: "network".to_owned(),
            policy_chain_genesis_hash: "03".repeat(32),
            registry_hash: "03".repeat(32),
            head_policy_version: 1,
            head_policy_hash: "03".repeat(32),
            current_policy: Some(current),
            trusted_checkpoint_height: 120_960,
            trusted_checkpoint_context_id: "03".repeat(32),
            evaluated_block_height: 120_961,
            evaluated_context_id: "03".repeat(32),
            evaluated_block_hash: "03".repeat(32),
            observed_ledger_tip_height: 120_961,
            more_available: false,
        };
        let account_id = fixture_account(30);
        let parameters = hijiri_parameters(Q16::from_parts(0, 0xC000));
        let quote = verified
            .evaluate_hijiri_quote(&account_id, &parameters, None)
            .expect("evaluate separately supplied Hijiri state")
            .expect("enabled policy has a quote");
        assert_eq!(
            quote.assurance,
            VALIDATION_FEE_HIJIRI_QUOTE_EVALUATED_ASSURANCE_V1
        );
        assert_eq!(quote.account_id, account_id);
        assert_eq!(
            quote.default_account_risk_q16,
            Q16::from_parts(0, 0xC000).raw()
        );
        assert_eq!(
            quote.effective_account_risk_q16,
            Q16::from_parts(0, 0xC000).raw()
        );
        assert_eq!(quote.fee_multiplier_q16, Q16::from_parts(1, 0x4000).raw());
        assert_eq!(quote.fee_asset_definition_id, fee_asset_definition_id);
        assert_eq!(quote.treasury_account_id, treasury_account_id);
        assert_eq!(quote.evaluated_state_height, "120961");
        assert_eq!(quote.quoted_execution_height, "120961");
        assert_eq!(quote.base_per_transfer_fee_minor_units, "10");
        assert_eq!(quote.adjusted_per_transfer_fee_minor_units, "13");
        assert_eq!(quote.qualifying_transfer_count, 1);
        assert_eq!(quote.aggregate_base_fee_minor_units, "10");
        assert_eq!(quote.aggregate_adjusted_fee_minor_units, "13");
        assert!(quote.account_risk_revision.is_none());
        assert!(quote.account_risk_digest.is_none());
        let quote_json = norito::json::to_string(&quote).expect("serialize Hijiri quote");
        for key in [
            "evaluatedStateHeight",
            "quotedExecutionHeight",
            "accountId",
            "treasuryAccountId",
            "hijiriParametersRevision",
            "hijiriParametersDigest",
            "defaultAccountRiskQ16",
            "effectiveAccountRiskQ16",
            "accountRiskRevision",
            "accountRiskDigest",
            "feeMultiplierQ16",
            "hijiriFeeQuoteHash",
            "basePerTransferFeeMinorUnits",
            "adjustedPerTransferFeeMinorUnits",
            "qualifyingTransferCount",
            "aggregateBaseFeeMinorUnits",
            "aggregateAdjustedFeeMinorUnits",
        ] {
            assert!(
                quote_json.contains(&format!(r#""{key}":"#)),
                "missing {key}"
            );
        }
        assert!(quote_json.contains(VALIDATION_FEE_HIJIRI_QUOTE_EVALUATED_ASSURANCE_V1));
        let quote_roundtrip: ValidationFeeHijiriQuoteProjectionV1 =
            norito::json::from_str(&quote_json).expect("roundtrip Hijiri quote");
        assert_eq!(quote_roundtrip, quote);
    }
    #[test]
    fn hijiri_quote_binds_explicit_risk_presence_even_when_value_matches_default() {
        let account_id = fixture_account(31);
        let risk = Q16::from_parts(0, 0x4000);
        let parameters = hijiri_parameters(risk);
        let base = ValidationFeeHijiriQuoteBaseV1::try_new(
            42,
            43,
            3,
            [0x03; 32],
            fixture_asset_definition().to_string(),
            fixture_account(41).to_string(),
            VALIDATION_FEE_DS_SCALE,
            VALIDATION_FEE_BASE_MINOR_UNITS_V1,
        )
        .expect("valid quote base");
        let default_quote =
            evaluate_hijiri_quote_v1(base.clone(), &account_id, &parameters, None, 1)
                .expect("evaluate default-risk quote");
        let explicit_risk = HijiriAccountRiskV1::try_new(account_id.clone(), 1, None, risk)
            .expect("valid explicit account risk");
        let explicit_quote =
            evaluate_hijiri_quote_v1(base, &account_id, &parameters, Some(&explicit_risk), 1)
                .expect("evaluate explicit-risk quote");
        assert_eq!(
            default_quote.effective_account_risk_q16,
            explicit_quote.effective_account_risk_q16
        );
        assert_eq!(
            default_quote.adjusted_per_transfer_fee_minor_units,
            explicit_quote.adjusted_per_transfer_fee_minor_units
        );
        assert_ne!(
            default_quote.hijiri_fee_quote_hash, explicit_quote.hijiri_fee_quote_hash,
            "None and Some(record) are distinct committed quote inputs"
        );
        assert_eq!(explicit_quote.account_risk_revision.as_deref(), Some("1"));
        assert_eq!(
            explicit_quote.account_risk_digest,
            Some(hex::encode(
                explicit_risk
                    .digest()
                    .expect("digest explicit account risk")
            ))
        );
    }
    #[test]
    fn hijiri_quote_rounds_one_aggregate_instead_of_each_transfer() {
        let account_id = fixture_account(34);
        let treasury_account_id = fixture_account(38);
        let parameters = hijiri_parameters(Q16::from_parts(0, 0xC000));
        let quote = evaluate_hijiri_quote_v1(
            ValidationFeeHijiriQuoteBaseV1::try_new(
                42,
                43,
                1,
                [0x03; 32],
                fixture_asset_definition().to_string(),
                treasury_account_id.to_string(),
                VALIDATION_FEE_DS_SCALE,
                VALIDATION_FEE_BASE_MINOR_UNITS_V1,
            )
            .expect("valid quote base"),
            &account_id,
            &parameters,
            None,
            3,
        )
        .expect("evaluate aggregate quote");
        assert_eq!(quote.adjusted_per_transfer_fee_minor_units, "13");
        assert_eq!(quote.qualifying_transfer_count, 3);
        assert_eq!(quote.aggregate_base_fee_minor_units, "30");
        assert_eq!(quote.aggregate_adjusted_fee_minor_units, "38");
        assert_ne!(
            quote.aggregate_adjusted_fee_minor_units,
            (3_u64 * 13).to_string(),
            "already-rounded per-transfer quotes must not be multiplied"
        );
        quote
            .validate_coherence()
            .expect("generated aggregate quote is self-consistent");
    }
    #[test]
    fn hijiri_quote_coherence_validator_rejects_malformed_response() {
        let account_id = fixture_account(36);
        let treasury_account_id = fixture_account(39);
        let parameters = hijiri_parameters(Q16::from_parts(0, 0xC000));
        let quote = evaluate_hijiri_quote_v1(
            ValidationFeeHijiriQuoteBaseV1::try_new(
                42,
                43,
                1,
                [0x03; 32],
                fixture_asset_definition().to_string(),
                treasury_account_id.to_string(),
                VALIDATION_FEE_DS_SCALE,
                VALIDATION_FEE_BASE_MINOR_UNITS_V1,
            )
            .expect("valid quote base"),
            &account_id,
            &parameters,
            None,
            3,
        )
        .expect("evaluate coherent quote");
        quote
            .validate_coherence()
            .expect("generated quote is coherent");

        let mut malformed = quote.clone();
        malformed.aggregate_base_fee_minor_units = "31".to_owned();
        assert!(malformed.validate_coherence().is_err());

        let mut malformed = quote.clone();
        malformed.aggregate_adjusted_fee_minor_units = "39".to_owned();
        assert!(malformed.validate_coherence().is_err());

        let mut malformed = quote.clone();
        malformed.hijiri_parameters_digest = "AA".repeat(32);
        assert!(malformed.validate_coherence().is_err());

        let mut malformed = quote.clone();
        malformed.hijiri_parameters_digest = "01".repeat(32);
        assert!(
            malformed.validate_coherence().is_err(),
            "a canonical-looking digest must still match the composite binding"
        );

        let mut malformed = quote.clone();
        malformed.fee_asset_definition_id = "asset".to_owned();
        assert!(malformed.validate_coherence().is_err());

        let mut malformed = quote.clone();
        malformed.treasury_account_id = "treasury".to_owned();
        assert!(malformed.validate_coherence().is_err());

        let mut malformed = quote.clone();
        malformed.effective_account_risk_q16 = Q16::ZERO.raw();
        assert!(malformed.validate_coherence().is_err());

        let mut malformed = quote;
        malformed.account_risk_revision = Some("1".to_owned());
        assert!(malformed.validate_coherence().is_err());
    }
    #[test]
    fn hijiri_live_quote_validator_binds_request_and_successor_height() {
        let account_id = fixture_account(37);
        let treasury_account_id = fixture_account(40);
        let parameters = hijiri_parameters(Q16::ZERO);
        let quote = evaluate_hijiri_quote_v1(
            ValidationFeeHijiriQuoteBaseV1::try_new(
                42,
                43,
                1,
                [0x03; 32],
                fixture_asset_definition().to_string(),
                treasury_account_id.to_string(),
                VALIDATION_FEE_DS_SCALE,
                VALIDATION_FEE_BASE_MINOR_UNITS_V1,
            )
            .expect("valid live quote base"),
            &account_id,
            &parameters,
            None,
            2,
        )
        .expect("evaluate live quote");
        let request = ValidationFeeHijiriQuoteRequestV1 {
            version: VALIDATION_FEE_HIJIRI_QUOTE_VERSION_V1,
            account_id: account_id.clone(),
            qualifying_transfer_count: 2,
        };
        quote
            .validate_for_request(&request)
            .expect("live quote binds the request and next height");

        let mut wrong_count = request.clone();
        wrong_count.qualifying_transfer_count = 1;
        assert!(quote.validate_for_request(&wrong_count).is_err());

        let mut historical = quote;
        historical.quoted_execution_height = historical.evaluated_state_height.clone();
        historical
            .validate_coherence()
            .expect("general validator admits a proof-derived equal-height projection");
        assert!(historical.validate_for_request(&request).is_err());
    }
    #[test]
    fn hijiri_quote_rejects_an_account_risk_record_for_another_account() {
        let account_id = fixture_account(32);
        let parameters = hijiri_parameters(Q16::ZERO);
        let other_risk =
            HijiriAccountRiskV1::try_new(fixture_account(33), 1, None, Q16::from_parts(0, 0xC000))
                .expect("valid risk for another account");
        let error = evaluate_hijiri_quote_v1(
            ValidationFeeHijiriQuoteBaseV1::try_new(
                42,
                43,
                1,
                [0x03; 32],
                fixture_asset_definition().to_string(),
                fixture_account(42).to_string(),
                VALIDATION_FEE_DS_SCALE,
                VALIDATION_FEE_BASE_MINOR_UNITS_V1,
            )
            .expect("valid quote base"),
            &account_id,
            &parameters,
            Some(&other_risk),
            1,
        )
        .expect_err("a risk record cannot be replayed for another account");
        assert!(
            error.contains("does not match its account-risk record"),
            "unexpected error: {error}"
        );
    }
    #[test]
    fn hijiri_quote_base_rejects_noncanonical_fee_coordinates() {
        let asset = fixture_asset_definition().to_string();
        let treasury = fixture_account(43).to_string();
        let build = |asset, treasury| {
            ValidationFeeHijiriQuoteBaseV1::try_new(
                42,
                43,
                1,
                [0x03; 32],
                asset,
                treasury,
                VALIDATION_FEE_DS_SCALE,
                VALIDATION_FEE_BASE_MINOR_UNITS_V1,
            )
        };
        assert!(build("asset".to_owned(), treasury.clone()).is_err());
        assert!(build(asset, "treasury".to_owned()).is_err());
    }
    #[test]
    fn hijiri_quote_base_rejects_invalid_policy_invariants() {
        let build = |evaluated_state_height,
                     quoted_execution_height,
                     active_policy_version,
                     active_policy_hash,
                     fee_scale,
                     base_fee_minor_units| {
            ValidationFeeHijiriQuoteBaseV1::try_new(
                evaluated_state_height,
                quoted_execution_height,
                active_policy_version,
                active_policy_hash,
                fixture_asset_definition().to_string(),
                fixture_account(45).to_string(),
                fee_scale,
                base_fee_minor_units,
            )
        };
        assert!(
            build(
                42,
                43,
                1,
                [0x03; 32],
                VALIDATION_FEE_DS_SCALE,
                VALIDATION_FEE_BASE_MINOR_UNITS_V1,
            )
            .is_ok()
        );
        for invalid in [
            build(
                0,
                1,
                1,
                [0x03; 32],
                VALIDATION_FEE_DS_SCALE,
                VALIDATION_FEE_BASE_MINOR_UNITS_V1,
            ),
            build(
                42,
                44,
                1,
                [0x03; 32],
                VALIDATION_FEE_DS_SCALE,
                VALIDATION_FEE_BASE_MINOR_UNITS_V1,
            ),
            build(
                43,
                42,
                1,
                [0x03; 32],
                VALIDATION_FEE_DS_SCALE,
                VALIDATION_FEE_BASE_MINOR_UNITS_V1,
            ),
            build(
                42,
                43,
                0,
                [0x03; 32],
                VALIDATION_FEE_DS_SCALE,
                VALIDATION_FEE_BASE_MINOR_UNITS_V1,
            ),
            build(
                42,
                43,
                1,
                [0x02; 32],
                VALIDATION_FEE_DS_SCALE,
                VALIDATION_FEE_BASE_MINOR_UNITS_V1,
            ),
            build(
                42,
                43,
                1,
                [0x03; 32],
                VALIDATION_FEE_DS_SCALE - 1,
                VALIDATION_FEE_BASE_MINOR_UNITS_V1,
            ),
            build(
                42,
                43,
                1,
                [0x03; 32],
                VALIDATION_FEE_DS_SCALE,
                VALIDATION_FEE_BASE_MINOR_UNITS_V1 + 1,
            ),
        ] {
            assert!(invalid.is_err());
        }
    }
    #[test]
    fn hijiri_quote_is_absent_before_base_policy_enactment() {
        let projection = ValidationFeeVerifiedPolicyProjectionV1 {
            schema: VALIDATION_FEE_VERIFIED_POLICY_PROJECTION_SCHEMA_NAME.to_owned(),
            version: VALIDATION_FEE_POLICY_PROOF_VERSION_V1,
            network_id: "network".to_owned(),
            policy_chain_genesis_hash: "03".repeat(32),
            registry_hash: "03".repeat(32),
            head_policy_version: 0,
            head_policy_hash: "03".repeat(32),
            current_policy: None,
            trusted_checkpoint_height: 1,
            trusted_checkpoint_context_id: "03".repeat(32),
            evaluated_block_height: 1,
            evaluated_context_id: "03".repeat(32),
            evaluated_block_hash: "03".repeat(32),
            observed_ledger_tip_height: 1,
            more_available: false,
        };
        let account_id = fixture_account(46);
        assert_eq!(
            projection
                .evaluate_hijiri_quote(&account_id, &hijiri_parameters(Q16::ZERO), None)
                .expect("an absent base policy is a valid pre-enactment state"),
            None
        );
    }
    #[test]
    fn exact_decimal_u64_rejects_noncanonical_spellings() {
        assert_eq!(exact_decimal_u64("height", "42"), Ok(42));
        assert!(exact_decimal_u64("height", "042").is_err());
        assert!(exact_decimal_u64("height", "+42").is_err());
        assert!(exact_decimal_u64("height", "-1").is_err());
    }
    #[test]
    fn hijiri_quote_request_enforces_version_and_transfer_bound() {
        let request = |version, qualifying_transfer_count| ValidationFeeHijiriQuoteRequestV1 {
            version,
            account_id: fixture_account(35),
            qualifying_transfer_count,
        };
        let valid = request(VALIDATION_FEE_HIJIRI_QUOTE_VERSION_V1, 1);
        assert!(valid.validate().is_ok());
        let json = norito::json::to_string(&valid).expect("serialize Hijiri quote request");
        assert!(json.contains(r#""accountId":"#));
        assert!(json.contains(r#""qualifyingTransferCount":1"#));
        assert_eq!(
            norito::json::from_str::<ValidationFeeHijiriQuoteRequestV1>(&json)
                .expect("roundtrip Hijiri quote request"),
            valid
        );
        assert!(
            request(
                VALIDATION_FEE_HIJIRI_QUOTE_VERSION_V1,
                VALIDATION_FEE_HIJIRI_QUOTE_MAX_QUALIFYING_TRANSFERS_V1,
            )
            .validate()
            .is_ok()
        );
        assert!(request(0, 1).validate().is_err());
        assert!(
            request(VALIDATION_FEE_HIJIRI_QUOTE_VERSION_V1, 0)
                .validate()
                .is_err()
        );
        assert!(
            request(
                VALIDATION_FEE_HIJIRI_QUOTE_VERSION_V1,
                VALIDATION_FEE_HIJIRI_QUOTE_MAX_QUALIFYING_TRANSFERS_V1 + 1,
            )
            .validate()
            .is_err()
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
    fn proposal_status_json_is_the_exact_five_pascal_case_strings() {
        for (status, label) in [
            (ValidationFeeProposalStatusV1::Proposed, "Proposed"),
            (ValidationFeeProposalStatusV1::Rejected, "Rejected"),
            (ValidationFeeProposalStatusV1::Enacted, "Enacted"),
            (ValidationFeeProposalStatusV1::Superseded, "Superseded"),
            (
                ValidationFeeProposalStatusV1::ExecutionFailed,
                "ExecutionFailed",
            ),
        ] {
            let json = norito::json::to_string(&status).expect("serialize proposal status");
            assert_eq!(json, format!("\"{label}\""));
            assert_eq!(
                norito::json::from_str::<ValidationFeeProposalStatusV1>(&json)
                    .expect("deserialize proposal status"),
                status
            );
        }
        for retired in [
            r#""Approved""#,
            r#""PROPOSED""#,
            r#"{"status":"PROPOSED","value":null}"#,
        ] {
            assert!(
                norito::json::from_str::<ValidationFeeProposalStatusV1>(retired).is_err(),
                "retired status representation must reject: {retired}"
            );
        }
    }
    #[test]
    fn proposal_created_height_json_rejects_inexact_numbers() {
        let maximum = iroha_data_model::parliament_types::FIRST_RELEASE_MAX_EXACT_JSON_U64;
        let maximum_json = maximum.to_string();
        let mut parser = norito::json::Parser::new(&maximum_json);
        assert_eq!(
            first_release_exact_json_u64_number::deserialize(&mut parser)
                .expect("the exact JSON integer maximum is valid"),
            maximum
        );
        let hostile_json = (maximum + 1).to_string();
        let mut parser = norito::json::Parser::new(&hostile_json);
        assert!(
            first_release_exact_json_u64_number::deserialize(&mut parser).is_err(),
            "one above the exact JSON integer maximum must reject"
        );
    }
    #[test]
    fn proposal_list_query_defaults_to_a_bounded_page() {
        let query: ValidationFeeProposalListQueryV1 =
            norito::json::from_str("{}").expect("decode default proposal page query");
        assert_eq!(query, ValidationFeeProposalListQueryV1::default());
        assert_eq!(query.limit, VALIDATION_FEE_PROPOSAL_PAGE_DEFAULT_LIMIT_V1);
    }
}
