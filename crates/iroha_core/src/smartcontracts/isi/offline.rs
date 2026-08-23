//! Kagemusha offline-cash instruction execution.
mod kagemusha_marker;
mod kagemusha_release_lifecycle;
mod kagemusha_runtime_effective_config;
mod kagemusha_terminal_registry_v4;
use super::prelude::*;
use crate::smartcontracts::isi::asset::isi::assert_numeric_spec_with;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    account::AccountId,
    asset::{
        AssetBalancePolicy, AssetBalanceScope, AssetDefinitionId, AssetId,
        definition::ConfidentialPolicyMode,
    },
    confidential::ConfidentialStatus,
    isi::{
        error::InstructionExecutionError,
        offline::{
            ActivateKagemushaRecursiveReleaseV4, AuthorizeKagemushaTairaCanaryV4,
            CancelKagemushaRecursiveReleaseV4, DeactivateKagemushaRecursiveIssuanceV4,
            EnableKagemushaRecursiveIssuanceV4, RecordKagemushaTairaCanaryV4,
            RedeemKagemushaRecursiveV4, RegisterOfflineDeviceAttestation,
            SetOfflineDeviceAttestationPolicy, TopUpKagemushaRecursiveV4,
        },
    },
    offline::{
        KAGEMUSHA_ACTIVE_DEVICE_REGISTRATIONS_MAX_GLOBAL_V1,
        KAGEMUSHA_ACTIVE_DEVICE_REGISTRATIONS_MAX_PER_ACCOUNT_V1,
        KAGEMUSHA_IOS_APP_ATTEST_ASSERTION_AUTH_DATA_FIXED_HEADER_BYTES_V1,
        KAGEMUSHA_IOS_APP_ATTEST_ASSERTION_AUTH_DATA_MAX_BYTES_V1,
        KAGEMUSHA_IOS_APP_ATTEST_ASSERTION_AUTH_DATA_MIN_BYTES_V1,
        KagemushaActiveReceiverActiveEntryV1, KagemushaActiveReceiverAmbiguousEntryV1,
        KagemushaActiveReceiverEntryV1, KagemushaActiveReceiverKeyV1,
        KagemushaActiveReceiverSnapshotV1, KagemushaActiveReceiverValueV1,
        KagemushaOnlineHardwareAssertionV1, KagemushaRecipientPaymentRequestV2,
        KagemushaRecursiveSpendBranchClaimV2, KagemushaRecursiveSpendBranchPathV2,
        KagemushaRecursiveSpendTopUpAnchorRefV2, KagemushaRecursiveSpendTopUpAnchorV4,
        KagemushaRequestAuthorizationV2, OFFLINE_DEVICE_ATTESTATION_DEVICE_ID_MAX_BYTES_V1,
        OFFLINE_DEVICE_ATTESTATION_KEY_ID_MAX_BYTES_V1,
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_ANDROID_APPS_V1,
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_ANDROID_SIGNING_CERTIFICATES_V1,
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_APP_IDENTIFIER_BYTES_V1,
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_CANONICAL_BYTES_V1,
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_APPS_V1,
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_BUNDLE_VERSION_BYTES_V1,
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_BUNDLE_VERSIONS_V1,
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_VALIDATION_CATEGORIES_V1,
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_REVOKED_CERTIFICATES_V1,
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TEAM_ID_BYTES_V1,
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOT_DER_BYTES_V1,
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOTS_PER_PLATFORM_V1,
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOTS_V1,
        OFFLINE_DEVICE_ATTESTATION_POLICY_VERSION_V2, OFFLINE_REJECTION_REASON_PREFIX,
        OfflineAndroidAppAttestationPolicy, OfflineAndroidAttestationStatusSnapshotV1,
        OfflineAndroidAttestedDevicePropertiesV2, OfflineAndroidDeviceSecurityLevelV2,
        OfflineAndroidDeviceVulnerabilityRuleV2, OfflineDeviceAttestationPolicy,
        OfflineDeviceAttestationPolicyV1, OfflineDeviceAttestationPolicyViewV1,
        OfflineDeviceAttestationRegistration, OfflineDeviceAttestationTrustedRoot,
        OfflineDevicePolicyFinalityBindingV1, OfflineIosAppAttestationPolicy,
    },
    proof::{ProofAttachment, VerifyingKeyBox, VerifyingKeyRecord},
    state_path::StatePath,
    transaction::SignedTransaction,
    zk::{BackendTag, OpenVerifyEnvelope},
};
use iroha_primitives::numeric::Quantity;
pub(crate) use isi::signed_kagemusha_taira_canary_wire_identity_v1;
use kagemusha_marker::v2_marker as kagemusha_v2_marker;
#[cfg(test)]
pub(crate) use kagemusha_release_lifecycle::tests::staged_lifecycle_for_test;
pub(crate) use kagemusha_release_lifecycle::{
    LifecycleEntrypointContext, direct_lifecycle_entrypoint_kind,
    require_local_runtime_effective_config, signed_lifecycle_entrypoint_context,
    validate_runtime_consensus_parameter_update,
};
pub use kagemusha_runtime_effective_config::VerifiedKagemushaV4RuntimeEffectiveConfigV1;
pub use kagemusha_terminal_registry_v4::{
    KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_MAX_BYTES_V1, KagemushaCatalogQualificationSealV1,
    KagemushaReleaseCatalogV4, KagemushaValidatorQualificationCatalogCaptureV1,
};
use kagemusha_terminal_registry_v4::{commit_v4_promotion_id, plan_v4_promotion_id};
use p256::PublicKey as P256PublicKey;
use sha2::{Digest as _, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    io::Cursor,
    sync::LazyLock,
};
use x509_parser::{
    extensions::ParsedExtension,
    prelude::{FromDer as _, X509Certificate},
    time::ASN1Time,
};
const CAN_MANAGE_OFFLINE_ESCROW_PERMISSION: &str = "CanManageOfflineEscrow";
const CAN_MANAGE_OFFLINE_DEVICE_ATTESTATION_POLICY_PERMISSION: &str =
    "CanManageOfflineDeviceAttestationPolicy";
const CAN_ACTIVATE_KAGEMUSHA_RECURSIVE_RELEASE_V4_PERMISSION: &str =
    "CanActivateKagemushaRecursiveReleaseV4";
/// One-shot proof that an offline top-up debit passed the exact signed-request checks.
pub(in crate::smartcontracts::isi) struct VerifiedKagemushaTopUpDebit {
    source_authority: AccountId,
    operation_id: [u8; 32],
    source_id: AssetId,
    destination_id: AssetId,
    amount: Quantity,
}
impl VerifiedKagemushaTopUpDebit {
    fn new(
        source_authority: AccountId,
        operation_id: [u8; 32],
        source_id: AssetId,
        destination_id: AssetId,
        amount: Quantity,
    ) -> Self {
        Self {
            source_authority,
            operation_id,
            source_id,
            destination_id,
            amount,
        }
    }
    pub(in crate::smartcontracts::isi) fn into_parts(
        self,
    ) -> (AccountId, [u8; 32], AssetId, AssetId, Quantity) {
        (
            self.source_authority,
            self.operation_id,
            self.source_id,
            self.destination_id,
            self.amount,
        )
    }
}
/// One-shot proof that an offline redemption passed recursive-proof and retained-anchor checks.
pub(in crate::smartcontracts::isi) struct VerifiedKagemushaRedemptionDebit {
    operation_id: [u8; 32],
    source_id: AssetId,
    destination_id: AssetId,
    amount: Quantity,
}
impl VerifiedKagemushaRedemptionDebit {
    fn new(
        operation_id: [u8; 32],
        source_id: AssetId,
        destination_id: AssetId,
        amount: Quantity,
    ) -> Self {
        Self {
            operation_id,
            source_id,
            destination_id,
            amount,
        }
    }
    pub(in crate::smartcontracts::isi) fn into_parts(
        self,
    ) -> ([u8; 32], AssetId, AssetId, Quantity) {
        (
            self.operation_id,
            self.source_id,
            self.destination_id,
            self.amount,
        )
    }
}
static OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY: LazyLock<StatePath> = LazyLock::new(|| {
    "offline_device_attestation_policy"
        .parse()
        .expect("static Offline device attestation policy key")
});
fn labeled_invariant(label: &str, message: impl Into<String>) -> InstructionExecutionError {
    let message = message.into();
    let boxed: Box<str> = format!("{OFFLINE_REJECTION_REASON_PREFIX}{label}:{message}").into();
    InstructionExecutionError::InvariantViolation(boxed)
}
fn invalid_attestation(message: impl Into<String>) -> InstructionExecutionError {
    labeled_invariant("invalid_attestation", message)
}
macro_rules! reject_invalid_attestation {
    ($condition:expr, $message:expr $(,)?) => {
        if $condition {
            return Err(invalid_attestation($message).into());
        }
    };
}
fn decode_canonical_offline_proof_envelope(
    bytes: &[u8],
    message: &'static str,
) -> Result<OpenVerifyEnvelope, Error> {
    norito::decode_canonical(bytes).map_err(|_| labeled_invariant("invalid_proof", message).into())
}
/// Key-material-free projection of one authenticated ABI-21 recursive verifier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaRecursiveVerifierReadinessV4 {
    /// Stable Torii role; consensus storage IDs are release-qualified by the owner-manifest digest.
    /// Readiness exposes the fixed ABI-21 Eq/Ep role rather than that rotating identity.
    pub id: iroha_data_model::proof::VerifyingKeyId,
    /// Governance-managed version of the selected registry record.
    pub version: u32,
    /// Backend circuit identifier bound to the verifier.
    pub circuit_id: String,
    /// Domain-separated commitment to the verifying key and backend.
    pub commitment: [u8; 32],
    /// Stable hash of the verifier's public-input layout.
    pub public_inputs_schema_hash: [u8; 32],
    /// Authenticated release proof-pair limit shared with the artifact set.
    pub max_proof_bytes: u32,
    /// Inclusive issuance activation height shared with the artifact set.
    pub activation_height: u64,
    /// First height at which release issuance closes.
    ///
    /// This is not the consensus verifier record's withdrawal height: that
    /// record remains active so notes issued during the window stay redeemable.
    pub withdrawal_height: Option<u64>,
}
/// Exact authenticated V4 release identity safe to publish through readiness.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaAuthenticatedArtifactSetReadinessV4 {
    /// Human-readable generation of the authenticated release.
    pub generation: String,
    /// SHA-256 digest of the canonical release manifest.
    pub manifest_sha256: [u8; 32],
    /// SHA-256 digest of the locally trusted release policy.
    pub release_policy_sha256: [u8; 32],
    /// SHA-256 digest of the canonical signed release attestation.
    pub release_attestation_sha256: [u8; 32],
    /// First height at which the release may issue notes.
    pub activation_height: u64,
    /// First height at which new issuance must stop.
    pub withdrawal_height: u64,
    /// Authenticated upper bound for one canonical proof-pair payload.
    pub max_proof_bytes: u32,
    /// Authoritative fixed scale of the asset bound to the release.
    pub asset_scale: u32,
}
/// Exact on-chain registration admission selected for one signed receiver request.
///
/// This key-material-free projection is returned to Torii only after the request
/// signature, current governed policy, device identity, asset scope, P-256 key,
/// registration expiry, canonical storage key, and admission provenance all match.
/// Raw attestation report and evidence bytes are absent from this active-state projection;
/// their hashes and the original submitted registration hash remain bound in state, while the
/// exact submitted bytes remain in signed transaction history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaRecipientRegistrationResolutionV1 {
    /// Validated registration projection with raw report and evidence bytes removed.
    pub registration: OfflineDeviceAttestationRegistration,
    /// SHA-256 hash of the original canonical registration submitted for admission.
    pub registration_hash: [u8; 32],
    /// Governed policy hash recorded when the registration was admitted.
    pub admission_policy_hash: [u8; 32],
    /// Height of the block that admitted the registration.
    pub admission_height: u64,
    /// Canonical signed transaction that admitted the registration.
    pub admission_transaction_hash: HashOf<SignedTransaction>,
}
/// Chain-derived V4 recursive readiness selected from one committed snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaRecursiveReadinessV4 {
    /// Selected Eq-step verifier registry projection.
    pub step_eq: KagemushaRecursiveVerifierReadinessV4,
    /// Selected Ep-step verifier registry projection.
    pub step_ep: KagemushaRecursiveVerifierReadinessV4,
    /// Authenticated release identity shared by both step verifiers.
    pub artifact_set: KagemushaAuthenticatedArtifactSetReadinessV4,
    /// `None` only when the authenticated material constructs the configured verifier.
    pub proof_backend_error: Option<String>,
}
/// Exact transaction-selected ABI-21 release authenticated for admission.
///
/// Unlike readiness selection, this projection does not require the release's
/// issuance window to be open. That distinction lets already-issued notes use
/// their exact parent verifier throughout its redemption lifetime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaRecursiveTransactionReleaseV4 {
    /// Selected Eq-step verifier registry projection.
    pub step_eq: KagemushaRecursiveVerifierReadinessV4,
    /// Selected Ep-step verifier registry projection.
    pub step_ep: KagemushaRecursiveVerifierReadinessV4,
    /// Exact authenticated release identity shared by the verifier pair.
    pub artifact_set: KagemushaAuthenticatedArtifactSetReadinessV4,
    /// Whether this release may issue a new note at the current block.
    pub issuance_active: bool,
}
fn select_active_kagemusha_v4_verifier(
    world: &impl WorldReadOnly,
    parity: iroha_data_model::offline::KagemushaPastaCycleParityV1,
    block_height: u64,
    role: &str,
) -> Result<Option<(iroha_data_model::proof::VerifyingKeyId, VerifyingKeyRecord)>, String> {
    let circuit_id = kagemusha_v4_circuit_id(parity);
    let mut selected: Option<(iroha_data_model::proof::VerifyingKeyId, VerifyingKeyRecord)> = None;
    for ((indexed_circuit_id, indexed_version), id) in world.verifying_keys_by_circuit().iter() {
        if indexed_circuit_id != circuit_id {
            continue;
        }
        let record = world.verifying_keys().get(id).ok_or_else(|| {
            format!("Kagemusha V4 {role} index version {indexed_version} points at a missing key")
        })?;
        if record.version != *indexed_version || record.circuit_id != circuit_id {
            return Err(format!(
                "Kagemusha V4 {role} index and verifier record disagree"
            ));
        }
        ensure_release_qualified_kagemusha_v4_verifier_id(id, record, parity, role)?;
        let supersedes_selected = selected
            .as_ref()
            .is_none_or(|(_, selected_record)| record.version > selected_record.version);
        if record.is_active_at(block_height) && supersedes_selected {
            selected = Some((id.clone(), record.clone()));
        }
    }
    Ok(selected)
}
fn indexed_kagemusha_v4_verifier_v4(
    world: &impl WorldReadOnly,
    parity: iroha_data_model::offline::KagemushaPastaCycleParityV1,
    version: u32,
    role: &str,
) -> Result<Option<(iroha_data_model::proof::VerifyingKeyId, VerifyingKeyRecord)>, String> {
    let circuit_id = kagemusha_v4_circuit_id(parity);
    let index_key = (circuit_id.to_owned(), version);
    let Some(id) = world.verifying_keys_by_circuit().get(&index_key) else {
        return Ok(None);
    };
    let record = world.verifying_keys().get(id).ok_or_else(|| {
        format!("Kagemusha V4 {role} index version {version} points at a missing key")
    })?;
    if record.version != version || record.circuit_id != circuit_id {
        return Err(format!(
            "Kagemusha V4 {role} index and verifier record disagree"
        ));
    }
    ensure_release_qualified_kagemusha_v4_verifier_id(id, record, parity, role)?;
    Ok(Some((id.clone(), record.clone())))
}
/// Visit every release whose terminal Eq/Ep verifier records have Active status.
///
/// Terminal records deliberately outlive their issuance windows so historic offline notes can still
/// be redeemed. A command that explicitly uses the release cache therefore authenticates every
/// Active version, including future-activation records, not merely the newest registry entry. This
/// is command-scoped validation, never node startup or capability admission.
fn visit_active_kagemusha_v4_release_pairs(
    world: &impl WorldReadOnly,
    _block_height: u64,
    mut visit: impl FnMut(&VerifyingKeyRecord, &VerifyingKeyRecord) -> Result<(), String>,
) -> Result<usize, String> {
    let mut versions = BTreeSet::new();
    for ((circuit_id, version), _) in world.verifying_keys_by_circuit().iter() {
        if circuit_id == iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4
            || circuit_id
                == iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4
        {
            versions.insert(*version);
        }
    }
    let mut visited = 0_usize;
    for version in versions {
        let step_eq = indexed_kagemusha_v4_verifier_v4(
            world,
            iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEq,
            version,
            "Eq",
        )?;
        let step_ep = indexed_kagemusha_v4_verifier_v4(
            world,
            iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEp,
            version,
            "Ep",
        )?;
        let (step_eq_record, step_ep_record) = match (step_eq, step_ep) {
            (None, None) => continue,
            (Some((_, record)), None) => {
                if record.status == ConfidentialStatus::Active {
                    return Err(format!(
                        "active Kagemusha V4 Eq/Ep verifier pair version {version} is incomplete"
                    ));
                }
                continue;
            }
            (None, Some((_, record))) => {
                if record.status == ConfidentialStatus::Active {
                    return Err(format!(
                        "active Kagemusha V4 Eq/Ep verifier pair version {version} is incomplete"
                    ));
                }
                continue;
            }
            (Some((_, step_eq_record)), Some((_, step_ep_record))) => {
                (step_eq_record, step_ep_record)
            }
        };
        let step_eq_active = step_eq_record.status == ConfidentialStatus::Active;
        let step_ep_active = step_ep_record.status == ConfidentialStatus::Active;
        if step_eq_active != step_ep_active {
            return Err(format!(
                "Kagemusha V4 Eq/Ep verifier activation version {version} is not atomic"
            ));
        }
        if !step_eq_active {
            continue;
        }
        if version == 0
            || step_eq_record.activation_height != step_ep_record.activation_height
            || step_eq_record.withdraw_height != step_ep_record.withdraw_height
        {
            return Err(format!(
                "Kagemusha V4 Eq/Ep verifier activation metadata version {version} is not atomic"
            ));
        }
        visit(&step_eq_record, &step_ep_record)?;
        visited = visited.saturating_add(1);
    }
    Ok(visited)
}
fn kagemusha_v4_circuit_id(
    parity: iroha_data_model::offline::KagemushaPastaCycleParityV1,
) -> &'static str {
    match parity {
        iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEq => {
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4
        }
        iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEp => {
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4
        }
    }
}
fn kagemusha_v4_logical_role_id(
    parity: iroha_data_model::offline::KagemushaPastaCycleParityV1,
) -> iroha_data_model::proof::VerifyingKeyId {
    let role = match parity {
        iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEq => {
            iroha_data_model::offline::KAGEMUSHA_VERIFIER_ROLE_STEP_EQ_V4
        }
        iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEp => {
            iroha_data_model::offline::KAGEMUSHA_VERIFIER_ROLE_STEP_EP_V4
        }
    };
    // This is a public logical capability, not the consensus registry key.
    // Actual records use `kagemusha_recursive_spend_verifier_key_id_v4` and
    // therefore retain their release digest and V4 backend identity.
    iroha_data_model::proof::VerifyingKeyId::new(crate::zk::ZK_BACKEND_HALO2_IPA, role)
}
fn ensure_release_qualified_kagemusha_v4_verifier_id(
    id: &iroha_data_model::proof::VerifyingKeyId,
    record: &VerifyingKeyRecord,
    parity: iroha_data_model::offline::KagemushaPastaCycleParityV1,
    role: &str,
) -> Result<(), String> {
    let manifest_sha256 =
        kagemusha_terminal_registry_v4::verifier_owner_manifest_sha256(record, role)?;
    let expected = iroha_data_model::offline::kagemusha_recursive_spend_verifier_key_id_v4(
        parity,
        manifest_sha256,
    );
    if record.circuit_id != kagemusha_v4_circuit_id(parity)
        || !id.is_portable_registry_id()
        || id != &expected
    {
        return Err(format!(
            "Kagemusha V4 {role} verifier id is not the exact release-qualified registry identity"
        ));
    }
    Ok(())
}
fn decode_kagemusha_v4_consensus_release_state(
    key: &StatePath,
    payload: &[u8],
) -> Result<
    Option<(
        iroha_data_model::offline::KagemushaRecursiveSpendArtifactBindingV4,
        iroha_data_model::offline::KagemushaRecursiveSpendReleaseRecordV4,
    )>,
    String,
> {
    if !key
        .as_ref()
        .starts_with(kagemusha_terminal_registry_v4::TERMINAL_RELEASE_STATE_KEY_PREFIX_V4)
    {
        return Ok(None);
    }
    let release_record: iroha_data_model::offline::KagemushaRecursiveSpendReleaseRecordV4 =
        norito::decode_canonical(payload)
            .map_err(|error| format!("failed to decode Kagemusha V4 release state: {error}"))?;
    release_record
        .validate_structure()
        .map_err(|error| format!("invalid Kagemusha V4 release state: {error}"))?;
    let manifest_sha256: [u8; 32] = Sha256::digest(
        norito::encode_canonical(&release_record.manifest)
            .map_err(|error| format!("failed to encode Kagemusha V4 manifest: {error}"))?,
    )
    .into();
    let binding = iroha_data_model::offline::KagemushaRecursiveSpendArtifactBindingV4 {
        version: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
        generation: release_record.manifest.generation.clone(),
        manifest_sha256,
    };
    let expected_key = kagemusha_terminal_registry_v4::release_state_key(&binding)?;
    if key != &expected_key {
        return Err(
            "Kagemusha V4 release state key is not content-addressed by its manifest".to_owned(),
        );
    }
    Ok(Some((binding, release_record)))
}
const fn kagemusha_v4_issuance_active_at(
    activation_height: u64,
    withdrawal_height: u64,
    block_height: u64,
) -> bool {
    block_height >= activation_height && block_height < withdrawal_height
}
const fn kagemusha_v4_issuance_windows_overlap(
    first_activation_height: u64,
    first_withdrawal_height: u64,
    second_activation_height: u64,
    second_withdrawal_height: u64,
) -> bool {
    first_activation_height < second_withdrawal_height
        && second_activation_height < first_withdrawal_height
}
fn project_kagemusha_v4_verifier(
    parity: iroha_data_model::offline::KagemushaPastaCycleParityV1,
    record: VerifyingKeyRecord,
    artifact_set: &KagemushaAuthenticatedArtifactSetReadinessV4,
) -> KagemushaRecursiveVerifierReadinessV4 {
    KagemushaRecursiveVerifierReadinessV4 {
        id: kagemusha_v4_logical_role_id(parity),
        version: record.version,
        circuit_id: record.circuit_id,
        commitment: record.commitment,
        public_inputs_schema_hash: record.public_inputs_schema_hash,
        max_proof_bytes: artifact_set.max_proof_bytes,
        activation_height: artifact_set.activation_height,
        // Verifier records deliberately remain active after issuance closes so
        // previously issued notes can be redeemed. Readiness reports the
        // release issuance boundary because SDK atomic binding describes note
        // creation, not the verifier record's longer redemption lifetime.
        withdrawal_height: Some(artifact_set.withdrawal_height),
    }
}
/// Resolve the exact active ABI-21 Eq/Ep registry release for Torii readiness.
///
/// This is read-only and does not admit, verify, or redeem a transaction.
pub fn resolve_kagemusha_recursive_readiness_v4(
    world: &impl WorldReadOnly,
    catalog: &KagemushaReleaseCatalogV4,
    network_id: &iroha_data_model::NetworkId,
    asset: &AssetDefinitionId,
    asset_scale: u32,
    block_height: u64,
) -> Result<Option<KagemushaRecursiveReadinessV4>, String> {
    let step_eq = select_active_kagemusha_v4_verifier(
        world,
        iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEq,
        block_height,
        "Eq",
    )?;
    let step_ep = select_active_kagemusha_v4_verifier(
        world,
        iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEp,
        block_height,
        "Ep",
    )?;
    let ((_, step_eq_record), (_, step_ep_record)) = match (step_eq, step_ep) {
        (None, None) => return Ok(None),
        (Some(step_eq), Some(step_ep)) => (step_eq, step_ep),
        _ => return Err("Kagemusha V4 Eq/Ep activation pair is incomplete".to_owned()),
    };
    let cached = catalog.resolve_activation_records(&step_eq_record, &step_ep_record)?;
    let resolved = cached.resolved();
    let manifest = resolved.release().manifest();
    if !kagemusha_v4_issuance_active_at(
        manifest.activation_height,
        manifest.withdrawal_height,
        block_height,
    ) {
        return Ok(None);
    }
    if &manifest.network_id != network_id
        || &manifest.asset != asset
        || manifest.asset_scale != asset_scale
    {
        return Err(
            "Kagemusha V4 authenticated release is not bound to the requested network/asset/scale"
                .to_owned(),
        );
    }
    let resolved_artifact_set = resolved.artifact_set();
    let lifecycle_binding = iroha_data_model::offline::KagemushaRecursiveSpendArtifactBindingV4 {
        version: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
        generation: resolved_artifact_set.generation.clone(),
        manifest_sha256: resolved_artifact_set.manifest_sha256,
    };
    if !kagemusha_release_lifecycle::issuance_enabled(world, &lifecycle_binding)? {
        return Ok(None);
    }
    let artifact_set = KagemushaAuthenticatedArtifactSetReadinessV4 {
        generation: resolved_artifact_set.generation,
        manifest_sha256: resolved_artifact_set.manifest_sha256,
        release_policy_sha256: resolved_artifact_set.release_policy_sha256,
        release_attestation_sha256: resolved_artifact_set.release_attestation_sha256,
        activation_height: resolved_artifact_set.activation_height,
        withdrawal_height: resolved_artifact_set.withdrawal_height,
        max_proof_bytes: resolved_artifact_set.max_proof_bytes,
        asset_scale: resolved_artifact_set.asset_scale,
    };
    Ok(Some(KagemushaRecursiveReadinessV4 {
        step_eq: project_kagemusha_v4_verifier(
            iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEq,
            step_eq_record,
            &artifact_set,
        ),
        step_ep: project_kagemusha_v4_verifier(
            iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEp,
            step_ep_record,
            &artifact_set,
        ),
        artifact_set,
        proof_backend_error: None,
    }))
}
fn exact_kagemusha_v4_transaction_verifier_record(
    world: &impl WorldReadOnly,
    binding: &iroha_data_model::offline::KagemushaRecursiveSpendArtifactBindingV4,
    parity: iroha_data_model::offline::KagemushaPastaCycleParityV1,
    requested_height: u64,
    current_height: u64,
) -> Result<VerifyingKeyRecord, String> {
    if requested_height == 0 || requested_height > current_height {
        return Err(
            "Kagemusha V4 requested verifier height is zero or ahead of the current block"
                .to_owned(),
        );
    }
    let circuit_id = kagemusha_v4_circuit_id(parity);
    let role = match parity {
        iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEq => "Eq",
        iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEp => "Ep",
    };
    let id = iroha_data_model::offline::kagemusha_recursive_spend_verifier_key_id_v4(
        parity,
        binding.manifest_sha256,
    );
    let record = world.verifying_keys().get(&id).cloned().ok_or_else(|| {
        format!("Kagemusha V4 {role} verifier is not registered for the selected release")
    })?;
    ensure_release_qualified_kagemusha_v4_verifier_id(&id, &record, parity, role)?;
    let circuit_key = (circuit_id.to_owned(), record.version);
    if record.version == 0
        || record.circuit_id != circuit_id
        || record.status != ConfidentialStatus::Active
        || !record.is_active_at(requested_height)
        || !record.is_active_at(current_height)
        || world.verifying_keys_by_circuit().get(&circuit_key) != Some(&id)
    {
        return Err(format!(
            "Kagemusha V4 {role} verifier is not the exact active release circuit/version"
        ));
    }
    Ok(record)
}
/// Resolve one exact transaction binding without imposing an issuance window.
///
/// The consensus Eq/Ep records, immutable startup catalog, native verifier,
/// transaction network/asset/scale, and content-addressed consensus release
/// record must all identify the same authenticated release. Callers separately
/// enforce `issuance_active` only for operations that create a new note.
pub fn resolve_kagemusha_recursive_transaction_release_v4(
    world: &impl WorldReadOnly,
    catalog: &KagemushaReleaseCatalogV4,
    binding: &iroha_data_model::offline::KagemushaRecursiveSpendArtifactBindingV4,
    requested_height: u64,
    current_height: u64,
    network_id: &iroha_data_model::NetworkId,
    asset: &AssetDefinitionId,
    asset_scale: u32,
) -> Result<KagemushaRecursiveTransactionReleaseV4, String> {
    binding
        .validate()
        .map_err(|error| format!("invalid Kagemusha V4 artifact binding: {error}"))?;
    let step_eq_record = exact_kagemusha_v4_transaction_verifier_record(
        world,
        binding,
        iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEq,
        requested_height,
        current_height,
    )?;
    let step_ep_record = exact_kagemusha_v4_transaction_verifier_record(
        world,
        binding,
        iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEp,
        requested_height,
        current_height,
    )?;
    if step_eq_record.version != step_ep_record.version {
        return Err("Kagemusha V4 Eq/Ep verifier activation versions are not atomic".to_owned());
    }
    let cached = catalog.resolve_binding(binding)?;
    cached.validate_verifier_records(&step_eq_record, &step_ep_record)?;
    let resolved = cached.resolved();
    let manifest = resolved.release().manifest();
    if &manifest.network_id != network_id
        || &manifest.asset != asset
        || manifest.asset_scale != asset_scale
    {
        return Err(
            "Kagemusha V4 authenticated release does not match the transaction network, asset, or scale"
                .to_owned(),
        );
    }
    let release_key = kagemusha_terminal_registry_v4::release_state_key(binding)?;
    let expected_record = norito::encode_canonical(cached.release_record())
        .map_err(|error| format!("failed to encode cached Kagemusha V4 release record: {error}"))?;
    if world
        .smart_contract_state()
        .get(&release_key)
        .is_none_or(|record| record != &expected_record)
    {
        return Err(
            "Kagemusha V4 consensus release record differs from local authenticated material"
                .to_owned(),
        );
    }
    let resolved_artifact_set = resolved.artifact_set();
    let artifact_set = KagemushaAuthenticatedArtifactSetReadinessV4 {
        generation: resolved_artifact_set.generation,
        manifest_sha256: resolved_artifact_set.manifest_sha256,
        release_policy_sha256: resolved_artifact_set.release_policy_sha256,
        release_attestation_sha256: resolved_artifact_set.release_attestation_sha256,
        activation_height: resolved_artifact_set.activation_height,
        withdrawal_height: resolved_artifact_set.withdrawal_height,
        max_proof_bytes: resolved_artifact_set.max_proof_bytes,
        asset_scale: resolved_artifact_set.asset_scale,
    };
    // Lifecycle corruption disables issuance but must not strand redemption of
    // notes already created under an authenticated release.
    let lifecycle_enabled =
        kagemusha_release_lifecycle::issuance_enabled(world, binding).unwrap_or(false);
    Ok(KagemushaRecursiveTransactionReleaseV4 {
        step_eq: project_kagemusha_v4_verifier(
            iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEq,
            step_eq_record,
            &artifact_set,
        ),
        step_ep: project_kagemusha_v4_verifier(
            iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEp,
            step_ep_record,
            &artifact_set,
        ),
        issuance_active: lifecycle_enabled
            && kagemusha_v4_issuance_active_at(
                artifact_set.activation_height,
                artifact_set.withdrawal_height,
                current_height,
            ),
        artifact_set,
    })
}
/// Validate an explicitly requested V4 proof release against exact material in the local cache.
///
/// This helper is for application-managed issuance/redemption proof workflows. It is not node
/// startup, health, readiness, dataspace, or asset admission: wallet-facing cash handoff support is
/// universal whether or not a release cache is configured.
pub fn ensure_kagemusha_active_release_material_v4(
    world: &impl WorldReadOnly,
    catalog: &KagemushaReleaseCatalogV4,
    block_height: u64,
) -> Result<(), String> {
    let mut covered_manifest_digests = BTreeSet::new();
    visit_active_kagemusha_v4_release_pairs(
        world,
        block_height,
        |step_eq_record, step_ep_record| {
            let cached = catalog.resolve_activation_records(step_eq_record, step_ep_record)?;
            let release = cached.resolved().release();
            let binding = iroha_data_model::offline::KagemushaRecursiveSpendArtifactBindingV4 {
                version: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
                generation: release.manifest().generation.clone(),
                manifest_sha256: release.manifest_sha256(),
            };
            covered_manifest_digests.insert(binding.manifest_sha256);
            let release_key = kagemusha_terminal_registry_v4::release_state_key(&binding)?;
            let expected = norito::encode_canonical(cached.release_record()).map_err(|error| {
                format!("failed to encode Kagemusha V4 release record: {error}")
            })?;
            if world
                .smart_contract_state()
                .get(&release_key)
                .is_none_or(|actual| actual != &expected)
            {
                return Err(
                    "active Kagemusha V4 consensus release record is absent or differs from local cache"
                        .to_owned(),
                );
            }
            Ok(())
        },
    )?;
    if covered_manifest_digests.is_empty() {
        return Err(
            "no active authenticated ABI-21/V4 Kagemusha Eq/Ep release is installed".to_owned(),
        );
    }
    for (key, payload) in world.smart_contract_state().iter() {
        let Some((binding, release_record)) =
            decode_kagemusha_v4_consensus_release_state(key, payload)?
        else {
            continue;
        };
        if !covered_manifest_digests.contains(&binding.manifest_sha256) {
            return Err(
                "Kagemusha V4 consensus release state has no terminal Active Eq/Ep pair".to_owned(),
            );
        }
        let cached = catalog.resolve_binding(&binding)?;
        if cached.release_record() != &release_record {
            return Err(
                "Kagemusha V4 consensus release record differs from local authenticated material"
                    .to_owned(),
            );
        }
    }
    Ok(())
}
#[cfg(test)]
#[path = "offline/recursive_readiness_tests.rs"]
mod recursive_readiness_tests;
fn resolve_offline_escrow_account(
    state_transaction: &mut StateTransaction<'_, '_>,
    definition: &AssetDefinitionId,
) -> Result<AccountId, Error> {
    let asset_definition = state_transaction.world.asset_definition(definition)?;
    // Offline support is a protocol primitive, not an asset enrollment mode.
    // Materialize the deterministic escrow only when an offline instruction
    // actually needs it. This keeps ordinary asset registration free of
    // offline side effects and removes any process-local catalog dependency.
    crate::smartcontracts::isi::domain::isi::ensure_offline_escrow_account(
        &asset_definition,
        asset_definition.owned_by(),
        state_transaction,
    )?;
    let derived = crate::smartcontracts::isi::domain::isi::offline_escrow_account_id(
        state_transaction.network_id(),
        definition,
    );
    Ok(derived)
}
pub(crate) fn is_offline_escrow_source_asset(
    state_transaction: &StateTransaction<'_, '_>,
    source_id: &AssetId,
) -> Result<bool, Error> {
    state_transaction
        .world
        .asset_definition(source_id.definition())?;
    let derived = crate::smartcontracts::isi::domain::isi::offline_escrow_account_id(
        state_transaction.network_id(),
        source_id.definition(),
    );
    Ok(&derived == source_id.account())
}
fn ensure_distinct_offline_escrow_account(
    escrow_account: &AccountId,
    participant_account: &AccountId,
    participant_role: &str,
    definition_id: &AssetDefinitionId,
) -> Result<(), Error> {
    if escrow_account == participant_account {
        return Err(labeled_invariant(
            "escrow_self_reference",
            format!(
                "offline escrow account for asset definition `{definition_id}` must be distinct from {participant_role} account `{participant_account}`",
            ),
        )
        .into());
    }
    Ok(())
}
fn canonical_kagemusha_asset_id(
    state_transaction: &StateTransaction<'_, '_>,
    asset: &AssetId,
) -> Result<AssetId, Error> {
    let definition = state_transaction
        .world
        .asset_definition(asset.definition())?;
    let scope = match definition.balance_scope_policy() {
        AssetBalancePolicy::Global => {
            if !matches!(asset.scope(), AssetBalanceScope::Global) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "global assets cannot be addressed with dataspace scope".into(),
                )
                .into());
            }
            AssetBalanceScope::Global
        }
        AssetBalancePolicy::DataspaceRestricted => match asset.scope() {
            AssetBalanceScope::Dataspace(dataspace) => AssetBalanceScope::Dataspace(*dataspace),
            AssetBalanceScope::Global => state_transaction
                .world
                .resolve_asset_balance_scope(asset.definition())?,
        },
    };
    Ok(AssetId::with_scope(
        asset.definition().clone(),
        asset.account().clone(),
        scope,
    ))
}
fn kagemusha_escrow_asset_id(source_asset: &AssetId, escrow_account: AccountId) -> AssetId {
    AssetId::with_scope(
        source_asset.definition().clone(),
        escrow_account,
        source_asset.scope().clone(),
    )
}
fn reserve_kagemusha_escrow(
    state_transaction: &mut StateTransaction<'_, '_>,
    source_authority: &AccountId,
    operation_id: [u8; 32],
    asset: &AssetId,
    amount: &Quantity,
) -> Result<(), Error> {
    let escrow_account = resolve_offline_escrow_account(state_transaction, asset.definition())?;
    if amount.is_zero() {
        return Ok(());
    }
    ensure_distinct_offline_escrow_account(
        &escrow_account,
        asset.account(),
        "note",
        asset.definition(),
    )?;
    let source_asset = canonical_kagemusha_asset_id(state_transaction, asset)?;
    let escrow_asset = kagemusha_escrow_asset_id(&source_asset, escrow_account);
    let authorization = VerifiedKagemushaTopUpDebit::new(
        source_authority.clone(),
        operation_id,
        source_asset,
        escrow_asset,
        amount.clone(),
    );
    crate::smartcontracts::isi::asset::isi::execute_verified_offline_top_up_transfer(
        state_transaction,
        authorization,
    )?;
    Ok(())
}
/// Execution logic for Kagemusha offline-cash instructions.
pub mod isi {
    use super::*;
    const KAGEMUSHA_DEVICE_REGISTRATION_DOMAIN: &str = "kagemusha-device-registration";
    const KAGEMUSHA_ATTESTATION_CHALLENGE_REPLAY_DOMAIN: &str = "kagemusha-attestation-challenge";
    const KAGEMUSHA_ATTESTATION_REPORT_REPLAY_DOMAIN: &str = "kagemusha-attestation-report";
    const KAGEMUSHA_ATTESTATION_EVIDENCE_REPLAY_DOMAIN: &str = "kagemusha-attestation-evidence";
    // Online registrations use one native-authored, read-only first-release namespace.
    const KAGEMUSHA_ONLINE_REGISTRATION_STATE_PREFIX: &str = "kagemusha_online_registration_";
    const KAGEMUSHA_ONLINE_REGISTRATION_STATE_VERSION_V4: u16 = 4;
    const KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_CANONICAL_BYTES_V4: usize = 4 * 1024;
    const KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_RETAINED_V4: usize =
        2 * KAGEMUSHA_ACTIVE_DEVICE_REGISTRATIONS_MAX_GLOBAL_V1;
    const KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_RETAINED_PER_ACCOUNT_V4: usize =
        2 * KAGEMUSHA_ACTIVE_DEVICE_REGISTRATIONS_MAX_PER_ACCOUNT_V1;
    const KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_TOTAL_BYTES_V4: usize =
        KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_CANONICAL_BYTES_V4
            * KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_RETAINED_V4;
    const _: () =
        assert!(KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_TOTAL_BYTES_V4 <= 4 * 1024 * 1024);
    const KAGEMUSHA_V4_OPERATION_DOMAIN: &str = "kagemusha-v4-operation";
    const KAGEMUSHA_V4_NONCE_DOMAIN: &str = "kagemusha-v4-authorization-nonce";
    const KAGEMUSHA_V4_PAYLOAD_DOMAIN: &str = "kagemusha-v4-payload";
    const KAGEMUSHA_V4_REQUEST_DOMAIN: &str = "kagemusha-v4-request";
    const KAGEMUSHA_V4_BRANCH_EXACT_DOMAIN: &str = "kagemusha-v4-redeemed-branch";
    const KAGEMUSHA_V4_BRANCH_DESCENDANT_DOMAIN: &str = "kagemusha-v4-redeemed-descendant";
    const KAGEMUSHA_V4_TRANSITION_SELECTED_DOMAIN: &str = "kagemusha-v4-transition-selected";
    const KAGEMUSHA_V4_TRANSITION_CHOICE_DOMAIN: &str = "kagemusha-v4-transition-choice";
    const KAGEMUSHA_V4_AUTHORIZED_CHANGE_CHILD_DOMAIN: &str =
        "kagemusha-v4-authorized-change-child";
    const OFFLINE_ATTESTATION_EVIDENCE_PREFIX: &[u8] = b"offline-device-attestation-evidence-v1";
    const KAGEMUSHA_ATTESTATION_RECENT_BLOCK_WINDOW: u64 = 128;
    const OFFLINE_ATTESTATION_PLATFORM_IOS_APP_ATTEST: &str = "ios-appattest";
    const OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT: &str = "android-keymint";
    const OFFLINE_ATTESTATION_IOS_ASSERTION_SCHEME: &str = "apple-appattest-counter-v1";
    const OFFLINE_ATTESTATION_IOS_ASSERTION_ALGORITHM: &str = "app-attest-p256";
    const OFFLINE_ATTESTATION_ANDROID_ASSERTION_SCHEME: &str =
        "android-keymint-ecdsa-p256-usage-limit-v1";
    const OFFLINE_ATTESTATION_ANDROID_ASSERTION_ALGORITHM: &str = "ecdsa-p256-sha256";
    const OFFLINE_ATTESTATION_P256_UNCOMPRESSED_PUBLIC_KEY_LEN: usize = 65;
    const OFFLINE_ATTESTATION_MAX_REPORT_BYTES: usize = 64 * 1024;
    const OFFLINE_ATTESTATION_MAX_EVIDENCE_BYTES: usize = 128 * 1024;
    const OFFLINE_ATTESTATION_MAX_X509_CERTIFICATE_BYTES: usize = 16 * 1024;
    const OFFLINE_ATTESTATION_MAX_X509_CHAIN_CERTIFICATES: usize = 8;
    const OFFLINE_ATTESTATION_MAX_IOS_X509_CHAIN_CERTIFICATES: usize = 4;
    const OFFLINE_ATTESTATION_APP_ATTEST_AUTH_DATA_MIN_LEN: usize = 37 + 16 + 2;
    const OFFLINE_ATTESTATION_APP_ATTEST_AUTH_DATA_MAX_LEN: usize = 8 * 1024;
    const OFFLINE_ATTESTATION_APP_ATTEST_FLAG_USER_PRESENT: u8 = 0x01;
    const OFFLINE_ATTESTATION_APP_ATTEST_FLAG_USER_VERIFIED: u8 = 0x04;
    const OFFLINE_ATTESTATION_APP_ATTEST_FLAG_ATTESTED_CREDENTIAL_DATA: u8 = 0x40;
    const OFFLINE_ATTESTATION_APP_ATTEST_FLAG_EXTENSION_DATA: u8 = 0x80;
    const OFFLINE_ATTESTATION_APP_ATTEST_NONCE_OID: &str = "1.2.840.113635.100.8.2";
    const OFFLINE_ATTESTATION_ANDROID_KEY_OID: &str = "1.3.6.1.4.1.11129.2.1.17";
    const OFFLINE_ATTESTATION_IOS_ENV_PRODUCTION: &str = "production";
    const OFFLINE_ATTESTATION_IOS_ENV_DEVELOPMENT: &str = "development";
    const OFFLINE_ATTESTATION_IOS_AAGUID_PRODUCTION: &[u8; 16] = b"appattest\0\0\0\0\0\0\0";
    const OFFLINE_ATTESTATION_IOS_AAGUID_DEVELOPMENT: &[u8; 16] = b"appattestdevelop";
    const OFFLINE_ATTESTATION_ANDROID_SECURITY_LEVEL_TRUSTED_ENVIRONMENT: i64 = 1;
    const OFFLINE_ATTESTATION_ANDROID_SECURITY_LEVEL_STRONG_BOX: i64 = 2;
    const OFFLINE_ATTESTATION_ANDROID_TAG_USAGE_COUNT_LIMIT: u32 = 405;
    const OFFLINE_ATTESTATION_ANDROID_TAG_ALL_APPLICATIONS: u32 = 600;
    const OFFLINE_ATTESTATION_ANDROID_TAG_ROOT_OF_TRUST: u32 = 704;
    const OFFLINE_ATTESTATION_ANDROID_TAG_OS_VERSION: u32 = 705;
    const OFFLINE_ATTESTATION_ANDROID_TAG_OS_PATCH_LEVEL: u32 = 706;
    const OFFLINE_ATTESTATION_ANDROID_TAG_ATTESTATION_APPLICATION_ID: u32 = 709;
    const OFFLINE_ATTESTATION_ANDROID_TAG_ATTESTATION_ID_BRAND: u32 = 710;
    const OFFLINE_ATTESTATION_ANDROID_TAG_ATTESTATION_ID_DEVICE: u32 = 711;
    const OFFLINE_ATTESTATION_ANDROID_TAG_ATTESTATION_ID_PRODUCT: u32 = 712;
    const OFFLINE_ATTESTATION_ANDROID_TAG_ATTESTATION_ID_MANUFACTURER: u32 = 716;
    const OFFLINE_ATTESTATION_ANDROID_TAG_ATTESTATION_ID_MODEL: u32 = 717;
    const OFFLINE_ATTESTATION_ANDROID_TAG_VENDOR_PATCH_LEVEL: u32 = 718;
    const OFFLINE_ATTESTATION_ANDROID_TAG_BOOT_PATCH_LEVEL: u32 = 719;
    const OFFLINE_ATTESTATION_ANDROID_VERIFIED_BOOT_STATE_VERIFIED: i64 = 0;
    include!("offline/device_attestation_roots.rs");
    struct IosAppAttestReport {
        auth_data: Vec<u8>,
        certificates: Vec<Vec<u8>>,
    }
    struct IosAppAttestAuthData {
        rp_id_hash: [u8; 32],
        sign_count: u32,
        aaguid: [u8; 16],
        credential_id: Vec<u8>,
        cose_key: Vec<u8>,
        extensions: IosAppAttestExtensionProperties,
    }
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct IosAppAttestExtensionProperties {
        validation_category: u32,
        bundle_version: String,
    }
    struct IosAppAttestAssertionAuthData {
        rp_id_hash: [u8; 32],
        sign_count: u32,
        extensions: IosAppAttestExtensionProperties,
    }
    struct AndroidKeyMintReport {
        certificates: Vec<Vec<u8>>,
    }
    struct AndroidKeyDescription {
        attestation_challenge: Vec<u8>,
        usage_count_limit: Option<i64>,
        all_applications: bool,
        application_id: Option<AndroidAttestationApplicationId>,
        device_properties: OfflineAndroidAttestedDevicePropertiesV2,
    }
    struct AndroidRootOfTrust {
        verified_boot_key: Vec<u8>,
        verified_boot_hash: [u8; 32],
    }
    #[derive(Default)]
    struct AndroidAuthorizationList {
        usage_count_limit: Option<i64>,
        all_applications: bool,
        application_id: Option<AndroidAttestationApplicationId>,
        root_of_trust: Option<AndroidRootOfTrust>,
        os_version: Option<i64>,
        os_patch_level: Option<i64>,
        brand: Option<String>,
        device: Option<String>,
        product: Option<String>,
        manufacturer: Option<String>,
        model: Option<String>,
        vendor_patch_level: Option<i64>,
        boot_patch_level: Option<i64>,
    }
    struct AndroidAttestationApplicationId {
        packages: Vec<AndroidAttestationPackageInfo>,
        signature_digests: Vec<Vec<u8>>,
    }
    struct AndroidAttestationPackageInfo {
        package_name: String,
    }
    #[derive(Copy, Clone)]
    struct DerTag {
        class_bits: u8,
        constructed: bool,
        number: u32,
        first_byte: u8,
    }
    struct DerReader<'a> {
        input: &'a [u8],
        offset: usize,
    }
    impl<'a> DerReader<'a> {
        fn new(input: &'a [u8]) -> Self {
            Self { input, offset: 0 }
        }
        fn sequence(input: &'a [u8]) -> Result<Self, Error> {
            let mut reader = Self::new(input);
            let sequence = reader.read_expected(0x30)?;
            reject_invalid_attestation!(
                reader.has_remaining(),
                "attestation extension has trailing DER bytes",
            );
            Ok(Self::new(sequence))
        }
        fn has_remaining(&self) -> bool {
            self.offset < self.input.len()
        }
        fn read_expected(&mut self, expected_tag: u8) -> Result<&'a [u8], Error> {
            let (tag, value) = self.read_tlv()?;
            reject_invalid_attestation!(
                tag != expected_tag,
                "attestation extension has an unexpected DER tag",
            );
            Ok(value)
        }
        fn read_single_expected(&mut self, expected_tag: u8) -> Result<&'a [u8], Error> {
            let value = self.read_expected(expected_tag)?;
            if self.has_remaining() {
                return Err(invalid_attestation(
                    "attestation extension DER has trailing inner bytes",
                )
                .into());
            }
            Ok(value)
        }
        fn read_null(&mut self) -> Result<(), Error> {
            let value = self.read_single_expected(0x05)?;
            if !value.is_empty() {
                return Err(
                    invalid_attestation("attestation extension DER NULL must be empty").into(),
                );
            }
            Ok(())
        }
        fn read_integer(&mut self) -> Result<i64, Error> {
            der_integer_to_i64(self.read_expected(0x02)?)
        }
        fn read_enumerated(&mut self) -> Result<i64, Error> {
            der_integer_to_i64(self.read_expected(0x0A)?)
        }
        fn read_octet_string(&mut self) -> Result<Vec<u8>, Error> {
            Ok(self.read_expected(0x04)?.to_vec())
        }
        fn read_sequence_bytes(&mut self) -> Result<Vec<u8>, Error> {
            Ok(self.read_expected(0x30)?.to_vec())
        }
        fn read_tlv(&mut self) -> Result<(u8, &'a [u8]), Error> {
            let (tag, value) = self.read_tlv_full()?;
            if tag.number >= 31 || tag.first_byte & 0x1F == 0x1F {
                return Err(invalid_attestation(
                    "attestation extension DER high-tag form is unsupported in this position",
                )
                .into());
            }
            Ok((tag.first_byte, value))
        }
        fn read_tlv_full(&mut self) -> Result<(DerTag, &'a [u8]), Error> {
            let (tag, value, _) = self.read_tlv_full_with_raw()?;
            Ok((tag, value))
        }
        fn read_tlv_full_with_raw(&mut self) -> Result<(DerTag, &'a [u8], &'a [u8]), Error> {
            if self.offset >= self.input.len() {
                return Err(invalid_attestation("attestation extension DER ended early").into());
            }
            let start = self.offset;
            let first_byte = self.input[self.offset];
            self.offset += 1;
            let mut number = u32::from(first_byte & 0x1F);
            if number == 0x1F {
                number = 0;
                let mut octets = 0usize;
                let mut first_high_tag_octet = true;
                loop {
                    if self.offset >= self.input.len() || octets >= 5 {
                        return Err(invalid_attestation(
                            "attestation extension DER high-tag number is invalid",
                        )
                        .into());
                    }
                    let byte = self.input[self.offset];
                    self.offset += 1;
                    octets += 1;
                    if first_high_tag_octet && byte & 0x7F == 0 {
                        return Err(invalid_attestation(
                            "attestation extension DER high-tag number is non-canonical",
                        )
                        .into());
                    }
                    first_high_tag_octet = false;
                    number = number
                        .checked_mul(128)
                        .and_then(|number| number.checked_add(u32::from(byte & 0x7F)))
                        .ok_or_else(|| {
                            invalid_attestation(
                                "attestation extension DER high-tag number is invalid",
                            )
                        })?;
                    if byte & 0x80 == 0 {
                        break;
                    }
                }
                if number < 31 {
                    return Err(invalid_attestation(
                        "attestation extension DER high-tag number is non-canonical",
                    )
                    .into());
                }
            }
            let length = self.read_length()?;
            let end = self
                .offset
                .checked_add(length)
                .ok_or_else(|| invalid_attestation("attestation extension DER length overflow"))?;
            if end > self.input.len() {
                return Err(
                    invalid_attestation("attestation extension DER length exceeds input").into(),
                );
            }
            let value = &self.input[self.offset..end];
            self.offset = end;
            let raw = &self.input[start..end];
            Ok((
                DerTag {
                    class_bits: first_byte & 0xC0,
                    constructed: first_byte & 0x20 != 0,
                    number,
                    first_byte,
                },
                value,
                raw,
            ))
        }
        fn read_length(&mut self) -> Result<usize, Error> {
            if self.offset >= self.input.len() {
                return Err(
                    invalid_attestation("attestation extension DER length is missing").into(),
                );
            }
            let first = self.input[self.offset];
            self.offset += 1;
            if first & 0x80 == 0 {
                return Ok(usize::from(first));
            }
            let octets = usize::from(first & 0x7F);
            if octets == 0 || octets > 4 || self.offset + octets > self.input.len() {
                return Err(invalid_attestation(
                    "attestation extension DER length encoding is unsupported",
                )
                .into());
            }
            let first_length_octet = self.input[self.offset];
            if first_length_octet == 0 || (octets == 1 && first_length_octet < 0x80) {
                return Err(invalid_attestation(
                    "attestation extension DER length encoding is non-canonical",
                )
                .into());
            }
            let mut length = 0usize;
            for _ in 0..octets {
                length = (length << 8) | usize::from(self.input[self.offset]);
                self.offset += 1;
            }
            Ok(length)
        }
    }
    fn der_integer_to_i64(bytes: &[u8]) -> Result<i64, Error> {
        if bytes.is_empty() || bytes.len() > 8 {
            return Err(
                invalid_attestation("attestation extension integer is out of range").into(),
            );
        }
        if bytes.len() > 1
            && ((bytes[0] == 0 && bytes[1] & 0x80 == 0)
                || (bytes[0] == 0xFF && bytes[1] & 0x80 != 0))
        {
            return Err(invalid_attestation(
                "attestation extension integer encoding is non-canonical",
            )
            .into());
        }
        if bytes[0] & 0x80 != 0 {
            return Err(
                invalid_attestation("attestation extension integer is out of range").into(),
            );
        }
        let mut value = 0i64;
        for byte in bytes {
            value = (value << 8) | i64::from(*byte);
        }
        Ok(value)
    }
    fn sha256_bytes(bytes: &[u8]) -> [u8; 32] {
        Sha256::digest(bytes).into()
    }
    fn sha256_concat(left: &[u8], right: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(left);
        hasher.update(right);
        hasher.finalize().into()
    }
    fn decode_trusted_root_der(root_b64: &str) -> Result<Vec<u8>, Error> {
        BASE64_STANDARD.decode(root_b64).map_err(|_| {
            labeled_invariant("invalid_attestation", "trusted root DER is invalid").into()
        })
    }
    /// Build the canonical fail-closed production device policy used by a
    /// Kagemusha release activation.
    ///
    /// The platform roots are the built-in Apple App Attest and Android KeyMint roots used by the
    /// native verifier. App identities and signing digests remain explicit operator input; this
    /// helper never invents or relaxes them.
    ///
    /// # Errors
    ///
    /// Returns an error when an app identity, allowlist, signing digest, or
    /// built-in root does not satisfy the production activation policy.
    #[allow(clippy::too_many_arguments)]
    pub fn production_offline_device_attestation_policy_v1(
        ios_team_id: String,
        ios_bundle_id: String,
        ios_validation_categories: Vec<u32>,
        ios_bundle_versions: Vec<String>,
        android_package_name: String,
        android_signing_certificate_sha256: Vec<[u8; 32]>,
        android_status_snapshot: OfflineAndroidAttestationStatusSnapshotV1,
        evaluation_time_ms: u64,
    ) -> Result<OfflineDeviceAttestationPolicy, String> {
        production_offline_device_attestation_policy_v2(
            1,
            ios_team_id,
            ios_bundle_id,
            ios_validation_categories,
            ios_bundle_versions,
            android_package_name,
            android_signing_certificate_sha256,
            android_status_snapshot,
            Vec::new(),
            evaluation_time_ms,
        )
    }
    /// Build a canonical production policy with a monotonic epoch and reviewed
    /// Android vulnerability rules.
    ///
    /// # Errors
    ///
    /// Returns an error when an identity, allowlist, rule, status snapshot, or
    /// built-in production trust root fails native activation validation.
    #[allow(clippy::too_many_arguments)]
    pub fn production_offline_device_attestation_policy_v2(
        policy_epoch: u64,
        ios_team_id: String,
        ios_bundle_id: String,
        mut ios_validation_categories: Vec<u32>,
        mut ios_bundle_versions: Vec<String>,
        android_package_name: String,
        mut android_signing_certificate_sha256: Vec<[u8; 32]>,
        android_status_snapshot: OfflineAndroidAttestationStatusSnapshotV1,
        mut android_vulnerability_rules: Vec<OfflineAndroidDeviceVulnerabilityRuleV2>,
        evaluation_time_ms: u64,
    ) -> Result<OfflineDeviceAttestationPolicy, String> {
        let original_category_count = ios_validation_categories.len();
        ios_validation_categories.sort_unstable();
        ios_validation_categories.dedup();
        if ios_validation_categories.len() != original_category_count {
            return Err("iOS validation categories must not contain duplicates".to_owned());
        }
        let original_version_count = ios_bundle_versions.len();
        ios_bundle_versions.sort();
        ios_bundle_versions.dedup();
        if ios_bundle_versions.len() != original_version_count {
            return Err("iOS bundle versions must not contain duplicates".to_owned());
        }
        let original_signer_count = android_signing_certificate_sha256.len();
        android_signing_certificate_sha256.sort_unstable();
        android_signing_certificate_sha256.dedup();
        if android_signing_certificate_sha256.len() != original_signer_count {
            return Err(
                "Android signing certificate digests must not contain duplicates".to_owned(),
            );
        }
        let original_rule_count = android_vulnerability_rules.len();
        android_vulnerability_rules.sort_by(|left, right| left.rule_id.cmp(&right.rule_id));
        android_vulnerability_rules.dedup_by(|left, right| left.rule_id == right.rule_id);
        if android_vulnerability_rules.len() != original_rule_count {
            return Err(
                "Android vulnerability rule identifiers must not contain duplicates".to_owned(),
            );
        }
        let policy = OfflineDeviceAttestationPolicy {
            version: OFFLINE_DEVICE_ATTESTATION_POLICY_VERSION_V2,
            policy_epoch,
            trusted_roots: vec![
                OfflineDeviceAttestationTrustedRoot {
                    platform: OFFLINE_ATTESTATION_PLATFORM_IOS_APP_ATTEST.to_owned(),
                    der: decode_trusted_root_der(APPLE_APP_ATTESTATION_ROOT_CA_DER_B64)
                        .map_err(|error| error.to_string())?,
                    not_before_ms: None,
                    not_after_ms: None,
                },
                OfflineDeviceAttestationTrustedRoot {
                    platform: OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT.to_owned(),
                    der: decode_trusted_root_der(ANDROID_KEY_ATTESTATION_ROOT_CA_DER_B64)
                        .map_err(|error| error.to_string())?,
                    not_before_ms: None,
                    not_after_ms: None,
                },
                OfflineDeviceAttestationTrustedRoot {
                    platform: OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT.to_owned(),
                    der: decode_trusted_root_der(ANDROID_KEY_ATTESTATION_CA_DER_B64)
                        .map_err(|error| error.to_string())?,
                    not_before_ms: None,
                    not_after_ms: None,
                },
            ],
            revoked_certificate_tbs_sha256: Vec::new(),
            ios_apps: vec![OfflineIosAppAttestationPolicy {
                team_id: ios_team_id,
                bundle_id: ios_bundle_id,
                environment: OFFLINE_ATTESTATION_IOS_ENV_PRODUCTION.to_owned(),
                allowed_validation_categories: ios_validation_categories,
                allowed_bundle_versions: ios_bundle_versions,
            }],
            android_apps: vec![OfflineAndroidAppAttestationPolicy {
                package_name: android_package_name,
                signing_certificate_sha256: android_signing_certificate_sha256
                    .into_iter()
                    .map(|digest| digest.to_vec())
                    .collect(),
            }],
            android_status_snapshot: Some(android_status_snapshot),
            android_vulnerability_rules,
            require_ios_app_policy: true,
            require_android_app_policy: true,
        };
        validate_offline_attestation_policy_for_release_activation(&policy, evaluation_time_ms)
            .map_err(|error| error.to_string())?;
        Ok(policy)
    }
    #[cfg(test)]
    pub(super) fn default_offline_device_attestation_policy()
    -> Result<OfflineDeviceAttestationPolicy, Error> {
        Ok(OfflineDeviceAttestationPolicy {
            version: OFFLINE_DEVICE_ATTESTATION_POLICY_VERSION_V2,
            policy_epoch: 1,
            trusted_roots: vec![
                OfflineDeviceAttestationTrustedRoot {
                    platform: OFFLINE_ATTESTATION_PLATFORM_IOS_APP_ATTEST.to_owned(),
                    der: decode_trusted_root_der(APPLE_APP_ATTESTATION_ROOT_CA_DER_B64)?,
                    not_before_ms: None,
                    not_after_ms: None,
                },
                OfflineDeviceAttestationTrustedRoot {
                    platform: OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT.to_owned(),
                    der: decode_trusted_root_der(ANDROID_KEY_ATTESTATION_ROOT_CA_DER_B64)?,
                    not_before_ms: None,
                    not_after_ms: None,
                },
                OfflineDeviceAttestationTrustedRoot {
                    platform: OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT.to_owned(),
                    der: decode_trusted_root_der(ANDROID_KEY_ATTESTATION_CA_DER_B64)?,
                    not_before_ms: None,
                    not_after_ms: None,
                },
            ],
            revoked_certificate_tbs_sha256: Vec::new(),
            ios_apps: Vec::new(),
            android_apps: Vec::new(),
            android_status_snapshot: None,
            android_vulnerability_rules: Vec::new(),
            require_ios_app_policy: false,
            require_android_app_policy: false,
        })
    }
    fn effective_offline_device_attestation_policy(
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<OfflineDeviceAttestationPolicy, Error> {
        match state_transaction
            .world
            .smart_contract_state
            .get(&*OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY)
        {
            Some(bytes) => {
                let policy = norito::decode_canonical::<OfflineDeviceAttestationPolicy>(bytes)
                    .map_err(|err| {
                        labeled_invariant(
                            "invalid_attestation_policy",
                            format!("failed to decode Offline device attestation policy: {err}"),
                        )
                    })?;
                Ok(policy)
            }
            None => Err(labeled_invariant(
                "invalid_attestation_policy",
                "Offline device attestation policy is not installed; hardware-backed offline operations are disabled",
            )
            .into()),
        }
    }
    include!("offline/attestation_policy_validation.rs");
    include!("offline/attestation_certificate_der_profile.rs");
    include!("offline/attestation_certificate_validation.rs");
    include!("offline/topup_shield_verifier_resolution.rs");
    fn resolve_kagemusha_unshield_verifier(
        asset: &AssetDefinitionId,
        proof: &ProofAttachment,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(VerifyingKeyBox, VerifyingKeyRecord), Error> {
        ensure_kagemusha_transparent_attachment(proof)?;
        let zk_state = state_transaction
            .world
            .zk_assets
            .get(asset)
            .ok_or_else(|| {
                labeled_invariant(
                    "verifier_key_invalid",
                    "recursive Kagemusha redemption requires configured shielded asset state",
                )
            })?;
        let binding = zk_state.vk_unshield.as_ref().ok_or_else(|| {
            labeled_invariant(
                "verifier_key_invalid",
                "recursive Kagemusha redemption requires a bound unshield verifier key",
            )
        })?;
        if proof.vk_ref != binding.id || proof.backend != binding.id.backend {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "recursive Kagemusha redeem proof must reference the asset-bound unshield verifier key",
            )
            .into());
        }
        let Some(commitment) = proof.vk_commitment else {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "recursive Kagemusha redeem proof must publish the asset-bound verifier-key commitment",
            )
            .into());
        };
        if commitment == [0u8; 32] {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "recursive Kagemusha redeem proof must publish a non-zero asset-bound verifier-key commitment",
            )
            .into());
        }
        if commitment != binding.commitment {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "recursive Kagemusha redeem verifier-key commitment does not match the asset binding",
            )
            .into());
        }
        let record = state_transaction
            .world
            .verifying_keys
            .get(&binding.id)
            .cloned()
            .ok_or_else(|| {
                labeled_invariant(
                    "verifier_key_invalid",
                    "recursive Kagemusha redeem verifier key is not registered",
                )
            })?;
        if !record.is_active_at(state_transaction.block_height()) {
            return Err(labeled_invariant(
                "verifier_key_inactive",
                "recursive Kagemusha redeem verifier key is not active at the current block height",
            )
            .into());
        }
        if record.namespace != crate::zk::KAGEMUSHA_VERIFIER_NAMESPACE {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "recursive Kagemusha redeem verifier key is not in the Kagemusha namespace",
            )
            .into());
        }
        if record.commitment != binding.commitment {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "recursive Kagemusha redeem verifier-key registry commitment does not match the asset binding",
            )
            .into());
        }
        if record.backend != BackendTag::Halo2IpaPasta
            || proof.backend.as_str() != crate::zk::ZK_BACKEND_HALO2_IPA
        {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "recursive Kagemusha redemption requires a transparent Halo2/IPA unshield verifier",
            )
            .into());
        }
        if record.curve != "pallas" {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "recursive Kagemusha redeem verifier key curve is not pallas",
            )
            .into());
        }
        if !crate::zk::confidential_v2::is_confidential_unshield_v3_circuit_id(&record.circuit_id) {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "recursive Kagemusha redemption requires a confidential unshield v3 verifier",
            )
            .into());
        }
        let circuit_key = (record.circuit_id.clone(), record.version);
        match state_transaction
            .world
            .verifying_keys_by_circuit
            .get(&circuit_key)
        {
            Some(active_id) if active_id == &binding.id => {}
            _ => {
                return Err(labeled_invariant(
                    "verifier_key_inactive",
                    "recursive Kagemusha redeem verifier circuit/version is not active",
                )
                .into());
            }
        }
        if record.max_proof_bytes == 0 {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "recursive Kagemusha redeem verifier key must publish a non-zero max_proof_bytes cap",
            )
            .into());
        }
        if proof.proof.bytes.len() > record.max_proof_bytes as usize {
            return Err(labeled_invariant(
                "invalid_proof",
                "recursive Kagemusha redeem proof exceeds verifier record max_proof_bytes",
            )
            .into());
        }
        let vk_box = record.key.clone().ok_or_else(|| {
            labeled_invariant(
                "verifier_key_invalid",
                "recursive Kagemusha redeem verifier key is not available inline",
            )
        })?;
        if vk_box.backend.as_str() != crate::zk::ZK_BACKEND_HALO2_IPA
            || vk_box.bytes.is_empty()
            || u32::try_from(vk_box.bytes.len()).ok() != Some(record.vk_len)
            || crate::zk::hash_vk(&vk_box) != record.commitment
        {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "recursive Kagemusha redeem inline verifier key does not match the registry record",
            )
            .into());
        }
        crate::zk::confidential_v2::ensure_confidential_unshield_v3_canonical_vk_box(&vk_box)
            .map_err(|err| labeled_invariant("verifier_key_invalid", err))?;
        let envelope = decode_canonical_offline_proof_envelope(
            &proof.proof.bytes,
            "recursive Kagemusha redeem proof must be a canonical OpenVerifyEnvelope",
        )?;
        if envelope.vk_hash == [0u8; 32] {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "recursive Kagemusha redeem proof envelope verifier-key hash must be non-zero",
            )
            .into());
        }
        if envelope.backend != BackendTag::Halo2IpaPasta
            || envelope.circuit_id != record.circuit_id
            || envelope.vk_hash != record.commitment
            || !envelope.aux.is_empty()
        {
            return Err(labeled_invariant(
                "invalid_proof",
                "recursive Kagemusha redeem proof envelope metadata mismatch",
            )
            .into());
        }
        let expected_schema =
            crate::zk::confidential_v2::CONFIDENTIAL_UNSHIELD_V3_PUBLIC_INPUTS_SCHEMA_V1;
        let expected_schema_hash: [u8; 32] = Hash::new(expected_schema).into();
        if envelope.public_inputs != expected_schema
            || record.public_inputs_schema_hash != expected_schema_hash
        {
            return Err(labeled_invariant(
                "verifier_schema_mismatch",
                "recursive Kagemusha redeem proof public-input schema mismatch",
            )
            .into());
        }
        if let Some(envelope_hash) = proof.envelope_hash {
            let expected_hash: [u8; 32] = Hash::new(&proof.proof.bytes).into();
            if envelope_hash != expected_hash {
                return Err(labeled_invariant(
                    "invalid_proof",
                    "recursive Kagemusha redeem proof envelope hash does not match the submitted envelope",
                )
                .into());
            }
        }
        Ok((vk_box, record))
    }
    fn ensure_kagemusha_v4_unshield_verifier_window(
        record: &VerifyingKeyRecord,
        requested_height: u64,
        current_height: u64,
    ) -> Result<(), Error> {
        if requested_height == 0
            || requested_height > current_height
            || !record.is_active_at(requested_height)
            || !record.is_active_at(current_height)
        {
            return Err(labeled_invariant(
                "verifier_key_inactive",
                "Kagemusha V4 unshield verifier is outside its requested or current activation window",
            )
            .into());
        }
        Ok(())
    }
    fn ensure_kagemusha_v4_redeem_public_inputs(
        request: &iroha_data_model::offline::KagemushaRecursiveSpendRedeemRequestV4,
        state_transaction: &StateTransaction<'_, '_>,
        vk_record: &VerifyingKeyRecord,
    ) -> Result<(), Error> {
        if !crate::zk::confidential_v2::is_confidential_unshield_v3_circuit_id(
            &vk_record.circuit_id,
        ) {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha V4 redemption requires an unshield-v3 proof attachment",
            )
            .into());
        }
        let statement = &request.bundle.statement;
        let zero = [0u8; 32];
        let expected_change = request
            .redemption
            .change_output
            .as_ref()
            .map_or(zero, |change| change.note_commitment);
        let (
            input_commitments,
            proof_nullifiers,
            proof_output,
            proof_root,
            public_amount,
            asset_tag,
            network_tag,
        ) = crate::zk::confidential_v2::parse_unshield_public_inputs_v3(
            &request.redeem_proof.proof.bytes,
        )
        .map_err(|err| labeled_invariant("invalid_proof", err.to_string()))?;
        let expected_public_amount =
            crate::zk::confidential_v2::encode_confidential_amount_v2(request.amount.atomic_units);
        let expected_asset_tag = crate::zk::confidential_v2::derive_confidential_asset_tag_v2(
            &statement.asset.to_string(),
        );
        let expected_network_tag = crate::zk::confidential_v2::derive_confidential_network_tag_v2(
            state_transaction.network_id(),
        );
        if input_commitments != [statement.current_note.note_commitment, zero]
            || proof_nullifiers != [statement.current_note.spend_nullifier, zero]
            || proof_output != expected_change
            || proof_root != statement.final_root
            || public_amount != expected_public_amount
            || asset_tag != expected_asset_tag
            || network_tag != expected_network_tag
        {
            return Err(labeled_invariant(
                "final_commitment_mismatch",
                "Kagemusha V4 unshield-v3 proof is not bound to the exact note, nullifier, root, scaled amount, asset, network, and full redemption output",
            )
            .into());
        }
        let parsed_binding = iroha_data_model::offline::KagemushaUnshieldPublicInputsBindingV2 {
            input_commitment_0: input_commitments[0],
            input_commitment_1: input_commitments[1],
            nullifier_0: proof_nullifiers[0],
            nullifier_1: proof_nullifiers[1],
            change_output_commitment: proof_output,
            root: proof_root,
            public_amount,
            asset_tag,
            network_tag,
        };
        if parsed_binding != request.redemption.unshield_public_inputs
            || parsed_binding
                .digest()
                .map_err(|err| labeled_invariant("invalid_proof", err.to_string()))?
                != request.redemption.unshield_public_inputs_digest
        {
            return Err(labeled_invariant(
                "proof_binding",
                "Kagemusha V4 redemption intent does not match the canonical unshield-v3 public inputs",
            )
            .into());
        }
        Ok(())
    }
    fn kagemusha_replay_key(domain: &str, value: &Hash) -> Hash {
        let mut preimage = Vec::with_capacity(domain.len() + Hash::LENGTH + 1);
        preimage.extend_from_slice(domain.as_bytes());
        preimage.push(b':');
        preimage.extend_from_slice(value.as_ref());
        Hash::new(&preimage)
    }
    fn kagemusha_device_registration_key(registration_hash: &Hash) -> Hash {
        kagemusha_replay_key(KAGEMUSHA_DEVICE_REGISTRATION_DOMAIN, registration_hash)
    }
    fn kagemusha_attestation_challenge_key(challenge_hash: &Hash) -> Hash {
        kagemusha_replay_key(
            KAGEMUSHA_ATTESTATION_CHALLENGE_REPLAY_DOMAIN,
            challenge_hash,
        )
    }
    fn kagemusha_attestation_report_key(report_hash: &Hash) -> Hash {
        kagemusha_replay_key(KAGEMUSHA_ATTESTATION_REPORT_REPLAY_DOMAIN, report_hash)
    }
    fn kagemusha_attestation_evidence_key(evidence_hash: &Hash) -> Hash {
        kagemusha_replay_key(KAGEMUSHA_ATTESTATION_EVIDENCE_REPLAY_DOMAIN, evidence_hash)
    }
    fn kagemusha_v4_authorization_markers(
        authorization: &KagemushaRequestAuthorizationV2,
    ) -> Result<[Hash; 4], Error> {
        let authority = authorization.authority.to_string();
        // Top-up anchors are keyed by operation id alone. Keep the replay
        // marker equally global so a second authority cannot claim the same
        // operation id while nonce, payload, and exact-request replay remain
        // scoped to their signing authority.
        let operation = kagemusha_v2_marker(
            KAGEMUSHA_V4_OPERATION_DOMAIN,
            &[&authorization.operation_id],
        );
        let nonce = kagemusha_v2_marker(
            KAGEMUSHA_V4_NONCE_DOMAIN,
            &[authority.as_bytes(), &authorization.nonce],
        );
        let payload = kagemusha_v2_marker(
            KAGEMUSHA_V4_PAYLOAD_DOMAIN,
            &[authority.as_bytes(), &authorization.payload_digest],
        );
        let authorization_archive = norito::encode_canonical(authorization).map_err(|err| {
            labeled_invariant(
                "invalid_authorization",
                format!("failed to encode exact Kagemusha authorization: {err}"),
            )
        })?;
        let request = kagemusha_v2_marker(
            KAGEMUSHA_V4_REQUEST_DOMAIN,
            &[
                authority.as_bytes(),
                &authorization.operation_id,
                &authorization.nonce,
                &authorization.payload_digest,
                &authorization_archive,
            ],
        );
        Ok([operation, nonce, payload, request])
    }
    enum KagemushaV4ReplayStatus {
        Fresh([Hash; 4]),
        Committed,
    }
    fn kagemusha_v4_replay_status(
        authorization: &KagemushaRequestAuthorizationV2,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<KagemushaV4ReplayStatus, Error> {
        let markers = kagemusha_v4_authorization_markers(authorization)?;
        let [operation, nonce, payload, request] = &markers;
        if state_transaction
            .world
            .kagemusha_replay_keys
            .get(request)
            .is_some()
        {
            return Ok(KagemushaV4ReplayStatus::Committed);
        }
        if [operation, nonce, payload].iter().any(|marker| {
            state_transaction
                .world
                .kagemusha_replay_keys
                .get(marker)
                .is_some()
        }) {
            return Err(labeled_invariant(
                "authorization_replay",
                "Kagemusha V4 operation id, nonce, or payload digest conflicts with a committed request",
            )
            .into());
        }
        Ok(KagemushaV4ReplayStatus::Fresh(markers))
    }
    fn commit_kagemusha_v4_replay_markers(
        markers: [Hash; 4],
        state_transaction: &mut StateTransaction<'_, '_>,
    ) {
        for marker in markers {
            state_transaction
                .world
                .kagemusha_replay_keys
                .insert(marker, ());
        }
    }
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
    struct KagemushaOnlineHardwareAssertionConsumptionV1 {
        operation_id: [u8; 32],
        nonce: [u8; 32],
        payload_digest: [u8; 32],
        assertion_hash: Hash,
    }
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
    enum KagemushaOnlineHardwareAssertionLifecycleV1 {
        AndroidKeyMintUnused,
        AndroidKeyMintConsumed(KagemushaOnlineHardwareAssertionConsumptionV1),
        IosAppAttest {
            last_sign_count: u32,
            last_consumption: Option<KagemushaOnlineHardwareAssertionConsumptionV1>,
        },
    }
    /// First-release compact schema. Earlier full-evidence records are intentionally not migrated.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
    struct KagemushaOnlineRegistrationStateV4 {
        version: u16,
        original_registration_hash: [u8; 32],
        registration_projection_hash: [u8; 32],
        admission_policy_hash: [u8; 32],
        admission_height: u64,
        admission_transaction_hash: HashOf<SignedTransaction>,
        /// Validated projection; raw report/evidence are not duplicated in active world state.
        /// The exact submitted bytes remain available through signed transaction history.
        registration: OfflineDeviceAttestationRegistration,
        lifecycle: KagemushaOnlineHardwareAssertionLifecycleV1,
    }
    struct KagemushaOnlineHardwareAssertionCommitPlan {
        state_key: StatePath,
        previous_archive: Vec<u8>,
        updated_archive: Vec<u8>,
    }
    enum KagemushaAuthenticatedHardwareAssertionV1 {
        AndroidKeyMint,
        IosAppAttest { sign_count: u32 },
    }
    struct KagemushaAuthenticatedDeviceV1 {
        state_key: StatePath,
        previous_archive: Vec<u8>,
        state: KagemushaOnlineRegistrationStateV4,
        consumption: KagemushaOnlineHardwareAssertionConsumptionV1,
        assertion: KagemushaAuthenticatedHardwareAssertionV1,
    }
    fn kagemusha_online_registration_range_start() -> StatePath {
        KAGEMUSHA_ONLINE_REGISTRATION_STATE_PREFIX
            .parse()
            .expect("static Kagemusha registration prefix is a valid state path")
    }
    fn kagemusha_online_registration_state_key(
        registration_hash: &[u8; 32],
    ) -> Result<StatePath, Error> {
        format!(
            "{KAGEMUSHA_ONLINE_REGISTRATION_STATE_PREFIX}{}",
            hex::encode(registration_hash)
        )
        .parse()
        .map_err(|err| {
            labeled_invariant(
                "invalid_attestation",
                format!("failed to derive Kagemusha registration state key: {err}"),
            )
            .into()
        })
    }
    fn canonical_registration_hash(
        registration: &OfflineDeviceAttestationRegistration,
    ) -> Result<Hash, Error> {
        norito::encode_canonical(registration)
            .map(Hash::new)
            .map_err(|err| {
                labeled_invariant(
                    "invalid_attestation",
                    format!("failed to encode persisted Kagemusha device registration: {err}"),
                )
                .into()
            })
    }
    fn compact_kagemusha_registration_projection(
        registration: &OfflineDeviceAttestationRegistration,
    ) -> OfflineDeviceAttestationRegistration {
        let mut compact = registration.clone();
        compact.attestation_report.clear();
        compact.evidence.clear();
        compact
    }
    fn decode_kagemusha_online_registration_state_v4(
        state_key: &StatePath,
        archive: &[u8],
    ) -> Result<KagemushaOnlineRegistrationStateV4, String> {
        if archive.len() > KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_CANONICAL_BYTES_V4 {
            return Err(format!(
                "Kagemusha registration state `{state_key}` exceeds the {}-byte protocol limit",
                KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_CANONICAL_BYTES_V4,
            ));
        }
        let state: KagemushaOnlineRegistrationStateV4 =
            norito::decode_canonical(archive).map_err(|error| {
                format!("Kagemusha registration state `{state_key}` is corrupt: {error}")
            })?;
        let key_hash_hex = state_key
            .as_ref()
            .strip_prefix(KAGEMUSHA_ONLINE_REGISTRATION_STATE_PREFIX)
            .ok_or_else(|| {
                format!("Kagemusha registration state key `{state_key}` is outside its namespace")
            })?;
        let projection_hash = canonical_registration_hash(&state.registration)
            .map(|hash| exact_hash_bytes(&hash))
            .map_err(|error| {
                format!("Kagemusha registration state `{state_key}` cannot be hashed: {error}")
            })?;
        if state.version != KAGEMUSHA_ONLINE_REGISTRATION_STATE_VERSION_V4
            || key_hash_hex.len() != 64
            || !key_hash_hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || hex::encode(state.original_registration_hash) != key_hash_hex
            || state.registration_projection_hash != projection_hash
            || !state.registration.attestation_report.is_empty()
            || !state.registration.evidence.is_empty()
            || state.admission_height == 0
            || state
                .admission_transaction_hash
                .as_ref()
                .iter()
                .all(|byte| *byte == 0)
        {
            return Err(format!(
                "Kagemusha registration state `{state_key}` is non-canonical, non-compact, or has invalid native provenance"
            ));
        }
        Ok(state)
    }
    fn encode_kagemusha_online_registration_state_v4(
        state: &KagemushaOnlineRegistrationStateV4,
    ) -> Result<Vec<u8>, Error> {
        let archive = norito::encode_canonical(state).map_err(|err| {
            labeled_invariant(
                "invalid_attestation",
                format!("failed to encode compact Kagemusha registration state: {err}"),
            )
        })?;
        if archive.len() > KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_CANONICAL_BYTES_V4 {
            return Err(labeled_invariant(
                "invalid_attestation",
                format!(
                    "compact Kagemusha registration state exceeds the {}-byte protocol limit",
                    KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_CANONICAL_BYTES_V4,
                ),
            )
            .into());
        }
        Ok(archive)
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum KagemushaOnlineRegistrationCapacityErrorV1 {
        Global,
        Account,
    }
    fn validate_kagemusha_online_registration_capacity_v1(
        global_after_admission: usize,
        account_after_admission: usize,
    ) -> Result<(), KagemushaOnlineRegistrationCapacityErrorV1> {
        if global_after_admission > KAGEMUSHA_ACTIVE_DEVICE_REGISTRATIONS_MAX_GLOBAL_V1 {
            return Err(KagemushaOnlineRegistrationCapacityErrorV1::Global);
        }
        if account_after_admission > KAGEMUSHA_ACTIVE_DEVICE_REGISTRATIONS_MAX_PER_ACCOUNT_V1 {
            return Err(KagemushaOnlineRegistrationCapacityErrorV1::Account);
        }
        Ok(())
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum KagemushaOnlineRegistrationRetainedCapacityErrorV4 {
        Global,
        Account,
    }
    fn validate_kagemusha_online_registration_retained_capacity_v4(
        global_after_admission: usize,
        account_after_admission: usize,
    ) -> Result<(), KagemushaOnlineRegistrationRetainedCapacityErrorV4> {
        if global_after_admission > KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_RETAINED_V4 {
            return Err(KagemushaOnlineRegistrationRetainedCapacityErrorV4::Global);
        }
        if account_after_admission > KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_RETAINED_PER_ACCOUNT_V4
        {
            return Err(KagemushaOnlineRegistrationRetainedCapacityErrorV4::Account);
        }
        Ok(())
    }
    #[derive(Debug)]
    struct KagemushaPrunableOnlineRegistrationV4 {
        state_key: StatePath,
        replay_keys: [Hash; 4],
    }
    #[derive(Debug, Default)]
    struct KagemushaOnlineRegistrationAdmissionPlanV1 {
        prunable: Vec<KagemushaPrunableOnlineRegistrationV4>,
    }
    impl KagemushaOnlineRegistrationAdmissionPlanV1 {
        fn commit(self, state_transaction: &mut StateTransaction<'_, '_>) {
            for prunable in self.prunable {
                state_transaction
                    .world
                    .smart_contract_state
                    .remove(prunable.state_key);
                for replay_key in prunable.replay_keys {
                    state_transaction
                        .world
                        .kagemusha_replay_keys
                        .remove(replay_key);
                }
            }
        }
    }
    fn kagemusha_registration_replay_horizon_elapsed(
        registration: &OfflineDeviceAttestationRegistration,
        committed_height: u64,
    ) -> bool {
        committed_height
            > registration
                .recent_block_height
                .saturating_add(KAGEMUSHA_ATTESTATION_RECENT_BLOCK_WINDOW)
    }
    fn kagemusha_registration_replay_keys(
        registration: &OfflineDeviceAttestationRegistration,
        registration_hash: &Hash,
    ) -> [Hash; 4] {
        [
            kagemusha_device_registration_key(registration_hash),
            kagemusha_attestation_challenge_key(&registration.challenge_hash),
            kagemusha_attestation_report_key(&registration.attestation_report_hash),
            kagemusha_attestation_evidence_key(&registration.evidence_hash),
        ]
    }
    fn ensure_kagemusha_registration_replay_keys_are_fresh(
        replay_keys: &[Hash; 4],
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if replay_keys.iter().any(|key| {
            state_transaction
                .world
                .kagemusha_replay_keys
                .get(key)
                .is_some()
        }) {
            return Err(labeled_invariant(
                "duplicate_attestation",
                "Kagemusha device attestation registration reuses registration or evidence material",
            )
            .into());
        }
        Ok(())
    }
    fn plan_kagemusha_online_registration_admission_v1(
        registration: &OfflineDeviceAttestationRegistration,
        admission_policy_hash: [u8; 32],
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<KagemushaOnlineRegistrationAdmissionPlanV1, Error> {
        let evaluated_at_ms = state_transaction.block_unix_timestamp_ms();
        let committed_height = state_transaction.block_hashes().len() as u64;
        let mut retained_count = 0_usize;
        let mut retained_account_count = 0_usize;
        let mut prunable_count = 0_usize;
        let mut prunable_account_count = 0_usize;
        let mut active_global_count = 0_usize;
        let mut active_account_count = 0_usize;
        let mut seen_replay_keys = BTreeSet::new();
        let mut plan = KagemushaOnlineRegistrationAdmissionPlanV1::default();
        // TODO: Replace this correctness-bounded prefix scan with consensus-maintained counters and an expiry cache to make admission and snapshot derivation cheaper.
        let range_start = kagemusha_online_registration_range_start();
        for (existing_key, existing_archive) in state_transaction
            .world
            .smart_contract_state
            .range(range_start..)
        {
            if !existing_key
                .as_ref()
                .starts_with(KAGEMUSHA_ONLINE_REGISTRATION_STATE_PREFIX)
            {
                break;
            }
            retained_count = retained_count.checked_add(1).ok_or_else(|| {
                labeled_invariant(
                    "registration_capacity_exceeded",
                    "Kagemusha device-registration count overflowed",
                )
            })?;
            if retained_count > KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_RETAINED_V4 {
                return Err(labeled_invariant(
                    "registration_capacity_exceeded",
                    "retained Kagemusha device registrations exceed the retained-state protocol limit",
                )
                .into());
            }
            let existing =
                decode_kagemusha_online_registration_state_v4(existing_key, existing_archive)
                    .map_err(|error| {
                        labeled_invariant(
                            "invalid_attestation",
                            format!(
                                "failed to validate existing Kagemusha registration state: {error}"
                            ),
                        )
                    })?;
            let other = &existing.registration;
            if other.account_id == registration.account_id {
                retained_account_count =
                    retained_account_count.checked_add(1).ok_or_else(|| {
                        labeled_invariant(
                            "registration_capacity_exceeded",
                            "retained per-account Kagemusha registration count overflowed",
                        )
                    })?;
                if retained_account_count
                    > KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_RETAINED_PER_ACCOUNT_V4
                {
                    return Err(labeled_invariant(
                        "registration_capacity_exceeded",
                        "retained Kagemusha registrations exceed the per-account protocol limit",
                    )
                    .into());
                }
            }
            let other_hash = Hash::prehashed(existing.original_registration_hash);
            let replay_keys = kagemusha_registration_replay_keys(other, &other_hash);
            if replay_keys.iter().any(|key| !seen_replay_keys.insert(*key)) {
                return Err(labeled_invariant(
                    "invalid_attestation",
                    "existing Kagemusha registration states reuse replay-protected material",
                )
                .into());
            }
            if replay_keys.iter().any(|key| {
                state_transaction
                    .world
                    .kagemusha_replay_keys
                    .get(key)
                    .is_none()
            }) {
                return Err(labeled_invariant(
                    "invalid_attestation",
                    "existing Kagemusha registration state is missing replay protection",
                )
                .into());
            }
            if other.expires_at_ms <= evaluated_at_ms
                || existing.admission_policy_hash != admission_policy_hash
            {
                if kagemusha_registration_replay_horizon_elapsed(other, committed_height) {
                    prunable_count += 1;
                    if other.account_id == registration.account_id {
                        prunable_account_count += 1;
                    }
                    plan.prunable.push(KagemushaPrunableOnlineRegistrationV4 {
                        state_key: existing_key.clone(),
                        replay_keys,
                    });
                }
                continue;
            }
            active_global_count = active_global_count.checked_add(1).ok_or_else(|| {
                labeled_invariant(
                    "registration_capacity_exceeded",
                    "active Kagemusha device-registration count overflowed",
                )
            })?;
            if other.account_id == registration.account_id {
                active_account_count = active_account_count.checked_add(1).ok_or_else(|| {
                    labeled_invariant(
                        "registration_capacity_exceeded",
                        "per-account Kagemusha device-registration count overflowed",
                    )
                })?;
            }
            if other.account_id == registration.account_id
                && other.device_id == registration.device_id
                && other.asset_definition_id == registration.asset_definition_id
                && other.public_key == registration.public_key
            {
                return Err(labeled_invariant(
                    "duplicate_attestation",
                    "an active registration already owns this account, device, asset, and P-256 key under the current policy",
                )
                .into());
            }
        }
        let global_after_admission = active_global_count.checked_add(1).ok_or_else(|| {
            labeled_invariant(
                "registration_capacity_exceeded",
                "active Kagemusha device-registration count overflowed",
            )
        })?;
        let account_after_admission = active_account_count.checked_add(1).ok_or_else(|| {
            labeled_invariant(
                "registration_capacity_exceeded",
                "per-account Kagemusha device-registration count overflowed",
            )
        })?;
        validate_kagemusha_online_registration_capacity_v1(
            global_after_admission,
            account_after_admission,
        )
        .map_err(|error| match error {
            KagemushaOnlineRegistrationCapacityErrorV1::Global => labeled_invariant(
                "registration_capacity_exceeded",
                "Kagemusha device registrations reached the global protocol limit",
            ),
            KagemushaOnlineRegistrationCapacityErrorV1::Account => labeled_invariant(
                "registration_capacity_exceeded",
                "Kagemusha device registrations reached the per-account protocol limit",
            ),
        })?;
        let retained_global_after_admission = retained_count
            .checked_sub(prunable_count)
            .and_then(|count| count.checked_add(1))
            .ok_or_else(|| {
                labeled_invariant(
                    "registration_capacity_exceeded",
                    "retained Kagemusha device-registration count overflowed",
                )
            })?;
        let retained_account_after_admission = retained_account_count
            .checked_sub(prunable_account_count)
            .and_then(|count| count.checked_add(1))
            .ok_or_else(|| {
                labeled_invariant(
                    "registration_capacity_exceeded",
                    "retained per-account Kagemusha registration count overflowed",
                )
            })?;
        validate_kagemusha_online_registration_retained_capacity_v4(
            retained_global_after_admission,
            retained_account_after_admission,
        )
        .map_err(|error| match error {
            KagemushaOnlineRegistrationRetainedCapacityErrorV4::Global => labeled_invariant(
                "registration_capacity_exceeded",
                "retained Kagemusha registrations reached the global protocol limit",
            ),
            KagemushaOnlineRegistrationRetainedCapacityErrorV4::Account => labeled_invariant(
                "registration_capacity_exceeded",
                "retained Kagemusha registrations reached the per-account protocol limit",
            ),
        })?;
        Ok(plan)
    }
    fn canonical_offline_device_attestation_policy_hash(
        policy: &OfflineDeviceAttestationPolicy,
    ) -> Result<[u8; 32], Error> {
        norito::encode_canonical(policy)
            .map(Hash::new)
            .map(|hash| exact_hash_bytes(&hash))
            .map_err(|err| {
                labeled_invariant(
                    "invalid_attestation_policy",
                    format!("failed to encode Offline device attestation policy: {err}"),
                )
                .into()
            })
    }
    fn current_offline_device_attestation_policy_from_world(
        world: &impl WorldReadOnly,
        evaluated_at_ms: u64,
    ) -> Result<(OfflineDeviceAttestationPolicy, [u8; 32]), String> {
        let archive = world
            .smart_contract_state()
            .get(&*OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY)
            .ok_or_else(|| {
                "the governed offline device-attestation policy is not installed".to_owned()
            })?;
        let policy: OfflineDeviceAttestationPolicy =
            norito::decode_canonical(archive).map_err(|error| {
                format!("the governed device-attestation policy is corrupt: {error}")
            })?;
        validate_offline_attestation_policy(&policy, evaluated_at_ms).map_err(|error| {
            format!("the governed device-attestation policy is invalid: {error}")
        })?;
        let policy_hash =
            canonical_offline_device_attestation_policy_hash(&policy).map_err(|error| {
                format!("the governed device-attestation policy cannot be hashed: {error}")
            })?;
        Ok((policy, policy_hash))
    }
    /// Project the governed policy into the typed finalized query response.
    ///
    /// Torii supplies a finality binding produced by its committed-state query
    /// corridor. This helper reads the exact canonical state bytes, repeats
    /// native policy validation, and derives the exclusive cache deadline from
    /// the authenticated Android status snapshot. It deliberately does not
    /// fabricate a block/QC binding from mutable node-local status.
    ///
    /// # Errors
    ///
    /// Returns an error when policy state is absent, corrupt, stale, lacks the
    /// production freshness source, or disagrees with the supplied finality
    /// binding.
    pub fn finalized_offline_device_attestation_policy_view_v1(
        world: &impl WorldReadOnly,
        finality: OfflineDevicePolicyFinalityBindingV1,
        evaluated_at_ms: u64,
    ) -> Result<OfflineDeviceAttestationPolicyViewV1, String> {
        let (policy, _) =
            current_offline_device_attestation_policy_from_world(world, evaluated_at_ms)?;
        let snapshot = policy.android_status_snapshot.as_ref().ok_or_else(|| {
            "the governed Offline device-attestation policy has no authenticated freshness snapshot"
                .to_owned()
        })?;
        let freshness_deadline_ms = android_attestation_status_snapshot_fresh_until_ms(snapshot)
            .map_err(|error| {
                format!("the governed device-attestation policy freshness is invalid: {error}")
            })?;
        if evaluated_at_ms >= freshness_deadline_ms {
            return Err("the governed Offline device-attestation policy is stale".to_owned());
        }
        OfflineDeviceAttestationPolicyViewV1::new_v1(&policy, freshness_deadline_ms, finality)
            .map_err(|error| format!("cannot project finalized device-attestation policy: {error}"))
    }
    /// Require the governed, canonical anti-rollback spend-authority policy.
    ///
    /// Startup calls this after Kura replay and before networking. A validator
    /// must never substitute a software-only or absent policy for the
    /// rollback-resistant hardware contract used by offline cash.
    pub fn ensure_offline_device_attestation_policy_ready_v1(
        world: &impl WorldReadOnly,
        evaluated_at_ms: u64,
    ) -> Result<(), String> {
        current_offline_device_attestation_policy_from_world(world, evaluated_at_ms).map(|_| ())
    }
    /// Derive the canonical end-of-block active-receiver snapshot.
    ///
    /// Only records in the canonical native-protected namespace are eligible. A corrupt protected
    /// record or policy produces a deterministic unavailable snapshot; multiple current
    /// registrations for the same account/device/asset tuple produce an explicit ambiguous leaf
    /// which cannot be routed.
    pub fn derive_kagemusha_active_receiver_snapshot_v1(
        world: &impl WorldReadOnly,
        evaluated_height: u64,
        evaluated_at_ms: u64,
    ) -> Result<KagemushaActiveReceiverSnapshotV1, String> {
        if evaluated_height == 0 {
            return Err("active-receiver evaluation must bind a committed block".to_owned());
        }
        let (policy, current_policy_hash) =
            match current_offline_device_attestation_policy_from_world(world, evaluated_at_ms) {
                Ok(value) => value,
                Err(error) => {
                    return KagemushaActiveReceiverSnapshotV1::unavailable(
                        evaluated_height,
                        evaluated_at_ms,
                        error.as_bytes(),
                    );
                }
            };
        let current_policy_hash_value = Hash::prehashed(current_policy_hash);
        let mut candidates =
            BTreeMap::<KagemushaActiveReceiverKeyV1, Vec<KagemushaActiveReceiverValueV1>>::new();
        let fail_closed = |reason: String| {
            KagemushaActiveReceiverSnapshotV1::unavailable(
                evaluated_height,
                evaluated_at_ms,
                reason.as_bytes(),
            )
        };
        let mut inspected_count = 0_usize;
        let mut active_registration_count = 0_usize;
        let range_start = kagemusha_online_registration_range_start();
        for (state_key, archive) in world.smart_contract_state().range(range_start..) {
            if !state_key
                .as_ref()
                .starts_with(KAGEMUSHA_ONLINE_REGISTRATION_STATE_PREFIX)
            {
                break;
            }
            inspected_count += 1;
            if inspected_count > KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_RETAINED_V4 {
                return fail_closed(
                    "protected Kagemusha registrations exceed the retained-state protocol limit"
                        .to_owned(),
                );
            }
            let state = match decode_kagemusha_online_registration_state_v4(state_key, archive) {
                Ok(state) => state,
                Err(error) => {
                    return fail_closed(format!(
                        "protected Kagemusha registration `{state_key}` is corrupt: {error}"
                    ));
                }
            };
            let registration_hash = state.original_registration_hash;
            if state.admission_height > evaluated_height {
                return fail_closed(format!(
                    "protected Kagemusha registration `{state_key}` has invalid native provenance"
                ));
            }
            if state.admission_policy_hash != current_policy_hash
                || state.registration.expires_at_ms <= evaluated_at_ms
            {
                continue;
            }
            active_registration_count += 1;
            if active_registration_count > KAGEMUSHA_ACTIVE_DEVICE_REGISTRATIONS_MAX_GLOBAL_V1 {
                return fail_closed(
                    "active Kagemusha registrations exceed the global protocol limit".to_owned(),
                );
            }
            let registration = &state.registration;
            validate_offline_attestation_platform_profile(registration).map_err(|error| {
                format!("protected registration `{state_key}` profile is invalid: {error}")
            })?;
            validate_offline_attestation_optional_metadata(registration).map_err(|error| {
                format!("protected registration `{state_key}` metadata is invalid: {error}")
            })?;
            match registration.platform.as_str() {
                OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT => {
                    let (package_name, signing_digest) = android_attestation_metadata(registration)
                        .map_err(|error| {
                            format!(
                                "protected Android registration `{state_key}` is invalid: {error}"
                            )
                        })?;
                    ensure_android_app_allowed_by_policy(&policy, &package_name, &signing_digest)
                        .map_err(|error| {
                            format!(
                                "protected Android registration `{state_key}` is no longer governed: {error}"
                            )
                        })?;
                }
                OFFLINE_ATTESTATION_PLATFORM_IOS_APP_ATTEST => {
                    let (team_id, bundle_id, environment) = ios_attestation_metadata(registration)
                        .map_err(|error| {
                            format!("protected iOS registration `{state_key}` is invalid: {error}")
                        })?;
                    ensure_ios_app_allowed_by_policy(&policy, &team_id, &bundle_id, &environment)
                        .map_err(|error| {
                            format!(
                                "protected iOS registration `{state_key}` is no longer governed: {error}"
                            )
                        })?;
                }
                _ => {
                    return fail_closed(format!(
                        "protected registration `{state_key}` uses an unsupported platform"
                    ));
                }
            }
            let Some(asset_definition_id) = registration.asset_definition_id.clone() else {
                continue;
            };
            let account_exists = world.accounts().get(&registration.account_id).is_some();
            let asset_definition_exists = world
                .asset_definitions()
                .get(&asset_definition_id)
                .is_some();
            if !account_exists || !asset_definition_exists {
                continue;
            }
            let key = KagemushaActiveReceiverKeyV1 {
                account_id: registration.account_id.clone(),
                device_id: registration.device_id.clone(),
                asset_definition_id,
            };
            candidates
                .entry(key)
                .or_default()
                .push(KagemushaActiveReceiverValueV1 {
                    registration_hash: Hash::prehashed(registration_hash),
                    registration_state_hash: Hash::new(archive),
                    admission_policy_hash: Hash::prehashed(state.admission_policy_hash),
                    current_policy_hash: current_policy_hash_value,
                    admission_height: state.admission_height,
                    admission_transaction_hash: Hash::prehashed(
                        *state.admission_transaction_hash.as_ref(),
                    ),
                    public_key: registration.public_key.clone(),
                    expires_at_ms: registration.expires_at_ms,
                    account_exists,
                    asset_definition_exists,
                });
        }
        let mut entries = Vec::with_capacity(candidates.len());
        for (key, mut values) in candidates {
            values.sort_by_key(|value| value.registration_state_hash);
            if values.len() == 1 {
                entries.push(KagemushaActiveReceiverEntryV1::Active(
                    KagemushaActiveReceiverActiveEntryV1 {
                        key,
                        value: values.pop().expect("one active receiver value exists"),
                    },
                ));
            } else {
                let candidate_count = u32::try_from(values.len()).map_err(|_| {
                    "ambiguous active-receiver candidate count does not fit u32".to_owned()
                })?;
                let hashes = values
                    .iter()
                    .map(|value| value.registration_state_hash)
                    .collect::<Vec<_>>();
                let hashes_archive =
                    norito::encode_canonical(&hashes).map_err(|error| error.to_string())?;
                entries.push(KagemushaActiveReceiverEntryV1::Ambiguous(
                    KagemushaActiveReceiverAmbiguousEntryV1 {
                        key,
                        candidate_count,
                        candidates_digest: Hash::new(hashes_archive),
                    },
                ));
            }
        }
        KagemushaActiveReceiverSnapshotV1::available(
            evaluated_height,
            evaluated_at_ms,
            current_policy_hash_value,
            entries,
        )
    }
    /// Load the exact protected registration named by one active snapshot leaf.
    ///
    /// This lookup is deliberately request-independent so Torii can publish a
    /// reusable receiver proof before the receiver creates a payment request.
    /// Every field is cross-checked against the freshly derived leaf; the
    /// returned registration is public payload, not a second source of trust.
    pub fn resolve_kagemusha_active_receiver_registration_v1(
        world: &impl WorldReadOnly,
        active: &KagemushaActiveReceiverActiveEntryV1,
        evaluated_height: u64,
        evaluated_at_ms: u64,
    ) -> Result<KagemushaRecipientRegistrationResolutionV1, String> {
        if evaluated_height == 0 {
            return Err("receiver-lineage evaluation must bind a committed block".to_owned());
        }
        let value = &active.value;
        let registration_hash = *value.registration_hash.as_ref();
        let state_key = kagemusha_online_registration_state_key(&registration_hash)
            .map_err(|error| format!("active registration key is invalid: {error}"))?;
        let archive = world
            .smart_contract_state()
            .get(&state_key)
            .ok_or_else(|| {
                "active registration archive is absent from protected state".to_owned()
            })?;
        if archive.len() > KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_CANONICAL_BYTES_V4 {
            return Err("active registration archive exceeds the protocol byte limit".to_owned());
        }
        if Hash::new(archive) != value.registration_state_hash {
            return Err(
                "active registration archive hash differs from the snapshot leaf".to_owned(),
            );
        }
        let state = decode_kagemusha_online_registration_state_v4(&state_key, archive)
            .map_err(|error| format!("active registration archive is invalid: {error}"))?;
        let (_policy, current_policy_hash) =
            current_offline_device_attestation_policy_from_world(world, evaluated_at_ms)?;
        let registration = &state.registration;
        let account_exists = world.accounts().get(&active.key.account_id).is_some();
        let asset_definition_exists = world
            .asset_definitions()
            .get(&active.key.asset_definition_id)
            .is_some();
        if state.original_registration_hash != registration_hash
            || state.admission_height > evaluated_height
            || state.admission_height != value.admission_height
            || Hash::prehashed(*state.admission_transaction_hash.as_ref())
                != value.admission_transaction_hash
            || Hash::prehashed(state.admission_policy_hash) != value.admission_policy_hash
            || Hash::prehashed(current_policy_hash) != value.current_policy_hash
            || state.admission_policy_hash != current_policy_hash
            || registration.account_id != active.key.account_id
            || registration.device_id != active.key.device_id
            || registration.asset_definition_id.as_ref() != Some(&active.key.asset_definition_id)
            || registration.public_key != value.public_key
            || registration.expires_at_ms != value.expires_at_ms
            || registration.expires_at_ms <= evaluated_at_ms
            || !account_exists
            || !asset_definition_exists
            || !value.account_exists
            || !value.asset_definition_exists
        {
            return Err(
                "protected registration state disagrees with the active-receiver snapshot leaf"
                    .to_owned(),
            );
        }
        Ok(KagemushaRecipientRegistrationResolutionV1 {
            registration: registration.clone(),
            registration_hash,
            admission_policy_hash: state.admission_policy_hash,
            admission_height: state.admission_height,
            admission_transaction_hash: state.admission_transaction_hash,
        })
    }
    /// Resolve the unique active on-chain registration for one signed receiver request.
    ///
    /// The lookup scans only canonical Kagemusha registration records and fails closed on corrupt
    /// state, policy rotation, expiry, provenance gaps, or an ambiguous exact tuple. The caller
    /// supplies the committed evaluation height/time from one immutable state snapshot.
    pub fn resolve_kagemusha_recipient_registration_v1(
        world: &impl WorldReadOnly,
        request: &KagemushaRecipientPaymentRequestV2,
        evaluated_height: u64,
        evaluated_at_ms: u64,
    ) -> Result<KagemushaRecipientRegistrationResolutionV1, String> {
        if evaluated_height == 0 || evaluated_at_ms == 0 {
            return Err("receiver-lineage evaluation must bind a committed block".to_owned());
        }
        request
            .validate_at(evaluated_at_ms)
            .map_err(|error| format!("the recipient payment request is invalid: {error}"))?;
        let (policy, current_policy_hash) =
            current_offline_device_attestation_policy_from_world(world, evaluated_at_ms)?;
        let mut exact = None;
        let mut tuple_seen = false;
        let mut current_policy_seen = false;
        let mut unexpired_seen = false;
        let mut inspected_count = 0_usize;
        let range_start = kagemusha_online_registration_range_start();
        for (state_key, archive) in world.smart_contract_state().range(range_start..) {
            if !state_key
                .as_ref()
                .starts_with(KAGEMUSHA_ONLINE_REGISTRATION_STATE_PREFIX)
            {
                break;
            }
            inspected_count += 1;
            if inspected_count > KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_RETAINED_V4 {
                return Err(
                    "Kagemusha registrations exceed the retained-state protocol limit".to_owned(),
                );
            }
            let state = decode_kagemusha_online_registration_state_v4(state_key, archive)?;
            let registration_hash = state.original_registration_hash;
            if state.admission_height > evaluated_height {
                return Err(format!(
                    "Kagemusha registration state `{state_key}` has invalid admission provenance"
                ));
            }
            let registration = &state.registration;
            if registration.account_id != request.recipient
                || registration.device_id != request.receiver_device_id
                || registration.asset_definition_id.as_ref() != Some(&request.asset)
                || registration.public_key != request.receiver_public_key
            {
                continue;
            }
            tuple_seen = true;
            if state.admission_policy_hash != current_policy_hash {
                continue;
            }
            current_policy_seen = true;
            if registration.expires_at_ms < request.expires_at_ms
                || registration.expires_at_ms <= evaluated_at_ms
            {
                continue;
            }
            unexpired_seen = true;
            validate_offline_attestation_platform_profile(registration).map_err(|error| {
                format!("the selected registration profile is invalid: {error}")
            })?;
            validate_offline_attestation_optional_metadata(registration).map_err(|error| {
                format!("the selected registration metadata is invalid: {error}")
            })?;
            match registration.platform.as_str() {
                OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT => {
                    let (package_name, signing_digest) = android_attestation_metadata(registration)
                        .map_err(|error| {
                            format!("the selected Android registration is invalid: {error}")
                        })?;
                    ensure_android_app_allowed_by_policy(&policy, &package_name, &signing_digest)
                        .map_err(|error| {
                        format!("the selected Android registration is no longer governed: {error}")
                    })?;
                }
                OFFLINE_ATTESTATION_PLATFORM_IOS_APP_ATTEST => {
                    let (team_id, bundle_id, environment) = ios_attestation_metadata(registration)
                        .map_err(|error| {
                            format!("the selected iOS registration is invalid: {error}")
                        })?;
                    ensure_ios_app_allowed_by_policy(&policy, &team_id, &bundle_id, &environment)
                        .map_err(|error| {
                        format!("the selected iOS registration is no longer governed: {error}")
                    })?;
                }
                _ => return Err("the selected registration platform is unsupported".to_owned()),
            }
            if exact.is_some() {
                return Err(
                    "multiple active registrations match the recipient account, device, asset, and P-256 key"
                        .to_owned(),
                );
            }
            exact = Some(KagemushaRecipientRegistrationResolutionV1 {
                registration: registration.clone(),
                registration_hash,
                admission_policy_hash: state.admission_policy_hash,
                admission_height: state.admission_height,
                admission_transaction_hash: state.admission_transaction_hash,
            });
        }
        exact.ok_or_else(|| {
            if !tuple_seen {
                "no on-chain registration matches the recipient account, device, asset, and P-256 key"
                    .to_owned()
            } else if !current_policy_seen {
                "the matching registration was admitted under a superseded attestation policy; the device must register again"
                    .to_owned()
            } else if !unexpired_seen {
                "the matching registration is expired or does not cover the signed request lifetime"
                    .to_owned()
            } else {
                "no active receiver registration is available".to_owned()
            }
        })
    }
    fn exact_hash_bytes(hash: &Hash) -> [u8; 32] {
        *hash.as_ref()
    }
    fn assertion_consumption(
        authorization: &KagemushaRequestAuthorizationV2,
    ) -> Result<KagemushaOnlineHardwareAssertionConsumptionV1, Error> {
        let assertion_archive = norito::encode_canonical(&authorization.hardware_assertion)
            .map_err(|err| {
                labeled_invariant(
                    "invalid_authorization",
                    format!("failed to encode Kagemusha hardware assertion: {err}"),
                )
            })?;
        Ok(KagemushaOnlineHardwareAssertionConsumptionV1 {
            operation_id: authorization.operation_id,
            nonce: authorization.nonce,
            payload_digest: authorization.payload_digest,
            assertion_hash: Hash::new(assertion_archive),
        })
    }
    include!("offline/kagemusha_redemption_policy_validation.rs");
    fn plan_kagemusha_online_hardware_assertion(
        mut authenticated: KagemushaAuthenticatedDeviceV1,
    ) -> Result<KagemushaOnlineHardwareAssertionCommitPlan, Error> {
        let next_lifecycle = match (&authenticated.assertion, &authenticated.state.lifecycle) {
            (
                KagemushaAuthenticatedHardwareAssertionV1::AndroidKeyMint,
                KagemushaOnlineHardwareAssertionLifecycleV1::AndroidKeyMintUnused,
            ) => KagemushaOnlineHardwareAssertionLifecycleV1::AndroidKeyMintConsumed(
                authenticated.consumption.clone(),
            ),
            (
                KagemushaAuthenticatedHardwareAssertionV1::AndroidKeyMint,
                KagemushaOnlineHardwareAssertionLifecycleV1::AndroidKeyMintConsumed(_),
            ) => {
                return Err(labeled_invariant(
                    "hardware_assertion_consumed",
                    "Android KeyMint registration has already authorized an operation",
                )
                .into());
            }
            (
                KagemushaAuthenticatedHardwareAssertionV1::IosAppAttest { sign_count },
                KagemushaOnlineHardwareAssertionLifecycleV1::IosAppAttest {
                    last_sign_count, ..
                },
            ) if sign_count > last_sign_count => {
                KagemushaOnlineHardwareAssertionLifecycleV1::IosAppAttest {
                    last_sign_count: *sign_count,
                    last_consumption: Some(authenticated.consumption.clone()),
                }
            }
            (
                KagemushaAuthenticatedHardwareAssertionV1::IosAppAttest { .. },
                KagemushaOnlineHardwareAssertionLifecycleV1::IosAppAttest { .. },
            ) => {
                return Err(labeled_invariant(
                    "invalid_authorization",
                    "iOS App Attest assertion counter must advance strictly",
                )
                .into());
            }
            _ => {
                return Err(labeled_invariant(
                    "invalid_attestation",
                    "Kagemusha authorization platform does not match its persisted registration lifecycle",
                )
                .into());
            }
        };
        authenticated.state.lifecycle = next_lifecycle;
        let updated_archive = encode_kagemusha_online_registration_state_v4(&authenticated.state)?;
        Ok(KagemushaOnlineHardwareAssertionCommitPlan {
            state_key: authenticated.state_key,
            previous_archive: authenticated.previous_archive,
            updated_archive,
        })
    }
    #[cfg(test)]
    fn ensure_registered_kagemusha_v2_device(
        authorization: &KagemushaRequestAuthorizationV2,
        asset: &AssetDefinitionId,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<KagemushaOnlineHardwareAssertionCommitPlan, Error> {
        let authenticated =
            authenticate_registered_kagemusha_v2_device(authorization, asset, state_transaction)?;
        plan_kagemusha_online_hardware_assertion(authenticated)
    }
    fn commit_kagemusha_online_hardware_assertion(
        plan: KagemushaOnlineHardwareAssertionCommitPlan,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if state_transaction
            .world
            .smart_contract_state
            .get(&plan.state_key)
            .is_none_or(|archive| archive.as_slice() != plan.previous_archive.as_slice())
        {
            return Err(labeled_invariant(
                "hardware_assertion_conflict",
                "Kagemusha hardware registration lifecycle changed during execution",
            )
            .into());
        }
        state_transaction
            .world
            .smart_contract_state
            .insert(plan.state_key, plan.updated_archive);
        Ok(())
    }
    fn kagemusha_v2_branch_marker(domain: &str, path: KagemushaRecursiveSpendBranchPathV2) -> Hash {
        kagemusha_v2_marker(
            domain,
            &[&path.lineage_root, &[path.depth], &path.path_bits],
        )
    }
    fn kagemusha_v2_branch_claim_prefix(
        claim: &KagemushaRecursiveSpendBranchClaimV2,
        depth: u8,
    ) -> Result<KagemushaRecursiveSpendBranchClaimV2, Error> {
        claim
            .prefix(depth)
            .map_err(|err| labeled_invariant("branch_conflict", err.to_string()).into())
    }
    fn kagemusha_v2_branch_claim_marker(
        domain: &str,
        claim: &KagemushaRecursiveSpendBranchClaimV2,
    ) -> Hash {
        kagemusha_v2_marker(
            domain,
            &[
                &claim.path.lineage_root,
                &[claim.path.depth],
                &claim.path.path_bits,
                &claim.transition_tags,
            ],
        )
    }
    fn kagemusha_v2_transition_choice_marker(
        prefix: KagemushaRecursiveSpendBranchPathV2,
        transition_tag: [u8; 24],
    ) -> Hash {
        kagemusha_v2_marker(
            KAGEMUSHA_V4_TRANSITION_CHOICE_DOMAIN,
            &[
                &prefix.lineage_root,
                &[prefix.depth],
                &prefix.path_bits,
                &transition_tag,
            ],
        )
    }
    fn validate_kagemusha_v2_branch_claim_batch(
        claims: &[KagemushaRecursiveSpendBranchClaimV2],
    ) -> Result<(), Error> {
        if claims.is_empty()
            || claims.len()
                > iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_CLAIMS_V2
        {
            return Err(labeled_invariant(
                "branch_conflict",
                "Kagemusha V2 branch set must contain one or two conflict claims",
            )
            .into());
        }
        for (index, claim) in claims.iter().enumerate() {
            claim
                .validate()
                .map_err(|err| labeled_invariant("branch_conflict", err.to_string()))?;
            for previous in &claims[..index] {
                if previous.path.conflicts_with(claim.path) {
                    return Err(labeled_invariant(
                        "branch_conflict",
                        "Kagemusha V2 branch set contains duplicate or overlapping ancestor and descendant claims",
                    )
                    .into());
                }
                if previous.path.lineage_root != claim.path.lineage_root {
                    continue;
                }
                let shared_depth = previous.path.depth.min(claim.path.depth);
                for parent_depth in 0..shared_depth {
                    let previous_prefix = previous
                        .path
                        .prefix(parent_depth)
                        .map_err(|err| labeled_invariant("branch_conflict", err.to_string()))?;
                    let claim_prefix = claim
                        .path
                        .prefix(parent_depth)
                        .map_err(|err| labeled_invariant("branch_conflict", err.to_string()))?;
                    if previous_prefix == claim_prefix
                        && previous.transition_tag_at(parent_depth)
                            != claim.transition_tag_at(parent_depth)
                    {
                        return Err(labeled_invariant(
                            "branch_conflict",
                            "Kagemusha V2 claims select different transitions at the same lineage prefix",
                        )
                        .into());
                    }
                }
            }
            if index > 0 && claims[index - 1].path >= claim.path {
                return Err(labeled_invariant(
                    "branch_conflict",
                    "Kagemusha V2 branch set is not in strict canonical order",
                )
                .into());
            }
        }
        Ok(())
    }
    fn ensure_kagemusha_v2_branch_claim_available(
        claim: &KagemushaRecursiveSpendBranchClaimV2,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        claim
            .validate()
            .map_err(|err| labeled_invariant("branch_conflict", err.to_string()))?;
        for depth in 0..=claim.path.depth {
            let prefix = claim
                .path
                .prefix(depth)
                .map_err(|err| labeled_invariant("branch_conflict", err.to_string()))?;
            let exact = kagemusha_v2_branch_marker(KAGEMUSHA_V4_BRANCH_EXACT_DOMAIN, prefix);
            if state_transaction
                .world
                .kagemusha_replay_keys
                .get(&exact)
                .is_some()
            {
                if depth < claim.path.depth {
                    let child = kagemusha_v2_branch_claim_prefix(claim, depth + 1)?;
                    let authorized_child = kagemusha_v2_branch_claim_marker(
                        KAGEMUSHA_V4_AUTHORIZED_CHANGE_CHILD_DOMAIN,
                        &child,
                    );
                    if state_transaction
                        .world
                        .kagemusha_replay_keys
                        .get(&authorized_child)
                        .is_some()
                    {
                        continue;
                    }
                }
                return Err(labeled_invariant(
                    "branch_conflict",
                    "Kagemusha V2 branch equals or descends from an already redeemed branch",
                )
                .into());
            }
        }
        let has_descendant =
            kagemusha_v2_branch_marker(KAGEMUSHA_V4_BRANCH_DESCENDANT_DOMAIN, claim.path);
        if state_transaction
            .world
            .kagemusha_replay_keys
            .get(&has_descendant)
            .is_some()
        {
            return Err(labeled_invariant(
                "branch_conflict",
                "Kagemusha V2 branch is an ancestor of an already redeemed branch",
            )
            .into());
        }
        Ok(())
    }
    fn stage_kagemusha_v2_transition_choices(
        claims: &[KagemushaRecursiveSpendBranchClaimV2],
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<Vec<(Hash, Hash)>, Error> {
        let mut staged = Vec::<(Hash, Hash)>::new();
        for claim in claims {
            for parent_depth in 0..claim.path.depth {
                let prefix = claim
                    .path
                    .prefix(parent_depth)
                    .map_err(|err| labeled_invariant("branch_conflict", err.to_string()))?;
                let transition_tag = claim.transition_tag_at(parent_depth).ok_or_else(|| {
                    labeled_invariant(
                        "branch_conflict",
                        "Kagemusha V2 branch claim is missing an active transition tag",
                    )
                })?;
                let selected =
                    kagemusha_v2_branch_marker(KAGEMUSHA_V4_TRANSITION_SELECTED_DOMAIN, prefix);
                let choice = kagemusha_v2_transition_choice_marker(prefix, transition_tag);
                let selected_exists = state_transaction
                    .world
                    .kagemusha_replay_keys
                    .get(&selected)
                    .is_some();
                let choice_exists = state_transaction
                    .world
                    .kagemusha_replay_keys
                    .get(&choice)
                    .is_some();
                match (selected_exists, choice_exists) {
                    (true, true) => {}
                    (true, false) => {
                        return Err(labeled_invariant(
                            "branch_conflict",
                            "Kagemusha V2 lineage prefix was already bound to a different transition choice",
                        )
                        .into());
                    }
                    (false, true) => {
                        return Err(labeled_invariant(
                            "branch_conflict",
                            "Kagemusha V2 transition-choice marker exists without its selection marker",
                        )
                        .into());
                    }
                    (false, false) => {
                        if let Some((_, staged_choice)) = staged
                            .iter()
                            .find(|(staged_selected, _)| *staged_selected == selected)
                        {
                            if *staged_choice != choice {
                                return Err(labeled_invariant(
                                    "branch_conflict",
                                    "Kagemusha V2 claims select different transitions at the same lineage prefix",
                                )
                                .into());
                            }
                        } else {
                            staged.push((selected, choice));
                        }
                    }
                }
            }
        }
        Ok(staged)
    }
    fn validate_kagemusha_v2_change_child_authorization(
        parent: &KagemushaRecursiveSpendBranchClaimV2,
        child: &KagemushaRecursiveSpendBranchClaimV2,
        redemption_binding_digest: [u8; 32],
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<Hash, Error> {
        parent
            .validate()
            .map_err(|err| labeled_invariant("branch_conflict", err.to_string()))?;
        child
            .validate()
            .map_err(|err| labeled_invariant("branch_conflict", err.to_string()))?;
        let expected = parent
            .child(
                iroha_data_model::offline::KagemushaRecursiveSpendBranchV2::Change,
                redemption_binding_digest,
            )
            .map_err(|err| labeled_invariant("branch_conflict", err.to_string()))?;
        if *child != expected {
            return Err(labeled_invariant(
                "branch_conflict",
                "Kagemusha V2 partial redemption change is not the exact transition-bound child",
            )
            .into());
        }
        let marker =
            kagemusha_v2_branch_claim_marker(KAGEMUSHA_V4_AUTHORIZED_CHANGE_CHILD_DOMAIN, child);
        if state_transaction
            .world
            .kagemusha_replay_keys
            .get(&marker)
            .is_some()
        {
            return Err(labeled_invariant(
                "branch_conflict",
                "Kagemusha V2 partial redemption change child is already registered",
            )
            .into());
        }
        Ok(marker)
    }
    #[derive(Debug)]
    struct KagemushaV2BranchCommitPlan {
        markers: Vec<Hash>,
    }
    impl KagemushaV2BranchCommitPlan {
        fn commit(self, state_transaction: &mut StateTransaction<'_, '_>) {
            for marker in self.markers {
                state_transaction
                    .world
                    .kagemusha_replay_keys
                    .insert(marker, ());
            }
        }
    }
    /// Validate and stage every marker needed to consume a set of V2 branches.
    ///
    /// Partial redemption may additionally authorize the deterministic change child of a consumed
    /// parent. This function is deliberately read-only: every path, ledger conflict, and transition
    /// choice is checked before a caller can commit even the first marker.
    fn plan_kagemusha_v2_consumed_branch_set(
        consumed_claims: &[KagemushaRecursiveSpendBranchClaimV2],
        redemption_binding_digest: Option<[u8; 32]>,
        change_children: &[(
            KagemushaRecursiveSpendBranchClaimV2,
            KagemushaRecursiveSpendBranchClaimV2,
        )],
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<KagemushaV2BranchCommitPlan, Error> {
        // Admission first: the data model enforces one non-zero 24-byte transition tag per
        // active path edge, with no padding, and a maximum of two claims.
        validate_kagemusha_v2_branch_claim_batch(consumed_claims)?;
        if change_children.is_empty() != redemption_binding_digest.is_none() {
            return Err(labeled_invariant(
                "branch_conflict",
                "Kagemusha V2 partial redemption change claims require exactly one transition binding digest",
            )
            .into());
        }
        if !change_children.is_empty() && change_children.len() != consumed_claims.len() {
            return Err(labeled_invariant(
                "branch_conflict",
                "Kagemusha V2 partial redemption must authorize exactly one change child per consumed claim",
            )
            .into());
        }
        let redemption_binding_digest = redemption_binding_digest.unwrap_or([0; 32]);
        if !change_children.is_empty() && redemption_binding_digest == [0; 32] {
            return Err(labeled_invariant(
                "branch_conflict",
                "Kagemusha V2 partial redemption binding digest must be non-zero",
            )
            .into());
        }
        let mut authorization_markers = Vec::with_capacity(change_children.len());
        for (index, (parent, child)) in change_children.iter().enumerate() {
            if consumed_claims.get(index) != Some(parent) {
                return Err(labeled_invariant(
                    "branch_conflict",
                    "Kagemusha V2 partial redemption change parents do not match the canonical consumed set",
                )
                .into());
            }
            if consumed_claims.contains(child) {
                return Err(labeled_invariant(
                    "branch_conflict",
                    "Kagemusha V2 partial redemption cannot consume its new change child",
                )
                .into());
            }
            authorization_markers.push(validate_kagemusha_v2_change_child_authorization(
                parent,
                child,
                redemption_binding_digest,
                state_transaction,
            )?);
        }
        // Every state lookup and every cross-claim transition choice check is
        // completed before the first set-only marker is written.
        let transition_markers =
            stage_kagemusha_v2_transition_choices(consumed_claims, state_transaction)?;
        for claim in consumed_claims {
            ensure_kagemusha_v2_branch_claim_available(claim, state_transaction)?;
        }
        let mut markers = BTreeSet::new();
        for (selected, choice) in transition_markers {
            markers.insert(selected);
            markers.insert(choice);
        }
        for claim in consumed_claims {
            markers.insert(kagemusha_v2_branch_marker(
                KAGEMUSHA_V4_BRANCH_EXACT_DOMAIN,
                claim.path,
            ));
            for depth in 0..claim.path.depth {
                let prefix = claim
                    .path
                    .prefix(depth)
                    .map_err(|err| labeled_invariant("branch_conflict", err.to_string()))?;
                markers.insert(kagemusha_v2_branch_marker(
                    KAGEMUSHA_V4_BRANCH_DESCENDANT_DOMAIN,
                    prefix,
                ));
            }
        }
        markers.extend(authorization_markers);
        Ok(KagemushaV2BranchCommitPlan {
            markers: markers.into_iter().collect(),
        })
    }
    fn kagemusha_v4_topup_anchor_state_key(operation_id: [u8; 32]) -> Result<StatePath, Error> {
        format!("kagemusha_v4_topup_anchor_{}", hex::encode(operation_id))
            .parse()
            .map_err(|err| {
                labeled_invariant(
                    "invalid_recursive_topup",
                    format!("failed to derive Kagemusha V4 anchor state key: {err}"),
                )
                .into()
            })
    }
    fn kagemusha_v4_topup_drawdown_state_key(operation_id: [u8; 32]) -> Result<StatePath, Error> {
        format!("kagemusha_v4_topup_drawdown_{}", hex::encode(operation_id))
            .parse()
            .map_err(|err| {
                labeled_invariant(
                    "topup_drawdown_invalid",
                    format!("failed to derive Kagemusha V4 anchor drawdown key: {err}"),
                )
                .into()
            })
    }
    fn load_kagemusha_v4_topup_drawdown(
        operation_id: [u8; 32],
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<u128, Error> {
        let key = kagemusha_v4_topup_drawdown_state_key(operation_id)?;
        let encoded = state_transaction
            .world
            .smart_contract_state
            .get(&key)
            .ok_or_else(|| {
                labeled_invariant(
                    "topup_drawdown_missing",
                    "Kagemusha V4 top-up anchor has no drawdown balance",
                )
            })?;
        crate::sumeragi::witness::record_read_kagemusha_v4_topup_drawdown(
            operation_id,
            Some(encoded),
        );
        let bytes: [u8; core::mem::size_of::<u128>()] =
            encoded.as_slice().try_into().map_err(|_| {
                labeled_invariant(
                    "topup_drawdown_invalid",
                    "Kagemusha V4 anchor drawdown must be one canonical little-endian u128",
                )
            })?;
        Ok(u128::from_le_bytes(bytes))
    }
    fn load_kagemusha_v4_topup_anchor(
        operation_id: [u8; 32],
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<KagemushaRecursiveSpendTopUpAnchorV4, Error> {
        let key = kagemusha_v4_topup_anchor_state_key(operation_id)?;
        let archive = state_transaction
            .world
            .smart_contract_state
            .get(&key)
            .ok_or_else(|| {
                labeled_invariant(
                    "topup_anchor_missing",
                    "Kagemusha V4 bundle has no finalized top-up anchor",
                )
            })?;
        crate::sumeragi::witness::record_read_kagemusha_v4_topup_anchor(
            operation_id,
            Some(archive.as_slice()),
        );
        let anchor: KagemushaRecursiveSpendTopUpAnchorV4 = norito::decode_canonical(archive)
            .map_err(|err| {
                labeled_invariant(
                    "topup_anchor_invalid",
                    format!("failed to decode persisted Kagemusha V4 top-up anchor: {err}"),
                )
            })?;
        anchor
            .validate_public_binding()
            .map_err(|err| labeled_invariant("topup_anchor_invalid", err.to_string()))?;
        if anchor.topup_operation_id != operation_id {
            return Err(labeled_invariant(
                "topup_anchor_invalid",
                "persisted Kagemusha V4 top-up anchor is non-canonical or keyed incorrectly",
            )
            .into());
        }
        Ok(anchor)
    }
    fn ensure_kagemusha_v4_topup_anchor_absent(
        operation_id: [u8; 32],
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let key = kagemusha_v4_topup_anchor_state_key(operation_id)?;
        let drawdown_key = kagemusha_v4_topup_drawdown_state_key(operation_id)?;
        let existing = state_transaction.world.smart_contract_state.get(&key);
        crate::sumeragi::witness::record_read_kagemusha_v4_topup_anchor(
            operation_id,
            existing.map(|archive| archive.as_slice()),
        );
        let existing_drawdown = state_transaction
            .world
            .smart_contract_state
            .get(&drawdown_key);
        crate::sumeragi::witness::record_read_kagemusha_v4_topup_drawdown(
            operation_id,
            existing_drawdown.map(Vec::as_slice),
        );
        if existing.is_some() || existing_drawdown.is_some() {
            return Err(labeled_invariant(
                "authorization_replay",
                "Kagemusha V4 top-up anchor or drawdown balance exists without its complete replay-marker set",
            )
            .into());
        }
        Ok(())
    }
    fn persist_kagemusha_v4_topup_anchor(
        anchor: &KagemushaRecursiveSpendTopUpAnchorV4,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        anchor
            .validate_public_binding()
            .map_err(|err| labeled_invariant("topup_anchor_invalid", err.to_string()))?;
        let archive = norito::encode_canonical(anchor).map_err(|err| {
            labeled_invariant(
                "topup_anchor_invalid",
                format!("failed to encode Kagemusha V4 top-up anchor: {err}"),
            )
        })?;
        persist_kagemusha_v4_topup_anchor_archive(
            anchor.topup_operation_id,
            anchor.anchor_digest,
            archive,
            state_transaction,
        )
    }
    fn persist_kagemusha_v4_topup_anchor_archive(
        operation_id: [u8; 32],
        anchor_digest: [u8; 32],
        archive: Vec<u8>,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let key = kagemusha_v4_topup_anchor_state_key(operation_id)?;
        let drawdown_key = kagemusha_v4_topup_drawdown_state_key(operation_id)?;
        let existing = state_transaction.world.smart_contract_state.get(&key);
        let existing_drawdown = state_transaction
            .world
            .smart_contract_state
            .get(&drawdown_key);
        if existing.is_some() || existing_drawdown.is_some() {
            return Err(labeled_invariant(
                "authorization_replay",
                "Kagemusha V4 top-up operation already has a finalized anchor or drawdown balance",
            )
            .into());
        }
        crate::sumeragi::witness::record_read_kagemusha_v4_topup_anchor(operation_id, None);
        crate::sumeragi::witness::record_write_kagemusha_v4_topup_anchor(
            operation_id,
            &anchor_digest,
        );
        crate::sumeragi::witness::record_read_kagemusha_v4_topup_drawdown(operation_id, None);
        crate::sumeragi::witness::record_write_kagemusha_v4_topup_drawdown(operation_id, 0);
        state_transaction
            .world
            .smart_contract_state
            .insert(key, archive);
        state_transaction
            .world
            .smart_contract_state
            .insert(drawdown_key, 0_u128.to_le_bytes().to_vec());
        Ok(())
    }
    fn kagemusha_v4_redemption_receipt_state_key(
        operation_id: [u8; 32],
    ) -> Result<StatePath, Error> {
        format!("kagemusha_v4_redemption_{}", hex::encode(operation_id))
            .parse()
            .map_err(|err| {
                labeled_invariant(
                    "invalid_recursive_redeem",
                    format!("failed to derive Kagemusha V4 redemption receipt key: {err}"),
                )
                .into()
            })
    }
    fn ensure_kagemusha_v4_redemption_receipt_matches(
        operation_id: [u8; 32],
        payload_digest: [u8; 32],
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let key = kagemusha_v4_redemption_receipt_state_key(operation_id)?;
        let receipt = state_transaction
            .world
            .smart_contract_state
            .get(&key)
            .ok_or_else(|| {
                labeled_invariant(
                    "authorization_replay",
                    "Kagemusha V4 redemption replay marker has no committed receipt",
                )
            })?;
        if receipt.as_slice() != payload_digest.as_slice() {
            return Err(labeled_invariant(
                "authorization_replay",
                "Kagemusha V4 redemption receipt does not match the retried request",
            )
            .into());
        }
        Ok(())
    }
    fn ensure_kagemusha_v4_redemption_receipt_absent(
        operation_id: [u8; 32],
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<StatePath, Error> {
        let key = kagemusha_v4_redemption_receipt_state_key(operation_id)?;
        if state_transaction
            .world
            .smart_contract_state
            .get(&key)
            .is_some()
        {
            return Err(labeled_invariant(
                "authorization_replay",
                "Kagemusha V4 redemption receipt exists without its complete replay-marker set",
            )
            .into());
        }
        Ok(key)
    }
    struct KagemushaV4ResolvedTopUpProvenance {
        source_asset: AssetId,
    }
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct KagemushaV4AnchorDrawdownBalance {
        operation_id: [u8; 32],
        capacity_atomic_units: u128,
        redeemed_atomic_units: u128,
    }
    #[derive(Debug)]
    struct KagemushaV4AnchorDrawdownUpdate {
        operation_id: [u8; 32],
        state_key: StatePath,
        redeemed_atomic_units: u128,
    }
    fn allocate_kagemusha_v4_anchor_drawdown(
        balances: &[KagemushaV4AnchorDrawdownBalance],
        redemption_atomic_units: u128,
    ) -> Option<Vec<([u8; 32], u128)>> {
        let mut seen_operations = BTreeSet::new();
        if redemption_atomic_units == 0
            || balances.is_empty()
            || balances
                .iter()
                .any(|balance| balance.redeemed_atomic_units > balance.capacity_atomic_units)
            || balances
                .iter()
                .any(|balance| !seen_operations.insert(balance.operation_id))
        {
            return None;
        }
        let mut remaining = redemption_atomic_units;
        let mut updates = Vec::with_capacity(balances.len());
        for balance in balances {
            if remaining == 0 {
                break;
            }
            let available = balance
                .capacity_atomic_units
                .checked_sub(balance.redeemed_atomic_units)?;
            let debit = available.min(remaining);
            if debit == 0 {
                continue;
            }
            let redeemed_atomic_units = balance.redeemed_atomic_units.checked_add(debit)?;
            updates.push((balance.operation_id, redeemed_atomic_units));
            remaining = remaining.checked_sub(debit)?;
        }
        (remaining == 0).then_some(updates)
    }
    fn plan_kagemusha_v4_anchor_drawdown(
        anchor_refs: &[KagemushaRecursiveSpendTopUpAnchorRefV2],
        redemption_atomic_units: u128,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<Vec<KagemushaV4AnchorDrawdownUpdate>, Error> {
        let mut balances = Vec::with_capacity(anchor_refs.len());
        for anchor_ref in anchor_refs {
            let anchor =
                load_kagemusha_v4_topup_anchor(anchor_ref.topup_operation_id, state_transaction)?;
            balances.push((anchor_ref.topup_operation_id, anchor.amount.atomic_units));
        }
        plan_kagemusha_v4_anchor_drawdown_capacities(
            balances.as_slice(),
            redemption_atomic_units,
            state_transaction,
        )
    }
    fn plan_kagemusha_v4_anchor_drawdown_capacities(
        capacities: &[([u8; 32], u128)],
        redemption_atomic_units: u128,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<Vec<KagemushaV4AnchorDrawdownUpdate>, Error> {
        let balances = capacities
            .iter()
            .map(|(operation_id, capacity_atomic_units)| {
                Ok(KagemushaV4AnchorDrawdownBalance {
                    operation_id: *operation_id,
                    capacity_atomic_units: *capacity_atomic_units,
                    redeemed_atomic_units: load_kagemusha_v4_topup_drawdown(
                        *operation_id,
                        state_transaction,
                    )?,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let updates = allocate_kagemusha_v4_anchor_drawdown(
            balances.as_slice(),
            redemption_atomic_units,
        )
        .ok_or_else(|| {
            labeled_invariant(
                "topup_drawdown_exhausted",
                "Kagemusha V4 redemption exceeds the unredeemed balance of its finalized top-up anchors",
            )
        })?;
        updates
            .into_iter()
            .map(|(operation_id, redeemed_atomic_units)| {
                Ok(KagemushaV4AnchorDrawdownUpdate {
                    operation_id,
                    state_key: kagemusha_v4_topup_drawdown_state_key(operation_id)?,
                    redeemed_atomic_units,
                })
            })
            .collect()
    }
    fn commit_kagemusha_v4_anchor_drawdown(
        updates: Vec<KagemushaV4AnchorDrawdownUpdate>,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) {
        for update in updates {
            crate::sumeragi::witness::record_write_kagemusha_v4_topup_drawdown(
                update.operation_id,
                update.redeemed_atomic_units,
            );
            state_transaction.world.smart_contract_state.insert(
                update.state_key,
                update.redeemed_atomic_units.to_le_bytes().to_vec(),
            );
        }
    }
    fn validate_kagemusha_v4_finalized_topup_anchors(
        anchor_refs: &[KagemushaRecursiveSpendTopUpAnchorRefV2],
        current_note_atomic_units: u128,
        requested_height: u64,
        zk_state: &crate::state::ZkAssetState,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<KagemushaV4ResolvedTopUpProvenance, Error> {
        let mut canonical_source_asset = None;
        let mut seen_operations = BTreeSet::new();
        let mut anchored_total = 0_u128;
        for supplied_ref in anchor_refs {
            supplied_ref
                .validate()
                .map_err(|err| labeled_invariant("topup_anchor_invalid", err.to_string()))?;
            if !seen_operations.insert(supplied_ref.topup_operation_id) {
                return Err(labeled_invariant(
                    "topup_anchor_invalid",
                    "Kagemusha V4 redemption repeats a top-up operation anchor",
                )
                .into());
            }
            let persisted =
                load_kagemusha_v4_topup_anchor(supplied_ref.topup_operation_id, state_transaction)?;
            if persisted.anchor_digest != supplied_ref.anchor_digest
                || persisted
                    .compact_ref()
                    .map_err(|err| labeled_invariant("topup_anchor_invalid", err.to_string()))?
                    != *supplied_ref
            {
                return Err(labeled_invariant(
                    "topup_anchor_mismatch",
                    "Kagemusha V4 redemption anchor differs from the finalized chain receipt",
                )
                .into());
            }
            if persisted.finalized_height > requested_height
                || persisted.finalized_height > state_transaction.block_height()
            {
                return Err(labeled_invariant(
                    "topup_anchor_invalid",
                    "Kagemusha V4 redemption predates one of its finalized top-up anchors",
                )
                .into());
            }
            if !zk_state
                .commitments
                .contains(&persisted.current_note.note_commitment)
            {
                return Err(labeled_invariant(
                    "topup_anchor_mismatch",
                    "Kagemusha V4 finalized top-up evidence is inconsistent with confidential ledger state",
                )
                .into());
            }
            anchored_total = anchored_total
                .checked_add(persisted.amount.atomic_units)
                .ok_or_else(|| {
                    labeled_invariant(
                        "amount_mismatch",
                        "Kagemusha V4 finalized top-up amount total overflows u128",
                    )
                })?;
            let canonical = canonical_kagemusha_asset_id(state_transaction, &persisted.asset)?;
            match &canonical_source_asset {
                None => canonical_source_asset = Some(canonical),
                Some(source) if source.scope() == canonical.scope() => {}
                Some(_) => {
                    return Err(labeled_invariant(
                        "asset_mismatch",
                        "Kagemusha V4 cannot join top-up anchors from different asset-balance scopes",
                    )
                    .into());
                }
            }
        }
        if anchored_total < current_note_atomic_units {
            return Err(labeled_invariant(
                "amount_mismatch",
                "Kagemusha V4 spendable note exceeds its finalized top-up provenance",
            )
            .into());
        }
        let source_asset = canonical_source_asset.ok_or_else(|| {
            Error::from(labeled_invariant(
                "topup_anchor_missing",
                "Kagemusha V4 redemption has no finalized top-up provenance",
            ))
        })?;
        Ok(KagemushaV4ResolvedTopUpProvenance { source_asset })
    }
    #[derive(Debug)]
    struct KagemushaV2EscrowCreditPlan {
        operation_id: [u8; 32],
        escrow_asset: AssetId,
        recipient_asset: AssetId,
        amount: Quantity,
    }
    impl KagemushaV2EscrowCreditPlan {
        fn commit(self, state_transaction: &mut StateTransaction<'_, '_>) -> Result<(), Error> {
            let authorization = VerifiedKagemushaRedemptionDebit::new(
                self.operation_id,
                self.escrow_asset,
                self.recipient_asset,
                self.amount,
            );
            crate::smartcontracts::isi::asset::isi::execute_verified_offline_redemption_transfer(
                state_transaction,
                authorization,
            )?;
            Ok(())
        }
    }
    fn plan_kagemusha_v2_escrow_credit(
        operation_id: [u8; 32],
        source_asset: &AssetId,
        recipient: &AccountId,
        amount: &Quantity,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<KagemushaV2EscrowCreditPlan, Error> {
        let definition_id = source_asset.definition().clone();
        state_transaction.world.asset_definition(&definition_id)?;
        let escrow_account = crate::smartcontracts::isi::domain::isi::offline_escrow_account_id(
            state_transaction.network_id(),
            &definition_id,
        );
        state_transaction.world.account(recipient)?;
        state_transaction.world.account(&escrow_account)?;
        ensure_distinct_offline_escrow_account(
            &escrow_account,
            recipient,
            "recipient",
            &definition_id,
        )?;
        let recipient_asset = AssetId::with_scope(
            definition_id,
            recipient.clone(),
            source_asset.scope().clone(),
        );
        let escrow_asset = kagemusha_escrow_asset_id(source_asset, escrow_account);
        state_transaction
            .world
            .precheck_numeric_asset_transfer_delta_exact(&escrow_asset, &recipient_asset, amount)?;
        Ok(KagemushaV2EscrowCreditPlan {
            operation_id,
            escrow_asset,
            recipient_asset,
            amount: amount.clone(),
        })
    }
    fn is_zero_hash(hash: &Hash) -> bool {
        hash.as_ref().iter().all(|byte| *byte == 0)
    }
    fn world_has_offline_permission(
        world: &impl WorldReadOnly,
        authority: &AccountId,
        required: &Permission,
    ) -> bool {
        // These first-release capabilities carry no scope. Match the complete
        // canonical permission so a same-name token with attacker-controlled
        // payload cannot acquire administrative authority.
        if world
            .account_permissions()
            .get(authority)
            .is_some_and(|permissions| permissions.contains(required))
        {
            return true;
        }
        world.account_roles_iter(authority).any(|role_id| {
            world
                .roles()
                .get(role_id)
                .is_some_and(|role| role.permissions().any(|permission| permission == required))
        })
    }
    /// Canonical unit-valued permission required to manage offline escrow.
    pub fn offline_escrow_manager_permission() -> Permission {
        Permission::new(
            CAN_MANAGE_OFFLINE_ESCROW_PERMISSION.into(),
            iroha_primitives::json::Json::new(()),
        )
    }
    /// Return whether an account holds the exact offline escrow permission,
    /// either directly or through an assigned role.
    pub fn world_has_offline_escrow_manager_permission(
        world: &impl WorldReadOnly,
        authority: &AccountId,
    ) -> bool {
        let required = offline_escrow_manager_permission();
        world_has_offline_permission(world, authority, &required)
    }
    fn can_activate_kagemusha_recursive_release_v4(
        world: &impl WorldReadOnly,
        authority: &AccountId,
    ) -> bool {
        let required = Permission::new(
            CAN_ACTIVATE_KAGEMUSHA_RECURSIVE_RELEASE_V4_PERMISSION.into(),
            iroha_primitives::json::Json::new(()),
        );
        world_has_offline_permission(world, authority, &required)
    }
    pub(super) fn ensure_kagemusha_recursive_release_v4_activation_authorized(
        state_transaction: &StateTransaction<'_, '_>,
        authority: &AccountId,
    ) -> Result<(), Error> {
        if !can_activate_kagemusha_recursive_release_v4(&state_transaction.world, authority) {
            return Err(labeled_invariant(
                "unauthorized_controller",
                "Kagemusha V4 release activation requires CanActivateKagemushaRecursiveReleaseV4",
            )
            .into());
        }
        if !can_manage_offline_device_attestation_policy(state_transaction, authority) {
            return Err(labeled_invariant(
                "unauthorized_controller",
                "Kagemusha V4 release activation also requires CanManageOfflineDeviceAttestationPolicy",
            )
            .into());
        }
        Ok(())
    }
    fn is_offline_escrow_manager(
        authority: &AccountId,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> bool {
        world_has_offline_escrow_manager_permission(&state_transaction.world, authority)
    }
    fn ensure_can_submit_kagemusha_for_account(
        account: &AccountId,
        authority: &AccountId,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if account == authority || is_offline_escrow_manager(authority, state_transaction) {
            Ok(())
        } else {
            Err(labeled_invariant(
                "unauthorized_controller",
                "only the Kagemusha account or an offline escrow manager may submit this request",
            )
            .into())
        }
    }
    fn ensure_can_submit_kagemusha_topup(
        asset: &AssetId,
        authority: &AccountId,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if asset.account() == authority || is_offline_escrow_manager(authority, state_transaction) {
            Ok(())
        } else {
            Err(labeled_invariant(
                "unauthorized_controller",
                "only the top-up payer or an offline escrow manager may submit recursive Kagemusha top-ups",
            )
            .into())
        }
    }
    fn authenticate_kagemusha_v4_topup_submission_before_replay(
        asset: &AssetId,
        authority: &AccountId,
        authorization: &KagemushaRequestAuthorizationV2,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(KagemushaAuthenticatedDeviceV1, KagemushaV4ReplayStatus), Error> {
        // A committed marker is consensus state, not an authentication oracle. Authenticate the
        // transaction submitter and the registered hardware assertion before consulting it.
        ensure_can_submit_kagemusha_topup(asset, authority, state_transaction)?;
        let authenticated = authenticate_registered_kagemusha_v2_device(
            authorization,
            asset.definition(),
            state_transaction,
        )?;
        let replay = kagemusha_v4_replay_status(authorization, state_transaction)?;
        Ok((authenticated, replay))
    }
    fn authenticate_kagemusha_v4_redeem_submission_before_replay(
        recipient: &AccountId,
        asset: &AssetDefinitionId,
        authority: &AccountId,
        authorization: &KagemushaRequestAuthorizationV2,
        release_policy: &OfflineDeviceAttestationPolicy,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(KagemushaAuthenticatedDeviceV1, KagemushaV4ReplayStatus), Error> {
        // Preserve the same auth-before-state boundary for redemption receipts and markers.
        ensure_can_submit_kagemusha_for_account(recipient, authority, state_transaction)?;
        let authenticated = authenticate_registered_kagemusha_v2_device_against_policy(
            authorization,
            asset,
            release_policy,
            state_transaction,
        )?;
        let replay = kagemusha_v4_replay_status(authorization, state_transaction)?;
        Ok((authenticated, replay))
    }
    fn validate_offline_attestation_recent_block(
        registration: &OfflineDeviceAttestationRegistration,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        reject_invalid_attestation!(
            registration.recent_block_height == 0,
            "offline device attestation must bind a committed block height",
        );
        let committed_height = state_transaction.block_hashes().len() as u64;
        reject_invalid_attestation!(
            registration.recent_block_height > committed_height,
            "offline device attestation references a block height that is not committed",
        );
        if committed_height.saturating_sub(registration.recent_block_height)
            > KAGEMUSHA_ATTESTATION_RECENT_BLOCK_WINDOW
        {
            return Err(labeled_invariant(
                "stale_attestation",
                "offline device attestation challenge is outside the recent block window",
            )
            .into());
        }
        let block_hash = state_transaction
            .block_hashes()
            .get(registration.recent_block_height.saturating_sub(1) as usize)
            .ok_or_else(|| {
                invalid_attestation(
                    "offline device attestation references a missing committed block",
                )
            })?;
        reject_invalid_attestation!(
            block_hash.as_ref() != registration.recent_block_hash.as_ref(),
            "offline device attestation recent block hash does not match ledger state",
        );
        Ok(())
    }
    fn validate_offline_attestation_platform_profile(
        registration: &OfflineDeviceAttestationRegistration,
    ) -> Result<(), Error> {
        validate_p256_uncompressed_public_key(&registration.assertion_public_key)?;
        match registration.platform.as_str() {
            OFFLINE_ATTESTATION_PLATFORM_IOS_APP_ATTEST => {
                reject_invalid_attestation!(
                    registration.assertion_scheme != OFFLINE_ATTESTATION_IOS_ASSERTION_SCHEME
                        || registration.assertion_key_algorithm
                            != OFFLINE_ATTESTATION_IOS_ASSERTION_ALGORITHM
                        || registration.assertion_usage_count_limit.is_some()
                        || registration.android_attested_device_properties.is_some(),
                    "iOS App Attest registrations must use the canonical App Attest assertion profile",
                );
            }
            OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT => {
                reject_invalid_attestation!(
                    registration.assertion_scheme != OFFLINE_ATTESTATION_ANDROID_ASSERTION_SCHEME
                        || registration.assertion_key_algorithm
                            != OFFLINE_ATTESTATION_ANDROID_ASSERTION_ALGORITHM
                        || registration.assertion_usage_count_limit != Some(1)
                        || !registration.one_use,
                    "Android KeyMint registrations must use the canonical one-use P-256 assertion profile",
                );
            }
            _ => {
                return Err(invalid_attestation(
                    "offline device attestation platform is unsupported",
                )
                .into());
            }
        }
        Ok(())
    }
    fn validate_optional_attestation_metadata_string(
        value: Option<&str>,
        field: &'static str,
        maximum_bytes: usize,
    ) -> Result<(), Error> {
        let Some(value) = value else {
            return Ok(());
        };
        reject_invalid_attestation!(
            value.trim().is_empty(),
            format!("offline device attestation {field} must not be empty when present"),
        );
        reject_invalid_attestation!(
            value.trim() != value,
            format!("offline device attestation {field} must not contain surrounding whitespace"),
        );
        reject_invalid_attestation!(
            value.len() > maximum_bytes || value.chars().any(char::is_control),
            format!(
                "offline device attestation {field} must contain at most {maximum_bytes} bytes and no control characters"
            ),
        );
        Ok(())
    }
    fn validate_offline_attestation_optional_metadata(
        registration: &OfflineDeviceAttestationRegistration,
    ) -> Result<(), Error> {
        for (field, value, maximum_bytes) in [
            (
                "ios_team_id",
                registration.ios_team_id.as_deref(),
                OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TEAM_ID_BYTES_V1,
            ),
            (
                "ios_bundle_id",
                registration.ios_bundle_id.as_deref(),
                OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_APP_IDENTIFIER_BYTES_V1,
            ),
            (
                "ios_environment",
                registration.ios_environment.as_deref(),
                OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_APP_IDENTIFIER_BYTES_V1,
            ),
            (
                "android_package_name",
                registration.android_package_name.as_deref(),
                OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_APP_IDENTIFIER_BYTES_V1,
            ),
        ] {
            validate_optional_attestation_metadata_string(value, field, maximum_bytes)?;
        }
        Ok(())
    }
    include!("offline/device_attestation_cbor_helpers.rs");
    fn decode_ios_app_attest_extensions(
        input: &[u8],
        source: &str,
        validation_category_key: &str,
        bundle_version_key: &str,
    ) -> Result<IosAppAttestExtensionProperties, Error> {
        let mut offset = 0usize;
        let (major, entries) = read_definite_cbor_header(input, &mut offset, source)?;
        reject_invalid_attestation!(
            major != 5 || entries != 2,
            format!("iOS App Attest {source} extensions must be one two-entry CBOR map"),
        );
        let mut validation_category = None;
        let mut bundle_version = None;
        for _ in 0..2 {
            let key = read_definite_cbor_text(input, &mut offset, source)?;
            if key == validation_category_key && validation_category.is_none() {
                let (major, value) = read_definite_cbor_header(input, &mut offset, source)?;
                reject_invalid_attestation!(
                    major != 0,
                    format!(
                        "iOS App Attest {source} validation category must be an unsigned integer"
                    ),
                );
                validation_category = Some(u32::try_from(value).map_err(|_| {
                    labeled_invariant(
                        "invalid_attestation",
                        format!("iOS App Attest {source} validation category is out of range"),
                    )
                })?);
            } else if key == bundle_version_key && bundle_version.is_none() {
                bundle_version =
                    Some(read_definite_cbor_text(input, &mut offset, source)?.to_owned());
            } else {
                return Err(invalid_attestation(
                    format!(
                        "iOS App Attest {source} extensions must contain each exact {validation_category_key}/{bundle_version_key} key once"
                    ),
                )
                .into());
            }
        }
        reject_invalid_attestation!(
            offset != input.len(),
            format!("iOS App Attest {source} extensions have trailing CBOR bytes"),
        );
        let validation_category = validation_category.expect("both exact extension keys checked");
        reject_invalid_attestation!(
            !matches!(validation_category, 1..=6 | 10),
            format!("iOS App Attest {source} validation category is unsupported"),
        );
        let bundle_version = bundle_version.expect("both exact extension keys checked");
        reject_invalid_attestation!(
            bundle_version.is_empty()
                || bundle_version.len()
                    > OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_BUNDLE_VERSION_BYTES_V1
                || !bundle_version.is_ascii()
                || bundle_version.trim() != bundle_version
                || bundle_version.chars().any(char::is_control),
            format!("iOS App Attest {source} bundle version is invalid"),
        );
        Ok(IosAppAttestExtensionProperties {
            validation_category,
            bundle_version,
        })
    }
    fn decode_ios_app_attest_attestation_extensions(
        input: &[u8],
    ) -> Result<IosAppAttestExtensionProperties, Error> {
        decode_ios_app_attest_extensions(
            input,
            "attestation",
            "apple_validation_category_01",
            "apple_bundle_version_01",
        )
    }
    fn decode_ios_app_attest_assertion_extensions(
        input: &[u8],
    ) -> Result<IosAppAttestExtensionProperties, Error> {
        decode_ios_app_attest_extensions(input, "assertion", "validationCategory", "bundleVersion")
    }
    fn parse_ios_app_attest_report(
        registration: &OfflineDeviceAttestationRegistration,
    ) -> Result<IosAppAttestReport, Error> {
        let value = decode_cbor_value_exact(
            &registration.attestation_report,
            "iOS App Attest report must be a CBOR attestation object",
            "iOS App Attest report has trailing CBOR bytes",
        )?;
        let ciborium::value::Value::Map(map) = value else {
            return Err(invalid_attestation("iOS App Attest report must be a CBOR map").into());
        };
        reject_invalid_attestation!(
            cbor_text_value(&map, "fmt")?.as_deref() != Some("apple-appattest"),
            "iOS App Attest report format must be apple-appattest",
        );
        let auth_data = cbor_bytes_value(&map, "authData")?
            .ok_or_else(|| invalid_attestation("iOS App Attest report is missing authData"))?;
        let att_stmt = cbor_map_value(&map, "attStmt")?
            .ok_or_else(|| invalid_attestation("iOS App Attest report is missing attStmt"))?;
        let x5c = cbor_array_value(att_stmt, "x5c")?.ok_or_else(|| {
            invalid_attestation("iOS App Attest report is missing certificate chain")
        })?;
        reject_invalid_attestation!(
            !(2..=OFFLINE_ATTESTATION_MAX_IOS_X509_CHAIN_CERTIFICATES).contains(&x5c.len()),
            "iOS App Attest report must include a bounded certificate chain",
        );
        let mut certificates = Vec::with_capacity(x5c.len());
        for value in x5c {
            let ciborium::value::Value::Bytes(certificate) = value else {
                return Err(invalid_attestation(
                    "iOS App Attest certificate chain entries must be bytes",
                )
                .into());
            };
            reject_invalid_attestation!(
                certificate.is_empty()
                    || certificate.len() > OFFLINE_ATTESTATION_MAX_X509_CERTIFICATE_BYTES,
                "iOS App Attest certificate chain entry size is outside protocol bounds",
            );
            certificates.push(certificate.clone());
        }
        Ok(IosAppAttestReport {
            auth_data,
            certificates,
        })
    }
    fn parse_ios_app_attest_auth_data(auth_data: &[u8]) -> Result<IosAppAttestAuthData, Error> {
        reject_invalid_attestation!(
            !(OFFLINE_ATTESTATION_APP_ATTEST_AUTH_DATA_MIN_LEN
                ..=OFFLINE_ATTESTATION_APP_ATTEST_AUTH_DATA_MAX_LEN)
                .contains(&auth_data.len()),
            "iOS App Attest authData length is outside protocol bounds",
        );
        let flags = auth_data[32];
        let allowed_flags = OFFLINE_ATTESTATION_APP_ATTEST_FLAG_USER_PRESENT
            | OFFLINE_ATTESTATION_APP_ATTEST_FLAG_USER_VERIFIED
            | OFFLINE_ATTESTATION_APP_ATTEST_FLAG_ATTESTED_CREDENTIAL_DATA
            | OFFLINE_ATTESTATION_APP_ATTEST_FLAG_EXTENSION_DATA;
        reject_invalid_attestation!(
            flags & OFFLINE_ATTESTATION_APP_ATTEST_FLAG_ATTESTED_CREDENTIAL_DATA == 0
                || flags & OFFLINE_ATTESTATION_APP_ATTEST_FLAG_EXTENSION_DATA == 0
                || flags & !allowed_flags != 0,
            "iOS App Attest authData flags are invalid or missing attested credential or extension data",
        );
        let rp_id_hash = auth_data[0..32]
            .try_into()
            .expect("authData length already checked");
        let sign_count = u32::from_be_bytes(
            auth_data[33..37]
                .try_into()
                .expect("authData length already checked"),
        );
        let aaguid = auth_data[37..53]
            .try_into()
            .expect("authData length already checked");
        let credential_id_len = u16::from_be_bytes(
            auth_data[53..55]
                .try_into()
                .expect("authData length already checked"),
        ) as usize;
        let credential_id_start = 55usize;
        let credential_id_end = credential_id_start.saturating_add(credential_id_len);
        reject_invalid_attestation!(
            credential_id_end > auth_data.len(),
            "iOS App Attest credential id exceeds authData bounds",
        );
        let credential_and_extensions = &auth_data[credential_id_end..];
        reject_invalid_attestation!(
            credential_and_extensions.is_empty(),
            "iOS App Attest credential public key is missing",
        );
        let mut cursor = Cursor::new(credential_and_extensions);
        let _: ciborium::value::Value = ciborium::de::from_reader(&mut cursor).map_err(|_| {
            labeled_invariant(
                "invalid_attestation",
                "iOS App Attest credential public key must be one CBOR value",
            )
        })?;
        let cose_key_end = usize::try_from(cursor.position()).map_err(|_| {
            labeled_invariant(
                "invalid_attestation",
                "iOS App Attest credential public key length is out of range",
            )
        })?;
        let cose_key = credential_and_extensions[..cose_key_end].to_vec();
        let extension_bytes = &credential_and_extensions[cose_key_end..];
        reject_invalid_attestation!(
            extension_bytes.is_empty(),
            "iOS App Attest authData sets ED without extension data",
        );
        let extensions = decode_ios_app_attest_attestation_extensions(extension_bytes)?;
        Ok(IosAppAttestAuthData {
            rp_id_hash,
            sign_count,
            aaguid,
            credential_id: auth_data[credential_id_start..credential_id_end].to_vec(),
            cose_key,
            extensions,
        })
    }
    fn parse_ios_app_attest_assertion_auth_data(
        auth_data: &[u8],
    ) -> Result<IosAppAttestAssertionAuthData, Error> {
        if !(KAGEMUSHA_IOS_APP_ATTEST_ASSERTION_AUTH_DATA_MIN_BYTES_V1
            ..=KAGEMUSHA_IOS_APP_ATTEST_ASSERTION_AUTH_DATA_MAX_BYTES_V1)
            .contains(&auth_data.len())
        {
            return Err(labeled_invariant(
                "invalid_authorization",
                "iOS App Attest assertion authData length is outside protocol bounds",
            )
            .into());
        }
        let rp_id_hash = auth_data[..32]
            .try_into()
            .expect("App Attest assertion minimum length is checked");
        let flags = auth_data[32];
        if flags != OFFLINE_ATTESTATION_APP_ATTEST_FLAG_EXTENSION_DATA {
            return Err(labeled_invariant(
                "invalid_authorization",
                "iOS App Attest assertion authData must contain only the extension-data flag",
            )
            .into());
        }
        let sign_count = u32::from_be_bytes(
            auth_data[33..37]
                .try_into()
                .expect("App Attest assertion minimum length is checked"),
        );
        let extension_bytes =
            &auth_data[KAGEMUSHA_IOS_APP_ATTEST_ASSERTION_AUTH_DATA_FIXED_HEADER_BYTES_V1..];
        let extensions = decode_ios_app_attest_assertion_extensions(extension_bytes)?;
        Ok(IosAppAttestAssertionAuthData {
            rp_id_hash,
            sign_count,
            extensions,
        })
    }
    fn validate_ios_app_attest_assertion_identity(
        auth_data: &IosAppAttestAssertionAuthData,
        expected_rp_id_hash: [u8; 32],
    ) -> Result<(), Error> {
        if auth_data.rp_id_hash != expected_rp_id_hash || auth_data.sign_count == 0 {
            return Err(labeled_invariant(
                "invalid_authorization",
                "iOS App Attest RP hash or assertion counter is invalid",
            )
            .into());
        }
        Ok(())
    }
    #[cfg(test)]
    fn validate_ios_app_attest_assertion_binding(
        auth_data: &IosAppAttestAssertionAuthData,
        expected_rp_id_hash: [u8; 32],
        last_sign_count: u32,
    ) -> Result<(), Error> {
        validate_ios_app_attest_assertion_identity(auth_data, expected_rp_id_hash)?;
        if auth_data.sign_count <= last_sign_count {
            return Err(labeled_invariant(
                "invalid_authorization",
                "iOS App Attest assertion counter must advance strictly",
            )
            .into());
        }
        Ok(())
    }
    fn ios_attestation_metadata(
        registration: &OfflineDeviceAttestationRegistration,
    ) -> Result<(String, String, String), Error> {
        let team_id = registration
            .ios_team_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_ascii_uppercase)
            .ok_or_else(|| {
                labeled_invariant(
                    "invalid_attestation",
                    "iOS App Attest registration is missing the Apple Team ID",
                )
            })?;
        let bundle_id = registration
            .ios_bundle_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| {
                labeled_invariant(
                    "invalid_attestation",
                    "iOS App Attest registration is missing the bundle identifier",
                )
            })?;
        let environment = registration
            .ios_environment
            .as_deref()
            .map(str::trim)
            .map(str::to_ascii_lowercase)
            .ok_or_else(|| {
                labeled_invariant(
                    "invalid_attestation",
                    "iOS App Attest registration is missing the environment",
                )
            })?;
        if environment != OFFLINE_ATTESTATION_IOS_ENV_PRODUCTION
            && environment != OFFLINE_ATTESTATION_IOS_ENV_DEVELOPMENT
        {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest environment must be production or development",
            )
            .into());
        }
        Ok((team_id, bundle_id, environment))
    }
    fn ios_app_policy_matches(
        policy: &OfflineIosAppAttestationPolicy,
        team_id: &str,
        bundle_id: &str,
        environment: &str,
    ) -> bool {
        policy.team_id.eq_ignore_ascii_case(team_id)
            && policy.bundle_id == bundle_id
            && policy.environment.eq_ignore_ascii_case(environment)
    }
    fn ensure_ios_app_allowed_by_policy<'a>(
        policy: &'a OfflineDeviceAttestationPolicy,
        team_id: &str,
        bundle_id: &str,
        environment: &str,
    ) -> Result<&'a OfflineIosAppAttestationPolicy, Error> {
        if !policy.require_ios_app_policy {
            return Err(labeled_invariant(
                "invalid_attestation_policy",
                "iOS App Attest is not explicitly enabled by Offline device attestation policy",
            )
            .into());
        }
        if let Some(app) = policy
            .ios_apps
            .iter()
            .find(|app| ios_app_policy_matches(app, team_id, bundle_id, environment))
        {
            return Ok(app);
        }
        Err(labeled_invariant(
            "invalid_attestation_policy",
            "iOS App Attest app identity is not allowed by Offline device attestation policy",
        )
        .into())
    }
    fn validate_ios_app_attest_extensions_against_policy(
        app_policy: &OfflineIosAppAttestationPolicy,
        extensions: &IosAppAttestExtensionProperties,
    ) -> Result<(), Error> {
        if app_policy
            .allowed_validation_categories
            .contains(&extensions.validation_category)
            && app_policy
                .allowed_bundle_versions
                .contains(&extensions.bundle_version)
        {
            Ok(())
        } else {
            Err(labeled_invariant(
                "invalid_attestation_policy",
                "iOS App Attest validation category or bundle version is not allowed for this app",
            )
            .into())
        }
    }
    fn android_attestation_metadata(
        registration: &OfflineDeviceAttestationRegistration,
    ) -> Result<(String, [u8; 32]), Error> {
        let package_name = registration
            .android_package_name
            .as_deref()
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| {
                labeled_invariant(
                    "invalid_attestation",
                    "Android KeyMint registration is missing the package name",
                )
            })?;
        let signing_digest = registration
            .android_signing_certificate_sha256
            .as_deref()
            .ok_or_else(|| {
                labeled_invariant(
                    "invalid_attestation",
                    "Android KeyMint registration is missing the signing certificate digest",
                )
            })
            .and_then(|digest| {
                digest.try_into().map_err(|_| {
                    labeled_invariant(
                        "invalid_attestation",
                        "Android KeyMint signing certificate digest must be 32 bytes",
                    )
                    .into()
                })
            })?;
        if signing_digest == [0u8; 32] {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint signing certificate digest must be non-zero",
            )
            .into());
        }
        Ok((package_name, signing_digest))
    }
    fn android_app_policy_matches(
        policy: &OfflineAndroidAppAttestationPolicy,
        package_name: &str,
        signing_digest: &[u8; 32],
    ) -> bool {
        policy.package_name == package_name
            && policy
                .signing_certificate_sha256
                .iter()
                .any(|candidate| candidate.as_slice() == signing_digest)
    }
    fn ensure_android_app_allowed_by_policy(
        policy: &OfflineDeviceAttestationPolicy,
        package_name: &str,
        signing_digest: &[u8; 32],
    ) -> Result<(), Error> {
        if !policy.require_android_app_policy {
            return Err(labeled_invariant(
                "invalid_attestation_policy",
                "Android KeyMint is not explicitly enabled by Offline device attestation policy",
            )
            .into());
        }
        if policy
            .android_apps
            .iter()
            .any(|app| android_app_policy_matches(app, package_name, signing_digest))
        {
            return Ok(());
        }
        Err(labeled_invariant(
            "invalid_attestation_policy",
            "Android KeyMint app identity is not allowed by Offline device attestation policy",
        )
        .into())
    }
    fn validate_android_key_id(
        registration: &OfflineDeviceAttestationRegistration,
    ) -> Result<(), Error> {
        let expected_key_id = hex::encode(sha256_bytes(&registration.assertion_public_key));
        if registration.key_id != expected_key_id {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint key_id must be lowercase hex SHA-256 of the assertion public key",
            )
            .into());
        }
        Ok(())
    }
    fn extract_der_octet_string(input: &[u8], depth: usize) -> Result<Vec<u8>, Error> {
        if depth > 4 {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest nonce extension is too deeply nested",
            )
            .into());
        }
        let mut reader = DerReader::new(input);
        let (tag, value) = reader.read_tlv()?;
        if reader.has_remaining() {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest nonce extension has trailing DER bytes",
            )
            .into());
        }
        match tag {
            0x04 => Ok(value.to_vec()),
            0x30 | 0xA1 => extract_der_octet_string(value, depth + 1),
            _ => Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest nonce extension must contain an OCTET STRING",
            )
            .into()),
        }
    }
    fn validate_app_attest_cose_p256_key(
        cose_key_bytes: &[u8],
        expected_public_key: &[u8],
    ) -> Result<(), Error> {
        let value = decode_cbor_value_exact(
            cose_key_bytes,
            "iOS App Attest credential public key must be CBOR",
            "iOS App Attest credential public key has trailing CBOR bytes",
        )?;
        let ciborium::value::Value::Map(map) = value else {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest credential public key must be a COSE map",
            )
            .into());
        };
        if cbor_int_value(&map, 1)? != Some(2)
            || cbor_int_value(&map, -1)? != Some(1)
            || cbor_int_value(&map, 3)?.is_some_and(|alg| alg != -7)
        {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest credential public key must be ES256 P-256",
            )
            .into());
        }
        let x = cbor_bytes_value_i(&map, -2)?.ok_or_else(|| {
            labeled_invariant(
                "invalid_attestation",
                "iOS App Attest credential public key is missing an x coordinate",
            )
        })?;
        let y = cbor_bytes_value_i(&map, -3)?.ok_or_else(|| {
            labeled_invariant(
                "invalid_attestation",
                "iOS App Attest credential public key is missing a y coordinate",
            )
        })?;
        if x.len() != 32 || y.len() != 32 {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest credential public key coordinates must be 32 bytes",
            )
            .into());
        }
        let mut public_key =
            Vec::with_capacity(OFFLINE_ATTESTATION_P256_UNCOMPRESSED_PUBLIC_KEY_LEN);
        public_key.push(0x04);
        public_key.extend_from_slice(&x);
        public_key.extend_from_slice(&y);
        if public_key != expected_public_key {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest credential public key does not match the registered assertion key",
            )
            .into());
        }
        Ok(())
    }
    fn validate_ios_app_attest_report(
        registration: &OfflineDeviceAttestationRegistration,
        policy: &OfflineDeviceAttestationPolicy,
        block_unix_timestamp_ms: u64,
    ) -> Result<(), Error> {
        let report = parse_ios_app_attest_report(registration)?;
        let auth_data = parse_ios_app_attest_auth_data(&report.auth_data)?;
        if auth_data.sign_count != 0 {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest attestation counter must start at zero",
            )
            .into());
        }
        let (team_id, bundle_id, environment) = ios_attestation_metadata(registration)?;
        let app_policy =
            ensure_ios_app_allowed_by_policy(policy, &team_id, &bundle_id, &environment)?;
        validate_ios_app_attest_extensions_against_policy(app_policy, &auth_data.extensions)?;
        let expected_aaguid = if environment == OFFLINE_ATTESTATION_IOS_ENV_DEVELOPMENT {
            OFFLINE_ATTESTATION_IOS_AAGUID_DEVELOPMENT
        } else {
            OFFLINE_ATTESTATION_IOS_AAGUID_PRODUCTION
        };
        if auth_data.aaguid.as_slice() != expected_aaguid {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest AAGUID does not match the registered environment",
            )
            .into());
        }
        let rp_id = format!("{team_id}.{bundle_id}");
        if auth_data.rp_id_hash != sha256_bytes(rp_id.as_bytes()) {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest app identity hash does not match Team ID and bundle ID",
            )
            .into());
        }
        let expected_key_id = decode_canonical_ios_app_attest_key_id(&registration.key_id)?;
        if auth_data.credential_id != expected_key_id {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest credential id does not match key_id",
            )
            .into());
        }
        validate_app_attest_cose_p256_key(&auth_data.cose_key, &registration.assertion_public_key)?;
        let trusted_roots =
            trusted_root_der_for_platform(policy, &registration.platform, block_unix_timestamp_ms)?;
        let revoked_certificate_tbs_sha256 = policy_revoked_certificate_tbs_hashes(policy)?;
        let evaluation_time = x509_evaluation_time(block_unix_timestamp_ms)?;
        validate_attestation_certificate_chain(
            &report.certificates,
            &trusted_roots,
            &revoked_certificate_tbs_sha256,
            evaluation_time,
        )?;
        let leaf = parse_x509_certificate_der(&report.certificates[0])?;
        let leaf_public_key = x509_subject_public_key_bytes(&leaf);
        if leaf_public_key != registration.assertion_public_key {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest certificate public key does not match the registered assertion key",
            )
            .into());
        }
        if sha256_bytes(&leaf_public_key).as_slice() != expected_key_id.as_slice() {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest certificate public key hash does not match key_id",
            )
            .into());
        }
        let nonce_extension = x509_unique_extension_value(
            &leaf,
            OFFLINE_ATTESTATION_APP_ATTEST_NONCE_OID,
            "iOS App Attest certificate contains duplicate nonce extensions",
        )?
        .ok_or_else(|| {
            labeled_invariant(
                "invalid_attestation",
                "iOS App Attest certificate is missing the nonce extension",
            )
        })?;
        let nonce = extract_der_octet_string(&nonce_extension, 0)?;
        let expected_nonce = sha256_concat(&report.auth_data, registration.challenge_hash.as_ref());
        if nonce != expected_nonce {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest nonce extension does not bind the attestation challenge",
            )
            .into());
        }
        Ok(())
    }
    fn decode_canonical_ios_app_attest_key_id(
        key_id: &str,
    ) -> Result<Vec<u8>, InstructionExecutionError> {
        let decoded = BASE64_STANDARD
            .decode(key_id.as_bytes())
            .map_err(|_| invalid_ios_app_attest_key_id())?;
        if decoded.is_empty() || BASE64_STANDARD.encode(&decoded) != key_id {
            return Err(invalid_ios_app_attest_key_id());
        }
        Ok(decoded)
    }
    fn invalid_ios_app_attest_key_id() -> InstructionExecutionError {
        labeled_invariant(
            "invalid_attestation",
            "iOS App Attest key_id must be canonical standard base64 credential bytes",
        )
    }
    fn parse_android_keymint_report(
        registration: &OfflineDeviceAttestationRegistration,
    ) -> Result<AndroidKeyMintReport, Error> {
        let value = decode_cbor_value_exact(
            &registration.attestation_report,
            "Android KeyMint report must be a CBOR certificate array",
            "Android KeyMint report has trailing CBOR bytes",
        )?;
        let ciborium::value::Value::Array(certificates) = value else {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint report must be a CBOR certificate array",
            )
            .into());
        };
        if !(1..=OFFLINE_ATTESTATION_MAX_X509_CHAIN_CERTIFICATES).contains(&certificates.len()) {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint report must include a bounded certificate chain",
            )
            .into());
        }
        let mut certificate_der = Vec::with_capacity(certificates.len());
        for value in certificates {
            let ciborium::value::Value::Bytes(certificate) = value else {
                return Err(labeled_invariant(
                    "invalid_attestation",
                    "Android KeyMint certificate entries must be bytes",
                )
                .into());
            };
            if certificate.is_empty()
                || certificate.len() > OFFLINE_ATTESTATION_MAX_X509_CERTIFICATE_BYTES
            {
                return Err(labeled_invariant(
                    "invalid_attestation",
                    "Android KeyMint certificate entry size is outside protocol bounds",
                )
                .into());
            }
            certificate_der.push(certificate);
        }
        Ok(AndroidKeyMintReport {
            certificates: certificate_der,
        })
    }
    fn der_single_integer(input: &[u8]) -> Result<i64, Error> {
        let mut reader = DerReader::new(input);
        reader.read_integer().and_then(|value| {
            if reader.has_remaining() {
                Err(labeled_invariant(
                    "invalid_attestation",
                    "Android KeyMint authorization value has trailing bytes",
                )
                .into())
            } else {
                Ok(value)
            }
        })
    }
    fn der_single_octet_string(input: &[u8]) -> Result<Vec<u8>, Error> {
        let mut reader = DerReader::new(input);
        let value = reader.read_octet_string()?;
        if reader.has_remaining() {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint authorization OCTET STRING has trailing bytes",
            )
            .into());
        }
        Ok(value)
    }
    include!("offline/android_attestation_authorization_validation.rs");
    fn parse_android_key_description(
        extension_value: &[u8],
    ) -> Result<AndroidKeyDescription, Error> {
        let mut reader = DerReader::sequence(extension_value)?;
        let attestation_version = reader.read_integer()?;
        if attestation_version <= 0 {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint attestation version must be positive",
            )
            .into());
        }
        let attestation_security_level = reader.read_enumerated()?;
        let keymint_version = reader.read_integer()?;
        if keymint_version <= 0 {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint version must be positive",
            )
            .into());
        }
        let keymint_security_level = reader.read_enumerated()?;
        let attestation_challenge = reader.read_octet_string()?;
        let _unique_id = reader.read_octet_string()?;
        let software_enforced = reader.read_sequence_bytes()?;
        let hardware_enforced = reader.read_sequence_bytes()?;
        if reader.has_remaining() {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint attestation extension has trailing fields",
            )
            .into());
        }
        let software = parse_android_authorization_list(&software_enforced, false)?;
        let hardware = parse_android_authorization_list(&hardware_enforced, true)?;
        if software.usage_count_limit.is_some() {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint usageCountLimit must be hardwareEnforced, not softwareEnforced",
            )
            .into());
        }
        if software.all_applications && hardware.all_applications {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint authorization lists duplicate allApplications",
            )
            .into());
        }
        if software.application_id.is_some() && hardware.application_id.is_some() {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint authorization lists duplicate attestationApplicationId",
            )
            .into());
        }
        let root_of_trust = hardware.root_of_trust.ok_or_else(|| {
            labeled_invariant(
                "invalid_attestation",
                "Android KeyMint attestation must contain exactly one hardware rootOfTrust",
            )
        })?;
        if attestation_security_level != keymint_security_level
            || !is_android_hardware_security_level(attestation_security_level)
        {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint attestation and key security levels must be the same hardware boundary",
            )
            .into());
        }
        let security_level = match keymint_security_level {
            OFFLINE_ATTESTATION_ANDROID_SECURITY_LEVEL_TRUSTED_ENVIRONMENT => {
                OfflineAndroidDeviceSecurityLevelV2::TrustedEnvironment
            }
            OFFLINE_ATTESTATION_ANDROID_SECURITY_LEVEL_STRONG_BOX => {
                OfflineAndroidDeviceSecurityLevelV2::StrongBox
            }
            _ => unreachable!("hardware security level was checked"),
        };
        let positive_u32 = |value: Option<i64>| {
            value
                .and_then(|value| u32::try_from(value).ok())
                .filter(|value| *value > 0)
                .unwrap_or_default()
        };
        let attestation_version = u32::try_from(attestation_version).map_err(|_| {
            labeled_invariant(
                "invalid_attestation",
                "Android KeyMint attestation version exceeds the supported range",
            )
        })?;
        let keymint_version = u32::try_from(keymint_version).map_err(|_| {
            labeled_invariant(
                "invalid_attestation",
                "Android KeyMint version exceeds the supported range",
            )
        })?;
        Ok(AndroidKeyDescription {
            attestation_challenge,
            usage_count_limit: hardware.usage_count_limit,
            all_applications: software.all_applications || hardware.all_applications,
            application_id: software.application_id.or(hardware.application_id),
            device_properties: OfflineAndroidAttestedDevicePropertiesV2 {
                version:
                    iroha_data_model::offline::OFFLINE_ANDROID_ATTESTED_DEVICE_PROPERTIES_VERSION_V2,
                attestation_version,
                keymint_version,
                security_level,
                brand: hardware.brand.unwrap_or_default(),
                device: hardware.device.unwrap_or_default(),
                product: hardware.product.unwrap_or_default(),
                manufacturer: hardware.manufacturer.unwrap_or_default(),
                model: hardware.model.unwrap_or_default(),
                os_version: positive_u32(hardware.os_version),
                os_patch_level: positive_u32(hardware.os_patch_level),
                vendor_patch_level: positive_u32(hardware.vendor_patch_level),
                boot_patch_level: positive_u32(hardware.boot_patch_level),
                verified_boot_key: root_of_trust.verified_boot_key,
                verified_boot_hash: root_of_trust.verified_boot_hash,
            },
        })
    }
    fn is_android_hardware_security_level(level: i64) -> bool {
        level == OFFLINE_ATTESTATION_ANDROID_SECURITY_LEVEL_TRUSTED_ENVIRONMENT
            || level == OFFLINE_ATTESTATION_ANDROID_SECURITY_LEVEL_STRONG_BOX
    }
    fn validate_android_keymint_report(
        registration: &OfflineDeviceAttestationRegistration,
        policy: &OfflineDeviceAttestationPolicy,
        block_unix_timestamp_ms: u64,
    ) -> Result<(), Error> {
        let report = parse_android_keymint_report(registration)?;
        let trusted_roots =
            trusted_root_der_for_platform(policy, &registration.platform, block_unix_timestamp_ms)?;
        let revoked_certificate_tbs_sha256 = policy_revoked_certificate_tbs_hashes(policy)?;
        let status_snapshot = policy.android_status_snapshot.as_ref().ok_or_else(|| {
            labeled_invariant(
                "invalid_attestation_policy",
                "Android attestation requires a governed status snapshot",
            )
        })?;
        let non_valid_certificate_serials = status_snapshot
            .non_valid_serials
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        let evaluation_time = x509_evaluation_time(block_unix_timestamp_ms)?;
        // Registrations are valid on `[admitted_at_ms, expires_at_ms)`, so the
        // final certificate-validity point is the preceding millisecond.
        let registration_last_valid_time = x509_evaluation_time(
            registration
                .expires_at_ms
                .checked_sub(1)
                .ok_or_else(|| invalid_attestation("Android registration expiry underflows"))?,
        )?;
        validate_android_key_attestation_certificate_chain(
            &report.certificates,
            &trusted_roots,
            &revoked_certificate_tbs_sha256,
            &non_valid_certificate_serials,
            evaluation_time,
            registration_last_valid_time,
        )?;
        let attested_certificate_der = report.certificates.first().ok_or_else(|| {
            labeled_invariant(
                "invalid_attestation",
                "Android KeyMint certificate chain is missing the attested leaf certificate",
            )
        })?;
        let attested_certificate = parse_x509_certificate_der(attested_certificate_der)?;
        let extension_value = android_keymint_leaf_attestation_extension(&report.certificates)?;
        let key_description = parse_android_key_description(&extension_value)?;
        if key_description.attestation_challenge != registration.challenge_hash.as_ref() {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint attestation challenge does not match the canonical challenge",
            )
            .into());
        }
        if key_description.usage_count_limit != Some(1) {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint attestation must bind usageCountLimit to one",
            )
            .into());
        }
        if key_description.all_applications {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint attestation must not be scoped to all applications",
            )
            .into());
        }
        let (package_name, signing_digest) = android_attestation_metadata(registration)?;
        ensure_android_app_allowed_by_policy(policy, &package_name, &signing_digest)?;
        let application_id = key_description.application_id.ok_or_else(|| {
            labeled_invariant(
                "invalid_attestation",
                "Android KeyMint attestation is missing attestationApplicationId",
            )
        })?;
        validate_android_attestation_application_id_matches(
            &application_id,
            &package_name,
            &signing_digest,
        )?;
        let submitted_properties = registration
            .android_attested_device_properties
            .as_ref()
            .ok_or_else(|| {
                labeled_invariant(
                    "invalid_attestation",
                    "Android KeyMint registration V2 must carry the exact attested device properties",
                )
            })?;
        if submitted_properties != &key_description.device_properties {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint registration device properties do not match the hardware KeyDescription",
            )
            .into());
        }
        let eligibility =
            policy.evaluate_verified_android_device_v2(Some(submitted_properties), true);
        if eligibility.outcome
            != iroha_data_model::offline::OfflineDeviceEligibilityOutcomeV1::Eligible
        {
            return Err(labeled_invariant(
                "drain_only",
                format!(
                    "Android KeyMint device is not eligible for new offline activity: {:?}",
                    eligibility.reason
                ),
            )
            .into());
        }
        let subject_public_key = x509_subject_public_key_bytes(&attested_certificate);
        if subject_public_key != registration.assertion_public_key {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint certificate public key does not match the registered assertion key",
            )
            .into());
        }
        // Android cannot challenge-bind `key_id`: KeyMint creates this public
        // key while processing the challenge. Bind it here, after the leaf
        // certificate key has been authenticated, so a submitted identifier
        // cannot select or substitute a different assertion key.
        validate_android_key_id(registration)?;
        Ok(())
    }
    fn validate_offline_attestation_report(
        registration: &OfflineDeviceAttestationRegistration,
        policy: &OfflineDeviceAttestationPolicy,
        block_unix_timestamp_ms: u64,
    ) -> Result<(), Error> {
        match registration.platform.as_str() {
            OFFLINE_ATTESTATION_PLATFORM_IOS_APP_ATTEST => {
                validate_ios_app_attest_report(registration, policy, block_unix_timestamp_ms)
            }
            OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT => {
                validate_android_keymint_report(registration, policy, block_unix_timestamp_ms)
            }
            _ => Err(labeled_invariant(
                "invalid_attestation",
                "offline device attestation platform is unsupported",
            )
            .into()),
        }
    }
    fn validate_offline_attestation_evidence_bytes(
        registration: &OfflineDeviceAttestationRegistration,
    ) -> Result<(), Error> {
        if registration.attestation_report.is_empty() || registration.evidence.is_empty() {
            return Err(labeled_invariant(
                "invalid_attestation",
                "offline device attestation report and evidence bytes must be non-empty",
            )
            .into());
        }
        if registration.attestation_report.len() > OFFLINE_ATTESTATION_MAX_REPORT_BYTES {
            return Err(labeled_invariant(
                "invalid_attestation",
                "offline device attestation report exceeds the on-chain size limit",
            )
            .into());
        }
        if registration.evidence.len() > OFFLINE_ATTESTATION_MAX_EVIDENCE_BYTES {
            return Err(labeled_invariant(
                "invalid_attestation",
                "offline device attestation evidence exceeds the on-chain size limit",
            )
            .into());
        }
        if Hash::new(&registration.attestation_report) != registration.attestation_report_hash {
            return Err(labeled_invariant(
                "invalid_attestation",
                "offline device attestation report hash does not match report bytes",
            )
            .into());
        }
        if Hash::new(&registration.evidence) != registration.evidence_hash {
            return Err(labeled_invariant(
                "invalid_attestation",
                "offline device attestation evidence hash does not match evidence bytes",
            )
            .into());
        }
        if registration.evidence.len() != OFFLINE_ATTESTATION_EVIDENCE_PREFIX.len() + Hash::LENGTH
            || !registration
                .evidence
                .starts_with(OFFLINE_ATTESTATION_EVIDENCE_PREFIX)
            || &registration.evidence[OFFLINE_ATTESTATION_EVIDENCE_PREFIX.len()..]
                != registration.attestation_report_hash.as_ref()
        {
            return Err(labeled_invariant(
                "invalid_attestation",
                "offline device attestation evidence envelope must bind the attestation report hash",
            )
            .into());
        }
        Ok(())
    }
    include!("offline/device_attestation_registration_validation.rs");
    impl Execute for RegisterOfflineDeviceAttestation {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let registration = self.registration;
            let (registration_hash, admission_policy_hash) =
                validate_offline_device_attestation_registration(
                    &registration,
                    authority,
                    state_transaction,
                )?;
            let admission_height = state_transaction.block_height();
            let admission_transaction_hash = state_transaction
                .current_tx_hash
                .ok_or_else(|| {
                    labeled_invariant(
                        "invalid_attestation",
                        "current signed transaction hash is unavailable for device-registration provenance",
                    )
                })?;
            let replay_keys = kagemusha_registration_replay_keys(&registration, &registration_hash);
            let registration_hash_bytes = exact_hash_bytes(&registration_hash);
            let registration_state_key =
                kagemusha_online_registration_state_key(&registration_hash_bytes)?;
            ensure_kagemusha_registration_replay_keys_are_fresh(&replay_keys, state_transaction)?;
            if state_transaction
                .world
                .smart_contract_state
                .get(&registration_state_key)
                .is_some()
            {
                return Err(labeled_invariant(
                    "duplicate_attestation",
                    "Kagemusha device attestation registration state already exists",
                )
                .into());
            }
            let registration_admission_plan = plan_kagemusha_online_registration_admission_v1(
                &registration,
                admission_policy_hash,
                state_transaction,
            )?;
            let lifecycle = match registration.platform.as_str() {
                OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT => {
                    KagemushaOnlineHardwareAssertionLifecycleV1::AndroidKeyMintUnused
                }
                OFFLINE_ATTESTATION_PLATFORM_IOS_APP_ATTEST => {
                    KagemushaOnlineHardwareAssertionLifecycleV1::IosAppAttest {
                        last_sign_count: 0,
                        last_consumption: None,
                    }
                }
                _ => {
                    return Err(labeled_invariant(
                        "invalid_attestation",
                        "offline device attestation platform is unsupported",
                    )
                    .into());
                }
            };
            let registration = compact_kagemusha_registration_projection(&registration);
            let registration_projection_hash =
                canonical_registration_hash(&registration).map(|hash| exact_hash_bytes(&hash))?;
            let registration_state_archive = encode_kagemusha_online_registration_state_v4(
                &KagemushaOnlineRegistrationStateV4 {
                    version: KAGEMUSHA_ONLINE_REGISTRATION_STATE_VERSION_V4,
                    original_registration_hash: registration_hash_bytes,
                    registration_projection_hash,
                    admission_policy_hash,
                    admission_height,
                    admission_transaction_hash,
                    registration,
                    lifecycle,
                },
            )?;
            registration_admission_plan.commit(state_transaction);
            for replay_key in replay_keys {
                state_transaction
                    .world
                    .kagemusha_replay_keys
                    .insert(replay_key, ());
            }
            state_transaction
                .world
                .smart_contract_state
                .insert(registration_state_key, registration_state_archive);
            Ok(())
        }
    }
    impl Execute for SetOfflineDeviceAttestationPolicy {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if !can_manage_offline_device_attestation_policy(state_transaction, authority) {
                return Err(labeled_invariant(
                    "unauthorized_controller",
                    "only an Offline device attestation policy manager may update verifier policy",
                )
                .into());
            }
            let policy = self.policy;
            validate_offline_attestation_policy(
                &policy,
                state_transaction.block_unix_timestamp_ms(),
            )?;
            validate_offline_attestation_policy_transition_from_state(&policy, state_transaction)?;
            let bytes = norito::encode_canonical(&policy).map_err(|err| {
                labeled_invariant(
                    "invalid_attestation_policy",
                    format!("failed to encode Offline device attestation policy: {err}"),
                )
            })?;
            state_transaction.world.smart_contract_state.insert(
                (*OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY).clone(),
                bytes,
            );
            Ok(())
        }
    }
    fn offline_device_attestation_policy_manager_permission() -> Permission {
        Permission::new(
            CAN_MANAGE_OFFLINE_DEVICE_ATTESTATION_POLICY_PERMISSION.into(),
            iroha_primitives::json::Json::new(()),
        )
    }
    fn can_manage_offline_device_attestation_policy(
        state_transaction: &StateTransaction<'_, '_>,
        authority: &AccountId,
    ) -> bool {
        let required = offline_device_attestation_policy_manager_permission();
        world_has_offline_permission(&state_transaction.world, authority, &required)
    }
    fn ensure_kagemusha_v4_topup_shield_public_inputs(
        request: &iroha_data_model::offline::KagemushaRecursiveSpendTopUpRequestV4,
        authoritative_initial_root: [u8; 32],
        authoritative_finalized_root: [u8; 32],
        authoritative_leaf_index: u32,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let public = crate::zk::confidential_v2::parse_kagemusha_topup_shield_public_inputs_v2(
            &request.shield_evidence.proof.proof.bytes,
        )
        .map_err(|err| labeled_invariant("invalid_proof", err))?;
        let expected_asset_tag = crate::zk::confidential_v2::derive_confidential_asset_tag_v2(
            &request.asset.definition().to_string(),
        );
        let expected_network_tag = crate::zk::confidential_v2::derive_confidential_network_tag_v2(
            state_transaction.network_id(),
        );
        let expected_payer_tag = crate::zk::confidential_v2::derive_kagemusha_topup_payer_tag_v2(
            &request.authorization.authority.to_string(),
        );
        let expected_operation_tag =
            crate::zk::confidential_v2::derive_kagemusha_topup_operation_tag_v2(
                &request.operation_id,
            );
        if request.shield_evidence.initial_root != authoritative_initial_root
            || request.shield_evidence.finalized_root != authoritative_finalized_root
            || request.shield_evidence.leaf_index != authoritative_leaf_index
            || public.output_commitment != request.current_note.note_commitment
            || public.spend_nullifier != request.current_note.spend_nullifier
            || public.initial_root != authoritative_initial_root
            || public.finalized_root != authoritative_finalized_root
            || public.atomic_amount
                != crate::zk::confidential_v2::encode_confidential_amount_v2(
                    request.amount.atomic_units,
                )
            || public.asset_scale
                != crate::zk::confidential_v2::encode_kagemusha_topup_u32_v2(request.amount.scale)
            || public.leaf_index
                != crate::zk::confidential_v2::encode_kagemusha_topup_u32_v2(
                    authoritative_leaf_index,
                )
            || public.asset_tag != expected_asset_tag
            || public.network_tag != expected_network_tag
            || public.payer_tag != expected_payer_tag
            || public.operation_tag != expected_operation_tag
        {
            return Err(labeled_invariant(
                "proof_binding",
                "Kagemusha top-up shield proof does not bind the authoritative amount, scale, note, tree, asset, chain, payer, and operation",
            )
            .into());
        }
        Ok(())
    }
    /// Reject overlap between a new top-up note and either confidential-state namespace.
    fn ensure_kagemusha_v4_topup_note_is_fresh(
        zk_state: &crate::state::ZkAssetState,
        note_commitment: [u8; 32],
        spend_nullifier: [u8; 32],
    ) -> Result<(), InstructionExecutionError> {
        if zk_state.commitments.contains(&note_commitment) {
            return Err(labeled_invariant(
                "duplicate_output",
                "Kagemusha top-up note commitment already exists",
            ));
        }
        if zk_state.nullifiers.contains(&note_commitment) {
            return Err(labeled_invariant(
                "proof_binding",
                "Kagemusha top-up note commitment collides with an already spent nullifier",
            ));
        }
        if zk_state.nullifiers.contains(&spend_nullifier)
            || zk_state.commitments.contains(&spend_nullifier)
        {
            return Err(labeled_invariant(
                "duplicate_nullifier",
                "Kagemusha top-up spend nullifier collides with existing confidential state",
            ));
        }
        Ok(())
    }
    fn ensure_kagemusha_v4_anchor_matches_topup_request(
        anchor: &KagemushaRecursiveSpendTopUpAnchorV4,
        request: &iroha_data_model::offline::KagemushaRecursiveSpendTopUpRequestV4,
    ) -> Result<(), Error> {
        if anchor.network_id != request.current_note.network_id
            || anchor.payer != request.authorization.authority
            || anchor.asset != request.asset
            || anchor.asset_scale != request.amount.scale
            || anchor.amount != request.amount
            || anchor.initial_root != request.shield_evidence.initial_root
            || anchor.finalized_root != request.shield_evidence.finalized_root
            || anchor.shield_leaf_index != request.shield_evidence.leaf_index
            || anchor.current_note != request.current_note
            || anchor.topup_operation_id != request.operation_id
            || anchor.shield_verifier_id != request.shield_evidence.proof.vk_ref
            || Some(anchor.shield_verifier_commitment)
                != request.shield_evidence.proof.vk_commitment
            || anchor.artifact_binding != request.artifact_binding
        {
            return Err(labeled_invariant(
                "topup_anchor_mismatch",
                "persisted Kagemusha V4 top-up anchor does not match the signed request",
            )
            .into());
        }
        Ok(())
    }
    fn finalized_kagemusha_v4_topup_anchor(
        request: &iroha_data_model::offline::KagemushaRecursiveSpendTopUpRequestV4,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<KagemushaRecursiveSpendTopUpAnchorV4, Error> {
        let shield_verifier_commitment =
            request.shield_evidence.proof.vk_commitment.ok_or_else(|| {
                labeled_invariant(
                    "verifier_key_invalid",
                    "Kagemusha V4 top-up shield proof has no verifier commitment",
                )
            })?;
        let finalized_tx_hash = *state_transaction
            .current_tx_hash
            .as_ref()
            .ok_or_else(|| {
                labeled_invariant(
                    "topup_anchor_invalid",
                    "current signed transaction hash is unavailable for Kagemusha V4 top-up",
                )
            })?
            .as_ref();
        let anchor = KagemushaRecursiveSpendTopUpAnchorV4 {
            version: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_TOPUP_ANCHOR_VERSION_V4,
            network_id: request.current_note.network_id,
            payer: request.authorization.authority.clone(),
            asset: request.asset.clone(),
            asset_scale: request.amount.scale,
            amount: request.amount,
            initial_root: request.shield_evidence.initial_root,
            finalized_root: request.shield_evidence.finalized_root,
            shield_leaf_index: request.shield_evidence.leaf_index,
            current_note: request.current_note.clone(),
            topup_operation_id: request.operation_id,
            shield_verifier_id: request.shield_evidence.proof.vk_ref.clone(),
            shield_verifier_commitment,
            artifact_binding: request.artifact_binding.clone(),
            finalized_height: state_transaction.block_height(),
            finalized_tx_hash,
            anchor_digest: [0; 32],
        }
        .finalize_digest()
        .map_err(|err| labeled_invariant("topup_anchor_invalid", err.to_string()))?;
        ensure_kagemusha_v4_anchor_matches_topup_request(&anchor, request)?;
        Ok(anchor)
    }
    struct KagemushaResolvedTransactionReleaseV4 {
        cached: std::sync::Arc<kagemusha_terminal_registry_v4::KagemushaCachedReleaseV4>,
        issuance_active: bool,
    }
    fn resolve_kagemusha_v4_transaction_release(
        binding: &iroha_data_model::offline::KagemushaRecursiveSpendArtifactBindingV4,
        requested_height: u64,
        network_id: &iroha_data_model::NetworkId,
        asset: &AssetDefinitionId,
        asset_scale: u32,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<KagemushaResolvedTransactionReleaseV4, Error> {
        let release = super::resolve_kagemusha_recursive_transaction_release_v4(
            &state_transaction.world,
            &state_transaction.kagemusha_release_catalog,
            binding,
            requested_height,
            state_transaction.block_height(),
            network_id,
            asset,
            asset_scale,
        )
        .map_err(|error| labeled_invariant("recursive_release_mismatch", error))?;
        let cached = state_transaction
            .kagemusha_release_catalog
            .resolve_binding(binding)
            .map_err(|error| labeled_invariant("verifier_key_invalid", error))?
            .clone();
        Ok(KagemushaResolvedTransactionReleaseV4 {
            cached,
            issuance_active: release.issuance_active,
        })
    }
    fn verify_kagemusha_v4_recursive_bundle(
        bundle: &iroha_data_model::offline::KagemushaRecursiveSpendBundleV4,
        verifier: &crate::zk::kagemusha_v2::KagemushaPastaCycleOpaqueVerifierV4,
    ) -> Result<(), Error> {
        verifier
            .verify_bundle_v4(bundle)
            .map_err(|error| labeled_invariant("invalid_recursive_bundle", error).into())
    }
    struct KagemushaV4RedemptionCommitPlan {
        definition_id: AssetDefinitionId,
        zk_asset_state: crate::state::ZkAssetState,
        escrow_credit: KagemushaV2EscrowCreditPlan,
        anchor_drawdown: Vec<KagemushaV4AnchorDrawdownUpdate>,
        branch_commit: KagemushaV2BranchCommitPlan,
        receipt_key: StatePath,
        receipt_digest: [u8; 32],
        replay_markers: [Hash; 4],
    }
    impl KagemushaV4RedemptionCommitPlan {
        fn commit(self, state_transaction: &mut StateTransaction<'_, '_>) -> Result<(), Error> {
            // The balance move is the only fallible ledger mutation remaining.
            // Every proof, conflict marker, tree update, and receipt collision was
            // validated while constructing this plan.
            self.escrow_credit.commit(state_transaction)?;
            state_transaction
                .world
                .zk_assets
                .remove(self.definition_id.clone());
            state_transaction
                .world
                .zk_assets
                .insert(self.definition_id, self.zk_asset_state);
            commit_kagemusha_v4_anchor_drawdown(self.anchor_drawdown, state_transaction);
            self.branch_commit.commit(state_transaction);
            state_transaction
                .world
                .smart_contract_state
                .insert(self.receipt_key, self.receipt_digest.to_vec());
            commit_kagemusha_v4_replay_markers(self.replay_markers, state_transaction);
            Ok(())
        }
    }
    struct KagemushaV4RedemptionPlanInput<'a> {
        definition_id: &'a AssetDefinitionId,
        source_asset: &'a AssetId,
        recipient: &'a AccountId,
        amount: Quantity,
        redemption_atomic_units: u128,
        topup_anchor_refs: &'a [KagemushaRecursiveSpendTopUpAnchorRefV2],
        current_nullifier: [u8; 32],
        consumed_claims: &'a [KagemushaRecursiveSpendBranchClaimV2],
        redemption_binding: Option<[u8; 32]>,
        change_output: Option<&'a iroha_data_model::offline::KagemushaSpendableNoteDescriptorV2>,
        change_children: &'a [(
            KagemushaRecursiveSpendBranchClaimV2,
            KagemushaRecursiveSpendBranchClaimV2,
        )],
        operation_id: [u8; 32],
        receipt_digest: [u8; 32],
        replay_markers: [Hash; 4],
    }
    fn plan_kagemusha_v4_redemption_state_commit(
        input: KagemushaV4RedemptionPlanInput<'_>,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<KagemushaV4RedemptionCommitPlan, Error> {
        let mut zk_asset_state = state_transaction
            .world
            .zk_assets
            .get(input.definition_id)
            .cloned()
            .ok_or_else(|| {
                labeled_invariant(
                    "verifier_key_invalid",
                    "Kagemusha V4 redemption requires configured shielded asset state",
                )
            })?;
        if zk_asset_state.nullifiers.contains(&input.current_nullifier) {
            return Err(labeled_invariant(
                "duplicate_nullifier",
                "Kagemusha V4 spendable-note nullifier is already redeemed",
            )
            .into());
        }
        if zk_asset_state
            .commitments
            .contains(&input.current_nullifier)
        {
            return Err(labeled_invariant(
                "proof_binding",
                "Kagemusha V4 spendable-note nullifier collides with a confidential commitment",
            )
            .into());
        }
        if let Some(change) = input.change_output {
            if zk_asset_state.commitments.contains(&change.note_commitment) {
                return Err(labeled_invariant(
                    "duplicate_output",
                    "Kagemusha V4 redemption change commitment already exists",
                )
                .into());
            }
            if zk_asset_state.nullifiers.contains(&change.spend_nullifier)
                || change.spend_nullifier == input.current_nullifier
            {
                return Err(labeled_invariant(
                    "duplicate_nullifier",
                    "Kagemusha V4 redemption change nullifier collides with ledger state",
                )
                .into());
            }
            if change.note_commitment == input.current_nullifier
                || zk_asset_state.nullifiers.contains(&change.note_commitment)
                || zk_asset_state.commitments.contains(&change.spend_nullifier)
            {
                return Err(labeled_invariant(
                    "proof_binding",
                    "Kagemusha V4 redemption change material overlaps an existing commitment or nullifier",
                )
                .into());
            }
        }
        let branch_commit = plan_kagemusha_v2_consumed_branch_set(
            input.consumed_claims,
            input.redemption_binding,
            input.change_children,
            state_transaction,
        )?;
        let anchor_drawdown = plan_kagemusha_v4_anchor_drawdown(
            input.topup_anchor_refs,
            input.redemption_atomic_units,
            state_transaction,
        )?;
        if !zk_asset_state.nullifiers.insert(input.current_nullifier) {
            unreachable!("the V4 nullifier was checked before insertion into the cloned state");
        }
        if let Some(change) = input.change_output {
            crate::smartcontracts::isi::world::isi::push_confidential_commitment_for_asset(
                &mut zk_asset_state,
                change.note_commitment,
                state_transaction,
            )?;
            let _frontier_update = zk_asset_state
                .record_frontier_checkpoint(
                    state_transaction.block_height(),
                    state_transaction.zk.tree_frontier_checkpoint_interval,
                    state_transaction.zk.reorg_depth_bound,
                )
                .map_err(|err| {
                    labeled_invariant(
                        "confidential_tree",
                        format!("failed to checkpoint canonical confidential tree: {err}"),
                    )
                })?;
        }
        let escrow_credit = plan_kagemusha_v2_escrow_credit(
            input.operation_id,
            input.source_asset,
            input.recipient,
            &input.amount,
            state_transaction,
        )?;
        let receipt_key =
            ensure_kagemusha_v4_redemption_receipt_absent(input.operation_id, state_transaction)?;
        Ok(KagemushaV4RedemptionCommitPlan {
            definition_id: input.definition_id.clone(),
            zk_asset_state,
            escrow_credit,
            anchor_drawdown,
            branch_commit,
            receipt_key,
            receipt_digest: input.receipt_digest,
            replay_markers: input.replay_markers,
        })
    }
    fn plan_kagemusha_v4_redemption_commit(
        request: &iroha_data_model::offline::KagemushaRecursiveSpendRedeemRequestV4,
        source_asset: &AssetId,
        receipt_digest: [u8; 32],
        replay_markers: [Hash; 4],
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<KagemushaV4RedemptionCommitPlan, Error> {
        let statement = &request.bundle.statement;
        let amount = request.amount.public_quantity();
        let current_nullifier = statement.current_note.spend_nullifier;
        let (redemption_binding, change_children) =
            if let Some(change) = request.offline_change.as_ref() {
                let binding = request
                    .redemption
                    .binding_digest()
                    .map_err(|err| labeled_invariant("proof_binding", err.to_string()))?;
                let children = request
                    .redemption
                    .parent_branch_claims
                    .iter()
                    .cloned()
                    .zip(change.branch_claims.iter().cloned())
                    .collect::<Vec<_>>();
                (Some(binding), children)
            } else {
                (None, Vec::new())
            };
        plan_kagemusha_v4_redemption_state_commit(
            KagemushaV4RedemptionPlanInput {
                definition_id: &statement.asset,
                source_asset,
                recipient: &request.recipient,
                amount,
                redemption_atomic_units: request.amount.atomic_units,
                topup_anchor_refs: &statement.topup_anchor_refs,
                current_nullifier,
                consumed_claims: &request.redemption.parent_branch_claims,
                redemption_binding,
                change_output: request.offline_change.as_ref().map(|change| &change.output),
                change_children: &change_children,
                operation_id: request.operation_id,
                receipt_digest,
                replay_markers,
            },
            state_transaction,
        )
    }
    fn ensure_kagemusha_v4_policy_will_be_convertible(
        definition_id: &AssetDefinitionId,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let definition = state_transaction.world.asset_definition(definition_id)?;
        let mut policy = *definition.confidential_policy();
        let block_height = state_transaction.block_height();
        if let Some(transition) = policy.pending_transition
            && block_height >= transition.effective_height()
            && transition.new_mode() == ConfidentialPolicyMode::ShieldedOnly
            && state_transaction.world.asset_total_amount(definition_id)? > Quantity::zero()
        {
            // `apply_policy_if_due` aborts a due ShieldedOnly transition while
            // transparent supply remains. Retain the current mode because an
            // opened conversion window is irreversible in ABI V1.
            policy.pending_transition = None;
        } else {
            policy = policy.apply_if_due(block_height).0;
        }
        if policy.mode() != ConfidentialPolicyMode::Convertible {
            return Err(labeled_invariant(
                "unshield_not_permitted",
                "Kagemusha V4 operation is not permitted by confidential asset policy",
            )
            .into());
        }
        Ok(())
    }
    fn ensure_kagemusha_v4_redemption_live_context(
        bundle_network_id: &iroha_data_model::NetworkId,
        redemption_network_id: &iroha_data_model::NetworkId,
        live_network_id: &iroha_data_model::NetworkId,
        amount_scale: u32,
        statement_scale: u32,
        live_scale: u32,
    ) -> Result<(), Error> {
        if bundle_network_id != live_network_id || redemption_network_id != live_network_id {
            return Err(labeled_invariant(
                "wrong_network",
                "Kagemusha V4 redemption NetworkId does not match this network",
            )
            .into());
        }
        if amount_scale != live_scale || statement_scale != live_scale {
            return Err(labeled_invariant(
                "amount_scale_mismatch",
                "Kagemusha V4 redemption scale does not equal the live asset scale",
            )
            .into());
        }
        Ok(())
    }
    fn kagemusha_v4_release_binding(
        release_record: &iroha_data_model::offline::KagemushaRecursiveSpendReleaseRecordV4,
    ) -> Result<iroha_data_model::offline::KagemushaRecursiveSpendArtifactBindingV4, Error> {
        let manifest_bytes =
            norito::encode_canonical(&release_record.manifest).map_err(|error| {
                labeled_invariant(
                    "recursive_release_invalid",
                    format!("failed to encode Kagemusha V4 manifest: {error}"),
                )
            })?;
        Ok(
            iroha_data_model::offline::KagemushaRecursiveSpendArtifactBindingV4 {
                version: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
                generation: release_record.manifest.generation.clone(),
                manifest_sha256: Sha256::digest(manifest_bytes).into(),
            },
        )
    }
    fn kagemusha_v4_next_verifier_version(
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<u32, Error> {
        let maximum_for = |circuit_id: &str| {
            state_transaction
                .world
                .verifying_keys_by_circuit
                .iter()
                .filter_map(|((indexed_circuit, version), _)| {
                    (indexed_circuit == circuit_id).then_some(*version)
                })
                .max()
                .unwrap_or(0)
        };
        let step_eq =
            maximum_for(iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4);
        let step_ep =
            maximum_for(iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4);
        if step_eq != step_ep {
            return Err(labeled_invariant(
                "recursive_release_overlap",
                "existing Kagemusha V4 Eq/Ep verifier version history is not atomic",
            )
            .into());
        }
        step_eq.checked_add(1).ok_or_else(|| {
            labeled_invariant(
                "recursive_release_invalid",
                "Kagemusha V4 verifier version exhausted u32",
            )
            .into()
        })
    }
    fn ensure_kagemusha_v4_non_overlapping_issuance(
        binding: &iroha_data_model::offline::KagemushaRecursiveSpendArtifactBindingV4,
        manifest: &iroha_data_model::offline::KagemushaRecursiveSpendArtifactManifestV4,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        for (state_key, payload) in state_transaction.world.smart_contract_state.iter() {
            let Some((other_binding, release_record)) =
                decode_kagemusha_v4_consensus_release_state(state_key, payload)
                    .map_err(|error| labeled_invariant("recursive_release_invalid", error))?
            else {
                continue;
            };
            if other_binding.manifest_sha256 == binding.manifest_sha256 {
                continue;
            }
            let cached = state_transaction
                .kagemusha_release_catalog
                .resolve_binding(&other_binding)
                .map_err(|error| labeled_invariant("recursive_release_invalid", error))?;
            if cached.release_record() != &release_record {
                return Err(labeled_invariant(
                    "recursive_release_invalid",
                    "Kagemusha V4 consensus release record differs from local authenticated material",
                )
                .into());
            }
            let other = &release_record.manifest;
            let same_scope = other.network_id == manifest.network_id
                && other.asset == manifest.asset
                && other.asset_scale == manifest.asset_scale;
            let overlaps = kagemusha_v4_issuance_windows_overlap(
                manifest.activation_height,
                manifest.withdrawal_height,
                other.activation_height,
                other.withdrawal_height,
            );
            if same_scope && overlaps {
                return Err(labeled_invariant(
                    "recursive_release_overlap",
                    "Kagemusha V4 issuance windows overlap for the same chain, asset, and scale",
                )
                .into());
            }
        }
        Ok(())
    }
    impl Execute for TopUpKagemushaRecursiveV4 {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let request = self.request;
            request
                .validate_public_binding()
                .map_err(|err| labeled_invariant("invalid_recursive_topup", err.to_string()))?;
            request
                .validate_authorization_at(state_transaction.block_unix_timestamp_ms())
                .map_err(|err| labeled_invariant("invalid_authorization", err.to_string()))?;
            if request.asset.account() != &request.authorization.authority {
                return Err(labeled_invariant(
                    "unauthorized_controller",
                    "Kagemusha V4 top-up authority must equal the charged asset account",
                )
                .into());
            }
            let (authenticated_device, replay_status) =
                authenticate_kagemusha_v4_topup_submission_before_replay(
                    &request.asset,
                    authority,
                    &request.authorization,
                    state_transaction,
                )?;
            let replay_markers = match replay_status {
                KagemushaV4ReplayStatus::Committed => {
                    let anchor = load_kagemusha_v4_topup_anchor(
                        request.authorization.operation_id,
                        state_transaction,
                    )?;
                    ensure_kagemusha_v4_anchor_matches_topup_request(&anchor, &request)?;
                    let redeemed = load_kagemusha_v4_topup_drawdown(
                        request.authorization.operation_id,
                        state_transaction,
                    )?;
                    if redeemed > anchor.amount.atomic_units {
                        return Err(labeled_invariant(
                            "topup_drawdown_invalid",
                            "Kagemusha V4 top-up drawdown exceeds its finalized anchor",
                        )
                        .into());
                    }
                    return Ok(());
                }
                KagemushaV4ReplayStatus::Fresh(markers) => markers,
            };
            let hardware_assertion_commit =
                plan_kagemusha_online_hardware_assertion(authenticated_device)?;
            if request.current_note.network_id != *state_transaction.network_id() {
                return Err(labeled_invariant(
                    "wrong_network",
                    "Kagemusha V4 top-up NetworkId does not match this network",
                )
                .into());
            }
            let spec = state_transaction.numeric_spec_for(request.asset.definition())?;
            let live_scale = spec.scale().ok_or_else(|| {
                labeled_invariant(
                    "amount_scale_invalid",
                    "Kagemusha V4 requires an asset definition with a fixed numeric scale",
                )
            })?;
            if request.amount.scale != live_scale {
                return Err(labeled_invariant(
                    "amount_scale_mismatch",
                    "Kagemusha V4 top-up amount scale does not equal the live asset scale",
                )
                .into());
            }
            let release = resolve_kagemusha_v4_transaction_release(
                &request.artifact_binding,
                state_transaction.block_height(),
                state_transaction.network_id(),
                request.asset.definition(),
                live_scale,
                state_transaction,
            )?;
            if !release.issuance_active {
                return Err(labeled_invariant(
                    "recursive_release_withdrawn",
                    "Kagemusha V4 top-up is outside the authenticated issuance window",
                )
                .into());
            }
            let amount = request.amount.public_quantity();
            if amount.scale() != live_scale {
                return Err(labeled_invariant(
                    "amount_scale_mismatch",
                    "Kagemusha V4 top-up quantity encoding changed the authoritative scale",
                )
                .into());
            }
            assert_numeric_spec_with(amount.as_numeric(), spec)?;
            ensure_kagemusha_v4_policy_will_be_convertible(
                request.asset.definition(),
                state_transaction,
            )?;
            let mut zk_state = state_transaction
                .world
                .zk_assets
                .get(request.asset.definition())
                .cloned()
                .ok_or_else(|| {
                    labeled_invariant(
                        "verifier_key_invalid",
                        "Kagemusha V4 top-up requires configured confidential asset state",
                    )
                })?;
            zk_state
                .validate_tree_metadata()
                .map_err(|err| labeled_invariant("topup_anchor_invalid", err))?;
            let authoritative_initial_root = zk_state
                .current_root()
                .map_err(|err| labeled_invariant("topup_anchor_invalid", err))?;
            let authoritative_leaf_index =
                u32::try_from(zk_state.commitments.len()).map_err(|_| {
                    labeled_invariant(
                        "topup_tree_full",
                        "Kagemusha confidential tree position does not fit the protocol index",
                    )
                })?;
            if zk_state.commitments.len() >= zk_state.tree_profile.capacity() {
                return Err(labeled_invariant(
                    "topup_tree_full",
                    "Kagemusha confidential tree has no remaining top-up leaves",
                )
                .into());
            }
            ensure_kagemusha_v4_topup_note_is_fresh(
                &zk_state,
                request.current_note.note_commitment,
                request.current_note.spend_nullifier,
            )?;
            let mut commitments_after = zk_state.commitments.clone();
            commitments_after.push(request.current_note.note_commitment);
            let authoritative_finalized_root = zk_state
                .tree_profile
                .compute_root(&commitments_after)
                .map_err(|err| labeled_invariant("topup_anchor_invalid", err))?;
            let (shield_vk, _shield_record) = resolve_kagemusha_topup_shield_verifier(
                request.asset.definition(),
                &request.shield_evidence.proof,
                state_transaction,
            )?;
            ensure_kagemusha_v4_topup_shield_public_inputs(
                &request,
                authoritative_initial_root,
                authoritative_finalized_root,
                authoritative_leaf_index,
                state_transaction,
            )?;
            let anchor = finalized_kagemusha_v4_topup_anchor(&request, state_transaction)?;
            ensure_kagemusha_v4_topup_anchor_absent(request.operation_id, state_transaction)?;
            state_transaction
                .register_confidential_proof(request.shield_evidence.proof.proof.bytes.len())?;
            state_transaction.register_commitments(1)?;
            let report = crate::zk::verify_backend_with_timing_checked(
                request.shield_evidence.proof.backend.as_str(),
                &request.shield_evidence.proof.proof,
                Some(&shield_vk),
                &state_transaction.zk,
            );
            if !report.ok {
                return Err(labeled_invariant(
                    "invalid_proof",
                    "Kagemusha V4 top-up shield proof verification failed",
                )
                .into());
            }
            let policy_mode = crate::smartcontracts::isi::world::isi::apply_policy_if_due(
                state_transaction,
                request.asset.definition(),
            )?
            .mode();
            if policy_mode != ConfidentialPolicyMode::Convertible {
                return Err(labeled_invariant(
                    "confidential_policy_changed",
                    "Kagemusha V4 confidential policy changed after read-only top-up admission",
                )
                .into());
            }
            reserve_kagemusha_escrow(
                state_transaction,
                &request.authorization.authority,
                request.operation_id,
                &request.asset,
                &amount,
            )?;
            let finalized_root =
                crate::smartcontracts::isi::world::isi::push_confidential_commitment_for_asset(
                    &mut zk_state,
                    request.current_note.note_commitment,
                    state_transaction,
                )?;
            if finalized_root != authoritative_finalized_root {
                return Err(labeled_invariant(
                    "topup_anchor_mismatch",
                    "Kagemusha V4 shield root does not equal the authoritative finalized root",
                )
                .into());
            }
            let _frontier_update = zk_state
                .record_frontier_checkpoint(
                    state_transaction.block_height(),
                    state_transaction.zk.tree_frontier_checkpoint_interval,
                    state_transaction.zk.reorg_depth_bound,
                )
                .map_err(|err| {
                    labeled_invariant(
                        "confidential_tree",
                        format!("failed to checkpoint canonical confidential tree: {err}"),
                    )
                })?;
            state_transaction
                .world
                .zk_assets
                .remove(request.asset.definition().clone());
            state_transaction
                .world
                .zk_assets
                .insert(request.asset.definition().clone(), zk_state);
            persist_kagemusha_v4_topup_anchor(&anchor, state_transaction)?;
            commit_kagemusha_online_hardware_assertion(
                hardware_assertion_commit,
                state_transaction,
            )?;
            commit_kagemusha_v4_replay_markers(replay_markers, state_transaction);
            Ok(())
        }
    }
    impl Execute for RedeemKagemushaRecursiveV4 {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let request = self.request;
            request
                .validate_public_binding()
                .map_err(|err| labeled_invariant("invalid_recursive_redeem", err.to_string()))?;
            let payload_digest = request
                .unsigned_payload_digest()
                .map_err(|err| labeled_invariant("invalid_recursive_redeem", err.to_string()))?;
            request
                .validate_authorization_at(state_transaction.block_unix_timestamp_ms())
                .map_err(|err| labeled_invariant("invalid_authorization", err.to_string()))?;
            let statement = &request.bundle.statement;
            let release_policy = kagemusha_release_lifecycle::redemption_policy(
                &state_transaction.world,
                &statement.artifact_binding,
            )
            .map_err(|error| labeled_invariant("recursive_release_mismatch", error))?;
            let (authenticated_device, replay_status) =
                authenticate_kagemusha_v4_redeem_submission_before_replay(
                    &request.recipient,
                    &statement.asset,
                    authority,
                    &request.authorization,
                    &release_policy,
                    state_transaction,
                )?;
            let replay_markers = match replay_status {
                KagemushaV4ReplayStatus::Committed => {
                    ensure_kagemusha_v4_redemption_receipt_matches(
                        request.operation_id,
                        payload_digest,
                        state_transaction,
                    )?;
                    return Ok(());
                }
                KagemushaV4ReplayStatus::Fresh(markers) => markers,
            };
            let hardware_assertion_commit =
                plan_kagemusha_online_hardware_assertion(authenticated_device)?;
            let spec = state_transaction.numeric_spec_for(&statement.asset)?;
            let live_scale = spec.scale().ok_or_else(|| {
                labeled_invariant(
                    "amount_scale_invalid",
                    "Kagemusha V4 requires an asset definition with a fixed numeric scale",
                )
            })?;
            ensure_kagemusha_v4_redemption_live_context(
                &statement.network_id,
                &request.redemption.network_id,
                state_transaction.network_id(),
                request.amount.scale,
                statement.asset_scale,
                live_scale,
            )?;
            let amount = request.amount.public_quantity();
            if amount.scale() != live_scale {
                return Err(labeled_invariant(
                    "amount_scale_mismatch",
                    "Kagemusha V4 redemption quantity encoding changed the authoritative scale",
                )
                .into());
            }
            assert_numeric_spec_with(amount.as_numeric(), spec)?;
            let parent_release = resolve_kagemusha_v4_transaction_release(
                &statement.artifact_binding,
                request.block_height,
                &statement.network_id,
                &statement.asset,
                statement.asset_scale,
                state_transaction,
            )?;
            let change_release = request
                .offline_change
                .as_ref()
                .map(|change| {
                    resolve_kagemusha_v4_transaction_release(
                        &change.bundle.statement.artifact_binding,
                        request.block_height,
                        &change.bundle.statement.network_id,
                        &change.bundle.statement.asset,
                        change.bundle.statement.asset_scale,
                        state_transaction,
                    )
                })
                .transpose()?;
            if change_release
                .as_ref()
                .is_some_and(|release| !release.issuance_active)
            {
                return Err(labeled_invariant(
                    "recursive_release_withdrawn",
                    "Kagemusha V4 partial redemption with offline change is outside the issuance window",
                )
                .into());
            }
            let parent_operation =
                crate::zk::kagemusha_step_transition::KagemushaStepOperationVectorV4::from(
                    &request.bundle.operation,
                );
            parent_operation
                .to_fields()
                .map_err(|error| labeled_invariant("invalid_recursive_bundle", error))?;
            let expected_change_operation = request
                .offline_change
                .as_ref()
                .map(|change| {
                    let expected = crate::zk::kagemusha_step_transition::KagemushaStepOperationVectorV4::from_redemption_change_public_v4(
                        &request.redemption,
                        &change.bundle.statement,
                    )?;
                    let carried = crate::zk::kagemusha_step_transition::KagemushaStepOperationVectorV4::from(
                        &change.bundle.operation,
                    );
                    carried.to_fields()?;
                    if carried != expected {
                        return Err(
                            "Kagemusha V4 change operation does not match the submitted redemption"
                                .to_owned(),
                        );
                    }
                    Ok(expected)
                })
                .transpose()
                .map_err(|error| labeled_invariant("invalid_recursive_bundle", error))?;
            let zk_state = state_transaction
                .world
                .zk_assets
                .get(&statement.asset)
                .cloned()
                .ok_or_else(|| {
                    labeled_invariant(
                        "verifier_key_invalid",
                        "Kagemusha V4 redemption requires configured shielded asset state",
                    )
                })?;
            let provenance = validate_kagemusha_v4_finalized_topup_anchors(
                &statement.topup_anchor_refs,
                statement.current_note.amount.atomic_units,
                request.block_height,
                &zk_state,
                state_transaction,
            )?;
            let (redeem_vk, redeem_record) = resolve_kagemusha_unshield_verifier(
                &statement.asset,
                &request.redeem_proof,
                state_transaction,
            )?;
            ensure_kagemusha_v4_unshield_verifier_window(
                &redeem_record,
                request.block_height,
                state_transaction.block_height(),
            )?;
            ensure_kagemusha_v4_redeem_public_inputs(&request, state_transaction, &redeem_record)?;
            let commit_plan = plan_kagemusha_v4_redemption_commit(
                &request,
                &provenance.source_asset,
                payload_digest,
                replay_markers,
                state_transaction,
            )?;
            ensure_kagemusha_v4_policy_will_be_convertible(&statement.asset, state_transaction)?;
            state_transaction.register_confidential_proof(
                request
                    .bundle
                    .recursive_proof
                    .proof_envelope
                    .proof
                    .bytes
                    .len(),
            )?;
            state_transaction
                .register_confidential_proof(request.redeem_proof.proof.bytes.len())?;
            if let Some(change) = request.offline_change.as_ref() {
                state_transaction.register_confidential_proof(
                    change
                        .bundle
                        .recursive_proof
                        .proof_envelope
                        .proof
                        .bytes
                        .len(),
                )?;
                state_transaction.register_commitments(1)?;
            }
            state_transaction.register_nullifiers(1)?;
            verify_kagemusha_v4_recursive_bundle(
                &request.bundle,
                parent_release.cached.verifier(),
            )?;
            let redeem_report = crate::zk::verify_backend_with_timing_checked(
                request.redeem_proof.backend.as_str(),
                &request.redeem_proof.proof,
                Some(&redeem_vk),
                &state_transaction.zk,
            );
            if !redeem_report.ok {
                return Err(labeled_invariant(
                    "invalid_proof",
                    "Kagemusha V4 unshield proof verification failed",
                )
                .into());
            }
            if let (Some(change), Some(change_release), Some(expected_operation)) = (
                request.offline_change.as_ref(),
                change_release.as_ref(),
                expected_change_operation.as_ref(),
            ) {
                change_release
                    .cached
                    .verifier()
                    .verify_bundle_operation_v4(&change.bundle, expected_operation)
                    .map_err(|error| labeled_invariant("invalid_recursive_bundle", error))?;
            }
            let policy_mode = crate::smartcontracts::isi::world::isi::apply_policy_if_due(
                state_transaction,
                &statement.asset,
            )?
            .mode();
            if policy_mode != ConfidentialPolicyMode::Convertible {
                return Err(labeled_invariant(
                    "confidential_policy_changed",
                    "Kagemusha V4 confidential policy changed after read-only admission",
                )
                .into());
            }
            commit_kagemusha_online_hardware_assertion(
                hardware_assertion_commit,
                state_transaction,
            )?;
            commit_plan.commit(state_transaction)
        }
    }
    include!("offline/kagemusha_activation.rs");
    include!("offline/kagemusha_taira_canary.rs");
    include!("offline/isi_tests.rs");
}
