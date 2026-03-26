#![allow(clippy::items_after_test_module, clippy::redundant_pub_crate)]

use super::prelude::*;
#[cfg(feature = "zk-stark")]
use crate::zk_stark::verify_stark_fri_envelope;
use crate::{
    smartcontracts::{ValidQuery, isi::asset::isi::assert_numeric_spec_with},
    state::StateReadOnly,
};
mod aggregate_proof;
mod balance_proof;
use std::{
    collections::{BTreeSet, btree_map::Entry},
    str::FromStr,
    time::Duration,
};

use self::attestation::verify_platform_proof;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use iroha_config::parameters::actual;
use iroha_config::parameters::actual::OfflineProofMode;
use iroha_crypto::Hash;
#[cfg(test)]
use iroha_crypto::{Algorithm, KeyPair, Signature};
#[cfg(test)]
use iroha_data_model::offline::{OfflineAllowanceCommitment, OfflineWalletPolicy};
use iroha_data_model::{
    ChainId,
    asset::{AssetDefinitionId, AssetId},
    events::data::offline::{
        OfflineAllowanceReclaimed, OfflineTransferArchived, OfflineTransferEvent,
        OfflineTransferPruned, OfflineTransferSettled,
    },
    isi::{
        error::{InstructionExecutionError, MathError},
        offline::{
            CommitOfflineLineageOperation, LoadOfflineEscrowBalance,
            ReclaimExpiredOfflineAllowance, RedeemOfflineEscrowBalance, RegisterOfflineAllowance,
            RegisterOfflineLineage, RegisterOfflineVerdictRevocation,
            SubmitOfflineToOnlineTransfer,
        },
    },
    metadata::Metadata,
    name::Name,
    offline::{
        AGGREGATE_PROOF_METADATA_OFFLINE_AGGREGATE_BACKEND,
        AGGREGATE_PROOF_METADATA_OFFLINE_AGGREGATE_CIRCUIT_ID,
        AGGREGATE_PROOF_METADATA_OFFLINE_AGGREGATE_PUBLIC_INPUTS_B64,
        AGGREGATE_PROOF_METADATA_OFFLINE_AGGREGATE_RECURSION_DEPTH, AGGREGATE_PROOF_VERSION_V1,
        AGGREGATE_PROOF_VERSION_V2, ANDROID_PROVISIONED_APP_ID_KEY, AndroidHmsSafetyDetectMetadata,
        AndroidIntegrityMetadata, AndroidIntegrityPolicy, AndroidMarkerKeyMetadata,
        AndroidPlayIntegrityMetadata, AndroidProvisionedMetadata, AndroidProvisionedProof,
        HmsSafetyDetectEvaluation, OFFLINE_BUILD_CLAIM_MIN_BUILD_NUMBER_KEY,
        OFFLINE_LINEAGE_EPOCH_KEY, OFFLINE_LINEAGE_PREV_CERTIFICATE_ID_HEX_KEY,
        OFFLINE_LINEAGE_SCOPE_KEY, OFFLINE_REJECTION_REASON_PREFIX, OfflineAllowanceRecord,
        OfflineBalanceProof, OfflineBuildClaim, OfflineBuildClaimPlatform, OfflineCounterState,
        OfflineCounterSummary, OfflineLineageRecord, OfflinePlatformProof,
        OfflinePlatformTokenSnapshot, OfflineProofRequestError, OfflineSpendReceipt,
        OfflineToOnlineTransfer, OfflineTransferRecord, OfflineTransferRejectionPlatform,
        OfflineTransferRejectionReason, OfflineTransferStatus, OfflineVerdictRevocation,
        OfflineVerdictSnapshot, OfflineWalletCertificate, PROVISIONED_COUNTER_PREFIX,
        PlayIntegrityAppVerdict, PlayIntegrityDeviceVerdict, PlayIntegrityEnvironment,
        canonical_app_attest_key_id, canonical_receipts, chain_bound_receipt_hash,
        compute_receipts_root, ensure_single_counter_scope, marker_series_from_public_key,
        receipts_are_canonical,
    },
    query::{
        dsl::{CompoundPredicate, EvaluatePredicate},
        error::{FindError, QueryExecutionFail},
        offline::prelude::{
            FindOfflineAllowanceByCertificateId, FindOfflineAllowances,
            FindOfflineCounterSummaries, FindOfflineToOnlineTransferById,
            FindOfflineToOnlineTransfers, FindOfflineToOnlineTransfersByController,
            FindOfflineToOnlineTransfersByPolicy, FindOfflineToOnlineTransfersByReceiver,
            FindOfflineToOnlineTransfersByStatus, FindOfflineVerdictRevocations,
        },
    },
};
#[cfg(test)]
use iroha_primitives::json::Json as IrohaJson;
use iroha_primitives::numeric::Numeric;
use iroha_telemetry::metrics;
#[cfg(test)]
use norito::to_bytes;
use sha2::{Digest, Sha256};

pub use self::balance_proof::{build_balance_proof, compute_commitment};
use self::{
    aggregate_proof::{
        verify_fastpq_counter_proof, verify_fastpq_replay_proof, verify_fastpq_sum_proof,
    },
    balance_proof::{VerificationInputs, verify_balance_proof},
};

const IOS_TEAM_ID_KEY: &str = "ios.app_attest.team_id";
const IOS_BUNDLE_ID_KEY: &str = "ios.app_attest.bundle_id";
const IOS_ENVIRONMENT_KEY: &str = "ios.app_attest.environment";
const CAN_MANAGE_OFFLINE_ESCROW_PERMISSION: &str = "CanManageOfflineEscrow";
#[cfg(test)]
const ANDROID_PACKAGE_NAMES_KEY: &str = "android.attestation.package_names";
#[cfg(test)]
const ANDROID_SIGNATURE_DIGESTS_KEY: &str = "android.attestation.signing_digests_sha256";
#[cfg(test)]
const ANDROID_REQUIRE_STRONGBOX_KEY: &str = "android.attestation.require_strongbox";
#[cfg(test)]
const ANDROID_REQUIRE_ROLLBACK_KEY: &str = "android.attestation.require_rollback_resistance";
#[cfg(test)]
const ANDROID_INTEGRITY_POLICY_KEY: &str = AndroidIntegrityPolicy::METADATA_KEY;
#[cfg(test)]
const ANDROID_PLAY_PROJECT_KEY: &str = "android.play_integrity.cloud_project_number";
#[cfg(test)]
const ANDROID_PLAY_ENVIRONMENT_KEY: &str = "android.play_integrity.environment";
#[cfg(test)]
const ANDROID_PLAY_PACKAGE_NAMES_KEY: &str = "android.play_integrity.package_names";
#[cfg(test)]
const ANDROID_PLAY_DIGESTS_KEY: &str = "android.play_integrity.signing_digests_sha256";
#[cfg(test)]
const ANDROID_PLAY_APP_VERDICTS_KEY: &str = "android.play_integrity.allowed_app_verdicts";
#[cfg(test)]
const ANDROID_PLAY_DEVICE_VERDICTS_KEY: &str = "android.play_integrity.allowed_device_verdicts";
#[cfg(test)]
const ANDROID_PLAY_MAX_AGE_KEY: &str = "android.play_integrity.max_token_age_ms";
#[cfg(test)]
const ANDROID_HMS_APP_ID_KEY: &str = "android.hms_safety_detect.app_id";
#[cfg(test)]
const ANDROID_HMS_PACKAGE_NAMES_KEY: &str = "android.hms_safety_detect.package_names";
#[cfg(test)]
const ANDROID_HMS_DIGESTS_KEY: &str = "android.hms_safety_detect.signing_digests_sha256";
#[cfg(test)]
const ANDROID_HMS_EVALUATIONS_KEY: &str = "android.hms_safety_detect.required_evaluations";
#[cfg(test)]
const ANDROID_HMS_MAX_AGE_KEY: &str = "android.hms_safety_detect.max_token_age_ms";
const ANDROID_PROVISIONED_DEVICE_ID_KEY: &str = "android.provisioned.device_id";
#[cfg(test)]
const ANDROID_PROVISIONED_INSPECTOR_KEY: &str = "android.provisioned.inspector_public_key";
#[cfg(test)]
const ANDROID_PROVISIONED_MANIFEST_SCHEMA_KEY: &str = "android.provisioned.manifest_schema";
#[cfg(test)]
const ANDROID_PROVISIONED_MANIFEST_VERSION_KEY: &str = "android.provisioned.manifest_version";
#[cfg(test)]
const ANDROID_PROVISIONED_MAX_AGE_KEY: &str = "android.provisioned.max_manifest_age_ms";
#[cfg(test)]
const ANDROID_PROVISIONED_MANIFEST_DIGEST_KEY: &str = "android.provisioned.manifest_digest";

const KM_TAG_PURPOSE: u32 = 1;
const KM_TAG_ALGORITHM: u32 = 2;
const KM_TAG_KEY_SIZE: u32 = 3;
const KM_TAG_EC_CURVE: u32 = 10;
const KM_TAG_ROLLBACK_RESISTANCE: u32 = 303;
const KM_TAG_ALL_APPLICATIONS: u32 = 600;
const KM_TAG_ORIGIN: u32 = 702;
const KM_TAG_ROOT_OF_TRUST: u32 = 704;
const KM_TAG_ATTESTATION_APPLICATION_ID: u32 = 709;
const KM_SECURITY_LEVEL_SOFTWARE: u32 = 0;
const KM_SECURITY_LEVEL_TRUSTED_ENVIRONMENT: u32 = 1;
const KM_SECURITY_LEVEL_STRONG_BOX: u32 = 2;
const KM_PURPOSE_SIGN: u32 = 2;
const KM_ALGORITHM_EC: u32 = 3;
const KM_EC_CURVE_P256: u32 = 1;
const KM_ORIGIN_GENERATED: u32 = 0;
const KM_VERIFIED_BOOT_STATE_VERIFIED: u32 = 0;

#[cfg(feature = "zk-stark")]
fn verify_recursive_stark_envelope(proof: &[u8]) -> bool {
    verify_stark_fri_envelope(proof)
}

#[cfg(not(feature = "zk-stark"))]
fn verify_recursive_stark_envelope(_proof: &[u8]) -> bool {
    false
}

fn labeled_invariant(label: &str, message: impl Into<String>) -> InstructionExecutionError {
    let message = message.into();
    let boxed: Box<str> = format!("{OFFLINE_REJECTION_REASON_PREFIX}{label}:{message}").into();
    InstructionExecutionError::InvariantViolation(boxed)
}

fn rejection_code(reason: OfflineTransferRejectionReason) -> &'static str {
    match reason {
        OfflineTransferRejectionReason::CounterViolation => "counter_conflict",
        OfflineTransferRejectionReason::AllowanceDepleted => "allowance_exceeded",
        _ => reason.as_label(),
    }
}

fn rejection_error(
    reason: OfflineTransferRejectionReason,
    platform: OfflineTransferRejectionPlatform,
    message: impl Into<String>,
) -> InstructionExecutionError {
    metrics::global_or_default().record_offline_transfer_rejection(platform, reason);
    labeled_invariant(rejection_code(reason), message)
}

fn map_platform_err<T>(
    result: Result<T, InstructionExecutionError>,
    reason: OfflineTransferRejectionReason,
    platform: OfflineTransferRejectionPlatform,
) -> Result<T, InstructionExecutionError> {
    result.map_err(|err| rejection_error(reason, platform, err.to_string()))
}

fn map_counter_scope_error(err: OfflineProofRequestError) -> InstructionExecutionError {
    match err {
        OfflineProofRequestError::MissingReceipts => rejection_error(
            OfflineTransferRejectionReason::EmptyBundle,
            OfflineTransferRejectionPlatform::General,
            "offline bundle must include at least one receipt",
        ),
        OfflineProofRequestError::MixedCounterScopes => rejection_error(
            OfflineTransferRejectionReason::MixedCounterScopes,
            OfflineTransferRejectionPlatform::General,
            "offline bundle mixes counter scopes",
        ),
        OfflineProofRequestError::InvalidCounterScope { platform, reason } => rejection_error(
            OfflineTransferRejectionReason::PlatformMetadataInvalid,
            platform,
            reason,
        ),
        other => InstructionExecutionError::InvariantViolation(
            format!("unexpected counter scope error: {other}").into(),
        ),
    }
}

fn ensure_certificate_policy(
    certificate: &OfflineWalletCertificate,
    expected_scale: u32,
) -> Result<(), InstructionExecutionError> {
    if certificate.issued_at_ms >= certificate.expires_at_ms {
        return Err(InstructionExecutionError::InvariantViolation(
            "certificate expiry must be greater than issuance timestamp".into(),
        ));
    }
    if certificate.policy.expires_at_ms < certificate.expires_at_ms {
        return Err(InstructionExecutionError::InvariantViolation(
            "policy expiry must cover the certificate lifetime".into(),
        ));
    }
    if certificate.policy.max_balance <= Numeric::zero() {
        return Err(InstructionExecutionError::InvariantViolation(
            "policy max balance must be positive".into(),
        ));
    }
    if certificate.policy.max_tx_value <= Numeric::zero() {
        return Err(InstructionExecutionError::InvariantViolation(
            "policy max transaction value must be positive".into(),
        ));
    }
    ensure_expected_scale(
        &certificate.policy.max_balance,
        expected_scale,
        "policy max balance",
    )?;
    ensure_expected_scale(
        &certificate.policy.max_tx_value,
        expected_scale,
        "policy max transaction value",
    )?;
    ensure_expected_scale(
        &certificate.allowance.amount,
        expected_scale,
        "allowance amount",
    )?;
    if certificate.policy.max_tx_value > certificate.policy.max_balance {
        return Err(InstructionExecutionError::InvariantViolation(
            "policy max transaction value cannot exceed max balance".into(),
        ));
    }
    if certificate.policy.max_balance < certificate.allowance.amount {
        return Err(InstructionExecutionError::InvariantViolation(
            "allowance amount exceeds policy max balance".into(),
        ));
    }
    if let Some(refresh_at_ms) = certificate.refresh_at_ms {
        if refresh_at_ms <= certificate.issued_at_ms {
            return Err(InstructionExecutionError::InvariantViolation(
                "refresh_at_ms must be greater than issued_at_ms".into(),
            ));
        }
        let expiry_bound = certificate
            .policy
            .expires_at_ms
            .min(certificate.expires_at_ms);
        if refresh_at_ms > expiry_bound {
            return Err(InstructionExecutionError::InvariantViolation(
                "refresh_at_ms must not exceed the certificate/policy expiry".into(),
            ));
        }
    }
    Ok(())
}

fn ensure_expected_scale(
    value: &Numeric,
    expected_scale: u32,
    label: &'static str,
) -> Result<(), InstructionExecutionError> {
    if value.scale() != expected_scale {
        return Err(InstructionExecutionError::InvariantViolation(
            format!("{label} must use scale {expected_scale}").into(),
        ));
    }
    Ok(())
}

fn ensure_operator_signature(
    certificate: &OfflineWalletCertificate,
) -> Result<(), InstructionExecutionError> {
    let payload = certificate.operator_signing_bytes().map_err(|err| {
        InstructionExecutionError::InvariantViolation(
            format!("failed to encode certificate payload: {err}").into(),
        )
    })?;
    let operator_key = certificate.operator.try_signatory().ok_or_else(|| {
        InstructionExecutionError::InvariantViolation(
            "operator account must be single-signature".into(),
        )
    })?;
    certificate
        .operator_signature
        .verify(operator_key, &payload)
        .map_err(|_| {
            InstructionExecutionError::InvariantViolation(
                "operator signature does not match operator account".into(),
            )
        })
}

fn ensure_allowance_owner_matches_controller(
    certificate: &OfflineWalletCertificate,
) -> Result<(), InstructionExecutionError> {
    if certificate.allowance.asset.account() != &certificate.controller {
        return Err(labeled_invariant(
            "allowance_owner_mismatch",
            "allowance asset account must match certificate controller",
        ));
    }
    Ok(())
}

#[derive(Clone)]
struct CertificateLineageMetadata {
    scope: String,
    epoch: u64,
    prev_certificate_id: Option<Hash>,
    min_build_number: u64,
}

fn parse_certificate_lineage_metadata(
    certificate: &OfflineWalletCertificate,
) -> Result<CertificateLineageMetadata, InstructionExecutionError> {
    let metadata = &certificate.metadata;

    let scope = metadata_string_field(metadata, OFFLINE_LINEAGE_SCOPE_KEY, "lineage scope")?;
    if scope.trim().is_empty() {
        return Err(rejection_error(
            OfflineTransferRejectionReason::LineageInvalid,
            OfflineTransferRejectionPlatform::General,
            "offline.lineage.scope must not be empty",
        ));
    }
    let scope = scope.trim().to_string();

    let epoch = metadata_u64_field(metadata, OFFLINE_LINEAGE_EPOCH_KEY, "lineage epoch")?;
    if epoch == 0 {
        return Err(rejection_error(
            OfflineTransferRejectionReason::LineageInvalid,
            OfflineTransferRejectionPlatform::General,
            "offline.lineage.epoch must be greater than zero",
        ));
    }

    let prev_certificate_id_hex =
        metadata_optional_string_field(metadata, OFFLINE_LINEAGE_PREV_CERTIFICATE_ID_HEX_KEY)?;
    let prev_certificate_id = prev_certificate_id_hex
        .as_deref()
        .map(parse_certificate_id_hex)
        .transpose()?;

    if epoch == 1 && prev_certificate_id.is_some() {
        return Err(rejection_error(
            OfflineTransferRejectionReason::LineageInvalid,
            OfflineTransferRejectionPlatform::General,
            "offline.lineage.prev_certificate_id_hex must be absent when epoch is 1",
        ));
    }
    if epoch > 1 && prev_certificate_id.is_none() {
        return Err(rejection_error(
            OfflineTransferRejectionReason::LineageInvalid,
            OfflineTransferRejectionPlatform::General,
            "offline.lineage.prev_certificate_id_hex is required when epoch is greater than 1",
        ));
    }

    let min_build_number = metadata_u64_field(
        metadata,
        OFFLINE_BUILD_CLAIM_MIN_BUILD_NUMBER_KEY,
        "minimum build number",
    )?;

    Ok(CertificateLineageMetadata {
        scope,
        epoch,
        prev_certificate_id,
        min_build_number,
    })
}

fn validate_certificate_lineage(
    state_transaction: &StateTransaction<'_, '_>,
    certificate: &OfflineWalletCertificate,
    lineage: &CertificateLineageMetadata,
) -> Result<(), InstructionExecutionError> {
    let mut head: Option<(Hash, u64)> = None;
    for (existing_id, record) in state_transaction.world.offline_allowances.iter() {
        if record.certificate.controller != certificate.controller
            || record.certificate.allowance.asset.definition()
                != certificate.allowance.asset.definition()
        {
            continue;
        }
        let existing_lineage = parse_certificate_lineage_metadata(&record.certificate)?;
        if existing_lineage.scope != lineage.scope {
            continue;
        }
        if existing_lineage.epoch == lineage.epoch {
            return Err(rejection_error(
                OfflineTransferRejectionReason::LineageInvalid,
                OfflineTransferRejectionPlatform::General,
                format!(
                    "duplicate lineage epoch {} for scope `{}`",
                    lineage.epoch, lineage.scope
                ),
            ));
        }
        match head {
            Some((head_id, head_epoch))
                if existing_lineage.epoch < head_epoch
                    || (existing_lineage.epoch == head_epoch && *existing_id >= head_id) => {}
            _ => {
                head = Some((*existing_id, existing_lineage.epoch));
            }
        }
    }

    match head {
        None => {
            if lineage.epoch != 1 {
                return Err(rejection_error(
                    OfflineTransferRejectionReason::LineageInvalid,
                    OfflineTransferRejectionPlatform::General,
                    format!(
                        "first certificate for scope `{}` must start with epoch 1",
                        lineage.scope
                    ),
                ));
            }
        }
        Some((head_id, head_epoch)) => {
            let expected_epoch = head_epoch.checked_add(1).ok_or_else(|| {
                rejection_error(
                    OfflineTransferRejectionReason::LineageInvalid,
                    OfflineTransferRejectionPlatform::General,
                    "lineage epoch overflow",
                )
            })?;
            if lineage.epoch != expected_epoch {
                return Err(rejection_error(
                    OfflineTransferRejectionReason::LineageInvalid,
                    OfflineTransferRejectionPlatform::General,
                    format!(
                        "lineage epoch mismatch for scope `{}`: expected {}, got {}",
                        lineage.scope, expected_epoch, lineage.epoch
                    ),
                ));
            }
            if lineage.prev_certificate_id != Some(head_id) {
                return Err(rejection_error(
                    OfflineTransferRejectionReason::LineageInvalid,
                    OfflineTransferRejectionPlatform::General,
                    format!(
                        "lineage previous certificate mismatch for scope `{}`",
                        lineage.scope
                    ),
                ));
            }
        }
    }

    Ok(())
}

fn metadata_value<'a>(
    metadata: &'a Metadata,
    key: &str,
) -> Result<Option<&'a iroha_primitives::json::Json>, InstructionExecutionError> {
    let name = Name::from_str(key).map_err(|err| {
        rejection_error(
            OfflineTransferRejectionReason::LineageInvalid,
            OfflineTransferRejectionPlatform::General,
            format!("invalid metadata key `{key}`: {err}"),
        )
    })?;
    Ok(metadata.get(&name))
}

fn metadata_optional_string_field(
    metadata: &Metadata,
    key: &str,
) -> Result<Option<String>, InstructionExecutionError> {
    metadata_value(metadata, key)?
        .map(|value| {
            value.try_into_any::<String>().map_err(|err| {
                rejection_error(
                    OfflineTransferRejectionReason::LineageInvalid,
                    OfflineTransferRejectionPlatform::General,
                    format!("metadata entry `{key}` is not a string: {err}"),
                )
            })
        })
        .transpose()
}

fn metadata_string_field(
    metadata: &Metadata,
    key: &str,
    label: &str,
) -> Result<String, InstructionExecutionError> {
    metadata_optional_string_field(metadata, key)?.ok_or_else(|| {
        rejection_error(
            OfflineTransferRejectionReason::LineageInvalid,
            OfflineTransferRejectionPlatform::General,
            format!("missing {label} metadata `{key}`"),
        )
    })
}

fn metadata_u64_field(
    metadata: &Metadata,
    key: &str,
    label: &str,
) -> Result<u64, InstructionExecutionError> {
    let value = metadata_value(metadata, key)?.ok_or_else(|| {
        rejection_error(
            OfflineTransferRejectionReason::LineageInvalid,
            OfflineTransferRejectionPlatform::General,
            format!("missing {label} metadata `{key}`"),
        )
    })?;
    if let Ok(parsed) = value.try_into_any::<u64>() {
        return Ok(parsed);
    }
    let text = value.try_into_any::<String>().map_err(|err| {
        rejection_error(
            OfflineTransferRejectionReason::LineageInvalid,
            OfflineTransferRejectionPlatform::General,
            format!("metadata entry `{key}` is not a u64/string: {err}"),
        )
    })?;
    text.parse::<u64>().map_err(|err| {
        rejection_error(
            OfflineTransferRejectionReason::LineageInvalid,
            OfflineTransferRejectionPlatform::General,
            format!("metadata entry `{key}` could not be parsed as u64: {err}"),
        )
    })
}

fn parse_certificate_id_hex(value: &str) -> Result<Hash, InstructionExecutionError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(rejection_error(
            OfflineTransferRejectionReason::LineageInvalid,
            OfflineTransferRejectionPlatform::General,
            "offline.lineage.prev_certificate_id_hex must not be empty",
        ));
    }
    let bytes = hex::decode(trimmed).map_err(|err| {
        rejection_error(
            OfflineTransferRejectionReason::LineageInvalid,
            OfflineTransferRejectionPlatform::General,
            format!("offline.lineage.prev_certificate_id_hex is not valid hex: {err}"),
        )
    })?;
    let digest: [u8; Hash::LENGTH] = bytes.try_into().map_err(|_| {
        rejection_error(
            OfflineTransferRejectionReason::LineageInvalid,
            OfflineTransferRejectionPlatform::General,
            format!(
                "offline.lineage.prev_certificate_id_hex must be {} bytes",
                Hash::LENGTH
            ),
        )
    })?;
    Ok(Hash::prehashed(digest))
}

fn resolve_offline_escrow_account(
    state_transaction: &mut StateTransaction<'_, '_>,
    definition: &AssetDefinitionId,
) -> Result<Option<AccountId>, InstructionExecutionError> {
    if let Some(account) = state_transaction
        .settlement
        .offline
        .escrow_accounts
        .get(definition)
    {
        return Ok(Some(account.clone()));
    }
    let asset_definition = state_transaction
        .world
        .asset_definition(definition)
        .map_err(Error::from)?;
    if crate::smartcontracts::isi::domain::isi::asset_definition_offline_enabled(
        asset_definition.metadata(),
    )? {
        crate::smartcontracts::isi::domain::isi::ensure_offline_escrow_account(
            &asset_definition,
            asset_definition.owned_by(),
            state_transaction,
        )?;
        let derived = crate::smartcontracts::isi::domain::isi::offline_escrow_account_id(
            state_transaction.chain_id(),
            definition,
        );
        return Ok(Some(derived));
    }
    if state_transaction.settlement.offline.escrow_required {
        return Err(labeled_invariant(
            "escrow_missing",
            format!("offline escrow account not configured for asset definition `{definition}`"),
        ));
    }
    Ok(None)
}

pub(crate) fn is_offline_escrow_source_asset(
    state_transaction: &StateTransaction<'_, '_>,
    source_id: &AssetId,
) -> Result<bool, Error> {
    if let Some(account) = state_transaction
        .settlement
        .offline
        .escrow_accounts
        .get(source_id.definition())
    {
        return Ok(account == source_id.account());
    }

    let asset_definition = state_transaction
        .world
        .asset_definition(source_id.definition())
        .map_err(Error::from)?;

    if !crate::smartcontracts::isi::domain::isi::asset_definition_offline_enabled(
        asset_definition.metadata(),
    )? {
        return Ok(false);
    }

    let derived = crate::smartcontracts::isi::domain::isi::offline_escrow_account_id(
        state_transaction.chain_id(),
        source_id.definition(),
    );
    Ok(&derived == source_id.account())
}

fn reserve_offline_allowance(
    state_transaction: &mut StateTransaction<'_, '_>,
    asset: &AssetId,
    amount: &Numeric,
) -> Result<(), Error> {
    let escrow_account = resolve_offline_escrow_account(state_transaction, asset.definition())?;
    let escrow_account = escrow_account.ok_or_else(|| {
        labeled_invariant(
            "escrow_missing",
            format!(
                "offline escrow account not configured for asset definition `{}`",
                asset.definition(),
            ),
        )
    })?;
    // Zero-amount allowances should not require a pre-existing asset entry.
    if amount.is_zero() {
        return Ok(());
    }
    let escrow_asset = AssetId::new(asset.definition().clone(), escrow_account);
    state_transaction
        .world
        .withdraw_numeric_asset(asset, amount)?;
    state_transaction
        .world
        .deposit_numeric_asset(&escrow_asset, amount)
}

fn is_allowance_reserve_shortfall(err: &Error) -> bool {
    matches!(
        err,
        Error::Find(FindError::Asset(_)) | Error::Math(MathError::NotEnoughQuantity)
    )
}

fn reserve_or_reallocate_amount(
    state_transaction: &mut StateTransaction<'_, '_>,
    controller: &AccountId,
    asset: &AssetId,
    amount: &Numeric,
    excluded_certificate_id: Option<Hash>,
) -> Result<(), Error> {
    match reserve_offline_allowance(state_transaction, asset, amount) {
        Ok(()) => return Ok(()),
        Err(err) if !is_allowance_reserve_shortfall(&err) => return Err(err),
        Err(_) => {}
    }

    let mut candidates = state_transaction
        .world
        .offline_allowances
        .iter()
        .filter(|(_, record)| {
            record.certificate.controller == *controller
                && record.certificate.allowance.asset == *asset
                && !record.remaining_amount.is_zero()
        })
        .filter(|(certificate_id, _)| Some(**certificate_id) != excluded_certificate_id)
        .map(|(certificate_id, record)| (*certificate_id, record.registered_at_ms))
        .collect::<Vec<_>>();
    candidates.sort_by(
        |(left_id, left_registered_at), (right_id, right_registered_at)| {
            right_registered_at
                .cmp(left_registered_at)
                .then_with(|| left_id.cmp(right_id))
        },
    );

    let mut shortfall = amount.clone();
    let mut reallocations = Vec::new();
    for (certificate_id, _) in candidates {
        if shortfall.is_zero() {
            break;
        }
        let record = state_transaction
            .world
            .offline_allowances
            .get(&certificate_id)
            .expect("candidate certificate id must remain present");
        let take = if record.remaining_amount > shortfall {
            shortfall.clone()
        } else {
            record.remaining_amount.clone()
        };
        if take.is_zero() {
            continue;
        }
        shortfall = shortfall
            .checked_sub(take.clone())
            .ok_or(MathError::NotEnoughQuantity)?;
        reallocations.push((certificate_id, take));
    }

    if !shortfall.is_zero() {
        reserve_offline_allowance(state_transaction, asset, &shortfall)?;
    }

    for (certificate_id, take) in reallocations {
        let record = state_transaction
            .world
            .offline_allowances
            .get_mut(&certificate_id)
            .expect("candidate certificate id must remain present");
        record.remaining_amount = record
            .remaining_amount
            .clone()
            .checked_sub(take)
            .ok_or(MathError::NotEnoughQuantity)?;
    }

    Ok(())
}

fn refund_allowance_from_escrow(
    state_transaction: &mut StateTransaction<'_, '_>,
    asset: &AssetId,
    controller: &AccountId,
    amount: &Numeric,
) -> Result<(), Error> {
    if amount.is_zero() {
        return Ok(());
    }

    let definition_id = asset.definition().clone();
    let spec = state_transaction.numeric_spec_for(&definition_id)?;
    assert_numeric_spec_with(amount, spec)?;
    let controller_asset = AssetId::new(definition_id.clone(), controller.clone());
    let current_balance = state_transaction
        .world
        .assets
        .get(&controller_asset)
        .map(|asset| asset.as_ref().clone())
        .unwrap_or_else(Numeric::zero);
    current_balance
        .checked_add(amount.clone())
        .ok_or(MathError::Overflow)?;

    let escrow_account = resolve_offline_escrow_account(state_transaction, &definition_id)?
        .ok_or_else(|| {
            labeled_invariant(
                "escrow_missing",
                format!(
                    "offline escrow account not configured for asset definition `{definition_id}`",
                ),
            )
        })?;
    let escrow_asset = AssetId::new(definition_id, escrow_account);
    state_transaction
        .world
        .withdraw_numeric_asset(&escrow_asset, amount)?;
    if let Err(err) = state_transaction
        .world
        .deposit_numeric_asset(&controller_asset, amount)
    {
        state_transaction
            .world
            .deposit_numeric_asset(&escrow_asset, amount)
            .expect("escrow refund must succeed after failed controller credit");
        return Err(err);
    }

    Ok(())
}

// Certificates linked through `prev_certificate_id` form a single active lineage head. When a
// successor is registered, any predecessor reserve is rolled into the new head up to the new
// certificate amount, and excess predecessor reserve is returned on-chain instead of remaining
// as a stale allowance that can no longer produce valid platform proofs.
fn reserve_or_reallocate_allowance(
    state_transaction: &mut StateTransaction<'_, '_>,
    certificate: &OfflineWalletCertificate,
    lineage: &CertificateLineageMetadata,
) -> Result<(), Error> {
    if let Some(previous_certificate_id) = lineage.prev_certificate_id {
        let previous_record = state_transaction
            .world
            .offline_allowances
            .get(&previous_certificate_id)
            .cloned()
            .ok_or_else(|| {
                rejection_error(
                    OfflineTransferRejectionReason::LineageInvalid,
                    OfflineTransferRejectionPlatform::General,
                    "previous lineage certificate is not registered",
                )
            })?;
        if previous_record.certificate.controller != certificate.controller
            || previous_record.certificate.allowance.asset != certificate.allowance.asset
        {
            return Err(rejection_error(
                OfflineTransferRejectionReason::LineageInvalid,
                OfflineTransferRejectionPlatform::General,
                "renewed certificate must keep the predecessor controller and asset",
            ));
        }

        let reused_amount = if previous_record.remaining_amount > certificate.allowance.amount {
            certificate.allowance.amount.clone()
        } else {
            previous_record.remaining_amount.clone()
        };
        let shortfall = certificate
            .allowance
            .amount
            .clone()
            .checked_sub(reused_amount.clone())
            .ok_or(MathError::NotEnoughQuantity)?;
        reserve_or_reallocate_amount(
            state_transaction,
            &certificate.controller,
            &certificate.allowance.asset,
            &shortfall,
            Some(previous_certificate_id),
        )?;

        let refund_amount = previous_record
            .remaining_amount
            .checked_sub(reused_amount)
            .ok_or(MathError::NotEnoughQuantity)?;
        refund_allowance_from_escrow(
            state_transaction,
            &certificate.allowance.asset,
            &certificate.controller,
            &refund_amount,
        )?;

        let previous_record = state_transaction
            .world
            .offline_allowances
            .get_mut(&previous_certificate_id)
            .expect("validated predecessor certificate must remain present");
        previous_record.remaining_amount = Numeric::zero();
        return Ok(());
    }

    reserve_or_reallocate_amount(
        state_transaction,
        &certificate.controller,
        &certificate.allowance.asset,
        &certificate.allowance.amount,
        None,
    )
}

fn ensure_uniform_asset(receipts: &[OfflineSpendReceipt]) -> Result<AssetId, Error> {
    let first = receipts.first().ok_or_else(|| {
        rejection_error(
            OfflineTransferRejectionReason::EmptyBundle,
            OfflineTransferRejectionPlatform::General,
            "empty receipt set",
        )
    })?;
    let definition = first.asset.definition().clone();
    for receipt in receipts.iter().skip(1) {
        if receipt.asset.definition() != &definition {
            return Err(rejection_error(
                OfflineTransferRejectionReason::NonUniformAsset,
                OfflineTransferRejectionPlatform::General,
                "receipts reference multiple asset definitions",
            ));
        }
    }
    Ok(AssetId::new(definition, first.asset.account().clone()))
}

fn aggregate_amount(
    receipts: &[OfflineSpendReceipt],
    expected_scale: u32,
) -> Result<Numeric, Error> {
    let mut acc = Numeric::zero();
    for receipt in receipts {
        if receipt.amount.scale() != expected_scale {
            return Err(rejection_error(
                OfflineTransferRejectionReason::InvalidReceiptAmount,
                OfflineTransferRejectionPlatform::General,
                format!("receipt amount must use scale {expected_scale}"),
            ));
        }
        if receipt.amount <= Numeric::zero() {
            return Err(rejection_error(
                OfflineTransferRejectionReason::InvalidReceiptAmount,
                OfflineTransferRejectionPlatform::General,
                "receipt amount must be positive",
            ));
        }
        acc = acc
            .checked_add(receipt.amount.clone())
            .ok_or(MathError::Overflow)?;
    }
    if acc.is_zero() {
        Err(InstructionExecutionError::InvariantViolation(
            "aggregate receipt amount must be positive".into(),
        ))
    } else {
        Ok(acc)
    }
}

#[derive(Clone)]
struct ReceiptChallenge {
    iroha_hash: Hash,
    iroha_bytes: [u8; Hash::LENGTH],
    client_data_hash: [u8; 32],
}

fn derive_receipt_challenge(
    receipt: &OfflineSpendReceipt,
    chain_id: &ChainId,
) -> Result<ReceiptChallenge, InstructionExecutionError> {
    if chain_id.as_str().trim().is_empty() {
        return Err(InstructionExecutionError::InvariantViolation(
            "offline receipt chain_id must not be empty".into(),
        ));
    }
    let bytes = receipt.challenge_bytes().map_err(|err| {
        InstructionExecutionError::InvariantViolation(
            format!("failed to encode receipt challenge: {err}").into(),
        )
    })?;
    let iroha_hash = chain_bound_receipt_hash(chain_id, &bytes);
    let mut iroha_bytes = [0u8; Hash::LENGTH];
    iroha_bytes.copy_from_slice(iroha_hash.as_ref());
    let client_data_hash = Sha256::digest(iroha_hash.as_ref());

    Ok(ReceiptChallenge {
        iroha_hash,
        iroha_bytes,
        client_data_hash: client_data_hash.into(),
    })
}

#[derive(Clone)]
struct IosMetadata {
    team_id: String,
    bundle_id: String,
    environment: AppleEnvironment,
}

#[derive(Clone, Copy, Debug)]
enum AppleEnvironment {
    Production,
    Development,
}

#[derive(Clone)]
struct MarkerBindingMetadata {
    package_names: BTreeSet<String>,
    signing_digests: BTreeSet<Vec<u8>>,
    require_strongbox: bool,
    require_rollback_resistance: bool,
}

impl MarkerBindingMetadata {
    fn from_marker(meta: &AndroidMarkerKeyMetadata) -> Self {
        Self {
            package_names: meta.package_names.clone(),
            signing_digests: meta.signing_digests_sha256.clone(),
            require_strongbox: meta.require_strongbox,
            require_rollback_resistance: meta.require_rollback_resistance,
        }
    }

    fn from_play(meta: &AndroidPlayIntegrityMetadata) -> Self {
        Self {
            package_names: meta.package_names.clone(),
            signing_digests: meta.signing_digests_sha256.clone(),
            require_strongbox: false,
            require_rollback_resistance: false,
        }
    }

    fn from_hms(meta: &AndroidHmsSafetyDetectMetadata) -> Self {
        Self {
            package_names: meta.package_names.clone(),
            signing_digests: meta.signing_digests_sha256.clone(),
            require_strongbox: false,
            require_rollback_resistance: false,
        }
    }
}

fn extract_ios_metadata(metadata: &Metadata) -> Result<IosMetadata, InstructionExecutionError> {
    let platform = OfflineTransferRejectionPlatform::Apple;
    let mut team_id = metadata_string(metadata, IOS_TEAM_ID_KEY, platform)?;
    team_id.make_ascii_uppercase();
    let bundle_id = metadata_string(metadata, IOS_BUNDLE_ID_KEY, platform)?;
    let environment_value = metadata_string(metadata, IOS_ENVIRONMENT_KEY, platform)?;
    let environment = match environment_value.as_str() {
        "production" => AppleEnvironment::Production,
        "development" => AppleEnvironment::Development,
        other => {
            return Err(rejection_error(
                OfflineTransferRejectionReason::PlatformMetadataInvalid,
                platform,
                format!("unrecognised ios.app_attest.environment value `{other}`"),
            ));
        }
    };
    Ok(IosMetadata {
        team_id,
        bundle_id,
        environment,
    })
}

fn android_integrity_metadata(
    metadata: &Metadata,
) -> Result<AndroidIntegrityMetadata, InstructionExecutionError> {
    let platform = OfflineTransferRejectionPlatform::Android;
    let parsed = AndroidIntegrityMetadata::from_metadata(metadata).map_err(|err| {
        rejection_error(
            OfflineTransferRejectionReason::PlatformMetadataInvalid,
            platform,
            format!("android metadata invalid: {err}"),
        )
    })?;
    parsed.ok_or_else(|| {
        rejection_error(
            OfflineTransferRejectionReason::PlatformMetadataInvalid,
            platform,
            "android integrity metadata is missing",
        )
    })
}

fn metadata_string(
    metadata: &Metadata,
    key: &str,
    platform: OfflineTransferRejectionPlatform,
) -> Result<String, InstructionExecutionError> {
    let name = Name::from_str(key).map_err(|err| {
        rejection_error(
            OfflineTransferRejectionReason::PlatformMetadataInvalid,
            platform,
            format!("invalid metadata key `{key}`: {err}"),
        )
    })?;
    let value = metadata.get(&name).ok_or_else(|| {
        rejection_error(
            OfflineTransferRejectionReason::PlatformMetadataInvalid,
            platform,
            format!("required metadata entry `{key}` is missing"),
        )
    })?;
    value.try_into_any::<String>().map_err(|err| {
        rejection_error(
            OfflineTransferRejectionReason::PlatformMetadataInvalid,
            platform,
            format!("metadata entry `{key}` is not a string: {err}"),
        )
    })
}

fn provisioned_device_id(manifest: &Metadata) -> Result<String, InstructionExecutionError> {
    let platform = OfflineTransferRejectionPlatform::Android;
    let device_id = metadata_string(manifest, ANDROID_PROVISIONED_DEVICE_ID_KEY, platform)?;
    let trimmed = device_id.trim();
    if trimmed.is_empty() {
        return Err(rejection_error(
            OfflineTransferRejectionReason::PlatformMetadataInvalid,
            platform,
            "android.provisioned.device_id must not be empty",
        ));
    }
    Ok(trimmed.to_string())
}

fn provisioned_counter_scope(
    proof: &AndroidProvisionedProof,
) -> Result<String, InstructionExecutionError> {
    let platform = OfflineTransferRejectionPlatform::Android;
    let schema = proof.manifest_schema.trim();
    if schema.is_empty() {
        return Err(rejection_error(
            OfflineTransferRejectionReason::PlatformMetadataInvalid,
            platform,
            "provisioned manifest schema must not be empty",
        ));
    }
    let device_id = provisioned_device_id(&proof.device_manifest)?;
    Ok(format!("{PROVISIONED_COUNTER_PREFIX}{schema}::{device_id}"))
}

/// Execution logic for offline allowance instructions.
pub mod isi {
    use super::*;
    use crate::smartcontracts::isi::asset::isi::assert_numeric_spec_with;

    const GENESIS_HEIGHT: u64 = 1;

    fn expected_scale_for_allowance(
        certificate: &OfflineWalletCertificate,
        spec: iroha_primitives::numeric::NumericSpec,
    ) -> Result<u32, Error> {
        let allowance_scale = certificate.allowance.amount.scale();
        if let Some(spec_scale) = spec.scale() {
            if allowance_scale != spec_scale {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!("allowance amount must use scale {spec_scale}").into(),
                ));
            }
            return Ok(spec_scale);
        }
        Ok(allowance_scale)
    }

    fn offline_rejection_code_from_error(err: &InstructionExecutionError) -> Option<&str> {
        let InstructionExecutionError::InvariantViolation(message) = err else {
            return None;
        };
        let raw = message.strip_prefix(OFFLINE_REJECTION_REASON_PREFIX)?;
        Some(raw.split_once(':').map_or(raw, |(code, _)| code))
    }

    fn normalize_offline_rejection_code(code: &str) -> Option<String> {
        let reason = OfflineTransferRejectionReason::from_str(code).ok()?;
        Some(rejection_code(reason).to_owned())
    }

    impl Execute for RegisterOfflineLineage {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            register_lineage(self, authority, state_transaction)
        }
    }

    impl Execute for CommitOfflineLineageOperation {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            commit_lineage_operation(self, authority, state_transaction)
        }
    }

    impl Execute for RegisterOfflineAllowance {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            register_allowance(self, authority, state_transaction)
        }
    }

    impl Execute for SubmitOfflineToOnlineTransfer {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let transfer = self.transfer;
            let transfer_for_rejected_row = transfer.clone();
            match submit_transfer(
                SubmitOfflineToOnlineTransfer { transfer },
                authority,
                state_transaction,
            ) {
                Ok(()) => Ok(()),
                Err(err) => {
                    let Some(raw_code) = offline_rejection_code_from_error(&err) else {
                        return Err(err);
                    };
                    let Some(reason_code) = normalize_offline_rejection_code(raw_code) else {
                        return Err(err);
                    };
                    if reason_code
                        == rejection_code(OfflineTransferRejectionReason::DuplicateBundle)
                    {
                        return Err(err);
                    }
                    if state_transaction
                        .world
                        .offline_to_online_transfers
                        .get(&transfer_for_rejected_row.bundle_id)
                        .is_some()
                    {
                        return Err(err);
                    }

                    let recorded_at_ms = state_transaction.block_unix_timestamp_ms();
                    let recorded_at_height = state_transaction.block_height();
                    let mut rejected_record = OfflineTransferRecord {
                        controller: transfer_for_rejected_row
                            .receipts
                            .first()
                            .map(|receipt| receipt.from.clone())
                            .unwrap_or_else(|| authority.clone()),
                        transfer: transfer_for_rejected_row,
                        status: OfflineTransferStatus::Rejected,
                        rejection_reason: Some(reason_code),
                        recorded_at_ms,
                        recorded_at_height,
                        archived_at_height: None,
                        history: Vec::new(),
                        pos_verdict_snapshots: Vec::new(),
                        verdict_snapshot: None,
                        platform_snapshot: None,
                    };
                    rejected_record.push_history_entry(
                        OfflineTransferStatus::Rejected,
                        recorded_at_ms,
                        None,
                    );
                    insert_transfer_record(state_transaction, rejected_record);
                    Ok(())
                }
            }
        }
    }

    impl Execute for RegisterOfflineVerdictRevocation {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            register_verdict_revocation(self, authority, state_transaction)
        }
    }

    impl Execute for ReclaimExpiredOfflineAllowance {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            reclaim_expired_allowance(self, authority, state_transaction)
        }
    }

    impl Execute for LoadOfflineEscrowBalance {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            load_offline_escrow_balance(self, authority, state_transaction)
        }
    }

    impl Execute for RedeemOfflineEscrowBalance {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            redeem_offline_escrow_balance(self, authority, state_transaction)
        }
    }

    fn ensure_can_manage_offline_escrow(
        authority: &AccountId,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let can_manage_offline_escrow = state_transaction
            .world
            .account_permissions
            .get(authority)
            .is_some_and(|perms| {
                perms
                    .iter()
                    .any(|permission| permission.name() == CAN_MANAGE_OFFLINE_ESCROW_PERMISSION)
            });
        if can_manage_offline_escrow {
            Ok(())
        } else {
            Err(labeled_invariant(
                "unauthorized_controller",
                "only an offline escrow manager may mutate shared offline lineages",
            ))
        }
    }

    fn validate_lineage_numeric(raw: &str, field: &'static str) -> Result<Numeric, Error> {
        raw.parse::<Numeric>().map_err(|_| {
            InstructionExecutionError::InvariantViolation(
                format!("offline lineage {field} must be a valid numeric string").into(),
            )
        })
    }

    fn validate_lineage_record(record: &OfflineLineageRecord) -> Result<(), Error> {
        if record.lineage_state.lineage_id.trim().is_empty()
            || record.lineage_state.account_id.trim().is_empty()
            || record.lineage_state.device_id.trim().is_empty()
            || record.lineage_state.offline_public_key.trim().is_empty()
            || record.lineage_state.asset_definition_id.trim().is_empty()
            || record.app_attest_key_id.trim().is_empty()
        {
            return Err(labeled_invariant(
                "invalid_lineage",
                "offline lineage fields must be non-empty",
            ));
        }
        if record.lineage_state.authorization.lineage_id != record.lineage_state.lineage_id
            || record.lineage_state.authorization.account_id != record.lineage_state.account_id
            || record.lineage_state.authorization.device_id != record.lineage_state.device_id
            || record.lineage_state.authorization.offline_public_key
                != record.lineage_state.offline_public_key
            || record.lineage_state.authorization.app_attest_key_id != record.app_attest_key_id
        {
            return Err(labeled_invariant(
                "invalid_lineage",
                "offline lineage authorization does not match the lineage identity",
            ));
        }
        let balance = validate_lineage_numeric(&record.lineage_state.balance, "balance")?;
        let parked =
            validate_lineage_numeric(&record.lineage_state.locked_balance, "locked_balance")?;
        if parked > balance {
            return Err(labeled_invariant(
                "invalid_lineage",
                "offline lineage locked_balance cannot exceed balance",
            ));
        }
        let _ = validate_lineage_numeric(
            &record.lineage_state.authorization.max_balance,
            "authorization.max_balance",
        )?;
        let _ = validate_lineage_numeric(
            &record.lineage_state.authorization.max_tx_value,
            "authorization.max_tx_value",
        )?;
        Ok(())
    }

    fn register_lineage(
        isi: RegisterOfflineLineage,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_can_manage_offline_escrow(authority, state_transaction)?;
        let record = isi.lineage;
        validate_lineage_record(&record)?;
        let lineage_id = record.lineage_state.lineage_id.clone();
        if state_transaction
            .world
            .offline_lineages
            .get(&lineage_id)
            .is_some()
        {
            return Err(labeled_invariant(
                "lineage_duplicate",
                "offline lineage already exists",
            ));
        }
        state_transaction
            .world
            .offline_lineages
            .insert(lineage_id, record);
        Ok(())
    }

    fn commit_lineage_operation(
        isi: CommitOfflineLineageOperation,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_can_manage_offline_escrow(authority, state_transaction)?;
        let record = isi.lineage;
        let result = isi.result;
        validate_lineage_record(&record)?;
        let lineage_id = record.lineage_state.lineage_id.clone();
        if result.operation_key.trim().is_empty()
            || result.kind.trim().is_empty()
            || result.request_hash_hex.trim().is_empty()
        {
            return Err(labeled_invariant(
                "invalid_operation",
                "offline lineage operation metadata must be non-empty",
            ));
        }
        if result.lineage_id != lineage_id || result.envelope.lineage_state != record.lineage_state
        {
            return Err(labeled_invariant(
                "invalid_operation",
                "offline lineage operation result does not match the updated lineage state",
            ));
        }
        let current = state_transaction
            .world
            .offline_lineages
            .get(&lineage_id)
            .cloned();
        let existing_operation_result = state_transaction
            .world
            .offline_lineage_operation_results
            .get(&result.operation_key)
            .cloned();
        if let Some(current) = current {
            if current.lineage_state == record.lineage_state
                && isi.expected_server_revision == current.lineage_state.server_revision
                && isi.expected_state_hash == current.lineage_state.server_state_hash
            {
                if let Some(existing) = existing_operation_result {
                    if existing.kind != result.kind
                        || existing.request_hash_hex != result.request_hash_hex
                        || existing.lineage_id != result.lineage_id
                        || existing.envelope.lineage_state != result.envelope.lineage_state
                    {
                        return Err(labeled_invariant(
                            "operation_duplicate",
                            "offline lineage operation_id is already bound to a different result",
                        ));
                    }
                    let existing_settlement = existing.envelope.settlement.clone();
                    let incoming_settlement = result.envelope.settlement.clone();
                    if existing_settlement == incoming_settlement {
                        return Ok(());
                    }
                    if existing_settlement.is_none() && incoming_settlement.is_some() {
                        state_transaction
                            .world
                            .offline_lineage_operation_results
                            .insert(result.operation_key.clone(), result);
                        return Ok(());
                    }
                    return Err(labeled_invariant(
                        "operation_duplicate",
                        "offline lineage operation_id already exists",
                    ));
                }
            }
            if current.lineage_state.server_revision != isi.expected_server_revision
                || current.lineage_state.server_state_hash != isi.expected_state_hash
            {
                return Err(labeled_invariant(
                    "stale_lineage",
                    "offline lineage mutation is based on a stale anchor",
                ));
            }
            if current.lineage_state.account_id != record.lineage_state.account_id
                || current.lineage_state.device_id != record.lineage_state.device_id
                || current.lineage_state.offline_public_key
                    != record.lineage_state.offline_public_key
                || current.lineage_state.asset_definition_id
                    != record.lineage_state.asset_definition_id
                || current.app_attest_key_id != record.app_attest_key_id
            {
                return Err(labeled_invariant(
                    "invalid_lineage",
                    "offline lineage mutation cannot change lineage identity",
                ));
            }
            if record.lineage_state.pending_local_revision
                < current.lineage_state.pending_local_revision
            {
                return Err(labeled_invariant(
                    "invalid_lineage",
                    "offline lineage mutation cannot rewind pending_local_revision",
                ));
            }
        } else if isi.expected_server_revision != 0 || !isi.expected_state_hash.is_empty() {
            return Err(labeled_invariant(
                "lineage_not_found",
                "offline lineage not found",
            ));
        }
        if existing_operation_result.is_some() {
            return Err(labeled_invariant(
                "operation_duplicate",
                "offline lineage operation_id already exists",
            ));
        }
        if record.lineage_state.server_revision != isi.expected_server_revision.saturating_add(1) {
            return Err(labeled_invariant(
                "invalid_lineage",
                "offline lineage mutation must advance server_revision by exactly one",
            ));
        }
        state_transaction
            .world
            .offline_lineages
            .insert(lineage_id, record);
        state_transaction
            .world
            .offline_lineage_operation_results
            .insert(result.operation_key.clone(), result);
        Ok(())
    }

    fn load_offline_escrow_balance(
        isi: LoadOfflineEscrowBalance,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let asset = isi.asset;
        let amount = isi.amount;
        let can_manage_offline_escrow = state_transaction
            .world
            .account_permissions
            .get(authority)
            .is_some_and(|perms| {
                perms
                    .iter()
                    .any(|permission| permission.name() == CAN_MANAGE_OFFLINE_ESCROW_PERMISSION)
            });
        if asset.account() != authority && !can_manage_offline_escrow {
            return Err(labeled_invariant(
                "unauthorized_controller",
                "only the controller or an offline escrow manager may load online balance into offline escrow",
            ));
        }
        let spec = state_transaction.numeric_spec_for(asset.definition())?;
        assert_numeric_spec_with(&amount, spec)?;
        if amount <= Numeric::zero() {
            return Err(labeled_invariant(
                "invalid_amount",
                "offline load amount must be positive",
            ));
        }
        reserve_offline_allowance(state_transaction, &asset, &amount)
    }

    fn redeem_offline_escrow_balance(
        isi: RedeemOfflineEscrowBalance,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let asset = isi.asset;
        let amount = isi.amount;
        let can_manage_offline_escrow = state_transaction
            .world
            .account_permissions
            .get(authority)
            .is_some_and(|perms| {
                perms
                    .iter()
                    .any(|permission| permission.name() == CAN_MANAGE_OFFLINE_ESCROW_PERMISSION)
            });
        if asset.account() != authority && !can_manage_offline_escrow {
            return Err(labeled_invariant(
                "unauthorized_controller",
                "only the controller or an offline escrow manager may redeem offline escrow back online",
            ));
        }
        let spec = state_transaction.numeric_spec_for(asset.definition())?;
        assert_numeric_spec_with(&amount, spec)?;
        if amount <= Numeric::zero() {
            return Err(labeled_invariant(
                "invalid_amount",
                "offline redeem amount must be positive",
            ));
        }
        refund_allowance_from_escrow(state_transaction, &asset, authority, &amount)
    }

    fn register_allowance(
        isi: RegisterOfflineAllowance,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let certificate = isi.certificate;
        let is_genesis_block = state_transaction.block_height() == GENESIS_HEIGHT;

        ensure_allowance_owner_matches_controller(&certificate)?;

        if is_genesis_block {
            let certificate_id = certificate.certificate_id();
            reserve_offline_allowance(
                state_transaction,
                &certificate.allowance.asset,
                &certificate.allowance.amount,
            )?;
            let record = OfflineAllowanceRecord {
                certificate: certificate.clone(),
                current_commitment: certificate.allowance.commitment.clone(),
                registered_at_ms: certificate.issued_at_ms,
                remaining_amount: certificate.allowance.amount,
                counter_state: OfflineCounterState::default(),
                verdict_id: certificate.verdict_id,
                attestation_nonce: certificate.attestation_nonce,
                refresh_at_ms: certificate.refresh_at_ms,
            };
            state_transaction
                .world
                .offline_allowances
                .insert(certificate_id, record);
            return Ok(());
        }

        if !is_offline_allowance_authority(&certificate.controller, authority, is_genesis_block) {
            return Err(labeled_invariant(
                "unauthorized_controller",
                "only the certificate controller may register an offline allowance",
            ));
        }

        let spec = state_transaction.numeric_spec_for(certificate.allowance.asset.definition())?;
        let expected_scale = expected_scale_for_allowance(&certificate, spec)?;
        assert_numeric_spec_with(&certificate.allowance.amount, spec)?;
        assert_numeric_spec_with(&certificate.policy.max_balance, spec)?;
        assert_numeric_spec_with(&certificate.policy.max_tx_value, spec)?;
        ensure_certificate_policy(&certificate, expected_scale)?;
        ensure_operator_signature(&certificate)?;
        let lineage = parse_certificate_lineage_metadata(&certificate)?;
        validate_certificate_lineage(state_transaction, &certificate, &lineage)?;

        let now_ms = state_transaction.block_unix_timestamp_ms();
        if certificate.issued_at_ms > now_ms {
            return Err(labeled_invariant(
                "issued_at_in_future",
                "certificate cannot be registered before issued_at_ms",
            ));
        }
        if certificate.expires_at_ms <= now_ms {
            return Err(labeled_invariant(
                OfflineTransferRejectionReason::CertificateExpired.as_label(),
                "certificate is expired",
            ));
        }

        if let Some(verdict_id) = certificate.verdict_id.as_ref() {
            if verdict_id_in_use(
                state_transaction
                    .world
                    .offline_allowances
                    .iter()
                    .map(|(_, record)| record),
                verdict_id,
            ) {
                return Err(labeled_invariant(
                    "verdict_id_duplicate",
                    "offline allowance verdict_id already registered",
                ));
            }
        }

        let certificate_id = certificate.certificate_id();
        if state_transaction
            .world
            .offline_allowances
            .get(&certificate_id)
            .is_some()
        {
            return Err(labeled_invariant(
                "allowance_duplicate",
                "offline allowance already registered",
            ));
        }

        reserve_or_reallocate_allowance(state_transaction, &certificate, &lineage)?;
        let record = OfflineAllowanceRecord {
            certificate: certificate.clone(),
            current_commitment: certificate.allowance.commitment.clone(),
            registered_at_ms: now_ms,
            remaining_amount: certificate.allowance.amount,
            counter_state: OfflineCounterState::default(),
            verdict_id: certificate.verdict_id,
            attestation_nonce: certificate.attestation_nonce,
            refresh_at_ms: certificate.refresh_at_ms,
        };
        state_transaction
            .world
            .offline_allowances
            .insert(certificate_id, record);
        Ok(())
    }

    pub(super) fn is_offline_allowance_authority(
        controller: &AccountId,
        authority: &AccountId,
        is_genesis_block: bool,
    ) -> bool {
        // Genesis transactions are always authored by the genesis account; allow them to seed
        // allowances even when the controller differs.
        controller == authority || is_genesis_block
    }

    fn verdict_id_in_use<'a, I>(records: I, verdict_id: &Hash) -> bool
    where
        I: IntoIterator<Item = &'a OfflineAllowanceRecord>,
    {
        records
            .into_iter()
            .any(|record| record.verdict_id.as_ref() == Some(verdict_id))
    }

    #[cfg(test)]
    mod register_tests {
        use std::{str::FromStr, sync::Arc, time::Duration};

        use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
        use iroha_crypto::Algorithm;
        use iroha_data_model::{
            ChainId,
            account::Account,
            asset::{Asset, AssetDefinition},
            block::BlockHeader,
            domain::{Domain, DomainId},
            offline::{
                AndroidProvisionedProof, AppleAppAttestProof, OFFLINE_ASSET_ENABLED_METADATA_KEY,
                OfflineBuildClaim, OfflineBuildClaimPlatform, OfflineCertificateBalanceProof,
                OfflineVerdictRevocationReason,
            },
            query::error::FindError,
        };
        use iroha_primitives::{json::Json, numeric::NumericSpec};
        use nonzero_ext::nonzero;

        use super::*;
        use crate::{
            kura::Kura,
            query::store::LiveQueryStore,
            state::{State, World},
        };

        fn offline_domain_id() -> DomainId {
            DomainId::from_str("offline").expect("domain id")
        }

        fn sample_account(seed: u8) -> AccountId {
            let keypair = iroha_crypto::KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
            AccountId::new(keypair.public_key().clone())
        }

        fn set_certificate_lineage(
            certificate: &mut OfflineWalletCertificate,
            scope: &str,
            epoch: u64,
            prev_certificate_id: Option<Hash>,
        ) {
            let mut metadata = Metadata::default();
            metadata.insert(
                OFFLINE_LINEAGE_SCOPE_KEY.parse().expect("metadata key"),
                Json::new(scope.to_owned()),
            );
            metadata.insert(
                OFFLINE_LINEAGE_EPOCH_KEY.parse().expect("metadata key"),
                Json::new(epoch.to_string()),
            );
            metadata.insert(
                OFFLINE_BUILD_CLAIM_MIN_BUILD_NUMBER_KEY
                    .parse()
                    .expect("metadata key"),
                Json::new("1".to_owned()),
            );
            if let Some(prev_certificate_id) = prev_certificate_id {
                metadata.insert(
                    OFFLINE_LINEAGE_PREV_CERTIFICATE_ID_HEX_KEY
                        .parse()
                        .expect("metadata key"),
                    Json::new(hex::encode(prev_certificate_id.as_ref())),
                );
            }
            certificate.metadata = metadata;
        }

        fn sample_certificate() -> OfflineWalletCertificate {
            let controller = sample_account(0x01);
            let definition =
                AssetDefinitionId::new("offline".parse().unwrap(), "xor".parse().unwrap());
            let asset = AssetId::new(definition, controller.clone());
            let spend_pair = iroha_crypto::KeyPair::from_seed(vec![0xEE; 32], Algorithm::Ed25519);
            let mut certificate = OfflineWalletCertificate {
                controller,
                operator: sample_account(0x01),
                allowance: OfflineAllowanceCommitment {
                    asset,
                    amount: Numeric::new(1_000, 0),
                    commitment: vec![0xAB; 32],
                },
                spend_public_key: spend_pair.public_key().clone(),
                attestation_report: Vec::new(),
                issued_at_ms: 1_700_000_000,
                expires_at_ms: 1_800_000_000,
                policy: OfflineWalletPolicy {
                    max_balance: Numeric::new(1_000, 0),
                    max_tx_value: Numeric::new(250, 0),
                    expires_at_ms: 1_800_000_000,
                },
                operator_signature: Signature::from_bytes(&[0; 64]),
                metadata: Metadata::default(),
                verdict_id: None,
                attestation_nonce: None,
                refresh_at_ms: None,
            };
            set_certificate_lineage(&mut certificate, "register-tests-default", 1, None);
            certificate
        }

        #[test]
        fn expected_scale_matches_asset_spec() {
            let mut certificate = sample_certificate();
            certificate.allowance.amount = Numeric::new(1_000, 2);
            let spec = NumericSpec::fractional(2);
            assert_eq!(
                expected_scale_for_allowance(&certificate, spec).expect("expected scale"),
                2
            );
        }

        #[test]
        fn expected_scale_rejects_scale_mismatch() {
            let mut certificate = sample_certificate();
            certificate.allowance.amount = Numeric::new(1_000, 2);
            let spec = NumericSpec::fractional(0);
            assert!(expected_scale_for_allowance(&certificate, spec).is_err());
        }

        #[test]
        fn allowance_owner_must_match_controller() {
            let mut certificate = sample_certificate();
            let definition = certificate.allowance.asset.definition().clone();
            let other_account = sample_account(0xFF);
            certificate.allowance.asset =
                iroha_data_model::asset::AssetId::new(definition, other_account);
            assert!(ensure_allowance_owner_matches_controller(&certificate).is_err());
        }

        #[test]
        fn register_allowance_reserves_escrow_when_required() {
            let mut certificate = sample_certificate();
            let operator_keys = KeyPair::from_seed(vec![0x01; 32], Algorithm::Ed25519);
            certificate.operator_signature = Signature::new(
                operator_keys.private_key(),
                &certificate
                    .operator_signing_bytes()
                    .expect("certificate signing bytes"),
            );

            let controller = certificate.controller.clone();
            let escrow = sample_account(0x02);
            let definition_id = certificate.allowance.asset.definition().clone();
            certificate.expires_at_ms = certificate.issued_at_ms + 1;
            certificate.policy.expires_at_ms = certificate.expires_at_ms;
            let domain_id = offline_domain_id();
            let domain = Domain::new(domain_id.clone()).build(&controller);
            let controller_account =
                Account::new(controller.clone().to_account_id(domain_id.clone()))
                    .build(&controller);
            let escrow_account =
                Account::new(escrow.clone().to_account_id(domain_id)).build(&escrow);
            let asset_definition =
                AssetDefinition::new(definition_id.clone(), NumericSpec::integer())
                    .build(&controller);
            let world = World::with(
                [domain],
                [controller_account, escrow_account],
                [asset_definition],
            );

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new(world, Arc::clone(&kura), query);
            state.settlement.offline.escrow_required = true;
            state
                .settlement
                .offline
                .escrow_accounts
                .insert(definition_id.clone(), escrow.clone());

            let header = BlockHeader::new(
                nonzero!(1_u64),
                None,
                None,
                None,
                certificate.issued_at_ms + 1,
                0,
            );
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            transaction
                .world
                .deposit_numeric_asset(&certificate.allowance.asset, &certificate.allowance.amount)
                .expect("prefund allowance asset");

            register_allowance(
                RegisterOfflineAllowance {
                    certificate: certificate.clone(),
                },
                &controller,
                &mut transaction,
            )
            .expect("register allowance");

            let controller_balance = transaction
                .world
                .assets
                .get(&certificate.allowance.asset)
                .map(|asset| asset.as_ref().clone())
                .unwrap_or_else(Numeric::zero);
            assert_eq!(controller_balance, Numeric::zero());

            let escrow_asset = AssetId::new(definition_id, escrow);
            let escrow_balance = transaction
                .world
                .assets
                .get(&escrow_asset)
                .map(|asset| asset.as_ref().clone())
                .unwrap_or_else(Numeric::zero);
            assert_eq!(escrow_balance, certificate.allowance.amount);
        }

        #[test]
        fn register_allowance_zero_amount_does_not_require_prefund() {
            let mut certificate = sample_certificate();
            certificate.allowance.amount = Numeric::new(0, 0);
            let operator_keys = KeyPair::from_seed(vec![0x01; 32], Algorithm::Ed25519);
            certificate.operator_signature = Signature::new(
                operator_keys.private_key(),
                &certificate
                    .operator_signing_bytes()
                    .expect("certificate signing bytes"),
            );

            let controller = certificate.controller.clone();
            let escrow = sample_account(0x02);
            let definition_id = certificate.allowance.asset.definition().clone();
            let domain_id = offline_domain_id();
            let domain = Domain::new(domain_id.clone()).build(&controller);
            let controller_account =
                Account::new(controller.clone().to_account_id(domain_id.clone()))
                    .build(&controller);
            let escrow_account =
                Account::new(escrow.clone().to_account_id(domain_id)).build(&escrow);
            let asset_definition =
                AssetDefinition::new(definition_id.clone(), NumericSpec::integer())
                    .build(&controller);
            let world = World::with(
                [domain],
                [controller_account, escrow_account],
                [asset_definition],
            );

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new(world, Arc::clone(&kura), query);
            state.settlement.offline.escrow_required = true;
            state
                .settlement
                .offline
                .escrow_accounts
                .insert(definition_id.clone(), escrow.clone());

            let header = BlockHeader::new(
                nonzero!(2_u64),
                None,
                None,
                None,
                certificate.issued_at_ms + 1,
                0,
            );
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            register_allowance(
                RegisterOfflineAllowance {
                    certificate: certificate.clone(),
                },
                &controller,
                &mut transaction,
            )
            .expect("register allowance");

            assert!(
                transaction
                    .world
                    .assets
                    .get(&certificate.allowance.asset)
                    .is_none(),
                "controller asset should not be created for zero amount"
            );
            let escrow_asset = AssetId::new(definition_id, escrow);
            assert!(
                transaction.world.assets.get(&escrow_asset).is_none(),
                "escrow asset should not be created for zero amount"
            );
            let certificate_id = certificate.certificate_id();
            assert!(
                transaction
                    .world
                    .offline_allowances
                    .get(&certificate_id)
                    .is_some(),
                "allowance record should be created"
            );
        }

        #[test]
        fn register_allowance_nonzero_requires_prefund_when_escrow_required() {
            let mut certificate = sample_certificate();
            let operator_keys = KeyPair::from_seed(vec![0x01; 32], Algorithm::Ed25519);
            certificate.operator_signature = Signature::new(
                operator_keys.private_key(),
                &certificate
                    .operator_signing_bytes()
                    .expect("certificate signing bytes"),
            );

            let controller = certificate.controller.clone();
            let escrow = sample_account(0x02);
            let definition_id = certificate.allowance.asset.definition().clone();
            let domain_id = offline_domain_id();
            let domain = Domain::new(domain_id.clone()).build(&controller);
            let controller_account =
                Account::new(controller.clone().to_account_id(domain_id.clone()))
                    .build(&controller);
            let escrow_account =
                Account::new(escrow.clone().to_account_id(domain_id)).build(&escrow);
            let asset_definition =
                AssetDefinition::new(definition_id.clone(), NumericSpec::integer())
                    .build(&controller);
            let world = World::with(
                [domain],
                [controller_account, escrow_account],
                [asset_definition],
            );

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new(world, Arc::clone(&kura), query);
            state.settlement.offline.escrow_required = true;
            state
                .settlement
                .offline
                .escrow_accounts
                .insert(definition_id, escrow);

            let header = BlockHeader::new(
                nonzero!(2_u64),
                None,
                None,
                None,
                certificate.issued_at_ms + 1,
                0,
            );
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = register_allowance(
                RegisterOfflineAllowance {
                    certificate: certificate.clone(),
                },
                &controller,
                &mut transaction,
            )
            .expect_err("non-zero allowance without prefund should fail");
            assert!(
                matches!(err, Error::Find(FindError::Asset(_))),
                "unexpected error: {err}"
            );
        }

        #[test]
        fn register_allowance_zero_amount_still_requires_escrow_config_when_required() {
            let mut certificate = sample_certificate();
            certificate.allowance.amount = Numeric::new(0, 0);
            let operator_keys = KeyPair::from_seed(vec![0x01; 32], Algorithm::Ed25519);
            certificate.operator_signature = Signature::new(
                operator_keys.private_key(),
                &certificate
                    .operator_signing_bytes()
                    .expect("certificate signing bytes"),
            );

            let controller = certificate.controller.clone();
            let definition_id = certificate.allowance.asset.definition().clone();
            let domain_id = offline_domain_id();
            let domain = Domain::new(domain_id.clone()).build(&controller);
            let controller_account =
                Account::new(controller.clone().to_account_id(domain_id)).build(&controller);
            let asset_definition =
                AssetDefinition::new(definition_id, NumericSpec::integer()).build(&controller);
            let world = World::with([domain], [controller_account], [asset_definition]);

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new(world, Arc::clone(&kura), query);
            state.settlement.offline.escrow_required = true;

            let header = BlockHeader::new(
                nonzero!(2_u64),
                None,
                None,
                None,
                certificate.issued_at_ms + 1,
                0,
            );
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = register_allowance(
                RegisterOfflineAllowance {
                    certificate: certificate.clone(),
                },
                &controller,
                &mut transaction,
            )
            .expect_err("missing escrow config should be rejected");
            let expected = format!("{OFFLINE_REJECTION_REASON_PREFIX}escrow_missing");
            assert!(
                err.to_string().contains(&expected),
                "unexpected rejection: {err}"
            );
        }

        #[test]
        fn register_allowance_fails_without_escrow_when_not_required_and_metadata_disabled_or_missing()
         {
            for offline_enabled in [None, Some(false)] {
                let mut certificate = sample_certificate();
                let operator_keys = KeyPair::from_seed(vec![0x01; 32], Algorithm::Ed25519);
                certificate.operator_signature = Signature::new(
                    operator_keys.private_key(),
                    &certificate
                        .operator_signing_bytes()
                        .expect("certificate signing bytes"),
                );

                let controller = certificate.controller.clone();
                let definition_id = certificate.allowance.asset.definition().clone();
                let domain_id = offline_domain_id();
                let domain = Domain::new(domain_id.clone()).build(&controller);
                let controller_account =
                    Account::new(controller.clone().to_account_id(domain_id)).build(&controller);
                let mut asset_definition =
                    AssetDefinition::new(definition_id.clone(), NumericSpec::integer())
                        .build(&controller);
                if let Some(offline_enabled) = offline_enabled {
                    asset_definition.metadata_mut().insert(
                        OFFLINE_ASSET_ENABLED_METADATA_KEY
                            .parse()
                            .expect("metadata key"),
                        Json::new(offline_enabled),
                    );
                }
                let world = World::with([domain], [controller_account], [asset_definition]);

                let kura = Kura::blank_kura_for_testing();
                let query = LiveQueryStore::start_test();
                let mut state = State::new(world, Arc::clone(&kura), query);
                state.settlement.offline.escrow_required = false;

                let header = BlockHeader::new(
                    nonzero!(2_u64),
                    None,
                    None,
                    None,
                    certificate.issued_at_ms + 1,
                    0,
                );
                let mut block = state.block(header);
                let mut transaction = block.transaction();
                transaction
                    .world
                    .deposit_numeric_asset(
                        &certificate.allowance.asset,
                        &certificate.allowance.amount,
                    )
                    .expect("prefund allowance asset");

                let err = register_allowance(
                    RegisterOfflineAllowance {
                        certificate: certificate.clone(),
                    },
                    &controller,
                    &mut transaction,
                )
                .expect_err("missing escrow should be rejected");
                let expected = format!("{OFFLINE_REJECTION_REASON_PREFIX}escrow_missing");
                assert!(
                    err.to_string().contains(&expected),
                    "unexpected error for offline_enabled={offline_enabled:?}: {err}"
                );
            }
        }

        #[test]
        fn register_allowance_derives_escrow_from_metadata_when_map_missing() {
            let mut certificate = sample_certificate();
            let operator_keys = KeyPair::from_seed(vec![0x01; 32], Algorithm::Ed25519);
            certificate.operator_signature = Signature::new(
                operator_keys.private_key(),
                &certificate
                    .operator_signing_bytes()
                    .expect("certificate signing bytes"),
            );

            let controller = certificate.controller.clone();
            let definition_id = certificate.allowance.asset.definition().clone();
            let domain_id = offline_domain_id();
            let domain = Domain::new(domain_id.clone()).build(&controller);
            let controller_account =
                Account::new(controller.clone().to_account_id(domain_id)).build(&controller);

            let mut asset_definition =
                AssetDefinition::new(definition_id.clone(), NumericSpec::integer())
                    .build(&controller);
            asset_definition.metadata_mut().insert(
                OFFLINE_ASSET_ENABLED_METADATA_KEY
                    .parse()
                    .expect("metadata key"),
                Json::new(true),
            );

            let chain_id: ChainId = "testnet".parse().expect("chain id");
            let escrow_account_id =
                crate::smartcontracts::isi::domain::isi::offline_escrow_account_id(
                    &chain_id,
                    &definition_id,
                );

            let world = World::with([domain], [controller_account], [asset_definition]);
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_with_chain(world, Arc::clone(&kura), query, chain_id);
            state.settlement.offline.escrow_required = true;

            let header = BlockHeader::new(
                nonzero!(1_u64),
                None,
                None,
                None,
                certificate.issued_at_ms + 1,
                0,
            );
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            transaction
                .world
                .deposit_numeric_asset(&certificate.allowance.asset, &certificate.allowance.amount)
                .expect("prefund allowance asset");

            register_allowance(
                RegisterOfflineAllowance {
                    certificate: certificate.clone(),
                },
                &controller,
                &mut transaction,
            )
            .expect("register allowance");

            assert!(
                transaction.world.account(&escrow_account_id).is_ok(),
                "escrow account should be created"
            );

            let escrow_asset = AssetId::new(definition_id, escrow_account_id);
            let escrow_balance = transaction
                .world
                .assets
                .get(&escrow_asset)
                .map(|asset| asset.as_ref().clone())
                .unwrap_or_else(Numeric::zero);
            assert_eq!(escrow_balance, certificate.allowance.amount);
        }

        #[test]
        fn credit_deposit_account_debits_escrow_when_required() {
            let escrow = sample_account(0x02);
            let deposit = sample_account(0x03);
            let definition_id =
                AssetDefinitionId::new("offline".parse().unwrap(), "xor".parse().unwrap());
            let domain_id = offline_domain_id();
            let domain = Domain::new(domain_id.clone()).build(&deposit);
            let escrow_account =
                Account::new(escrow.clone().to_account_id(domain_id.clone())).build(&escrow);
            let deposit_account =
                Account::new(deposit.clone().to_account_id(domain_id)).build(&deposit);
            let asset_definition =
                AssetDefinition::new(definition_id.clone(), NumericSpec::integer()).build(&deposit);
            let world = World::with(
                [domain],
                [escrow_account, deposit_account],
                [asset_definition],
            );

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new(world, Arc::clone(&kura), query);
            state.settlement.offline.escrow_required = true;
            state
                .settlement
                .offline
                .escrow_accounts
                .insert(definition_id.clone(), escrow.clone());

            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 1_700_000_000, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let escrow_asset = AssetId::new(definition_id.clone(), escrow.clone());
            transaction
                .world
                .deposit_numeric_asset(&escrow_asset, &Numeric::new(250, 0))
                .expect("prefund escrow");

            let asset = AssetId::new(definition_id.clone(), deposit.clone());
            let amount = Numeric::new(100, 0);
            credit_deposit_account(&mut transaction, &asset, &deposit, &amount)
                .expect("credit deposit account");

            let escrow_balance = transaction
                .world
                .assets
                .get(&escrow_asset)
                .map(|asset| asset.as_ref().clone())
                .unwrap_or_else(Numeric::zero);
            assert_eq!(escrow_balance, Numeric::new(150, 0));

            let deposit_asset = AssetId::new(definition_id, deposit);
            let deposit_balance = transaction
                .world
                .assets
                .get(&deposit_asset)
                .map(|asset| asset.as_ref().clone())
                .unwrap_or_else(Numeric::zero);
            assert_eq!(deposit_balance, amount);
        }

        #[test]
        fn credit_deposit_account_rejects_missing_escrow_when_not_required() {
            let deposit = sample_account(0x03);
            let definition_id =
                AssetDefinitionId::new("offline".parse().unwrap(), "xor".parse().unwrap());
            let domain_id = offline_domain_id();
            let domain = Domain::new(domain_id.clone()).build(&deposit);
            let deposit_account =
                Account::new(deposit.clone().to_account_id(domain_id)).build(&deposit);
            let mut asset_definition =
                AssetDefinition::new(definition_id.clone(), NumericSpec::integer()).build(&deposit);
            asset_definition.metadata_mut().insert(
                OFFLINE_ASSET_ENABLED_METADATA_KEY
                    .parse()
                    .expect("metadata key"),
                Json::new(false),
            );
            let world = World::with([domain], [deposit_account], [asset_definition]);

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new(world, Arc::clone(&kura), query);
            state.settlement.offline.escrow_required = false;

            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 1_700_000_000, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let asset = AssetId::new(definition_id.clone(), deposit.clone());
            let amount = Numeric::new(100, 0);
            let err = credit_deposit_account(&mut transaction, &asset, &deposit, &amount)
                .expect_err("missing escrow should reject settlement credit");
            let expected = format!("{OFFLINE_REJECTION_REASON_PREFIX}escrow_missing");
            assert!(err.to_string().contains(&expected));

            let deposit_asset = AssetId::new(definition_id, deposit);
            assert!(
                transaction.world.assets.get(&deposit_asset).is_none(),
                "deposit account must not be credited when escrow is missing"
            );
        }

        #[test]
        fn credit_deposit_account_rejects_insufficient_escrow_without_credit() {
            let escrow = sample_account(0x02);
            let deposit = sample_account(0x03);
            let definition_id =
                AssetDefinitionId::new("offline".parse().unwrap(), "xor".parse().unwrap());
            let domain_id = offline_domain_id();
            let domain = Domain::new(domain_id.clone()).build(&deposit);
            let escrow_account =
                Account::new(escrow.clone().to_account_id(domain_id.clone())).build(&escrow);
            let deposit_account =
                Account::new(deposit.clone().to_account_id(domain_id)).build(&deposit);
            let asset_definition =
                AssetDefinition::new(definition_id.clone(), NumericSpec::integer()).build(&deposit);
            let world = World::with(
                [domain],
                [escrow_account, deposit_account],
                [asset_definition],
            );

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new(world, Arc::clone(&kura), query);
            state.settlement.offline.escrow_required = true;
            state
                .settlement
                .offline
                .escrow_accounts
                .insert(definition_id.clone(), escrow.clone());

            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 1_700_000_000, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let escrow_asset = AssetId::new(definition_id.clone(), escrow.clone());
            transaction
                .world
                .deposit_numeric_asset(&escrow_asset, &Numeric::new(50, 0))
                .expect("prefund escrow");

            let asset = AssetId::new(definition_id.clone(), deposit.clone());
            let amount = Numeric::new(100, 0);
            credit_deposit_account(&mut transaction, &asset, &deposit, &amount)
                .expect_err("insufficient escrow must reject settlement credit");

            let escrow_balance = transaction
                .world
                .assets
                .get(&escrow_asset)
                .map(|asset| asset.as_ref().clone())
                .unwrap_or_else(Numeric::zero);
            assert_eq!(escrow_balance, Numeric::new(50, 0));

            let deposit_asset = AssetId::new(definition_id, deposit);
            assert!(
                transaction.world.assets.get(&deposit_asset).is_none(),
                "deposit account must remain unchanged when escrow is insufficient"
            );
        }

        #[test]
        fn credit_deposit_account_rejects_missing_deposit_without_escrow_debit() {
            let escrow = sample_account(0x02);
            let deposit = sample_account(0x03);
            let definition_id =
                AssetDefinitionId::new("offline".parse().unwrap(), "xor".parse().unwrap());
            let domain_id = offline_domain_id();
            let domain = Domain::new(domain_id.clone()).build(&escrow);
            let escrow_account =
                Account::new(escrow.clone().to_account_id(domain_id)).build(&escrow);
            let asset_definition =
                AssetDefinition::new(definition_id.clone(), NumericSpec::integer()).build(&escrow);
            let world = World::with([domain], [escrow_account], [asset_definition]);

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new(world, Arc::clone(&kura), query);
            state.settlement.offline.escrow_required = true;
            state
                .settlement
                .offline
                .escrow_accounts
                .insert(definition_id.clone(), escrow.clone());

            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 1_700_000_000, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let escrow_asset = AssetId::new(definition_id.clone(), escrow.clone());
            transaction
                .world
                .deposit_numeric_asset(&escrow_asset, &Numeric::new(250, 0))
                .expect("prefund escrow");

            let asset = AssetId::new(definition_id.clone(), escrow.clone());
            let amount = Numeric::new(100, 0);
            credit_deposit_account(&mut transaction, &asset, &deposit, &amount)
                .expect_err("missing deposit account must reject settlement credit");

            let escrow_balance = transaction
                .world
                .assets
                .get(&escrow_asset)
                .map(|asset| asset.as_ref().clone())
                .unwrap_or_else(Numeric::zero);
            assert_eq!(escrow_balance, Numeric::new(250, 0));

            let deposit_asset = AssetId::new(definition_id, deposit);
            assert!(
                transaction.world.assets.get(&deposit_asset).is_none(),
                "escrow debit must be reverted when destination deposit fails"
            );
        }

        fn sample_transfer_record_with_policy(
            bundle_tag: &'static [u8],
            policy: AndroidIntegrityPolicy,
        ) -> OfflineTransferRecord {
            let certificate = sample_certificate();
            let receiver = sample_account(0x02);
            let deposit_account = sample_account(0x03);
            let receipt = OfflineSpendReceipt {
                tx_id: Hash::new(b"receipt"),
                from: certificate.controller.clone(),
                to: receiver.clone(),
                asset: certificate.allowance.asset.clone(),
                amount: Numeric::new(100, 0),
                issued_at_ms: 1_700_000_100,
                invoice_id: "INV-001".into(),
                platform_proof: OfflinePlatformProof::AppleAppAttest(AppleAppAttestProof {
                    key_id: BASE64_STANDARD.encode(b"apple"),
                    counter: 1,
                    assertion: vec![],
                    challenge_hash: Hash::new(b"challenge"),
                }),
                platform_snapshot: None,
                sender_certificate_id: certificate.certificate_id(),
                sender_signature: Signature::from_bytes(&[0; 64]),
                build_claim: None,
            };
            let snapshot = OfflinePlatformTokenSnapshot {
                policy: policy.as_str().into(),
                attestation_jws_b64: "token".into(),
            };
            let transfer = OfflineToOnlineTransfer {
                bundle_id: Hash::new(bundle_tag),
                receiver,
                deposit_account,
                receipts: vec![receipt],
                balance_proof: OfflineBalanceProof {
                    initial_commitment: certificate.allowance.clone(),
                    resulting_commitment: vec![0; 32],
                    claimed_delta: Numeric::new(100, 0),
                    zk_proof: None,
                },
                balance_proofs: None,
                aggregate_proof: None,
                attachments: None,
                platform_snapshot: Some(snapshot.clone()),
            };
            OfflineTransferRecord {
                transfer,
                controller: certificate.controller.clone(),
                status: OfflineTransferStatus::Settled,
                rejection_reason: None,
                recorded_at_ms: 1,
                recorded_at_height: 1,
                archived_at_height: None,
                history: Vec::new(),
                pos_verdict_snapshots: vec![OfflineVerdictSnapshot::from_certificate(&certificate)],
                verdict_snapshot: None,
                platform_snapshot: Some(snapshot),
            }
        }

        fn record_with_verdict(verdict: Option<Hash>) -> OfflineAllowanceRecord {
            OfflineAllowanceRecord {
                certificate: sample_certificate(),
                current_commitment: vec![0; 32],
                registered_at_ms: 1_700_000_001,
                remaining_amount: Numeric::new(1_000, 0),
                counter_state: OfflineCounterState::default(),
                verdict_id: verdict,
                attestation_nonce: None,
                refresh_at_ms: None,
            }
        }

        fn sample_transfer_with_receipt_timestamp(
            issued_at_ms: u64,
        ) -> (OfflineToOnlineTransfer, OfflineAllowanceRecord) {
            let certificate = sample_certificate();
            let controller = certificate.controller.clone();
            let receipt = OfflineSpendReceipt {
                tx_id: Hash::new(b"receipt-ts"),
                from: controller.clone(),
                to: controller.clone(),
                asset: certificate.allowance.asset.clone(),
                amount: Numeric::new(100, 0),
                issued_at_ms,
                invoice_id: "INV-TS".into(),
                platform_proof: OfflinePlatformProof::AppleAppAttest(AppleAppAttestProof {
                    key_id: BASE64_STANDARD.encode(b"apple"),
                    counter: 1,
                    assertion: vec![],
                    challenge_hash: Hash::new(b"challenge"),
                }),
                platform_snapshot: None,
                sender_certificate_id: certificate.certificate_id(),
                sender_signature: Signature::from_bytes(&[0; 64]),
                build_claim: None,
            };
            let transfer = OfflineToOnlineTransfer {
                bundle_id: Hash::new(b"bundle-ts"),
                receiver: controller.clone(),
                deposit_account: controller.clone(),
                receipts: vec![receipt],
                balance_proof: OfflineBalanceProof {
                    initial_commitment: certificate.allowance.clone(),
                    resulting_commitment: vec![0; 32],
                    claimed_delta: Numeric::new(100, 0),
                    zk_proof: None,
                },
                balance_proofs: None,
                aggregate_proof: None,
                attachments: None,
                platform_snapshot: None,
            };
            let record = OfflineAllowanceRecord {
                certificate,
                current_commitment: vec![0; 32],
                registered_at_ms: 1_700_000_010,
                remaining_amount: Numeric::new(1_000, 0),
                counter_state: OfflineCounterState::default(),
                verdict_id: None,
                attestation_nonce: None,
                refresh_at_ms: None,
            };
            (transfer, record)
        }

        fn attach_android_provisioned_build_claim(
            receipt: &mut OfflineSpendReceipt,
            certificate: &mut OfflineWalletCertificate,
            claim_id: Hash,
        ) {
            let app_id = "com.example.offline.android";
            certificate.metadata.insert(
                ANDROID_PROVISIONED_APP_ID_KEY
                    .parse()
                    .expect("metadata key"),
                Json::new(app_id.to_owned()),
            );
            let operator_keys = KeyPair::from_seed(vec![0x01; 32], Algorithm::Ed25519);
            certificate.operator_signature = Signature::new(
                operator_keys.private_key(),
                &certificate
                    .operator_signing_bytes()
                    .expect("certificate signing bytes"),
            );

            let mut device_manifest = Metadata::default();
            device_manifest.insert(
                Name::from_str(ANDROID_PROVISIONED_DEVICE_ID_KEY).expect("metadata key"),
                Json::new("device-001".to_owned()),
            );
            receipt.platform_proof = OfflinePlatformProof::Provisioned(AndroidProvisionedProof {
                manifest_schema: "offline_provisioning_current".to_owned(),
                manifest_version: Some(1),
                manifest_issued_at_ms: receipt.issued_at_ms.saturating_sub(1),
                challenge_hash: Hash::new(b"provisioned-challenge"),
                counter: 1,
                device_manifest,
                inspector_signature: Signature::from_bytes(&[0; 64]),
            });

            let lineage_scope = metadata_string_field(
                &certificate.metadata,
                OFFLINE_LINEAGE_SCOPE_KEY,
                "lineage scope",
            )
            .expect("lineage scope");
            let mut build_claim = OfflineBuildClaim {
                claim_id,
                platform: OfflineBuildClaimPlatform::Android,
                app_id: app_id.to_owned(),
                build_number: 1,
                issued_at_ms: receipt.issued_at_ms.saturating_sub(1),
                expires_at_ms: receipt.issued_at_ms + 60_000,
                lineage_scope,
                nonce: receipt.tx_id,
                operator_signature: Signature::from_bytes(&[0; 64]),
            };
            build_claim.operator_signature = Signature::new(
                operator_keys.private_key(),
                &build_claim
                    .signing_bytes()
                    .expect("build claim signing bytes"),
            );
            receipt.build_claim = Some(build_claim);
            receipt.sender_certificate_id = certificate.certificate_id();

            let spend_pair = KeyPair::from_seed(vec![0xEE; 32], Algorithm::Ed25519);
            receipt.sender_signature = Signature::new(
                spend_pair.private_key(),
                &receipt.signing_bytes().expect("receipt signing bytes"),
            );
        }

        #[test]
        fn receipt_amount_exceeds_policy_max_is_rejected() {
            let mut certificate = sample_certificate();
            certificate.policy.max_tx_value = Numeric::new(100, 0);
            let receiver = sample_account(0x02);
            let deposit_account = sample_account(0x03);
            let receipt = OfflineSpendReceipt {
                tx_id: Hash::new(b"receipt-max"),
                from: certificate.controller.clone(),
                to: receiver.clone(),
                asset: certificate.allowance.asset.clone(),
                amount: Numeric::new(150, 0),
                issued_at_ms: 1_700_000_100,
                invoice_id: "INV-MAX".into(),
                platform_proof: OfflinePlatformProof::AppleAppAttest(AppleAppAttestProof {
                    key_id: BASE64_STANDARD.encode(b"apple"),
                    counter: 1,
                    assertion: vec![],
                    challenge_hash: Hash::new(b"challenge"),
                }),
                platform_snapshot: None,
                sender_certificate_id: certificate.certificate_id(),
                sender_signature: Signature::from_bytes(&[0; 64]),
                build_claim: None,
            };
            let transfer = OfflineToOnlineTransfer {
                bundle_id: Hash::new(b"bundle-max"),
                receiver,
                deposit_account,
                receipts: vec![receipt],
                balance_proof: OfflineBalanceProof {
                    initial_commitment: certificate.allowance.clone(),
                    resulting_commitment: vec![0; 32],
                    claimed_delta: Numeric::new(150, 0),
                    zk_proof: None,
                },
                balance_proofs: None,
                aggregate_proof: None,
                attachments: None,
                platform_snapshot: None,
            };
            let record = OfflineAllowanceRecord {
                certificate,
                current_commitment: vec![0; 32],
                registered_at_ms: 1_700_000_001,
                remaining_amount: Numeric::new(1_000, 0),
                counter_state: OfflineCounterState::default(),
                verdict_id: None,
                attestation_nonce: None,
                refresh_at_ms: None,
            };

            let err = super::ensure_receipt_targets(&transfer.receipts, &transfer, &record)
                .expect_err("receipt over max_tx_value should be rejected");
            let expected = format!(
                "{OFFLINE_REJECTION_REASON_PREFIX}{}",
                OfflineTransferRejectionReason::MaxTxValueExceeded.as_label()
            );
            assert!(err.to_string().contains(&expected));
        }

        #[test]
        fn receipt_amount_fractional_is_rejected() {
            let certificate = sample_certificate();
            let receiver = sample_account(0x02);
            let receipt = OfflineSpendReceipt {
                tx_id: Hash::new(b"receipt-frac"),
                from: certificate.controller.clone(),
                to: receiver,
                asset: certificate.allowance.asset.clone(),
                amount: Numeric::new(5, 1),
                issued_at_ms: 1_700_000_100,
                invoice_id: "INV-FRAC".into(),
                platform_proof: OfflinePlatformProof::AppleAppAttest(AppleAppAttestProof {
                    key_id: BASE64_STANDARD.encode(b"apple"),
                    counter: 1,
                    assertion: vec![],
                    challenge_hash: Hash::new(b"challenge"),
                }),
                platform_snapshot: None,
                sender_certificate_id: certificate.certificate_id(),
                sender_signature: Signature::from_bytes(&[0; 64]),
                build_claim: None,
            };

            let err = super::aggregate_amount(&[receipt], certificate.allowance.amount.scale())
                .expect_err("fractional receipt should be rejected");
            let expected = format!(
                "{OFFLINE_REJECTION_REASON_PREFIX}{}",
                OfflineTransferRejectionReason::InvalidReceiptAmount.as_label()
            );
            assert!(err.to_string().contains(&expected));
        }

        #[test]
        fn certificate_policy_rejects_fractional_values() {
            let mut certificate = sample_certificate();
            certificate.policy.max_balance = Numeric::new(1_000, 1);
            assert!(
                ensure_certificate_policy(&certificate, certificate.allowance.amount.scale())
                    .is_err()
            );

            let mut certificate = sample_certificate();
            certificate.policy.max_tx_value = Numeric::new(250, 1);
            assert!(
                ensure_certificate_policy(&certificate, certificate.allowance.amount.scale())
                    .is_err()
            );

            let mut certificate = sample_certificate();
            certificate.allowance.amount = Numeric::new(1_000, 1);
            assert!(
                ensure_certificate_policy(&certificate, certificate.allowance.amount.scale())
                    .is_err()
            );
        }

        #[test]
        fn receipt_timestamp_accepts_valid_window() {
            let issued_at_ms = 1_700_000_100;
            let (transfer, record) = sample_transfer_with_receipt_timestamp(issued_at_ms);
            let block_timestamp_ms = record.certificate.issued_at_ms + 1_000;
            let cfg = actual::Offline::default();
            ensure_receipt_timestamps(&transfer.receipts, &record, block_timestamp_ms, &cfg)
                .expect("receipt timestamp should be valid");
        }

        #[test]
        fn receipt_timestamp_rejects_before_certificate() {
            let issued_at_ms = 1_699_999_999;
            let (transfer, record) = sample_transfer_with_receipt_timestamp(issued_at_ms);
            let block_timestamp_ms = record.certificate.issued_at_ms + 1_000;
            let cfg = actual::Offline::default();
            let err =
                ensure_receipt_timestamps(&transfer.receipts, &record, block_timestamp_ms, &cfg)
                    .expect_err("receipt timestamp should be rejected");
            let expected = format!(
                "{OFFLINE_REJECTION_REASON_PREFIX}{}",
                OfflineTransferRejectionReason::ReceiptTimestampInvalid.as_label()
            );
            assert!(err.to_string().contains(&expected));
        }

        #[test]
        fn receipt_timestamp_rejects_expired() {
            let issued_at_ms = 1_700_000_100;
            let (transfer, record) = sample_transfer_with_receipt_timestamp(issued_at_ms);
            let block_timestamp_ms = issued_at_ms + 100;
            let cfg = actual::Offline {
                max_receipt_age: Duration::from_millis(50),
                ..Default::default()
            };
            let err =
                ensure_receipt_timestamps(&transfer.receipts, &record, block_timestamp_ms, &cfg)
                    .expect_err("receipt should be expired");
            let expected = format!(
                "{OFFLINE_REJECTION_REASON_PREFIX}{}",
                OfflineTransferRejectionReason::ReceiptExpired.as_label()
            );
            assert!(err.to_string().contains(&expected));
        }

        #[test]
        fn verdict_helper_detects_duplicates() {
            let verdict = Hash::new(b"duplicate");
            let record = record_with_verdict(Some(verdict));
            assert!(verdict_id_in_use([&record], &verdict));

            let other = Hash::new(b"other");
            assert!(!verdict_id_in_use([&record], &other));
        }

        #[test]
        fn platform_snapshot_captures_play_integrity_payload() {
            use std::collections::BTreeSet;

            use base64::Engine as _;

            let mut certificate = sample_certificate();
            certificate.attestation_report = b"play-token".to_vec();
            let play_metadata = AndroidPlayIntegrityMetadata {
                cloud_project_number: 42,
                environment: PlayIntegrityEnvironment::Production,
                package_names: BTreeSet::from(["com.example.app".to_string()]),
                signing_digests_sha256: BTreeSet::from([vec![0xAB; 32]]),
                allowed_app_verdicts: BTreeSet::from([PlayIntegrityAppVerdict::PlayRecognized]),
                allowed_device_verdicts: BTreeSet::from([PlayIntegrityDeviceVerdict::Strong]),
                max_token_age_ms: Some(60_000),
            };
            let snapshot = super::derive_platform_token_snapshot(
                &certificate,
                Some(&AndroidIntegrityMetadata::PlayIntegrity(play_metadata)),
            )
            .expect("play snapshot");
            assert_eq!(
                snapshot.policy_label(),
                AndroidIntegrityPolicy::PlayIntegrity.as_str()
            );
            assert_eq!(
                snapshot.attestation_jws_b64(),
                BASE64_STANDARD.encode(b"play-token")
            );
        }

        #[test]
        fn platform_snapshot_captures_hms_payload() {
            use std::collections::BTreeSet;

            use base64::Engine as _;

            let mut certificate = sample_certificate();
            certificate.attestation_report = b"hms-token".to_vec();
            let hms_metadata = AndroidHmsSafetyDetectMetadata {
                app_id: "123456789".into(),
                package_names: BTreeSet::from(["com.hms.app".to_string()]),
                signing_digests_sha256: BTreeSet::from([vec![0xCD; 32]]),
                required_evaluations: BTreeSet::from([HmsSafetyDetectEvaluation::StrongIntegrity]),
                max_token_age_ms: Some(120_000),
            };
            let snapshot = super::derive_platform_token_snapshot(
                &certificate,
                Some(&AndroidIntegrityMetadata::HmsSafetyDetect(hms_metadata)),
            )
            .expect("hms snapshot");
            assert_eq!(
                snapshot.policy_label(),
                AndroidIntegrityPolicy::HmsSafetyDetect.as_str()
            );
            assert_eq!(
                snapshot.attestation_jws_b64(),
                BASE64_STANDARD.encode(b"hms-token")
            );
        }

        #[test]
        fn platform_snapshot_absent_for_marker_or_empty_report() {
            use std::collections::BTreeSet;

            let mut certificate = sample_certificate();
            certificate.attestation_report.clear();
            let play_metadata =
                AndroidIntegrityMetadata::PlayIntegrity(AndroidPlayIntegrityMetadata {
                    cloud_project_number: 7,
                    environment: PlayIntegrityEnvironment::Production,
                    package_names: BTreeSet::from(["com.example.app".to_string()]),
                    signing_digests_sha256: BTreeSet::from([vec![0xEF; 32]]),
                    allowed_app_verdicts: BTreeSet::from([PlayIntegrityAppVerdict::PlayRecognized]),
                    allowed_device_verdicts: BTreeSet::from([PlayIntegrityDeviceVerdict::Strong]),
                    max_token_age_ms: None,
                });
            assert!(
                super::derive_platform_token_snapshot(&certificate, Some(&play_metadata)).is_none()
            );

            certificate.attestation_report = b"marker".to_vec();
            let marker_metadata = AndroidIntegrityMetadata::MarkerKey(AndroidMarkerKeyMetadata {
                package_names: BTreeSet::from(["com.example.app".to_string()]),
                signing_digests_sha256: BTreeSet::from([vec![0xAA; 32]]),
                require_strongbox: true,
                require_rollback_resistance: true,
            });
            assert!(
                super::derive_platform_token_snapshot(&certificate, Some(&marker_metadata))
                    .is_none()
            );
        }

        #[test]
        fn submitted_platform_snapshot_requires_matching_metadata() {
            use std::collections::BTreeSet;

            let snapshot = OfflinePlatformTokenSnapshot {
                policy: AndroidIntegrityPolicy::PlayIntegrity.as_str().into(),
                attestation_jws_b64: "token".into(),
            };
            let metadata =
                AndroidIntegrityMetadata::HmsSafetyDetect(AndroidHmsSafetyDetectMetadata {
                    app_id: "123".into(),
                    package_names: BTreeSet::from(["com.example.hms".to_string()]),
                    signing_digests_sha256: BTreeSet::from([vec![0xAA; 32]]),
                    required_evaluations: BTreeSet::from([
                        HmsSafetyDetectEvaluation::BasicIntegrity,
                    ]),
                    max_token_age_ms: Some(30_000),
                });
            assert!(
                super::validate_platform_snapshot(Some(&snapshot), Some(&metadata)).is_err(),
                "mismatched policies must be rejected"
            );
        }

        #[test]
        fn submitted_platform_snapshot_allowed_for_matching_policy() {
            use std::collections::BTreeSet;

            let snapshot = OfflinePlatformTokenSnapshot {
                policy: AndroidIntegrityPolicy::PlayIntegrity.as_str().into(),
                attestation_jws_b64: "token".into(),
            };
            let metadata = AndroidIntegrityMetadata::PlayIntegrity(AndroidPlayIntegrityMetadata {
                cloud_project_number: 42,
                environment: PlayIntegrityEnvironment::Production,
                package_names: BTreeSet::from(["com.example.app".to_string()]),
                signing_digests_sha256: BTreeSet::from([vec![0xCD; 32]]),
                allowed_app_verdicts: BTreeSet::from([PlayIntegrityAppVerdict::PlayRecognized]),
                allowed_device_verdicts: BTreeSet::from([PlayIntegrityDeviceVerdict::Strong]),
                max_token_age_ms: Some(60_000),
            });
            let result =
                super::validate_platform_snapshot(Some(&snapshot), Some(&metadata)).unwrap();
            assert!(result.is_some());
        }

        #[test]
        fn receipt_snapshot_preferred_over_transfer_snapshot() {
            let receipt_snapshot = OfflinePlatformTokenSnapshot {
                policy: AndroidIntegrityPolicy::PlayIntegrity.as_str().into(),
                attestation_jws_b64: "receipt-token".into(),
            };
            let transfer_snapshot = OfflinePlatformTokenSnapshot {
                policy: AndroidIntegrityPolicy::HmsSafetyDetect.as_str().into(),
                attestation_jws_b64: "bundle-token".into(),
            };
            let resolved = super::select_attestation_snapshot(
                Some(&receipt_snapshot),
                Some(&transfer_snapshot),
            )
            .expect("snapshot");
            assert_eq!(resolved.attestation_jws_b64(), "receipt-token");
        }

        #[test]
        fn attestation_snapshot_falls_back_to_bundle_submission() {
            let transfer_snapshot = OfflinePlatformTokenSnapshot {
                policy: AndroidIntegrityPolicy::PlayIntegrity.as_str().into(),
                attestation_jws_b64: "bundle-token".into(),
            };
            let resolved = super::select_attestation_snapshot(None, Some(&transfer_snapshot))
                .expect("snapshot");
            assert_eq!(resolved.attestation_jws_b64(), "bundle-token");
        }

        #[test]
        fn refresh_before_issued_is_rejected() {
            let mut certificate = sample_certificate();
            certificate.refresh_at_ms = Some(certificate.issued_at_ms);
            assert!(
                ensure_certificate_policy(&certificate, certificate.allowance.amount.scale())
                    .is_err()
            );
        }

        #[test]
        fn refresh_within_window_is_allowed() {
            let mut certificate = sample_certificate();
            certificate.refresh_at_ms = Some(certificate.issued_at_ms + 10_000);
            assert!(
                ensure_certificate_policy(&certificate, certificate.allowance.amount.scale())
                    .is_ok()
            );
        }

        #[test]
        fn refresh_after_expiry_is_rejected() {
            let mut certificate = sample_certificate();
            certificate.refresh_at_ms = Some(certificate.expires_at_ms + 1);
            assert!(
                ensure_certificate_policy(&certificate, certificate.allowance.amount.scale())
                    .is_err()
            );
        }

        #[test]
        fn register_allowance_rejects_future_certificate() {
            let mut certificate = sample_certificate();
            certificate.issued_at_ms += 1_000;
            certificate.expires_at_ms = certificate.issued_at_ms + 60_000;
            certificate.policy.expires_at_ms = certificate.expires_at_ms;
            let operator_keys = KeyPair::from_seed(vec![0x01; 32], Algorithm::Ed25519);
            certificate.operator_signature = Signature::new(
                operator_keys.private_key(),
                &certificate
                    .operator_signing_bytes()
                    .expect("certificate signing bytes"),
            );

            let controller = certificate.controller.clone();
            let escrow = sample_account(0x02);
            let definition_id = certificate.allowance.asset.definition().clone();
            let domain_id = offline_domain_id();
            let domain = Domain::new(domain_id.clone()).build(&controller);
            let controller_account =
                Account::new(controller.clone().to_account_id(domain_id.clone()))
                    .build(&controller);
            let escrow_account =
                Account::new(escrow.clone().to_account_id(domain_id)).build(&escrow);
            let asset_definition = {
                let __asset_definition_id = definition_id.clone();
                AssetDefinition::new(__asset_definition_id.clone(), NumericSpec::integer())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .build(&controller);
            let controller_asset_entry = Asset::new(
                AssetId::new(definition_id.clone(), controller.clone()),
                Numeric::new(50, 0),
            );
            let escrow_asset_entry = Asset::new(
                AssetId::new(definition_id.clone(), escrow.clone()),
                Numeric::new(50, 0),
            );
            let world = World::with_assets(
                [domain],
                [controller_account, escrow_account],
                [asset_definition],
                [controller_asset_entry, escrow_asset_entry],
                [],
            );

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new(world, Arc::clone(&kura), query);
            state.settlement.offline.escrow_required = true;
            state
                .settlement
                .offline
                .escrow_accounts
                .insert(definition_id, escrow);

            let block_timestamp_ms = certificate.issued_at_ms - 1;
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, block_timestamp_ms, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            transaction
                .world
                .deposit_numeric_asset(&certificate.allowance.asset, &certificate.allowance.amount)
                .expect("prefund allowance asset");

            let err = register_allowance(
                RegisterOfflineAllowance {
                    certificate: certificate.clone(),
                },
                &controller,
                &mut transaction,
            )
            .expect_err("future-issued certificate should be rejected");
            let expected = format!("{OFFLINE_REJECTION_REASON_PREFIX}issued_at_in_future");
            assert!(err.to_string().contains(&expected));
        }

        #[test]
        fn register_allowance_rejects_expired_certificate() {
            let mut certificate = sample_certificate();
            certificate.expires_at_ms = certificate.issued_at_ms + 1;
            certificate.policy.expires_at_ms = certificate.expires_at_ms;
            let operator_keys = KeyPair::from_seed(vec![0x01; 32], Algorithm::Ed25519);
            certificate.operator_signature = Signature::new(
                operator_keys.private_key(),
                &certificate
                    .operator_signing_bytes()
                    .expect("certificate signing bytes"),
            );

            let controller = certificate.controller.clone();
            let escrow = sample_account(0x02);
            let definition_id = certificate.allowance.asset.definition().clone();
            let domain_id = offline_domain_id();
            let domain = Domain::new(domain_id.clone()).build(&controller);
            let controller_account =
                Account::new(controller.clone().to_account_id(domain_id.clone()))
                    .build(&controller);
            let escrow_account =
                Account::new(escrow.clone().to_account_id(domain_id)).build(&escrow);
            let asset_definition = {
                let __asset_definition_id = definition_id.clone();
                AssetDefinition::new(__asset_definition_id.clone(), NumericSpec::integer())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .build(&controller);
            let world = World::with(
                [domain],
                [controller_account, escrow_account],
                [asset_definition],
            );

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new(world, Arc::clone(&kura), query);
            state.settlement.offline.escrow_required = true;
            state
                .settlement
                .offline
                .escrow_accounts
                .insert(definition_id, escrow);

            let block_timestamp_ms = certificate.expires_at_ms;
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, block_timestamp_ms, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            transaction
                .world
                .deposit_numeric_asset(&certificate.allowance.asset, &certificate.allowance.amount)
                .expect("prefund allowance asset");

            let err = register_allowance(
                RegisterOfflineAllowance {
                    certificate: certificate.clone(),
                },
                &controller,
                &mut transaction,
            )
            .expect_err("expired certificate should be rejected");
            let expected = format!(
                "{OFFLINE_REJECTION_REASON_PREFIX}{}",
                OfflineTransferRejectionReason::CertificateExpired.as_label()
            );
            assert!(err.to_string().contains(&expected));
        }

        #[test]
        fn verdict_refresh_expired_is_rejected() {
            let mut record = record_with_verdict(None);
            record.refresh_at_ms = Some(record.registered_at_ms + 5_000);
            let block_timestamp = record.registered_at_ms + 6_000;
            assert!(enforce_verdict_refresh_window(&record, block_timestamp).is_err());
        }

        #[test]
        fn verdict_refresh_future_is_allowed() {
            let mut record = record_with_verdict(None);
            record.refresh_at_ms = Some(record.registered_at_ms + 10_000);
            let block_timestamp = record.registered_at_ms + 5_000;
            assert!(enforce_verdict_refresh_window(&record, block_timestamp).is_ok());
        }

        #[test]
        fn register_verdict_revocation_persists_record() {
            let verdict_id = Hash::new(b"revocation-test");
            let record = record_with_verdict(Some(verdict_id));
            let controller = record.certificate.controller.clone();
            let certificate_id = record.certificate.certificate_id();

            let mut world = World::default();
            world
                .offline_allowances
                .insert(certificate_id, record.clone());

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new(world, Arc::clone(&kura), query);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let revocation = OfflineVerdictRevocation {
                verdict_id,
                issuer: controller.clone(),
                revoked_at_ms: 0,
                reason: OfflineVerdictRevocationReason::DeviceCompromised,
                note: Some("device lost".into()),
                metadata: Metadata::default(),
            };
            register_verdict_revocation(
                RegisterOfflineVerdictRevocation { revocation },
                &controller,
                &mut transaction,
            )
            .expect("revocation should succeed");

            let stored = transaction
                .world
                .offline_verdict_revocations
                .get(&verdict_id)
                .expect("revocation stored");
            assert_eq!(
                stored.reason,
                OfflineVerdictRevocationReason::DeviceCompromised
            );
            assert_eq!(stored.note.as_deref(), Some("device lost"));
            assert_eq!(stored.issuer, controller);
            assert_eq!(stored.revoked_at_ms, transaction.block_unix_timestamp_ms());
        }

        #[test]
        fn register_verdict_revocation_emits_event() {
            let verdict_id = Hash::new(b"revocation-event");
            let record = record_with_verdict(Some(verdict_id));
            let controller = record.certificate.controller.clone();
            let certificate_id = record.certificate.certificate_id();

            let mut world = World::default();
            world
                .offline_allowances
                .insert(certificate_id, record.clone());

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new(world, Arc::clone(&kura), query);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let revocation = OfflineVerdictRevocation {
                verdict_id,
                issuer: controller.clone(),
                revoked_at_ms: 0,
                reason: OfflineVerdictRevocationReason::PolicyViolation,
                note: Some("revocation for testing".into()),
                metadata: Metadata::default(),
            };
            register_verdict_revocation(
                RegisterOfflineVerdictRevocation { revocation },
                &controller,
                &mut transaction,
            )
            .expect("revocation should succeed");

            let emitted = transaction
                .world
                .internal_event_buf
                .iter()
                .find_map(|event| match event.as_ref() {
                    iroha_data_model::events::data::DataEvent::Offline(
                        OfflineTransferEvent::RevocationImported(payload),
                    ) => Some(payload.clone()),
                    _ => None,
                })
                .expect("revocation event should be emitted");

            assert_eq!(emitted.verdict_id, verdict_id);
            assert_eq!(emitted.issuer, controller);
            assert_eq!(emitted.revoked_at_ms, transaction.block_unix_timestamp_ms());
            assert_eq!(
                emitted.reason,
                OfflineVerdictRevocationReason::PolicyViolation
            );
            assert_eq!(emitted.note.as_deref(), Some("revocation for testing"));
        }

        #[test]
        fn find_transfers_by_policy_matches_snapshot() {
            let mut world = World::default();
            let play = sample_transfer_record_with_policy(
                b"bundle-play",
                AndroidIntegrityPolicy::PlayIntegrity,
            );
            let hms = sample_transfer_record_with_policy(
                b"bundle-hms",
                AndroidIntegrityPolicy::HmsSafetyDetect,
            );
            world
                .offline_to_online_transfers
                .insert(play.transfer.bundle_id, play.clone());
            world
                .offline_to_online_transfers
                .insert(hms.transfer.bundle_id, hms);

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new(world, Arc::clone(&kura), query);
            let state_view = state.view();
            let results: Vec<_> = FindOfflineToOnlineTransfersByPolicy {
                policy: AndroidIntegrityPolicy::PlayIntegrity,
            }
            .execute(CompoundPredicate::PASS, &state_view)
            .expect("query")
            .collect();
            assert_eq!(results.len(), 1);
            let snapshot = results[0].platform_snapshot.as_ref().expect("snapshot");
            assert_eq!(
                snapshot.policy(),
                Some(AndroidIntegrityPolicy::PlayIntegrity)
            );
        }

        #[test]
        fn submit_transfer_rejects_delta_mismatch() {
            let issued_at_ms = 1_700_000_100;
            let (mut transfer, mut record) = sample_transfer_with_receipt_timestamp(issued_at_ms);
            record
                .current_commitment
                .clone_from(&record.certificate.allowance.commitment);
            transfer.balance_proof.claimed_delta = Numeric::new(200, 0);

            let controller = record.certificate.controller.clone();
            let domain_id = offline_domain_id();
            let domain = Domain::new(domain_id.clone()).build(&controller);
            let account =
                Account::new(controller.clone().to_account_id(domain_id)).build(&controller);
            let definition_id = record.certificate.allowance.asset.definition().clone();
            let asset_definition = {
                let __asset_definition_id = definition_id;
                AssetDefinition::new(__asset_definition_id.clone(), NumericSpec::integer())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .build(&controller);
            let mut world = World::with([domain], [account], [asset_definition]);
            let certificate_id = record.certificate.certificate_id();
            world.offline_allowances.insert(certificate_id, record);

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new(world, Arc::clone(&kura), query);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, issued_at_ms + 100, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let authority = transfer.receiver.clone();
            let err = submit_transfer(
                SubmitOfflineToOnlineTransfer { transfer },
                &authority,
                &mut transaction,
            )
            .expect_err("delta mismatch should be rejected");
            let expected = format!(
                "{OFFLINE_REJECTION_REASON_PREFIX}{}",
                OfflineTransferRejectionReason::DeltaMismatch.as_label()
            );
            assert!(err.to_string().contains(&expected));
        }

        #[test]
        fn submit_transfer_rejects_missing_per_certificate_balance_proof() {
            let issued_at_ms = 1_700_000_100;
            let (mut transfer, mut record) = sample_transfer_with_receipt_timestamp(issued_at_ms);
            record
                .current_commitment
                .clone_from(&record.certificate.allowance.commitment);
            let primary_certificate_id = record.certificate.certificate_id();

            let mut secondary_record = record.clone();
            secondary_record.certificate.allowance.commitment = vec![0xAA; 32];
            secondary_record.current_commitment =
                secondary_record.certificate.allowance.commitment.clone();
            let secondary_certificate_id = secondary_record.certificate.certificate_id();

            let mut secondary_receipt = transfer.receipts[0].clone();
            secondary_receipt.tx_id = Hash::new(b"receipt-ts-secondary");
            secondary_receipt.invoice_id = "INV-TS-SECONDARY".into();
            secondary_receipt.sender_certificate_id = secondary_certificate_id;
            if let OfflinePlatformProof::AppleAppAttest(proof) =
                &mut secondary_receipt.platform_proof
            {
                proof.counter = 2;
            }
            transfer.receipts.push(secondary_receipt);
            transfer.balance_proofs = Some(vec![OfflineCertificateBalanceProof::new(
                primary_certificate_id,
                transfer.balance_proof.clone(),
            )]);

            let controller = record.certificate.controller.clone();
            let domain_id = offline_domain_id();
            let domain = Domain::new(domain_id.clone()).build(&controller);
            let account =
                Account::new(controller.clone().to_account_id(domain_id)).build(&controller);
            let definition_id = record.certificate.allowance.asset.definition().clone();
            let asset_definition = {
                let __asset_definition_id = definition_id;
                AssetDefinition::new(__asset_definition_id.clone(), NumericSpec::integer())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .build(&controller);
            let mut world = World::with([domain], [account], [asset_definition]);
            world
                .offline_allowances
                .insert(primary_certificate_id, record);
            world
                .offline_allowances
                .insert(secondary_certificate_id, secondary_record);

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new(world, Arc::clone(&kura), query);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, issued_at_ms + 100, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let authority = transfer.receiver.clone();
            let err = submit_transfer(
                SubmitOfflineToOnlineTransfer { transfer },
                &authority,
                &mut transaction,
            )
            .expect_err("missing per-certificate proof should reject settlement");
            let expected = format!(
                "{OFFLINE_REJECTION_REASON_PREFIX}{}",
                OfflineTransferRejectionReason::BalanceProofInvalid.as_label()
            );
            let rendered = err.to_string();
            assert!(
                rendered.contains(&expected)
                    || rendered.contains("missing per-certificate balance proof"),
                "unexpected error: {rendered}"
            );
        }

        #[test]
        fn submit_transfer_rejects_duplicate_bundle_without_mutating_allowance() {
            let issued_at_ms = 1_700_000_100;
            let (transfer, mut record) = sample_transfer_with_receipt_timestamp(issued_at_ms);
            record
                .current_commitment
                .clone_from(&record.certificate.allowance.commitment);

            let controller = record.certificate.controller.clone();
            let domain_id = offline_domain_id();
            let domain = Domain::new(domain_id.clone()).build(&controller);
            let account =
                Account::new(controller.clone().to_account_id(domain_id)).build(&controller);
            let definition_id = record.certificate.allowance.asset.definition().clone();
            let asset_definition = {
                let __asset_definition_id = definition_id;
                AssetDefinition::new(__asset_definition_id.clone(), NumericSpec::integer())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .build(&controller);
            let mut world = World::with([domain], [account], [asset_definition]);
            let certificate_id = record.certificate.certificate_id();
            world.offline_allowances.insert(certificate_id, record);
            world.offline_to_online_transfers.insert(
                transfer.bundle_id,
                OfflineTransferRecord {
                    transfer: transfer.clone(),
                    controller: controller.clone(),
                    status: OfflineTransferStatus::Settled,
                    rejection_reason: None,
                    recorded_at_ms: issued_at_ms,
                    recorded_at_height: 1,
                    archived_at_height: None,
                    history: Vec::new(),
                    pos_verdict_snapshots: Vec::new(),
                    verdict_snapshot: None,
                    platform_snapshot: None,
                },
            );

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new(world, Arc::clone(&kura), query);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, issued_at_ms + 100, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let authority = transfer.receiver.clone();
            let err = submit_transfer(
                SubmitOfflineToOnlineTransfer {
                    transfer: transfer.clone(),
                },
                &authority,
                &mut transaction,
            )
            .expect_err("duplicate bundle should reject settlement");
            let expected = format!(
                "{OFFLINE_REJECTION_REASON_PREFIX}{}",
                OfflineTransferRejectionReason::DuplicateBundle.as_label()
            );
            assert!(err.to_string().contains(&expected));

            let stored = transaction
                .world
                .offline_allowances
                .get(&certificate_id)
                .expect("allowance should remain stored");
            assert_eq!(
                stored.remaining_amount,
                Numeric::new(1_000, 0),
                "remaining amount must not change on duplicate bundle rejection"
            );
            assert_eq!(
                transaction.world.offline_to_online_transfers.len(),
                1,
                "duplicate rejection must not append a second transfer record"
            );
        }

        #[test]
        fn execute_submit_transfer_persists_rejected_record_for_offline_rejection() {
            let issued_at_ms = 1_700_000_100;
            let (mut transfer, mut record) = sample_transfer_with_receipt_timestamp(issued_at_ms);
            record
                .current_commitment
                .clone_from(&record.certificate.allowance.commitment);
            transfer.balance_proof.claimed_delta = Numeric::new(200, 0);

            let controller = record.certificate.controller.clone();
            let domain_id = offline_domain_id();
            let domain = Domain::new(domain_id.clone()).build(&controller);
            let account =
                Account::new(controller.clone().to_account_id(domain_id)).build(&controller);
            let definition_id = record.certificate.allowance.asset.definition().clone();
            let asset_definition = {
                let __asset_definition_id = definition_id.clone();
                AssetDefinition::new(__asset_definition_id.clone(), NumericSpec::integer())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .build(&controller);
            let mut world = World::with([domain], [account], [asset_definition]);
            let certificate_id = record.certificate.certificate_id();
            world.offline_allowances.insert(certificate_id, record);

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new(world, Arc::clone(&kura), query);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, issued_at_ms + 100, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let authority = transfer.receiver.clone();
            SubmitOfflineToOnlineTransfer {
                transfer: transfer.clone(),
            }
            .execute(&authority, &mut transaction)
            .expect("offline rejection should be persisted as rejected record");

            let stored = transaction
                .world
                .offline_to_online_transfers
                .get(&transfer.bundle_id)
                .expect("rejected transfer row should be stored");
            assert_eq!(stored.status, OfflineTransferStatus::Rejected);
            assert_eq!(
                stored.rejection_reason.as_deref(),
                Some(OfflineTransferRejectionReason::DeltaMismatch.as_label())
            );
            assert_eq!(stored.history.len(), 1);
            assert_eq!(stored.history[0].status, OfflineTransferStatus::Rejected);

            let allowance = transaction
                .world
                .offline_allowances
                .get(&certificate_id)
                .expect("allowance should remain stored");
            assert_eq!(
                allowance.remaining_amount,
                Numeric::new(1_000, 0),
                "remaining amount must not change on rejected settlement"
            );

            let deposit_asset = AssetId::new(definition_id, transfer.deposit_account.clone());
            assert!(
                transaction.world.assets.get(&deposit_asset).is_none(),
                "deposit account must not be credited on rejected settlement"
            );
        }

        #[test]
        fn execute_submit_transfer_preserves_duplicate_bundle_error() {
            let issued_at_ms = 1_700_000_100;
            let (transfer, mut record) = sample_transfer_with_receipt_timestamp(issued_at_ms);
            record
                .current_commitment
                .clone_from(&record.certificate.allowance.commitment);

            let controller = record.certificate.controller.clone();
            let domain_id = offline_domain_id();
            let domain = Domain::new(domain_id.clone()).build(&controller);
            let account =
                Account::new(controller.clone().to_account_id(domain_id)).build(&controller);
            let definition_id = record.certificate.allowance.asset.definition().clone();
            let asset_definition = {
                let __asset_definition_id = definition_id;
                AssetDefinition::new(__asset_definition_id.clone(), NumericSpec::integer())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .build(&controller);
            let mut world = World::with([domain], [account], [asset_definition]);
            let certificate_id = record.certificate.certificate_id();
            world.offline_allowances.insert(certificate_id, record);
            world.offline_to_online_transfers.insert(
                transfer.bundle_id,
                OfflineTransferRecord {
                    transfer: transfer.clone(),
                    controller: controller.clone(),
                    status: OfflineTransferStatus::Settled,
                    rejection_reason: None,
                    recorded_at_ms: issued_at_ms,
                    recorded_at_height: 1,
                    archived_at_height: None,
                    history: Vec::new(),
                    pos_verdict_snapshots: Vec::new(),
                    verdict_snapshot: None,
                    platform_snapshot: None,
                },
            );

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new(world, Arc::clone(&kura), query);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, issued_at_ms + 100, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let authority = transfer.receiver.clone();
            let err = SubmitOfflineToOnlineTransfer {
                transfer: transfer.clone(),
            }
            .execute(&authority, &mut transaction)
            .expect_err("duplicate bundle should remain a submit-time error");
            let expected = format!(
                "{OFFLINE_REJECTION_REASON_PREFIX}{}",
                OfflineTransferRejectionReason::DuplicateBundle.as_label()
            );
            assert!(err.to_string().contains(&expected));
            assert_eq!(
                transaction.world.offline_to_online_transfers.len(),
                1,
                "duplicate rejection must not append a second transfer record"
            );
        }

        #[test]
        fn submit_transfer_rejects_replayed_build_claim_even_without_transfer_history() {
            let issued_at_ms = 1_700_000_100;
            let (mut transfer, mut record) = sample_transfer_with_receipt_timestamp(issued_at_ms);
            record
                .current_commitment
                .clone_from(&record.certificate.allowance.commitment);

            let claim_id = Hash::new(b"claim-replay-pruned");
            attach_android_provisioned_build_claim(
                transfer.receipts.get_mut(0).expect("receipt"),
                &mut record.certificate,
                claim_id,
            );

            let controller = record.certificate.controller.clone();
            let domain_id = offline_domain_id();
            let domain = Domain::new(domain_id.clone()).build(&controller);
            let account =
                Account::new(controller.clone().to_account_id(domain_id)).build(&controller);
            let definition_id = record.certificate.allowance.asset.definition().clone();
            let asset_definition = {
                let __asset_definition_id = definition_id;
                AssetDefinition::new(__asset_definition_id.clone(), NumericSpec::integer())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .build(&controller);
            let mut world = World::with([domain], [account], [asset_definition]);
            let certificate_id = record.certificate.certificate_id();
            world.offline_allowances.insert(certificate_id, record);
            world.offline_consumed_build_claim_ids.insert(claim_id, ());

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new(world, Arc::clone(&kura), query);
            state.settlement.offline.skip_platform_attestation = true;
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, issued_at_ms + 100, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let authority = transfer.receiver.clone();
            let err = submit_transfer(
                SubmitOfflineToOnlineTransfer { transfer },
                &authority,
                &mut transaction,
            )
            .expect_err("replayed build claim should be rejected");
            let expected = format!(
                "{OFFLINE_REJECTION_REASON_PREFIX}{}",
                OfflineTransferRejectionReason::BuildClaimReplayed.as_label()
            );
            assert!(
                err.to_string().contains(&expected),
                "unexpected rejection: {err}"
            );
        }

        #[test]
        fn submit_transfer_rejects_insufficient_escrow_without_mutating_allowance_or_credit() {
            let issued_at_ms = 1_700_000_100;
            let controller = sample_account(0x01);
            let receiver = sample_account(0x02);
            let deposit_account = sample_account(0x03);
            let escrow_account = sample_account(0x04);
            let definition_id =
                AssetDefinitionId::new("offline".parse().unwrap(), "xor".parse().unwrap());
            let asset = AssetId::new(definition_id.clone(), controller.clone());
            let operator_keys = KeyPair::from_seed(vec![0x01; 32], Algorithm::Ed25519);
            let spend_pair = KeyPair::from_seed(vec![0xEE; 32], Algorithm::Ed25519);

            let mut initial_blinding = [0u8; 32];
            initial_blinding[0] = 7;
            let mut resulting_blinding = [0u8; 32];
            resulting_blinding[0] = 9;
            let initial_commitment =
                compute_commitment(&Numeric::new(0, 0), 0, &initial_blinding).expect("commitment");
            let resulting_commitment =
                compute_commitment(&Numeric::new(100, 0), 0, &resulting_blinding)
                    .expect("commitment");

            let mut certificate = OfflineWalletCertificate {
                controller: controller.clone(),
                operator: controller.clone(),
                allowance: OfflineAllowanceCommitment {
                    asset: asset.clone(),
                    amount: Numeric::new(1_000, 0),
                    commitment: initial_commitment.clone(),
                },
                spend_public_key: spend_pair.public_key().clone(),
                attestation_report: Vec::new(),
                issued_at_ms: issued_at_ms - 10,
                expires_at_ms: issued_at_ms + 600_000,
                policy: OfflineWalletPolicy {
                    max_balance: Numeric::new(1_000, 0),
                    max_tx_value: Numeric::new(250, 0),
                    expires_at_ms: issued_at_ms + 600_000,
                },
                operator_signature: Signature::from_bytes(&[0; 64]),
                metadata: Metadata::default(),
                verdict_id: None,
                attestation_nonce: None,
                refresh_at_ms: None,
            };
            certificate.operator_signature = Signature::new(
                operator_keys.private_key(),
                &certificate
                    .operator_signing_bytes()
                    .expect("certificate signing bytes"),
            );

            let mut receipt = OfflineSpendReceipt {
                tx_id: Hash::new(b"insufficient-escrow-receipt"),
                from: controller.clone(),
                to: receiver.clone(),
                asset: asset.clone(),
                amount: Numeric::new(100, 0),
                issued_at_ms,
                invoice_id: "INV-ESCROW".into(),
                platform_proof: OfflinePlatformProof::AppleAppAttest(AppleAppAttestProof {
                    key_id: BASE64_STANDARD.encode(b"apple"),
                    counter: 1,
                    assertion: vec![],
                    challenge_hash: Hash::new(b"challenge"),
                }),
                platform_snapshot: None,
                sender_certificate_id: certificate.certificate_id(),
                sender_signature: Signature::from_bytes(&[0; 64]),
                build_claim: None,
            };
            receipt.sender_signature = Signature::new(
                spend_pair.private_key(),
                &receipt.signing_bytes().expect("receipt signing bytes"),
            );

            let chain_id: ChainId = "testnet".parse().expect("chain id");
            let claimed_delta = Numeric::new(100, 0);
            let zk_proof = build_balance_proof(
                &chain_id,
                0,
                &claimed_delta,
                &Numeric::new(100, 0),
                &initial_commitment,
                &resulting_commitment,
                &initial_blinding,
                &resulting_blinding,
            )
            .expect("balance proof");
            let transfer = OfflineToOnlineTransfer {
                bundle_id: Hash::new(b"insufficient-escrow-bundle"),
                receiver: receiver.clone(),
                deposit_account: deposit_account.clone(),
                receipts: vec![receipt],
                balance_proof: OfflineBalanceProof {
                    initial_commitment: certificate.allowance.clone(),
                    resulting_commitment: resulting_commitment.clone(),
                    claimed_delta: claimed_delta.clone(),
                    zk_proof: Some(zk_proof),
                },
                balance_proofs: None,
                aggregate_proof: None,
                attachments: None,
                platform_snapshot: None,
            };

            let record = OfflineAllowanceRecord {
                certificate: certificate.clone(),
                current_commitment: initial_commitment,
                registered_at_ms: issued_at_ms - 1,
                remaining_amount: Numeric::new(1_000, 0),
                counter_state: OfflineCounterState::default(),
                verdict_id: None,
                attestation_nonce: None,
                refresh_at_ms: None,
            };

            let domain_id = offline_domain_id();
            let domain = Domain::new(domain_id.clone()).build(&controller);
            let controller_account =
                Account::new(controller.clone().to_account_id(domain_id.clone()))
                    .build(&controller);
            let receiver_account =
                Account::new(receiver.to_account_id(domain_id.clone())).build(&controller);
            let deposit = Account::new(deposit_account.clone().to_account_id(domain_id.clone()))
                .build(&controller);
            let escrow =
                Account::new(escrow_account.clone().to_account_id(domain_id)).build(&controller);
            let asset_definition = {
                let __asset_definition_id = definition_id.clone();
                AssetDefinition::new(__asset_definition_id.clone(), NumericSpec::integer())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .build(&controller);
            let mut world = World::with(
                [domain],
                [controller_account, receiver_account, deposit, escrow],
                [asset_definition],
            );
            let certificate_id = certificate.certificate_id();
            world.offline_allowances.insert(certificate_id, record);

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_with_chain(world, Arc::clone(&kura), query, chain_id);
            state.settlement.offline.escrow_required = true;
            state.settlement.offline.skip_platform_attestation = true;
            state.settlement.offline.proof_mode = OfflineProofMode::Optional;
            state
                .settlement
                .offline
                .escrow_accounts
                .insert(definition_id.clone(), escrow_account.clone());

            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, issued_at_ms + 100, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            let escrow_asset = AssetId::new(definition_id.clone(), escrow_account);
            transaction
                .world
                .deposit_numeric_asset(&escrow_asset, &Numeric::new(50, 0))
                .expect("prefund escrow");

            let authority = transfer.receiver.clone();
            submit_transfer(
                SubmitOfflineToOnlineTransfer {
                    transfer: transfer.clone(),
                },
                &authority,
                &mut transaction,
            )
            .expect_err("insufficient escrow should reject settlement");

            let stored = transaction
                .world
                .offline_allowances
                .get(&certificate_id)
                .expect("allowance should remain stored");
            assert_eq!(
                stored.remaining_amount,
                Numeric::new(1_000, 0),
                "remaining amount must not change when settlement credit fails"
            );
            assert!(
                transaction
                    .world
                    .offline_to_online_transfers
                    .get(&transfer.bundle_id)
                    .is_none(),
                "failed settlement must not be stored"
            );
            let deposit_asset = AssetId::new(definition_id, transfer.deposit_account.clone());
            assert!(
                transaction.world.assets.get(&deposit_asset).is_none(),
                "deposit account must not be credited when escrow is insufficient"
            );
        }

        #[test]
        fn submit_transfer_rejects_replayed_counter_without_credit() {
            let issued_at_ms = 1_700_000_100;
            let (transfer, mut record) = sample_transfer_with_receipt_timestamp(issued_at_ms);
            record
                .current_commitment
                .clone_from(&record.certificate.allowance.commitment);
            let replay_scope = canonical_app_attest_key_id(&BASE64_STANDARD.encode(b"apple"))
                .expect("canonical key id");
            record
                .counter_state
                .apple_key_counters
                .insert(replay_scope, 1);

            let controller = record.certificate.controller.clone();
            let domain_id = offline_domain_id();
            let domain = Domain::new(domain_id.clone()).build(&controller);
            let account =
                Account::new(controller.clone().to_account_id(domain_id)).build(&controller);
            let definition_id = record.certificate.allowance.asset.definition().clone();
            let asset_definition = {
                let __asset_definition_id = definition_id;
                AssetDefinition::new(__asset_definition_id.clone(), NumericSpec::integer())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .build(&controller);
            let mut world = World::with([domain], [account], [asset_definition]);
            let certificate_id = record.certificate.certificate_id();
            world.offline_allowances.insert(certificate_id, record);

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new(world, Arc::clone(&kura), query);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, issued_at_ms + 100, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let authority = transfer.receiver.clone();
            let err = submit_transfer(
                SubmitOfflineToOnlineTransfer {
                    transfer: transfer.clone(),
                },
                &authority,
                &mut transaction,
            )
            .expect_err("replayed counter should reject settlement");
            let expected = format!("{OFFLINE_REJECTION_REASON_PREFIX}counter_conflict");
            assert!(err.to_string().contains(&expected));

            let stored = transaction
                .world
                .offline_allowances
                .get(&certificate_id)
                .expect("allowance should remain stored");
            assert_eq!(
                stored.remaining_amount,
                Numeric::new(1_000, 0),
                "remaining amount must not change on counter replay rejection"
            );
            assert!(
                transaction
                    .world
                    .offline_to_online_transfers
                    .get(&transfer.bundle_id)
                    .is_none(),
                "replayed bundle must not be stored"
            );
            assert!(
                transaction
                    .world
                    .assets
                    .iter()
                    .all(|(asset_id, _)| asset_id.account() != &transfer.deposit_account),
                "deposit account must not be credited on replay rejection"
            );
        }

        #[test]
        fn submit_transfer_rejects_revoked_verdict() {
            let issued_at_ms = 1_700_000_100;
            let (transfer, mut record) = sample_transfer_with_receipt_timestamp(issued_at_ms);
            record
                .current_commitment
                .clone_from(&record.certificate.allowance.commitment);
            let verdict_id = Hash::new(b"revoked-verdict");
            record.verdict_id = Some(verdict_id);

            let controller = record.certificate.controller.clone();
            let domain_id = offline_domain_id();
            let domain = Domain::new(domain_id.clone()).build(&controller);
            let account =
                Account::new(controller.clone().to_account_id(domain_id)).build(&controller);
            let definition_id = record.certificate.allowance.asset.definition().clone();
            let asset_definition = {
                let __asset_definition_id = definition_id.clone();
                AssetDefinition::new(__asset_definition_id.clone(), NumericSpec::integer())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .build(&controller);
            let mut world = World::with([domain], [account], [asset_definition]);
            let certificate_id = record.certificate.certificate_id();
            world.offline_allowances.insert(certificate_id, record);
            world.offline_verdict_revocations.insert(
                verdict_id,
                OfflineVerdictRevocation {
                    verdict_id,
                    issuer: controller.clone(),
                    revoked_at_ms: issued_at_ms + 10,
                    reason: OfflineVerdictRevocationReason::PolicyViolation,
                    note: Some("revoked in test".into()),
                    metadata: Metadata::default(),
                },
            );

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new(world, Arc::clone(&kura), query);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, issued_at_ms + 100, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let authority = transfer.receiver.clone();
            let err = submit_transfer(
                SubmitOfflineToOnlineTransfer {
                    transfer: transfer.clone(),
                },
                &authority,
                &mut transaction,
            )
            .expect_err("revoked verdict should reject settlement");
            let expected = format!(
                "{OFFLINE_REJECTION_REASON_PREFIX}{}",
                OfflineTransferRejectionReason::VerdictExpired.as_label()
            );
            assert!(err.to_string().contains(&expected));

            let stored = transaction
                .world
                .offline_allowances
                .get(&certificate_id)
                .expect("allowance should remain stored");
            assert_eq!(
                stored.remaining_amount,
                Numeric::new(1_000, 0),
                "remaining amount must not change on rejection"
            );
            assert!(
                transaction
                    .world
                    .offline_to_online_transfers
                    .get(&transfer.bundle_id)
                    .is_none(),
                "rejected transfer must not be stored"
            );
            assert!(
                transaction
                    .world
                    .assets
                    .iter()
                    .all(|(asset_id, _)| asset_id.account() != &transfer.deposit_account),
                "deposit account must not be credited on rejection"
            );
        }

        #[test]
        fn reclaim_expired_allowance_refunds_controller_and_removes_record() {
            let mut certificate = sample_certificate();
            certificate.allowance.amount = Numeric::new(50, 0);
            certificate.policy.max_balance = Numeric::new(50, 0);
            certificate.policy.max_tx_value = Numeric::new(50, 0);
            certificate.expires_at_ms = certificate.issued_at_ms + 10;
            certificate.policy.expires_at_ms = certificate.expires_at_ms;
            let operator_keys = KeyPair::from_seed(vec![0x01; 32], Algorithm::Ed25519);
            certificate.operator_signature = Signature::new(
                operator_keys.private_key(),
                &certificate
                    .operator_signing_bytes()
                    .expect("certificate signing bytes"),
            );

            let controller = certificate.controller.clone();
            let escrow = sample_account(0x02);
            let definition_id = certificate.allowance.asset.definition().clone();
            let record = OfflineAllowanceRecord {
                certificate: certificate.clone(),
                current_commitment: certificate.allowance.commitment.clone(),
                registered_at_ms: certificate.issued_at_ms,
                remaining_amount: Numeric::new(50, 0),
                counter_state: OfflineCounterState::default(),
                verdict_id: None,
                attestation_nonce: None,
                refresh_at_ms: None,
            };
            let domain_id = offline_domain_id();
            let domain = Domain::new(domain_id.clone()).build(&controller);
            let controller_account =
                Account::new(controller.clone().to_account_id(domain_id.clone()))
                    .build(&controller);
            let escrow_account =
                Account::new(escrow.clone().to_account_id(domain_id)).build(&escrow);
            let asset_definition = {
                let __asset_definition_id = definition_id.clone();
                AssetDefinition::new(__asset_definition_id.clone(), NumericSpec::integer())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .build(&controller);
            let controller_asset_entry = Asset::new(
                AssetId::new(definition_id.clone(), controller.clone()),
                Numeric::new(50, 0),
            );
            let escrow_asset_entry = Asset::new(
                AssetId::new(definition_id.clone(), escrow.clone()),
                Numeric::new(50, 0),
            );
            let mut world = World::with_assets(
                [domain],
                [controller_account, escrow_account],
                [asset_definition],
                [controller_asset_entry, escrow_asset_entry],
                [],
            );
            let certificate_id = certificate.certificate_id();
            world.offline_allowances.insert(certificate_id, record);

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new(world, Arc::clone(&kura), query);
            state.settlement.offline.escrow_required = true;
            state
                .settlement
                .offline
                .escrow_accounts
                .insert(definition_id.clone(), escrow.clone());

            let reclaim_header = BlockHeader::new(
                nonzero!(1_u64),
                None,
                None,
                None,
                certificate.expires_at_ms + 1,
                0,
            );
            let mut reclaim_block = state.block(reclaim_header);
            let mut reclaim_tx = reclaim_block.transaction();
            reclaim_expired_allowance(
                ReclaimExpiredOfflineAllowance { certificate_id },
                &controller,
                &mut reclaim_tx,
            )
            .expect("reclaim should succeed");

            let controller_asset = AssetId::new(definition_id.clone(), controller.clone());
            let controller_balance = reclaim_tx
                .world
                .assets
                .get(&controller_asset)
                .map(|asset| asset.as_ref().clone())
                .unwrap_or_else(Numeric::zero);
            assert_eq!(controller_balance, Numeric::new(100, 0));

            let escrow_asset = AssetId::new(definition_id.clone(), escrow);
            assert!(
                reclaim_tx.world.assets.get(&escrow_asset).is_none(),
                "escrow asset should be removed after full reclaim"
            );
            assert!(
                reclaim_tx
                    .world
                    .offline_allowances
                    .get(&certificate_id)
                    .is_none(),
                "allowance record must be removed after reclaim"
            );

            let emitted = reclaim_tx
                .world
                .internal_event_buf
                .iter()
                .find_map(|event| match event.as_ref() {
                    iroha_data_model::events::data::DataEvent::Offline(
                        OfflineTransferEvent::AllowanceReclaimed(payload),
                    ) => Some(payload.clone()),
                    _ => None,
                })
                .expect("reclaim event should be emitted");
            assert_eq!(emitted.certificate_id, certificate_id);
            assert_eq!(emitted.controller, controller);
            assert_eq!(emitted.asset_definition, definition_id);
            assert_eq!(emitted.amount, Numeric::new(50, 0));

            let second_err = reclaim_expired_allowance(
                ReclaimExpiredOfflineAllowance { certificate_id },
                &controller,
                &mut reclaim_tx,
            )
            .expect_err("reclaim must fail after allowance is removed");
            assert!(
                second_err.to_string().contains("allowance_not_found"),
                "unexpected error: {second_err}"
            );
        }

        #[test]
        fn reclaim_expired_allowance_rejects_before_expiry() {
            let mut certificate = sample_certificate();
            certificate.allowance.amount = Numeric::new(50, 0);
            certificate.policy.max_balance = Numeric::new(50, 0);
            certificate.policy.max_tx_value = Numeric::new(50, 0);
            let controller = certificate.controller.clone();
            let escrow = sample_account(0x02);
            let definition_id = certificate.allowance.asset.definition().clone();

            let record = OfflineAllowanceRecord {
                certificate: certificate.clone(),
                current_commitment: certificate.allowance.commitment.clone(),
                registered_at_ms: certificate.issued_at_ms,
                remaining_amount: Numeric::new(50, 0),
                counter_state: OfflineCounterState::default(),
                verdict_id: None,
                attestation_nonce: None,
                refresh_at_ms: None,
            };

            let domain_id = offline_domain_id();
            let domain = Domain::new(domain_id.clone()).build(&controller);
            let controller_account =
                Account::new(controller.clone().to_account_id(domain_id.clone()))
                    .build(&controller);
            let escrow_account =
                Account::new(escrow.clone().to_account_id(domain_id)).build(&escrow);
            let asset_definition = {
                let __asset_definition_id = definition_id.clone();
                AssetDefinition::new(__asset_definition_id.clone(), NumericSpec::integer())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .build(&controller);
            let escrow_asset = Asset::new(
                AssetId::new(definition_id.clone(), escrow.clone()),
                Numeric::new(50, 0),
            );
            let mut world = World::with_assets(
                [domain],
                [controller_account, escrow_account],
                [asset_definition],
                [escrow_asset],
                [],
            );
            let certificate_id = certificate.certificate_id();
            world.offline_allowances.insert(certificate_id, record);

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new(world, Arc::clone(&kura), query);
            state.settlement.offline.escrow_required = true;
            state
                .settlement
                .offline
                .escrow_accounts
                .insert(definition_id.clone(), escrow.clone());

            let header = BlockHeader::new(
                nonzero!(1_u64),
                None,
                None,
                None,
                certificate.issued_at_ms + 1,
                0,
            );
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            let err = reclaim_expired_allowance(
                ReclaimExpiredOfflineAllowance { certificate_id },
                &controller,
                &mut transaction,
            )
            .expect_err("reclaim before expiry should fail");
            assert!(
                err.to_string().contains("allowance_not_expired"),
                "unexpected error: {err}"
            );
            assert!(
                transaction
                    .world
                    .offline_allowances
                    .get(&certificate_id)
                    .is_some(),
                "allowance record must remain when reclaim is rejected"
            );
        }

        #[test]
        fn reclaim_expired_allowance_rejects_unauthorized_controller() {
            let mut certificate = sample_certificate();
            certificate.allowance.amount = Numeric::new(50, 0);
            certificate.policy.max_balance = Numeric::new(50, 0);
            certificate.policy.max_tx_value = Numeric::new(50, 0);
            certificate.expires_at_ms = certificate.issued_at_ms + 1;
            certificate.policy.expires_at_ms = certificate.expires_at_ms;
            let controller = certificate.controller.clone();
            let unauthorized = sample_account(0x09);
            let escrow = sample_account(0x02);
            let definition_id = certificate.allowance.asset.definition().clone();

            let record = OfflineAllowanceRecord {
                certificate: certificate.clone(),
                current_commitment: certificate.allowance.commitment.clone(),
                registered_at_ms: certificate.issued_at_ms,
                remaining_amount: Numeric::new(50, 0),
                counter_state: OfflineCounterState::default(),
                verdict_id: None,
                attestation_nonce: None,
                refresh_at_ms: None,
            };

            let domain_id = offline_domain_id();
            let domain = Domain::new(domain_id.clone()).build(&controller);
            let controller_account =
                Account::new(controller.clone().to_account_id(domain_id.clone()))
                    .build(&controller);
            let unauthorized_account =
                Account::new(unauthorized.clone().to_account_id(domain_id.clone()))
                    .build(&unauthorized);
            let escrow_account =
                Account::new(escrow.clone().to_account_id(domain_id)).build(&escrow);
            let asset_definition = {
                let __asset_definition_id = definition_id.clone();
                AssetDefinition::new(__asset_definition_id.clone(), NumericSpec::integer())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .build(&controller);
            let escrow_asset = Asset::new(
                AssetId::new(definition_id.clone(), escrow.clone()),
                Numeric::new(50, 0),
            );
            let mut world = World::with_assets(
                [domain],
                [controller_account, unauthorized_account, escrow_account],
                [asset_definition],
                [escrow_asset],
                [],
            );
            let certificate_id = certificate.certificate_id();
            world.offline_allowances.insert(certificate_id, record);

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new(world, Arc::clone(&kura), query);
            state.settlement.offline.escrow_required = true;
            state
                .settlement
                .offline
                .escrow_accounts
                .insert(definition_id, escrow);

            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 1_900_000_000, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            let err = reclaim_expired_allowance(
                ReclaimExpiredOfflineAllowance { certificate_id },
                &unauthorized,
                &mut transaction,
            )
            .expect_err("unauthorized controller must be rejected");
            assert!(
                err.to_string().contains("unauthorized_controller"),
                "unexpected error: {err}"
            );
            assert!(
                transaction
                    .world
                    .offline_allowances
                    .get(&certificate_id)
                    .is_some(),
                "allowance record must remain when reclaim is rejected"
            );
        }

        #[test]
        fn register_allowance_reissue_reallocates_existing_allowance_without_new_balance() {
            let now_ms = 1_700_000_500;
            let mut previous_certificate = sample_certificate();
            previous_certificate.allowance.amount = Numeric::new(50, 0);
            previous_certificate.policy.max_balance = Numeric::new(50, 0);
            previous_certificate.policy.max_tx_value = Numeric::new(50, 0);
            previous_certificate.issued_at_ms = now_ms - 2_000;
            previous_certificate.expires_at_ms = now_ms + 10_000;
            previous_certificate.policy.expires_at_ms = previous_certificate.expires_at_ms;
            set_certificate_lineage(&mut previous_certificate, "register-reissue", 1, None);
            let previous_id = previous_certificate.certificate_id();

            let previous_record = OfflineAllowanceRecord {
                certificate: previous_certificate.clone(),
                current_commitment: previous_certificate.allowance.commitment.clone(),
                registered_at_ms: now_ms - 1_500,
                remaining_amount: Numeric::new(50, 0),
                counter_state: OfflineCounterState::default(),
                verdict_id: None,
                attestation_nonce: None,
                refresh_at_ms: None,
            };

            let mut reissued_certificate = sample_certificate();
            reissued_certificate.allowance.amount = Numeric::new(50, 0);
            reissued_certificate.policy.max_balance = Numeric::new(50, 0);
            reissued_certificate.policy.max_tx_value = Numeric::new(50, 0);
            reissued_certificate.allowance.commitment = vec![0xCD; 32];
            reissued_certificate.issued_at_ms = now_ms - 10;
            reissued_certificate.expires_at_ms = now_ms + 20_000;
            reissued_certificate.policy.expires_at_ms = reissued_certificate.expires_at_ms;
            set_certificate_lineage(
                &mut reissued_certificate,
                "register-reissue",
                2,
                Some(previous_id),
            );
            let reissue_spend = KeyPair::from_seed(vec![0x88; 32], Algorithm::Ed25519);
            reissued_certificate.spend_public_key = reissue_spend.public_key().clone();
            let operator_keys = KeyPair::from_seed(vec![0x01; 32], Algorithm::Ed25519);
            reissued_certificate.operator_signature = Signature::new(
                operator_keys.private_key(),
                &reissued_certificate
                    .operator_signing_bytes()
                    .expect("certificate signing bytes"),
            );
            let reissued_id = reissued_certificate.certificate_id();

            let controller = reissued_certificate.controller.clone();
            let escrow = sample_account(0x02);
            let definition_id = reissued_certificate.allowance.asset.definition().clone();
            let domain_id = offline_domain_id();
            let domain = Domain::new(domain_id.clone()).build(&controller);
            let controller_account =
                Account::new(controller.clone().to_account_id(domain_id.clone()))
                    .build(&controller);
            let escrow_account =
                Account::new(escrow.clone().to_account_id(domain_id)).build(&escrow);
            let asset_definition = {
                let __asset_definition_id = definition_id.clone();
                AssetDefinition::new(__asset_definition_id.clone(), NumericSpec::integer())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .build(&controller);
            let escrow_asset = Asset::new(
                AssetId::new(definition_id.clone(), escrow.clone()),
                Numeric::new(50, 0),
            );
            let mut world = World::with_assets(
                [domain],
                [controller_account, escrow_account],
                [asset_definition],
                [escrow_asset],
                [],
            );
            world
                .offline_allowances
                .insert(previous_id, previous_record);

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new(world, Arc::clone(&kura), query);
            state.settlement.offline.escrow_required = true;
            state
                .settlement
                .offline
                .escrow_accounts
                .insert(definition_id.clone(), escrow.clone());

            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, now_ms, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            register_allowance(
                RegisterOfflineAllowance {
                    certificate: reissued_certificate,
                },
                &controller,
                &mut transaction,
            )
            .expect("reissue register should reuse existing reserved amount");

            let previous = transaction
                .world
                .offline_allowances
                .get(&previous_id)
                .expect("previous record must remain");
            assert_eq!(
                previous.remaining_amount,
                Numeric::zero(),
                "reissue should consume remaining amount from previous allowance",
            );

            let reissued = transaction
                .world
                .offline_allowances
                .get(&reissued_id)
                .expect("reissued allowance must be registered");
            assert_eq!(reissued.remaining_amount, Numeric::new(50, 0));

            let escrow_balance = transaction
                .world
                .assets
                .get(&AssetId::new(definition_id.clone(), escrow))
                .map(|asset| asset.as_ref().clone())
                .unwrap_or_else(Numeric::zero);
            assert_eq!(
                escrow_balance,
                Numeric::new(50, 0),
                "reissue should not require extra escrow funding",
            );

            let controller_balance = transaction
                .world
                .assets
                .get(&AssetId::new(definition_id, controller))
                .map(|asset| asset.as_ref().clone())
                .unwrap_or_else(Numeric::zero);
            assert_eq!(
                controller_balance,
                Numeric::zero(),
                "controller balance should stay unchanged when reissuing in-place",
            );
        }

        #[test]
        fn register_allowance_reissue_uses_existing_capacity_then_reserves_shortfall() {
            let now_ms = 1_700_000_500;
            let mut previous_certificate = sample_certificate();
            previous_certificate.allowance.amount = Numeric::new(30, 0);
            previous_certificate.policy.max_balance = Numeric::new(30, 0);
            previous_certificate.policy.max_tx_value = Numeric::new(30, 0);
            previous_certificate.issued_at_ms = now_ms - 2_000;
            previous_certificate.expires_at_ms = now_ms + 10_000;
            previous_certificate.policy.expires_at_ms = previous_certificate.expires_at_ms;
            set_certificate_lineage(&mut previous_certificate, "register-reissue", 1, None);
            let previous_id = previous_certificate.certificate_id();

            let previous_record = OfflineAllowanceRecord {
                certificate: previous_certificate.clone(),
                current_commitment: previous_certificate.allowance.commitment.clone(),
                registered_at_ms: now_ms - 1_500,
                remaining_amount: Numeric::new(30, 0),
                counter_state: OfflineCounterState::default(),
                verdict_id: None,
                attestation_nonce: None,
                refresh_at_ms: None,
            };

            let mut reissued_certificate = sample_certificate();
            reissued_certificate.allowance.amount = Numeric::new(50, 0);
            reissued_certificate.policy.max_balance = Numeric::new(50, 0);
            reissued_certificate.policy.max_tx_value = Numeric::new(50, 0);
            reissued_certificate.allowance.commitment = vec![0xEF; 32];
            reissued_certificate.issued_at_ms = now_ms - 10;
            reissued_certificate.expires_at_ms = now_ms + 20_000;
            reissued_certificate.policy.expires_at_ms = reissued_certificate.expires_at_ms;
            set_certificate_lineage(
                &mut reissued_certificate,
                "register-reissue",
                2,
                Some(previous_id),
            );
            let reissue_spend = KeyPair::from_seed(vec![0x99; 32], Algorithm::Ed25519);
            reissued_certificate.spend_public_key = reissue_spend.public_key().clone();
            let operator_keys = KeyPair::from_seed(vec![0x01; 32], Algorithm::Ed25519);
            reissued_certificate.operator_signature = Signature::new(
                operator_keys.private_key(),
                &reissued_certificate
                    .operator_signing_bytes()
                    .expect("certificate signing bytes"),
            );
            let reissued_id = reissued_certificate.certificate_id();

            let controller = reissued_certificate.controller.clone();
            let escrow = sample_account(0x02);
            let definition_id = reissued_certificate.allowance.asset.definition().clone();
            let domain_id = offline_domain_id();
            let domain = Domain::new(domain_id.clone()).build(&controller);
            let controller_account =
                Account::new(controller.clone().to_account_id(domain_id.clone()))
                    .build(&controller);
            let escrow_account =
                Account::new(escrow.clone().to_account_id(domain_id)).build(&escrow);
            let asset_definition = {
                let __asset_definition_id = definition_id.clone();
                AssetDefinition::new(__asset_definition_id.clone(), NumericSpec::integer())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .build(&controller);
            let controller_asset = Asset::new(
                AssetId::new(definition_id.clone(), controller.clone()),
                Numeric::new(20, 0),
            );
            let escrow_asset = Asset::new(
                AssetId::new(definition_id.clone(), escrow.clone()),
                Numeric::new(30, 0),
            );
            let mut world = World::with_assets(
                [domain],
                [controller_account, escrow_account],
                [asset_definition],
                [controller_asset, escrow_asset],
                [],
            );
            world
                .offline_allowances
                .insert(previous_id, previous_record);

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new(world, Arc::clone(&kura), query);
            state.settlement.offline.escrow_required = true;
            state
                .settlement
                .offline
                .escrow_accounts
                .insert(definition_id.clone(), escrow.clone());

            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, now_ms, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            register_allowance(
                RegisterOfflineAllowance {
                    certificate: reissued_certificate,
                },
                &controller,
                &mut transaction,
            )
            .expect("reissue register should reserve only the remaining shortfall");

            let previous = transaction
                .world
                .offline_allowances
                .get(&previous_id)
                .expect("previous record must remain");
            assert_eq!(previous.remaining_amount, Numeric::zero());

            let reissued = transaction
                .world
                .offline_allowances
                .get(&reissued_id)
                .expect("reissued allowance must be registered");
            assert_eq!(reissued.remaining_amount, Numeric::new(50, 0));

            let escrow_balance = transaction
                .world
                .assets
                .get(&AssetId::new(definition_id.clone(), escrow))
                .map(|asset| asset.as_ref().clone())
                .unwrap_or_else(Numeric::zero);
            assert_eq!(
                escrow_balance,
                Numeric::new(50, 0),
                "escrow balance should reflect reused capacity plus reserved shortfall",
            );

            let controller_balance = transaction
                .world
                .assets
                .get(&AssetId::new(definition_id, controller))
                .map(|asset| asset.as_ref().clone())
                .unwrap_or_else(Numeric::zero);
            assert_eq!(
                controller_balance,
                Numeric::zero(),
                "controller should fund only the uncovered shortfall",
            );
        }

        #[test]
        fn register_allowance_reissue_supersedes_head_allowance_and_refunds_excess_capacity() {
            let now_ms = 1_700_000_500;
            let mut older_certificate = sample_certificate();
            older_certificate.allowance.amount = Numeric::new(40, 0);
            older_certificate.policy.max_balance = Numeric::new(40, 0);
            older_certificate.policy.max_tx_value = Numeric::new(40, 0);
            older_certificate.allowance.commitment = vec![0xA1; 32];
            older_certificate.issued_at_ms = now_ms - 4_000;
            older_certificate.expires_at_ms = now_ms + 20_000;
            older_certificate.policy.expires_at_ms = older_certificate.expires_at_ms;
            set_certificate_lineage(&mut older_certificate, "register-reissue", 1, None);
            let older_id = older_certificate.certificate_id();
            let older_record = OfflineAllowanceRecord {
                certificate: older_certificate,
                current_commitment: vec![0xA1; 32],
                registered_at_ms: now_ms - 3_000,
                remaining_amount: Numeric::new(40, 0),
                counter_state: OfflineCounterState::default(),
                verdict_id: None,
                attestation_nonce: None,
                refresh_at_ms: None,
            };

            let mut newer_certificate = sample_certificate();
            newer_certificate.allowance.amount = Numeric::new(40, 0);
            newer_certificate.policy.max_balance = Numeric::new(40, 0);
            newer_certificate.policy.max_tx_value = Numeric::new(40, 0);
            newer_certificate.allowance.commitment = vec![0xB2; 32];
            newer_certificate.issued_at_ms = now_ms - 2_000;
            newer_certificate.expires_at_ms = now_ms + 20_000;
            newer_certificate.policy.expires_at_ms = newer_certificate.expires_at_ms;
            set_certificate_lineage(
                &mut newer_certificate,
                "register-reissue",
                2,
                Some(older_id),
            );
            let newer_id = newer_certificate.certificate_id();
            let newer_record = OfflineAllowanceRecord {
                certificate: newer_certificate,
                current_commitment: vec![0xB2; 32],
                registered_at_ms: now_ms - 1_000,
                remaining_amount: Numeric::new(40, 0),
                counter_state: OfflineCounterState::default(),
                verdict_id: None,
                attestation_nonce: None,
                refresh_at_ms: None,
            };

            let mut reissued_certificate = sample_certificate();
            reissued_certificate.allowance.amount = Numeric::new(30, 0);
            reissued_certificate.policy.max_balance = Numeric::new(30, 0);
            reissued_certificate.policy.max_tx_value = Numeric::new(30, 0);
            reissued_certificate.allowance.commitment = vec![0xCC; 32];
            reissued_certificate.issued_at_ms = now_ms - 10;
            reissued_certificate.expires_at_ms = now_ms + 20_000;
            reissued_certificate.policy.expires_at_ms = reissued_certificate.expires_at_ms;
            set_certificate_lineage(
                &mut reissued_certificate,
                "register-reissue",
                3,
                Some(newer_id),
            );
            let reissue_spend = KeyPair::from_seed(vec![0xA4; 32], Algorithm::Ed25519);
            reissued_certificate.spend_public_key = reissue_spend.public_key().clone();
            let operator_keys = KeyPair::from_seed(vec![0x01; 32], Algorithm::Ed25519);
            reissued_certificate.operator_signature = Signature::new(
                operator_keys.private_key(),
                &reissued_certificate
                    .operator_signing_bytes()
                    .expect("certificate signing bytes"),
            );
            let reissued_id = reissued_certificate.certificate_id();

            let controller = reissued_certificate.controller.clone();
            let escrow = sample_account(0x02);
            let definition_id = reissued_certificate.allowance.asset.definition().clone();
            let domain_id = offline_domain_id();
            let domain = Domain::new(domain_id.clone()).build(&controller);
            let controller_account =
                Account::new(controller.clone().to_account_id(domain_id.clone()))
                    .build(&controller);
            let escrow_account =
                Account::new(escrow.clone().to_account_id(domain_id)).build(&escrow);
            let asset_definition = {
                let __asset_definition_id = definition_id.clone();
                AssetDefinition::new(__asset_definition_id.clone(), NumericSpec::integer())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .build(&controller);
            let escrow_asset = Asset::new(
                AssetId::new(definition_id.clone(), escrow.clone()),
                Numeric::new(80, 0),
            );
            let mut world = World::with_assets(
                [domain],
                [controller_account, escrow_account],
                [asset_definition],
                [escrow_asset],
                [],
            );
            world.offline_allowances.insert(older_id, older_record);
            world.offline_allowances.insert(newer_id, newer_record);

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new(world, Arc::clone(&kura), query);
            state.settlement.offline.escrow_required = true;
            state
                .settlement
                .offline
                .escrow_accounts
                .insert(definition_id.clone(), escrow.clone());

            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, now_ms, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            register_allowance(
                RegisterOfflineAllowance {
                    certificate: reissued_certificate,
                },
                &controller,
                &mut transaction,
            )
            .expect("reissue should consume newest capacity deterministically");

            let older = transaction
                .world
                .offline_allowances
                .get(&older_id)
                .expect("older record should remain");
            assert_eq!(
                older.remaining_amount,
                Numeric::new(40, 0),
                "older allowance should remain untouched when newer capacity is sufficient",
            );

            let newer = transaction
                .world
                .offline_allowances
                .get(&newer_id)
                .expect("newer record should remain");
            assert_eq!(
                newer.remaining_amount,
                Numeric::zero(),
                "renewal/reissue should fully invalidate the direct predecessor allowance",
            );

            let reissued = transaction
                .world
                .offline_allowances
                .get(&reissued_id)
                .expect("reissued allowance must be registered");
            assert_eq!(reissued.remaining_amount, Numeric::new(30, 0));

            let escrow_balance = transaction
                .world
                .assets
                .get(&AssetId::new(definition_id.clone(), escrow))
                .map(|asset| asset.as_ref().clone())
                .unwrap_or_else(Numeric::zero);
            assert_eq!(
                escrow_balance,
                Numeric::new(70, 0),
                "excess predecessor reserve should be released back on-chain",
            );

            let controller_balance = transaction
                .world
                .assets
                .get(&AssetId::new(definition_id, controller))
                .map(|asset| asset.as_ref().clone())
                .unwrap_or_else(Numeric::zero);
            assert_eq!(
                controller_balance,
                Numeric::new(10, 0),
                "unused predecessor reserve should be refunded to the controller",
            );
        }

        #[test]
        fn renewal_supersedes_previous_allowance_even_when_new_amount_can_be_fully_reserved() {
            let now_ms = 1_700_000_500;
            let mut previous_certificate = sample_certificate();
            previous_certificate.allowance.amount = Numeric::new(50, 0);
            previous_certificate.policy.max_balance = Numeric::new(50, 0);
            previous_certificate.policy.max_tx_value = Numeric::new(50, 0);
            previous_certificate.issued_at_ms = now_ms - 2_000;
            previous_certificate.expires_at_ms = now_ms + 10_000;
            previous_certificate.policy.expires_at_ms = previous_certificate.expires_at_ms;
            set_certificate_lineage(&mut previous_certificate, "register-renewal", 1, None);
            let previous_id = previous_certificate.certificate_id();

            let previous_record = OfflineAllowanceRecord {
                certificate: previous_certificate,
                current_commitment: vec![0; 32],
                registered_at_ms: now_ms - 200,
                remaining_amount: Numeric::new(50, 0),
                counter_state: OfflineCounterState::default(),
                verdict_id: None,
                attestation_nonce: None,
                refresh_at_ms: None,
            };

            let mut renewal_certificate = sample_certificate();
            renewal_certificate.allowance.amount = Numeric::new(20, 0);
            renewal_certificate.policy.max_balance = Numeric::new(20, 0);
            renewal_certificate.policy.max_tx_value = Numeric::new(20, 0);
            renewal_certificate.issued_at_ms = now_ms - 10;
            renewal_certificate.expires_at_ms = now_ms + 10_000;
            renewal_certificate.policy.expires_at_ms = renewal_certificate.expires_at_ms;
            set_certificate_lineage(
                &mut renewal_certificate,
                "register-renewal",
                2,
                Some(previous_id),
            );
            let operator_keys = KeyPair::from_seed(vec![0x01; 32], Algorithm::Ed25519);
            renewal_certificate.operator_signature = Signature::new(
                operator_keys.private_key(),
                &renewal_certificate
                    .operator_signing_bytes()
                    .expect("certificate signing bytes"),
            );
            let renewal_id = renewal_certificate.certificate_id();

            let controller = renewal_certificate.controller.clone();
            let escrow = sample_account(0x02);
            let definition_id = renewal_certificate.allowance.asset.definition().clone();
            let domain_id = offline_domain_id();
            let domain = Domain::new(domain_id.clone()).build(&controller);
            let controller_account =
                Account::new(controller.clone().to_account_id(domain_id.clone()))
                    .build(&controller);
            let escrow_account =
                Account::new(escrow.clone().to_account_id(domain_id)).build(&escrow);
            let asset_definition = {
                let __asset_definition_id = definition_id.clone();
                AssetDefinition::new(__asset_definition_id.clone(), NumericSpec::integer())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .build(&controller);
            let controller_asset = Asset::new(
                AssetId::new(definition_id.clone(), controller.clone()),
                Numeric::new(50, 0),
            );
            let escrow_asset = Asset::new(
                AssetId::new(definition_id.clone(), escrow.clone()),
                Numeric::new(50, 0),
            );
            let mut world = World::with_assets(
                [domain],
                [controller_account, escrow_account],
                [asset_definition],
                [controller_asset, escrow_asset],
                [],
            );
            world
                .offline_allowances
                .insert(previous_id, previous_record);
            let controller_asset = AssetId::new(definition_id.clone(), controller.clone());

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new(world, Arc::clone(&kura), query);
            state.settlement.offline.escrow_required = true;
            state
                .settlement
                .offline
                .escrow_accounts
                .insert(definition_id.clone(), escrow.clone());

            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, now_ms, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            register_allowance(
                RegisterOfflineAllowance {
                    certificate: renewal_certificate,
                },
                &controller,
                &mut transaction,
            )
            .expect("renewal register should succeed");

            let previous = transaction
                .world
                .offline_allowances
                .get(&previous_id)
                .expect("previous allowance must remain for lineage history");
            assert_eq!(
                previous.remaining_amount,
                Numeric::zero(),
                "renewal should invalidate the superseded allowance",
            );
            assert!(
                transaction
                    .world
                    .offline_allowances
                    .get(&renewal_id)
                    .is_some(),
                "renewal must create a distinct allowance record"
            );

            let escrow_balance = transaction
                .world
                .assets
                .get(&AssetId::new(definition_id.clone(), escrow))
                .map(|asset| asset.as_ref().clone())
                .unwrap_or_else(Numeric::zero);
            assert_eq!(
                escrow_balance,
                Numeric::new(20, 0),
                "renewal should leave escrow backed only by the new allowance head",
            );

            let on_chain_balance = transaction
                .world
                .assets
                .get(&controller_asset)
                .map(|asset| asset.as_ref().clone())
                .unwrap_or_else(Numeric::zero);
            assert_eq!(
                on_chain_balance,
                Numeric::new(80, 0),
                "unused predecessor reserve should be returned to the controller",
            );
        }
    }

    #[allow(clippy::too_many_lines)]
    fn submit_transfer(
        isi: SubmitOfflineToOnlineTransfer,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let transfer = isi.transfer;
        if &transfer.receiver != authority {
            return Err(rejection_error(
                OfflineTransferRejectionReason::UnauthorizedReceiver,
                OfflineTransferRejectionPlatform::General,
                "only the designated receiver may submit the offline transfer bundle",
            ));
        }
        if transfer.receipts.is_empty() {
            return Err(rejection_error(
                OfflineTransferRejectionReason::EmptyBundle,
                OfflineTransferRejectionPlatform::General,
                "offline bundle must include at least one receipt",
            ));
        }
        if !receipts_are_canonical(&transfer.receipts) {
            return Err(rejection_error(
                OfflineTransferRejectionReason::ReceiptOrderInvalid,
                OfflineTransferRejectionPlatform::General,
                "offline bundle receipts must be ordered by (counter, tx_id)",
            ));
        }
        if state_transaction
            .world
            .offline_to_online_transfers
            .get(&transfer.bundle_id)
            .is_some()
        {
            return Err(rejection_error(
                OfflineTransferRejectionReason::DuplicateBundle,
                OfflineTransferRejectionPlatform::General,
                "bundle id already exists",
            ));
        }

        let mut invoice_ids = BTreeSet::new();
        for receipt in &transfer.receipts {
            if !invoice_ids.insert(receipt.invoice_id.clone()) {
                return Err(rejection_error(
                    OfflineTransferRejectionReason::InvoiceDuplicate,
                    OfflineTransferRejectionPlatform::General,
                    "offline bundle contains duplicate invoice_id values",
                ));
            }
        }

        let asset = ensure_uniform_asset(&transfer.receipts)?;
        let block_timestamp_ms = state_transaction.block_unix_timestamp_ms();
        let block_height = state_transaction.block_height();
        let spec = state_transaction.numeric_spec_for(asset.definition())?;
        let receipt_count = u32::try_from(transfer.receipts.len()).map_err(|_| {
            InstructionExecutionError::InvariantViolation(
                "receipt count exceeds supported range".into(),
            )
        })?;

        let mut receipts_by_certificate: std::collections::BTreeMap<
            Hash,
            Vec<OfflineSpendReceipt>,
        > = std::collections::BTreeMap::new();
        for receipt in &transfer.receipts {
            receipts_by_certificate
                .entry(receipt.sender_certificate_id)
                .or_default()
                .push(receipt.clone());
        }
        let multi_certificate_bundle = receipts_by_certificate.len() > 1;

        let explicit_balance_proofs = transfer
            .balance_proofs
            .as_ref()
            .filter(|entries| !entries.is_empty());
        if multi_certificate_bundle && explicit_balance_proofs.is_none() {
            return Err(rejection_error(
                OfflineTransferRejectionReason::MixedCertificates,
                OfflineTransferRejectionPlatform::General,
                "offline bundle mixes certificates but has no per-certificate balance proofs",
            ));
        }

        let mut balance_proof_by_certificate: std::collections::BTreeMap<
            Hash,
            &OfflineBalanceProof,
        > = std::collections::BTreeMap::new();
        if let Some(entries) = explicit_balance_proofs {
            for entry in entries {
                if balance_proof_by_certificate
                    .insert(entry.sender_certificate_id, &entry.balance_proof)
                    .is_some()
                {
                    return Err(rejection_error(
                        OfflineTransferRejectionReason::BalanceProofInvalid,
                        OfflineTransferRejectionPlatform::General,
                        "duplicate per-certificate balance proof entry",
                    ));
                }
            }
            for certificate_id in receipts_by_certificate.keys() {
                if !balance_proof_by_certificate.contains_key(certificate_id) {
                    return Err(rejection_error(
                        OfflineTransferRejectionReason::BalanceProofInvalid,
                        OfflineTransferRejectionPlatform::General,
                        "missing per-certificate balance proof for receipt certificate",
                    ));
                }
            }
            for certificate_id in balance_proof_by_certificate.keys() {
                if !receipts_by_certificate.contains_key(certificate_id) {
                    return Err(rejection_error(
                        OfflineTransferRejectionReason::BalanceProofInvalid,
                        OfflineTransferRejectionPlatform::General,
                        "per-certificate balance proof references unknown certificate",
                    ));
                }
            }
        } else {
            let certificate_id = transfer
                .receipts
                .first()
                .expect("non-empty receipts")
                .sender_certificate_id;
            balance_proof_by_certificate.insert(certificate_id, &transfer.balance_proof);
        }

        if multi_certificate_bundle {
            for certificate_receipts in receipts_by_certificate.values() {
                if let Err(err) = ensure_single_counter_scope(certificate_receipts) {
                    return Err(map_counter_scope_error(err));
                }
            }
        } else if let Err(err) = ensure_single_counter_scope(&transfer.receipts) {
            return Err(map_counter_scope_error(err));
        }

        verify_aggregate_proof_envelope(
            &transfer,
            state_transaction.settlement.offline.proof_mode,
        )?;

        let mut staged_updates: Vec<(Hash, Numeric, Vec<u8>, OfflineCounterState)> = Vec::new();
        let mut certificates_by_id: std::collections::BTreeMap<Hash, OfflineWalletCertificate> =
            std::collections::BTreeMap::new();
        let mut policy_labels = BTreeSet::new();
        let mut settled_platform_snapshot: Option<OfflinePlatformTokenSnapshot> = None;
        let mut expected_controller: Option<AccountId> = None;
        let mut claimed_amount = Numeric::zero();
        let mut claim_ids_in_bundle = BTreeSet::new();

        for (certificate_id, certificate_receipts) in &receipts_by_certificate {
            let record = state_transaction
                .world
                .offline_allowances
                .get(certificate_id)
                .ok_or_else(|| {
                    rejection_error(
                        OfflineTransferRejectionReason::AllowanceNotRegistered,
                        OfflineTransferRejectionPlatform::General,
                        "offline allowance certificate not registered",
                    )
                })?
                .clone();
            let lineage = parse_certificate_lineage_metadata(&record.certificate)?;

            if let Some(expected) = expected_controller.as_ref() {
                if &record.certificate.controller != expected {
                    return Err(rejection_error(
                        OfflineTransferRejectionReason::ReceiptSenderMismatch,
                        OfflineTransferRejectionPlatform::General,
                        "offline bundle mixes receipts from different controllers",
                    ));
                }
            } else {
                expected_controller = Some(record.certificate.controller.clone());
            }

            if let Some(verdict_id) = record.verdict_id {
                if state_transaction
                    .world
                    .offline_verdict_revocations
                    .get(&verdict_id)
                    .is_some()
                {
                    return Err(rejection_error(
                        OfflineTransferRejectionReason::VerdictExpired,
                        OfflineTransferRejectionPlatform::General,
                        "offline attestation verdict revoked; refresh allowance before reconciling",
                    ));
                }
            }

            enforce_certificate_window(&record, block_timestamp_ms)?;
            enforce_verdict_refresh_window(&record, block_timestamp_ms)?;

            let expected_scale = expected_scale_for_allowance(&record.certificate, spec)?;
            let certificate_claimed_amount =
                aggregate_amount(certificate_receipts, expected_scale)?;
            let balance_proof = balance_proof_by_certificate
                .get(certificate_id)
                .ok_or_else(|| {
                    rejection_error(
                        OfflineTransferRejectionReason::BalanceProofInvalid,
                        OfflineTransferRejectionPlatform::General,
                        "missing balance proof for certificate",
                    )
                })?;
            if certificate_claimed_amount != balance_proof.claimed_delta().clone() {
                return Err(rejection_error(
                    OfflineTransferRejectionReason::DeltaMismatch,
                    OfflineTransferRejectionPlatform::General,
                    "claimed delta does not match sum of receipt amounts",
                ));
            }

            let integrity_metadata = android_integrity_metadata(&record.certificate.metadata).ok();
            if multi_certificate_bundle && transfer.platform_snapshot.is_some() {
                return Err(rejection_error(
                    OfflineTransferRejectionReason::PlatformMetadataInvalid,
                    OfflineTransferRejectionPlatform::General,
                    "bundle-level platform snapshot is not supported for multi-certificate bundles",
                ));
            }
            let submitted_platform_snapshot = validate_platform_snapshot(
                if multi_certificate_bundle {
                    None
                } else {
                    transfer.platform_snapshot.as_ref()
                },
                integrity_metadata.as_ref(),
            )?;
            if !multi_certificate_bundle {
                settled_platform_snapshot = submitted_platform_snapshot.clone().or_else(|| {
                    derive_platform_token_snapshot(&record.certificate, integrity_metadata.as_ref())
                });
            }
            if let Some(policy) = integrity_metadata
                .as_ref()
                .map(AndroidIntegrityMetadata::policy_slug)
            {
                policy_labels.insert(policy);
            }

            ensure_receipt_targets(certificate_receipts, &transfer, &record)?;
            ensure_receipt_timestamps(
                certificate_receipts,
                &record,
                block_timestamp_ms,
                &state_transaction.settlement.offline,
            )?;
            ensure_commitment_alignment(balance_proof, &record)?;

            let mut staged_counters = record.counter_state.clone();
            stage_receipt_counters(&mut staged_counters, certificate_receipts)?;

            let submitted_snapshot_ref = submitted_platform_snapshot.as_ref();
            for receipt in certificate_receipts {
                verify_receipt_signature(receipt, &record.certificate)?;
                let receipt_snapshot = validate_platform_snapshot(
                    receipt.platform_snapshot.as_ref(),
                    integrity_metadata.as_ref(),
                )?;
                let attestation_snapshot =
                    select_attestation_snapshot(receipt_snapshot.as_ref(), submitted_snapshot_ref);
                verify_platform_proof(
                    receipt,
                    &record.certificate,
                    &state_transaction.chain_id,
                    block_timestamp_ms,
                    &state_transaction.settlement.offline,
                    attestation_snapshot,
                )?;
                verify_receipt_build_claim(
                    state_transaction,
                    receipt,
                    &record.certificate,
                    &lineage,
                    block_timestamp_ms,
                    &mut claim_ids_in_bundle,
                )?;
            }

            verify_balance_proof(&VerificationInputs {
                balance_proof,
                chain_id: &state_transaction.chain_id,
                expected_scale,
            })
            .map_err(|err| {
                rejection_error(
                    OfflineTransferRejectionReason::BalanceProofInvalid,
                    OfflineTransferRejectionPlatform::General,
                    err.to_string(),
                )
            })?;

            if certificate_claimed_amount > record.remaining_amount {
                return Err(rejection_error(
                    OfflineTransferRejectionReason::AllowanceDepleted,
                    OfflineTransferRejectionPlatform::General,
                    "offline allowance does not have enough remaining value",
                ));
            }

            claimed_amount = claimed_amount
                .checked_add(certificate_claimed_amount.clone())
                .ok_or(MathError::Overflow)?;
            staged_updates.push((
                *certificate_id,
                certificate_claimed_amount,
                balance_proof.resulting_commitment.clone(),
                staged_counters,
            ));
            certificates_by_id.insert(*certificate_id, record.certificate.clone());
        }

        let primary_certificate_id = transfer
            .receipts
            .first()
            .expect("non-empty receipts")
            .sender_certificate_id;
        let primary_certificate = certificates_by_id
            .get(&primary_certificate_id)
            .expect("certificate presence checked while staging updates");
        let pos_verdict_snapshots: Vec<_> = transfer
            .receipts
            .iter()
            .filter_map(|receipt| {
                certificates_by_id
                    .get(&receipt.sender_certificate_id)
                    .map(|certificate| {
                        let mut snapshot = OfflineVerdictSnapshot::from_certificate(certificate);
                        snapshot.certificate_id = receipt.sender_certificate_id;
                        snapshot
                    })
            })
            .collect();

        let mut audit_record = OfflineTransferRecord {
            transfer: transfer.clone(),
            controller: expected_controller
                .expect("controller presence checked while staging updates"),
            status: OfflineTransferStatus::Settled,
            rejection_reason: None,
            recorded_at_ms: block_timestamp_ms,
            recorded_at_height: block_height,
            archived_at_height: None,
            history: Vec::new(),
            pos_verdict_snapshots,
            verdict_snapshot: Some(OfflineVerdictSnapshot::from_certificate(
                primary_certificate,
            )),
            platform_snapshot: settled_platform_snapshot,
        };
        let history_snapshot = audit_record.verdict_snapshot.clone();
        audit_record.push_history_entry(
            OfflineTransferStatus::Settled,
            block_timestamp_ms,
            history_snapshot.as_ref(),
        );

        credit_deposit_account(
            state_transaction,
            &asset,
            &transfer.deposit_account,
            &claimed_amount,
        )?;

        for (certificate_id, certificate_claimed_amount, resulting_commitment, staged_counters) in
            staged_updates
        {
            let record = state_transaction
                .world
                .offline_allowances
                .get_mut(&certificate_id)
                .expect("allowance existence checked while staging updates");
            debug_assert!(certificate_claimed_amount <= record.remaining_amount);
            record.remaining_amount = record
                .remaining_amount
                .clone()
                .checked_sub(certificate_claimed_amount)
                .expect("claimed amount is bounded by remaining_amount");
            record.current_commitment.clone_from(&resulting_commitment);
            merge_counter_state(&mut record.counter_state, &staged_counters);
        }

        insert_transfer_record(state_transaction, audit_record.clone());
        emit_transfer_event(
            state_transaction,
            &audit_record,
            claimed_amount.clone(),
            receipt_count,
        );
        record_transfer_metrics(&audit_record, &claimed_amount);
        for policy in policy_labels {
            metrics::global_or_default().record_offline_attestation_policy(policy);
        }
        apply_transfer_retention(state_transaction);

        Ok(())
    }

    fn reclaim_expired_allowance(
        isi: ReclaimExpiredOfflineAllowance,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let certificate_id = isi.certificate_id;
        let record = state_transaction
            .world
            .offline_allowances
            .get(&certificate_id)
            .ok_or_else(|| {
                labeled_invariant(
                    "allowance_not_found",
                    "offline allowance certificate not registered",
                )
            })?
            .clone();

        if &record.certificate.controller != authority {
            return Err(labeled_invariant(
                "unauthorized_controller",
                "only the allowance controller may reclaim an expired offline allowance",
            ));
        }

        let block_timestamp_ms = state_transaction.block_unix_timestamp_ms();
        let certificate_expired = record.certificate.expires_at_ms <= block_timestamp_ms;
        let policy_expired = record.certificate.policy.expires_at_ms <= block_timestamp_ms;
        if !certificate_expired && !policy_expired {
            return Err(labeled_invariant(
                "allowance_not_expired",
                "offline allowance has not expired yet",
            ));
        }

        let definition_id = record.certificate.allowance.asset.definition().clone();
        let controller = record.certificate.controller.clone();
        let amount = record.remaining_amount.clone();

        let spec = state_transaction.numeric_spec_for(&definition_id)?;
        assert_numeric_spec_with(&amount, spec)?;
        state_transaction.world.account(&controller)?;

        refund_allowance_from_escrow(
            state_transaction,
            &record.certificate.allowance.asset,
            &controller,
            &amount,
        )?;

        let removed = state_transaction
            .world
            .offline_allowances
            .remove(certificate_id);
        if removed.is_none() {
            return Err(labeled_invariant(
                "allowance_not_found",
                "offline allowance certificate not registered",
            ));
        }

        let payload = OfflineAllowanceReclaimed {
            certificate_id,
            controller,
            asset_definition: definition_id,
            amount,
            reclaimed_at_ms: block_timestamp_ms,
        };
        emit_reclaim_event(state_transaction, payload);
        Ok(())
    }

    fn register_verdict_revocation(
        isi: RegisterOfflineVerdictRevocation,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let mut revocation = isi.revocation;
        let verdict_id = revocation.verdict_id;
        if state_transaction
            .world
            .offline_verdict_revocations
            .get(&verdict_id)
            .is_some()
        {
            return Err(labeled_invariant(
                "verdict_duplicate",
                "offline verdict_id already revoked",
            ));
        }

        let mut matching_record = None;
        for (certificate_id, record) in state_transaction.world.offline_allowances.iter() {
            if record.verdict_id.as_ref() == Some(&verdict_id) {
                matching_record = Some((certificate_id, record));
                break;
            }
        }

        let (_, record) = matching_record.ok_or_else(|| {
            labeled_invariant(
                "allowance_not_found",
                "no offline allowance is associated with the supplied verdict_id",
            )
        })?;

        if &record.certificate.controller != authority {
            return Err(labeled_invariant(
                "unauthorized_controller",
                "only the allowance controller may revoke its attestation verdict",
            ));
        }

        revocation.issuer = authority.clone();
        revocation.revoked_at_ms = state_transaction.block_unix_timestamp_ms();

        state_transaction
            .world
            .offline_verdict_revocations
            .insert(verdict_id, revocation.clone());
        emit_revocation_event(state_transaction, &revocation);
        Ok(())
    }

    fn aggregate_v2_public_inputs_digest(
        transfer: &OfflineToOnlineTransfer,
        receipts_root: &iroha_data_model::offline::PoseidonDigest,
    ) -> Result<[u8; 32], String> {
        let mut hasher = Sha256::new();
        hasher.update(b"iroha.offline.aggregate.v2.public_inputs");
        hasher.update(transfer.bundle_id.as_ref());
        hasher.update(receipts_root.as_bytes());

        let claimed_delta = transfer
            .balance_proof
            .claimed_delta
            .try_mantissa_u128()
            .ok_or_else(|| "claimed delta out of range".to_string())?;
        hasher.update(claimed_delta.to_le_bytes());
        hasher.update(transfer.balance_proof.claimed_delta.scale().to_be_bytes());
        hasher.update(
            u64::try_from(transfer.receipts.len())
                .map_err(|_| "receipt count out of range".to_string())?
                .to_le_bytes(),
        );

        for receipt in canonical_receipts(&transfer.receipts) {
            hasher.update(receipt.sender_certificate_id.as_ref());
            hasher.update(receipt.tx_id.as_ref());
            hasher.update(receipt.platform_proof.counter().to_be_bytes());
            let amount = receipt
                .amount
                .try_mantissa_u128()
                .ok_or_else(|| "receipt amount out of range".to_string())?;
            hasher.update(amount.to_le_bytes());
            hasher.update(receipt.amount.scale().to_be_bytes());
        }

        let digest = hasher.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        Ok(out)
    }

    fn verify_aggregate_proof_envelope_v2(
        transfer: &OfflineToOnlineTransfer,
        envelope: &iroha_data_model::offline::AggregateProofEnvelope,
        receipts_root: &iroha_data_model::offline::PoseidonDigest,
    ) -> Result<(), Error> {
        let platform = OfflineTransferRejectionPlatform::General;
        let backend = metadata_string(
            &envelope.metadata,
            AGGREGATE_PROOF_METADATA_OFFLINE_AGGREGATE_BACKEND,
            platform,
        )?;
        if backend != "stark/fri/poseidon2-goldilocks" {
            return Err(rejection_error(
                OfflineTransferRejectionReason::AggregateProofHashError,
                platform,
                format!("unsupported aggregate v2 backend `{backend}`"),
            ));
        }

        let _circuit_id = metadata_string(
            &envelope.metadata,
            AGGREGATE_PROOF_METADATA_OFFLINE_AGGREGATE_CIRCUIT_ID,
            platform,
        )?;
        let public_inputs_b64 = metadata_string(
            &envelope.metadata,
            AGGREGATE_PROOF_METADATA_OFFLINE_AGGREGATE_PUBLIC_INPUTS_B64,
            platform,
        )?;
        let recursion_depth_raw = metadata_string(
            &envelope.metadata,
            AGGREGATE_PROOF_METADATA_OFFLINE_AGGREGATE_RECURSION_DEPTH,
            platform,
        )?;
        let recursion_depth = recursion_depth_raw.parse::<u32>().map_err(|_| {
            rejection_error(
                OfflineTransferRejectionReason::AggregateProofHashError,
                platform,
                "aggregate v2 recursion depth metadata is invalid",
            )
        })?;
        if recursion_depth == 0 {
            return Err(rejection_error(
                OfflineTransferRejectionReason::AggregateProofHashError,
                platform,
                "aggregate v2 recursion depth must be greater than zero",
            ));
        }

        let expected_inputs =
            aggregate_v2_public_inputs_digest(transfer, receipts_root).map_err(|err| {
                rejection_error(
                    OfflineTransferRejectionReason::AggregateProofHashError,
                    platform,
                    format!("failed to derive aggregate v2 public inputs: {err}"),
                )
            })?;
        let provided_inputs = BASE64_STANDARD
            .decode(public_inputs_b64.as_bytes())
            .map_err(|_| {
                rejection_error(
                    OfflineTransferRejectionReason::AggregateProofHashError,
                    platform,
                    "aggregate v2 public inputs are not valid base64",
                )
            })?;
        if provided_inputs != expected_inputs {
            return Err(rejection_error(
                OfflineTransferRejectionReason::AggregateProofHashError,
                platform,
                "aggregate v2 public inputs do not match bundle contents",
            ));
        }

        let proof = envelope.proof_sum.as_deref().ok_or_else(|| {
            rejection_error(
                OfflineTransferRejectionReason::AggregateProofMissing,
                platform,
                "aggregate v2 proof_sum is required",
            )
        })?;
        if proof.is_empty() || !verify_recursive_stark_envelope(proof) {
            return Err(rejection_error(
                OfflineTransferRejectionReason::AggregateProofHashError,
                platform,
                "aggregate v2 recursive STARK proof is invalid",
            ));
        }

        if envelope
            .proof_counter
            .as_ref()
            .is_some_and(|bytes| !bytes.is_empty())
            || envelope
                .proof_replay
                .as_ref()
                .is_some_and(|bytes| !bytes.is_empty())
        {
            return Err(rejection_error(
                OfflineTransferRejectionReason::AggregateProofHashError,
                platform,
                "aggregate v2 uses proof_sum only; proof_counter/proof_replay must be empty",
            ));
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn verify_aggregate_proof_envelope(
        transfer: &OfflineToOnlineTransfer,
        proof_mode: OfflineProofMode,
    ) -> Result<(), Error> {
        let Some(envelope) = transfer.aggregate_proof.as_ref() else {
            if matches!(proof_mode, OfflineProofMode::Required) {
                return Err(rejection_error(
                    OfflineTransferRejectionReason::AggregateProofMissing,
                    OfflineTransferRejectionPlatform::General,
                    "aggregate proofs are required for offline bundles",
                ));
            }
            return Ok(());
        };

        match envelope.version {
            AGGREGATE_PROOF_VERSION_V1 => {}
            AGGREGATE_PROOF_VERSION_V2 => {}
            _ => {
                return Err(rejection_error(
                    OfflineTransferRejectionReason::AggregateProofVersionUnsupported,
                    OfflineTransferRejectionPlatform::General,
                    format!(
                        "aggregate proof version {} is not supported",
                        envelope.version
                    ),
                ));
            }
        }

        if matches!(proof_mode, OfflineProofMode::Required) {
            let mut missing = Vec::new();
            if envelope.proof_sum.as_ref().is_none_or(Vec::is_empty) {
                missing.push("sum");
            }
            if envelope.version == AGGREGATE_PROOF_VERSION_V1 {
                if envelope.proof_counter.as_ref().is_none_or(Vec::is_empty) {
                    missing.push("counter");
                }
                if envelope.proof_replay.as_ref().is_none_or(Vec::is_empty) {
                    missing.push("replay");
                }
            }
            if !missing.is_empty() {
                return Err(rejection_error(
                    OfflineTransferRejectionReason::AggregateProofMissing,
                    OfflineTransferRejectionPlatform::General,
                    format!("aggregate proofs missing: {}", missing.join(", ")),
                ));
            }
        }

        let receipts_root = compute_receipts_root(&transfer.receipts).map_err(|err| {
            rejection_error(
                OfflineTransferRejectionReason::AggregateProofHashError,
                OfflineTransferRejectionPlatform::General,
                format!("failed to hash receipts: {err}"),
            )
        })?;

        if envelope.receipts_root != receipts_root {
            return Err(rejection_error(
                OfflineTransferRejectionReason::AggregateProofRootMismatch,
                OfflineTransferRejectionPlatform::General,
                "aggregate proof receipts_root does not match transfer receipts",
            ));
        }

        if envelope.version == AGGREGATE_PROOF_VERSION_V2 {
            return verify_aggregate_proof_envelope_v2(transfer, envelope, &receipts_root);
        }

        if let Some(proof_sum) = envelope.proof_sum.as_deref() {
            if proof_sum.is_empty() {
                return Err(rejection_error(
                    OfflineTransferRejectionReason::AggregateProofHashError,
                    OfflineTransferRejectionPlatform::General,
                    "aggregate sum proof bytes are empty",
                ));
            }
            verify_fastpq_sum_proof(transfer, &receipts_root, proof_sum).map_err(|err| {
                rejection_error(
                    OfflineTransferRejectionReason::AggregateProofHashError,
                    OfflineTransferRejectionPlatform::General,
                    format!("aggregate sum proof invalid: {err}"),
                )
            })?;
        }

        if let Some(proof_counter) = envelope.proof_counter.as_deref() {
            if proof_counter.is_empty() {
                return Err(rejection_error(
                    OfflineTransferRejectionReason::AggregateProofHashError,
                    OfflineTransferRejectionPlatform::General,
                    "aggregate counter proof bytes are empty",
                ));
            }
            verify_fastpq_counter_proof(transfer, &receipts_root, proof_counter).map_err(
                |err| {
                    rejection_error(
                        OfflineTransferRejectionReason::AggregateProofHashError,
                        OfflineTransferRejectionPlatform::General,
                        format!("aggregate counter proof invalid: {err}"),
                    )
                },
            )?;
        }

        if let Some(proof_replay) = envelope.proof_replay.as_deref() {
            if proof_replay.is_empty() {
                return Err(rejection_error(
                    OfflineTransferRejectionReason::AggregateProofHashError,
                    OfflineTransferRejectionPlatform::General,
                    "aggregate replay proof bytes are empty",
                ));
            }
            verify_fastpq_replay_proof(transfer, &receipts_root, proof_replay).map_err(|err| {
                rejection_error(
                    OfflineTransferRejectionReason::AggregateProofHashError,
                    OfflineTransferRejectionPlatform::General,
                    format!("aggregate replay proof invalid: {err}"),
                )
            })?;
        }

        Ok(())
    }

    fn ensure_receipt_targets(
        receipts: &[OfflineSpendReceipt],
        transfer: &OfflineToOnlineTransfer,
        record: &OfflineAllowanceRecord,
    ) -> Result<(), Error> {
        for receipt in receipts {
            if receipt.to != transfer.receiver {
                return Err(rejection_error(
                    OfflineTransferRejectionReason::ReceiptReceiverMismatch,
                    OfflineTransferRejectionPlatform::General,
                    "receipt receiver mismatch",
                ));
            }
            if receipt.from != record.certificate.controller {
                return Err(rejection_error(
                    OfflineTransferRejectionReason::ReceiptSenderMismatch,
                    OfflineTransferRejectionPlatform::General,
                    "receipt sender does not match certificate controller",
                ));
            }
            if receipt.asset != record.certificate.allowance.asset {
                return Err(rejection_error(
                    OfflineTransferRejectionReason::ReceiptAssetMismatch,
                    OfflineTransferRejectionPlatform::General,
                    "receipt asset does not match allowance asset",
                ));
            }
            if receipt.amount > record.certificate.policy.max_tx_value.clone() {
                return Err(rejection_error(
                    OfflineTransferRejectionReason::MaxTxValueExceeded,
                    OfflineTransferRejectionPlatform::General,
                    "receipt amount exceeds policy max_tx_value",
                ));
            }
        }
        Ok(())
    }

    fn ensure_commitment_alignment(
        proof: &OfflineBalanceProof,
        record: &OfflineAllowanceRecord,
    ) -> Result<(), Error> {
        if proof.initial_commitment.asset != record.certificate.allowance.asset {
            return Err(rejection_error(
                OfflineTransferRejectionReason::BalanceAssetMismatch,
                OfflineTransferRejectionPlatform::General,
                "balance proof asset does not match allowance asset",
            ));
        }
        if proof.initial_commitment.commitment != record.current_commitment {
            return Err(rejection_error(
                OfflineTransferRejectionReason::CommitmentMismatch,
                OfflineTransferRejectionPlatform::General,
                "balance proof initial commitment does not match recorded commitment",
            ));
        }
        Ok(())
    }

    fn ensure_receipt_timestamps(
        receipts: &[OfflineSpendReceipt],
        record: &OfflineAllowanceRecord,
        block_timestamp_ms: u64,
        settlement_cfg: &actual::Offline,
    ) -> Result<(), Error> {
        let max_age_ms = duration_to_millis(settlement_cfg.max_receipt_age);
        let expiry_bound = record
            .certificate
            .expires_at_ms
            .min(record.certificate.policy.expires_at_ms);
        for receipt in receipts {
            let issued_at_ms = receipt.issued_at_ms;
            if issued_at_ms < record.certificate.issued_at_ms {
                return Err(rejection_error(
                    OfflineTransferRejectionReason::ReceiptTimestampInvalid,
                    OfflineTransferRejectionPlatform::General,
                    "receipt issued before certificate issuance",
                ));
            }
            if issued_at_ms < record.registered_at_ms {
                return Err(rejection_error(
                    OfflineTransferRejectionReason::ReceiptTimestampInvalid,
                    OfflineTransferRejectionPlatform::General,
                    "receipt issued before allowance registration",
                ));
            }
            if issued_at_ms > expiry_bound {
                return Err(rejection_error(
                    OfflineTransferRejectionReason::ReceiptTimestampInvalid,
                    OfflineTransferRejectionPlatform::General,
                    "receipt issued after certificate/policy expiry",
                ));
            }
            if issued_at_ms > block_timestamp_ms {
                return Err(rejection_error(
                    OfflineTransferRejectionReason::ReceiptTimestampInvalid,
                    OfflineTransferRejectionPlatform::General,
                    "receipt timestamp is in the future",
                ));
            }
            if max_age_ms != 0 {
                let age = block_timestamp_ms.saturating_sub(issued_at_ms);
                if age > max_age_ms {
                    return Err(rejection_error(
                        OfflineTransferRejectionReason::ReceiptExpired,
                        OfflineTransferRejectionPlatform::General,
                        "receipt exceeds max receipt age",
                    ));
                }
            }
        }
        Ok(())
    }

    fn duration_to_millis(duration: Duration) -> u64 {
        u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
    }

    fn enforce_certificate_window(
        record: &OfflineAllowanceRecord,
        block_timestamp_ms: u64,
    ) -> Result<(), Error> {
        if record.certificate.expires_at_ms <= block_timestamp_ms {
            return Err(rejection_error(
                OfflineTransferRejectionReason::CertificateExpired,
                OfflineTransferRejectionPlatform::General,
                "offline allowance certificate expired",
            ));
        }
        if record.certificate.policy.expires_at_ms <= block_timestamp_ms {
            return Err(rejection_error(
                OfflineTransferRejectionReason::PolicyExpired,
                OfflineTransferRejectionPlatform::General,
                "offline allowance policy expired",
            ));
        }
        Ok(())
    }

    fn enforce_verdict_refresh_window(
        record: &OfflineAllowanceRecord,
        block_timestamp_ms: u64,
    ) -> Result<(), Error> {
        let refresh_deadline = record.refresh_at_ms.or(record.certificate.refresh_at_ms);
        if let Some(refresh_at_ms) = refresh_deadline {
            if refresh_at_ms <= block_timestamp_ms {
                return Err(rejection_error(
                    OfflineTransferRejectionReason::VerdictExpired,
                    OfflineTransferRejectionPlatform::General,
                    "offline attestation verdict expired; refresh allowance before reconciling",
                ));
            }
        }
        Ok(())
    }

    fn derive_platform_token_snapshot(
        certificate: &OfflineWalletCertificate,
        metadata: Option<&AndroidIntegrityMetadata>,
    ) -> Option<OfflinePlatformTokenSnapshot> {
        let attestation = &certificate.attestation_report;
        if attestation.is_empty() {
            return None;
        }
        let encoded = BASE64_STANDARD.encode(attestation);
        let metadata = metadata?;
        match metadata {
            AndroidIntegrityMetadata::PlayIntegrity(_) => Some(OfflinePlatformTokenSnapshot {
                policy: AndroidIntegrityPolicy::PlayIntegrity.as_str().into(),
                attestation_jws_b64: encoded,
            }),
            AndroidIntegrityMetadata::HmsSafetyDetect(_) => Some(OfflinePlatformTokenSnapshot {
                policy: AndroidIntegrityPolicy::HmsSafetyDetect.as_str().into(),
                attestation_jws_b64: encoded,
            }),
            _ => None,
        }
    }

    fn validate_platform_snapshot(
        snapshot: Option<&OfflinePlatformTokenSnapshot>,
        metadata: Option<&AndroidIntegrityMetadata>,
    ) -> Result<Option<OfflinePlatformTokenSnapshot>, InstructionExecutionError> {
        let Some(snapshot) = snapshot else {
            return Ok(None);
        };
        let platform = OfflineTransferRejectionPlatform::Android;
        let Some(metadata) = metadata else {
            return Err(rejection_error(
                OfflineTransferRejectionReason::PlatformMetadataInvalid,
                platform,
                "platform token snapshot cannot be used without android integrity metadata",
            ));
        };
        let Some(policy) = snapshot.policy() else {
            return Err(rejection_error(
                OfflineTransferRejectionReason::PlatformMetadataInvalid,
                platform,
                "platform token snapshot contains an unknown policy",
            ));
        };
        match (policy, metadata) {
            (AndroidIntegrityPolicy::PlayIntegrity, AndroidIntegrityMetadata::PlayIntegrity(_))
            | (
                AndroidIntegrityPolicy::HmsSafetyDetect,
                AndroidIntegrityMetadata::HmsSafetyDetect(_),
            ) => Ok(Some(snapshot.clone())),
            _ => Err(rejection_error(
                OfflineTransferRejectionReason::PlatformMetadataInvalid,
                platform,
                "platform token snapshot policy does not match allowance metadata",
            )),
        }
    }

    /// Prefer a receipt-scoped attestation snapshot when available, falling back to the
    /// bundle-scoped submission. This keeps OA10 receipts compatible with wallets that
    /// attach Play Integrity or HMS Safety Detect tokens per spend.
    fn select_attestation_snapshot<'a>(
        receipt_snapshot: Option<&'a OfflinePlatformTokenSnapshot>,
        submitted_snapshot: Option<&'a OfflinePlatformTokenSnapshot>,
    ) -> Option<&'a OfflinePlatformTokenSnapshot> {
        receipt_snapshot.or(submitted_snapshot)
    }

    fn insert_transfer_record(
        state_transaction: &mut StateTransaction<'_, '_>,
        record: OfflineTransferRecord,
    ) {
        let bundle_id = record.transfer.bundle_id;
        if record.status != OfflineTransferStatus::Rejected {
            for receipt in &record.transfer.receipts {
                if let Some(claim) = &receipt.build_claim {
                    state_transaction
                        .world
                        .offline_consumed_build_claim_ids
                        .insert(claim.claim_id, ());
                }
            }
        }
        {
            let sender_index = &mut state_transaction.world.offline_transfer_sender_index;
            if let Some(set) = sender_index.get_mut(&record.controller) {
                set.insert(bundle_id);
            } else {
                let mut set = BTreeSet::new();
                set.insert(bundle_id);
                sender_index.insert(record.controller.clone(), set);
            }
        }
        {
            let receiver_index = &mut state_transaction.world.offline_transfer_receiver_index;
            if let Some(set) = receiver_index.get_mut(&record.transfer.receiver) {
                set.insert(bundle_id);
            } else {
                let mut set = BTreeSet::new();
                set.insert(bundle_id);
                receiver_index.insert(record.transfer.receiver.clone(), set);
            }
        }
        {
            let status_index = &mut state_transaction.world.offline_transfer_status_index;
            if let Some(set) = status_index.get_mut(&record.status) {
                set.insert(bundle_id);
            } else {
                let mut set = BTreeSet::new();
                set.insert(bundle_id);
                status_index.insert(record.status, set);
            }
        }
        let prev = state_transaction
            .world
            .offline_to_online_transfers
            .insert(bundle_id, record);
        assert!(
            prev.is_none(),
            "bundle id unexpectedly duplicated after pre-validation"
        );
    }

    fn credit_deposit_account(
        state_transaction: &mut StateTransaction<'_, '_>,
        asset: &AssetId,
        deposit_account: &AccountId,
        amount: &Numeric,
    ) -> Result<(), Error> {
        let definition_id = asset.definition().clone();
        let deposit_asset = AssetId::new(definition_id.clone(), deposit_account.clone());
        let spec = state_transaction.numeric_spec_for(&definition_id)?;
        assert_numeric_spec_with(amount, spec)?;
        state_transaction.world.account(deposit_account)?;
        if !amount.is_zero() {
            let current_balance = state_transaction
                .world
                .assets
                .get(&deposit_asset)
                .map(|asset| asset.as_ref().clone())
                .unwrap_or_else(Numeric::zero);
            current_balance
                .checked_add(amount.clone())
                .ok_or(MathError::Overflow)?;
        }
        let escrow_account = resolve_offline_escrow_account(state_transaction, &definition_id)?;
        let escrow_account = escrow_account.ok_or_else(|| {
            labeled_invariant(
                "escrow_missing",
                format!(
                    "offline escrow account not configured for asset definition `{definition_id}`",
                ),
            )
        })?;
        if amount.is_zero() {
            return state_transaction
                .world
                .deposit_numeric_asset(&deposit_asset, amount);
        }
        let escrow_asset = AssetId::new(definition_id, escrow_account);
        state_transaction
            .world
            .withdraw_numeric_asset(&escrow_asset, amount)?;
        if let Err(err) = state_transaction
            .world
            .deposit_numeric_asset(&deposit_asset, amount)
        {
            state_transaction
                .world
                .deposit_numeric_asset(&escrow_asset, amount)
                .expect("escrow refund must succeed after failed deposit credit");
            return Err(err);
        }
        Ok(())
    }

    fn emit_revocation_event(
        state_transaction: &mut StateTransaction<'_, '_>,
        revocation: &OfflineVerdictRevocation,
    ) {
        state_transaction
            .world
            .emit_events(Some(OfflineTransferEvent::RevocationImported(
                revocation.clone(),
            )));
    }

    fn emit_reclaim_event(
        state_transaction: &mut StateTransaction<'_, '_>,
        payload: OfflineAllowanceReclaimed,
    ) {
        state_transaction
            .world
            .emit_events(Some(OfflineTransferEvent::AllowanceReclaimed(payload)));
    }

    fn emit_transfer_event(
        state_transaction: &mut StateTransaction<'_, '_>,
        record: &OfflineTransferRecord,
        claimed_amount: Numeric,
        receipt_count: u32,
    ) {
        let asset_definition = record
            .transfer
            .receipts
            .first()
            .map(|receipt| receipt.asset.definition().clone())
            .unwrap_or_else(|| {
                record
                    .transfer
                    .balance_proof
                    .initial_commitment
                    .asset
                    .definition()
                    .clone()
            });
        let payload = OfflineTransferSettled {
            bundle_id: record.transfer.bundle_id,
            controller: record.controller.clone(),
            receiver: record.transfer.receiver.clone(),
            deposit_account: record.transfer.deposit_account.clone(),
            asset_definition,
            amount: claimed_amount,
            receipt_count,
            recorded_at_ms: record.recorded_at_ms,
            platform_snapshot: record.platform_snapshot.clone(),
        };
        state_transaction
            .world
            .emit_events(Some(OfflineTransferEvent::Settled(payload)));
    }

    fn emit_transfer_archived_event(
        state_transaction: &mut StateTransaction<'_, '_>,
        payload: OfflineTransferArchived,
    ) {
        state_transaction
            .world
            .emit_events(Some(OfflineTransferEvent::Archived(payload)));
    }

    fn emit_transfer_pruned_event(
        state_transaction: &mut StateTransaction<'_, '_>,
        payload: OfflineTransferPruned,
    ) {
        state_transaction
            .world
            .emit_events(Some(OfflineTransferEvent::Pruned(payload)));
    }

    fn record_transfer_metrics(record: &OfflineTransferRecord, claimed_amount: &Numeric) {
        let receipt_count = u32::try_from(record.transfer.receipts.len()).unwrap_or(u32::MAX);
        let metrics_handle = metrics::global_or_default();
        metrics_handle.record_offline_transfer_settlement(claimed_amount.to_f64(), receipt_count);
    }

    fn record_transfer_archived_metric() {
        metrics::global_or_default().inc_offline_transfer_archived();
    }

    fn record_transfer_pruned_metric() {
        metrics::global_or_default().inc_offline_transfer_pruned();
    }

    fn apply_transfer_retention(state_transaction: &mut StateTransaction<'_, '_>) {
        let policy = &state_transaction.settlement.offline;
        if policy.hot_retention_blocks == 0 {
            return;
        }
        if policy.archive_batch_size == 0 {
            return;
        }
        let cutoff_height = state_transaction
            .block_height()
            .saturating_sub(policy.hot_retention_blocks);
        let candidates = {
            let mut collected = Vec::new();
            for (bundle_id, record) in state_transaction.world.offline_to_online_transfers.iter() {
                if record.status == OfflineTransferStatus::Settled
                    && record.recorded_at_height <= cutoff_height
                {
                    collected.push(*bundle_id);
                    if collected.len() >= policy.archive_batch_size {
                        break;
                    }
                }
            }
            collected
        };
        if candidates.is_empty() {
            return;
        }
        let archived_at_height = state_transaction.block_height();
        let archived_at_ms = state_transaction.block_unix_timestamp_ms();
        for bundle_id in candidates {
            let archived_event = if let Some(record) = state_transaction
                .world
                .offline_to_online_transfers
                .get_mut(&bundle_id)
            {
                if record.status != OfflineTransferStatus::Settled
                    || record.recorded_at_height > cutoff_height
                {
                    None
                } else {
                    record.status = OfflineTransferStatus::Archived;
                    record.archived_at_height = Some(archived_at_height);
                    let history_snapshot = record.verdict_snapshot.clone();
                    record.push_history_entry(
                        OfflineTransferStatus::Archived,
                        archived_at_ms,
                        history_snapshot.as_ref(),
                    );
                    record_transfer_archived_metric();
                    {
                        let status_index =
                            &mut state_transaction.world.offline_transfer_status_index;
                        if let Some(set) = status_index.get_mut(&OfflineTransferStatus::Settled) {
                            set.remove(&bundle_id);
                            if set.is_empty() {
                                status_index.remove(OfflineTransferStatus::Settled);
                            }
                        }
                        if let Some(set) = status_index.get_mut(&OfflineTransferStatus::Archived) {
                            set.insert(bundle_id);
                        } else {
                            let mut set = BTreeSet::new();
                            set.insert(bundle_id);
                            status_index.insert(OfflineTransferStatus::Archived, set);
                        }
                    }
                    Some(OfflineTransferArchived {
                        bundle_id,
                        archived_at_height,
                        recorded_at_height: record.recorded_at_height,
                        archived_at_ms,
                    })
                }
            } else {
                None
            };
            if let Some(event) = archived_event {
                emit_transfer_archived_event(state_transaction, event);
            }
        }
        prune_archived_transfers(state_transaction);
    }

    fn prune_archived_transfers(state_transaction: &mut StateTransaction<'_, '_>) {
        let policy = &state_transaction.settlement.offline;
        if policy.cold_retention_blocks == 0 {
            return;
        }
        if policy.prune_batch_size == 0 {
            return;
        }
        let cutoff_height = state_transaction
            .block_height()
            .saturating_sub(policy.cold_retention_blocks);
        let mut candidates = Vec::new();
        // POS-facing revocation notifications are emitted when bundles are registered; pruning
        // here only enforces the retention window for archived transfers.
        for (bundle_id, record) in state_transaction.world.offline_to_online_transfers.iter() {
            if record.status != OfflineTransferStatus::Archived {
                continue;
            }
            if let Some(archived_at_height) = record.archived_at_height {
                if archived_at_height <= cutoff_height {
                    candidates.push((*bundle_id, archived_at_height));
                    if candidates.len() >= policy.prune_batch_size {
                        break;
                    }
                }
            }
        }
        if candidates.is_empty() {
            return;
        }
        let pruned_at_height = state_transaction.block_height();
        let pruned_at_ms = state_transaction.block_unix_timestamp_ms();
        for (bundle_id, archived_at_height) in candidates {
            if let Some(record) = state_transaction
                .world
                .offline_to_online_transfers
                .remove(bundle_id)
            {
                if let Some(set) = state_transaction
                    .world
                    .offline_transfer_sender_index
                    .get_mut(&record.controller)
                {
                    set.remove(&bundle_id);
                    if set.is_empty() {
                        state_transaction
                            .world
                            .offline_transfer_sender_index
                            .remove(record.controller.clone());
                    }
                }
                if let Some(set) = state_transaction
                    .world
                    .offline_transfer_receiver_index
                    .get_mut(&record.transfer.receiver)
                {
                    set.remove(&bundle_id);
                    if set.is_empty() {
                        state_transaction
                            .world
                            .offline_transfer_receiver_index
                            .remove(record.transfer.receiver.clone());
                    }
                }
                if let Some(set) = state_transaction
                    .world
                    .offline_transfer_status_index
                    .get_mut(&OfflineTransferStatus::Archived)
                {
                    set.remove(&bundle_id);
                    if set.is_empty() {
                        state_transaction
                            .world
                            .offline_transfer_status_index
                            .remove(OfflineTransferStatus::Archived);
                    }
                }
                record_transfer_pruned_metric();
                emit_transfer_pruned_event(
                    state_transaction,
                    OfflineTransferPruned {
                        bundle_id,
                        archived_at_height,
                        pruned_at_height,
                        pruned_at_ms,
                    },
                );
            }
        }
    }

    fn merge_counter_state(accumulator: &mut OfflineCounterState, staged: &OfflineCounterState) {
        for (key, value) in &staged.apple_key_counters {
            let entry = accumulator
                .apple_key_counters
                .entry(key.clone())
                .or_default();
            *entry = (*entry).max(*value);
        }
        for (key, value) in &staged.android_series_counters {
            let entry = accumulator
                .android_series_counters
                .entry(key.clone())
                .or_default();
            *entry = (*entry).max(*value);
        }
    }

    /// Stage counter updates for receipt platform proofs.
    pub(super) fn stage_receipt_counters(
        staged_state: &mut OfflineCounterState,
        receipts: &[OfflineSpendReceipt],
    ) -> Result<(), InstructionExecutionError> {
        for receipt in receipts {
            stage_counter_update(staged_state, &receipt.platform_proof, &receipt.tx_id)?;
        }
        Ok(())
    }

    fn stage_counter_update(
        staged_state: &mut OfflineCounterState,
        proof: &OfflinePlatformProof,
        tx_id: &Hash,
    ) -> Result<(), InstructionExecutionError> {
        match proof {
            OfflinePlatformProof::AppleAppAttest(app_attest) => {
                let key_id = canonical_app_attest_key_id(&app_attest.key_id)
                    .map_err(map_counter_scope_error)?;
                enforce_monotonic_entry(
                    staged_state.apple_key_counters.entry(key_id.clone()),
                    app_attest.counter,
                    "App Attest key",
                    key_id.as_str(),
                    tx_id,
                    OfflineTransferRejectionPlatform::Apple,
                )
            }
            OfflinePlatformProof::AndroidMarkerKey(marker) => {
                let series = marker_series_from_public_key(&marker.marker_public_key)
                    .map_err(map_counter_scope_error)?;
                enforce_monotonic_entry(
                    staged_state.android_series_counters.entry(series.clone()),
                    marker.counter,
                    "Android marker series",
                    series.as_str(),
                    tx_id,
                    OfflineTransferRejectionPlatform::Android,
                )
            }
            OfflinePlatformProof::Provisioned(provisioned) => {
                let scope = provisioned_counter_scope(provisioned)?;
                enforce_monotonic_entry(
                    staged_state.android_series_counters.entry(scope.clone()),
                    provisioned.counter,
                    "Provisioned inspector scope",
                    scope.as_str(),
                    tx_id,
                    OfflineTransferRejectionPlatform::Android,
                )
            }
        }
    }

    fn enforce_monotonic_entry(
        entry: Entry<'_, String, u64>,
        counter: u64,
        platform_label: &str,
        scope: &str,
        tx_id: &Hash,
        platform: OfflineTransferRejectionPlatform,
    ) -> Result<(), InstructionExecutionError> {
        match entry {
            Entry::Vacant(slot) => {
                slot.insert(counter);
                Ok(())
            }
            Entry::Occupied(mut slot) => {
                let previous = *slot.get();
                let expected = previous.checked_add(1).ok_or_else(|| {
                    rejection_error(
                        OfflineTransferRejectionReason::CounterViolation,
                        platform,
                        format!(
                            "{platform_label} counter saturated for scope '{scope}' (tx {tx_id:?})"
                        ),
                    )
                })?;
                if counter != expected {
                    return Err(rejection_error(
                        OfflineTransferRejectionReason::CounterViolation,
                        platform,
                        format!(
                            "{platform_label} counter jump for scope '{scope}' in tx {tx_id:?}: expected {expected}, got {counter}"
                        ),
                    ));
                }
                slot.insert(counter);
                Ok(())
            }
        }
    }

    fn verify_receipt_signature(
        receipt: &OfflineSpendReceipt,
        certificate: &OfflineWalletCertificate,
    ) -> Result<(), Error> {
        let payload = receipt.signing_bytes().map_err(|err| {
            InstructionExecutionError::InvariantViolation(
                format!("failed to encode receipt payload: {err}").into(),
            )
        })?;
        let spend_key = certificate.spend_public_key.clone();
        receipt
            .sender_signature
            .verify(&spend_key, &payload)
            .map_err(|_| {
                rejection_error(
                    OfflineTransferRejectionReason::ReceiptSignatureInvalid,
                    OfflineTransferRejectionPlatform::General,
                    "receipt signature does not match spend public key",
                )
            })
    }

    fn verify_receipt_build_claim(
        state_transaction: &StateTransaction<'_, '_>,
        receipt: &OfflineSpendReceipt,
        certificate: &OfflineWalletCertificate,
        lineage: &CertificateLineageMetadata,
        block_timestamp_ms: u64,
        claim_ids_in_bundle: &mut BTreeSet<Hash>,
    ) -> Result<(), Error> {
        if state_transaction
            .settlement
            .offline
            .skip_build_claim_verification
        {
            return Ok(());
        }
        let claim = receipt.build_claim.as_ref().ok_or_else(|| {
            rejection_error(
                OfflineTransferRejectionReason::BuildClaimMissing,
                OfflineTransferRejectionPlatform::General,
                "receipt is missing build claim",
            )
        })?;

        if !claim_ids_in_bundle.insert(claim.claim_id) {
            return Err(rejection_error(
                OfflineTransferRejectionReason::BuildClaimReplayed,
                OfflineTransferRejectionPlatform::General,
                "build claim already used in this bundle",
            )
            .into());
        }

        if is_build_claim_consumed(state_transaction, &claim.claim_id) {
            return Err(rejection_error(
                OfflineTransferRejectionReason::BuildClaimReplayed,
                OfflineTransferRejectionPlatform::General,
                "build claim already consumed by a previous transfer",
            )
            .into());
        }

        if claim.lineage_scope.trim().is_empty() {
            return Err(rejection_error(
                OfflineTransferRejectionReason::BuildClaimInvalid,
                OfflineTransferRejectionPlatform::General,
                "build claim lineage scope must not be empty",
            )
            .into());
        }
        if claim.lineage_scope.trim() != lineage.scope {
            return Err(rejection_error(
                OfflineTransferRejectionReason::BuildClaimInvalid,
                OfflineTransferRejectionPlatform::General,
                "build claim lineage scope mismatch",
            )
            .into());
        }

        if claim.nonce != receipt.tx_id {
            return Err(rejection_error(
                OfflineTransferRejectionReason::BuildClaimInvalid,
                OfflineTransferRejectionPlatform::General,
                "build claim nonce does not match receipt tx_id",
            )
            .into());
        }

        if claim.build_number < lineage.min_build_number {
            return Err(rejection_error(
                OfflineTransferRejectionReason::BuildClaimBuildTooLow,
                OfflineTransferRejectionPlatform::General,
                "build claim build number is lower than certificate minimum",
            )
            .into());
        }

        if claim.issued_at_ms > claim.expires_at_ms
            || receipt.issued_at_ms < claim.issued_at_ms
            || receipt.issued_at_ms > claim.expires_at_ms
            || block_timestamp_ms > claim.expires_at_ms
        {
            return Err(rejection_error(
                OfflineTransferRejectionReason::BuildClaimExpired,
                OfflineTransferRejectionPlatform::General,
                "build claim timestamp window is invalid or expired",
            )
            .into());
        }

        let expected_platform = match &receipt.platform_proof {
            OfflinePlatformProof::AppleAppAttest(_) => OfflineBuildClaimPlatform::Apple,
            OfflinePlatformProof::AndroidMarkerKey(_) | OfflinePlatformProof::Provisioned(_) => {
                OfflineBuildClaimPlatform::Android
            }
        };
        if claim.platform != expected_platform {
            return Err(rejection_error(
                OfflineTransferRejectionReason::BuildClaimInvalid,
                OfflineTransferRejectionPlatform::General,
                "build claim platform does not match receipt platform proof",
            )
            .into());
        }

        verify_build_claim_app_binding(claim, receipt, certificate)?;

        let operator_key = certificate.operator.try_signatory().ok_or_else(|| {
            rejection_error(
                OfflineTransferRejectionReason::BuildClaimInvalid,
                OfflineTransferRejectionPlatform::General,
                "certificate operator account must be single-signature for build claims",
            )
        })?;
        let payload = claim.signing_bytes().map_err(|err| {
            rejection_error(
                OfflineTransferRejectionReason::BuildClaimInvalid,
                OfflineTransferRejectionPlatform::General,
                format!("failed to encode build claim payload: {err}"),
            )
        })?;
        claim
            .operator_signature
            .verify(operator_key, &payload)
            .map_err(|_| {
                rejection_error(
                    OfflineTransferRejectionReason::BuildClaimInvalid,
                    OfflineTransferRejectionPlatform::General,
                    "build claim signature does not match certificate operator",
                )
            })?;

        Ok(())
    }

    fn verify_build_claim_app_binding(
        claim: &OfflineBuildClaim,
        receipt: &OfflineSpendReceipt,
        certificate: &OfflineWalletCertificate,
    ) -> Result<(), Error> {
        match &receipt.platform_proof {
            OfflinePlatformProof::AppleAppAttest(_) => {
                let ios = extract_ios_metadata(&certificate.metadata).map_err(|err| {
                    rejection_error(
                        OfflineTransferRejectionReason::BuildClaimInvalid,
                        OfflineTransferRejectionPlatform::General,
                        format!("ios metadata missing for build claim validation: {err}"),
                    )
                })?;
                if claim.app_id != ios.bundle_id {
                    return Err(rejection_error(
                        OfflineTransferRejectionReason::BuildClaimInvalid,
                        OfflineTransferRejectionPlatform::General,
                        "build claim app_id does not match ios.app_attest.bundle_id",
                    )
                    .into());
                }
            }
            OfflinePlatformProof::AndroidMarkerKey(_) => {
                let metadata =
                    android_integrity_metadata(&certificate.metadata).map_err(|err| {
                        rejection_error(
                            OfflineTransferRejectionReason::BuildClaimInvalid,
                            OfflineTransferRejectionPlatform::General,
                            format!("android metadata missing for build claim validation: {err}"),
                        )
                    })?;
                match metadata {
                    AndroidIntegrityMetadata::MarkerKey(meta) => {
                        if !meta.package_names.contains(&claim.app_id) {
                            return Err(rejection_error(
                                OfflineTransferRejectionReason::BuildClaimInvalid,
                                OfflineTransferRejectionPlatform::General,
                                "build claim app_id is not in marker-key package whitelist",
                            )
                            .into());
                        }
                    }
                    AndroidIntegrityMetadata::PlayIntegrity(meta) => {
                        if !meta.package_names.contains(&claim.app_id) {
                            return Err(rejection_error(
                                OfflineTransferRejectionReason::BuildClaimInvalid,
                                OfflineTransferRejectionPlatform::General,
                                "build claim app_id is not in play integrity package whitelist",
                            )
                            .into());
                        }
                    }
                    AndroidIntegrityMetadata::HmsSafetyDetect(meta) => {
                        if !meta.package_names.contains(&claim.app_id) {
                            return Err(rejection_error(
                                OfflineTransferRejectionReason::BuildClaimInvalid,
                                OfflineTransferRejectionPlatform::General,
                                "build claim app_id is not in hms package whitelist",
                            )
                            .into());
                        }
                    }
                    AndroidIntegrityMetadata::Provisioned(_) => {
                        return Err(rejection_error(
                            OfflineTransferRejectionReason::BuildClaimInvalid,
                            OfflineTransferRejectionPlatform::General,
                            "marker-key receipt cannot be validated against provisioned metadata",
                        )
                        .into());
                    }
                }
            }
            OfflinePlatformProof::Provisioned(_) => {
                let configured_app_id = build_claim_metadata_string(
                    &certificate.metadata,
                    ANDROID_PROVISIONED_APP_ID_KEY,
                )?;
                if claim.app_id != configured_app_id {
                    return Err(rejection_error(
                        OfflineTransferRejectionReason::BuildClaimInvalid,
                        OfflineTransferRejectionPlatform::General,
                        "build claim app_id does not match android.provisioned.app_id",
                    )
                    .into());
                }
            }
        }
        Ok(())
    }

    fn build_claim_metadata_string(metadata: &Metadata, key: &str) -> Result<String, Error> {
        let name = Name::from_str(key).map_err(|err| {
            rejection_error(
                OfflineTransferRejectionReason::BuildClaimInvalid,
                OfflineTransferRejectionPlatform::General,
                format!("invalid build claim metadata key `{key}`: {err}"),
            )
        })?;
        let value = metadata.get(&name).ok_or_else(|| {
            rejection_error(
                OfflineTransferRejectionReason::BuildClaimInvalid,
                OfflineTransferRejectionPlatform::General,
                format!("missing build claim metadata `{key}`"),
            )
        })?;
        let value = value.try_into_any::<String>().map_err(|err| {
            rejection_error(
                OfflineTransferRejectionReason::BuildClaimInvalid,
                OfflineTransferRejectionPlatform::General,
                format!("build claim metadata `{key}` is not a string: {err}"),
            )
        })?;
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(rejection_error(
                OfflineTransferRejectionReason::BuildClaimInvalid,
                OfflineTransferRejectionPlatform::General,
                format!("build claim metadata `{key}` must not be empty"),
            )
            .into());
        }
        Ok(trimmed.to_string())
    }

    fn is_build_claim_consumed(
        state_transaction: &StateTransaction<'_, '_>,
        claim_id: &Hash,
    ) -> bool {
        if state_transaction
            .world
            .offline_consumed_build_claim_ids
            .get(claim_id)
            .is_some()
        {
            return true;
        }

        // Fallback for pre-index snapshots while rolling out persisted replay tracking.
        state_transaction
            .world
            .offline_to_online_transfers
            .iter()
            .filter(|(_, record)| record.status != OfflineTransferStatus::Rejected)
            .any(|(_, record)| {
                record.transfer.receipts.iter().any(|receipt| {
                    receipt
                        .build_claim
                        .as_ref()
                        .is_some_and(|claim| &claim.claim_id == claim_id)
                })
            })
    }
}

impl ValidQuery for FindOfflineAllowances {
    fn execute(
        self,
        filter: CompoundPredicate<OfflineAllowanceRecord>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = OfflineAllowanceRecord>, QueryExecutionFail> {
        Ok(state_ro
            .world()
            .offline_allowances()
            .iter()
            .map(|(_, record)| record.clone())
            .filter(move |record| filter.applies(record)))
    }
}

impl ValidQuery for FindOfflineAllowanceByCertificateId {
    fn execute(
        self,
        filter: CompoundPredicate<OfflineAllowanceRecord>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = OfflineAllowanceRecord>, QueryExecutionFail> {
        Ok(state_ro
            .world()
            .offline_allowances()
            .get(&self.certificate_id)
            .into_iter()
            .filter(move |record| filter.applies(*record))
            .cloned())
    }
}

impl ValidQuery for FindOfflineCounterSummaries {
    fn execute(
        self,
        filter: CompoundPredicate<OfflineCounterSummary>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = OfflineCounterSummary>, QueryExecutionFail> {
        Ok(state_ro
            .world()
            .offline_allowances()
            .iter()
            .map(|(_, record)| OfflineCounterSummary::from(record))
            .filter(move |summary| filter.applies(summary)))
    }
}

impl ValidQuery for FindOfflineVerdictRevocations {
    fn execute(
        self,
        filter: CompoundPredicate<OfflineVerdictRevocation>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = OfflineVerdictRevocation>, QueryExecutionFail> {
        Ok(state_ro
            .world()
            .offline_verdict_revocations()
            .iter()
            .map(|(_, record)| record.clone())
            .filter(move |record| filter.applies(record)))
    }
}

impl ValidQuery for FindOfflineToOnlineTransfers {
    fn execute(
        self,
        filter: CompoundPredicate<OfflineTransferRecord>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = OfflineTransferRecord>, QueryExecutionFail> {
        Ok(state_ro
            .world()
            .offline_to_online_transfers()
            .iter()
            .map(|(_, transfer)| transfer.clone())
            .filter(move |transfer| filter.applies(transfer)))
    }
}

impl ValidQuery for FindOfflineToOnlineTransfersByController {
    fn execute(
        self,
        filter: CompoundPredicate<OfflineTransferRecord>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = OfflineTransferRecord>, QueryExecutionFail> {
        let world = state_ro.world();
        let bundle_ids: Vec<Hash> = world
            .offline_transfer_sender_index()
            .get(&self.controller)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default();
        Ok(bundle_ids
            .into_iter()
            .filter_map(move |bundle_id| {
                world.offline_to_online_transfers().get(&bundle_id).cloned()
            })
            .filter(move |record| filter.applies(record)))
    }
}

impl ValidQuery for FindOfflineToOnlineTransfersByReceiver {
    fn execute(
        self,
        filter: CompoundPredicate<OfflineTransferRecord>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = OfflineTransferRecord>, QueryExecutionFail> {
        let world = state_ro.world();
        let bundle_ids: Vec<Hash> = world
            .offline_transfer_receiver_index()
            .get(&self.receiver)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default();
        Ok(bundle_ids
            .into_iter()
            .filter_map(move |bundle_id| {
                world.offline_to_online_transfers().get(&bundle_id).cloned()
            })
            .filter(move |record| filter.applies(record)))
    }
}

impl ValidQuery for FindOfflineToOnlineTransfersByStatus {
    fn execute(
        self,
        filter: CompoundPredicate<OfflineTransferRecord>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = OfflineTransferRecord>, QueryExecutionFail> {
        let world = state_ro.world();
        let bundle_ids: Vec<Hash> = world
            .offline_transfer_status_index()
            .get(&self.status)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default();
        Ok(bundle_ids
            .into_iter()
            .filter_map(move |bundle_id| {
                world.offline_to_online_transfers().get(&bundle_id).cloned()
            })
            .filter(move |record| filter.applies(record)))
    }
}

impl ValidQuery for FindOfflineToOnlineTransfersByPolicy {
    fn execute(
        self,
        filter: CompoundPredicate<OfflineTransferRecord>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = OfflineTransferRecord>, QueryExecutionFail> {
        let policy = self.policy;
        Ok(state_ro
            .world()
            .offline_to_online_transfers()
            .iter()
            .filter_map(move |(_, record)| {
                record
                    .platform_snapshot
                    .as_ref()
                    .filter(|snapshot| snapshot.policy() == Some(policy))
                    .map(|_| record.clone())
            })
            .filter(move |record| filter.applies(record)))
    }
}

impl ValidQuery for FindOfflineToOnlineTransferById {
    fn execute(
        self,
        filter: CompoundPredicate<OfflineTransferRecord>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = OfflineTransferRecord>, QueryExecutionFail> {
        Ok(state_ro
            .world()
            .offline_to_online_transfers()
            .get(&self.bundle_id)
            .into_iter()
            .filter(move |transfer| filter.applies(*transfer))
            .cloned())
    }
}

mod attestation {
    use std::{str, sync::LazyLock};

    use base64::engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD};
    #[cfg(test)]
    use ciborium::ser::into_writer;
    use ciborium::{de::from_reader, value::Value};
    use der_parser::oid;
    use iroha_config::parameters::actual;
    use iroha_data_model::offline::{AndroidMarkerKeyProof, AppleAppAttestProof};
    use iroha_logger::prelude::*;
    use norito::json::{Map as JsonMap, Value as JsonValue};
    #[cfg(test)]
    use once_cell::sync::OnceCell;
    #[cfg(test)]
    use p256::ecdsa::signature::hazmat::PrehashSigner as _;
    use p256::ecdsa::{
        Signature as P256Signature, VerifyingKey,
        signature::{DigestVerifier, hazmat::PrehashVerifier as _},
    };
    #[cfg(test)]
    use p256::ecdsa::{SigningKey, signature::DigestSigner};
    #[cfg(test)]
    use p256::pkcs8::DecodePrivateKey;
    #[cfg(test)]
    use rcgen::{
        BasicConstraints, CertificateParams, CertifiedIssuer, CustomExtension, DistinguishedName,
        DnType, IsCa, KeyPair as RcgenKeyPair,
    };
    use sha2::{Digest, Sha256};
    use x509_parser::{certificate::X509Certificate, prelude::FromDer, time::ASN1Time};
    #[cfg(test)]
    use yasna::Tag;

    use super::*;
    use crate::smartcontracts::isi::error::InstructionExecutionError;

    const APPLE_NONCE_OID: oid::Oid<'static> = oid!(1.2.840.113635.100.8.2);
    #[cfg(test)]
    const APPLE_NONCE_OID_COMPONENTS: &[u64] = &[1, 2, 840, 113_635, 100, 8, 2];

    const APPLE_APP_ATTEST_ROOT_PEM: &str = "-----BEGIN CERTIFICATE-----\nMIICITCCAaegAwIBAgIQC/O+DvHN0uD7jG5yH2IXmDAKBggqhkjOPQQDAzBSMSYw\nJAYDVQQDDB1BcHBsZSBBcHAgQXR0ZXN0YXRpb24gUm9vdCBDQTETMBEGA1UECgwK\nQXBwbGUgSW5jLjETMBEGA1UECAwKQ2FsaWZvcm5pYTAeFw0yMDAzMTgxODMyNTNa\nFw00NTAzMTUwMDAwMDBaMFIxJjAkBgNVBAMMHUFwcGxlIEFwcCBBdHRlc3RhdGlv\nbiBSb290IENBMRMwEQYDVQQKDApBcHBsZSBJbmMuMRMwEQYDVQQIDApDYWxpZm9y\nbmlhMHYwEAYHKoZIzj0CAQYFK4EEACIDYgAERTHhmLW07ATaFQIEVwTtT4dyctdh\nNbJhFs/Ii2FdCgAHGbpphY3+d8qjuDngIN3WVhQUBHAoMeQ/cLiP1sOUtgjqK9au\nYen1mMEvRq9Sk3Jm5X8U62H+xTD3FE9TgS41o0IwQDAPBgNVHRMBAf8EBTADAQH/\nMB0GA1UdDgQWBBSskRBTM72+aEH/pwyp5frq5eWKoTAOBgNVHQ8BAf8EBAMCAQYw\nCgYIKoZIzj0EAwMDaAAwZQIwQgFGnByvsiVbpTKwSga0kP0e8EeDS4+sQmTvb7vn\n53O5+FRXgeLhpJ06ysC5PrOyAjEAp5U4xDgEgllF7En3VcE3iexZZtKeYnpqtijV\noyFraWVIyd/dganmrduC1bmTBGwD\n-----END CERTIFICATE-----\n";

    static APPLE_ROOT_DER: LazyLock<&'static [u8]> = LazyLock::new(|| {
        let bytes =
            decode_pem_bytes(APPLE_APP_ATTEST_ROOT_PEM).expect("apple app attest root must decode");
        Box::leak(bytes.into_boxed_slice())
    });

    static APPLE_ROOT_ANCHORS: LazyLock<Box<[&'static [u8]]>> =
        LazyLock::new(|| Box::new([*APPLE_ROOT_DER]));

    static GOOGLE_ATTESTATION_ROOT_RSA: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../certs/google_attestation_root_rsa.der"
    ));
    static GOOGLE_ATTESTATION_ROOT_ECDSA: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../certs/google_attestation_root_ecdsa.der"
    ));

    static ANDROID_ROOT_ANCHORS: LazyLock<Box<[&'static [u8]]>> =
        LazyLock::new(|| Box::new([GOOGLE_ATTESTATION_ROOT_RSA, GOOGLE_ATTESTATION_ROOT_ECDSA]));

    static GOOGLE_PLAY_INTEGRITY_ROOT: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../certs/google_play_integrity_root.der"
    ));
    static PLAY_INTEGRITY_ROOTS: LazyLock<Box<[&'static [u8]]>> =
        LazyLock::new(|| Box::new([GOOGLE_PLAY_INTEGRITY_ROOT]));

    static HUAWEI_SAFETY_DETECT_ROOT: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../certs/huawei_safety_detect_root.der"
    ));
    static HUAWEI_ROOT_ANCHORS: LazyLock<Box<[&'static [u8]]>> =
        LazyLock::new(|| Box::new([HUAWEI_SAFETY_DETECT_ROOT]));

    #[cfg(test)]
    static CUSTOM_APPLE_ROOT: OnceCell<&'static [&'static [u8]]> = OnceCell::new();
    #[cfg(test)]
    static CUSTOM_ANDROID_ROOTS: OnceCell<&'static [&'static [u8]]> = OnceCell::new();
    #[cfg(test)]
    static CUSTOM_PLAY_ROOTS: OnceCell<&'static [&'static [u8]]> = OnceCell::new();
    #[cfg(test)]
    static CUSTOM_HMS_ROOTS: OnceCell<&'static [&'static [u8]]> = OnceCell::new();

    #[cfg(test)]
    pub(super) fn register_test_apple_root(der: &'static [u8]) {
        let leaked: &'static [&'static [u8]] = Box::leak(Box::new([der]));
        let _ = CUSTOM_APPLE_ROOT.set(leaked);
    }

    #[cfg(test)]
    pub(super) fn register_test_android_root(der: &'static [u8]) {
        let leaked: &'static [&'static [u8]] = Box::leak(Box::new([der]));
        let _ = CUSTOM_ANDROID_ROOTS.set(leaked);
    }

    #[cfg(test)]
    pub(super) fn register_test_play_root(der: &'static [u8]) {
        let leaked: &'static [&'static [u8]] = Box::leak(Box::new([der]));
        let _ = CUSTOM_PLAY_ROOTS.set(leaked);
    }

    #[cfg(test)]
    pub(super) fn register_test_hms_root(der: &'static [u8]) {
        let leaked: &'static [&'static [u8]] = Box::leak(Box::new([der]));
        let _ = CUSTOM_HMS_ROOTS.set(leaked);
    }

    fn apple_trust_anchors() -> &'static [&'static [u8]] {
        #[cfg(test)]
        if let Some(custom) = CUSTOM_APPLE_ROOT.get() {
            return custom;
        }
        APPLE_ROOT_ANCHORS.as_ref()
    }

    fn builtin_android_trust_anchors() -> &'static [&'static [u8]] {
        #[cfg(test)]
        if let Some(custom) = CUSTOM_ANDROID_ROOTS.get() {
            return custom;
        }
        ANDROID_ROOT_ANCHORS.as_ref()
    }

    fn play_integrity_trust_anchors() -> &'static [&'static [u8]] {
        #[cfg(test)]
        if let Some(custom) = CUSTOM_PLAY_ROOTS.get() {
            return custom;
        }
        PLAY_INTEGRITY_ROOTS.as_ref()
    }

    fn hms_trust_anchors() -> &'static [&'static [u8]] {
        #[cfg(test)]
        if let Some(custom) = CUSTOM_HMS_ROOTS.get() {
            return custom;
        }
        HUAWEI_ROOT_ANCHORS.as_ref()
    }

    pub(crate) fn decode_snapshot_token(
        snapshot: &OfflinePlatformTokenSnapshot,
        expected_policy: AndroidIntegrityPolicy,
        platform: OfflineTransferRejectionPlatform,
    ) -> Result<Vec<u8>, InstructionExecutionError> {
        if snapshot.policy_label() != expected_policy.as_str() {
            return Err(rejection_error(
                OfflineTransferRejectionReason::PlatformMetadataInvalid,
                platform,
                "platform token snapshot policy does not match the expected platform",
            ));
        }
        BASE64_STANDARD
            .decode(snapshot.attestation_jws_b64.as_bytes())
            .map_err(|_| {
                rejection_error(
                    OfflineTransferRejectionReason::PlatformAttestationInvalid,
                    platform,
                    "platform token snapshot attestation_jws_b64 is not valid base64",
                )
            })
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn verify_platform_proof(
        receipt: &OfflineSpendReceipt,
        certificate: &OfflineWalletCertificate,
        chain_id: &ChainId,
        block_timestamp_ms: u64,
        settlement_cfg: &iroha_config::parameters::actual::Offline,
        submitted_snapshot: Option<&OfflinePlatformTokenSnapshot>,
    ) -> Result<(), InstructionExecutionError> {
        if settlement_cfg.skip_platform_attestation {
            return Ok(());
        }
        match &receipt.platform_proof {
            OfflinePlatformProof::AppleAppAttest(proof) => verify_apple_attestation(
                receipt,
                certificate,
                chain_id,
                proof,
                block_timestamp_ms,
                settlement_cfg,
            ),
            OfflinePlatformProof::AndroidMarkerKey(proof) => {
                let platform = OfflineTransferRejectionPlatform::Android;
                let metadata = android_integrity_metadata(&certificate.metadata)?;
                let challenge = map_platform_err(
                    derive_receipt_challenge(receipt, chain_id),
                    OfflineTransferRejectionReason::PlatformAttestationInvalid,
                    platform,
                )?;
                match metadata {
                    AndroidIntegrityMetadata::MarkerKey(marker) => {
                        let binding = MarkerBindingMetadata::from_marker(&marker);
                        map_platform_err(
                            verify_marker_key_attestation(
                                proof,
                                &binding,
                                &challenge,
                                block_timestamp_ms,
                                settlement_cfg,
                            ),
                            OfflineTransferRejectionReason::PlatformAttestationInvalid,
                            platform,
                        )?;
                        map_platform_err(
                            verify_marker_signature(proof, &challenge),
                            OfflineTransferRejectionReason::PlatformSignatureInvalid,
                            platform,
                        )
                    }
                    AndroidIntegrityMetadata::PlayIntegrity(play) => {
                        map_platform_err(
                            verify_play_integrity_token(
                                certificate,
                                &play,
                                block_timestamp_ms,
                                settlement_cfg,
                                submitted_snapshot,
                            ),
                            OfflineTransferRejectionReason::PlatformAttestationInvalid,
                            platform,
                        )?;
                        let binding = MarkerBindingMetadata::from_play(&play);
                        map_platform_err(
                            verify_marker_key_attestation(
                                proof,
                                &binding,
                                &challenge,
                                block_timestamp_ms,
                                settlement_cfg,
                            ),
                            OfflineTransferRejectionReason::PlatformAttestationInvalid,
                            platform,
                        )?;
                        map_platform_err(
                            verify_marker_signature(proof, &challenge),
                            OfflineTransferRejectionReason::PlatformSignatureInvalid,
                            platform,
                        )
                    }
                    AndroidIntegrityMetadata::HmsSafetyDetect(hms) => {
                        map_platform_err(
                            verify_hms_integrity_token(
                                certificate,
                                &hms,
                                block_timestamp_ms,
                                settlement_cfg,
                                submitted_snapshot,
                            ),
                            OfflineTransferRejectionReason::PlatformAttestationInvalid,
                            platform,
                        )?;
                        let binding = MarkerBindingMetadata::from_hms(&hms);
                        map_platform_err(
                            verify_marker_key_attestation(
                                proof,
                                &binding,
                                &challenge,
                                block_timestamp_ms,
                                settlement_cfg,
                            ),
                            OfflineTransferRejectionReason::PlatformAttestationInvalid,
                            platform,
                        )?;
                        map_platform_err(
                            verify_marker_signature(proof, &challenge),
                            OfflineTransferRejectionReason::PlatformSignatureInvalid,
                            platform,
                        )
                    }
                    AndroidIntegrityMetadata::Provisioned(_) => Err(rejection_error(
                        OfflineTransferRejectionReason::PlatformMetadataInvalid,
                        platform,
                        "android.integrity.policy `provisioned` cannot be used with marker-key proofs",
                    )),
                }
            }
            OfflinePlatformProof::Provisioned(proof) => {
                let platform = OfflineTransferRejectionPlatform::Android;
                let metadata = android_integrity_metadata(&certificate.metadata)?;
                match metadata {
                    AndroidIntegrityMetadata::Provisioned(config) => {
                        verify_provisioned_attestation(
                            receipt,
                            chain_id,
                            &config,
                            proof,
                            block_timestamp_ms,
                        )
                    }
                    _ => Err(rejection_error(
                        OfflineTransferRejectionReason::PlatformMetadataInvalid,
                        platform,
                        "provisioned proofs require android.provisioned metadata",
                    )),
                }
            }
        }
    }

    fn verify_apple_attestation(
        receipt: &OfflineSpendReceipt,
        certificate: &OfflineWalletCertificate,
        chain_id: &ChainId,
        proof: &AppleAppAttestProof,
        block_timestamp_ms: u64,
        settlement_cfg: &actual::Offline,
    ) -> Result<(), InstructionExecutionError> {
        let platform = OfflineTransferRejectionPlatform::Apple;
        warn!(
            "[AppAttest] verify_apple_attestation: report_len={}, block_ts={block_timestamp_ms}",
            certificate.attestation_report.len()
        );
        if certificate.attestation_report.is_empty() {
            warn!("[AppAttest] FAIL: attestation report is empty");
            return Err(rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationMissing,
                platform,
                "ios attestation report is missing",
            ));
        }

        let metadata = extract_ios_metadata(&certificate.metadata).inspect_err(|e| {
            warn!("[AppAttest] FAIL: extract_ios_metadata: {e}");
        })?;
        warn!(
            "[AppAttest] metadata: team_id={}, bundle_id={}, env={:?}",
            metadata.team_id, metadata.bundle_id, metadata.environment
        );
        let challenge = map_platform_err(
            derive_receipt_challenge(receipt, chain_id).inspect_err(|e| {
                warn!("[AppAttest] FAIL: derive_receipt_challenge: {e}");
            }),
            OfflineTransferRejectionReason::PlatformAttestationInvalid,
            platform,
        )?;
        if proof.challenge_hash != challenge.iroha_hash {
            warn!(
                "[AppAttest] FAIL: challenge hash mismatch: proof={} expected={}",
                hex::encode(proof.challenge_hash.as_ref()),
                hex::encode(challenge.iroha_hash.as_ref())
            );
            return Err(rejection_error(
                OfflineTransferRejectionReason::PlatformChallengeMismatch,
                platform,
                "app attest challenge hash mismatch",
            ));
        }

        let key_identifier = map_platform_err(
            decode_key_id(&proof.key_id).inspect_err(|e| {
                warn!("[AppAttest] FAIL: decode_key_id: {e}");
            }),
            OfflineTransferRejectionReason::PlatformMetadataInvalid,
            platform,
        )?;
        // The attestation certificate was created at top-up time with a
        // clientDataHash different from the receipt-derived challenge.
        // The certificate MUST carry attestation_nonce for verification.
        let attest_cdh: [u8; 32] = certificate
            .attestation_nonce
            .as_ref()
            .map(|n| {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(n.as_ref());
                arr
            })
            .ok_or_else(|| {
                warn!("[AppAttest] FAIL: certificate missing attestation_nonce");
                rejection_error(
                    OfflineTransferRejectionReason::PlatformAttestationInvalid,
                    platform,
                    "certificate missing attestation_nonce",
                )
            })?;
        let attestation = map_platform_err(
            AppleAttestation::from_certificate(
                &certificate.attestation_report,
                &metadata,
                &key_identifier,
                block_timestamp_ms,
                &attest_cdh,
            )
            .inspect_err(|e| {
                warn!("[AppAttest] FAIL: from_certificate: {e}");
            }),
            OfflineTransferRejectionReason::PlatformAttestationInvalid,
            platform,
        )?;
        let assertion = map_platform_err(
            AppleAssertion::parse(&proof.assertion).inspect_err(|e| {
                warn!("[AppAttest] FAIL: AppleAssertion::parse: {e}");
            }),
            OfflineTransferRejectionReason::PlatformAttestationInvalid,
            platform,
        )?;
        let client_data_hash = map_platform_err(
            assertion
                .validate(
                    &metadata,
                    proof.counter,
                    &challenge,
                    &attestation,
                    settlement_cfg,
                )
                .inspect_err(|e| {
                    warn!("[AppAttest] FAIL: assertion.validate: {e}");
                }),
            OfflineTransferRejectionReason::PlatformAttestationInvalid,
            platform,
        )?;
        map_platform_err(
            verify_apple_signature(
                attestation.verifying_key(),
                &assertion,
                &client_data_hash,
                &challenge.iroha_bytes,
                settlement_cfg,
            )
            .inspect_err(|e| {
                warn!("[AppAttest] FAIL: verify_apple_signature: {e}");
            }),
            OfflineTransferRejectionReason::PlatformSignatureInvalid,
            platform,
        )?;
        warn!("[AppAttest] verify_apple_attestation: OK");
        Ok(())
    }

    /// Inputs required to validate an App Attest assertion outside the legacy
    /// receipt/certificate flow while reusing the same verifier pipeline.
    #[derive(Debug, Clone)]
    pub struct LineageAppleAppAttestVerification {
        /// Metadata containing `ios.app_attest.*` keys.
        pub metadata: Metadata,
        /// Raw App Attest attestation object bytes.
        pub attestation_report: Vec<u8>,
        /// Stable App Attest key identifier.
        pub key_id: String,
        /// Raw App Attest assertion bytes.
        pub assertion: Vec<u8>,
        /// Expected monotonic sign counter.
        pub counter: u64,
        /// Expected challenge hash bound to the reserve operation.
        pub challenge_hash: [u8; 32],
    }

    /// Validate an App Attest assertion using the same attestation chain,
    /// nonce, counter, rpId, and signature checks as the legacy offline
    /// settlement path.
    ///
    /// The caller is responsible for deriving `challenge_hash` from the
    /// reserve operation payload in the exact same way as the client.
    pub fn verify_lineage_apple_app_attest(
        verification: &LineageAppleAppAttestVerification,
        block_timestamp_ms: u64,
        settlement_cfg: &actual::Offline,
    ) -> Result<(), InstructionExecutionError> {
        let metadata = extract_ios_metadata(&verification.metadata)?;
        let key_identifier = decode_key_id(&verification.key_id)?;
        let attestation = AppleAttestation::from_certificate(
            &verification.attestation_report,
            &metadata,
            &key_identifier,
            block_timestamp_ms,
            &verification.challenge_hash,
        )?;
        let assertion = AppleAssertion::parse(&verification.assertion)?;
        let challenge = ReceiptChallenge {
            iroha_hash: Hash::prehashed(verification.challenge_hash),
            iroha_bytes: verification.challenge_hash,
            client_data_hash: verification.challenge_hash,
        };
        let client_data_hash = assertion.validate(
            &metadata,
            verification.counter,
            &challenge,
            &attestation,
            settlement_cfg,
        )?;
        verify_apple_signature(
            attestation.verifying_key(),
            &assertion,
            &client_data_hash,
            &challenge.iroha_bytes,
            settlement_cfg,
        )
    }

    fn verify_marker_key_attestation(
        proof: &AndroidMarkerKeyProof,
        binding: &MarkerBindingMetadata,
        challenge: &ReceiptChallenge,
        block_timestamp_ms: u64,
        settlement_cfg: &actual::Offline,
    ) -> Result<(), InstructionExecutionError> {
        let platform = OfflineTransferRejectionPlatform::Android;
        if proof.attestation.is_empty() {
            return Err(rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationMissing,
                platform,
                "android marker key attestation is missing",
            ));
        }
        if binding.require_strongbox && proof.marker_signature.is_none() {
            return Err(rejection_error(
                OfflineTransferRejectionReason::PlatformSignatureMissing,
                platform,
                "android marker signature required when require_strongbox=true",
            ));
        }

        let attestation_chain = map_platform_err(
            decode_android_attestation(&proof.attestation),
            OfflineTransferRejectionReason::PlatformAttestationInvalid,
            platform,
        )?;
        let leaf_cert = map_platform_err(
            verify_android_chain(&attestation_chain, block_timestamp_ms, settlement_cfg),
            OfflineTransferRejectionReason::PlatformAttestationInvalid,
            platform,
        )?;
        let spki = leaf_cert.public_key();
        if spki.subject_public_key.data.is_empty() {
            return Err(rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationInvalid,
                platform,
                "android attestation leaf missing public key",
            ));
        }
        let leaf_key = VerifyingKey::from_sec1_bytes(spki.subject_public_key.data.as_ref())
            .map_err(|_| {
                rejection_error(
                    OfflineTransferRejectionReason::PlatformAttestationInvalid,
                    platform,
                    "android attestation leaf contains invalid P-256 key",
                )
            })?;
        let marker_key =
            VerifyingKey::from_sec1_bytes(proof.marker_public_key.as_ref()).map_err(|_| {
                rejection_error(
                    OfflineTransferRejectionReason::PlatformAttestationInvalid,
                    platform,
                    "android marker_public_key is not valid P-256 SEC1 bytes",
                )
            })?;
        if leaf_key.to_encoded_point(false).as_bytes()
            != marker_key.to_encoded_point(false).as_bytes()
        {
            return Err(rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationInvalid,
                platform,
                "android marker_public_key does not match attestation leaf key",
            ));
        }
        if let Ok(expected) = marker_series_from_public_key(&proof.marker_public_key) {
            if proof.series != expected {
                return Err(rejection_error(
                    OfflineTransferRejectionReason::PlatformAttestationInvalid,
                    platform,
                    "android marker series does not match marker_public_key",
                ));
            }
        }
        let key_description = map_platform_err(
            parse_key_description(&leaf_cert),
            OfflineTransferRejectionReason::PlatformAttestationInvalid,
            platform,
        )?;
        map_platform_err(
            validate_android_key_description(&key_description, binding, challenge),
            OfflineTransferRejectionReason::PlatformAttestationInvalid,
            platform,
        )
    }

    #[allow(clippy::too_many_lines)]
    fn verify_play_integrity_token(
        certificate: &OfflineWalletCertificate,
        metadata: &AndroidPlayIntegrityMetadata,
        block_timestamp_ms: u64,
        settlement_cfg: &actual::Offline,
        submitted_snapshot: Option<&OfflinePlatformTokenSnapshot>,
    ) -> Result<(), InstructionExecutionError> {
        let platform = OfflineTransferRejectionPlatform::Android;
        let attestation_bytes = match submitted_snapshot {
            Some(snapshot) => decode_snapshot_token(
                snapshot,
                AndroidIntegrityPolicy::PlayIntegrity,
                OfflineTransferRejectionPlatform::Android,
            )?,
            None => certificate.attestation_report.clone(),
        };
        if attestation_bytes.is_empty() {
            return Err(rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationMissing,
                platform,
                "play integrity token is missing",
            ));
        }
        let parsed = parse_jws_report(&attestation_bytes, platform, "ES256")?;
        let verifying_key = verifying_key_from_chain(
            &parsed.certificates,
            block_timestamp_ms,
            settlement_cfg,
            play_integrity_trust_anchors(),
            platform,
        )?;
        verify_es256_signature(
            &verifying_key,
            &parsed.signed_bytes,
            &parsed.signature,
            platform,
        )?;
        let payload = parsed.payload.as_object().ok_or_else(|| {
            rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationInvalid,
                platform,
                "play integrity payload must be an object",
            )
        })?;
        let request_details = json_expect_object(payload, "requestDetails", platform)?;
        let request_package = json_expect_string(request_details, "requestPackageName", platform)?;
        if !metadata.package_names.contains(request_package) {
            return Err(rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationInvalid,
                platform,
                "play integrity package is not allowed",
            ));
        }
        let nonce_b64 = json_expect_string(request_details, "nonce", platform)?;
        let nonce_bytes =
            decode_base64_url(nonce_b64).map_err(|_| invalid_attestation(platform))?;
        ensure_attestation_nonce(certificate, &nonce_bytes, platform)?;
        let issued_at = json_expect_u64(request_details, "timestampMillis", platform)?;
        ensure_token_fresh(
            issued_at,
            block_timestamp_ms,
            metadata.max_token_age_ms,
            platform,
        )?;
        let aud_value = payload
            .get("aud")
            .ok_or_else(|| invalid_attestation(platform))?;
        let audience = match aud_value {
            JsonValue::Number(num) => num.as_u64(),
            JsonValue::String(text) => text.parse().ok(),
            _ => None,
        }
        .ok_or_else(|| invalid_attestation(platform))?;
        if audience != metadata.cloud_project_number {
            return Err(rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationInvalid,
                platform,
                "play integrity audience mismatch",
            ));
        }
        let issuer = json_expect_string(payload, "iss", platform)?;
        if !play_integrity_issuers(metadata.environment).contains(&issuer) {
            return Err(rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationInvalid,
                platform,
                "play integrity issuer mismatch",
            ));
        }
        let app_integrity = json_expect_object(payload, "appIntegrity", platform)?;
        let app_package = json_expect_string(app_integrity, "packageName", platform)?;
        if app_package != request_package {
            return Err(rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationInvalid,
                platform,
                "play integrity request package mismatch",
            ));
        }
        let mut digest_ok = false;
        for entry in json_expect_array(app_integrity, "certificateSha256Digest", platform)? {
            let digest_b64 = entry
                .as_str()
                .ok_or_else(|| invalid_attestation(platform))?;
            let digest = decode_payload_digest(digest_b64).map_err(|()| {
                rejection_error(
                    OfflineTransferRejectionReason::PlatformAttestationInvalid,
                    platform,
                    "play integrity certificate digest is malformed",
                )
            })?;
            if metadata.signing_digests_sha256.contains(&digest) {
                digest_ok = true;
                break;
            }
        }
        if !digest_ok {
            return Err(rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationInvalid,
                platform,
                "play integrity signer digest not allowed",
            ));
        }
        let app_verdict = json_expect_string(app_integrity, "appRecognitionVerdict", platform)?;
        let verdict = play_app_verdict_from_payload(app_verdict)
            .ok_or_else(|| invalid_attestation(platform))?;
        if !metadata.allowed_app_verdicts.contains(&verdict) {
            return Err(rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationInvalid,
                platform,
                "play integrity app verdict rejected by policy",
            ));
        }
        let device_integrity = json_expect_object(payload, "deviceIntegrity", platform)?;
        let actual_device_verdicts =
            json_expect_array(device_integrity, "deviceRecognitionVerdict", platform)?;
        if actual_device_verdicts.is_empty() {
            return Err(invalid_attestation(platform));
        }
        for entry in actual_device_verdicts {
            let verdict_str = entry
                .as_str()
                .ok_or_else(|| invalid_attestation(platform))?;
            let mapped = play_device_verdict_from_payload(verdict_str)
                .ok_or_else(|| invalid_attestation(platform))?;
            if !metadata.allowed_device_verdicts.contains(&mapped) {
                return Err(rejection_error(
                    OfflineTransferRejectionReason::PlatformAttestationInvalid,
                    platform,
                    "play integrity device verdict rejected by policy",
                ));
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn verify_hms_integrity_token(
        certificate: &OfflineWalletCertificate,
        metadata: &AndroidHmsSafetyDetectMetadata,
        block_timestamp_ms: u64,
        settlement_cfg: &actual::Offline,
        submitted_snapshot: Option<&OfflinePlatformTokenSnapshot>,
    ) -> Result<(), InstructionExecutionError> {
        let platform = OfflineTransferRejectionPlatform::Android;
        let attestation_bytes = match submitted_snapshot {
            Some(snapshot) => decode_snapshot_token(
                snapshot,
                AndroidIntegrityPolicy::HmsSafetyDetect,
                OfflineTransferRejectionPlatform::Android,
            )?,
            None => certificate.attestation_report.clone(),
        };
        if attestation_bytes.is_empty() {
            return Err(rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationMissing,
                platform,
                "hms safety detect token is missing",
            ));
        }
        let parsed = parse_jws_report(&attestation_bytes, platform, "ES256")?;
        let verifying_key = verifying_key_from_chain(
            &parsed.certificates,
            block_timestamp_ms,
            settlement_cfg,
            hms_trust_anchors(),
            platform,
        )?;
        verify_es256_signature(
            &verifying_key,
            &parsed.signed_bytes,
            &parsed.signature,
            platform,
        )?;
        let payload = parsed.payload.as_object().ok_or_else(|| {
            rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationInvalid,
                platform,
                "safety detect payload must be an object",
            )
        })?;
        let package = json_expect_string(payload, "apkPackageName", platform)?;
        if !metadata.package_names.contains(package) {
            return Err(rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationInvalid,
                platform,
                "safety detect package not allowed",
            ));
        }
        let app_id = json_expect_string(payload, "appId", platform)?;
        if app_id != metadata.app_id {
            return Err(rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationInvalid,
                platform,
                "safety detect app id mismatch",
            ));
        }
        let nonce_b64 = json_expect_string(payload, "nonce", platform)?;
        let nonce = decode_base64_url(nonce_b64).map_err(|_| invalid_attestation(platform))?;
        ensure_attestation_nonce(certificate, &nonce, platform)?;
        let timestamp_ms = json_expect_u64(payload, "timestampMs", platform)?;
        ensure_token_fresh(
            timestamp_ms,
            block_timestamp_ms,
            metadata.max_token_age_ms,
            platform,
        )?;
        let mut digest_ok = false;
        for entry in json_expect_array(payload, "apkCertificateDigestSha256", platform)? {
            let digest_str = entry
                .as_str()
                .ok_or_else(|| invalid_attestation(platform))?;
            let digest = decode_payload_digest(digest_str).map_err(|()| {
                rejection_error(
                    OfflineTransferRejectionReason::PlatformAttestationInvalid,
                    platform,
                    "safety detect signer digest malformed",
                )
            })?;
            if metadata.signing_digests_sha256.contains(&digest) {
                digest_ok = true;
                break;
            }
        }
        if !digest_ok {
            return Err(rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationInvalid,
                platform,
                "safety detect signer digest not allowed",
            ));
        }
        let evaluations_value = json_expect_string(payload, "evaluationType", platform)?;
        let actual_evaluations = parse_hms_evaluations(evaluations_value);
        if metadata
            .required_evaluations
            .iter()
            .any(|required| !actual_evaluations.contains(required))
        {
            return Err(rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationInvalid,
                platform,
                "safety detect evaluation missing required level",
            ));
        }
        Ok(())
    }

    fn verify_provisioned_attestation(
        receipt: &OfflineSpendReceipt,
        chain_id: &ChainId,
        metadata: &AndroidProvisionedMetadata,
        proof: &AndroidProvisionedProof,
        block_timestamp_ms: u64,
    ) -> Result<(), InstructionExecutionError> {
        let platform = OfflineTransferRejectionPlatform::Android;
        let schema = proof.manifest_schema.trim();
        if schema != metadata.manifest_schema {
            return Err(rejection_error(
                OfflineTransferRejectionReason::PlatformMetadataInvalid,
                platform,
                "provisioned manifest schema does not match certificate metadata",
            ));
        }
        if let Some(expected) = metadata.manifest_version {
            match proof.manifest_version {
                Some(actual) if actual == expected => {}
                Some(_) => {
                    return Err(rejection_error(
                        OfflineTransferRejectionReason::PlatformMetadataInvalid,
                        platform,
                        "provisioned manifest version mismatch",
                    ));
                }
                None => {
                    return Err(rejection_error(
                        OfflineTransferRejectionReason::PlatformMetadataInvalid,
                        platform,
                        "provisioned manifest missing required version",
                    ));
                }
            }
        }
        if block_timestamp_ms < proof.manifest_issued_at_ms {
            return Err(rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationInvalid,
                platform,
                "provisioned manifest timestamp is in the future",
            ));
        }
        if let Some(max_age) = metadata.max_manifest_age_ms {
            if block_timestamp_ms.saturating_sub(proof.manifest_issued_at_ms) > max_age {
                return Err(rejection_error(
                    OfflineTransferRejectionReason::PlatformAttestationInvalid,
                    platform,
                    "provisioned manifest expired",
                ));
            }
        }
        let challenge = map_platform_err(
            derive_receipt_challenge(receipt, chain_id),
            OfflineTransferRejectionReason::PlatformAttestationInvalid,
            platform,
        )?;
        if proof.challenge_hash != challenge.iroha_hash {
            return Err(rejection_error(
                OfflineTransferRejectionReason::PlatformChallengeMismatch,
                platform,
                "provisioned challenge hash mismatch",
            ));
        }
        if let Some(expected_digest) = metadata.manifest_digest {
            let actual_digest = proof.manifest_digest().map_err(|err| {
                rejection_error(
                    OfflineTransferRejectionReason::PlatformAttestationInvalid,
                    platform,
                    format!("failed to encode provisioned manifest: {err}"),
                )
            })?;
            if actual_digest != expected_digest {
                return Err(rejection_error(
                    OfflineTransferRejectionReason::PlatformAttestationInvalid,
                    platform,
                    "provisioned manifest digest mismatch",
                ));
            }
        }
        // Ensure the manifest includes the required device identifier.
        let _ = provisioned_device_id(&proof.device_manifest)?;
        let payload = proof.signing_bytes().map_err(|err| {
            rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationInvalid,
                platform,
                format!("failed to encode provisioned manifest payload: {err}"),
            )
        })?;
        proof
            .inspector_signature
            .verify(&metadata.inspector_public_key, &payload)
            .map_err(|_| {
                rejection_error(
                    OfflineTransferRejectionReason::PlatformSignatureInvalid,
                    platform,
                    "provisioned inspector signature invalid",
                )
            })
    }

    fn parse_key_description<'a>(
        cert: &'a X509Certificate<'a>,
    ) -> Result<KeyDescription, InstructionExecutionError> {
        let extensions = cert.extensions();
        if extensions.is_empty() {
            return Err(InstructionExecutionError::InvariantViolation(
                "android attestation certificate is missing extensions".into(),
            ));
        }
        let key_desc_oid = oid!(1.3.6.1.4.1.11129.2.1.17);
        let ext = extensions
            .iter()
            .find(|ext| ext.oid == key_desc_oid)
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    "android attestation certificate does not contain keyDescription extension"
                        .into(),
                )
            })?;
        let mut reader = DerReader::new(ext.value);
        let attestation_version = reader.read_integer("attestationVersion")?;
        if attestation_version == 0 {
            return Err(InstructionExecutionError::InvariantViolation(
                "android attestationVersion must be positive".into(),
            ));
        }
        let attestation_security_level =
            SecurityLevel::try_from(reader.read_enumerated("attestationSecurityLevel")?)?;
        let keymaster_version = reader.read_integer("keymasterVersion")?;
        if keymaster_version == 0 {
            return Err(InstructionExecutionError::InvariantViolation(
                "android keymasterVersion must be positive".into(),
            ));
        }
        let keymaster_security_level =
            SecurityLevel::try_from(reader.read_enumerated("keymasterSecurityLevel")?)?;
        let attestation_challenge = reader.read_octet_string("attestationChallenge")?.to_vec();
        let _unique_id = reader.read_octet_string("uniqueId")?;
        let software_bytes = reader.read_sequence_bytes("softwareEnforced")?;
        let tee_bytes = reader.read_sequence_bytes("teeEnforced")?;
        let strongbox_bytes = if reader.has_remaining() {
            Some(reader.read_sequence_bytes("strongBoxEnforced")?)
        } else {
            None
        };
        if reader.has_remaining() {
            return Err(InstructionExecutionError::InvariantViolation(
                "android keyDescription contained trailing data".into(),
            ));
        }
        let software = parse_authorization_list(software_bytes)?;
        let tee = parse_authorization_list(tee_bytes)?;
        let strongbox = if let Some(bytes) = strongbox_bytes {
            Some(parse_authorization_list(bytes)?)
        } else {
            None
        };
        Ok(KeyDescription {
            attestation_security_level,
            keymaster_security_level,
            attestation_challenge,
            software,
            tee,
            strongbox,
        })
    }

    fn parse_authorization_list(
        data: &[u8],
    ) -> Result<AuthorizationList, InstructionExecutionError> {
        let mut cursor = DerReader::new(data);
        let mut list = AuthorizationList::default();
        while cursor.has_remaining() {
            let tlv = cursor.read_tlv()?;
            if tlv.class != TagClass::ContextSpecific {
                return Err(InstructionExecutionError::InvariantViolation(
                    "authorization entry must use a context-specific tag".into(),
                ));
            }
            match tlv.tag {
                KM_TAG_PURPOSE => {
                    list.purposes = parse_set_of_integers(tlv.value, "purpose")?;
                }
                KM_TAG_ALGORITHM => {
                    list.algorithm = Some(parse_explicit_integer(tlv.value, "algorithm")?);
                }
                KM_TAG_KEY_SIZE => {
                    list.key_size_bits = Some(parse_explicit_integer(tlv.value, "keySize")?);
                }
                KM_TAG_EC_CURVE => {
                    list.ec_curve = Some(parse_explicit_integer(tlv.value, "ecCurve")?);
                }
                KM_TAG_ORIGIN => {
                    list.origin = Some(parse_explicit_integer(tlv.value, "origin")?);
                }
                KM_TAG_ATTESTATION_APPLICATION_ID => {
                    list.attestation_app_id = Some(parse_attestation_application_id(tlv.value)?);
                }
                KM_TAG_ROLLBACK_RESISTANCE => {
                    list.rollback_resistance =
                        parse_explicit_bool(tlv.value, "rollbackResistance")?;
                }
                KM_TAG_ALL_APPLICATIONS => {
                    list.all_applications = parse_explicit_bool(tlv.value, "allApplications")?;
                }
                KM_TAG_ROOT_OF_TRUST => {
                    list.root_of_trust = Some(parse_root_of_trust(tlv.value)?);
                }
                _ => {}
            }
        }
        Ok(list)
    }

    fn parse_attestation_application_id(
        value: &[u8],
    ) -> Result<AttestationApplicationId, InstructionExecutionError> {
        let mut reader = DerReader::new(value);
        let seq =
            reader.expect_universal(TagClass::Universal, true, 16, "attestationApplicationId")?;
        let mut seq_reader = DerReader::new(seq);
        let packages_set =
            seq_reader.expect_universal(TagClass::Universal, true, 17, "packageInfos")?;
        let mut package_reader = DerReader::new(packages_set);
        let mut package_names = Vec::new();
        while package_reader.has_remaining() {
            let entry =
                package_reader.expect_universal(TagClass::Universal, true, 16, "packageInfo")?;
            let mut entry_reader = DerReader::new(entry);
            let name_bytes =
                entry_reader.expect_universal(TagClass::Universal, false, 4, "packageName")?;
            let name = String::from_utf8(name_bytes.to_vec()).map_err(|_| {
                InstructionExecutionError::InvariantViolation(
                    "package_name must be valid UTF-8".into(),
                )
            })?;
            package_names.push(name);
            let _version =
                entry_reader.expect_universal(TagClass::Universal, false, 2, "packageVersion")?;
            if entry_reader.has_remaining() {
                return Err(InstructionExecutionError::InvariantViolation(
                    "packageInfo contained trailing data".into(),
                ));
            }
        }

        let digests_set =
            seq_reader.expect_universal(TagClass::Universal, true, 17, "signature_digests")?;
        let mut digests_reader = DerReader::new(digests_set);
        let mut signature_digests = Vec::new();
        while digests_reader.has_remaining() {
            let digest = digests_reader.expect_universal(
                TagClass::Universal,
                false,
                4,
                "signatureDigest",
            )?;
            signature_digests.push(digest.to_vec());
        }
        if seq_reader.has_remaining() {
            return Err(InstructionExecutionError::InvariantViolation(
                "attestationApplicationId contained trailing fields".into(),
            ));
        }
        if package_names.is_empty() || signature_digests.is_empty() {
            return Err(InstructionExecutionError::InvariantViolation(
                "attestationApplicationId must include at least one package and digest".into(),
            ));
        }
        if reader.has_remaining() {
            return Err(InstructionExecutionError::InvariantViolation(
                "attestationApplicationId wrapper contained trailing data".into(),
            ));
        }
        Ok(AttestationApplicationId {
            package_names,
            signature_digests,
        })
    }

    fn parse_root_of_trust(value: &[u8]) -> Result<RootOfTrust, InstructionExecutionError> {
        let mut reader = DerReader::new(value);
        let seq = reader.expect_universal(TagClass::Universal, true, 16, "rootOfTrust")?;
        let mut seq_reader = DerReader::new(seq);
        let _verified_boot_key =
            seq_reader.expect_universal(TagClass::Universal, false, 4, "verifiedBootKey")?;
        let locked_bytes =
            seq_reader.expect_universal(TagClass::Universal, false, 1, "deviceLocked")?;
        let device_locked = parse_bool(locked_bytes, "deviceLocked")?;
        let verified_state = parse_unsigned_integer_bytes(
            seq_reader.expect_universal(TagClass::Universal, false, 10, "verifiedBootState")?,
            "verifiedBootState",
        )?;
        if seq_reader.has_remaining() {
            let _ =
                seq_reader.expect_universal(TagClass::Universal, false, 4, "verifiedBootHash")?;
        }
        if seq_reader.has_remaining() {
            return Err(InstructionExecutionError::InvariantViolation(
                "rootOfTrust contained trailing data".into(),
            ));
        }
        if reader.has_remaining() {
            return Err(InstructionExecutionError::InvariantViolation(
                "rootOfTrust wrapper contained trailing data".into(),
            ));
        }
        Ok(RootOfTrust {
            device_locked,
            verified_boot_state: verified_state.try_into().map_err(|_| {
                InstructionExecutionError::InvariantViolation(
                    "verifiedBootState exceeds supported range".into(),
                )
            })?,
        })
    }

    fn parse_set_of_integers(
        value: &[u8],
        label: &str,
    ) -> Result<Vec<u32>, InstructionExecutionError> {
        let mut reader = DerReader::new(value);
        let set = reader.expect_universal(TagClass::Universal, true, 17, label)?;
        let mut set_reader = DerReader::new(set);
        let mut values = Vec::new();
        while set_reader.has_remaining() {
            let bytes = set_reader.expect_universal(TagClass::Universal, false, 2, label)?;
            let num = parse_unsigned_integer_bytes(bytes, label)?;
            values.push(num.try_into().map_err(|_| {
                InstructionExecutionError::InvariantViolation(
                    format!("`{label}` value does not fit in u32").into(),
                )
            })?);
        }
        if reader.has_remaining() {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("`{label}` contained trailing data").into(),
            ));
        }
        Ok(values)
    }

    fn parse_explicit_integer(value: &[u8], label: &str) -> Result<u32, InstructionExecutionError> {
        let mut reader = DerReader::new(value);
        let num = reader.read_integer(label)?;
        if reader.has_remaining() {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("`{label}` contained trailing data").into(),
            ));
        }
        num.try_into().map_err(|_| {
            InstructionExecutionError::InvariantViolation(
                format!("`{label}` exceeds supported range").into(),
            )
        })
    }

    fn parse_explicit_bool(value: &[u8], label: &str) -> Result<bool, InstructionExecutionError> {
        if value.is_empty() {
            return Ok(true);
        }
        let mut reader = DerReader::new(value);
        let bytes = reader.expect_universal(TagClass::Universal, false, 1, label)?;
        if reader.has_remaining() {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("`{label}` contained trailing data").into(),
            ));
        }
        parse_bool(bytes, label)
    }

    fn parse_unsigned_integer_bytes(
        bytes: &[u8],
        label: &str,
    ) -> Result<u64, InstructionExecutionError> {
        if bytes.is_empty() {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("`{label}` is missing its integer payload").into(),
            ));
        }
        if bytes.len() > 9 {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("`{label}` integer is too large").into(),
            ));
        }
        let mut value = 0u64;
        for &b in bytes {
            value = (value << 8) | u64::from(b);
        }
        Ok(value)
    }

    fn parse_bool(bytes: &[u8], label: &str) -> Result<bool, InstructionExecutionError> {
        if bytes.len() != 1 {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("`{label}` must be a single-byte boolean").into(),
            ));
        }
        Ok(bytes[0] != 0)
    }

    struct KeyDescription {
        attestation_security_level: SecurityLevel,
        keymaster_security_level: SecurityLevel,
        attestation_challenge: Vec<u8>,
        software: AuthorizationList,
        tee: AuthorizationList,
        strongbox: Option<AuthorizationList>,
    }

    #[derive(Default, Clone)]
    struct AuthorizationList {
        purposes: Vec<u32>,
        algorithm: Option<u32>,
        key_size_bits: Option<u32>,
        ec_curve: Option<u32>,
        origin: Option<u32>,
        attestation_app_id: Option<AttestationApplicationId>,
        rollback_resistance: bool,
        all_applications: bool,
        root_of_trust: Option<RootOfTrust>,
    }

    #[derive(Clone)]
    struct AttestationApplicationId {
        package_names: Vec<String>,
        signature_digests: Vec<Vec<u8>>,
    }

    #[derive(Clone)]
    struct RootOfTrust {
        device_locked: bool,
        verified_boot_state: u32,
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum SecurityLevel {
        Software,
        TrustedEnvironment,
        StrongBox,
    }

    impl TryFrom<u64> for SecurityLevel {
        type Error = InstructionExecutionError;

        fn try_from(value: u64) -> Result<Self, Self::Error> {
            let level = u32::try_from(value).map_err(|_| {
                InstructionExecutionError::InvariantViolation(
                    format!("unknown android security level `{value}`").into(),
                )
            })?;
            match level {
                KM_SECURITY_LEVEL_SOFTWARE => Ok(SecurityLevel::Software),
                KM_SECURITY_LEVEL_TRUSTED_ENVIRONMENT => Ok(SecurityLevel::TrustedEnvironment),
                KM_SECURITY_LEVEL_STRONG_BOX => Ok(SecurityLevel::StrongBox),
                other => Err(InstructionExecutionError::InvariantViolation(
                    format!("unknown android security level `{other}`").into(),
                )),
            }
        }
    }

    struct DerReader<'a> {
        data: &'a [u8],
        offset: usize,
    }

    impl<'a> DerReader<'a> {
        fn new(data: &'a [u8]) -> Self {
            Self { data, offset: 0 }
        }

        fn has_remaining(&self) -> bool {
            self.offset < self.data.len()
        }

        fn read_integer(&mut self, label: &str) -> Result<u64, InstructionExecutionError> {
            let value = self.expect_universal(TagClass::Universal, false, 2, label)?;
            parse_unsigned_integer_bytes(value, label)
        }

        fn read_enumerated(&mut self, label: &str) -> Result<u64, InstructionExecutionError> {
            let value = self.expect_universal(TagClass::Universal, false, 10, label)?;
            parse_unsigned_integer_bytes(value, label)
        }

        fn read_octet_string(
            &mut self,
            label: &str,
        ) -> Result<&'a [u8], InstructionExecutionError> {
            self.expect_universal(TagClass::Universal, false, 4, label)
        }

        fn read_sequence_bytes(
            &mut self,
            label: &str,
        ) -> Result<&'a [u8], InstructionExecutionError> {
            self.expect_universal(TagClass::Universal, true, 16, label)
        }

        fn expect_universal(
            &mut self,
            class: TagClass,
            constructed: bool,
            tag: u32,
            label: &str,
        ) -> Result<&'a [u8], InstructionExecutionError> {
            let tlv = self.read_tlv()?;
            if tlv.class != class || tlv.constructed != constructed || tlv.tag != tag {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!("unexpected DER tag while parsing `{label}`").into(),
                ));
            }
            Ok(tlv.value)
        }

        fn read_tlv(&mut self) -> Result<Tlv<'a>, InstructionExecutionError> {
            if self.offset >= self.data.len() {
                return Err(InstructionExecutionError::InvariantViolation(
                    "unexpected end of DER input".into(),
                ));
            }
            let tag_byte = self.data[self.offset];
            self.offset += 1;
            let class = match tag_byte >> 6 {
                0 => TagClass::Universal,
                1 => TagClass::Application,
                2 => TagClass::ContextSpecific,
                _ => TagClass::Private,
            };
            let constructed = (tag_byte & 0x20) != 0;
            let mut tag_number = u32::from(tag_byte & 0x1F);
            if tag_number == 0x1F {
                tag_number = 0;
                loop {
                    if self.offset >= self.data.len() {
                        return Err(InstructionExecutionError::InvariantViolation(
                            "invalid DER tag encoding".into(),
                        ));
                    }
                    let byte = self.data[self.offset];
                    self.offset += 1;
                    tag_number = (tag_number << 7) | u32::from(byte & 0x7F);
                    if byte & 0x80 == 0 {
                        break;
                    }
                }
            }
            let length = self.read_length()?;
            if self.offset + length > self.data.len() {
                return Err(InstructionExecutionError::InvariantViolation(
                    "DER value exceeds available input".into(),
                ));
            }
            let value = &self.data[self.offset..self.offset + length];
            self.offset += length;
            Ok(Tlv {
                class,
                constructed,
                tag: tag_number,
                value,
            })
        }

        fn read_length(&mut self) -> Result<usize, InstructionExecutionError> {
            if self.offset >= self.data.len() {
                return Err(InstructionExecutionError::InvariantViolation(
                    "invalid DER length encoding".into(),
                ));
            }
            let first = self.data[self.offset];
            self.offset += 1;
            if first & 0x80 == 0 {
                return Ok(first as usize);
            }
            let octets = (first & 0x7F) as usize;
            if octets == 0 || octets > 4 {
                return Err(InstructionExecutionError::InvariantViolation(
                    "unsupported DER length encoding".into(),
                ));
            }
            if self.offset + octets > self.data.len() {
                return Err(InstructionExecutionError::InvariantViolation(
                    "invalid DER length encoding".into(),
                ));
            }
            let mut length = 0usize;
            for _ in 0..octets {
                length = (length << 8) | self.data[self.offset] as usize;
                self.offset += 1;
            }
            Ok(length)
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum TagClass {
        Universal,
        Application,
        ContextSpecific,
        Private,
    }

    struct Tlv<'a> {
        class: TagClass,
        constructed: bool,
        tag: u32,
        value: &'a [u8],
    }

    #[allow(clippy::too_many_lines)]
    fn validate_android_key_description(
        desc: &KeyDescription,
        meta: &MarkerBindingMetadata,
        challenge: &ReceiptChallenge,
    ) -> Result<(), InstructionExecutionError> {
        if desc.attestation_challenge.len() != challenge.iroha_bytes.len()
            || desc.attestation_challenge != challenge.iroha_bytes
        {
            return Err(InstructionExecutionError::InvariantViolation(
                "android attestation challenge mismatch".into(),
            ));
        }
        if desc.attestation_security_level == SecurityLevel::Software
            || desc.keymaster_security_level == SecurityLevel::Software
        {
            return Err(InstructionExecutionError::InvariantViolation(
                "android attestation must not use software-only security".into(),
            ));
        }
        if meta.require_strongbox && desc.attestation_security_level != SecurityLevel::StrongBox {
            return Err(InstructionExecutionError::InvariantViolation(
                "strongbox attestation required by policy".into(),
            ));
        }

        let mut lists = Vec::new();
        lists.push(&desc.software);
        lists.push(&desc.tee);
        if let Some(sb) = desc.strongbox.as_ref() {
            lists.push(sb);
        }
        if lists.iter().any(|list| list.all_applications) {
            return Err(InstructionExecutionError::InvariantViolation(
                "android attestation key must be bound to an application ID".into(),
            ));
        }

        let attestation_app_id = desc
            .software
            .attestation_app_id
            .as_ref()
            .or(desc.tee.attestation_app_id.as_ref())
            .or_else(|| {
                desc.strongbox
                    .as_ref()
                    .and_then(|s| s.attestation_app_id.as_ref())
            })
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    "android attestation missing attestationApplicationId".into(),
                )
            })?;
        if !attestation_app_id
            .package_names
            .iter()
            .any(|pkg| meta.package_names.contains(pkg))
        {
            return Err(InstructionExecutionError::InvariantViolation(
                "android attestation package is not allowed".into(),
            ));
        }
        if !attestation_app_id
            .signature_digests
            .iter()
            .any(|digest| meta.signing_digests.contains(digest))
        {
            return Err(InstructionExecutionError::InvariantViolation(
                "android attestation signing digest is not allowed".into(),
            ));
        }

        let purpose_ok = lists
            .iter()
            .any(|list| list.purposes.contains(&KM_PURPOSE_SIGN));
        if !purpose_ok {
            return Err(InstructionExecutionError::InvariantViolation(
                "android attestation key must permit SIGN purpose".into(),
            ));
        }

        let algorithm = first_present([
            desc.software.algorithm,
            desc.tee.algorithm,
            desc.strongbox.as_ref().and_then(|s| s.algorithm),
        ])
        .ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                "android attestation missing algorithm field".into(),
            )
        })?;
        if algorithm != KM_ALGORITHM_EC {
            return Err(InstructionExecutionError::InvariantViolation(
                "android attestation must use EC keys".into(),
            ));
        }

        let key_size = first_present([
            desc.software.key_size_bits,
            desc.tee.key_size_bits,
            desc.strongbox.as_ref().and_then(|s| s.key_size_bits),
        ])
        .ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                "android attestation missing keySize".into(),
            )
        })?;
        if key_size != 256 {
            return Err(InstructionExecutionError::InvariantViolation(
                "android attestation key must be 256 bits".into(),
            ));
        }

        let curve = first_present([
            desc.software.ec_curve,
            desc.tee.ec_curve,
            desc.strongbox.as_ref().and_then(|s| s.ec_curve),
        ])
        .ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                "android attestation missing ecCurve".into(),
            )
        })?;
        if curve != KM_EC_CURVE_P256 {
            return Err(InstructionExecutionError::InvariantViolation(
                "android attestation ecCurve must be P-256".into(),
            ));
        }

        let origin = first_present([
            desc.software.origin,
            desc.tee.origin,
            desc.strongbox.as_ref().and_then(|s| s.origin),
        ])
        .ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                "android attestation missing origin field".into(),
            )
        })?;
        if origin != KM_ORIGIN_GENERATED {
            return Err(InstructionExecutionError::InvariantViolation(
                "android attestation must use generated keys".into(),
            ));
        }

        let has_rr = lists.iter().any(|list| list.rollback_resistance);
        if meta.require_rollback_resistance && !has_rr {
            return Err(InstructionExecutionError::InvariantViolation(
                "android attestation must be rollback-resistant".into(),
            ));
        }

        let root = desc
            .strongbox
            .as_ref()
            .and_then(|list| list.root_of_trust.clone())
            .or_else(|| desc.tee.root_of_trust.clone())
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    "android attestation missing rootOfTrust".into(),
                )
            })?;
        if !root.device_locked {
            return Err(InstructionExecutionError::InvariantViolation(
                "android device must be locked".into(),
            ));
        }
        if root.verified_boot_state != KM_VERIFIED_BOOT_STATE_VERIFIED {
            return Err(InstructionExecutionError::InvariantViolation(
                "android verifiedBootState must be Verified".into(),
            ));
        }

        Ok(())
    }

    fn first_present<T: Copy>(values: [Option<T>; 3]) -> Option<T> {
        values.into_iter().flatten().next()
    }

    fn verify_marker_signature(
        proof: &AndroidMarkerKeyProof,
        challenge: &ReceiptChallenge,
    ) -> Result<(), InstructionExecutionError> {
        if let Some(signature) = &proof.marker_signature {
            let marker_key = VerifyingKey::from_sec1_bytes(proof.marker_public_key.as_ref())
                .map_err(|_| {
                    InstructionExecutionError::InvariantViolation(
                        "android marker_public_key is not valid P-256 SEC1 bytes".into(),
                    )
                })?;
            let sig = P256Signature::from_slice(signature).map_err(|_| {
                InstructionExecutionError::InvariantViolation(
                    "android marker_signature must be a 64-byte raw signature".into(),
                )
            })?;
            marker_key
                .verify_prehash(challenge.client_data_hash.as_ref(), &sig)
                .map_err(|_| {
                    InstructionExecutionError::InvariantViolation(
                        "android marker signature does not match marker_public_key".into(),
                    )
                })?;
        }
        Ok(())
    }

    pub(super) struct AppleAttestation {
        verifying_key: VerifyingKey,
        rp_id_hash: [u8; 32],
    }

    impl AppleAttestation {
        /// `attest_client_data_hash` — the clientDataHash that was passed to
        /// `DCAppAttestService.attestKey()` at registration (top-up) time.
        pub(super) fn from_certificate(
            report: &[u8],
            metadata: &IosMetadata,
            key_identifier: &[u8],
            block_timestamp_ms: u64,
            attest_client_data_hash: &[u8; 32],
        ) -> Result<Self, InstructionExecutionError> {
            let attestation_object = decode_attestation_object(report).inspect_err(|e| {
                warn!("[AppAttest] FAIL: decode_attestation_object: {e}");
            })?;
            warn!(
                "[AppAttest] from_certificate: certs={}, auth_data_len={}",
                attestation_object.certificates.len(),
                attestation_object.auth_data.len()
            );
            let auth_data = parse_attestation_auth_data(&attestation_object.auth_data)
                .inspect_err(|e| {
                    warn!("[AppAttest] FAIL: parse_attestation_auth_data: {e}");
                })?;

            if auth_data.sign_count != 0 {
                warn!(
                    "[AppAttest] FAIL: sign_count={} (expected 0)",
                    auth_data.sign_count
                );
                return Err(InstructionExecutionError::InvariantViolation(
                    "app attest registration counter must start at zero".into(),
                ));
            }

            let expected_aaguid = expected_aaguid(metadata.environment);
            if auth_data.aaguid != *expected_aaguid {
                warn!(
                    "[AppAttest] FAIL: aaguid mismatch: got={} expected={} env={:?}",
                    hex::encode(&auth_data.aaguid),
                    hex::encode(expected_aaguid),
                    metadata.environment
                );
                return Err(InstructionExecutionError::InvariantViolation(
                    "app attest aaguid does not match certificate environment".into(),
                ));
            }

            if auth_data.credential_id != key_identifier {
                warn!(
                    "[AppAttest] FAIL: credential_id mismatch: got_len={} expected_len={}",
                    auth_data.credential_id.len(),
                    key_identifier.len()
                );
                return Err(InstructionExecutionError::InvariantViolation(
                    "app attest credential id does not match proof key id".into(),
                ));
            }

            let expected_rp_hash = expected_rp_id_hash(metadata);
            if auth_data.rp_id_hash != expected_rp_hash {
                warn!(
                    "[AppAttest] FAIL: rpIdHash mismatch: got={} expected={} (team={}.bundle={})",
                    hex::encode(auth_data.rp_id_hash),
                    hex::encode(expected_rp_hash),
                    metadata.team_id,
                    metadata.bundle_id
                );
                return Err(InstructionExecutionError::InvariantViolation(
                    "app attest rpIdHash does not match declared metadata".into(),
                ));
            }

            let verifying_key =
                verify_attestation_chain(&attestation_object.certificates, block_timestamp_ms)
                    .inspect_err(|e| {
                        warn!("[AppAttest] FAIL: verify_attestation_chain: {e}");
                    })?;
            verify_attestation_nonce(
                &attestation_object.certificates[0],
                &attestation_object.auth_data,
                attest_client_data_hash,
            )
            .inspect_err(|e| {
                warn!("[AppAttest] FAIL: verify_attestation_nonce: {e}");
            })?;

            warn!("[AppAttest] from_certificate: OK");
            Ok(Self {
                verifying_key,
                rp_id_hash: expected_rp_hash,
            })
        }

        pub(super) fn verifying_key(&self) -> &VerifyingKey {
            &self.verifying_key
        }
    }

    struct AttestationObject {
        auth_data: Vec<u8>,
        certificates: Vec<Vec<u8>>,
    }

    fn decode_attestation_object(
        report: &[u8],
    ) -> Result<AttestationObject, InstructionExecutionError> {
        let value: Value = from_reader(report).map_err(|err| {
            InstructionExecutionError::InvariantViolation(
                format!("failed to decode app attest CBOR: {err}").into(),
            )
        })?;

        let map = match value {
            Value::Map(map) => map,
            _ => {
                return Err(InstructionExecutionError::InvariantViolation(
                    "attestation report must be a CBOR map".into(),
                ));
            }
        };

        let fmt = expect_text(&map, "fmt")?;
        if fmt != "apple-appattest" {
            return Err(InstructionExecutionError::InvariantViolation(
                "unexpected app attest format identifier".into(),
            ));
        }

        let auth_data = expect_bytes(&map, "authData")?;
        let att_stmt = expect_map(&map, "attStmt")?;
        let certificates = expect_bytes_array(att_stmt, "x5c")?;

        Ok(AttestationObject {
            auth_data,
            certificates,
        })
    }

    fn expect_entry<'a>(
        map: &'a [(Value, Value)],
        key: &str,
    ) -> Result<&'a Value, InstructionExecutionError> {
        map.iter()
            .find(|(candidate, _)| match candidate {
                Value::Text(text) => text == key,
                _ => false,
            })
            .map(|(_, value)| value)
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!("attestation report missing `{key}` entry").into(),
                )
            })
    }

    fn expect_text(map: &[(Value, Value)], key: &str) -> Result<String, InstructionExecutionError> {
        match expect_entry(map, key)? {
            Value::Text(text) => Ok(text.clone()),
            _ => Err(InstructionExecutionError::InvariantViolation(
                format!("attestation entry `{key}` must be a string").into(),
            )),
        }
    }

    fn expect_bytes(
        map: &[(Value, Value)],
        key: &str,
    ) -> Result<Vec<u8>, InstructionExecutionError> {
        match expect_entry(map, key)? {
            Value::Bytes(bytes) => Ok(bytes.clone()),
            _ => Err(InstructionExecutionError::InvariantViolation(
                format!("attestation entry `{key}` must be a byte array").into(),
            )),
        }
    }

    fn expect_map<'a>(
        map: &'a [(Value, Value)],
        key: &str,
    ) -> Result<&'a [(Value, Value)], InstructionExecutionError> {
        match expect_entry(map, key)? {
            Value::Map(entries) => Ok(entries.as_slice()),
            _ => Err(InstructionExecutionError::InvariantViolation(
                format!("attestation entry `{key}` must be a map").into(),
            )),
        }
    }

    fn expect_bytes_array(
        map: &[(Value, Value)],
        key: &str,
    ) -> Result<Vec<Vec<u8>>, InstructionExecutionError> {
        match expect_entry(map, key)? {
            Value::Array(entries) => {
                let mut result = Vec::with_capacity(entries.len());
                for entry in entries {
                    if let Value::Bytes(bytes) = entry {
                        result.push(bytes.clone());
                    } else {
                        return Err(InstructionExecutionError::InvariantViolation(
                            format!(
                                "attestation entry `{key}` must contain only byte array elements"
                            )
                            .into(),
                        ));
                    }
                }
                Ok(result)
            }
            _ => Err(InstructionExecutionError::InvariantViolation(
                format!("attestation entry `{key}` must be an array").into(),
            )),
        }
    }

    fn cbor_map_optional_bytes(map: &[(Value, Value)], keys: &[&str]) -> Option<Vec<u8>> {
        for key in keys {
            if let Some((_, Value::Bytes(bytes))) = map
                .iter()
                .find(|(candidate, _)| matches!(candidate, Value::Text(text) if text == key))
            {
                return Some(bytes.clone());
            }
        }
        None
    }

    fn cbor_map_bytes(
        map: &[(Value, Value)],
        keys: &[&str],
    ) -> Result<Vec<u8>, InstructionExecutionError> {
        cbor_map_optional_bytes(map, keys).ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                format!("app attest assertion missing `{}` entry", keys.join(" | ")).into(),
            )
        })
    }

    struct AttestationAuthData<'a> {
        rp_id_hash: [u8; 32],
        sign_count: u32,
        aaguid: [u8; 16],
        credential_id: &'a [u8],
    }

    fn parse_attestation_auth_data(
        auth_data: &[u8],
    ) -> Result<AttestationAuthData<'_>, InstructionExecutionError> {
        if auth_data.len() < 37 + 16 + 2 {
            return Err(InstructionExecutionError::InvariantViolation(
                "authenticator data is too short".into(),
            ));
        }
        let rp_id_hash = auth_data[0..32].try_into().expect("slice length verified");
        let flags = auth_data[32];
        if flags & 0x40 == 0 {
            return Err(InstructionExecutionError::InvariantViolation(
                "attested credential data flag must be set".into(),
            ));
        }
        let sign_count_bytes = auth_data[33..37].try_into().expect("slice length verified");
        let sign_count = u32::from_be_bytes(sign_count_bytes);

        let mut offset = 37;
        let aaguid = auth_data[offset..offset + 16]
            .try_into()
            .expect("slice length verified");
        offset += 16;

        let credential_len_bytes = auth_data[offset..offset + 2]
            .try_into()
            .expect("slice length verified");
        offset += 2;
        let credential_len = u16::from_be_bytes(credential_len_bytes) as usize;
        if auth_data.len() < offset + credential_len {
            return Err(InstructionExecutionError::InvariantViolation(
                "credential id extends past authData bounds".into(),
            ));
        }
        let credential_id = &auth_data[offset..offset + credential_len];

        Ok(AttestationAuthData {
            rp_id_hash,
            sign_count,
            aaguid,
            credential_id,
        })
    }

    struct AssertionAuthData {
        rp_id_hash: [u8; 32],
        flags: u8,
        sign_count: u32,
    }

    fn parse_assertion_auth_data(
        auth_data: &[u8],
    ) -> Result<AssertionAuthData, InstructionExecutionError> {
        if auth_data.len() < 37 {
            return Err(InstructionExecutionError::InvariantViolation(
                "app attest assertion authenticatorData is too short".into(),
            ));
        }
        let rp_id_hash = auth_data[0..32].try_into().expect("slice length verified");
        let flags = auth_data[32];
        let sign_count =
            u32::from_be_bytes(auth_data[33..37].try_into().expect("slice length verified"));
        Ok(AssertionAuthData {
            rp_id_hash,
            flags,
            sign_count,
        })
    }

    fn parse_apple_signature(
        signature_bytes: &[u8],
    ) -> Result<P256Signature, InstructionExecutionError> {
        if let Ok(signature) = P256Signature::from_der(signature_bytes) {
            return Ok(signature);
        }
        if signature_bytes.len() == 64 {
            return P256Signature::from_slice(signature_bytes).map_err(|_| {
                InstructionExecutionError::InvariantViolation(
                    "app attest raw signature bytes are malformed".into(),
                )
            });
        }
        Err(InstructionExecutionError::InvariantViolation(
            "app attest signature must be DER or 64-byte raw format".into(),
        ))
    }

    pub(super) struct AppleAssertion {
        authenticator_data: Vec<u8>,
        rp_id_hash: [u8; 32],
        _flags: u8,
        sign_count: u32,
        client_data_hash: Option<[u8; 32]>,
        signature: P256Signature,
    }

    impl AppleAssertion {
        pub(super) fn parse(bytes: &[u8]) -> Result<Self, InstructionExecutionError> {
            if let Ok(assertion) = Self::parse_cbor(bytes) {
                return Ok(assertion);
            }
            Self::parse_compact(bytes)
        }

        fn parse_compact(bytes: &[u8]) -> Result<Self, InstructionExecutionError> {
            const AUTH_DATA_LEN: usize = 37;
            const CLIENT_HASH_LEN: usize = 32;
            if bytes.len() <= AUTH_DATA_LEN + CLIENT_HASH_LEN {
                return Err(InstructionExecutionError::InvariantViolation(
                    "app attest assertion is too short".into(),
                ));
            }
            let (authenticator_data, remainder) = bytes.split_at(AUTH_DATA_LEN);
            let (client_hash, signature_bytes) = remainder.split_at(CLIENT_HASH_LEN);
            let signature = parse_apple_signature(signature_bytes)?;
            let parsed_auth_data = parse_assertion_auth_data(authenticator_data)?;

            let auth_copy = authenticator_data.to_vec();
            Ok(Self {
                authenticator_data: auth_copy.clone(),
                rp_id_hash: parsed_auth_data.rp_id_hash,
                _flags: parsed_auth_data.flags,
                sign_count: parsed_auth_data.sign_count,
                client_data_hash: Some(client_hash.try_into().expect("slice length verified")),
                signature,
            })
        }

        fn parse_cbor(bytes: &[u8]) -> Result<Self, InstructionExecutionError> {
            let value: Value = from_reader(bytes).map_err(|err| {
                InstructionExecutionError::InvariantViolation(
                    format!("failed to decode app attest assertion CBOR: {err}").into(),
                )
            })?;
            let map = match value {
                Value::Map(map) => map,
                _ => {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "app attest assertion must be a CBOR map".into(),
                    ));
                }
            };

            let authenticator_data = cbor_map_bytes(&map, &["authenticatorData", "authData"])?;
            let signature_bytes = cbor_map_bytes(&map, &["signature", "sig"])?;
            let signature = parse_apple_signature(&signature_bytes)?;
            let parsed_auth_data = parse_assertion_auth_data(&authenticator_data)?;
            let client_data_hash =
                match cbor_map_optional_bytes(&map, &["clientDataHash", "client_data_hash"]) {
                    Some(bytes) => {
                        let hash: [u8; 32] = bytes.try_into().map_err(|_| {
                            InstructionExecutionError::InvariantViolation(
                                "app attest clientDataHash must be 32 bytes".into(),
                            )
                        })?;
                        Some(hash)
                    }
                    None => None,
                };

            Ok(Self {
                authenticator_data,
                rp_id_hash: parsed_auth_data.rp_id_hash,
                _flags: parsed_auth_data.flags,
                sign_count: parsed_auth_data.sign_count,
                client_data_hash,
                signature,
            })
        }

        pub(super) fn validate(
            &self,
            _metadata: &IosMetadata,
            counter: u64,
            challenge: &ReceiptChallenge,
            attestation: &AppleAttestation,
            settlement_cfg: &actual::Offline,
        ) -> Result<[u8; 32], InstructionExecutionError> {
            if self.rp_id_hash != attestation.rp_id_hash {
                warn!(
                    "[AppAttest] FAIL: assertion rpIdHash mismatch: got={} expected={}",
                    hex::encode(self.rp_id_hash),
                    hex::encode(attestation.rp_id_hash)
                );
                return Err(InstructionExecutionError::InvariantViolation(
                    "app attest assertion rpIdHash mismatch".into(),
                ));
            }
            let client_data_hash = if let Some(client_data_hash) = self.client_data_hash {
                if client_data_hash == challenge.client_data_hash {
                    client_data_hash
                } else if !settlement_cfg.apple_app_attest_strict_signature
                    && client_data_hash == challenge.iroha_bytes
                {
                    warn!(
                        "[AppAttest] assertion uses raw receipt challenge hash clientDataHash compatibility path"
                    );
                    client_data_hash
                } else {
                    warn!(
                        "[AppAttest] FAIL: assertion client_data_hash mismatch: got={} expected={}",
                        hex::encode(client_data_hash),
                        hex::encode(challenge.client_data_hash)
                    );
                    return Err(InstructionExecutionError::InvariantViolation(
                        "app attest assertion client data hash mismatch".into(),
                    ));
                }
            } else {
                challenge.client_data_hash
            };
            if u64::from(self.sign_count) != counter {
                warn!(
                    "[AppAttest] FAIL: assertion counter mismatch: got={} expected={}",
                    self.sign_count, counter
                );
                return Err(InstructionExecutionError::InvariantViolation(
                    "app attest counter mismatch".into(),
                ));
            }
            Ok(client_data_hash)
        }
    }

    fn app_attest_signature_prehash(
        authenticator_data: &[u8],
        client_data_hash_component: &[u8],
    ) -> [u8; 32] {
        // App Attest signatures are produced over `nonce = SHA256(authData || clientDataHash...)`
        // as a message, so ECDSA hashes that nonce once more internally.
        let mut nonce_digest = Sha256::new();
        nonce_digest.update(authenticator_data);
        nonce_digest.update(client_data_hash_component);
        let nonce: [u8; 32] = nonce_digest.finalize().into();

        let mut final_digest = Sha256::new();
        final_digest.update(nonce);
        final_digest.finalize().into()
    }

    pub(super) fn verify_apple_signature(
        verifying_key: &VerifyingKey,
        assertion: &AppleAssertion,
        client_data_hash: &[u8; 32],
        receipt_challenge_hash: &[u8; 32],
        settlement_cfg: &actual::Offline,
    ) -> Result<(), InstructionExecutionError> {
        let primary_prehash =
            app_attest_signature_prehash(&assertion.authenticator_data, client_data_hash);
        if verifying_key
            .verify_prehash(primary_prehash.as_ref(), &assertion.signature)
            .is_ok()
        {
            return Ok(());
        }

        if settlement_cfg.apple_app_attest_strict_signature {
            warn!(
                "[AppAttest] FAIL: strict signature mode enabled and primary verification failed"
            );
            return Err(InstructionExecutionError::InvariantViolation(
                "app attest signature does not verify".into(),
            ));
        }

        if client_data_hash != receipt_challenge_hash {
            let raw_challenge_prehash =
                app_attest_signature_prehash(&assertion.authenticator_data, receipt_challenge_hash);
            if verifying_key
                .verify_prehash(raw_challenge_prehash.as_ref(), &assertion.signature)
                .is_ok()
            {
                metrics::global_or_default().inc_offline_app_attest_signature_compat();
                warn!(
                    "[AppAttest] verify_apple_signature: accepted raw receipt challenge hash compatibility path"
                );
                return Ok(());
            }
        }

        // Some App Attest clients sign against SHA256(clientDataHash) rather
        // than the raw 32-byte clientDataHash.
        let hashed_client_data = Sha256::digest(client_data_hash);
        let fallback_prehash = app_attest_signature_prehash(
            &assertion.authenticator_data,
            hashed_client_data.as_ref(),
        );
        if verifying_key
            .verify_prehash(fallback_prehash.as_ref(), &assertion.signature)
            .is_ok()
        {
            metrics::global_or_default().inc_offline_app_attest_signature_compat();
            warn!(
                "[AppAttest] verify_apple_signature: accepted SHA256(clientDataHash) compatibility path"
            );
            return Ok(());
        }

        warn!("[AppAttest] FAIL: signature verification failed (primary + compatibility paths)");
        Err(InstructionExecutionError::InvariantViolation(
            "app attest signature does not verify".into(),
        ))
    }

    fn verify_attestation_chain(
        certificates: &[Vec<u8>],
        block_timestamp_ms: u64,
    ) -> Result<VerifyingKey, InstructionExecutionError> {
        warn!(
            "[AppAttest] verify_attestation_chain: certs={}, block_ts={block_timestamp_ms}",
            certificates.len()
        );
        if certificates.len() < 2 {
            warn!(
                "[AppAttest] FAIL: chain too short: {} certs",
                certificates.len()
            );
            return Err(InstructionExecutionError::InvariantViolation(
                "attestation must include leaf and intermediate certificates".into(),
            ));
        }
        let block_time = asn1_time_from_unix_ms(block_timestamp_ms)?;
        let (_, leaf_cert) = X509Certificate::from_der(&certificates[0]).map_err(|err| {
            warn!("[AppAttest] FAIL: parse leaf cert: {err}");
            InstructionExecutionError::InvariantViolation(
                format!("failed to parse leaf attestation cert: {err}").into(),
            )
        })?;
        let (_, intermediate_cert) =
            X509Certificate::from_der(&certificates[1]).map_err(|err| {
                warn!("[AppAttest] FAIL: parse intermediate cert: {err}");
                InstructionExecutionError::InvariantViolation(
                    format!("failed to parse intermediate attestation cert: {err}").into(),
                )
            })?;

        warn!(
            "[AppAttest] chain: leaf_not_before={:?} leaf_not_after={:?} intermediate_not_before={:?} intermediate_not_after={:?} block_time={:?}",
            leaf_cert.validity().not_before,
            leaf_cert.validity().not_after,
            intermediate_cert.validity().not_before,
            intermediate_cert.validity().not_after,
            block_time
        );

        check_certificate_validity(&leaf_cert, block_time).inspect_err(|e| {
            warn!("[AppAttest] FAIL: leaf cert validity: {e}");
        })?;
        check_certificate_validity(&intermediate_cert, block_time).inspect_err(|e| {
            warn!("[AppAttest] FAIL: intermediate cert validity: {e}");
        })?;

        leaf_cert
            .verify_signature(Some(intermediate_cert.public_key()))
            .map_err(|e| {
                warn!("[AppAttest] FAIL: leaf not signed by intermediate: {e}");
                InstructionExecutionError::InvariantViolation(
                    "leaf attestation certificate not signed by intermediate".into(),
                )
            })?;

        let mut anchored = false;
        let anchors = apple_trust_anchors();
        warn!(
            "[AppAttest] chain: checking {} trust anchors",
            anchors.len()
        );
        for anchor in anchors {
            if let Ok((_, root)) = X509Certificate::from_der(anchor) {
                if intermediate_cert
                    .verify_signature(Some(root.public_key()))
                    .is_ok()
                {
                    anchored = true;
                    break;
                }
            }
        }

        if !anchored {
            warn!("[AppAttest] FAIL: intermediate cert does not chain to any Apple root");
            return Err(InstructionExecutionError::InvariantViolation(
                "intermediate attestation certificate does not chain to trusted Apple root".into(),
            ));
        }

        let spki = leaf_cert.public_key();
        if spki.subject_public_key.data.is_empty() {
            warn!("[AppAttest] FAIL: leaf cert missing public key");
            return Err(InstructionExecutionError::InvariantViolation(
                "attestation leaf certificate missing public key".into(),
            ));
        }
        let verifying_key = VerifyingKey::from_sec1_bytes(spki.subject_public_key.data.as_ref())
            .map_err(|e| {
                warn!("[AppAttest] FAIL: invalid P-256 key in leaf: {e}");
                InstructionExecutionError::InvariantViolation(
                    "attestation leaf certificate does not contain a valid P-256 key".into(),
                )
            })?;
        warn!("[AppAttest] verify_attestation_chain: OK");
        Ok(verifying_key)
    }

    fn decode_app_attest_nonce_extension(
        nonce_ext_der: &[u8],
    ) -> Result<Vec<u8>, yasna::ASN1Error> {
        // Apple encodes the nonce extension (OID 1.2.840.113635.100.8.2) in at least
        // three observed formats depending on iOS version and environment:
        //
        // 1. Real device (iOS 18+):  SEQUENCE { [1] EXPLICIT OCTET STRING { <nonce> } }
        // 2. Earlier production:     SEQUENCE { SEQUENCE { [0] EXPLICIT OCTET STRING { <nonce> } } }
        // 3. Historical / test:          OCTET STRING { <nonce> }

        // Format 1: real Apple device — SEQUENCE { [1] EXPLICIT { OCTET STRING } }
        yasna::parse_der(nonce_ext_der, |reader| {
            reader.read_sequence(|seq| {
                seq.next()
                    .read_tagged(yasna::Tag::context(1), |tag_reader| tag_reader.read_bytes())
            })
        })
        // Format 2: earlier production — SEQUENCE { SEQUENCE { [0] EXPLICIT { OCTET STRING } } }
        .or_else(|_| {
            yasna::parse_der(nonce_ext_der, |reader| {
                reader.read_sequence(|seq| {
                    seq.next().read_sequence(|inner| {
                        inner
                            .next()
                            .read_tagged(yasna::Tag::context(0), |tag_reader| {
                                tag_reader.read_bytes()
                            })
                    })
                })
            })
        })
        // Format 3: historical plain OCTET STRING
        .or_else(|_| yasna::parse_der(nonce_ext_der, |reader| reader.read_bytes()))
    }

    fn verify_attestation_nonce(
        leaf_der: &[u8],
        auth_data: &[u8],
        client_data_hash: &[u8; 32],
    ) -> Result<(), InstructionExecutionError> {
        let mut digest = Sha256::new();
        digest.update(auth_data);
        digest.update(client_data_hash);
        let expected: [u8; 32] = digest.finalize().into();

        let (_, cert) = X509Certificate::from_der(leaf_der).map_err(|err| {
            warn!("[AppAttest] FAIL: nonce: parse leaf cert: {err}");
            InstructionExecutionError::InvariantViolation(
                format!("failed to parse app attest leaf certificate: {err}").into(),
            )
        })?;
        let extensions = cert.extensions();
        if extensions.is_empty() {
            warn!("[AppAttest] FAIL: nonce: leaf cert has no extensions");
            return Err(InstructionExecutionError::InvariantViolation(
                "app attest leaf certificate is missing extensions".into(),
            ));
        }
        let nonce_ext = extensions
            .iter()
            .find(|ext| ext.oid == APPLE_NONCE_OID)
            .ok_or_else(|| {
                let oids: Vec<String> = extensions.iter().map(|e| e.oid.to_string()).collect();
                warn!(
                    "[AppAttest] FAIL: nonce: OID {} not found among: {:?}",
                    APPLE_NONCE_OID, oids
                );
                InstructionExecutionError::InvariantViolation(
                    "app attest leaf certificate missing nonce extension".into(),
                )
            })?;
        let nonce_bytes = decode_app_attest_nonce_extension(nonce_ext.value).map_err(|err| {
            warn!(
                "[AppAttest] FAIL: nonce: decode extension DER: {err}, raw_hex={}",
                hex::encode(nonce_ext.value)
            );
            InstructionExecutionError::InvariantViolation(
                format!("failed to decode app attest nonce extension: {err}").into(),
            )
        })?;
        if nonce_bytes.as_slice() != expected {
            warn!(
                "[AppAttest] FAIL: nonce mismatch: got={} expected={}",
                hex::encode(&nonce_bytes),
                hex::encode(expected)
            );
            return Err(InstructionExecutionError::InvariantViolation(
                "app attest nonce does not match transaction challenge".into(),
            ));
        }
        warn!("[AppAttest] verify_attestation_nonce: OK");
        Ok(())
    }

    fn check_certificate_validity(
        cert: &X509Certificate<'_>,
        block_time: ASN1Time,
    ) -> Result<(), InstructionExecutionError> {
        if block_time < cert.validity().not_before || block_time > cert.validity().not_after {
            return Err(InstructionExecutionError::InvariantViolation(
                "attestation certificate is not valid for current block time".into(),
            ));
        }
        Ok(())
    }

    fn asn1_time_from_unix_ms(
        block_timestamp_ms: u64,
    ) -> Result<ASN1Time, InstructionExecutionError> {
        let seconds = i64::try_from(block_timestamp_ms / 1000).map_err(|_| {
            InstructionExecutionError::InvariantViolation("block timestamp is out of range".into())
        })?;
        ASN1Time::from_timestamp(seconds).map_err(|err| {
            InstructionExecutionError::InvariantViolation(
                format!("failed to convert block timestamp: {err}").into(),
            )
        })
    }

    pub(super) fn decode_key_id(key_id: &str) -> Result<Vec<u8>, InstructionExecutionError> {
        let canonical = canonical_app_attest_key_id(key_id).map_err(|err| {
            InstructionExecutionError::InvariantViolation(
                format!("invalid app attest key identifier: {err}").into(),
            )
        })?;
        BASE64_STANDARD.decode(canonical.as_bytes()).map_err(|_| {
            InstructionExecutionError::InvariantViolation(
                "invalid app attest key identifier encoding".into(),
            )
        })
    }

    fn expected_aaguid(env: AppleEnvironment) -> &'static [u8; 16] {
        match env {
            AppleEnvironment::Production => &APPLE_PRODUCTION_AAGUID,
            AppleEnvironment::Development => &APPLE_DEVELOPMENT_AAGUID,
        }
    }

    const APPLE_PRODUCTION_AAGUID: [u8; 16] = *b"appattest\0\0\0\0\0\0\0";
    const APPLE_DEVELOPMENT_AAGUID: [u8; 16] = *b"appattestdevelop";

    fn expected_rp_id_hash(metadata: &IosMetadata) -> [u8; 32] {
        let rp = format!("{}.{}", metadata.team_id, metadata.bundle_id);
        Sha256::digest(rp.as_bytes()).into()
    }

    fn verify_android_chain<'a>(
        certificates: &'a [Vec<u8>],
        block_timestamp_ms: u64,
        settlement_cfg: &actual::Offline,
    ) -> Result<X509Certificate<'a>, InstructionExecutionError> {
        verify_certificate_chain(
            certificates,
            block_timestamp_ms,
            settlement_cfg,
            builtin_android_trust_anchors(),
        )
    }

    fn verify_certificate_chain<'a>(
        certificates: &'a [Vec<u8>],
        block_timestamp_ms: u64,
        settlement_cfg: &actual::Offline,
        builtin: &'static [&'static [u8]],
    ) -> Result<X509Certificate<'a>, InstructionExecutionError> {
        if certificates.is_empty() {
            return Err(InstructionExecutionError::InvariantViolation(
                "attestation chain is empty".into(),
            ));
        }
        let block_time = asn1_time_from_unix_ms(block_timestamp_ms)?;
        let (_, leaf_cert) = X509Certificate::from_der(&certificates[0]).map_err(|err| {
            InstructionExecutionError::InvariantViolation(
                format!("failed to parse attestation leaf cert: {err}").into(),
            )
        })?;
        check_certificate_validity(&leaf_cert, block_time)?;

        for window in certificates.windows(2) {
            let (_, child) = X509Certificate::from_der(&window[0]).map_err(|err| {
                InstructionExecutionError::InvariantViolation(
                    format!("failed to parse attestation cert: {err}").into(),
                )
            })?;
            let (_, parent) = X509Certificate::from_der(&window[1]).map_err(|err| {
                InstructionExecutionError::InvariantViolation(
                    format!("failed to parse attestation cert: {err}").into(),
                )
            })?;
            check_certificate_validity(&child, block_time)?;
            check_certificate_validity(&parent, block_time)?;
            child
                .verify_signature(Some(parent.public_key()))
                .map_err(|_| {
                    InstructionExecutionError::InvariantViolation(
                        "attestation chain is not internally signed".into(),
                    )
                })?;
        }

        let last_bytes = certificates
            .last()
            .expect("attestation chain cannot be empty");
        let (_, last_cert) = X509Certificate::from_der(last_bytes).map_err(|err| {
            InstructionExecutionError::InvariantViolation(
                format!("failed to parse attestation cert: {err}").into(),
            )
        })?;

        let mut anchored = false;
        for anchor in &settlement_cfg.android_trust_anchors {
            if anchor_matches(anchor, last_bytes, &last_cert) {
                anchored = true;
                break;
            }
        }
        if !anchored {
            for anchor in builtin {
                if anchor_matches(anchor, last_bytes, &last_cert) {
                    anchored = true;
                    break;
                }
            }
        }

        if !anchored {
            return Err(InstructionExecutionError::InvariantViolation(
                "attestation chain does not terminate at a trusted root".into(),
            ));
        }

        Ok(leaf_cert)
    }

    fn anchor_matches(
        anchor_bytes: &[u8],
        last_bytes: &[u8],
        last_cert: &X509Certificate<'_>,
    ) -> bool {
        if last_bytes == anchor_bytes {
            return true;
        }
        if let Ok((_, root)) = X509Certificate::from_der(anchor_bytes) {
            return last_cert.verify_signature(Some(root.public_key())).is_ok();
        }
        false
    }

    struct ParsedJws {
        payload: JsonValue,
        signed_bytes: Vec<u8>,
        signature: Vec<u8>,
        certificates: Vec<Vec<u8>>,
    }

    #[allow(clippy::too_many_lines)]
    fn parse_jws_report(
        report: &[u8],
        platform: OfflineTransferRejectionPlatform,
        expected_alg: &str,
    ) -> Result<ParsedJws, InstructionExecutionError> {
        let token = str::from_utf8(report).map_err(|_| {
            rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationInvalid,
                platform,
                "attestation report must be valid UTF-8",
            )
        })?;
        let mut segments = token.split('.');
        let header_segment = segments.next().ok_or_else(|| {
            rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationInvalid,
                platform,
                "attestation header segment missing",
            )
        })?;
        let payload_segment = segments.next().ok_or_else(|| {
            rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationInvalid,
                platform,
                "attestation payload segment missing",
            )
        })?;
        let signature_segment = segments.next().ok_or_else(|| {
            rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationInvalid,
                platform,
                "attestation signature segment missing",
            )
        })?;
        if segments.next().is_some() {
            return Err(rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationInvalid,
                platform,
                "attestation JWS must contain exactly three segments",
            ));
        }
        let header_bytes =
            decode_base64_url(header_segment).map_err(|_| invalid_attestation(platform))?;
        let payload_bytes =
            decode_base64_url(payload_segment).map_err(|_| invalid_attestation(platform))?;
        let signature =
            decode_base64_url(signature_segment).map_err(|_| invalid_attestation(platform))?;
        let header: JsonValue = norito::json::from_slice(&header_bytes).map_err(|err| {
            rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationInvalid,
                platform,
                format!("failed to decode attestation header: {err}"),
            )
        })?;
        let payload: JsonValue = norito::json::from_slice(&payload_bytes).map_err(|err| {
            rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationInvalid,
                platform,
                format!("failed to decode attestation payload: {err}"),
            )
        })?;
        let header_map = header.as_object().ok_or_else(|| {
            rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationInvalid,
                platform,
                "attestation header must be a JSON object",
            )
        })?;
        let alg = header_map
            .get("alg")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| invalid_attestation(platform))?;
        if alg != expected_alg {
            return Err(rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationInvalid,
                platform,
                format!("unexpected attestation algorithm `{alg}`"),
            ));
        }
        let x5c = header_map
            .get("x5c")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| invalid_attestation(platform))?;
        if x5c.is_empty() {
            return Err(rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationInvalid,
                platform,
                "attestation header missing certificate chain",
            ));
        }
        let mut certificates = Vec::with_capacity(x5c.len());
        for entry in x5c {
            let cert_b64 = entry
                .as_str()
                .ok_or_else(|| invalid_attestation(platform))?;
            let der = BASE64_STANDARD
                .decode(cert_b64.as_bytes())
                .map_err(|_| invalid_attestation(platform))?;
            certificates.push(der);
        }
        let signed_bytes = format!("{header_segment}.{payload_segment}").into_bytes();
        Ok(ParsedJws {
            payload,
            signed_bytes,
            signature,
            certificates,
        })
    }

    fn decode_base64_url(input: &str) -> Result<Vec<u8>, base64::DecodeError> {
        URL_SAFE_NO_PAD
            .decode(input.as_bytes())
            .or_else(|_| URL_SAFE.decode(input.as_bytes()))
            .or_else(|_| BASE64_STANDARD.decode(input.as_bytes()))
    }

    fn decode_payload_digest(value: &str) -> Result<Vec<u8>, ()> {
        let sanitized: String = value
            .chars()
            .filter(|c| !c.is_ascii_whitespace() && *c != ':')
            .collect();
        if !sanitized.is_empty()
            && sanitized.chars().all(|c| c.is_ascii_hexdigit())
            && sanitized.len().is_multiple_of(2)
        {
            return hex::decode(&sanitized).map_err(|_| ());
        }
        BASE64_STANDARD.decode(value.as_bytes()).map_err(|_| ())
    }

    fn invalid_attestation(
        platform: OfflineTransferRejectionPlatform,
    ) -> InstructionExecutionError {
        rejection_error(
            OfflineTransferRejectionReason::PlatformAttestationInvalid,
            platform,
            "attestation token is malformed",
        )
    }

    fn verifying_key_from_chain(
        certificates: &[Vec<u8>],
        block_timestamp_ms: u64,
        settlement_cfg: &actual::Offline,
        builtin: &'static [&'static [u8]],
        platform: OfflineTransferRejectionPlatform,
    ) -> Result<VerifyingKey, InstructionExecutionError> {
        let leaf =
            verify_certificate_chain(certificates, block_timestamp_ms, settlement_cfg, builtin)?;
        let spki = leaf.public_key();
        if spki.subject_public_key.data.is_empty() {
            return Err(rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationInvalid,
                platform,
                "attestation certificate missing public key",
            ));
        }
        VerifyingKey::from_sec1_bytes(spki.subject_public_key.data.as_ref()).map_err(|_| {
            rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationInvalid,
                platform,
                "attestation certificate contains invalid P-256 key",
            )
        })
    }

    fn verify_es256_signature(
        verifying_key: &VerifyingKey,
        signed_bytes: &[u8],
        signature: &[u8],
        platform: OfflineTransferRejectionPlatform,
    ) -> Result<(), InstructionExecutionError> {
        let sig = P256Signature::from_slice(signature).map_err(|_| {
            rejection_error(
                OfflineTransferRejectionReason::PlatformSignatureInvalid,
                platform,
                "attestation signature is malformed",
            )
        })?;
        let mut digest = Sha256::new();
        digest.update(signed_bytes);
        verifying_key.verify_digest(digest, &sig).map_err(|_| {
            rejection_error(
                OfflineTransferRejectionReason::PlatformSignatureInvalid,
                platform,
                "attestation signature failed verification",
            )
        })
    }

    fn ensure_attestation_nonce(
        certificate: &OfflineWalletCertificate,
        expected: &[u8],
        platform: OfflineTransferRejectionPlatform,
    ) -> Result<(), InstructionExecutionError> {
        match &certificate.attestation_nonce {
            Some(nonce) if nonce.as_ref() == expected => Ok(()),
            Some(_) => Err(rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationInvalid,
                platform,
                "attestation nonce does not match certificate metadata",
            )),
            None => Err(rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationInvalid,
                platform,
                "certificate missing attestation_nonce metadata",
            )),
        }
    }

    fn ensure_token_fresh(
        issued_at_ms: u64,
        block_timestamp_ms: u64,
        max_age_ms: Option<u64>,
        platform: OfflineTransferRejectionPlatform,
    ) -> Result<(), InstructionExecutionError> {
        if block_timestamp_ms < issued_at_ms {
            return Err(rejection_error(
                OfflineTransferRejectionReason::PlatformAttestationInvalid,
                platform,
                "attestation timestamp is in the future",
            ));
        }
        if let Some(max_age) = max_age_ms {
            if block_timestamp_ms.saturating_sub(issued_at_ms) > max_age {
                return Err(rejection_error(
                    OfflineTransferRejectionReason::PlatformAttestationInvalid,
                    platform,
                    "attestation token is stale",
                ));
            }
        }
        Ok(())
    }

    const PLAY_PRODUCTION_ISSUERS: &[&str] = &["https://playintegrity.googleapis.com"];
    const PLAY_TESTING_ISSUERS: &[&str] = &[
        "https://playintegrity.googleapis.com",
        "https://staging-playintegrity.googleapis.com",
    ];

    fn play_integrity_issuers(env: PlayIntegrityEnvironment) -> &'static [&'static str] {
        match env {
            PlayIntegrityEnvironment::Production => PLAY_PRODUCTION_ISSUERS,
            PlayIntegrityEnvironment::Testing => PLAY_TESTING_ISSUERS,
        }
    }

    fn play_app_verdict_from_payload(value: &str) -> Option<PlayIntegrityAppVerdict> {
        match value {
            "PLAY_RECOGNIZED" => Some(PlayIntegrityAppVerdict::PlayRecognized),
            "LICENSED" => Some(PlayIntegrityAppVerdict::Licensed),
            "UNLICENSED" => Some(PlayIntegrityAppVerdict::Unlicensed),
            _ => None,
        }
    }

    fn play_device_verdict_from_payload(value: &str) -> Option<PlayIntegrityDeviceVerdict> {
        match value {
            "MEETS_STRONG_INTEGRITY" => Some(PlayIntegrityDeviceVerdict::Strong),
            "MEETS_DEVICE_INTEGRITY" => Some(PlayIntegrityDeviceVerdict::Device),
            "MEETS_BASIC_INTEGRITY" => Some(PlayIntegrityDeviceVerdict::Basic),
            "MEETS_VIRTUAL_INTEGRITY" => Some(PlayIntegrityDeviceVerdict::Virtual),
            _ => None,
        }
    }

    fn parse_hms_evaluations(value: &str) -> BTreeSet<HmsSafetyDetectEvaluation> {
        let mut result = BTreeSet::new();
        for token in value.split(|c: char| c == ',' || c == '|' || c.is_ascii_whitespace()) {
            if token.is_empty() {
                continue;
            }
            let normalized = token.trim().to_ascii_lowercase();
            let evaluation = match normalized.as_str() {
                "basic" | "basic_integrity" => Some(HmsSafetyDetectEvaluation::BasicIntegrity),
                "system" | "system_integrity" => Some(HmsSafetyDetectEvaluation::SystemIntegrity),
                "strong" | "strong_integrity" => Some(HmsSafetyDetectEvaluation::StrongIntegrity),
                _ => None,
            };
            if let Some(eval) = evaluation {
                result.insert(eval);
            }
        }
        result
    }

    fn json_expect_object<'a>(
        map: &'a JsonMap,
        key: &str,
        platform: OfflineTransferRejectionPlatform,
    ) -> Result<&'a JsonMap, InstructionExecutionError> {
        map.get(key)
            .and_then(JsonValue::as_object)
            .ok_or_else(|| invalid_attestation(platform))
    }

    fn json_expect_array<'a>(
        map: &'a JsonMap,
        key: &str,
        platform: OfflineTransferRejectionPlatform,
    ) -> Result<&'a [JsonValue], InstructionExecutionError> {
        map.get(key)
            .and_then(JsonValue::as_array)
            .map(Vec::as_slice)
            .ok_or_else(|| invalid_attestation(platform))
    }

    fn json_expect_string<'a>(
        map: &'a JsonMap,
        key: &str,
        platform: OfflineTransferRejectionPlatform,
    ) -> Result<&'a str, InstructionExecutionError> {
        map.get(key)
            .and_then(JsonValue::as_str)
            .ok_or_else(|| invalid_attestation(platform))
    }

    fn json_expect_u64(
        map: &JsonMap,
        key: &str,
        platform: OfflineTransferRejectionPlatform,
    ) -> Result<u64, InstructionExecutionError> {
        map.get(key)
            .and_then(JsonValue::as_u64)
            .ok_or_else(|| invalid_attestation(platform))
    }

    fn decode_android_attestation(
        report: &[u8],
    ) -> Result<Vec<Vec<u8>>, InstructionExecutionError> {
        let value: Value = from_reader(report).map_err(|err| {
            InstructionExecutionError::InvariantViolation(
                format!("failed to decode android attestation CBOR: {err}").into(),
            )
        })?;
        match value {
            Value::Array(entries) if !entries.is_empty() => {
                let mut chain = Vec::with_capacity(entries.len());
                for entry in entries {
                    if let Value::Bytes(bytes) = entry {
                        chain.push(bytes);
                    } else {
                        return Err(InstructionExecutionError::InvariantViolation(
                            "android attestation chain must be an array of DER-encoded certificates"
                                .into(),
                        ));
                    }
                }
                Ok(chain)
            }
            _ => Err(InstructionExecutionError::InvariantViolation(
                "android attestation must be a CBOR array of certificates".into(),
            )),
        }
    }

    fn decode_pem_bytes(src: &str) -> Result<Vec<u8>, base64::DecodeError> {
        let body: String = src
            .lines()
            .filter(|line| !line.starts_with("-----"))
            .collect();
        BASE64_STANDARD.decode(body.as_bytes())
    }

    #[cfg(test)]
    use rcgen::date_time_ymd;
    #[cfg(test)]
    #[allow(clippy::items_after_test_module)]
    mod tests {
        use std::{
            fs,
            path::{Path, PathBuf},
            sync::LazyLock,
        };

        use ciborium::value::Value;
        use iroha_config::parameters::actual;
        use iroha_data_model::{
            metadata::Metadata,
            offline::{
                OfflineAllowanceCommitment, OfflineSpendReceipt, OfflineWalletCertificate,
                OfflineWalletPolicy,
            },
        };
        use iroha_primitives::numeric::Numeric;
        use iroha_test_samples::{ALICE_ID, SAMPLE_GENESIS_ACCOUNT_ID};
        use p256::{EncodedPoint, pkcs8::DecodePrivateKey};
        use rcgen::{
            BasicConstraints, CertificateParams, CertifiedIssuer, CustomExtension,
            DistinguishedName, DnType, IsCa, KeyPair as RcgenKeyPair,
        };

        use super::*;
        use crate::smartcontracts::offline::isi::is_offline_allowance_authority;

        const TEAM_ID: &str = "TEAMID1234";
        const BUNDLE_ID: &str = "com.example.wallet";
        pub(super) const ANDROID_PACKAGE: &str = "com.example.wallet.android";
        pub(super) const ANDROID_SIGNING_DIGEST: [u8; 32] = [0x55; 32];
        pub(super) const ANDROID_KEY_DESCRIPTION_OID: &[u64] = &[1, 3, 6, 1, 4, 1, 11129, 2, 1, 17];
        const TEST_CHAIN_ID: &str = "testnet";

        #[derive(Clone, Copy)]
        enum AppleNonceExtensionFormat {
            /// Real Apple device (iOS 18+): SEQUENCE { [1] EXPLICIT { OCTET STRING } }
            RealDeviceTag1,
            /// Earlier production: SEQUENCE { SEQUENCE { [0] EXPLICIT { OCTET STRING } } }
            ProductionDerSequence,
            /// Historical plain OCTET STRING
            LegacyOctetString,
        }

        #[derive(Clone, Copy)]
        enum AppleAssertionEncoding {
            Compact,
            CborDerSignature,
            CborRawSignature,
            CompactHashedClientData,
            CompactRawReceiptHash,
            CborDerSignatureRawReceiptHash,
        }

        fn sample_chain_id() -> ChainId {
            TEST_CHAIN_ID.parse().expect("chain id")
        }

        #[test]
        fn receipt_challenge_rejects_empty_chain_id() {
            let fixture = AndroidFixture::new();
            let empty_chain_id: ChainId = "".parse().expect("chain id");
            assert!(derive_receipt_challenge(&fixture.receipt, &empty_chain_id).is_err());
        }

        #[test]
        fn genesis_height_allows_non_controller_authority() {
            let controller = ALICE_ID.clone();
            let genesis_authority = SAMPLE_GENESIS_ACCOUNT_ID.clone();
            assert!(
                !is_offline_allowance_authority(&controller, &genesis_authority, false),
                "non-controller authority must be rejected after genesis"
            );
            assert!(
                is_offline_allowance_authority(&controller, &genesis_authority, true),
                "genesis block must be able to seed allowances even when the controller differs"
            );
        }

        #[test]
        fn apple_attestation_verifies() {
            let fixture = AppleFixture::new();
            let cfg = default_offline_policy();
            let chain_id = sample_chain_id();
            verify_platform_proof(
                &fixture.receipt,
                &fixture.certificate,
                &chain_id,
                fixture.certificate.issued_at_ms + 1_000,
                &cfg,
                None,
            )
            .unwrap();
        }

        #[test]
        fn apple_attestation_verifies_with_cbor_assertion_der_signature() {
            let fixture =
                AppleFixture::new_with_assertion_encoding(AppleAssertionEncoding::CborDerSignature);
            let cfg = default_offline_policy();
            let chain_id = sample_chain_id();
            verify_platform_proof(
                &fixture.receipt,
                &fixture.certificate,
                &chain_id,
                fixture.certificate.issued_at_ms + 1_000,
                &cfg,
                None,
            )
            .expect("CBOR App Attest assertions with DER signatures should verify");
        }

        #[test]
        fn apple_attestation_verifies_with_cbor_assertion_raw_signature() {
            let fixture =
                AppleFixture::new_with_assertion_encoding(AppleAssertionEncoding::CborRawSignature);
            let cfg = default_offline_policy();
            let chain_id = sample_chain_id();
            verify_platform_proof(
                &fixture.receipt,
                &fixture.certificate,
                &chain_id,
                fixture.certificate.issued_at_ms + 1_000,
                &cfg,
                None,
            )
            .expect("CBOR App Attest assertions with 64-byte raw signatures should verify");
        }

        #[test]
        fn apple_attestation_verifies_with_hashed_client_data_signature_compat() {
            let fixture = AppleFixture::new_with_assertion_encoding(
                AppleAssertionEncoding::CompactHashedClientData,
            );
            let cfg = default_offline_policy();
            let chain_id = sample_chain_id();
            verify_platform_proof(
                &fixture.receipt,
                &fixture.certificate,
                &chain_id,
                fixture.certificate.issued_at_ms + 1_000,
                &cfg,
                None,
            )
            .expect("compatibility signature path should verify");
        }

        #[test]
        fn apple_attestation_rejects_hashed_client_data_signature_compat_in_strict_mode() {
            let fixture = AppleFixture::new_with_assertion_encoding(
                AppleAssertionEncoding::CompactHashedClientData,
            );
            let mut cfg = default_offline_policy();
            cfg.apple_app_attest_strict_signature = true;
            let chain_id = sample_chain_id();
            assert!(
                verify_platform_proof(
                    &fixture.receipt,
                    &fixture.certificate,
                    &chain_id,
                    fixture.certificate.issued_at_ms + 1_000,
                    &cfg,
                    None,
                )
                .is_err(),
                "strict signature mode must reject compatibility-only assertions"
            );
        }

        #[test]
        fn apple_attestation_verifies_with_raw_receipt_hash_client_data_hash_compat() {
            let fixture = AppleFixture::new_with_assertion_encoding(
                AppleAssertionEncoding::CompactRawReceiptHash,
            );
            let cfg = default_offline_policy();
            let chain_id = sample_chain_id();
            verify_platform_proof(
                &fixture.receipt,
                &fixture.certificate,
                &chain_id,
                fixture.certificate.issued_at_ms + 1_000,
                &cfg,
                None,
            )
            .expect("raw challenge hash clientDataHash should verify in non-strict mode");
        }

        #[test]
        fn apple_attestation_rejects_raw_receipt_hash_client_data_hash_compat_in_strict_mode() {
            let fixture = AppleFixture::new_with_assertion_encoding(
                AppleAssertionEncoding::CompactRawReceiptHash,
            );
            let mut cfg = default_offline_policy();
            cfg.apple_app_attest_strict_signature = true;
            let chain_id = sample_chain_id();
            assert!(
                verify_platform_proof(
                    &fixture.receipt,
                    &fixture.certificate,
                    &chain_id,
                    fixture.certificate.issued_at_ms + 1_000,
                    &cfg,
                    None,
                )
                .is_err(),
                "strict signature mode must reject raw challenge hash clientDataHash compatibility"
            );
        }

        #[test]
        fn apple_attestation_verifies_with_raw_receipt_hash_signature_compat() {
            let fixture = AppleFixture::new_with_assertion_encoding(
                AppleAssertionEncoding::CborDerSignatureRawReceiptHash,
            );
            let cfg = default_offline_policy();
            let chain_id = sample_chain_id();
            verify_platform_proof(
                &fixture.receipt,
                &fixture.certificate,
                &chain_id,
                fixture.certificate.issued_at_ms + 1_000,
                &cfg,
                None,
            )
            .expect("raw challenge hash signature compatibility path should verify");
        }

        #[test]
        fn apple_attestation_rejects_raw_receipt_hash_signature_compat_in_strict_mode() {
            let fixture = AppleFixture::new_with_assertion_encoding(
                AppleAssertionEncoding::CborDerSignatureRawReceiptHash,
            );
            let mut cfg = default_offline_policy();
            cfg.apple_app_attest_strict_signature = true;
            let chain_id = sample_chain_id();
            assert!(
                verify_platform_proof(
                    &fixture.receipt,
                    &fixture.certificate,
                    &chain_id,
                    fixture.certificate.issued_at_ms + 1_000,
                    &cfg,
                    None,
                )
                .is_err(),
                "strict signature mode must reject raw challenge hash signature compatibility"
            );
        }

        #[test]
        fn app_attest_signature_compat_fixture_replays_and_respects_strict_mode() {
            let fixture_path = workspace_root()
                .join("fixtures/offline_allowance/ios-demo/app_attest_signature_compat.json");
            let fixture_bytes = fs::read(&fixture_path).expect("read app attest signature fixture");
            let fixture_value: JsonValue =
                norito::json::from_slice(&fixture_bytes).expect("decode fixture JSON");
            let fixture_object = fixture_value
                .as_object()
                .expect("fixture root must be a JSON object");
            let decode_hex_field = |key: &str| -> Vec<u8> {
                let value = fixture_object
                    .get(key)
                    .and_then(JsonValue::as_str)
                    .unwrap_or_else(|| panic!("missing fixture field `{key}`"));
                hex::decode(value).unwrap_or_else(|err| panic!("invalid hex for `{key}`: {err}"))
            };

            let authenticator_data = decode_hex_field("authenticator_data_hex");
            let client_data_hash_bytes = decode_hex_field("client_data_hash_hex");
            let client_data_hash: [u8; 32] = client_data_hash_bytes
                .try_into()
                .expect("client_data_hash_hex must contain exactly 32 bytes");
            let signing_key_bytes = decode_hex_field("signing_key_hex");
            let signing_key = SigningKey::from_slice(&signing_key_bytes)
                .expect("fixture signing key must be a valid P-256 scalar");
            let verifying_key = signing_key.verifying_key();

            let hashed_client_data = Sha256::digest(client_data_hash);
            let fallback_prehash =
                app_attest_signature_prehash(&authenticator_data, hashed_client_data.as_ref());
            let signature = signing_key
                .sign_prehash(fallback_prehash.as_ref())
                .expect("fixture signing key must sign fallback prehash");

            let primary_prehash =
                app_attest_signature_prehash(&authenticator_data, &client_data_hash);
            assert!(
                verifying_key
                    .verify_prehash(primary_prehash.as_ref(), &signature)
                    .is_err(),
                "fixture must only verify through compatibility path"
            );

            let assertion = AppleAssertion {
                authenticator_data,
                rp_id_hash: [0u8; 32],
                _flags: 0,
                sign_count: 0,
                client_data_hash: Some(client_data_hash),
                signature,
            };

            let cfg = default_offline_policy();
            verify_apple_signature(
                verifying_key,
                &assertion,
                &client_data_hash,
                &[0u8; 32],
                &cfg,
            )
            .expect("compatibility fixture should verify in non-strict mode");

            let mut strict_cfg = cfg;
            strict_cfg.apple_app_attest_strict_signature = true;
            assert!(
                verify_apple_signature(
                    verifying_key,
                    &assertion,
                    &client_data_hash,
                    &[0u8; 32],
                    &strict_cfg,
                )
                .is_err(),
                "strict mode must reject compatibility-only fixture signatures"
            );
        }

        #[test]
        #[ignore = "requires IROHA_APPLE_ATTEST_REAL_DEVICE_FIXTURE pointing to a redacted real-device capture"]
        fn apple_attestation_real_device_fixture_replay_respects_strict_signature_policy() {
            let fixture_path = std::env::var("IROHA_APPLE_ATTEST_REAL_DEVICE_FIXTURE")
                .expect("set IROHA_APPLE_ATTEST_REAL_DEVICE_FIXTURE to the fixture JSON path");
            let fixture_bytes =
                fs::read(&fixture_path).expect("read IROHA_APPLE_ATTEST_REAL_DEVICE_FIXTURE");
            let fixture_value: JsonValue =
                norito::json::from_slice(&fixture_bytes).expect("decode fixture JSON");
            let fixture_object = fixture_value
                .as_object()
                .expect("fixture root must be a JSON object");

            let chain_id_text = fixture_object
                .get("chain_id")
                .and_then(JsonValue::as_str)
                .expect("fixture must include `chain_id`");
            let chain_id = chain_id_text
                .parse()
                .expect("fixture chain_id must be valid");

            let block_timestamp_ms = fixture_object
                .get("block_timestamp_ms")
                .and_then(JsonValue::as_u64)
                .expect("fixture must include integer `block_timestamp_ms`");

            let expect_non_strict_ok = fixture_object
                .get("expect_non_strict_ok")
                .and_then(JsonValue::as_bool)
                .unwrap_or(true);
            let expect_strict_ok = fixture_object
                .get("expect_strict_ok")
                .and_then(JsonValue::as_bool)
                .unwrap_or(false);

            let certificate: OfflineWalletCertificate = norito::json::from_value(
                fixture_object
                    .get("certificate")
                    .cloned()
                    .expect("fixture must include `certificate`"),
            )
            .expect("fixture certificate must decode");
            let receipt: OfflineSpendReceipt = norito::json::from_value(
                fixture_object
                    .get("receipt")
                    .cloned()
                    .expect("fixture must include `receipt`"),
            )
            .expect("fixture receipt must decode");

            let mut cfg = default_offline_policy();
            let non_strict_ok = verify_platform_proof(
                &receipt,
                &certificate,
                &chain_id,
                block_timestamp_ms,
                &cfg,
                None,
            )
            .is_ok();
            assert_eq!(
                non_strict_ok, expect_non_strict_ok,
                "non-strict expectation mismatch for fixture `{fixture_path}`",
            );

            cfg.apple_app_attest_strict_signature = true;
            let strict_ok = verify_platform_proof(
                &receipt,
                &certificate,
                &chain_id,
                block_timestamp_ms,
                &cfg,
                None,
            )
            .is_ok();
            assert_eq!(
                strict_ok, expect_strict_ok,
                "strict expectation mismatch for fixture `{fixture_path}`",
            );
        }

        #[test]
        fn app_attest_nonce_extension_fixture_der_decodes_all_formats() {
            const EXPECTED_NONCE: [u8; 32] = [
                0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
                0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B,
                0x1C, 0x1D, 0x1E, 0x1F,
            ];
            // Format 1: real Apple device (iOS 18+) — SEQUENCE { [1] EXPLICIT { OCTET STRING } }
            const REAL_DEVICE_DER: [u8; 38] = [
                0x30, 0x24, 0xA1, 0x22, 0x04, 0x20, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
                0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15,
                0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F,
            ];
            // Format 2: earlier production — SEQUENCE { SEQUENCE { [0] EXPLICIT { OCTET STRING } } }
            const PRODUCTION_DER: [u8; 40] = [
                0x30, 0x26, 0x30, 0x24, 0xA0, 0x22, 0x04, 0x20, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05,
                0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13,
                0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F,
            ];
            // Format 3: historical plain OCTET STRING
            const LEGACY_DER: [u8; 34] = [
                0x04, 0x20, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B,
                0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19,
                0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F,
            ];

            assert_eq!(
                decode_app_attest_nonce_extension(&REAL_DEVICE_DER)
                    .expect("real device DER decode"),
                EXPECTED_NONCE,
                "Format 1: SEQUENCE {{ [1] EXPLICIT {{ OCTET STRING }} }} — observed on real iOS 18+ device"
            );
            assert_eq!(
                decode_app_attest_nonce_extension(&PRODUCTION_DER).expect("production DER decode"),
                EXPECTED_NONCE,
                "Format 2: SEQUENCE {{ SEQUENCE {{ [0] EXPLICIT {{ OCTET STRING }} }} }}"
            );
            assert_eq!(
                decode_app_attest_nonce_extension(&LEGACY_DER).expect("historical DER decode"),
                EXPECTED_NONCE,
                "Format 3: plain OCTET STRING"
            );
        }

        #[test]
        fn apple_attestation_accepts_real_device_nonce_extension() {
            let fixture =
                AppleFixture::new_with_nonce_extension(AppleNonceExtensionFormat::RealDeviceTag1);
            let cfg = default_offline_policy();
            let chain_id = sample_chain_id();
            verify_platform_proof(
                &fixture.receipt,
                &fixture.certificate,
                &chain_id,
                fixture.certificate.issued_at_ms + 1_000,
                &cfg,
                None,
            )
            .expect("real device (iOS 18+) nonce extension should be supported");
        }

        #[test]
        fn apple_attestation_accepts_production_der_nonce_extension() {
            let fixture = AppleFixture::new_with_nonce_extension(
                AppleNonceExtensionFormat::ProductionDerSequence,
            );
            let cfg = default_offline_policy();
            let chain_id = sample_chain_id();
            verify_platform_proof(
                &fixture.receipt,
                &fixture.certificate,
                &chain_id,
                fixture.certificate.issued_at_ms + 1_000,
                &cfg,
                None,
            )
            .expect("production DER nonce extension should stay supported");
        }

        #[test]
        fn apple_attestation_accepts_legacy_nonce_extension() {
            let fixture = AppleFixture::new_with_nonce_extension(
                AppleNonceExtensionFormat::LegacyOctetString,
            );
            let cfg = default_offline_policy();
            let chain_id = sample_chain_id();
            verify_platform_proof(
                &fixture.receipt,
                &fixture.certificate,
                &chain_id,
                fixture.certificate.issued_at_ms + 1_000,
                &cfg,
                None,
            )
            .expect("historical nonce extension should stay supported");
        }

        #[test]
        fn apple_attestation_rejects_bad_challenge() {
            let mut fixture = AppleFixture::new();
            if let OfflinePlatformProof::AppleAppAttest(proof) = &mut fixture.receipt.platform_proof
            {
                proof.challenge_hash = Hash::new(b"wrong");
            }
            let cfg = default_offline_policy();
            let chain_id = sample_chain_id();
            assert!(
                verify_platform_proof(
                    &fixture.receipt,
                    &fixture.certificate,
                    &chain_id,
                    fixture.certificate.issued_at_ms + 1_000,
                    &cfg,
                    None,
                )
                .is_err()
            );
        }

        #[test]
        fn android_attestation_verifies() {
            let fixture = AndroidFixture::new();
            let cfg = default_offline_policy();
            let chain_id = sample_chain_id();
            verify_platform_proof(
                &fixture.receipt,
                &fixture.certificate,
                &chain_id,
                fixture.certificate.issued_at_ms + 5_000,
                &cfg,
                None,
            )
            .expect("android attestation proof should verify");
        }

        #[test]
        fn android_attestation_accepts_config_trust_anchor() {
            let chain = AndroidIssuerChain::unregistered_for_tests();
            let root_anchor = chain.root_der_bytes().to_vec();
            let fixture = AndroidFixture::from_chain(&chain);
            let mut cfg = default_offline_policy();
            cfg.android_trust_anchors = vec![root_anchor];
            let chain_id = sample_chain_id();
            verify_platform_proof(
                &fixture.receipt,
                &fixture.certificate,
                &chain_id,
                fixture.certificate.issued_at_ms + 5_000,
                &cfg,
                None,
            )
            .expect("configured trust anchor should validate attestation chain");
        }

        #[test]
        fn android_attestation_rejects_bad_challenge() {
            let mut fixture = AndroidFixture::new();
            fixture.receipt.invoice_id = "tampered".into();
            let cfg = default_offline_policy();
            let chain_id = sample_chain_id();
            assert!(
                verify_platform_proof(
                    &fixture.receipt,
                    &fixture.certificate,
                    &chain_id,
                    fixture.certificate.issued_at_ms + 5_000,
                    &cfg,
                    None,
                )
                .is_err()
            );
        }

        #[test]
        fn android_attestation_rejects_package_mismatch() {
            let mut fixture = AndroidFixture::new();
            metadata_insert(
                &mut fixture.certificate.metadata,
                ANDROID_PACKAGE_NAMES_KEY,
                IrohaJson::new(vec!["com.fake.wallet".to_string()]),
            );
            let cfg = default_offline_policy();
            let chain_id = sample_chain_id();
            assert!(
                verify_platform_proof(
                    &fixture.receipt,
                    &fixture.certificate,
                    &chain_id,
                    fixture.certificate.issued_at_ms + 5_000,
                    &cfg,
                    None,
                )
                .is_err()
            );
        }

        const PLAY_PROJECT_NUMBER: u64 = 4_242_424_242;
        const HMS_APP_ID: &str = "103000042";

        #[test]
        fn android_attestation_rejects_policy_mismatch() {
            let mut fixture = AndroidFixture::new();
            metadata_insert(
                &mut fixture.certificate.metadata,
                ANDROID_INTEGRITY_POLICY_KEY,
                IrohaJson::new("play_integrity"),
            );
            let cfg = default_offline_policy();
            let chain_id = sample_chain_id();
            assert!(
                verify_platform_proof(
                    &fixture.receipt,
                    &fixture.certificate,
                    &chain_id,
                    fixture.certificate.issued_at_ms + 5_000,
                    &cfg,
                    None,
                )
                .is_err()
            );
        }

        #[test]
        fn android_attestation_rejects_untrusted_custom_root() {
            let chain = AndroidIssuerChain::unregistered_for_tests();
            let fixture = AndroidFixture::from_chain(&chain);
            let cfg = default_offline_policy();
            let chain_id = sample_chain_id();
            assert!(
                verify_platform_proof(
                    &fixture.receipt,
                    &fixture.certificate,
                    &chain_id,
                    fixture.certificate.issued_at_ms + 5_000,
                    &cfg,
                    None,
                )
                .is_err(),
                "non-default attestation roots must be rejected when android_trust_anchors is empty"
            );
        }

        #[test]
        fn android_attestation_accepts_configured_custom_root() {
            let chain = AndroidIssuerChain::unregistered_for_tests();
            let fixture = AndroidFixture::from_chain(&chain);
            let mut cfg = default_offline_policy();
            cfg.android_trust_anchors = vec![chain.root_der_bytes().to_vec()];
            let chain_id = sample_chain_id();
            verify_platform_proof(
                &fixture.receipt,
                &fixture.certificate,
                &chain_id,
                fixture.certificate.issued_at_ms + 5_000,
                &cfg,
                None,
            )
            .expect("custom android_trust_anchors should allow non-default issuers");
        }

        #[test]
        fn android_attestation_accepts_custom_trust_anchor_fixture() {
            let fixture_dir = workspace_root()
                .join("fixtures")
                .join("android")
                .join("attestation")
                .join("mock_huawei");
            let chain_path = fixture_dir.join("chain.pem");
            let root_path = fixture_dir.join("trust_root_huawei.pem");
            let chain = read_pem_certificates(&chain_path);
            assert!(
                !chain.is_empty(),
                "mock_huawei chain must contain at least one certificate"
            );

            let builtin = builtin_android_trust_anchors();
            let mut cfg = default_offline_policy();
            let timestamp_ms = 1_763_078_400_000;
            let err = verify_certificate_chain(&chain, timestamp_ms, &cfg, builtin)
                .expect_err("missing custom trust anchor should fail");
            match err {
                InstructionExecutionError::InvariantViolation(msg) => assert!(
                    msg.contains("trusted root"),
                    "expected trusted root violation, got {msg}"
                ),
                other => panic!("unexpected error: {other:?}"),
            }

            let root_der = read_pem_certificates(&root_path)
                .into_iter()
                .next()
                .expect("root file must contain a certificate");
            cfg.android_trust_anchors.push(root_der);
            verify_certificate_chain(&chain, timestamp_ms, &cfg, builtin)
                .expect("custom trust anchor should validate the chain");
        }

        #[test]
        fn play_integrity_attestation_verifies() {
            let fixture = PlayIntegrityFixture::new();
            let cfg = default_offline_policy();
            let chain_id = sample_chain_id();
            verify_platform_proof(
                &fixture.receipt,
                &fixture.certificate,
                &chain_id,
                fixture.certificate.issued_at_ms + 20_000,
                &cfg,
                None,
            )
            .expect("play integrity attestation should verify");
        }

        #[test]
        fn hms_safety_detect_attestation_verifies() {
            let fixture = HmsIntegrityFixture::new();
            let cfg = default_offline_policy();
            let chain_id = sample_chain_id();
            verify_platform_proof(
                &fixture.receipt,
                &fixture.certificate,
                &chain_id,
                fixture.certificate.issued_at_ms + 20_000,
                &cfg,
                None,
            )
            .expect("hms safety detect attestation should verify");
        }

        #[test]
        fn provisioned_attestation_verifies() {
            let fixture = ProvisionedFixture::new();
            let cfg = default_offline_policy();
            let chain_id = sample_chain_id();
            verify_platform_proof(
                &fixture.receipt,
                &fixture.certificate,
                &chain_id,
                fixture.certificate.issued_at_ms + 15_000,
                &cfg,
                None,
            )
            .expect("provisioned attestation should verify");
        }

        #[test]
        fn provisioned_attestation_rejects_digest_mismatch() {
            let mut fixture = ProvisionedFixture::new();
            metadata_insert(
                &mut fixture.certificate.metadata,
                ANDROID_PROVISIONED_MANIFEST_DIGEST_KEY,
                IrohaJson::new(Hash::new(b"mismatch").to_string()),
            );
            let cfg = default_offline_policy();
            let chain_id = sample_chain_id();
            assert!(
                verify_platform_proof(
                    &fixture.receipt,
                    &fixture.certificate,
                    &chain_id,
                    fixture.certificate.issued_at_ms + 15_000,
                    &cfg,
                    None,
                )
                .is_err()
            );
        }

        fn default_offline_policy() -> actual::Offline {
            actual::Offline::default()
        }

        pub(super) fn metadata_insert(metadata: &mut Metadata, key: &str, value: IrohaJson) {
            let name = Name::from_str(key).expect("metadata key");
            let _ = metadata.insert(name, value);
        }

        pub(super) fn metadata_remove(metadata: &mut Metadata, key: &str) {
            if let Ok(name) = Name::from_str(key) {
                let _ = metadata.remove(&name);
            }
        }

        fn workspace_root() -> PathBuf {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(Path::parent)
                .expect("workspace root")
                .to_path_buf()
        }

        fn read_pem_certificates(path: &Path) -> Vec<Vec<u8>> {
            let contents = fs::read_to_string(path).expect("read PEM file");
            decode_pem_blocks(&contents)
        }

        fn decode_pem_blocks(contents: &str) -> Vec<Vec<u8>> {
            const BEGIN: &str = "-----BEGIN CERTIFICATE-----";
            const END: &str = "-----END CERTIFICATE-----";
            let mut remaining = contents;
            let mut certs = Vec::new();
            while let Some(start_idx) = remaining.find(BEGIN) {
                remaining = &remaining[start_idx + BEGIN.len()..];
                let end_idx = remaining
                    .find(END)
                    .expect("PEM certificate missing END marker");
                let block = &remaining[..end_idx];
                let data: String = block
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .collect();
                let der = BASE64_STANDARD
                    .decode(data.as_bytes())
                    .expect("decode PEM certificate");
                certs.push(der);
                remaining = &remaining[end_idx + END.len()..];
            }
            certs
        }

        struct AndroidFixture {
            certificate: OfflineWalletCertificate,
            receipt: OfflineSpendReceipt,
        }

        impl AndroidFixture {
            fn new() -> Self {
                Self::from_chain(AndroidIssuerChain::instance())
            }

            fn from_chain(chain: &AndroidIssuerChain) -> Self {
                let metadata = AndroidFixtureMetadata::default();
                let spend_pair = spend_keypair();
                let operator_pair = operator_keypair();
                let certificate = chain.build_certificate(&metadata, &spend_pair, &operator_pair);
                let marker_leaf = RcgenKeyPair::generate().expect("marker key");
                let marker_signing_key = SigningKey::from_pkcs8_der(&marker_leaf.serialize_der())
                    .expect("marker signing key");
                let marker_public_key = VerifyingKey::from(&marker_signing_key)
                    .to_encoded_point(false)
                    .as_bytes()
                    .to_vec();
                let mut receipt =
                    chain.build_receipt(&certificate, &spend_pair, &marker_public_key);
                let chain_id = sample_chain_id();
                let challenge = derive_receipt_challenge(&receipt, &chain_id).expect("challenge");
                let attestation = chain.issue_attestation(&challenge, &metadata, marker_leaf);
                let marker_signature: P256Signature = marker_signing_key
                    .sign_prehash(challenge.client_data_hash.as_ref())
                    .expect("marker signature");
                let marker_signature = marker_signature.to_bytes().to_vec();
                receipt.platform_proof =
                    OfflinePlatformProof::AndroidMarkerKey(AndroidMarkerKeyProof {
                        series: marker_series_from_public_key(&marker_public_key)
                            .expect("marker series"),
                        counter: 7,
                        marker_public_key,
                        marker_signature: Some(marker_signature),
                        attestation,
                    });
                Self {
                    certificate,
                    receipt,
                }
            }
        }

        struct PlayIntegrityFixture {
            certificate: OfflineWalletCertificate,
            receipt: OfflineSpendReceipt,
        }

        impl PlayIntegrityFixture {
            fn new() -> Self {
                static SIGNER: LazyLock<ExternalJwsSigner> =
                    LazyLock::new(|| ExternalJwsSigner::generate("Play Root CA", "Play Leaf"));
                register_test_play_root(SIGNER.root_der);
                let meta = AndroidFixtureMetadata::default();
                let base = AndroidFixture::new();
                let mut certificate = base.certificate;
                let receipt = base.receipt;
                let nonce_bytes = [0x11; 32];
                certificate.attestation_nonce = Some(Hash::prehashed(nonce_bytes));
                certificate.refresh_at_ms = Some(certificate.issued_at_ms + 30_000);
                configure_play_metadata(&mut certificate, &meta);
                let payload =
                    build_play_payload(&meta, &nonce_bytes, certificate.issued_at_ms + 10_000);
                certificate.attestation_report = SIGNER.build_token(&payload).into_bytes();
                Self {
                    certificate,
                    receipt,
                }
            }
        }

        struct HmsIntegrityFixture {
            certificate: OfflineWalletCertificate,
            receipt: OfflineSpendReceipt,
        }

        impl HmsIntegrityFixture {
            fn new() -> Self {
                static SIGNER: LazyLock<ExternalJwsSigner> =
                    LazyLock::new(|| ExternalJwsSigner::generate("HMS Root CA", "HMS Leaf"));
                register_test_hms_root(SIGNER.root_der);
                let meta = AndroidFixtureMetadata::default();
                let base = AndroidFixture::new();
                let mut certificate = base.certificate;
                let receipt = base.receipt;
                let mut nonce_bytes = [0x22; 32];
                nonce_bytes[Hash::LENGTH - 1] |= 1;
                certificate.attestation_nonce = Some(Hash::prehashed(nonce_bytes));
                certificate.refresh_at_ms = Some(certificate.issued_at_ms + 60_000);
                configure_hms_metadata(&mut certificate, &meta);
                let payload =
                    build_hms_payload(&meta, &nonce_bytes, certificate.issued_at_ms + 5_000);
                certificate.attestation_report = SIGNER.build_token(&payload).into_bytes();
                Self {
                    certificate,
                    receipt,
                }
            }
        }

        const PROVISIONED_SCHEMA: &str = "offline_provisioning_current";
        const PROVISIONED_VERSION: u32 = 1;
        const PROVISIONED_MAX_AGE: u64 = 604_800_000;

        struct ProvisionedFixture {
            certificate: OfflineWalletCertificate,
            receipt: OfflineSpendReceipt,
        }

        impl ProvisionedFixture {
            fn new() -> Self {
                let base = AndroidFixture::new();
                let inspector = operator_keypair();
                let mut certificate = base.certificate;
                let mut receipt = base.receipt;

                metadata_remove(&mut certificate.metadata, ANDROID_PACKAGE_NAMES_KEY);
                metadata_remove(&mut certificate.metadata, ANDROID_SIGNATURE_DIGESTS_KEY);
                metadata_remove(&mut certificate.metadata, ANDROID_REQUIRE_STRONGBOX_KEY);
                metadata_remove(&mut certificate.metadata, ANDROID_REQUIRE_ROLLBACK_KEY);

                let manifest = build_provisioned_manifest();
                let manifest_bytes = to_bytes(&manifest).expect("manifest bytes");
                let manifest_digest = Hash::new(&manifest_bytes);
                configure_provisioned_metadata(
                    &mut certificate,
                    inspector.public_key(),
                    manifest_digest,
                );
                certificate.refresh_at_ms =
                    Some(certificate.issued_at_ms + PROVISIONED_MAX_AGE.saturating_sub(30_000));

                let chain_id = sample_chain_id();
                let challenge = derive_receipt_challenge(&receipt, &chain_id).expect("challenge");
                let mut proof = AndroidProvisionedProof {
                    manifest_schema: PROVISIONED_SCHEMA.to_string(),
                    manifest_version: Some(PROVISIONED_VERSION),
                    manifest_issued_at_ms: certificate.issued_at_ms + 10_000,
                    challenge_hash: challenge.iroha_hash,
                    counter: 5,
                    device_manifest: manifest,
                    inspector_signature: Signature::from_bytes(&[0; 64]),
                };
                let payload = proof.signing_bytes().expect("manifest payload");
                proof.inspector_signature = Signature::new(inspector.private_key(), &payload);

                receipt.platform_proof = OfflinePlatformProof::Provisioned(proof);
                Self {
                    certificate,
                    receipt,
                }
            }
        }

        fn configure_play_metadata(
            certificate: &mut OfflineWalletCertificate,
            meta: &AndroidFixtureMetadata,
        ) {
            metadata_remove(&mut certificate.metadata, ANDROID_PACKAGE_NAMES_KEY);
            metadata_remove(&mut certificate.metadata, ANDROID_SIGNATURE_DIGESTS_KEY);
            metadata_remove(&mut certificate.metadata, ANDROID_REQUIRE_STRONGBOX_KEY);
            metadata_remove(&mut certificate.metadata, ANDROID_REQUIRE_ROLLBACK_KEY);
            metadata_insert(
                &mut certificate.metadata,
                ANDROID_INTEGRITY_POLICY_KEY,
                IrohaJson::new("play_integrity"),
            );
            metadata_insert(
                &mut certificate.metadata,
                ANDROID_PLAY_PROJECT_KEY,
                IrohaJson::new(PLAY_PROJECT_NUMBER),
            );
            metadata_insert(
                &mut certificate.metadata,
                ANDROID_PLAY_ENVIRONMENT_KEY,
                IrohaJson::new(PlayIntegrityEnvironment::Production.as_str().to_string()),
            );
            metadata_insert(
                &mut certificate.metadata,
                ANDROID_PLAY_PACKAGE_NAMES_KEY,
                IrohaJson::new(meta.package_names()),
            );
            metadata_insert(
                &mut certificate.metadata,
                ANDROID_PLAY_DIGESTS_KEY,
                IrohaJson::new(meta.signing_digests_hex()),
            );
            metadata_insert(
                &mut certificate.metadata,
                ANDROID_PLAY_APP_VERDICTS_KEY,
                IrohaJson::new(vec!["play_recognized".to_string()]),
            );
            metadata_insert(
                &mut certificate.metadata,
                ANDROID_PLAY_DEVICE_VERDICTS_KEY,
                IrohaJson::new(vec!["device".to_string()]),
            );
            metadata_insert(
                &mut certificate.metadata,
                ANDROID_PLAY_MAX_AGE_KEY,
                IrohaJson::new(60_000u64),
            );
        }

        fn configure_hms_metadata(
            certificate: &mut OfflineWalletCertificate,
            meta: &AndroidFixtureMetadata,
        ) {
            metadata_remove(&mut certificate.metadata, ANDROID_PACKAGE_NAMES_KEY);
            metadata_remove(&mut certificate.metadata, ANDROID_SIGNATURE_DIGESTS_KEY);
            metadata_remove(&mut certificate.metadata, ANDROID_REQUIRE_STRONGBOX_KEY);
            metadata_remove(&mut certificate.metadata, ANDROID_REQUIRE_ROLLBACK_KEY);
            metadata_insert(
                &mut certificate.metadata,
                ANDROID_INTEGRITY_POLICY_KEY,
                IrohaJson::new("hms_safety_detect"),
            );
            metadata_insert(
                &mut certificate.metadata,
                ANDROID_HMS_APP_ID_KEY,
                IrohaJson::new(HMS_APP_ID.to_string()),
            );
            metadata_insert(
                &mut certificate.metadata,
                ANDROID_HMS_PACKAGE_NAMES_KEY,
                IrohaJson::new(meta.package_names()),
            );
            metadata_insert(
                &mut certificate.metadata,
                ANDROID_HMS_DIGESTS_KEY,
                IrohaJson::new(meta.signing_digests_hex()),
            );
            metadata_insert(
                &mut certificate.metadata,
                ANDROID_HMS_EVALUATIONS_KEY,
                IrohaJson::new(vec![
                    "strong_integrity".to_string(),
                    "system_integrity".to_string(),
                ]),
            );
            metadata_insert(
                &mut certificate.metadata,
                ANDROID_HMS_MAX_AGE_KEY,
                IrohaJson::new(3_600_000u64),
            );
        }

        fn configure_provisioned_metadata(
            certificate: &mut OfflineWalletCertificate,
            inspector_key: &iroha_crypto::PublicKey,
            manifest_digest: Hash,
        ) {
            metadata_insert(
                &mut certificate.metadata,
                ANDROID_INTEGRITY_POLICY_KEY,
                IrohaJson::new("provisioned"),
            );
            metadata_insert(
                &mut certificate.metadata,
                ANDROID_PROVISIONED_INSPECTOR_KEY,
                IrohaJson::new(inspector_key.to_string()),
            );
            metadata_insert(
                &mut certificate.metadata,
                ANDROID_PROVISIONED_MANIFEST_SCHEMA_KEY,
                IrohaJson::new(PROVISIONED_SCHEMA),
            );
            metadata_insert(
                &mut certificate.metadata,
                ANDROID_PROVISIONED_MANIFEST_VERSION_KEY,
                IrohaJson::new(PROVISIONED_VERSION),
            );
            metadata_insert(
                &mut certificate.metadata,
                ANDROID_PROVISIONED_MAX_AGE_KEY,
                IrohaJson::new(PROVISIONED_MAX_AGE),
            );
            metadata_insert(
                &mut certificate.metadata,
                ANDROID_PROVISIONED_MANIFEST_DIGEST_KEY,
                IrohaJson::new(manifest_digest.to_string()),
            );
        }

        fn build_provisioned_manifest() -> Metadata {
            let mut manifest = Metadata::default();
            metadata_insert(
                &mut manifest,
                ANDROID_PROVISIONED_DEVICE_ID_KEY,
                IrohaJson::new("device-alpha"),
            );
            metadata_insert(
                &mut manifest,
                "android.provisioned.audit_ticket",
                IrohaJson::new("ticket-001"),
            );
            metadata_insert(
                &mut manifest,
                "android.provisioned.os_patch",
                IrohaJson::new("2025-02-05"),
            );
            manifest
        }

        fn build_play_payload(
            meta: &AndroidFixtureMetadata,
            nonce: &[u8],
            timestamp_ms: u64,
        ) -> String {
            let digest_hex = meta
                .signing_digests_hex()
                .into_iter()
                .next()
                .expect("digest hex");
            let digest_bytes = hex::decode(digest_hex).expect("digest bytes");
            let digest_b64 = BASE64_STANDARD.encode(digest_bytes);
            let nonce_b64 = BASE64_STANDARD.encode(nonce);
            format!(
                "{{\"aud\":{aud},\"iss\":\"https://playintegrity.googleapis.com\",\
                 \"requestDetails\":{{\"requestPackageName\":\"{pkg}\",\"nonce\":\"{nonce}\",\"timestampMillis\":{ts}}},\
                 \"appIntegrity\":{{\"packageName\":\"{pkg}\",\"certificateSha256Digest\":[\"{digest}\"],\"appRecognitionVerdict\":\"PLAY_RECOGNIZED\"}},\
                 \"deviceIntegrity\":{{\"deviceRecognitionVerdict\":[\"MEETS_DEVICE_INTEGRITY\"]}}}}",
                aud = PLAY_PROJECT_NUMBER,
                pkg = meta.package_name(),
                nonce = nonce_b64,
                ts = timestamp_ms,
                digest = digest_b64,
            )
        }

        fn build_hms_payload(
            meta: &AndroidFixtureMetadata,
            nonce: &[u8],
            timestamp_ms: u64,
        ) -> String {
            let digest_hex = meta
                .signing_digests_hex()
                .into_iter()
                .next()
                .expect("digest hex");
            let nonce_b64 = BASE64_STANDARD.encode(nonce);
            format!(
                "{{\"apkPackageName\":\"{pkg}\",\"appId\":\"{app_id}\",\"nonce\":\"{nonce}\",\
                 \"timestampMs\":{ts},\"apkCertificateDigestSha256\":[\"{digest}\"],\
                 \"evaluationType\":\"STRONG_INTEGRITY,SYSTEM_INTEGRITY\"}}",
                pkg = meta.package_name(),
                app_id = HMS_APP_ID,
                nonce = nonce_b64,
                ts = timestamp_ms,
                digest = digest_hex.to_ascii_uppercase(),
            )
        }

        struct AppleFixture {
            certificate: OfflineWalletCertificate,
            receipt: OfflineSpendReceipt,
        }

        impl AppleFixture {
            fn new() -> Self {
                Self::new_with_options(
                    AppleNonceExtensionFormat::RealDeviceTag1,
                    AppleAssertionEncoding::Compact,
                )
            }

            fn new_with_nonce_extension(nonce_extension: AppleNonceExtensionFormat) -> Self {
                Self::new_with_options(nonce_extension, AppleAssertionEncoding::Compact)
            }

            fn new_with_assertion_encoding(assertion_encoding: AppleAssertionEncoding) -> Self {
                Self::new_with_options(
                    AppleNonceExtensionFormat::RealDeviceTag1,
                    assertion_encoding,
                )
            }

            fn new_with_options(
                nonce_extension: AppleNonceExtensionFormat,
                assertion_encoding: AppleAssertionEncoding,
            ) -> Self {
                static CHAIN: LazyLock<TestChain> = LazyLock::new(TestChain::generate);
                register_test_apple_root(CHAIN.root_der);

                let spend_pair = spend_keypair();
                let operator_pair = operator_keypair();
                let leaf_keypair = RcgenKeyPair::generate().expect("leaf key");
                let signing_key_der = leaf_keypair.serialize_der();
                let signing_key =
                    SigningKey::from_pkcs8_der(&signing_key_der).expect("decode leaf key");
                let mut certificate = CHAIN.build_certificate(&spend_pair, &operator_pair);
                let receipt = CHAIN.build_receipt(
                    &certificate,
                    &spend_pair,
                    &signing_key,
                    7,
                    assertion_encoding,
                );

                // The attestation is created at top-up time with a challenge
                // independent of any receipt.  iOS uses
                // blake2b(accountId + spendPublicKey) as the clientDataHash
                // passed to DCAppAttestService.attestKey().  Simulate that
                // here with a deterministic stand-in so the attestation and
                // receipt challenges are intentionally different (matching the
                // real device flow).
                let topup_nonce = Hash::new(b"simulated-topup-attestation-challenge");
                let mut topup_cdh = [0u8; 32];
                topup_cdh.copy_from_slice(topup_nonce.as_ref());
                certificate.attestation_nonce = Some(topup_nonce);

                certificate.attestation_report = build_attestation_report(
                    &leaf_keypair,
                    &CHAIN.intermediate,
                    &CHAIN.intermediate_der,
                    &signing_key,
                    &CHAIN.credential_id,
                    &CHAIN.rp_id_hash,
                    &topup_cdh,
                    nonce_extension,
                );
                Self {
                    certificate,
                    receipt,
                }
            }
        }
        struct TestChain {
            root_der: &'static [u8],
            intermediate: CertifiedIssuer<'static, RcgenKeyPair>,
            intermediate_der: Vec<u8>,
            credential_id: Vec<u8>,
            metadata: IosMetadata,
            rp_id_hash: [u8; 32],
        }

        impl TestChain {
            fn generate() -> Self {
                let root = Self::self_signed_ca("Root CA");
                let intermediate = Self::ca_signed_by("Intermediate CA", &root);
                let root_der = leak(root.der().to_vec());
                let intermediate_der = intermediate.der().to_vec();
                let credential_id = vec![0xAB; 32];
                let metadata = IosMetadata {
                    team_id: TEAM_ID.into(),
                    bundle_id: BUNDLE_ID.into(),
                    environment: AppleEnvironment::Development,
                };
                let rp_id_hash = expected_rp_id_hash(&metadata);
                Self {
                    root_der,
                    intermediate,
                    intermediate_der,
                    credential_id,
                    metadata,
                    rp_id_hash,
                }
            }

            fn self_signed_ca(common_name: &str) -> CertifiedIssuer<'static, RcgenKeyPair> {
                let key = RcgenKeyPair::generate().expect("root key");
                let mut params = CertificateParams::new(vec![]).expect("root params");
                params.distinguished_name = Self::dn(common_name);
                params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
                Self::apply_validity(&mut params);
                CertifiedIssuer::self_signed(params, key).expect("root cert")
            }

            fn ca_signed_by(
                common_name: &str,
                issuer: &CertifiedIssuer<'_, RcgenKeyPair>,
            ) -> CertifiedIssuer<'static, RcgenKeyPair> {
                let key = RcgenKeyPair::generate().expect("intermediate key");
                let mut params = CertificateParams::new(vec![]).expect("intermediate params");
                params.distinguished_name = Self::dn(common_name);
                params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
                Self::apply_validity(&mut params);
                CertifiedIssuer::signed_by(params, key, issuer).expect("intermediate cert")
            }

            fn dn(common_name: &str) -> DistinguishedName {
                let mut dn = DistinguishedName::new();
                dn.push(DnType::CommonName, common_name);
                dn
            }

            fn apply_validity(params: &mut CertificateParams) {
                params.not_before = date_time_ymd(2023, 1, 1);
                params.not_after = date_time_ymd(2033, 1, 1);
            }

            fn build_certificate(
                &self,
                spend_pair: &iroha_crypto::KeyPair,
                operator_pair: &iroha_crypto::KeyPair,
            ) -> OfflineWalletCertificate {
                let controller = test_account_id(0xA1);
                let operator = AccountId::new(operator_pair.public_key().clone());
                let definition =
                    AssetDefinitionId::new("acme".parse().unwrap(), "usd".parse().unwrap());
                let asset = AssetId::new(definition, controller.clone());
                let mut certificate = OfflineWalletCertificate {
                    controller: controller.clone(),
                    operator,
                    allowance: OfflineAllowanceCommitment {
                        asset,
                        amount: Numeric::new(1_000, 0),
                        commitment: vec![0x33; 32],
                    },
                    spend_public_key: spend_pair.public_key().clone(),
                    attestation_report: Vec::new(),
                    issued_at_ms: 1_700_000_000_000,
                    expires_at_ms: 1_800_000_000_000,
                    policy: OfflineWalletPolicy {
                        max_balance: Numeric::new(5_000, 0),
                        max_tx_value: Numeric::new(1_000, 0),
                        expires_at_ms: 1_800_000_000_000,
                    },
                    operator_signature: Signature::from_bytes(&[0; 64]),
                    metadata: Metadata::default(),
                    verdict_id: None,
                    attestation_nonce: None,
                    refresh_at_ms: None,
                };
                certificate.metadata.insert(
                    IOS_TEAM_ID_KEY.parse().unwrap(),
                    IrohaJson::new(self.metadata.team_id.clone()),
                );
                certificate.metadata.insert(
                    IOS_BUNDLE_ID_KEY.parse().unwrap(),
                    IrohaJson::new(self.metadata.bundle_id.clone()),
                );
                certificate.metadata.insert(
                    IOS_ENVIRONMENT_KEY.parse().unwrap(),
                    IrohaJson::new(String::from("development")),
                );

                let payload = certificate
                    .operator_signing_bytes()
                    .expect("certificate payload");
                certificate.operator_signature =
                    Signature::new(operator_pair.private_key(), &payload);
                certificate
            }

            fn build_receipt(
                &self,
                certificate: &OfflineWalletCertificate,
                spend_pair: &iroha_crypto::KeyPair,
                signing_key: &SigningKey,
                counter: u64,
                assertion_encoding: AppleAssertionEncoding,
            ) -> OfflineSpendReceipt {
                let mut receipt = OfflineSpendReceipt {
                    tx_id: Hash::new(b"apple-offline"),
                    from: certificate.controller.clone(),
                    to: test_account_id(0xB1),
                    asset: certificate.allowance.asset.clone(),
                    amount: Numeric::new(250, 0),
                    issued_at_ms: certificate.issued_at_ms + 1_000,
                    invoice_id: "inv-app-attest".into(),
                    platform_proof: OfflinePlatformProof::AppleAppAttest(AppleAppAttestProof {
                        key_id: BASE64_STANDARD.encode(&self.credential_id),
                        counter,
                        assertion: Vec::new(),
                        challenge_hash: Hash::new([]),
                    }),
                    platform_snapshot: None,
                    sender_certificate_id: certificate.certificate_id(),
                    sender_signature: Signature::from_bytes(&[0; 64]),
                    build_claim: None,
                };
                let chain_id = sample_chain_id();
                let challenge = derive_receipt_challenge(&receipt, &chain_id).expect("challenge");
                let assertion_client_data_hash = match assertion_encoding {
                    AppleAssertionEncoding::CompactRawReceiptHash
                    | AppleAssertionEncoding::CborDerSignatureRawReceiptHash => {
                        &challenge.iroha_bytes
                    }
                    _ => &challenge.client_data_hash,
                };
                let assertion = build_assertion(
                    signing_key,
                    &self.rp_id_hash,
                    assertion_client_data_hash,
                    counter,
                    assertion_encoding,
                );
                receipt.platform_proof =
                    OfflinePlatformProof::AppleAppAttest(AppleAppAttestProof {
                        key_id: BASE64_STANDARD.encode(&self.credential_id),
                        counter,
                        assertion,
                        challenge_hash: challenge.iroha_hash,
                    });
                let payload = receipt.signing_bytes().expect("receipt payload");
                receipt.sender_signature = Signature::new(spend_pair.private_key(), &payload);
                receipt
            }
        }

        fn spend_keypair() -> iroha_crypto::KeyPair {
            iroha_crypto::KeyPair::from_seed(vec![0x55; 32], Algorithm::Ed25519)
        }

        fn operator_keypair() -> iroha_crypto::KeyPair {
            iroha_crypto::KeyPair::from_seed(vec![0x77; 32], Algorithm::Ed25519)
        }

        fn build_attestation_report(
            leaf_keypair: &RcgenKeyPair,
            intermediate: &CertifiedIssuer<'_, RcgenKeyPair>,
            intermediate_der: &[u8],
            signing_key: &SigningKey,
            credential_id: &[u8],
            rp_id_hash: &[u8; 32],
            client_data_hash: &[u8; 32],
            nonce_extension: AppleNonceExtensionFormat,
        ) -> Vec<u8> {
            let mut auth_data = Vec::new();
            auth_data.extend_from_slice(rp_id_hash);
            auth_data.push(0x41);
            auth_data.extend_from_slice(&[0, 0, 0, 0]);
            auth_data.extend_from_slice(&APPLE_DEVELOPMENT_AAGUID);
            let credential_len =
                u16::try_from(credential_id.len()).expect("credential id length fits u16");
            auth_data.extend_from_slice(&credential_len.to_be_bytes());
            auth_data.extend_from_slice(credential_id);
            auth_data.extend_from_slice(&cose_key_bytes(signing_key));

            let mut nonce_digest = Sha256::new();
            nonce_digest.update(&auth_data);
            nonce_digest.update(client_data_hash);
            let nonce = nonce_digest.finalize();
            let nonce_der = match nonce_extension {
                AppleNonceExtensionFormat::RealDeviceTag1 => {
                    // Real Apple device (iOS 18+) nonce extension format:
                    // SEQUENCE { [1] EXPLICIT OCTET STRING { <nonce> } }
                    yasna::construct_der(|writer| {
                        writer.write_sequence(|seq| {
                            seq.next()
                                .write_tagged(yasna::Tag::context(1), |tag_writer| {
                                    tag_writer.write_bytes(&nonce);
                                });
                        });
                    })
                }
                AppleNonceExtensionFormat::ProductionDerSequence => {
                    // Earlier production App Attest nonce extension format:
                    // SEQUENCE { SEQUENCE { [0] EXPLICIT OCTET STRING { <nonce> } } }
                    yasna::construct_der(|writer| {
                        writer.write_sequence(|seq| {
                            seq.next().write_sequence(|inner| {
                                inner
                                    .next()
                                    .write_tagged(yasna::Tag::context(0), |tag_writer| {
                                        tag_writer.write_bytes(&nonce);
                                    });
                            });
                        });
                    })
                }
                AppleNonceExtensionFormat::LegacyOctetString => {
                    yasna::construct_der(|writer| writer.write_bytes(&nonce))
                }
            };

            let mut params = CertificateParams::new(vec![]).expect("leaf params");
            params.distinguished_name = TestChain::dn("Leaf");
            params.is_ca = IsCa::ExplicitNoCa;
            TestChain::apply_validity(&mut params);
            params
                .custom_extensions
                .push(CustomExtension::from_oid_content(
                    APPLE_NONCE_OID_COMPONENTS,
                    nonce_der,
                ));
            let leaf = params
                .signed_by(leaf_keypair, intermediate)
                .expect("leaf cert");
            let leaf_der = leaf.der().to_vec();

            let att_stmt = Value::Map(vec![(
                Value::Text("x5c".into()),
                Value::Array(vec![
                    Value::Bytes(leaf_der),
                    Value::Bytes(intermediate_der.to_vec()),
                ]),
            )]);

            let value = Value::Map(vec![
                (
                    Value::Text("fmt".into()),
                    Value::Text("apple-appattest".into()),
                ),
                (Value::Text("authData".into()), Value::Bytes(auth_data)),
                (Value::Text("attStmt".into()), att_stmt),
            ]);
            let mut buf = Vec::new();
            into_writer(&value, &mut buf).expect("serialize attestation");
            buf
        }

        fn cose_key_bytes(signing_key: &SigningKey) -> Vec<u8> {
            let verifying_key = VerifyingKey::from(signing_key);
            let encoded = EncodedPoint::from(&verifying_key);
            let x = encoded
                .x()
                .expect("x coordinate")
                .iter()
                .copied()
                .collect::<Vec<u8>>();
            let y = encoded
                .y()
                .expect("y coordinate")
                .iter()
                .copied()
                .collect::<Vec<u8>>();
            let value = Value::Map(vec![
                (Value::Integer(1.into()), Value::Integer(2.into())),
                (Value::Integer(3.into()), Value::Integer((-7).into())),
                (Value::Integer((-1).into()), Value::Integer(1.into())),
                (Value::Integer((-2).into()), Value::Bytes(x)),
                (Value::Integer((-3).into()), Value::Bytes(y)),
            ]);
            let mut buf = Vec::new();
            into_writer(&value, &mut buf).expect("serialize cose key");
            buf
        }

        fn build_assertion_auth_data(rp_id_hash: &[u8; 32], counter: u64) -> Vec<u8> {
            let mut auth_data = Vec::new();
            auth_data.extend_from_slice(rp_id_hash);
            auth_data.push(0x01);
            let counter_u32 = u32::try_from(counter).expect("assertion counter fits in u32");
            auth_data.extend_from_slice(&counter_u32.to_be_bytes());
            auth_data
        }

        fn build_assertion(
            signing_key: &SigningKey,
            rp_id_hash: &[u8; 32],
            client_data_hash: &[u8; 32],
            counter: u64,
            encoding: AppleAssertionEncoding,
        ) -> Vec<u8> {
            let auth_data = build_assertion_auth_data(rp_id_hash, counter);

            let signature: P256Signature = match encoding {
                AppleAssertionEncoding::CompactHashedClientData => {
                    let hashed_client_data = Sha256::digest(client_data_hash);
                    let prehash =
                        app_attest_signature_prehash(&auth_data, hashed_client_data.as_ref());
                    signing_key
                        .sign_prehash(prehash.as_ref())
                        .expect("fixture signing key must sign compatibility prehash")
                }
                _ => {
                    let prehash = app_attest_signature_prehash(&auth_data, client_data_hash);
                    signing_key
                        .sign_prehash(prehash.as_ref())
                        .expect("fixture signing key must sign assertion prehash")
                }
            };
            match encoding {
                AppleAssertionEncoding::Compact
                | AppleAssertionEncoding::CompactHashedClientData
                | AppleAssertionEncoding::CompactRawReceiptHash => {
                    let mut assertion = auth_data;
                    assertion.extend_from_slice(client_data_hash);
                    assertion.extend_from_slice(signature.to_der().as_bytes());
                    assertion
                }
                AppleAssertionEncoding::CborDerSignature
                | AppleAssertionEncoding::CborDerSignatureRawReceiptHash => {
                    let value = Value::Map(vec![
                        (
                            Value::Text("authenticatorData".into()),
                            Value::Bytes(auth_data),
                        ),
                        (
                            Value::Text("signature".into()),
                            Value::Bytes(signature.to_der().as_bytes().to_vec()),
                        ),
                    ]);
                    let mut encoded = Vec::new();
                    into_writer(&value, &mut encoded).expect("serialize assertion");
                    encoded
                }
                AppleAssertionEncoding::CborRawSignature => {
                    let value = Value::Map(vec![
                        (
                            Value::Text("authenticatorData".into()),
                            Value::Bytes(auth_data),
                        ),
                        (
                            Value::Text("signature".into()),
                            Value::Bytes(signature.to_bytes().to_vec()),
                        ),
                    ]);
                    let mut encoded = Vec::new();
                    into_writer(&value, &mut encoded).expect("serialize assertion");
                    encoded
                }
            }
        }
    }

    #[cfg(test)]
    use tests::{
        ANDROID_KEY_DESCRIPTION_OID, ANDROID_PACKAGE, ANDROID_SIGNING_DIGEST, metadata_insert,
    };

    #[cfg(test)]
    fn leak(bytes: Vec<u8>) -> &'static [u8] {
        Box::leak(bytes.into_boxed_slice())
    }

    #[cfg(test)]
    struct AndroidFixtureMetadata {
        package_name: &'static str,
        signing_digest: [u8; 32],
    }

    #[cfg(test)]
    impl Default for AndroidFixtureMetadata {
        fn default() -> Self {
            Self {
                package_name: ANDROID_PACKAGE,
                signing_digest: ANDROID_SIGNING_DIGEST,
            }
        }
    }

    #[cfg(test)]
    impl AndroidFixtureMetadata {
        fn package_names(&self) -> Vec<String> {
            vec![self.package_name.to_string()]
        }

        fn package_name(&self) -> &str {
            self.package_name
        }

        fn signing_digests_hex(&self) -> Vec<String> {
            vec![hex::encode(self.signing_digest)]
        }
    }

    #[cfg(test)]
    struct AndroidIssuerChain {
        intermediate: CertifiedIssuer<'static, RcgenKeyPair>,
        intermediate_der: Vec<u8>,
        root_der: Vec<u8>,
    }

    #[cfg(test)]
    impl AndroidIssuerChain {
        fn instance() -> &'static Self {
            static INSTANCE: LazyLock<AndroidIssuerChain> =
                LazyLock::new(AndroidIssuerChain::generate);
            &INSTANCE
        }

        fn generate() -> Self {
            Self::generate_with_registration(true)
        }

        fn generate_with_registration(register_root: bool) -> Self {
            let root = CertifiedIssuer::self_signed(
                Self::ca_params("Android Root CA"),
                RcgenKeyPair::generate().expect("root key"),
            )
            .expect("android root issuer");
            let root_der = root.der().to_vec();
            if register_root {
                let leaked = leak(root_der.clone());
                register_test_android_root(leaked);
            }
            let intermediate = CertifiedIssuer::signed_by(
                Self::ca_params("Android Intermediate CA"),
                RcgenKeyPair::generate().expect("intermediate key"),
                &*root,
            )
            .expect("android intermediate");
            let intermediate_der = intermediate.der().to_vec();
            Self {
                intermediate,
                intermediate_der,
                root_der,
            }
        }

        fn unregistered_for_tests() -> Self {
            Self::generate_with_registration(false)
        }

        fn root_der_bytes(&self) -> &[u8] {
            &self.root_der
        }

        #[allow(clippy::unused_self)]
        fn build_certificate(
            &self,
            metadata: &AndroidFixtureMetadata,
            spend_pair: &KeyPair,
            operator_pair: &KeyPair,
        ) -> OfflineWalletCertificate {
            let controller = test_account_id(0xC3);
            let operator = AccountId::new(operator_pair.public_key().clone());
            let definition =
                AssetDefinitionId::new("acme".parse().unwrap(), "usd".parse().unwrap());
            let asset = AssetId::new(definition, controller.clone());
            let mut certificate = OfflineWalletCertificate {
                controller,
                operator,
                allowance: OfflineAllowanceCommitment {
                    asset,
                    amount: Numeric::new(2_000, 0),
                    commitment: vec![0x22; 32],
                },
                spend_public_key: spend_pair.public_key().clone(),
                attestation_report: Vec::new(),
                issued_at_ms: 1_690_000_000_000,
                expires_at_ms: 1_800_000_000_000,
                policy: OfflineWalletPolicy {
                    max_balance: Numeric::new(10_000, 0),
                    max_tx_value: Numeric::new(1_000, 0),
                    expires_at_ms: 1_800_000_000_000,
                },
                operator_signature: Signature::from_bytes(&[0; 64]),
                metadata: Metadata::default(),
                verdict_id: None,
                attestation_nonce: None,
                refresh_at_ms: None,
            };
            metadata_insert(
                &mut certificate.metadata,
                ANDROID_PACKAGE_NAMES_KEY,
                IrohaJson::new(metadata.package_names()),
            );
            metadata_insert(
                &mut certificate.metadata,
                ANDROID_SIGNATURE_DIGESTS_KEY,
                IrohaJson::new(metadata.signing_digests_hex()),
            );
            metadata_insert(
                &mut certificate.metadata,
                ANDROID_REQUIRE_STRONGBOX_KEY,
                IrohaJson::new(true),
            );
            metadata_insert(
                &mut certificate.metadata,
                ANDROID_REQUIRE_ROLLBACK_KEY,
                IrohaJson::new(true),
            );
            metadata_insert(
                &mut certificate.metadata,
                ANDROID_INTEGRITY_POLICY_KEY,
                IrohaJson::new(AndroidIntegrityPolicy::MarkerKey.as_str().to_string()),
            );
            let payload = certificate
                .operator_signing_bytes()
                .expect("certificate payload");
            certificate.operator_signature = Signature::new(operator_pair.private_key(), &payload);
            certificate
        }

        #[allow(clippy::unused_self)]
        fn build_receipt(
            &self,
            certificate: &OfflineWalletCertificate,
            spend_pair: &KeyPair,
            marker_public_key: &[u8],
        ) -> OfflineSpendReceipt {
            let series = marker_series_from_public_key(marker_public_key).expect("marker series");
            let mut receipt = OfflineSpendReceipt {
                tx_id: Hash::new(b"android-offline"),
                from: certificate.controller.clone(),
                to: test_account_id(0xD1),
                asset: certificate.allowance.asset.clone(),
                amount: Numeric::new(250, 0),
                issued_at_ms: certificate.issued_at_ms + 1_000,
                invoice_id: "inv-android-attest".into(),
                platform_proof: OfflinePlatformProof::AndroidMarkerKey(AndroidMarkerKeyProof {
                    series,
                    counter: 7,
                    marker_public_key: marker_public_key.to_vec(),
                    marker_signature: None,
                    attestation: Vec::new(),
                }),
                platform_snapshot: None,
                sender_certificate_id: certificate.certificate_id(),
                sender_signature: Signature::from_bytes(&[0; 64]),
                build_claim: None,
            };
            let payload = receipt.signing_bytes().expect("receipt payload");
            receipt.sender_signature = Signature::new(spend_pair.private_key(), &payload);
            receipt
        }

        fn issue_attestation(
            &self,
            challenge: &ReceiptChallenge,
            metadata: &AndroidFixtureMetadata,
            leaf_key: RcgenKeyPair,
        ) -> Vec<u8> {
            let mut params = Self::leaf_params("Android Marker Leaf");
            params
                .custom_extensions
                .push(CustomExtension::from_oid_content(
                    ANDROID_KEY_DESCRIPTION_OID,
                    build_key_description_der(challenge.iroha_bytes.as_ref(), metadata),
                ));
            let leaf = CertifiedIssuer::signed_by(params, leaf_key, &*self.intermediate)
                .expect("android leaf");
            let mut buf = Vec::new();
            into_writer(
                &Value::Array(vec![
                    Value::Bytes(leaf.der().to_vec()),
                    Value::Bytes(self.intermediate_der.clone()),
                ]),
                &mut buf,
            )
            .expect("serialize android attestation");
            buf
        }

        fn ca_params(common_name: &str) -> CertificateParams {
            let mut params = CertificateParams::new(vec![]).expect("ca params");
            params.distinguished_name = Self::dn(common_name);
            params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
            params.not_before = date_time_ymd(2023, 1, 1);
            params.not_after = date_time_ymd(2033, 1, 1);
            params
        }

        fn leaf_params(common_name: &str) -> CertificateParams {
            let mut params = CertificateParams::new(vec![]).expect("leaf params");
            params.distinguished_name = Self::dn(common_name);
            params.is_ca = IsCa::ExplicitNoCa;
            params.not_before = date_time_ymd(2023, 1, 1);
            params.not_after = date_time_ymd(2033, 1, 1);
            params
        }

        fn dn(common_name: &str) -> DistinguishedName {
            let mut dn = DistinguishedName::new();
            dn.push(DnType::CommonName, common_name);
            dn
        }
    }

    #[cfg(test)]
    struct ExternalJwsSigner {
        root_der: &'static [u8],
        leaf_der: Vec<u8>,
        signing_key: SigningKey,
    }

    #[cfg(test)]
    impl ExternalJwsSigner {
        fn generate(root_cn: &str, leaf_cn: &str) -> Self {
            let root = CertifiedIssuer::self_signed(
                AndroidIssuerChain::ca_params(root_cn),
                RcgenKeyPair::generate().expect("root key"),
            )
            .expect("external root");
            let root_der = leak(root.der().to_vec());
            let leaf_key = RcgenKeyPair::generate().expect("leaf key");
            let signing_key_der = leaf_key.serialize_der();
            let leaf = CertifiedIssuer::signed_by(
                AndroidIssuerChain::leaf_params(leaf_cn),
                leaf_key,
                &*root,
            )
            .expect("external leaf");
            let signing_key = SigningKey::from_pkcs8_der(&signing_key_der).expect("signing key");
            Self {
                root_der,
                leaf_der: leaf.der().to_vec(),
                signing_key,
            }
        }

        fn build_token(&self, payload_json: &str) -> String {
            let header = format!(
                "{{\"alg\":\"ES256\",\"typ\":\"JWT\",\"x5c\":[\"{}\",\"{}\"]}}",
                BASE64_STANDARD.encode(&self.leaf_der),
                BASE64_STANDARD.encode(self.root_der),
            );
            let header_b64 = URL_SAFE_NO_PAD.encode(header.as_bytes());
            let payload_b64 = URL_SAFE_NO_PAD.encode(payload_json.as_bytes());
            let signing_input = format!("{header_b64}.{payload_b64}");
            let mut digest = Sha256::new();
            digest.update(signing_input.as_bytes());
            let signature: P256Signature = self.signing_key.sign_digest(digest);
            let signature_b64 = URL_SAFE_NO_PAD.encode(signature.to_bytes());
            format!("{signing_input}.{signature_b64}")
        }
    }

    #[cfg(test)]
    #[allow(clippy::too_many_lines)]
    fn build_key_description_der(challenge: &[u8], metadata: &AndroidFixtureMetadata) -> Vec<u8> {
        let unique = vec![0x44; 16];
        let mut parts = Vec::new();
        parts.push(yasna::construct_der(|writer| writer.write_u64(4)));
        parts.push(yasna::construct_der(|writer| {
            writer.write_enum(i64::from(KM_SECURITY_LEVEL_STRONG_BOX));
        }));
        parts.push(yasna::construct_der(|writer| writer.write_u64(4)));
        parts.push(yasna::construct_der(|writer| {
            writer.write_enum(i64::from(KM_SECURITY_LEVEL_STRONG_BOX));
        }));
        parts.push(yasna::construct_der(|writer| writer.write_bytes(challenge)));
        parts.push(yasna::construct_der(|writer| writer.write_bytes(&unique)));
        let software_auth_list = yasna::construct_der(|writer| {
            writer.write_sequence(|writer| {
                writer
                    .next()
                    .write_tagged(Tag::context(u64::from(KM_TAG_PURPOSE)), |writer| {
                        writer.write_set(|writer| {
                            writer.next().write_i64(i64::from(KM_PURPOSE_SIGN));
                        });
                    });
                writer
                    .next()
                    .write_tagged(Tag::context(u64::from(KM_TAG_ALGORITHM)), |writer| {
                        writer.write_i64(i64::from(KM_ALGORITHM_EC))
                    });
                writer
                    .next()
                    .write_tagged(Tag::context(u64::from(KM_TAG_KEY_SIZE)), |writer| {
                        writer.write_i64(256)
                    });
                writer
                    .next()
                    .write_tagged(Tag::context(u64::from(KM_TAG_EC_CURVE)), |writer| {
                        writer.write_i64(i64::from(KM_EC_CURVE_P256))
                    });
                writer
                    .next()
                    .write_tagged(Tag::context(u64::from(KM_TAG_ORIGIN)), |writer| {
                        writer.write_i64(i64::from(KM_ORIGIN_GENERATED))
                    });
                writer.next().write_tagged(
                    Tag::context(u64::from(KM_TAG_ROLLBACK_RESISTANCE)),
                    |writer| writer.write_bool(true),
                );
                writer.next().write_tagged(
                    Tag::context(u64::from(KM_TAG_ATTESTATION_APPLICATION_ID)),
                    |writer| {
                        writer.write_sequence(|writer| {
                            writer.next().write_set(|writer| {
                                writer.next().write_sequence(|writer| {
                                    writer.next().write_bytes(metadata.package_name.as_bytes());
                                    writer.next().write_u64(1);
                                });
                            });
                            writer.next().write_set(|writer| {
                                writer.next().write_bytes(&metadata.signing_digest);
                            });
                        });
                    },
                );
            });
        });
        let tee_auth_list = yasna::construct_der(|writer| {
            writer.write_sequence(|writer| {
                writer
                    .next()
                    .write_tagged(Tag::context(u64::from(KM_TAG_PURPOSE)), |writer| {
                        writer.write_set(|writer| {
                            writer.next().write_i64(i64::from(KM_PURPOSE_SIGN));
                        });
                    });
                writer
                    .next()
                    .write_tagged(Tag::context(u64::from(KM_TAG_ALGORITHM)), |writer| {
                        writer.write_i64(i64::from(KM_ALGORITHM_EC))
                    });
                writer
                    .next()
                    .write_tagged(Tag::context(u64::from(KM_TAG_KEY_SIZE)), |writer| {
                        writer.write_i64(256)
                    });
                writer
                    .next()
                    .write_tagged(Tag::context(u64::from(KM_TAG_EC_CURVE)), |writer| {
                        writer.write_i64(i64::from(KM_EC_CURVE_P256))
                    });
                writer
                    .next()
                    .write_tagged(Tag::context(u64::from(KM_TAG_ORIGIN)), |writer| {
                        writer.write_i64(i64::from(KM_ORIGIN_GENERATED))
                    });
                writer.next().write_tagged(
                    Tag::context(u64::from(KM_TAG_ROLLBACK_RESISTANCE)),
                    |writer| writer.write_bool(true),
                );
                writer.next().write_tagged(
                    Tag::context(u64::from(KM_TAG_ROOT_OF_TRUST)),
                    |writer| {
                        writer.write_sequence(|writer| {
                            writer.next().write_bytes(&[0xAA; 32]);
                            writer.next().write_bool(true);
                            writer
                                .next()
                                .write_enum(i64::from(KM_VERIFIED_BOOT_STATE_VERIFIED));
                            writer.next().write_bytes(&[0xBB; 32]);
                        });
                    },
                );
            });
        });
        parts.push(software_auth_list);
        parts.push(tee_auth_list);
        parts.into_iter().flatten().collect()
    }

    #[cfg(test)]
    mod counter_state_tests {
        use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
        use iroha_data_model::{
            account::AccountId,
            asset::{AssetDefinitionId, AssetId},
            metadata::Metadata,
            offline::{
                AppleAppAttestProof, OfflineAllowanceCommitment, OfflinePlatformProof,
                OfflineSpendReceipt, OfflineWalletCertificate, OfflineWalletPolicy,
            },
        };
        use iroha_primitives::numeric::Numeric;

        use super::{super::isi::stage_receipt_counters, *};

        #[test]
        fn counter_state_rejects_non_contiguous_increment() {
            let mut staged = OfflineCounterState::default();
            let key_id = BASE64_STANDARD.encode(b"key-1");
            staged.apple_key_counters.insert(key_id.clone(), 2);

            let receipt = sample_receipt(4, &key_id);
            assert!(stage_receipt_counters(&mut staged, &[receipt]).is_err());
        }

        #[test]
        fn counter_state_accepts_contiguous_increment() {
            let mut staged = OfflineCounterState::default();
            let key_id = BASE64_STANDARD.encode(b"key-1");
            staged.apple_key_counters.insert(key_id.clone(), 2);

            let receipt = sample_receipt(3, &key_id);
            stage_receipt_counters(&mut staged, &[receipt]).expect("counter should increment");
            assert_eq!(staged.apple_key_counters.get(&key_id), Some(&3));
        }

        fn sample_receipt(counter: u64, key_id: &str) -> OfflineSpendReceipt {
            let controller: AccountId = test_account_id(1);
            let receiver: AccountId = test_account_id(2);
            let definition =
                AssetDefinitionId::new("counter".parse().unwrap(), "xor".parse().unwrap());
            let asset = AssetId::new(definition, controller.clone());
            let counter_byte = u8::try_from(counter).expect("counter fits u8");
            let certificate = OfflineWalletCertificate {
                controller: controller.clone(),
                operator: controller.clone(),
                allowance: OfflineAllowanceCommitment {
                    asset: asset.clone(),
                    amount: Numeric::new(1, 0),
                    commitment: vec![0xAA; 32],
                },
                spend_public_key: controller.signatory().clone(),
                attestation_report: Vec::new(),
                issued_at_ms: 1,
                expires_at_ms: 2,
                policy: OfflineWalletPolicy {
                    max_balance: Numeric::new(10, 0),
                    max_tx_value: Numeric::new(5, 0),
                    expires_at_ms: 2,
                },
                operator_signature: Signature::from_bytes(&[0; 64]),
                metadata: Metadata::default(),
                verdict_id: None,
                attestation_nonce: None,
                refresh_at_ms: None,
            };
            OfflineSpendReceipt {
                tx_id: Hash::new(vec![counter_byte; 32]),
                from: controller,
                to: receiver,
                asset,
                amount: Numeric::new(1, 0),
                issued_at_ms: 1,
                invoice_id: "inv-counter".into(),
                platform_proof: OfflinePlatformProof::AppleAppAttest(AppleAppAttestProof {
                    key_id: key_id.to_string(),
                    counter,
                    assertion: Vec::new(),
                    challenge_hash: Hash::new(b"counter-challenge"),
                }),
                platform_snapshot: None,
                sender_certificate_id: certificate.certificate_id(),
                sender_signature: Signature::from_bytes(&[0; 64]),
                build_claim: None,
            }
        }
    }

    #[cfg(test)]
    fn test_account_id(seed: u8) -> AccountId {
        let pair = iroha_crypto::KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        AccountId::new(pair.public_key().clone())
    }
}

pub use self::attestation::{LineageAppleAppAttestVerification, verify_lineage_apple_app_attest};

#[cfg(test)]
mod aggregate_proof_tests {
    use std::sync::OnceLock;

    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
    use blake2::{
        Blake2bVar,
        digest::{Update, VariableOutput},
    };
    use curve25519_dalek::{
        constants::RISTRETTO_BASEPOINT_POINT, ristretto::RistrettoPoint, scalar::Scalar,
    };
    use iroha_config::parameters::actual::OfflineProofMode;
    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use iroha_data_model::{
        account::AccountId,
        asset::{AssetDefinitionId, AssetId},
        metadata::Metadata,
        offline::{
            AGGREGATE_PROOF_METADATA_OFFLINE_AGGREGATE_BACKEND,
            AGGREGATE_PROOF_METADATA_OFFLINE_AGGREGATE_CIRCUIT_ID,
            AGGREGATE_PROOF_METADATA_OFFLINE_AGGREGATE_PUBLIC_INPUTS_B64,
            AGGREGATE_PROOF_METADATA_OFFLINE_AGGREGATE_RECURSION_DEPTH, AGGREGATE_PROOF_VERSION_V1,
            AGGREGATE_PROOF_VERSION_V2, AggregateProofEnvelope, AppleAppAttestProof,
            OFFLINE_FASTPQ_COUNTER_PROOF_DOMAIN, OFFLINE_FASTPQ_HKDF_DOMAIN,
            OFFLINE_FASTPQ_PROOF_VERSION_V1, OFFLINE_FASTPQ_REPLAY_CHAIN_DOMAIN,
            OFFLINE_FASTPQ_REPLAY_PROOF_DOMAIN, OFFLINE_FASTPQ_SUM_NONCE_DOMAIN,
            OFFLINE_FASTPQ_SUM_PROOF_DOMAIN, OfflineAllowanceCommitment, OfflineBalanceProof,
            OfflineFastpqCounterProof, OfflineFastpqReplayProof, OfflineFastpqSumProof,
            OfflinePlatformProof, OfflineProofBlindingSeed, OfflineSpendReceipt,
            OfflineToOnlineTransfer, OfflineWalletCertificate, OfflineWalletPolicy, PoseidonDigest,
            compute_receipts_root,
        },
    };
    use iroha_primitives::numeric::Numeric;
    use sha2::Sha512;

    use super::*;
    use crate::smartcontracts::isi::offline::isi::verify_aggregate_proof_envelope;

    const OFFLINE_GENERATOR_LABEL: &[u8] = b"iroha.offline.balance.generator.H.v1";

    static PEDERSEN_H: OnceLock<RistrettoPoint> = OnceLock::new();

    fn pedersen_generator_h() -> &'static RistrettoPoint {
        PEDERSEN_H
            .get_or_init(|| RistrettoPoint::hash_from_bytes::<Sha512>(OFFLINE_GENERATOR_LABEL))
    }

    fn numeric_to_scalar(value: &Numeric) -> Scalar {
        let mantissa = value.try_mantissa_u128().expect("mantissa");
        let mut bytes = [0u8; 32];
        bytes[..16].copy_from_slice(&mantissa.to_le_bytes());
        Scalar::from_bytes_mod_order(bytes)
    }

    fn numeric_to_le_bytes(value: &Numeric) -> [u8; 16] {
        let mantissa = value.try_mantissa_u128().expect("mantissa");
        let signed = i128::try_from(mantissa).expect("i128");
        signed.to_le_bytes()
    }

    fn blinding_scalar_from_seed(seed: &OfflineProofBlindingSeed) -> Scalar {
        let mut hasher = Blake2bVar::new(64).expect("valid Blake2b length");
        hasher.update(OFFLINE_FASTPQ_HKDF_DOMAIN);
        hasher.update(seed.hkdf_salt.as_ref());
        hasher.update(&seed.counter.to_be_bytes());
        let mut output = [0u8; 64];
        hasher
            .finalize_variable(&mut output)
            .expect("output size matches");
        Scalar::from_bytes_mod_order_wide(&output)
    }

    fn sum_proof_nonce(
        bundle_id: &Hash,
        certificate_id: &Hash,
        receipts_root: &PoseidonDigest,
        delta_le: &[u8; 16],
        blind_sum: &Scalar,
    ) -> Scalar {
        let mut hasher = Blake2bVar::new(64).expect("valid Blake2b length");
        hasher.update(OFFLINE_FASTPQ_SUM_NONCE_DOMAIN);
        hasher.update(bundle_id.as_ref());
        hasher.update(certificate_id.as_ref());
        hasher.update(receipts_root.as_bytes());
        hasher.update(delta_le);
        hasher.update(&blind_sum.to_bytes());
        let mut output = [0u8; 64];
        hasher
            .finalize_variable(&mut output)
            .expect("output size matches");
        Scalar::from_bytes_mod_order_wide(&output)
    }

    fn sum_proof_challenge(
        bundle_id: &Hash,
        certificate_id: &Hash,
        receipts_root: &PoseidonDigest,
        c_init: &RistrettoPoint,
        c_res: &RistrettoPoint,
        delta_le: &[u8; 16],
        r_point: &RistrettoPoint,
    ) -> Scalar {
        let mut hasher = Blake2bVar::new(64).expect("valid Blake2b length");
        hasher.update(OFFLINE_FASTPQ_SUM_PROOF_DOMAIN);
        hasher.update(bundle_id.as_ref());
        hasher.update(certificate_id.as_ref());
        hasher.update(receipts_root.as_bytes());
        hasher.update(c_init.compress().as_bytes());
        hasher.update(c_res.compress().as_bytes());
        hasher.update(delta_le);
        hasher.update(r_point.compress().as_bytes());
        let mut output = [0u8; 64];
        hasher
            .finalize_variable(&mut output)
            .expect("output size matches");
        Scalar::from_bytes_mod_order_wide(&output)
    }

    fn replay_chain(head: Hash, tx_ids: &[Hash]) -> Hash {
        let mut current = head;
        for tx_id in tx_ids {
            let mut buf =
                Vec::with_capacity(OFFLINE_FASTPQ_REPLAY_CHAIN_DOMAIN.len() + Hash::LENGTH * 2);
            buf.extend_from_slice(OFFLINE_FASTPQ_REPLAY_CHAIN_DOMAIN);
            buf.extend_from_slice(current.as_ref());
            buf.extend_from_slice(tx_id.as_ref());
            current = Hash::new(buf);
        }
        current
    }

    fn counter_digest(
        bundle_id: &Hash,
        receipts_root: &PoseidonDigest,
        checkpoint: u64,
        counters: &[u64],
    ) -> Hash {
        let mut buf = Vec::with_capacity(
            OFFLINE_FASTPQ_COUNTER_PROOF_DOMAIN.len()
                + Hash::LENGTH
                + Hash::LENGTH
                + 8
                + counters.len() * 8,
        );
        buf.extend_from_slice(OFFLINE_FASTPQ_COUNTER_PROOF_DOMAIN);
        buf.extend_from_slice(bundle_id.as_ref());
        buf.extend_from_slice(receipts_root.as_bytes());
        buf.extend_from_slice(&checkpoint.to_be_bytes());
        for counter in counters {
            buf.extend_from_slice(&counter.to_be_bytes());
        }
        Hash::new(buf)
    }

    fn replay_digest(
        bundle_id: &Hash,
        receipts_root: &PoseidonDigest,
        head: &Hash,
        tail: &Hash,
        tx_ids: &[Hash],
    ) -> Hash {
        let mut buf = Vec::with_capacity(
            OFFLINE_FASTPQ_REPLAY_PROOF_DOMAIN.len()
                + Hash::LENGTH * 3
                + Hash::LENGTH * tx_ids.len(),
        );
        buf.extend_from_slice(OFFLINE_FASTPQ_REPLAY_PROOF_DOMAIN);
        buf.extend_from_slice(bundle_id.as_ref());
        buf.extend_from_slice(receipts_root.as_bytes());
        buf.extend_from_slice(head.as_ref());
        buf.extend_from_slice(tail.as_ref());
        for tx_id in tx_ids {
            buf.extend_from_slice(tx_id.as_ref());
        }
        Hash::new(buf)
    }

    #[test]
    fn aggregate_proof_accepts_matching_root() {
        let transfer = sample_transfer_with_aggregate_proof();
        verify_aggregate_proof_envelope(&transfer, OfflineProofMode::Optional)
            .expect("aggregate proof should verify");
    }

    #[test]
    fn aggregate_proof_rejects_root_mismatch() {
        let mut transfer = sample_transfer_with_aggregate_proof();
        if let Some(envelope) = transfer.aggregate_proof.as_mut() {
            envelope.receipts_root = PoseidonDigest::zero();
        }
        assert!(verify_aggregate_proof_envelope(&transfer, OfflineProofMode::Optional).is_err());
    }

    #[test]
    fn aggregate_proof_rejects_version_mismatch() {
        let mut transfer = sample_transfer_with_aggregate_proof();
        if let Some(envelope) = transfer.aggregate_proof.as_mut() {
            envelope.version = AGGREGATE_PROOF_VERSION_V1 + 1;
        }
        assert!(verify_aggregate_proof_envelope(&transfer, OfflineProofMode::Optional).is_err());
    }

    #[test]
    fn aggregate_proof_required_rejects_missing() {
        let mut transfer = sample_transfer_with_aggregate_proof();
        transfer.aggregate_proof = None;
        assert!(verify_aggregate_proof_envelope(&transfer, OfflineProofMode::Required).is_err());
    }

    #[test]
    fn aggregate_proof_required_rejects_missing_components() {
        let mut transfer = sample_transfer_with_aggregate_proof();
        if let Some(envelope) = transfer.aggregate_proof.as_mut() {
            envelope.proof_sum = None;
        }
        assert!(verify_aggregate_proof_envelope(&transfer, OfflineProofMode::Required).is_err());
    }

    #[test]
    fn aggregate_proof_required_rejects_empty_components() {
        let mut transfer = sample_transfer_with_aggregate_proof();
        if let Some(envelope) = transfer.aggregate_proof.as_mut() {
            envelope.proof_counter = Some(Vec::new());
        }
        assert!(verify_aggregate_proof_envelope(&transfer, OfflineProofMode::Required).is_err());
    }

    #[test]
    fn aggregate_proof_v2_required_rejects_missing_sum_only() {
        let mut transfer = sample_transfer_with_aggregate_proof_v2();
        if let Some(envelope) = transfer.aggregate_proof.as_mut() {
            envelope.proof_sum = None;
        }
        let err = verify_aggregate_proof_envelope(&transfer, OfflineProofMode::Required)
            .expect_err("v2 missing sum must fail");
        let message = err.to_string();
        assert!(
            message.contains("aggregate proofs missing: sum"),
            "unexpected error: {message}"
        );
        assert!(
            !message.contains("counter") && !message.contains("replay"),
            "v2 required mode should not require counter/replay: {message}"
        );
    }

    #[test]
    fn aggregate_proof_v2_required_skips_legacy_missing_checks() {
        let transfer = sample_transfer_with_aggregate_proof_v2();
        let err = verify_aggregate_proof_envelope(&transfer, OfflineProofMode::Required)
            .expect_err("v2 placeholder payload must fail in v2 verification path");
        let message = err.to_string();
        assert!(
            !message.contains("aggregate proofs missing: counter")
                && !message.contains("aggregate proofs missing: replay"),
            "v2 required mode should not require counter/replay: {message}"
        );
    }

    #[test]
    fn aggregate_proof_optional_accepts_missing() {
        let mut transfer = sample_transfer_with_aggregate_proof();
        transfer.aggregate_proof = None;
        verify_aggregate_proof_envelope(&transfer, OfflineProofMode::Optional)
            .expect("missing proofs allowed in optional mode");
    }

    #[test]
    fn aggregate_proof_rejects_invalid_sum_proof() {
        let mut transfer = sample_transfer_with_aggregate_proof();
        if let Some(envelope) = transfer.aggregate_proof.as_mut() {
            if let Some(mut proof_sum) = envelope.proof_sum.clone() {
                proof_sum[0] ^= 0xFF;
                envelope.proof_sum = Some(proof_sum);
            }
        }
        assert!(verify_aggregate_proof_envelope(&transfer, OfflineProofMode::Optional).is_err());
    }

    #[allow(clippy::too_many_lines)]
    fn sample_transfer_with_aggregate_proof() -> OfflineToOnlineTransfer {
        let controller: AccountId = test_account_id(1);
        let receiver: AccountId = test_account_id(2);
        let asset_definition: AssetDefinitionId =
            AssetDefinitionId::new("agg".parse().unwrap(), "xor".parse().unwrap());
        let asset = AssetId::new(asset_definition, controller.clone());
        let spend_public_key = controller.signatory().clone();
        let c_init = RistrettoPoint::hash_from_bytes::<Sha512>(b"agg-commitment");
        let init_commitment_bytes = c_init.compress().to_bytes().to_vec();
        let certificate = OfflineWalletCertificate {
            controller: controller.clone(),
            operator: controller.clone(),
            allowance: OfflineAllowanceCommitment {
                asset: asset.clone(),
                amount: Numeric::new(1_000, 0),
                commitment: init_commitment_bytes.clone(),
            },
            spend_public_key,
            attestation_report: Vec::new(),
            issued_at_ms: 1_700_000_000,
            expires_at_ms: 1_900_000_000,
            policy: OfflineWalletPolicy {
                max_balance: Numeric::new(5_000, 0),
                max_tx_value: Numeric::new(1_000, 0),
                expires_at_ms: 1_900_000_000,
            },
            operator_signature: Signature::from_bytes(&[0; 64]),
            metadata: Metadata::default(),
            verdict_id: None,
            attestation_nonce: None,
            refresh_at_ms: None,
        };

        let receipt_a = OfflineSpendReceipt {
            tx_id: Hash::new(b"agg-receipt-a"),
            from: controller.clone(),
            to: receiver.clone(),
            asset: asset.clone(),
            amount: Numeric::new(10, 0),
            issued_at_ms: 1_700_000_100,
            invoice_id: "invoice-agg-a".into(),
            platform_proof: OfflinePlatformProof::AppleAppAttest(AppleAppAttestProof {
                key_id: BASE64_STANDARD.encode(b"agg-key"),
                counter: 10,
                assertion: Vec::new(),
                challenge_hash: Hash::new(b"challenge-a"),
            }),
            platform_snapshot: None,
            sender_certificate_id: certificate.certificate_id(),
            sender_signature: Signature::from_bytes(&[1; 64]),
            build_claim: None,
        };
        let receipt_b = OfflineSpendReceipt {
            tx_id: Hash::new(b"agg-receipt-b"),
            from: controller.clone(),
            to: receiver.clone(),
            asset: asset.clone(),
            amount: Numeric::new(15, 0),
            issued_at_ms: 1_700_000_200,
            invoice_id: "invoice-agg-b".into(),
            platform_proof: OfflinePlatformProof::AppleAppAttest(AppleAppAttestProof {
                key_id: BASE64_STANDARD.encode(b"agg-key"),
                counter: 11,
                assertion: Vec::new(),
                challenge_hash: Hash::new(b"challenge-b"),
            }),
            platform_snapshot: None,
            sender_certificate_id: certificate.certificate_id(),
            sender_signature: Signature::from_bytes(&[1; 64]),
            build_claim: None,
        };

        let receipts = vec![receipt_a, receipt_b];
        let claimed_delta = Numeric::new(25, 0);
        let certificate_id = certificate.certificate_id();
        let blind_sum = receipts
            .iter()
            .map(|receipt| {
                OfflineProofBlindingSeed::derive(certificate_id, receipt.platform_proof.counter())
            })
            .map(|seed| blinding_scalar_from_seed(&seed))
            .fold(Scalar::ZERO, |acc, scalar| acc + scalar);
        let delta_scalar = numeric_to_scalar(&claimed_delta);
        let expected_delta =
            RISTRETTO_BASEPOINT_POINT * delta_scalar + *pedersen_generator_h() * blind_sum;
        let c_res = c_init + expected_delta;
        let resulting_commitment = c_res.compress().to_bytes().to_vec();

        let balance_proof = OfflineBalanceProof {
            initial_commitment: certificate.allowance.clone(),
            resulting_commitment: resulting_commitment.clone(),
            claimed_delta: claimed_delta.clone(),
            zk_proof: None,
        };

        let receipts_root = compute_receipts_root(&receipts).expect("receipts root");
        let bundle_id = Hash::new(b"bundle-agg");
        let delta_le = numeric_to_le_bytes(&claimed_delta);
        let nonce = sum_proof_nonce(
            &bundle_id,
            &certificate_id,
            &receipts_root,
            &delta_le,
            &blind_sum,
        );
        let r_point = *pedersen_generator_h() * nonce;
        let challenge = sum_proof_challenge(
            &bundle_id,
            &certificate_id,
            &receipts_root,
            &c_init,
            &c_res,
            &delta_le,
            &r_point,
        );
        let s_scalar = nonce + challenge * blind_sum;
        let sum_proof = OfflineFastpqSumProof {
            version: OFFLINE_FASTPQ_PROOF_VERSION_V1,
            receipts_root,
            r_point: r_point.compress().to_bytes(),
            s_scalar: s_scalar.to_bytes(),
        };
        let proof_sum = norito::to_bytes(&sum_proof).expect("sum proof bytes");

        let counters: Vec<u64> = receipts
            .iter()
            .map(|receipt| receipt.platform_proof.counter())
            .collect();
        let counter_checkpoint = counters
            .first()
            .copied()
            .unwrap()
            .checked_sub(1)
            .expect("counter checkpoint");
        let counter_proof = OfflineFastpqCounterProof {
            version: OFFLINE_FASTPQ_PROOF_VERSION_V1,
            receipts_root,
            counter_checkpoint,
            digest: counter_digest(&bundle_id, &receipts_root, counter_checkpoint, &counters),
        };
        let proof_counter = norito::to_bytes(&counter_proof).expect("counter proof bytes");

        let replay_log_head = Hash::new(b"replay-head");
        let tx_ids: Vec<Hash> = receipts.iter().map(|receipt| receipt.tx_id).collect();
        let replay_log_tail = replay_chain(replay_log_head, &tx_ids);
        let replay_proof = OfflineFastpqReplayProof {
            version: OFFLINE_FASTPQ_PROOF_VERSION_V1,
            receipts_root,
            replay_log_head,
            replay_log_tail,
            digest: replay_digest(
                &bundle_id,
                &receipts_root,
                &replay_log_head,
                &replay_log_tail,
                &tx_ids,
            ),
        };
        let proof_replay = norito::to_bytes(&replay_proof).expect("replay proof bytes");

        let aggregate_proof = AggregateProofEnvelope {
            version: AGGREGATE_PROOF_VERSION_V1,
            receipts_root,
            proof_sum: Some(proof_sum),
            proof_counter: Some(proof_counter),
            proof_replay: Some(proof_replay),
            metadata: Metadata::default(),
        };

        OfflineToOnlineTransfer {
            bundle_id,
            receiver,
            deposit_account: certificate.controller.clone(),
            receipts,
            balance_proof,
            balance_proofs: None,
            aggregate_proof: Some(aggregate_proof),
            attachments: None,
            platform_snapshot: None,
        }
    }

    fn sample_transfer_with_aggregate_proof_v2() -> OfflineToOnlineTransfer {
        let mut transfer = sample_transfer_with_aggregate_proof();
        let public_inputs_b64 = BASE64_STANDARD.encode([0u8]);

        let mut metadata = Metadata::default();
        metadata.insert(
            AGGREGATE_PROOF_METADATA_OFFLINE_AGGREGATE_BACKEND
                .parse()
                .expect("metadata key"),
            "stark/fri/poseidon2-goldilocks",
        );
        metadata.insert(
            AGGREGATE_PROOF_METADATA_OFFLINE_AGGREGATE_CIRCUIT_ID
                .parse()
                .expect("metadata key"),
            "offline-merge",
        );
        metadata.insert(
            AGGREGATE_PROOF_METADATA_OFFLINE_AGGREGATE_PUBLIC_INPUTS_B64
                .parse()
                .expect("metadata key"),
            public_inputs_b64.as_str(),
        );
        metadata.insert(
            AGGREGATE_PROOF_METADATA_OFFLINE_AGGREGATE_RECURSION_DEPTH
                .parse()
                .expect("metadata key"),
            "\"1\"",
        );

        if let Some(envelope) = transfer.aggregate_proof.as_mut() {
            envelope.version = AGGREGATE_PROOF_VERSION_V2;
            envelope.proof_sum = Some(vec![0u8]);
            envelope.proof_counter = None;
            envelope.proof_replay = None;
            envelope.metadata = metadata;
        }

        transfer
    }

    fn test_account_id(seed: u8) -> AccountId {
        let pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        AccountId::new(pair.public_key().clone())
    }
}
