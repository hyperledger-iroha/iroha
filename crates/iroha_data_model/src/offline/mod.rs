//! Offline allowance certificates, platform proofs, and deposit bundles.

#[allow(unused_imports)]
use core::{cmp::Ordering, fmt, str::FromStr};
use std::collections::{BTreeMap, BTreeSet};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use iroha_crypto::{Hash, PublicKey, Signature};
use iroha_data_model_derive::model;
use iroha_primitives::{json::Json, numeric::Numeric};
use iroha_schema::IntoSchema;
use norito::{
    codec::{Decode, Encode},
    to_bytes,
};
use thiserror::Error;

pub use self::model::*;
use crate::{
    ChainId, account::AccountId, asset::AssetId, metadata::Metadata, name::Name,
    proof::ProofAttachmentList,
};

mod poseidon;
pub use poseidon::*;

/// Prefix embedded into offline instruction rejection messages.
///
/// The mobile SDKs parse the label after this prefix up to the first `:` to
/// recover stable machine-readable error codes such as `certificate_expired`,
/// `counter_conflict` (alias for `counter_violation`), or `allowance_exceeded`
/// (alias for `allowance_depleted`).
pub const OFFLINE_REJECTION_REASON_PREFIX: &str = "offline_reason::";
/// Asset-definition metadata key that enables offline allowances and escrow tracking.
pub const OFFLINE_ASSET_ENABLED_METADATA_KEY: &str = "offline.enabled";
/// Certificate metadata key carrying lineage scope identifier.
pub const OFFLINE_LINEAGE_SCOPE_KEY: &str = "offline.lineage.scope";
/// Certificate metadata key carrying lineage epoch.
pub const OFFLINE_LINEAGE_EPOCH_KEY: &str = "offline.lineage.epoch";
/// Certificate metadata key carrying previous certificate id (hex) for renewals.
pub const OFFLINE_LINEAGE_PREV_CERTIFICATE_ID_HEX_KEY: &str =
    "offline.lineage.prev_certificate_id_hex";
/// Certificate metadata key carrying minimum accepted app build number.
pub const OFFLINE_BUILD_CLAIM_MIN_BUILD_NUMBER_KEY: &str = "offline.build_claim.min_build_number";

/// Prefix used for Android marker-key series counter scopes.
pub const MARKER_COUNTER_PREFIX: &str = "marker::";

/// Prefix used for Android provisioned counter scopes.
pub const PROVISIONED_COUNTER_PREFIX: &str = "provisioned::";

fn remaining_amount_default() -> Numeric {
    Numeric::zero()
}

fn invalid_counter_scope_error(
    platform: OfflineTransferRejectionPlatform,
    reason: impl Into<String>,
) -> OfflineProofRequestError {
    OfflineProofRequestError::InvalidCounterScope {
        platform,
        reason: reason.into(),
    }
}

/// Canonicalize an iOS App Attest key id to standard base64.
///
/// # Errors
///
/// Returns [`OfflineProofRequestError::InvalidCounterScope`] if the key id is empty,
/// not valid base64, or not in canonical standard base64 form.
pub fn canonical_app_attest_key_id(key_id: &str) -> Result<String, OfflineProofRequestError> {
    if key_id.trim().is_empty() {
        return Err(invalid_counter_scope_error(
            OfflineTransferRejectionPlatform::Apple,
            "apple_app_attest.key_id must be non-empty",
        ));
    }
    let bytes = BASE64_STANDARD.decode(key_id.as_bytes()).map_err(|_| {
        invalid_counter_scope_error(
            OfflineTransferRejectionPlatform::Apple,
            "apple_app_attest.key_id must use standard base64 encoding",
        )
    })?;
    let canonical = BASE64_STANDARD.encode(bytes);
    if canonical != key_id {
        return Err(invalid_counter_scope_error(
            OfflineTransferRejectionPlatform::Apple,
            "apple_app_attest.key_id must use canonical base64 encoding",
        ));
    }
    Ok(canonical)
}

/// Derive the canonical Android marker-key series identifier from a raw public key.
///
/// # Errors
///
/// Returns [`OfflineProofRequestError::InvalidCounterScope`] if the public key is empty.
pub fn marker_series_from_public_key(
    marker_public_key: &[u8],
) -> Result<String, OfflineProofRequestError> {
    if marker_public_key.is_empty() {
        return Err(invalid_counter_scope_error(
            OfflineTransferRejectionPlatform::Android,
            "marker_public_key must be non-empty",
        ));
    }
    let hash = Hash::new(marker_public_key);
    Ok(format!("{MARKER_COUNTER_PREFIX}{hash}"))
}

/// Return receipts sorted by `(counter, tx_id)` for deterministic processing.
#[must_use]
pub fn canonical_receipts(receipts: &[OfflineSpendReceipt]) -> Vec<&OfflineSpendReceipt> {
    let mut ordered: Vec<&OfflineSpendReceipt> = receipts.iter().collect();
    ordered.sort_by(|lhs, rhs| receipt_cmp(lhs, rhs));
    ordered
}

/// Return true when receipts are already ordered by `(counter, tx_id)`.
#[must_use]
pub fn receipts_are_canonical(receipts: &[OfflineSpendReceipt]) -> bool {
    receipts
        .windows(2)
        .all(|pair| receipt_cmp(&pair[0], &pair[1]) != Ordering::Greater)
}

/// Validate that receipts share a single counter scope.
///
/// # Errors
///
/// Returns [`OfflineProofRequestError::MissingReceipts`] when receipts are empty,
/// [`OfflineProofRequestError::MixedCounterScopes`] when receipts mix scopes, or
/// [`OfflineProofRequestError::InvalidCounterScope`] when a receipt scope is malformed.
pub fn ensure_single_counter_scope(
    receipts: &[OfflineSpendReceipt],
) -> Result<(), OfflineProofRequestError> {
    let mut iter = receipts.iter();
    let first = iter
        .next()
        .ok_or(OfflineProofRequestError::MissingReceipts)?;
    let expected = receipt_counter_scope(first)?;
    for receipt in iter {
        let current = receipt_counter_scope(receipt)?;
        if current != expected {
            return Err(OfflineProofRequestError::MixedCounterScopes);
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CounterScopeKind {
    Apple,
    AndroidMarker,
    AndroidProvisioned,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CounterScope {
    kind: CounterScopeKind,
    scope: String,
}

fn receipt_counter_scope(
    receipt: &OfflineSpendReceipt,
) -> Result<CounterScope, OfflineProofRequestError> {
    match &receipt.platform_proof {
        OfflinePlatformProof::AppleAppAttest(proof) => {
            let key_id = canonical_app_attest_key_id(&proof.key_id)?;
            Ok(CounterScope {
                kind: CounterScopeKind::Apple,
                scope: key_id,
            })
        }
        OfflinePlatformProof::AndroidMarkerKey(proof) => {
            let derived = marker_series_from_public_key(&proof.marker_public_key)?;
            if proof.series != derived {
                return Err(invalid_counter_scope_error(
                    OfflineTransferRejectionPlatform::Android,
                    "marker series does not match marker_public_key",
                ));
            }
            Ok(CounterScope {
                kind: CounterScopeKind::AndroidMarker,
                scope: derived,
            })
        }
        OfflinePlatformProof::Provisioned(proof) => Ok(CounterScope {
            kind: CounterScopeKind::AndroidProvisioned,
            scope: provisioned_counter_scope(proof)?,
        }),
    }
}

fn provisioned_counter_scope(
    proof: &AndroidProvisionedProof,
) -> Result<String, OfflineProofRequestError> {
    let schema = proof.manifest_schema.trim();
    if schema.is_empty() {
        return Err(invalid_counter_scope_error(
            OfflineTransferRejectionPlatform::Android,
            "provisioned manifest_schema must be non-empty",
        ));
    }
    let device_id = provisioned_device_id(&proof.device_manifest)?;
    Ok(format!("{PROVISIONED_COUNTER_PREFIX}{schema}::{device_id}"))
}

fn provisioned_device_id(manifest: &Metadata) -> Result<String, OfflineProofRequestError> {
    let name = Name::from_str(ANDROID_PROVISIONED_DEVICE_ID_KEY).map_err(|err| {
        let _ = err;
        invalid_counter_scope_error(
            OfflineTransferRejectionPlatform::Android,
            "invalid android.provisioned.device_id metadata key",
        )
    })?;
    let value = manifest.get(&name).ok_or_else(|| {
        invalid_counter_scope_error(
            OfflineTransferRejectionPlatform::Android,
            "provisioned device_manifest missing android.provisioned.device_id",
        )
    })?;
    let device_id: String = value.try_into_any().map_err(|err| {
        let _ = err;
        invalid_counter_scope_error(
            OfflineTransferRejectionPlatform::Android,
            "provisioned device_id must be a string",
        )
    })?;
    let trimmed = device_id.trim();
    if trimmed.is_empty() {
        return Err(invalid_counter_scope_error(
            OfflineTransferRejectionPlatform::Android,
            "provisioned device_id must be non-empty",
        ));
    }
    Ok(trimmed.to_string())
}

fn receipt_cmp(lhs: &OfflineSpendReceipt, rhs: &OfflineSpendReceipt) -> Ordering {
    match lhs
        .platform_proof
        .counter()
        .cmp(&rhs.platform_proof.counter())
    {
        Ordering::Equal => lhs.tx_id.cmp(&rhs.tx_id),
        other => other,
    }
}

/// Canonical payload signed by operators when issuing offline wallet certificates.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
pub struct OfflineWalletCertificatePayload {
    /// Account that owns the allowance.
    pub controller: AccountId,
    /// Operator account that signs this certificate.
    pub operator: AccountId,
    /// Commitment to the allowance this certificate governs.
    pub allowance: OfflineAllowanceCommitment,
    /// Spend public key baked into the wallet.
    pub spend_public_key: PublicKey,
    /// Serialized device attestation report (App Attest, `KeyMint`, etc.).
    pub attestation_report: Vec<u8>,
    /// Issuance timestamp (unix ms).
    pub issued_at_ms: u64,
    /// Expiry timestamp (unix ms).
    pub expires_at_ms: u64,
    /// Policy knobs enforced for this wallet.
    pub policy: OfflineWalletPolicy,
    /// Additional metadata supplied by the issuer.
    pub metadata: Metadata,
    /// Optional unique identifier for the cached attestation verdict.
    pub verdict_id: Option<Hash>,
    /// Optional nonce supplied to the attestation provider.
    pub attestation_nonce: Option<Hash>,
    /// Optional timestamp (unix ms) indicating when the attestation/verdict must be refreshed.
    pub refresh_at_ms: Option<u64>,
}

impl From<&OfflineWalletCertificate> for OfflineWalletCertificatePayload {
    fn from(cert: &OfflineWalletCertificate) -> Self {
        Self {
            controller: cert.controller.clone(),
            operator: cert.operator.clone(),
            allowance: cert.allowance.clone(),
            spend_public_key: cert.spend_public_key.clone(),
            attestation_report: cert.attestation_report.clone(),
            issued_at_ms: cert.issued_at_ms,
            expires_at_ms: cert.expires_at_ms,
            policy: cert.policy.clone(),
            metadata: cert.metadata.clone(),
            verdict_id: cert.verdict_id,
            attestation_nonce: cert.attestation_nonce,
            refresh_at_ms: cert.refresh_at_ms,
        }
    }
}

/// Canonical payload signed by spend keys when emitting offline receipts.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
pub struct OfflineSpendReceiptPayload {
    /// Deterministic identifier for the offline spend.
    pub tx_id: Hash,
    /// Sender offline account.
    pub from: AccountId,
    /// Receiver offline account (payer's target).
    pub to: AccountId,
    /// Asset identifier being transferred.
    pub asset: AssetId,
    /// Amount received.
    pub amount: Numeric,
    /// Unix timestamp (ms) when the receipt was issued.
    pub issued_at_ms: u64,
    /// Invoice identifier provided by the receiver.
    pub invoice_id: String,
    /// Platform-specific counter proof.
    pub platform_proof: OfflinePlatformProof,
    /// Identifier of the sender's registered certificate.
    pub sender_certificate_id: Hash,
}

impl From<&OfflineSpendReceipt> for OfflineSpendReceiptPayload {
    fn from(receipt: &OfflineSpendReceipt) -> Self {
        Self {
            tx_id: receipt.tx_id,
            from: receipt.from.clone(),
            to: receipt.to.clone(),
            asset: receipt.asset.clone(),
            amount: receipt.amount.clone(),
            issued_at_ms: receipt.issued_at_ms,
            invoice_id: receipt.invoice_id.clone(),
            platform_proof: receipt.platform_proof.clone(),
            sender_certificate_id: receipt.sender_certificate_id,
        }
    }
}

/// Canonical payload hashed when deriving receipt challenges.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
pub struct OfflineReceiptChallengePreimage {
    /// Invoice identifier chosen by the receiver.
    pub invoice_id: String,
    /// Receiver account identifier.
    pub receiver: AccountId,
    /// Asset identifier being transferred.
    pub asset: AssetId,
    /// Amount credited to the receiver.
    pub amount: Numeric,
    /// Unix timestamp (ms) when the receipt was issued.
    pub issued_at_ms: u64,
    /// Identifier of the sender's registered certificate.
    pub sender_certificate_id: Hash,
    /// Nonce supplied by the sender (currently the receipt transaction id).
    pub nonce: Hash,
}

impl OfflineReceiptChallengePreimage {
    /// Construct a challenge preimage directly from the receipt that produced it.
    #[must_use]
    pub fn from_receipt(receipt: &OfflineSpendReceipt) -> Self {
        Self {
            invoice_id: receipt.invoice_id.clone(),
            receiver: receipt.to.clone(),
            asset: receipt.asset.clone(),
            amount: receipt.amount.clone(),
            issued_at_ms: receipt.issued_at_ms,
            sender_certificate_id: receipt.sender_certificate_id,
            nonce: receipt.tx_id,
        }
    }

    /// Serialize the preimage using the Norito codec.
    ///
    /// # Errors
    ///
    /// Returns an error when serialization fails.
    pub fn to_bytes(&self) -> Result<Vec<u8>, norito::Error> {
        to_bytes(self)
    }

    /// Compute the canonical hash of this preimage using the workspace hash primitive.
    ///
    /// # Errors
    ///
    /// Returns an error when serialization fails.
    pub fn hash(&self) -> Result<Hash, norito::Error> {
        let bytes = self.to_bytes()?;
        Ok(Hash::new(bytes))
    }

    /// Compute a chain-bound challenge hash using `Hash(chain_id) || preimage_bytes`.
    ///
    /// # Errors
    ///
    /// Returns an error when serialization fails.
    pub fn hash_with_chain_id(&self, chain_id: &ChainId) -> Result<Hash, norito::Error> {
        let bytes = self.to_bytes()?;
        Ok(chain_bound_receipt_hash(chain_id, &bytes))
    }
}

/// Derive a chain-bound receipt challenge hash using `Hash(chain_id) || preimage_bytes`.
#[must_use]
pub fn chain_bound_receipt_hash(chain_id: &ChainId, preimage_bytes: &[u8]) -> Hash {
    let context = Hash::new(chain_id.as_str().as_bytes());
    let mut data = Vec::with_capacity(Hash::LENGTH + preimage_bytes.len());
    data.extend_from_slice(context.as_ref());
    data.extend_from_slice(preimage_bytes);
    Hash::new(data)
}

/// Canonical payload operators sign when publishing POS backend manifests.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
pub struct OfflinePosProvisionManifestPayload {
    /// Logical identifier for this manifest (used by SDKs/POS devices for pinning).
    pub manifest_id: String,
    /// Rotation sequence number (monotonic).
    pub sequence: u32,
    /// Timestamp when the manifest was published (unix ms).
    pub published_at_ms: u64,
    /// Timestamp when the manifest becomes active (unix ms).
    pub valid_from_ms: u64,
    /// Timestamp when the manifest expires (unix ms).
    pub valid_until_ms: u64,
    /// Optional hint for the next rotation or refresh deadline (unix ms).
    pub rotation_hint_ms: Option<u64>,
    /// Operator account responsible for provisioning POS devices.
    pub operator: AccountId,
    /// Backend root entries pinned by the manifest.
    pub backend_roots: Vec<OfflinePosBackendRoot>,
    /// Additional metadata surfaced to POS clients.
    pub metadata: Metadata,
}

impl From<&OfflinePosProvisionManifest> for OfflinePosProvisionManifestPayload {
    fn from(manifest: &OfflinePosProvisionManifest) -> Self {
        Self {
            manifest_id: manifest.manifest_id.clone(),
            sequence: manifest.sequence,
            published_at_ms: manifest.published_at_ms,
            valid_from_ms: manifest.valid_from_ms,
            valid_until_ms: manifest.valid_until_ms,
            rotation_hint_ms: manifest.rotation_hint_ms,
            operator: manifest.operator.clone(),
            backend_roots: manifest.backend_roots.clone(),
            metadata: manifest.metadata.clone(),
        }
    }
}

impl OfflineWalletCertificate {
    /// Deterministic identifier for this certificate (`BLAKE2b` over its Norito encoding).
    pub fn certificate_id(&self) -> Hash {
        let bytes = to_bytes(self).expect("offline wallet certificate serialization must succeed");
        Hash::new(bytes)
    }

    /// Account that controls the allowance described by this certificate.
    #[must_use]
    pub fn controller(&self) -> &AccountId {
        &self.controller
    }

    /// Canonical payload operators must sign when issuing certificates.
    ///
    /// # Errors
    ///
    /// Returns an error when the payload cannot be serialized with Norito.
    pub fn operator_signing_bytes(&self) -> Result<Vec<u8>, norito::Error> {
        let payload = OfflineWalletCertificatePayload::from(self);
        to_bytes(&payload)
    }

    /// Returns typed Android integrity metadata when present on this certificate.
    ///
    /// # Errors
    ///
    /// Returns an error when the metadata entries are malformed.
    pub fn android_integrity_metadata(
        &self,
    ) -> Result<Option<AndroidIntegrityMetadata>, AndroidIntegrityMetadataError> {
        AndroidIntegrityMetadata::from_metadata(&self.metadata)
    }
}

impl OfflinePosProvisionManifest {
    /// Canonical payload operators must sign when publishing provisioning manifests.
    ///
    /// # Errors
    ///
    /// Returns an error when the payload cannot be serialized with Norito.
    pub fn operator_signing_bytes(&self) -> Result<Vec<u8>, norito::Error> {
        let payload = OfflinePosProvisionManifestPayload::from(self);
        to_bytes(&payload)
    }
}

impl OfflineToOnlineTransfer {
    /// Access the receipts included in this bundle.
    pub fn receipts(&self) -> &[OfflineSpendReceipt] {
        &self.receipts
    }

    /// Access the designated offline receiver for this bundle.
    pub fn receiver(&self) -> &AccountId {
        &self.receiver
    }

    /// Access the online account that receives the deposit.
    pub fn deposit_account(&self) -> &AccountId {
        &self.deposit_account
    }

    /// Access the commitment proof describing the deposited delta.
    pub fn balance_proof(&self) -> &OfflineBalanceProof {
        &self.balance_proof
    }

    /// Access optional per-certificate commitment proofs for multi-allowance bundles.
    #[must_use]
    pub fn balance_proofs(&self) -> Option<&[OfflineCertificateBalanceProof]> {
        self.balance_proofs.as_deref()
    }

    /// Resolve the commitment proof that corresponds to `certificate_id`.
    ///
    /// Falls back to the legacy `balance_proof` field when no explicit map is
    /// attached and the bundle is single-certificate.
    #[must_use]
    pub fn balance_proof_for_certificate(
        &self,
        certificate_id: Hash,
    ) -> Option<&OfflineBalanceProof> {
        if let Some(mapped) = self.balance_proofs.as_ref() {
            return mapped
                .iter()
                .find(|entry| entry.sender_certificate_id == certificate_id)
                .map(|entry| &entry.balance_proof);
        }
        self.primary_certificate_id()
            .filter(|id| *id == certificate_id)
            .map(|_| &self.balance_proof)
    }

    /// Borrow the aggregate proof envelope if the bundle carries one.
    #[must_use]
    pub fn aggregate_proof(&self) -> Option<&AggregateProofEnvelope> {
        self.aggregate_proof.as_ref()
    }

    /// Borrow the primary sender certificate identifier used by this bundle, if any.
    #[must_use]
    pub fn primary_certificate_id(&self) -> Option<Hash> {
        self.receipts
            .first()
            .map(|receipt| receipt.sender_certificate_id)
    }
}

impl OfflineSpendReceipt {
    /// Receiver recorded for this receipt.
    pub fn to(&self) -> &AccountId {
        &self.to
    }

    /// Amount credited by this receipt.
    pub fn amount(&self) -> &Numeric {
        &self.amount
    }

    /// Invoice identifier recorded for this receipt.
    pub fn invoice_id(&self) -> &str {
        &self.invoice_id
    }

    /// Asset identifier being transferred.
    pub fn asset(&self) -> &AssetId {
        &self.asset
    }

    /// Canonical payload signed by the sender's spend key.
    ///
    /// # Errors
    ///
    /// Returns an error when the payload cannot be serialized with Norito.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, norito::Error> {
        let payload = OfflineSpendReceiptPayload::from(self);
        to_bytes(&payload)
    }

    /// Canonical preimage hashed by platform counters and attestations.
    #[must_use]
    pub fn challenge_preimage(&self) -> OfflineReceiptChallengePreimage {
        OfflineReceiptChallengePreimage::from_receipt(self)
    }

    /// Norito-encoded bytes that the platform proof must commit to.
    ///
    /// # Errors
    ///
    /// Returns an error when serialization fails.
    pub fn challenge_bytes(&self) -> Result<Vec<u8>, norito::Error> {
        self.challenge_preimage().to_bytes()
    }

    /// Canonical hash derived from the receipt payload.
    ///
    /// # Errors
    ///
    /// Returns an error when serialization fails.
    pub fn challenge_hash(&self) -> Result<Hash, norito::Error> {
        self.challenge_preimage().hash()
    }

    /// Canonical hash derived from the receipt payload and chain context.
    ///
    /// # Errors
    ///
    /// Returns an error when serialization fails.
    pub fn challenge_hash_with_chain_id(&self, chain_id: &ChainId) -> Result<Hash, norito::Error> {
        self.challenge_preimage().hash_with_chain_id(chain_id)
    }
}

impl OfflineBalanceProof {
    /// Claimed delta encoded by the bundle.
    pub fn claimed_delta(&self) -> &Numeric {
        &self.claimed_delta
    }
}

const ANDROID_PACKAGE_NAMES_KEY: &str = "android.attestation.package_names";
const ANDROID_SIGNATURE_DIGESTS_KEY: &str = "android.attestation.signing_digests_sha256";
const ANDROID_REQUIRE_STRONGBOX_KEY: &str = "android.attestation.require_strongbox";
const ANDROID_REQUIRE_ROLLBACK_KEY: &str = "android.attestation.require_rollback_resistance";
const ANDROID_PLAY_PROJECT_KEY: &str = "android.play_integrity.cloud_project_number";
const ANDROID_PLAY_ENVIRONMENT_KEY: &str = "android.play_integrity.environment";
const ANDROID_PLAY_PACKAGE_NAMES_KEY: &str = "android.play_integrity.package_names";
const ANDROID_PLAY_DIGESTS_KEY: &str = "android.play_integrity.signing_digests_sha256";
const ANDROID_PLAY_APP_VERDICTS_KEY: &str = "android.play_integrity.allowed_app_verdicts";
const ANDROID_PLAY_DEVICE_VERDICTS_KEY: &str = "android.play_integrity.allowed_device_verdicts";
const ANDROID_PLAY_MAX_AGE_KEY: &str = "android.play_integrity.max_token_age_ms";
const ANDROID_HMS_APP_ID_KEY: &str = "android.hms_safety_detect.app_id";
const ANDROID_HMS_PACKAGE_NAMES_KEY: &str = "android.hms_safety_detect.package_names";
const ANDROID_HMS_DIGESTS_KEY: &str = "android.hms_safety_detect.signing_digests_sha256";
const ANDROID_HMS_EVALUATIONS_KEY: &str = "android.hms_safety_detect.required_evaluations";
const ANDROID_HMS_MAX_AGE_KEY: &str = "android.hms_safety_detect.max_token_age_ms";
const ANDROID_PROVISIONED_INSPECTOR_KEY: &str = "android.provisioned.inspector_public_key";
const ANDROID_PROVISIONED_MANIFEST_SCHEMA_KEY: &str = "android.provisioned.manifest_schema";
const ANDROID_PROVISIONED_MANIFEST_VERSION_KEY: &str = "android.provisioned.manifest_version";
const ANDROID_PROVISIONED_MAX_AGE_KEY: &str = "android.provisioned.max_manifest_age_ms";
const ANDROID_PROVISIONED_MANIFEST_DIGEST_KEY: &str = "android.provisioned.manifest_digest";
/// Certificate metadata key defining provisioned Android application identifier.
pub const ANDROID_PROVISIONED_APP_ID_KEY: &str = "android.provisioned.app_id";
#[allow(dead_code)]
const ANDROID_PROVISIONED_DEVICE_ID_KEY: &str = "android.provisioned.device_id";

/// Error surfaced when Android metadata entries are missing or malformed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum AndroidIntegrityMetadataError {
    /// Required metadata entry missing.
    #[error("metadata entry `{key}` is missing")]
    Missing {
        /// Missing key.
        key: &'static str,
    },
    /// Metadata entry exists but is malformed.
    #[error("metadata entry `{key}` is invalid: {reason}")]
    Invalid {
        /// Key that failed validation.
        key: &'static str,
        /// Underlying failure reason.
        reason: String,
    },
    /// Metadata entry is present but empty.
    #[error("metadata entry `{key}` must contain at least one value")]
    Empty {
        /// Key without values.
        key: &'static str,
    },
    /// Policy slug is not recognised.
    #[error("android integrity policy `{policy}` is not supported")]
    UnsupportedPolicy {
        /// Unsupported slug.
        policy: String,
    },
}

impl AndroidIntegrityMetadata {
    /// Attempts to parse typed Android metadata from a certificate metadata map.
    ///
    /// Returns `Ok(None)` when the certificate does not declare any Android policy.
    ///
    /// # Errors
    /// Returns [`AndroidIntegrityMetadataError`] when required metadata keys are missing, when
    /// Android fields are omitted, or when entries are malformed for the detected policy.
    pub fn from_metadata(
        metadata: &Metadata,
    ) -> Result<Option<Self>, AndroidIntegrityMetadataError> {
        let Some(policy) = detect_android_policy(metadata)? else {
            return Ok(None);
        };
        let typed = match policy {
            AndroidIntegrityPolicy::MarkerKey => {
                Self::MarkerKey(AndroidMarkerKeyMetadata::from_metadata(metadata)?)
            }
            AndroidIntegrityPolicy::PlayIntegrity => {
                Self::PlayIntegrity(AndroidPlayIntegrityMetadata::from_metadata(metadata)?)
            }
            AndroidIntegrityPolicy::HmsSafetyDetect => {
                Self::HmsSafetyDetect(AndroidHmsSafetyDetectMetadata::from_metadata(metadata)?)
            }
            AndroidIntegrityPolicy::Provisioned => {
                Self::Provisioned(AndroidProvisionedMetadata::from_metadata(metadata)?)
            }
        };
        Ok(Some(typed))
    }
}

impl AndroidMarkerKeyMetadata {
    fn from_metadata(metadata: &Metadata) -> Result<Self, AndroidIntegrityMetadataError> {
        let packages = metadata_string_list(metadata, ANDROID_PACKAGE_NAMES_KEY)?;
        let package_names = package_set(ANDROID_PACKAGE_NAMES_KEY, packages)?;
        let digests = metadata_string_list(metadata, ANDROID_SIGNATURE_DIGESTS_KEY)?;
        let signing_digests_sha256 = digest_set(ANDROID_SIGNATURE_DIGESTS_KEY, digests)?;
        let require_strongbox =
            metadata_bool(metadata, ANDROID_REQUIRE_STRONGBOX_KEY)?.unwrap_or(false);
        let require_rollback_resistance =
            metadata_bool(metadata, ANDROID_REQUIRE_ROLLBACK_KEY)?.unwrap_or(true);
        Ok(Self {
            package_names,
            signing_digests_sha256,
            require_strongbox,
            require_rollback_resistance,
        })
    }
}

impl AndroidPlayIntegrityMetadata {
    fn from_metadata(metadata: &Metadata) -> Result<Self, AndroidIntegrityMetadataError> {
        let cloud_project_number = metadata_u64(metadata, ANDROID_PLAY_PROJECT_KEY)?.ok_or(
            AndroidIntegrityMetadataError::Missing {
                key: ANDROID_PLAY_PROJECT_KEY,
            },
        )?;
        let environment_slug = metadata_string(metadata, ANDROID_PLAY_ENVIRONMENT_KEY)?.ok_or(
            AndroidIntegrityMetadataError::Missing {
                key: ANDROID_PLAY_ENVIRONMENT_KEY,
            },
        )?;
        let normalized_env = environment_slug.trim().to_ascii_lowercase();
        let environment =
            PlayIntegrityEnvironment::from_slug(&normalized_env).ok_or_else(|| {
                AndroidIntegrityMetadataError::Invalid {
                    key: ANDROID_PLAY_ENVIRONMENT_KEY,
                    reason: format!("unrecognised environment `{environment_slug}`"),
                }
            })?;
        let packages = package_set(
            ANDROID_PLAY_PACKAGE_NAMES_KEY,
            metadata_string_list(metadata, ANDROID_PLAY_PACKAGE_NAMES_KEY)?,
        )?;
        let digests = digest_set(
            ANDROID_PLAY_DIGESTS_KEY,
            metadata_string_list(metadata, ANDROID_PLAY_DIGESTS_KEY)?,
        )?;
        let app_verdicts = slug_set(
            ANDROID_PLAY_APP_VERDICTS_KEY,
            metadata_string_list(metadata, ANDROID_PLAY_APP_VERDICTS_KEY)?,
            PlayIntegrityAppVerdict::from_slug,
        )?;
        let device_verdicts = slug_set(
            ANDROID_PLAY_DEVICE_VERDICTS_KEY,
            metadata_string_list(metadata, ANDROID_PLAY_DEVICE_VERDICTS_KEY)?,
            PlayIntegrityDeviceVerdict::from_slug,
        )?;
        let max_token_age_ms = metadata_u64(metadata, ANDROID_PLAY_MAX_AGE_KEY)?;
        Ok(Self {
            cloud_project_number,
            environment,
            package_names: packages,
            signing_digests_sha256: digests,
            allowed_app_verdicts: app_verdicts,
            allowed_device_verdicts: device_verdicts,
            max_token_age_ms,
        })
    }
}

impl AndroidHmsSafetyDetectMetadata {
    fn from_metadata(metadata: &Metadata) -> Result<Self, AndroidIntegrityMetadataError> {
        let raw_app_id = metadata_string(metadata, ANDROID_HMS_APP_ID_KEY)?.ok_or(
            AndroidIntegrityMetadataError::Missing {
                key: ANDROID_HMS_APP_ID_KEY,
            },
        )?;
        let trimmed_app_id = raw_app_id.trim();
        if trimmed_app_id.is_empty() {
            return Err(AndroidIntegrityMetadataError::Invalid {
                key: ANDROID_HMS_APP_ID_KEY,
                reason: "app id must not be empty".into(),
            });
        }
        let packages = package_set(
            ANDROID_HMS_PACKAGE_NAMES_KEY,
            metadata_string_list(metadata, ANDROID_HMS_PACKAGE_NAMES_KEY)?,
        )?;
        let digests = digest_set(
            ANDROID_HMS_DIGESTS_KEY,
            metadata_string_list(metadata, ANDROID_HMS_DIGESTS_KEY)?,
        )?;
        let required_evaluations = slug_set(
            ANDROID_HMS_EVALUATIONS_KEY,
            metadata_string_list(metadata, ANDROID_HMS_EVALUATIONS_KEY)?,
            HmsSafetyDetectEvaluation::from_slug,
        )?;
        let max_token_age_ms = metadata_u64(metadata, ANDROID_HMS_MAX_AGE_KEY)?;
        Ok(Self {
            app_id: trimmed_app_id.to_string(),
            package_names: packages,
            signing_digests_sha256: digests,
            required_evaluations,
            max_token_age_ms,
        })
    }
}

impl AndroidProvisionedMetadata {
    fn from_metadata(metadata: &Metadata) -> Result<Self, AndroidIntegrityMetadataError> {
        let inspector = metadata_string(metadata, ANDROID_PROVISIONED_INSPECTOR_KEY)?.ok_or(
            AndroidIntegrityMetadataError::Missing {
                key: ANDROID_PROVISIONED_INSPECTOR_KEY,
            },
        )?;
        let inspector_public_key = PublicKey::from_str(inspector.trim()).map_err(|err| {
            AndroidIntegrityMetadataError::Invalid {
                key: ANDROID_PROVISIONED_INSPECTOR_KEY,
                reason: format!("{err}"),
            }
        })?;
        let raw_manifest_schema =
            metadata_string(metadata, ANDROID_PROVISIONED_MANIFEST_SCHEMA_KEY)?.ok_or(
                AndroidIntegrityMetadataError::Missing {
                    key: ANDROID_PROVISIONED_MANIFEST_SCHEMA_KEY,
                },
            )?;
        let trimmed_manifest_schema = raw_manifest_schema.trim();
        if trimmed_manifest_schema.is_empty() {
            return Err(AndroidIntegrityMetadataError::Invalid {
                key: ANDROID_PROVISIONED_MANIFEST_SCHEMA_KEY,
                reason: "manifest schema must not be empty".into(),
            });
        }
        let manifest_version = metadata_u32(metadata, ANDROID_PROVISIONED_MANIFEST_VERSION_KEY)?;
        let max_manifest_age_ms = metadata_u64(metadata, ANDROID_PROVISIONED_MAX_AGE_KEY)?;
        let manifest_digest = metadata_string(metadata, ANDROID_PROVISIONED_MANIFEST_DIGEST_KEY)?
            .map(|value| {
                parse_hash_or_digest(value.trim(), ANDROID_PROVISIONED_MANIFEST_DIGEST_KEY)
            })
            .transpose()?;
        Ok(Self {
            inspector_public_key,
            manifest_schema: trimmed_manifest_schema.to_string(),
            manifest_version,
            max_manifest_age_ms,
            manifest_digest,
        })
    }
}

impl PlayIntegrityAppVerdict {
    fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "play_recognized" | "playrecognized" => Some(Self::PlayRecognized),
            "licensed" => Some(Self::Licensed),
            "unlicensed" => Some(Self::Unlicensed),
            _ => None,
        }
    }
}

impl PlayIntegrityDeviceVerdict {
    fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "strong" => Some(Self::Strong),
            "device" => Some(Self::Device),
            "basic" => Some(Self::Basic),
            "virtual" => Some(Self::Virtual),
            _ => None,
        }
    }
}

impl PlayIntegrityEnvironment {
    /// Canonical slug stored in metadata.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::Testing => "testing",
        }
    }

    fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "production" => Some(Self::Production),
            "testing" => Some(Self::Testing),
            _ => None,
        }
    }
}

impl HmsSafetyDetectEvaluation {
    fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "basic_integrity" | "basicintegrity" => Some(Self::BasicIntegrity),
            "system_integrity" | "systemintegrity" => Some(Self::SystemIntegrity),
            "strong_integrity" | "strongintegrity" => Some(Self::StrongIntegrity),
            _ => None,
        }
    }
}

fn detect_android_policy(
    metadata: &Metadata,
) -> Result<Option<AndroidIntegrityPolicy>, AndroidIntegrityMetadataError> {
    if let Some(slug) = metadata_string(metadata, AndroidIntegrityPolicy::METADATA_KEY)? {
        let trimmed = slug.trim();
        if trimmed.is_empty() {
            return Err(AndroidIntegrityMetadataError::Invalid {
                key: AndroidIntegrityPolicy::METADATA_KEY,
                reason: "android.integrity.policy must not be empty".into(),
            });
        }
        let normalized = trimmed.replace(['-', ' '], "_").to_ascii_lowercase();
        return AndroidIntegrityPolicy::from_str(&normalized)
            .map(Some)
            .map_err(|_| AndroidIntegrityMetadataError::UnsupportedPolicy { policy: slug });
    }
    if metadata_contains(metadata, ANDROID_PACKAGE_NAMES_KEY)
        || metadata_contains(metadata, ANDROID_SIGNATURE_DIGESTS_KEY)
    {
        return Ok(Some(AndroidIntegrityPolicy::MarkerKey));
    }
    Ok(None)
}

fn metadata_contains(metadata: &Metadata, key: &'static str) -> bool {
    Name::from_str(key)
        .map(|name| metadata.contains(&name))
        .unwrap_or(false)
}

fn metadata_string(
    metadata: &Metadata,
    key: &'static str,
) -> Result<Option<String>, AndroidIntegrityMetadataError> {
    let Some(value) = metadata_value(metadata, key)? else {
        return Ok(None);
    };
    value
        .try_into_any::<String>()
        .map(Some)
        .map_err(|err| AndroidIntegrityMetadataError::Invalid {
            key,
            reason: err.to_string(),
        })
}

fn metadata_bool(
    metadata: &Metadata,
    key: &'static str,
) -> Result<Option<bool>, AndroidIntegrityMetadataError> {
    let Some(value) = metadata_value(metadata, key)? else {
        return Ok(None);
    };
    value
        .try_into_any::<bool>()
        .map(Some)
        .map_err(|err| AndroidIntegrityMetadataError::Invalid {
            key,
            reason: err.to_string(),
        })
}

fn metadata_u64(
    metadata: &Metadata,
    key: &'static str,
) -> Result<Option<u64>, AndroidIntegrityMetadataError> {
    let Some(value) = metadata_value(metadata, key)? else {
        return Ok(None);
    };
    if let Ok(parsed) = value.try_into_any::<u64>() {
        return Ok(Some(parsed));
    }
    let string_value =
        value
            .try_into_any::<String>()
            .map_err(|err| AndroidIntegrityMetadataError::Invalid {
                key,
                reason: err.to_string(),
            })?;
    string_value
        .parse::<u64>()
        .map(Some)
        .map_err(|err| AndroidIntegrityMetadataError::Invalid {
            key,
            reason: err.to_string(),
        })
}

fn metadata_u32(
    metadata: &Metadata,
    key: &'static str,
) -> Result<Option<u32>, AndroidIntegrityMetadataError> {
    let Some(value) = metadata_u64(metadata, key)? else {
        return Ok(None);
    };
    u32::try_from(value)
        .map(Some)
        .map_err(|err| AndroidIntegrityMetadataError::Invalid {
            key,
            reason: err.to_string(),
        })
}

fn metadata_string_list(
    metadata: &Metadata,
    key: &'static str,
) -> Result<Vec<String>, AndroidIntegrityMetadataError> {
    let Some(value) = metadata_value(metadata, key)? else {
        return Ok(Vec::new());
    };
    value
        .try_into_any::<Vec<String>>()
        .map_err(|err| AndroidIntegrityMetadataError::Invalid {
            key,
            reason: err.to_string(),
        })
}

fn metadata_value<'a>(
    metadata: &'a Metadata,
    key: &'static str,
) -> Result<Option<&'a Json>, AndroidIntegrityMetadataError> {
    let name = Name::from_str(key).map_err(|err| AndroidIntegrityMetadataError::Invalid {
        key,
        reason: err.reason().to_string(),
    })?;
    Ok(metadata.get(&name))
}

fn package_set(
    key: &'static str,
    entries: Vec<String>,
) -> Result<BTreeSet<String>, AndroidIntegrityMetadataError> {
    if entries.is_empty() {
        return Err(AndroidIntegrityMetadataError::Empty { key });
    }
    let mut set = BTreeSet::new();
    for entry in entries {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            return Err(AndroidIntegrityMetadataError::Invalid {
                key,
                reason: "package name must not be empty".into(),
            });
        }
        set.insert(trimmed.to_ascii_lowercase());
    }
    Ok(set)
}

fn digest_set(
    key: &'static str,
    entries: Vec<String>,
) -> Result<BTreeSet<Vec<u8>>, AndroidIntegrityMetadataError> {
    if entries.is_empty() {
        return Err(AndroidIntegrityMetadataError::Empty { key });
    }
    let mut set = BTreeSet::new();
    for entry in entries {
        set.insert(decode_digest(entry.trim(), key)?);
    }
    Ok(set)
}

fn slug_set<T, F>(
    key: &'static str,
    entries: Vec<String>,
    mut parser: F,
) -> Result<BTreeSet<T>, AndroidIntegrityMetadataError>
where
    T: Ord,
    F: FnMut(&str) -> Option<T>,
{
    if entries.is_empty() {
        return Err(AndroidIntegrityMetadataError::Empty { key });
    }
    let mut set = BTreeSet::new();
    for entry in entries {
        let normalized = entry.trim().to_ascii_lowercase();
        let Some(value) = parser(&normalized) else {
            return Err(AndroidIntegrityMetadataError::Invalid {
                key,
                reason: format!("unrecognised slug `{entry}`"),
            });
        };
        set.insert(value);
    }
    Ok(set)
}

fn decode_digest(input: &str, key: &'static str) -> Result<Vec<u8>, AndroidIntegrityMetadataError> {
    let sanitized: String = input
        .chars()
        .filter(|c| !c.is_ascii_whitespace() && *c != ':')
        .collect();
    if !sanitized.is_empty()
        && sanitized.chars().all(|c| c.is_ascii_hexdigit())
        && sanitized.len().is_multiple_of(2)
    {
        return hex::decode(&sanitized).map_err(|err| AndroidIntegrityMetadataError::Invalid {
            key,
            reason: format!("invalid hex digest `{input}`: {err}"),
        });
    }
    BASE64_STANDARD
        .decode(input.as_bytes())
        .map_err(|err| AndroidIntegrityMetadataError::Invalid {
            key,
            reason: format!("invalid digest `{input}`: {err}"),
        })
}

fn parse_hash_or_digest(
    value: &str,
    key: &'static str,
) -> Result<Hash, AndroidIntegrityMetadataError> {
    Hash::from_str(value).map_or_else(|_| decode_digest(value, key).map(Hash::new), Ok)
}

#[model]
mod model {
    use core::fmt;

    use super::*;

    /// Operator-issued binding commitment to an offline allowance.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineAllowanceCommitment {
        /// Asset identifier bound to the allowance.
        pub asset: AssetId,
        /// Allowance amount bound to the commitment.
        pub amount: Numeric,
        /// Commitment bytes emitted by the issuer (e.g., Pedersen commitment).
        pub commitment: Vec<u8>,
    }

    impl OfflineAllowanceCommitment {
        /// Construct a new allowance commitment from raw components.
        #[must_use]
        pub fn new(asset: AssetId, amount: Numeric, commitment: Vec<u8>) -> Self {
            Self {
                asset,
                amount,
                commitment,
            }
        }
    }

    /// Policy flags enforced by issuers for a given offline wallet.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineWalletPolicy {
        /// Maximum offline balance permitted under this certificate.
        pub max_balance: Numeric,
        /// Maximum value of a single offline spend.
        pub max_tx_value: Numeric,
        /// Expiry timestamp (unix ms) for the allowance.
        pub expires_at_ms: u64,
    }

    impl OfflineWalletPolicy {
        /// Construct a new offline wallet policy.
        #[must_use]
        pub fn new(max_balance: Numeric, max_tx_value: Numeric, expires_at_ms: u64) -> Self {
            Self {
                max_balance,
                max_tx_value,
                expires_at_ms,
            }
        }
    }

    /// Operator-signed certificate binding a spend key + device attestation to an allowance.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineWalletCertificate {
        /// Account that owns the allowance.
        pub controller: AccountId,
        /// Operator account that signed this certificate.
        pub operator: AccountId,
        /// Commitment to the allowance this certificate governs.
        pub allowance: OfflineAllowanceCommitment,
        /// Spend public key baked into the wallet.
        pub spend_public_key: PublicKey,
        /// Serialized device attestation report (App Attest, `KeyMint`, etc.).
        pub attestation_report: Vec<u8>,
        /// Issuance timestamp (unix ms).
        pub issued_at_ms: u64,
        /// Expiry timestamp (unix ms).
        pub expires_at_ms: u64,
        /// Policy knobs enforced for this wallet.
        pub policy: OfflineWalletPolicy,
        /// Operator signature over the certificate payload.
        pub operator_signature: Signature,
        /// Additional metadata supplied by the issuer.
        #[norito(default)]
        pub metadata: Metadata,
        /// Optional unique identifier for the attestation verdict cached on-device.
        #[norito(default)]
        pub verdict_id: Option<Hash>,
        /// Optional nonce supplied to the attestation provider during registration.
        #[norito(default)]
        pub attestation_nonce: Option<Hash>,
        /// Optional timestamp (unix ms) when the attestation/verdict must be refreshed.
        #[norito(default)]
        pub refresh_at_ms: Option<u64>,
    }

    impl OfflineWalletCertificate {
        /// Construct a certificate binding a spend key, allowance, and attestation.
        #[allow(clippy::too_many_arguments)]
        #[must_use]
        pub fn new(
            controller: AccountId,
            operator: AccountId,
            allowance: OfflineAllowanceCommitment,
            spend_public_key: PublicKey,
            attestation_report: Vec<u8>,
            issued_at_ms: u64,
            expires_at_ms: u64,
            policy: OfflineWalletPolicy,
            operator_signature: Signature,
            metadata: Metadata,
            verdict_id: Option<Hash>,
            attestation_nonce: Option<Hash>,
            refresh_at_ms: Option<u64>,
        ) -> Self {
            Self {
                controller,
                operator,
                allowance,
                spend_public_key,
                attestation_report,
                issued_at_ms,
                expires_at_ms,
                policy,
                operator_signature,
                metadata,
                verdict_id,
                attestation_nonce,
                refresh_at_ms,
            }
        }
    }

    /// iOS App Attest evidence payload.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct AppleAppAttestProof {
        /// Stable identifier for the App Attest key.
        ///
        /// Must use canonical standard base64 encoding.
        pub key_id: String,
        /// Monotonic hardware counter.
        pub counter: u64,
        /// Raw App Attest assertion bytes.
        pub assertion: Vec<u8>,
        /// Challenge hash bound to invoice + receiver context.
        pub challenge_hash: Hash,
    }

    impl AppleAppAttestProof {
        /// Construct an App Attest proof payload.
        #[must_use]
        pub fn new(key_id: String, counter: u64, assertion: Vec<u8>, challenge_hash: Hash) -> Self {
            Self {
                key_id,
                counter,
                assertion,
                challenge_hash,
            }
        }
    }

    /// Android marker-key alias evidence payload.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct AndroidMarkerKeyProof {
        /// Series identifier for this allowance epoch.
        pub series: String,
        /// Monotonic counter encoded in the alias sequence.
        pub counter: u64,
        /// Hardware-backed marker public key (SEC1-encoded P-256).
        pub marker_public_key: Vec<u8>,
        /// Optional signature produced by the marker key over the receipt challenge hash.
        ///
        /// When present, this must be a raw 64-byte `r||s` signature.
        pub marker_signature: Option<Vec<u8>>,
        /// Hardware attestation statement proving key provenance.
        pub attestation: Vec<u8>,
    }

    /// Provisioned inspector manifest signed by an operator-controlled key.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct AndroidProvisionedProof {
        /// Schema label for the manifest (must match `android.provisioned.manifest_schema`).
        pub manifest_schema: String,
        /// Version of the manifest schema (if any).
        #[norito(default)]
        pub manifest_version: Option<u32>,
        /// Unix timestamp (ms) when the manifest was inspected/signed.
        pub manifest_issued_at_ms: u64,
        /// Canonical challenge hash derived from the receipt payload.
        pub challenge_hash: Hash,
        /// Inspector-issued monotonic counter for the device scope.
        pub counter: u64,
        /// Structured manifest describing the inspected device.
        pub device_manifest: Metadata,
        /// Signature produced by the inspector key over the manifest payload.
        pub inspector_signature: Signature,
    }

    impl AndroidProvisionedProof {
        /// Canonical bytes signed by the inspector.
        ///
        /// # Errors
        ///
        /// Returns [`norito::Error`] when serialization fails.
        pub fn signing_bytes(&self) -> Result<Vec<u8>, norito::Error> {
            let payload = AndroidProvisionedManifestSignaturePayload::from(self);
            to_bytes(&payload)
        }

        /// Deterministic digest of the manifest payload.
        ///
        /// # Errors
        ///
        /// Returns [`norito::Error`] when serialization fails.
        pub fn manifest_digest(&self) -> Result<Hash, norito::Error> {
            let bytes = to_bytes(&self.device_manifest)?;
            Ok(Hash::new(bytes))
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    struct AndroidProvisionedManifestSignaturePayload {
        manifest_schema: String,
        #[norito(default)]
        manifest_version: Option<u32>,
        manifest_issued_at_ms: u64,
        challenge_hash: Hash,
        counter: u64,
        device_manifest: Metadata,
    }

    impl From<&AndroidProvisionedProof> for AndroidProvisionedManifestSignaturePayload {
        fn from(proof: &AndroidProvisionedProof) -> Self {
            Self {
                manifest_schema: proof.manifest_schema.clone(),
                manifest_version: proof.manifest_version,
                manifest_issued_at_ms: proof.manifest_issued_at_ms,
                challenge_hash: proof.challenge_hash,
                counter: proof.counter,
                device_manifest: proof.device_manifest.clone(),
            }
        }
    }

    /// Metadata requirements for Android marker-key / `KeyMint` attestations.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct AndroidMarkerKeyMetadata {
        /// Package names that must appear in the `KeyMint` attestation.
        pub package_names: BTreeSet<String>,
        /// Signing certificate digests (SHA-256) accepted for the application.
        pub signing_digests_sha256: BTreeSet<Vec<u8>>,
        /// Whether the allowance demands StrongBox-backed keys and marker signatures.
        pub require_strongbox: bool,
        /// Whether `KeyMint` rollback resistance must be asserted.
        pub require_rollback_resistance: bool,
    }

    /// Play Integrity application verdict classes.
    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Decode, Encode, IntoSchema,
    )]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(tag = "app_verdict", content = "value")]
    pub enum PlayIntegrityAppVerdict {
        /// Application is Play-distributed and recognized.
        #[default]
        PlayRecognized,
        /// Application matches a licensed package/signer combination.
        Licensed,
        /// Application is unlicensed but not explicitly revoked.
        Unlicensed,
    }

    /// Play Integrity device verdict classes.
    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Decode, Encode, IntoSchema,
    )]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(tag = "device_verdict", content = "value")]
    pub enum PlayIntegrityDeviceVerdict {
        /// Device satisfies strong integrity (hardware-backed with Trusted OS).
        #[default]
        Strong,
        /// Device satisfies the standard device integrity class (CTS profile).
        Device,
        /// Device only satisfies basic integrity (no root detection triggers).
        Basic,
        /// Device is virtualized (emulator) but still allowed explicitly.
        Virtual,
    }

    /// Play Integrity environment for attestation issuance.
    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Decode, Encode, IntoSchema,
    )]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(tag = "environment", content = "value")]
    pub enum PlayIntegrityEnvironment {
        /// Production Google Play Integrity backend.
        #[default]
        Production,
        /// Test/QA Play Integrity backend.
        Testing,
    }

    /// Metadata requirements for Play Integrity verdict validation.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct AndroidPlayIntegrityMetadata {
        /// Google Cloud project that issues the tokens.
        pub cloud_project_number: u64,
        /// Environment (production vs. testing) expected for tokens.
        pub environment: PlayIntegrityEnvironment,
        /// Package whitelist enforced for the allowance.
        pub package_names: BTreeSet<String>,
        /// Signing certificate digests accepted for issued tokens.
        pub signing_digests_sha256: BTreeSet<Vec<u8>>,
        /// App-level verdicts accepted for the allowance.
        pub allowed_app_verdicts: BTreeSet<PlayIntegrityAppVerdict>,
        /// Device-level verdicts accepted for the allowance.
        pub allowed_device_verdicts: BTreeSet<PlayIntegrityDeviceVerdict>,
        /// Maximum token age (ms) that operators accept before forcing refresh.
        pub max_token_age_ms: Option<u64>,
    }

    /// Huawei/HarmonyOS Safety Detect evaluation classes.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(tag = "hms_evaluation", content = "value")]
    pub enum HmsSafetyDetectEvaluation {
        /// Device passes basic integrity checks.
        BasicIntegrity,
        /// Device passes system integrity (CTS profile) checks.
        SystemIntegrity,
        /// Device satisfies the highest attestation level (hardware-backed).
        StrongIntegrity,
    }

    /// Metadata requirements for HMS Safety Detect attestations.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct AndroidHmsSafetyDetectMetadata {
        /// HMS application identifier (agconnect-services App ID).
        pub app_id: String,
        /// Package whitelist enforced for the allowance.
        pub package_names: BTreeSet<String>,
        /// Signing digests accepted for Safety Detect verdicts.
        pub signing_digests_sha256: BTreeSet<Vec<u8>>,
        /// Evaluation classes that must appear inside the verdict.
        pub required_evaluations: BTreeSet<HmsSafetyDetectEvaluation>,
        /// Optional TTL (ms) for issued verdicts.
        pub max_token_age_ms: Option<u64>,
    }

    /// Metadata requirements for operator-provisioned attestation paths.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct AndroidProvisionedMetadata {
        /// Inspector public key that signs device manifests.
        pub inspector_public_key: PublicKey,
        /// Schema label for device manifests (e.g., `offline_provisioning_v1`).
        pub manifest_schema: String,
        /// Optional manifest version enforced for the allowance.
        pub manifest_version: Option<u32>,
        /// Optional maximum age (ms) for the manifest+counters.
        pub max_manifest_age_ms: Option<u64>,
        /// Optional digest for the manifest template operators expect.
        pub manifest_digest: Option<Hash>,
    }

    /// Provider-specific knobs declared via `android.integrity.policy`.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(tag = "provider", content = "metadata")]
    pub enum AndroidIntegrityMetadata {
        /// Android `KeyMint` / marker-key configuration.
        MarkerKey(AndroidMarkerKeyMetadata),
        /// Google Play Integrity configuration.
        PlayIntegrity(AndroidPlayIntegrityMetadata),
        /// Huawei Safety Detect configuration.
        HmsSafetyDetect(AndroidHmsSafetyDetectMetadata),
        /// Operator-provisioned allowance configuration.
        Provisioned(AndroidProvisionedMetadata),
    }

    impl AndroidIntegrityMetadata {
        /// Returns the policy associated with this metadata payload.
        #[must_use]
        pub const fn policy(&self) -> AndroidIntegrityPolicy {
            match self {
                Self::MarkerKey(_) => AndroidIntegrityPolicy::MarkerKey,
                Self::PlayIntegrity(_) => AndroidIntegrityPolicy::PlayIntegrity,
                Self::HmsSafetyDetect(_) => AndroidIntegrityPolicy::HmsSafetyDetect,
                Self::Provisioned(_) => AndroidIntegrityPolicy::Provisioned,
            }
        }

        /// Canonical slug associated with this metadata payload.
        #[must_use]
        pub const fn policy_slug(&self) -> &'static str {
            self.policy().as_str()
        }
    }

    /// Declares which attestation provider a given Android allowance must use.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(tag = "policy", content = "value")]
    pub enum AndroidIntegrityPolicy {
        /// Default policy that relies on a hardware marker key + `KeyMint` attestation.
        MarkerKey,
        /// Google Play Integrity verdicts (expressed via Play Services / Google servers).
        PlayIntegrity,
        /// Huawei/HarmonyOS Safety Detect or Device Attestation verdicts.
        HmsSafetyDetect,
        /// Norito-provisioned attestation (operator-run diagnostics/provisioning).
        Provisioned,
    }

    impl AndroidIntegrityPolicy {
        /// Metadata key used to store the policy inside `OfflineWalletCertificate.metadata`.
        pub const METADATA_KEY: &'static str = "android.integrity.policy";

        /// Returns the canonical slug representation stored in metadata/config.
        #[must_use]
        pub const fn as_str(self) -> &'static str {
            match self {
                Self::MarkerKey => "marker_key",
                Self::PlayIntegrity => "play_integrity",
                Self::HmsSafetyDetect => "hms_safety_detect",
                Self::Provisioned => "provisioned",
            }
        }
    }

    impl fmt::Display for AndroidIntegrityPolicy {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(self.as_str())
        }
    }

    /// Error returned when parsing an Android integrity policy slug fails.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ParseAndroidIntegrityPolicyError;

    impl fmt::Display for ParseAndroidIntegrityPolicyError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("unrecognised android integrity policy")
        }
    }

    impl std::error::Error for ParseAndroidIntegrityPolicyError {}

    impl FromStr for AndroidIntegrityPolicy {
        type Err = ParseAndroidIntegrityPolicyError;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s {
                "marker_key" => Ok(Self::MarkerKey),
                "play_integrity" => Ok(Self::PlayIntegrity),
                "hms_safety_detect" => Ok(Self::HmsSafetyDetect),
                "provisioned" => Ok(Self::Provisioned),
                _ => Err(ParseAndroidIntegrityPolicyError),
            }
        }
    }

    /// Platform evidence proving that a spend advanced a hardware monotonic counter.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize),
        norito(tag = "platform", content = "proof")
    )]
    pub enum OfflinePlatformProof {
        /// iOS App Attest hardware counter evidence.
        AppleAppAttest(AppleAppAttestProof),
        /// Android marker-key alias counter evidence.
        AndroidMarkerKey(AndroidMarkerKeyProof),
        /// Operator-provisioned inspector manifest evidence.
        Provisioned(AndroidProvisionedProof),
    }

    /// Platform label encoded in offline build claims.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(tag = "platform", content = "value")]
    pub enum OfflineBuildClaimPlatform {
        /// iOS builds distributed for App Attest flows.
        Apple,
        /// Android builds distributed for marker/play/hms/provisioned flows.
        Android,
    }

    /// Operator-signed build attestation bound to one receipt nonce.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineBuildClaim {
        /// Deterministic claim identifier used for one-time replay protection.
        pub claim_id: Hash,
        /// Platform this build claim is valid for.
        pub platform: OfflineBuildClaimPlatform,
        /// Application identifier (`bundle_id` on iOS, package name on Android).
        pub app_id: String,
        /// Build number certified by the operator.
        pub build_number: u64,
        /// Claim issuance timestamp (unix ms).
        pub issued_at_ms: u64,
        /// Claim expiry timestamp (unix ms).
        pub expires_at_ms: u64,
        /// Lineage scope this claim is pinned to.
        pub lineage_scope: String,
        /// Receipt nonce this claim is bound to (currently the receipt `tx_id`).
        pub nonce: Hash,
        /// Operator signature over the canonical claim payload.
        pub operator_signature: Signature,
    }

    impl OfflineBuildClaim {
        /// Canonical bytes signed by the operator.
        ///
        /// # Errors
        ///
        /// Returns [`norito::Error`] when serialization fails.
        pub fn signing_bytes(&self) -> Result<Vec<u8>, norito::Error> {
            let payload = OfflineBuildClaimPayload::from(self);
            to_bytes(&payload)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    struct OfflineBuildClaimPayload {
        claim_id: Hash,
        platform: OfflineBuildClaimPlatform,
        app_id: String,
        build_number: u64,
        issued_at_ms: u64,
        expires_at_ms: u64,
        lineage_scope: String,
        nonce: Hash,
    }

    impl From<&OfflineBuildClaim> for OfflineBuildClaimPayload {
        fn from(claim: &OfflineBuildClaim) -> Self {
            Self {
                claim_id: claim.claim_id,
                platform: claim.platform,
                app_id: claim.app_id.clone(),
                build_number: claim.build_number,
                issued_at_ms: claim.issued_at_ms,
                expires_at_ms: claim.expires_at_ms,
                lineage_scope: claim.lineage_scope.clone(),
                nonce: claim.nonce,
            }
        }
    }

    /// Receiver-side evidence for a single offline spend.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineSpendReceipt {
        /// Deterministic identifier for the offline spend.
        pub tx_id: Hash,
        /// Sender offline account.
        pub from: AccountId,
        /// Receiver offline account (payer's target).
        pub to: AccountId,
        /// Asset identifier being transferred.
        pub asset: AssetId,
        /// Amount received.
        pub amount: Numeric,
        /// Unix timestamp (ms) when the receipt was issued.
        pub issued_at_ms: u64,
        /// Invoice identifier provided by the receiver.
        pub invoice_id: String,
        /// Platform-specific counter proof.
        pub platform_proof: OfflinePlatformProof,
        /// Optional platform attestation snapshot tied to this receipt.
        ///
        /// Wallets attach a Play Integrity or HMS Safety Detect token per receipt when
        /// individual spends require dedicated attestations (OA10 roadmap item).
        /// The ledger prefers this snapshot over the bundle-level snapshot when both
        /// are supplied.
        #[norito(default)]
        pub platform_snapshot: Option<OfflinePlatformTokenSnapshot>,
        /// Identifier of the sender's registered certificate.
        pub sender_certificate_id: Hash,
        /// Signature produced by the sender's spend key over the offline transaction payload.
        pub sender_signature: Signature,
        /// Optional operator-signed build claim tied to this spend.
        #[norito(default)]
        pub build_claim: Option<OfflineBuildClaim>,
    }

    impl OfflineSpendReceipt {
        /// Construct a spend receipt tying platform evidence to sender metadata.
        #[allow(clippy::too_many_arguments)]
        #[must_use]
        pub fn new(
            tx_id: Hash,
            from: AccountId,
            to: AccountId,
            asset: AssetId,
            amount: Numeric,
            issued_at_ms: u64,
            invoice_id: String,
            platform_proof: OfflinePlatformProof,
            platform_snapshot: Option<OfflinePlatformTokenSnapshot>,
            sender_certificate_id: Hash,
            sender_signature: Signature,
        ) -> Self {
            Self {
                tx_id,
                from,
                to,
                asset,
                amount,
                issued_at_ms,
                invoice_id,
                platform_proof,
                platform_snapshot,
                sender_certificate_id,
                sender_signature,
                build_claim: None,
            }
        }
    }

    /// Commitment delta claimed during an offline-to-online deposit.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineBalanceProof {
        /// Issued allowance commitment.
        pub initial_commitment: OfflineAllowanceCommitment,
        /// Commitment after applying all offline spends included in the bundle.
        pub resulting_commitment: Vec<u8>,
        /// Claimed delta \(\Delta\) that is being deposited online.
        pub claimed_delta: Numeric,
        /// Versioned zero-knowledge proof blob (delta + range proofs).
        ///
        /// The payload is required for ledger settlement and must use the v1 layout:
        /// `version (1 byte) || delta_proof (96 bytes) || range_proof (64 * 192 bytes)`.
        #[norito(default)]
        pub zk_proof: Option<Vec<u8>>,
    }

    impl OfflineBalanceProof {
        /// Construct a balance proof describing the allowance delta.
        #[must_use]
        pub fn new(
            initial_commitment: OfflineAllowanceCommitment,
            resulting_commitment: Vec<u8>,
            claimed_delta: Numeric,
            zk_proof: Option<Vec<u8>>,
        ) -> Self {
            Self {
                initial_commitment,
                resulting_commitment,
                claimed_delta,
                zk_proof,
            }
        }
    }

    /// Per-certificate commitment delta proof used by multi-allowance bundles.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineCertificateBalanceProof {
        /// Certificate backing the grouped receipt set.
        pub sender_certificate_id: Hash,
        /// Commitment delta proof for this certificate group.
        pub balance_proof: OfflineBalanceProof,
    }

    impl OfflineCertificateBalanceProof {
        /// Construct a per-certificate balance proof entry.
        #[must_use]
        pub fn new(sender_certificate_id: Hash, balance_proof: OfflineBalanceProof) -> Self {
            Self {
                sender_certificate_id,
                balance_proof,
            }
        }
    }

    /// FASTPQ HKDF domain used for Poseidon witness blindings.
    pub const OFFLINE_FASTPQ_HKDF_DOMAIN: &[u8; 23] = b"iroha.offline.fastpq.v1";

    /// Version of the offline FASTPQ witness request schema.
    pub const OFFLINE_PROOF_REQUEST_VERSION_V1: u16 = 1;

    /// Proof request types supported by the FASTPQ circuits.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    pub enum OfflineProofRequestKind {
        /// Sum circuit tying receipts to the balance delta.
        Sum,
        /// Counter-contiguity circuit protecting the allowance checkpoint.
        Counter,
        /// Replay-log circuit covering the receiver hash chain.
        Replay,
    }

    impl OfflineProofRequestKind {
        /// Canonical lowercase string representation.
        #[must_use]
        pub const fn as_str(self) -> &'static str {
            match self {
                Self::Sum => "sum",
                Self::Counter => "counter",
                Self::Replay => "replay",
            }
        }
    }

    impl fmt::Display for OfflineProofRequestKind {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(self.as_str())
        }
    }

    /// Error returned when parsing a proof request kind fails.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ParseOfflineProofRequestKindError;

    impl fmt::Display for ParseOfflineProofRequestKindError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("unrecognised offline proof request kind")
        }
    }

    impl std::error::Error for ParseOfflineProofRequestKindError {}

    impl FromStr for OfflineProofRequestKind {
        type Err = ParseOfflineProofRequestKindError;

        fn from_str(value: &str) -> Result<Self, Self::Err> {
            match value.to_ascii_lowercase().as_str() {
                "sum" => Ok(Self::Sum),
                "counter" => Ok(Self::Counter),
                "replay" => Ok(Self::Replay),
                _ => Err(ParseOfflineProofRequestKindError),
            }
        }
    }

    /// Common header shared by all FASTPQ witness request payloads.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineProofRequestHeader {
        /// Version of the witness request schema.
        pub version: u16,
        /// Identifier for the deposit bundle being proven.
        pub bundle_id: Hash,
        /// Certificate that authorised the offline allowance.
        pub certificate_id: Hash,
        /// Poseidon root covering the receipts included in this bundle.
        pub receipts_root: PoseidonDigest,
    }

    /// Deterministic HKDF salt tied to a receipt counter.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineProofBlindingSeed {
        /// Hardware counter value associated with the receipt.
        pub counter: u64,
        /// HKDF salt derived from `certificate_id‖counter`.
        pub hkdf_salt: Hash,
    }

    /// Witness request for the FASTPQ sum circuit.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineProofRequestSum {
        /// Shared request header.
        pub header: OfflineProofRequestHeader,
        /// Issued allowance commitment.
        pub initial_commitment: OfflineAllowanceCommitment,
        /// Commitment after applying the receipts reported in the bundle.
        pub resulting_commitment: Vec<u8>,
        /// Claimed delta deposited on-ledger.
        pub claimed_delta: Numeric,
        /// Amount for every receipt included in the bundle (ordered by `(counter, tx_id)`).
        pub receipt_amounts: Vec<Numeric>,
        /// Deterministic HKDF salts for each receipt/sponge blinding.
        pub blinding_seeds: Vec<OfflineProofBlindingSeed>,
    }

    /// Witness request for the FASTPQ counter-contiguity circuit.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineProofRequestCounter {
        /// Shared request header.
        pub header: OfflineProofRequestHeader,
        /// Last counter stored on-ledger when the allowance was registered/refreshed.
        pub counter_checkpoint: u64,
        /// Ordered counter values supplied by the receipts.
        pub counters: Vec<u64>,
    }

    /// Witness request for the FASTPQ replay-log circuit.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineProofRequestReplay {
        /// Shared request header.
        pub header: OfflineProofRequestHeader,
        /// Receiver log head before processing the bundle.
        pub replay_log_head: Hash,
        /// Receiver log tail after processing the bundle.
        pub replay_log_tail: Hash,
        /// Ordered transaction identifiers included in the bundle.
        pub tx_ids: Vec<Hash>,
    }

    /// Ledger submission that moves value from an offline allowance back on-ledger.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineToOnlineTransfer {
        /// Unique identifier for this deposit bundle.
        pub bundle_id: Hash,
        /// Receiver account that will be credited online.
        pub receiver: AccountId,
        /// Online account that ultimately receives on-ledger funds.
        pub deposit_account: AccountId,
        /// Receipts for each offline spend being consolidated.
        pub receipts: Vec<OfflineSpendReceipt>,
        /// Commitment delta justifying the deposited amount.
        pub balance_proof: OfflineBalanceProof,
        /// Optional per-certificate balance proofs for multi-allowance bundles.
        #[norito(default)]
        pub balance_proofs: Option<Vec<OfflineCertificateBalanceProof>>,
        /// Optional aggregate proof bundle covering receipt sums/counters/replay logs.
        #[norito(default)]
        pub aggregate_proof: Option<AggregateProofEnvelope>,
        /// Optional attachments (e.g., FASTPQ transcripts or regulator receipts).
        #[norito(default)]
        pub attachments: Option<ProofAttachmentList>,
        /// Optional platform attestation snapshot supplied by the submitting wallet.
        #[norito(default)]
        pub platform_snapshot: Option<OfflinePlatformTokenSnapshot>,
    }

    impl OfflineToOnlineTransfer {
        /// Construct an offline-to-online transfer bundle.
        #[allow(clippy::too_many_arguments)]
        #[must_use]
        pub fn new(
            bundle_id: Hash,
            receiver: AccountId,
            deposit_account: AccountId,
            receipts: Vec<OfflineSpendReceipt>,
            balance_proof: OfflineBalanceProof,
            aggregate_proof: Option<AggregateProofEnvelope>,
            attachments: Option<ProofAttachmentList>,
            platform_snapshot: Option<OfflinePlatformTokenSnapshot>,
        ) -> Self {
            Self {
                bundle_id,
                receiver,
                deposit_account,
                receipts,
                balance_proof,
                balance_proofs: None,
                aggregate_proof,
                attachments,
                platform_snapshot,
            }
        }

        /// Construct an offline-to-online transfer bundle with explicit per-certificate proofs.
        #[allow(clippy::too_many_arguments)]
        #[must_use]
        pub fn new_with_balance_proofs(
            bundle_id: Hash,
            receiver: AccountId,
            deposit_account: AccountId,
            receipts: Vec<OfflineSpendReceipt>,
            balance_proof: OfflineBalanceProof,
            balance_proofs: Vec<OfflineCertificateBalanceProof>,
            aggregate_proof: Option<AggregateProofEnvelope>,
            attachments: Option<ProofAttachmentList>,
            platform_snapshot: Option<OfflinePlatformTokenSnapshot>,
        ) -> Self {
            Self {
                bundle_id,
                receiver,
                deposit_account,
                receipts,
                balance_proof,
                balance_proofs: Some(balance_proofs),
                aggregate_proof,
                attachments,
                platform_snapshot,
            }
        }
    }

    /// Ledger-managed lifecycle status for an offline-to-online bundle.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(tag = "status", content = "state")]
    pub enum OfflineTransferStatus {
        /// Bundle has been fully verified and applied against the allowance but is still within the hot-retention window.
        Settled,
        /// Bundle was accepted for processing but rejected during settlement validation.
        Rejected,
        /// Bundle has satisfied the configured retention policy and may be moved to cold storage.
        Archived,
    }

    /// Lifecycle entry describing a status transition for a bundle.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineTransferLifecycleEntry {
        /// Status that became active during this transition.
        pub status: OfflineTransferStatus,
        /// Unix timestamp (ms) when the transition occurred.
        pub transitioned_at_ms: u64,
        /// Snapshot of the certificate verdict metadata captured at this transition.
        #[norito(default)]
        pub verdict_snapshot: Option<OfflineVerdictSnapshot>,
    }

    /// Snapshot of verdict/expiry metadata captured when a bundle settles.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineVerdictSnapshot {
        /// Deterministic certificate identifier backing the bundle.
        pub certificate_id: Hash,
        /// Cached attestation verdict identifier (if available).
        #[norito(default)]
        pub verdict_id: Option<Hash>,
        /// Cached attestation nonce (if available).
        #[norito(default)]
        pub attestation_nonce: Option<Hash>,
        /// Timestamp (unix ms) indicating when the attestation must be refreshed.
        #[norito(default)]
        pub refresh_at_ms: Option<u64>,
        /// Certificate expiry timestamp (unix ms).
        pub certificate_expires_at_ms: u64,
        /// Policy expiry timestamp (unix ms).
        pub policy_expires_at_ms: u64,
    }

    impl OfflineVerdictSnapshot {
        /// Capture the snapshot from a wallet certificate.
        #[must_use]
        pub fn from_certificate(certificate: &OfflineWalletCertificate) -> Self {
            Self {
                certificate_id: certificate.certificate_id(),
                verdict_id: certificate.verdict_id,
                attestation_nonce: certificate.attestation_nonce,
                refresh_at_ms: certificate.refresh_at_ms,
                certificate_expires_at_ms: certificate.expires_at_ms,
                policy_expires_at_ms: certificate.policy.expires_at_ms,
            }
        }
    }

    /// Snapshot of the platform attestation token captured when the bundle settled.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflinePlatformTokenSnapshot {
        /// Integrity policy slug describing the attestation provider.
        pub policy: String,
        /// Base64-encoded JWS payload returned by the provider.
        pub attestation_jws_b64: String,
    }

    impl OfflinePlatformTokenSnapshot {
        /// Canonical policy label associated with the snapshot.
        #[must_use]
        pub fn policy_label(&self) -> &str {
            self.policy.as_str()
        }

        /// Returns the policy enum when the slug is recognised.
        #[must_use]
        pub fn policy(&self) -> Option<AndroidIntegrityPolicy> {
            AndroidIntegrityPolicy::from_str(self.policy.as_str()).ok()
        }

        /// Accessor returning the encoded attestation payload.
        #[must_use]
        pub fn attestation_jws_b64(&self) -> &str {
            self.attestation_jws_b64.as_str()
        }
    }

    impl OfflineTransferStatus {
        /// Canonical label used in filters/JSON.
        #[must_use]
        pub const fn as_label(self) -> &'static str {
            match self {
                Self::Settled => "settled",
                Self::Rejected => "rejected",
                Self::Archived => "archived",
            }
        }
    }

    impl FromStr for OfflineTransferStatus {
        type Err = ();

        fn from_str(value: &str) -> Result<Self, Self::Err> {
            match value {
                "settled" => Ok(Self::Settled),
                "rejected" => Ok(Self::Rejected),
                "archived" => Ok(Self::Archived),
                _ => Err(()),
            }
        }
    }

    impl fmt::Display for OfflineTransferStatus {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(self.as_label())
        }
    }

    /// Platform classification recorded for offline transfer rejection telemetry.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(tag = "platform", content = "value")]
    pub enum OfflineTransferRejectionPlatform {
        /// General validation failure outside a specific platform boundary.
        General,
        /// iOS App Attest rejection path.
        Apple,
        /// Android `KeyMint`/marker-key rejection path.
        Android,
    }

    impl OfflineTransferRejectionPlatform {
        /// Metric/JSON label used for this platform.
        #[must_use]
        pub const fn as_label(self) -> &'static str {
            match self {
                Self::General => "general",
                Self::Apple => "apple",
                Self::Android => "android",
            }
        }
    }

    impl FromStr for OfflineTransferRejectionPlatform {
        type Err = ();

        fn from_str(value: &str) -> Result<Self, Self::Err> {
            match value {
                "general" => Ok(Self::General),
                "apple" => Ok(Self::Apple),
                "android" => Ok(Self::Android),
                _ => Err(()),
            }
        }
    }

    impl fmt::Display for OfflineTransferRejectionPlatform {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(self.as_label())
        }
    }

    /// Classification for offline bundle validation failures surfaced via telemetry/admin APIs.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(tag = "reason", content = "value")]
    pub enum OfflineTransferRejectionReason {
        /// The submitting party does not match the bundle receiver.
        UnauthorizedReceiver,
        /// Bundle contained zero receipts.
        EmptyBundle,
        /// Receipts referenced multiple asset definitions.
        NonUniformAsset,
        /// A receipt amount was zero or negative.
        InvalidReceiptAmount,
        /// A receipt amount exceeded the policy max transaction value.
        MaxTxValueExceeded,
        /// Claimed delta mismatched the sum of receipt amounts.
        DeltaMismatch,
        /// Receipts referenced multiple certificates.
        MixedCertificates,
        /// Receipts are not in canonical `(counter, tx_id)` order.
        ReceiptOrderInvalid,
        /// Receipts referenced multiple counter scopes.
        MixedCounterScopes,
        /// Referenced allowance certificate was not registered on-ledger.
        AllowanceNotRegistered,
        /// Certificate validity window elapsed.
        CertificateExpired,
        /// Allowance policy validity window elapsed.
        PolicyExpired,
        /// Receipt `to`/receiver mismatch.
        ReceiptReceiverMismatch,
        /// Receipt controller mismatch.
        ReceiptSenderMismatch,
        /// Receipt asset does not match the registered allowance asset.
        ReceiptAssetMismatch,
        /// Receipt timestamp is missing or violates allowed windows.
        ReceiptTimestampInvalid,
        /// Receipt exceeded the maximum allowed age.
        ReceiptExpired,
        /// Balance proof asset mismatch.
        BalanceAssetMismatch,
        /// Balance proof commitment mismatch.
        CommitmentMismatch,
        /// Balance proof is missing or invalid.
        BalanceProofInvalid,
        /// Platform hardware counter progression is invalid.
        CounterViolation,
        /// Spend-key signature failed verification.
        ReceiptSignatureInvalid,
        /// Platform challenge derivation mismatch.
        PlatformChallengeMismatch,
        /// Required attestation artifact missing from metadata or receipt.
        PlatformAttestationMissing,
        /// Platform metadata malformed or incomplete.
        PlatformMetadataInvalid,
        /// Platform attestation or statement failed verification.
        PlatformAttestationInvalid,
        /// Platform-specific signature missing.
        PlatformSignatureMissing,
        /// Platform-specific signature invalid.
        PlatformSignatureInvalid,
        /// Aggregate proof missing when required.
        AggregateProofMissing,
        /// Aggregate proof version is unsupported by this node.
        AggregateProofVersionUnsupported,
        /// Aggregate proof receipts root mismatched the actual receipts.
        AggregateProofRootMismatch,
        /// Failed to hash receipts into a Poseidon root.
        AggregateProofHashError,
        /// Allowance no longer has sufficient remaining value.
        AllowanceDepleted,
        /// Bundle identifier duplicates an existing record.
        DuplicateBundle,
        /// Bundle contains duplicate invoice identifiers.
        InvoiceDuplicate,
        /// Cached attestation verdict expired before bundle submission.
        VerdictExpired,
        /// Certificate lineage metadata failed strict newest-first checks.
        LineageInvalid,
        /// Required receipt build claim was not supplied.
        BuildClaimMissing,
        /// Build claim payload/signature did not verify.
        BuildClaimInvalid,
        /// Build claim timestamp window is not valid.
        BuildClaimExpired,
        /// Build claim id was already consumed by a previously settled receipt.
        BuildClaimReplayed,
        /// Build claim build number is lower than the certificate minimum.
        BuildClaimBuildTooLow,
    }

    impl OfflineTransferRejectionReason {
        /// Metric/JSON label for the rejection reason.
        #[must_use]
        pub const fn as_label(self) -> &'static str {
            match self {
                Self::UnauthorizedReceiver => "unauthorized_receiver",
                Self::EmptyBundle => "empty_bundle",
                Self::NonUniformAsset => "non_uniform_asset",
                Self::InvalidReceiptAmount => "invalid_receipt_amount",
                Self::MaxTxValueExceeded => "max_tx_value_exceeded",
                Self::DeltaMismatch => "delta_mismatch",
                Self::MixedCertificates => "mixed_certificates",
                Self::ReceiptOrderInvalid => "receipt_order_invalid",
                Self::MixedCounterScopes => "mixed_counter_scopes",
                Self::AllowanceNotRegistered => "allowance_not_registered",
                Self::CertificateExpired => "certificate_expired",
                Self::PolicyExpired => "policy_expired",
                Self::ReceiptReceiverMismatch => "receipt_receiver_mismatch",
                Self::ReceiptSenderMismatch => "receipt_sender_mismatch",
                Self::ReceiptAssetMismatch => "receipt_asset_mismatch",
                Self::ReceiptTimestampInvalid => "receipt_timestamp_invalid",
                Self::ReceiptExpired => "receipt_expired",
                Self::BalanceAssetMismatch => "balance_asset_mismatch",
                Self::CommitmentMismatch => "commitment_mismatch",
                Self::BalanceProofInvalid => "balance_proof_invalid",
                Self::CounterViolation => "counter_violation",
                Self::ReceiptSignatureInvalid => "receipt_signature_invalid",
                Self::PlatformChallengeMismatch => "platform_challenge_mismatch",
                Self::PlatformAttestationMissing => "platform_attestation_missing",
                Self::PlatformMetadataInvalid => "platform_metadata_invalid",
                Self::PlatformAttestationInvalid => "platform_attestation_invalid",
                Self::PlatformSignatureMissing => "platform_signature_missing",
                Self::PlatformSignatureInvalid => "platform_signature_invalid",
                Self::AggregateProofMissing => "aggregate_proof_missing",
                Self::AggregateProofVersionUnsupported => "aggregate_proof_version_unsupported",
                Self::AggregateProofRootMismatch => "aggregate_proof_root_mismatch",
                Self::AggregateProofHashError => "aggregate_proof_hash_error",
                Self::AllowanceDepleted => "allowance_depleted",
                Self::DuplicateBundle => "duplicate_bundle",
                Self::InvoiceDuplicate => "invoice_duplicate",
                Self::VerdictExpired => "verdict_expired",
                Self::LineageInvalid => "lineage_invalid",
                Self::BuildClaimMissing => "build_claim_missing",
                Self::BuildClaimInvalid => "build_claim_invalid",
                Self::BuildClaimExpired => "build_claim_expired",
                Self::BuildClaimReplayed => "build_claim_replayed",
                Self::BuildClaimBuildTooLow => "build_claim_build_too_low",
            }
        }
    }

    impl FromStr for OfflineTransferRejectionReason {
        type Err = ();

        fn from_str(value: &str) -> Result<Self, Self::Err> {
            match value {
                "unauthorized_receiver" => Ok(Self::UnauthorizedReceiver),
                "empty_bundle" => Ok(Self::EmptyBundle),
                "non_uniform_asset" => Ok(Self::NonUniformAsset),
                "invalid_receipt_amount" => Ok(Self::InvalidReceiptAmount),
                "max_tx_value_exceeded" => Ok(Self::MaxTxValueExceeded),
                "delta_mismatch" => Ok(Self::DeltaMismatch),
                "mixed_certificates" => Ok(Self::MixedCertificates),
                "receipt_order_invalid" => Ok(Self::ReceiptOrderInvalid),
                "mixed_counter_scopes" => Ok(Self::MixedCounterScopes),
                "allowance_not_registered" => Ok(Self::AllowanceNotRegistered),
                "certificate_expired" => Ok(Self::CertificateExpired),
                "policy_expired" => Ok(Self::PolicyExpired),
                "receipt_receiver_mismatch" => Ok(Self::ReceiptReceiverMismatch),
                "receipt_sender_mismatch" => Ok(Self::ReceiptSenderMismatch),
                "receipt_asset_mismatch" => Ok(Self::ReceiptAssetMismatch),
                "receipt_timestamp_invalid" => Ok(Self::ReceiptTimestampInvalid),
                "receipt_expired" => Ok(Self::ReceiptExpired),
                "balance_asset_mismatch" => Ok(Self::BalanceAssetMismatch),
                "commitment_mismatch" => Ok(Self::CommitmentMismatch),
                "balance_proof_invalid" => Ok(Self::BalanceProofInvalid),
                "counter_conflict" | "counter_violation" => Ok(Self::CounterViolation),
                "receipt_signature_invalid" => Ok(Self::ReceiptSignatureInvalid),
                "platform_challenge_mismatch" => Ok(Self::PlatformChallengeMismatch),
                "platform_attestation_missing" => Ok(Self::PlatformAttestationMissing),
                "platform_metadata_invalid" => Ok(Self::PlatformMetadataInvalid),
                "platform_attestation_invalid" => Ok(Self::PlatformAttestationInvalid),
                "platform_signature_missing" => Ok(Self::PlatformSignatureMissing),
                "platform_signature_invalid" => Ok(Self::PlatformSignatureInvalid),
                "aggregate_proof_missing" => Ok(Self::AggregateProofMissing),
                "aggregate_proof_version_unsupported" => Ok(Self::AggregateProofVersionUnsupported),
                "aggregate_proof_root_mismatch" => Ok(Self::AggregateProofRootMismatch),
                "aggregate_proof_hash_error" => Ok(Self::AggregateProofHashError),
                "allowance_exceeded" | "allowance_depleted" => Ok(Self::AllowanceDepleted),
                "duplicate_bundle" => Ok(Self::DuplicateBundle),
                "invoice_duplicate" => Ok(Self::InvoiceDuplicate),
                "verdict_expired" => Ok(Self::VerdictExpired),
                "lineage_invalid" => Ok(Self::LineageInvalid),
                "build_claim_missing" => Ok(Self::BuildClaimMissing),
                "build_claim_invalid" => Ok(Self::BuildClaimInvalid),
                "build_claim_expired" => Ok(Self::BuildClaimExpired),
                "build_claim_replayed" => Ok(Self::BuildClaimReplayed),
                "build_claim_build_too_low" => Ok(Self::BuildClaimBuildTooLow),
                _ => Err(()),
            }
        }
    }

    /// Ledger-maintained audit record for an offline-to-online bundle.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineTransferRecord {
        /// Full settlement bundle submitted by the receiver.
        pub transfer: OfflineToOnlineTransfer,
        /// Controller account bound to the originating allowance certificate.
        pub controller: AccountId,
        /// Current lifecycle status enforced by validators.
        pub status: OfflineTransferStatus,
        /// Stable rejection code when status is `rejected`.
        #[norito(default)]
        pub rejection_reason: Option<String>,
        /// Unix timestamp (ms) when the bundle settled on-ledger.
        pub recorded_at_ms: u64,
        /// Block height when the bundle was recorded.
        pub recorded_at_height: u64,
        /// Optional height when the bundle transitioned into the archived tier.
        #[norito(default)]
        pub archived_at_height: Option<u64>,
        /// Ordered lifecycle history recorded for auditing purposes.
        #[norito(default)]
        pub history: Vec<OfflineTransferLifecycleEntry>,
        /// POS importer verdict snapshots captured per receipt bundle (one entry per provided certificate).
        #[norito(default)]
        pub pos_verdict_snapshots: Vec<OfflineVerdictSnapshot>,
        /// Snapshot of the certificate verdict metadata captured at settlement time.
        #[norito(default)]
        pub verdict_snapshot: Option<OfflineVerdictSnapshot>,
        /// Snapshot of the platform-specific attestation token captured at settlement time.
        #[norito(default)]
        pub platform_snapshot: Option<OfflinePlatformTokenSnapshot>,
    }

    impl OfflineToOnlineTransfer {
        fn ordered_receipts(&self) -> Result<Vec<&OfflineSpendReceipt>, OfflineProofRequestError> {
            ensure_single_counter_scope(&self.receipts)?;
            Ok(canonical_receipts(&self.receipts))
        }

        fn proof_request_certificate_id(&self) -> Result<Hash, OfflineProofRequestError> {
            let first = self
                .receipts
                .first()
                .ok_or(OfflineProofRequestError::MissingReceipts)?;
            let certificate_id = first.sender_certificate_id;
            if self
                .receipts
                .iter()
                .any(|receipt| receipt.sender_certificate_id != certificate_id)
            {
                return Err(OfflineProofRequestError::MixedCertificates);
            }
            Ok(certificate_id)
        }

        /// Determine the inferred counter checkpoint for this bundle.
        ///
        /// # Errors
        ///
        /// Returns [`OfflineProofRequestError::MissingReceipts`] when the
        /// transfer lacks any receipts or
        /// [`OfflineProofRequestError::InvalidCounterSequence`] if the receipt
        /// counter cannot be decremented safely.
        pub fn counter_checkpoint_hint(&self) -> Result<u64, OfflineProofRequestError> {
            let receipts = self.ordered_receipts()?;
            let first = receipts
                .first()
                .ok_or(OfflineProofRequestError::MissingReceipts)?;
            first
                .platform_proof
                .counter()
                .checked_sub(1)
                .ok_or(OfflineProofRequestError::InvalidCounterSequence)
        }

        /// Derive the witness request header shared by all FASTPQ proofs.
        ///
        /// # Errors
        ///
        /// Returns [`OfflineProofRequestError`] if the receipts are inconsistent.
        pub fn to_proof_request_header(
            &self,
        ) -> Result<OfflineProofRequestHeader, OfflineProofRequestError> {
            ensure_single_counter_scope(&self.receipts)?;
            let receipts_root = compute_receipts_root(&self.receipts)?;
            let certificate_id = self.proof_request_certificate_id()?;
            Ok(OfflineProofRequestHeader {
                version: OFFLINE_PROOF_REQUEST_VERSION_V1,
                bundle_id: self.bundle_id,
                certificate_id,
                receipts_root,
            })
        }

        /// Build the witness payload for the FASTPQ sum circuit.
        ///
        /// # Errors
        ///
        /// Returns [`OfflineProofRequestError`] if the transfer lacks receipts or a certificate.
        pub fn to_proof_request_sum(
            &self,
        ) -> Result<OfflineProofRequestSum, OfflineProofRequestError> {
            let header = self.to_proof_request_header()?;
            let certificate_id = header.certificate_id;
            let balance_proof = self
                .balance_proof_for_certificate(certificate_id)
                .ok_or(OfflineProofRequestError::MissingBalanceProof)?;
            let receipts = self.ordered_receipts()?;
            let receipt_amounts: Vec<_> = receipts
                .iter()
                .map(|receipt| receipt.amount.clone())
                .collect();
            let blinding_seeds = receipts
                .iter()
                .map(|receipt| {
                    OfflineProofBlindingSeed::derive(
                        certificate_id,
                        receipt.platform_proof.counter(),
                    )
                })
                .collect();
            Ok(OfflineProofRequestSum {
                header,
                initial_commitment: balance_proof.initial_commitment.clone(),
                resulting_commitment: balance_proof.resulting_commitment.clone(),
                claimed_delta: balance_proof.claimed_delta.clone(),
                receipt_amounts,
                blinding_seeds,
            })
        }

        /// Build the witness payload for the FASTPQ counter circuit.
        ///
        /// # Errors
        ///
        /// Returns [`OfflineProofRequestError`] if the transfer lacks receipts or a certificate.
        pub fn to_proof_request_counter(
            &self,
            counter_checkpoint: u64,
        ) -> Result<OfflineProofRequestCounter, OfflineProofRequestError> {
            let header = self.to_proof_request_header()?;
            let receipts = self.ordered_receipts()?;
            let counters = receipts
                .iter()
                .map(|receipt| receipt.platform_proof.counter())
                .collect();
            Ok(OfflineProofRequestCounter {
                header,
                counter_checkpoint,
                counters,
            })
        }

        /// Build the witness payload for the FASTPQ replay circuit.
        ///
        /// # Errors
        ///
        /// Returns [`OfflineProofRequestError`] if the transfer lacks receipts or a certificate.
        pub fn to_proof_request_replay(
            &self,
            replay_log_head: Hash,
            replay_log_tail: Hash,
        ) -> Result<OfflineProofRequestReplay, OfflineProofRequestError> {
            let header = self.to_proof_request_header()?;
            let receipts = self.ordered_receipts()?;
            let tx_ids = receipts.iter().map(|receipt| receipt.tx_id).collect();
            Ok(OfflineProofRequestReplay {
                header,
                replay_log_head,
                replay_log_tail,
                tx_ids,
            })
        }

        /// Build per-certificate FASTPQ sum witness payloads for multi-allowance bundles.
        ///
        /// Requests are returned in deterministic order by `certificate_id`.
        ///
        /// # Errors
        ///
        /// Returns [`OfflineProofRequestError`] if receipts are inconsistent or balance proofs are missing.
        pub fn to_grouped_proof_request_sums(
            &self,
        ) -> Result<Vec<OfflineProofRequestSum>, OfflineProofRequestError> {
            if self.receipts.is_empty() {
                return Err(OfflineProofRequestError::MissingReceipts);
            }

            let mut by_certificate: BTreeMap<Hash, Vec<OfflineSpendReceipt>> = BTreeMap::new();
            for receipt in &self.receipts {
                by_certificate
                    .entry(receipt.sender_certificate_id)
                    .or_default()
                    .push(receipt.clone());
            }

            let mut requests = Vec::with_capacity(by_certificate.len());
            for (certificate_id, certificate_receipts) in by_certificate {
                ensure_single_counter_scope(&certificate_receipts)?;
                let receipts_root = compute_receipts_root(&certificate_receipts)?;
                let balance_proof = self
                    .balance_proof_for_certificate(certificate_id)
                    .ok_or(OfflineProofRequestError::MissingBalanceProof)?;
                let ordered = canonical_receipts(&certificate_receipts);
                let receipt_amounts: Vec<_> = ordered
                    .iter()
                    .map(|receipt| receipt.amount.clone())
                    .collect();
                let blinding_seeds = ordered
                    .iter()
                    .map(|receipt| {
                        OfflineProofBlindingSeed::derive(
                            certificate_id,
                            receipt.platform_proof.counter(),
                        )
                    })
                    .collect();
                requests.push(OfflineProofRequestSum {
                    header: OfflineProofRequestHeader {
                        version: OFFLINE_PROOF_REQUEST_VERSION_V1,
                        bundle_id: self.bundle_id,
                        certificate_id,
                        receipts_root,
                    },
                    initial_commitment: balance_proof.initial_commitment.clone(),
                    resulting_commitment: balance_proof.resulting_commitment.clone(),
                    claimed_delta: balance_proof.claimed_delta.clone(),
                    receipt_amounts,
                    blinding_seeds,
                });
            }
            Ok(requests)
        }
    }

    impl OfflineTransferRecord {
        /// Convenience accessor returning the bundle identifier.
        #[inline]
        #[must_use]
        pub fn bundle_id(&self) -> Hash {
            self.transfer.bundle_id
        }

        /// Convenience accessor returning the controller account id.
        #[inline]
        #[must_use]
        pub fn controller(&self) -> &AccountId {
            &self.controller
        }

        /// Borrow the sender certificate identifier backing this transfer, if receipts are available.
        #[must_use]
        pub fn primary_certificate_id(&self) -> Option<Hash> {
            self.transfer.primary_certificate_id()
        }

        /// Convenience accessor returning the sender certificate identifier when available.
        #[must_use]
        pub fn certificate_id(&self) -> Option<Hash> {
            self.primary_certificate_id()
        }

        /// Collect verdict snapshots for every receipt certificate in the bundle.
        #[must_use]
        pub fn pos_verdict_snapshots(&self) -> Vec<OfflineVerdictSnapshot> {
            self.pos_verdict_snapshots.clone()
        }

        /// Collect verdict snapshots for every receipt certificate in a transfer.
        #[must_use]
        pub fn collect_pos_verdict_snapshots(
            transfer: &OfflineToOnlineTransfer,
            certificate: &OfflineWalletCertificate,
        ) -> Vec<OfflineVerdictSnapshot> {
            transfer
                .receipts
                .iter()
                .map(|receipt| {
                    let mut snapshot = OfflineVerdictSnapshot::from_certificate(certificate);
                    snapshot.certificate_id = receipt.sender_certificate_id;
                    snapshot
                })
                .collect()
        }

        /// Determine the inferred counter checkpoint for this bundle.
        ///
        /// # Errors
        ///
        /// Returns [`OfflineProofRequestError::MissingReceipts`] when the
        /// transfer lacks any receipts or
        /// [`OfflineProofRequestError::InvalidCounterSequence`] if the receipt
        /// counter cannot be decremented safely.
        pub fn counter_checkpoint_hint(&self) -> Result<u64, OfflineProofRequestError> {
            self.transfer.counter_checkpoint_hint()
        }

        /// Derive the witness request header shared by all FASTPQ proofs.
        ///
        /// # Errors
        ///
        /// Returns [`OfflineProofRequestError`] if the receipts are inconsistent.
        pub fn to_proof_request_header(
            &self,
        ) -> Result<OfflineProofRequestHeader, OfflineProofRequestError> {
            self.transfer.to_proof_request_header()
        }

        /// Build the witness payload for the FASTPQ sum circuit.
        ///
        /// # Errors
        ///
        /// Returns [`OfflineProofRequestError`] if the transfer lacks receipts or a certificate.
        pub fn to_proof_request_sum(
            &self,
        ) -> Result<OfflineProofRequestSum, OfflineProofRequestError> {
            self.transfer.to_proof_request_sum()
        }

        /// Build the witness payload for the FASTPQ counter circuit.
        ///
        /// # Errors
        ///
        /// Returns [`OfflineProofRequestError`] if the transfer lacks receipts or a certificate.
        pub fn to_proof_request_counter(
            &self,
            counter_checkpoint: u64,
        ) -> Result<OfflineProofRequestCounter, OfflineProofRequestError> {
            self.transfer.to_proof_request_counter(counter_checkpoint)
        }

        /// Build the witness payload for the FASTPQ replay circuit.
        ///
        /// # Errors
        ///
        /// Returns [`OfflineProofRequestError`] if the transfer lacks receipts or a certificate.
        pub fn to_proof_request_replay(
            &self,
            replay_log_head: Hash,
            replay_log_tail: Hash,
        ) -> Result<OfflineProofRequestReplay, OfflineProofRequestError> {
            self.transfer
                .to_proof_request_replay(replay_log_head, replay_log_tail)
        }

        /// Build per-certificate FASTPQ sum witness payloads for multi-allowance bundles.
        ///
        /// Requests are returned in deterministic order by `certificate_id`.
        ///
        /// # Errors
        ///
        /// Returns [`OfflineProofRequestError`] if receipts are inconsistent or balance proofs are missing.
        pub fn to_grouped_proof_request_sums(
            &self,
        ) -> Result<Vec<OfflineProofRequestSum>, OfflineProofRequestError> {
            self.transfer.to_grouped_proof_request_sums()
        }

        /// Record a lifecycle transition and timestamp for auditing purposes.
        pub fn push_history_entry(
            &mut self,
            status: OfflineTransferStatus,
            transitioned_at_ms: u64,
            verdict_snapshot: Option<&OfflineVerdictSnapshot>,
        ) {
            self.history.push(OfflineTransferLifecycleEntry {
                status,
                transitioned_at_ms,
                verdict_snapshot: verdict_snapshot.cloned(),
            });
        }
    }

    impl OfflineProofBlindingSeed {
        /// Derive a deterministic blinding seed for a receipt counter.
        #[must_use]
        pub fn derive(certificate_id: Hash, counter: u64) -> Self {
            let mut input = Vec::with_capacity(Hash::LENGTH + 8);
            input.extend_from_slice(certificate_id.as_ref());
            input.extend_from_slice(&counter.to_be_bytes());
            let hkdf_salt = Hash::new(input);
            Self { counter, hkdf_salt }
        }
    }

    /// Errors encountered while constructing offline FASTPQ witness requests.
    #[derive(Debug, Error)]
    pub enum OfflineProofRequestError {
        /// The transfer did not carry any receipts.
        #[error("offline transfer missing receipts")]
        MissingReceipts,
        /// Receipts failed to reference a shared certificate.
        #[error("offline transfer missing sender certificate id")]
        MissingCertificate,
        /// Receipts reference multiple certificates and cannot derive a single proof header.
        #[error("offline transfer mixes sender certificates")]
        MixedCertificates,
        /// No balance proof was found for the selected certificate.
        #[error("offline transfer missing balance proof for sender certificate id")]
        MissingBalanceProof,
        /// The first receipt counter cannot produce a checkpoint.
        #[error("first receipt counter is zero; provide an explicit checkpoint")]
        InvalidCounterSequence,
        /// Receipts reference multiple counter scopes.
        #[error("offline transfer mixes counter scopes")]
        MixedCounterScopes,
        /// Counter scope metadata is malformed.
        #[error("invalid counter scope for {platform}: {reason}")]
        InvalidCounterScope {
            /// Platform classification for the scope.
            platform: OfflineTransferRejectionPlatform,
            /// Reason the scope is invalid.
            reason: String,
        },
        /// Poseidon hashing failed while building the proof header.
        #[error(transparent)]
        Poseidon(#[from] crate::offline::poseidon::OfflineReceiptMerkleError),
    }

    #[cfg(test)]
    mod tests {
        use core::str::FromStr;

        use iroha_crypto::{Hash, Signature};

        #[allow(unused_imports)]
        use super::{
            OfflineBalanceProof, OfflineCertificateBalanceProof, OfflineProofRequestError,
            OfflineProofRequestKind, OfflineToOnlineTransfer, OfflineTransferRecord,
            OfflineTransferRejectionReason, OfflineTransferStatus, OfflineVerdictRevocation,
            OfflineVerdictRevocationBundle, OfflineVerdictRevocationReason,
            OfflineWalletCertificate, OfflineWalletPolicy,
        };
        use crate::{AccountId, Metadata};

        #[test]
        fn status_label_roundtrip() {
            for status in [
                OfflineTransferStatus::Settled,
                OfflineTransferStatus::Rejected,
                OfflineTransferStatus::Archived,
            ] {
                let encoded = status.as_label();
                assert_eq!(OfflineTransferStatus::from_str(encoded).unwrap(), status);
            }
        }

        #[test]
        fn revocation_reason_roundtrip() {
            for reason in [
                OfflineVerdictRevocationReason::Unspecified,
                OfflineVerdictRevocationReason::DeviceCompromised,
                OfflineVerdictRevocationReason::DeviceLostOrStolen,
                OfflineVerdictRevocationReason::PolicyViolation,
                OfflineVerdictRevocationReason::IssuerRequest,
            ] {
                let encoded = reason.to_string();
                assert_eq!(
                    OfflineVerdictRevocationReason::from_str(&encoded).unwrap(),
                    reason
                );
            }
        }

        #[test]
        fn rejection_reason_accepts_stable_aliases() {
            assert_eq!(
                OfflineTransferRejectionReason::from_str("counter_conflict").unwrap(),
                OfflineTransferRejectionReason::CounterViolation
            );
            assert_eq!(
                OfflineTransferRejectionReason::from_str("allowance_exceeded").unwrap(),
                OfflineTransferRejectionReason::AllowanceDepleted
            );
            assert_eq!(
                OfflineTransferRejectionReason::from_str("max_tx_value_exceeded").unwrap(),
                OfflineTransferRejectionReason::MaxTxValueExceeded
            );
            assert_eq!(
                OfflineTransferRejectionReason::from_str("balance_proof_invalid").unwrap(),
                OfflineTransferRejectionReason::BalanceProofInvalid
            );
        }

        #[test]
        fn rejection_reason_parses_aggregate_proof_missing() {
            assert_eq!(
                OfflineTransferRejectionReason::from_str("aggregate_proof_missing").unwrap(),
                OfflineTransferRejectionReason::AggregateProofMissing
            );
        }

        #[test]
        fn revocation_bundle_signing_roundtrip() {
            let operator = AccountId::new(
                "ed0120F00DBABE0EDFACE0000000000000000000000000000000000000000000000000"
                    .parse()
                    .expect("public key"),
            );
            let revocation = OfflineVerdictRevocation {
                verdict_id: Hash::new(b"revocation-bundle"),
                issuer: operator.clone(),
                revoked_at_ms: 1_700_000_000,
                reason: OfflineVerdictRevocationReason::DeviceCompromised,
                note: Some("lost device".into()),
                metadata: Metadata::default(),
            };
            let bundle = OfflineVerdictRevocationBundle {
                bundle_id: "revocations-retail-v1".into(),
                sequence: 1,
                published_at_ms: 1_700_000_100,
                operator,
                metadata: Metadata::default(),
                revocations: vec![revocation],
                operator_signature: Signature::from_bytes(&[0; 64]),
            };
            let payload = bundle
                .operator_signing_bytes()
                .expect("bundle payload bytes");
            assert!(!payload.is_empty());
        }
    }

    /// Canonical reason codes attached to offline verdict revocations.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize),
        norito(tag = "revocation_reason", content = "value")
    )]
    pub enum OfflineVerdictRevocationReason {
        /// The specific reason was not provided.
        Unspecified,
        /// The attested device or secure element is believed to be compromised.
        DeviceCompromised,
        /// The device holding the allowance was reported lost or stolen.
        DeviceLostOrStolen,
        /// Governance or policy required the allowance to be withdrawn.
        PolicyViolation,
        /// The issuer requested a manual revocation (offboarding, rotation, etc.).
        IssuerRequest,
    }

    #[allow(clippy::derivable_impls)]
    impl Default for OfflineVerdictRevocationReason {
        fn default() -> Self {
            Self::Unspecified
        }
    }

    impl OfflineVerdictRevocationReason {
        /// Returns the canonical slug stored in Norito/JSON payloads.
        #[must_use]
        pub const fn as_str(self) -> &'static str {
            match self {
                Self::Unspecified => "unspecified",
                Self::DeviceCompromised => "device_compromised",
                Self::DeviceLostOrStolen => "device_lost_or_stolen",
                Self::PolicyViolation => "policy_violation",
                Self::IssuerRequest => "issuer_request",
            }
        }
    }

    impl fmt::Display for OfflineVerdictRevocationReason {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(self.as_str())
        }
    }

    /// Parsing error returned when revocation reason labels are unknown.
    #[derive(Debug, Clone, Copy, Error)]
    #[error("unrecognised offline revocation reason")]
    pub struct ParseOfflineVerdictRevocationReasonError;

    impl FromStr for OfflineVerdictRevocationReason {
        type Err = ParseOfflineVerdictRevocationReasonError;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s {
                "unspecified" => Ok(Self::Unspecified),
                "device_compromised" => Ok(Self::DeviceCompromised),
                "device_lost_or_stolen" => Ok(Self::DeviceLostOrStolen),
                "policy_violation" => Ok(Self::PolicyViolation),
                "issuer_request" => Ok(Self::IssuerRequest),
                _ => Err(ParseOfflineVerdictRevocationReasonError),
            }
        }
    }

    /// Audit record describing a revoked verdict identifier.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineVerdictRevocation {
        /// Identifier returned by the attestation backend that is now considered revoked.
        pub verdict_id: Hash,
        /// Account that executed the revocation on-ledger.
        pub issuer: AccountId,
        /// Unix timestamp (ms) when the revocation was recorded on-chain.
        pub revoked_at_ms: u64,
        /// Canonical reason code describing why the verdict was revoked.
        #[norito(default)]
        pub reason: OfflineVerdictRevocationReason,
        /// Human-readable note or incident identifier supplied by the issuer.
        #[norito(default)]
        pub note: Option<String>,
        /// Additional structured metadata supplied by the issuer.
        #[norito(default)]
        pub metadata: Metadata,
    }

    /// Canonical payload bundled when exporting revocation deny-lists for POS clients.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    pub struct OfflineVerdictRevocationBundlePayload {
        /// Logical identifier for the bundle (`revocations-retail-v1`, etc.).
        pub bundle_id: String,
        /// Rotation sequence number (monotonic per `bundle_id`).
        pub sequence: u32,
        /// Timestamp when the bundle was published (unix ms).
        pub published_at_ms: u64,
        /// Account that signed the bundle.
        pub operator: AccountId,
        /// Additional metadata surfaced to POS clients.
        pub metadata: Metadata,
        /// Revocation records carried by the bundle.
        pub revocations: Vec<OfflineVerdictRevocation>,
    }

    impl From<&OfflineVerdictRevocationBundle> for OfflineVerdictRevocationBundlePayload {
        fn from(bundle: &OfflineVerdictRevocationBundle) -> Self {
            Self {
                bundle_id: bundle.bundle_id.clone(),
                sequence: bundle.sequence,
                published_at_ms: bundle.published_at_ms,
                operator: bundle.operator.clone(),
                metadata: bundle.metadata.clone(),
                revocations: bundle.revocations.clone(),
            }
        }
    }

    /// Signed deny-list bundle distributed to POS devices.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineVerdictRevocationBundle {
        /// Logical identifier for the bundle.
        pub bundle_id: String,
        /// Rotation sequence number (monotonic per bundle id).
        pub sequence: u32,
        /// Timestamp when the bundle was published (unix ms).
        pub published_at_ms: u64,
        /// Account that signed the bundle.
        pub operator: AccountId,
        /// Additional metadata surfaced to POS clients.
        #[norito(default)]
        pub metadata: Metadata,
        /// Revocation records carried by the bundle.
        #[norito(default)]
        pub revocations: Vec<OfflineVerdictRevocation>,
        /// Operator signature over the bundle payload.
        pub operator_signature: Signature,
    }

    impl OfflineVerdictRevocationBundle {
        /// Serialize the bundle payload for signing.
        ///
        /// # Errors
        ///
        /// Returns an error when serialization fails.
        pub fn operator_signing_bytes(&self) -> Result<Vec<u8>, norito::Error> {
            let payload = OfflineVerdictRevocationBundlePayload::from(self);
            to_bytes(&payload)
        }
    }

    /// On-ledger record describing a registered offline allowance and its latest commitment.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineAllowanceRecord {
        /// Operator-issued certificate for the wallet.
        pub certificate: OfflineWalletCertificate,
        /// Latest commitment recorded on-ledger (initially the certificate's commitment).
        pub current_commitment: Vec<u8>,
        /// Unix timestamp (ms) when the record was registered.
        pub registered_at_ms: u64,
        /// Remaining allowance that has not been reconciled on-ledger.
        #[norito(default = "super::remaining_amount_default")]
        pub remaining_amount: Numeric,
        /// Platform counter checkpoints recorded for this allowance.
        #[norito(default)]
        pub counter_state: OfflineCounterState,
        /// Cached attestation verdict identifier (if provided during registration).
        #[norito(default)]
        pub verdict_id: Option<Hash>,
        /// Cached attestation nonce associated with the verdict.
        #[norito(default)]
        pub attestation_nonce: Option<Hash>,
        /// Timestamp (unix ms) indicating when the attestation should be refreshed.
        #[norito(default)]
        pub refresh_at_ms: Option<u64>,
    }

    /// Device assertion proof attached to lineage operations and transfer receipts.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineDeviceAttestation {
        /// Device-bound App Attest / Keystore key identifier.
        pub key_id: String,
        /// Monotonic assertion counter supplied by the platform.
        pub counter: u64,
        /// Base64-encoded platform assertion blob.
        pub assertion_base64: String,
        /// Lowercase hex challenge hash derived from the canonical lineage payload.
        pub challenge_hash_hex: String,
        /// Optional full attestation report for setup / load / refresh binding.
        #[norito(default)]
        pub attestation_report_base64: Option<String>,
        /// Optional iOS team identifier bound into the attestation.
        #[norito(default)]
        pub ios_team_id: Option<String>,
        /// Optional iOS bundle identifier bound into the attestation.
        #[norito(default)]
        pub ios_bundle_id: Option<String>,
        /// Optional iOS environment label bound into the attestation.
        #[norito(default)]
        pub ios_environment: Option<String>,
    }

    /// Canonical device binding carried in offline-cash authorizations.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineCashDeviceBinding {
        /// Platform label (`android` or `ios`).
        pub platform: String,
        /// Device-bound App Attest / Keystore key identifier.
        pub attestation_key_id: String,
        /// Device identifier bound to the lineage.
        pub device_id: String,
        /// Offline public key bound to the lineage.
        pub offline_public_key: String,
        /// Base64-encoded attestation report captured during binding.
        pub attestation_report_base64: String,
        /// Optional iOS team identifier bound into the attestation.
        #[norito(default)]
        pub ios_team_id: Option<String>,
        /// Optional iOS bundle identifier bound into the attestation.
        #[norito(default)]
        pub ios_bundle_id: Option<String>,
        /// Optional iOS environment label bound into the attestation.
        #[norito(default)]
        pub ios_environment: Option<String>,
    }

    /// Issuer-signed policy lease that gates offline spending for one bearer lineage.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineSpendAuthorization {
        /// Deterministic authorization identifier.
        pub authorization_id: String,
        /// Stable bearer lineage identifier.
        pub lineage_id: String,
        /// Controller account bound to this authorization.
        pub account_id: String,
        /// Device identifier bound to this authorization.
        pub device_id: String,
        /// Offline public key bound to this authorization.
        pub offline_public_key: String,
        /// Verdict identifier used for revocation snapshots.
        pub verdict_id: String,
        /// Maximum spendable bearer balance allowed while this authorization is active.
        pub max_balance: String,
        /// Maximum permitted single offline transfer amount.
        pub max_tx_value: String,
        /// Issuance timestamp (unix ms).
        pub issued_at_ms: u64,
        /// Refresh deadline timestamp (unix ms).
        pub refresh_at_ms: u64,
        /// Expiry timestamp (unix ms).
        pub expires_at_ms: u64,
        /// Canonical public device binding, when available.
        #[norito(default)]
        pub device_binding: Option<OfflineCashDeviceBinding>,
        /// Bound device attestation key identifier.
        pub app_attest_key_id: String,
        /// Issuer signature over the unsigned authorization payload.
        pub issuer_signature_base64: String,
    }

    /// Issuer-signed authoritative lineage anchor returned by Torii.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineLineageState {
        /// Stable bearer lineage identifier.
        pub lineage_id: String,
        /// Controller account bound to this lineage.
        pub account_id: String,
        /// Device identifier bound to this lineage.
        pub device_id: String,
        /// Offline public key bound to this lineage.
        pub offline_public_key: String,
        /// Asset definition tracked by this lineage.
        pub asset_definition_id: String,
        /// Total local bearer balance.
        pub balance: String,
        /// Portion of the balance that is parked under policy/expiry constraints.
        pub locked_balance: String,
        /// Authoritative server revision for this bearer lineage.
        pub server_revision: u64,
        /// Canonical state hash anchoring the bearer lineage.
        pub server_state_hash: String,
        /// Latest local receipt revision folded into the anchor.
        pub pending_local_revision: u64,
        /// Active spend authorization snapshot.
        pub authorization: OfflineSpendAuthorization,
        /// Issuer signature over the unsigned lineage-state payload.
        pub issuer_signature_base64: String,
    }

    /// Signed lineage envelope returned to clients.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineLineageEnvelope {
        /// Current lineage anchor.
        pub lineage_state: OfflineLineageState,
        /// On-ledger settlement proof for bearer load or redeem mutations.
        #[norito(default)]
        pub settlement: Option<OfflineMutationSettlement>,
    }

    /// Transparent proof payload binding an offline bearer mutation to a settlement commitment.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineTransparentZkProof {
        /// Proof backend identifier.
        pub backend: String,
        /// Fixed circuit identifier expected by wallets and Torii.
        pub circuit_id: String,
        /// Declared recursion depth.
        pub recursion_depth: u8,
        /// Hex-encoded public inputs commitment.
        pub public_inputs_hex: String,
        /// Norito-encoded proof envelope bytes.
        pub envelope_bytes: Vec<u8>,
    }

    /// Settlement artifact proving a load or redeem mutation finalized on-ledger.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineMutationSettlement {
        /// Mutation kind (`load` or `redeem`).
        pub kind: String,
        /// Client operation identifier.
        pub operation_id: String,
        /// Committed transaction hash.
        pub chain_tx_hash: String,
        /// Entrypoint hash for the committed transaction.
        pub entry_hash: String,
        /// Block height containing the committed transaction.
        pub block_height: u64,
        /// Pre-mutation authoritative state hash.
        pub pre_state_hash: String,
        /// Post-mutation authoritative state hash.
        pub post_state_hash: String,
        /// Hex-encoded settlement commitment bound into the proof domain.
        pub settlement_commitment_hex: String,
        /// Transparent proof attesting to the settlement commitment.
        pub proof: OfflineTransparentZkProof,
    }

    /// Signed revocation bundle distributed to wallets for offline deny-list enforcement.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineRevocationBundle {
        /// Bundle issuance timestamp (unix ms).
        pub issued_at_ms: u64,
        /// Bundle expiry timestamp (unix ms).
        pub expires_at_ms: u64,
        /// Revoked verdict identifiers encoded as lowercase hex strings.
        pub verdict_ids: Vec<String>,
        /// Issuer signature over the unsigned revocation bundle payload.
        pub issuer_signature_base64: String,
    }

    /// Signed lineage receipt exchanged directly between offline peers.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineTransferReceipt {
        /// Wire-format version.
        pub version: i32,
        /// Deterministic transfer identifier.
        pub transfer_id: String,
        /// Transfer direction relative to the holder of `lineage_id`.
        pub direction: String,
        /// Sender or receiver bearer lineage identifier.
        pub lineage_id: String,
        /// Controller account of the lineage holder.
        pub account_id: String,
        /// Device identifier of the lineage holder.
        pub device_id: String,
        /// Offline public key of the lineage holder.
        pub offline_public_key: String,
        /// Total balance before applying the receipt.
        pub pre_balance: String,
        /// Total balance after applying the receipt.
        pub post_balance: String,
        /// Parked balance before applying the receipt.
        pub pre_locked_balance: String,
        /// Parked balance after applying the receipt.
        pub post_locked_balance: String,
        /// Local state hash before applying the receipt.
        pub pre_state_hash: String,
        /// Local state hash after applying the receipt.
        pub post_state_hash: String,
        /// Monotonic local revision after applying the receipt.
        pub local_revision: u64,
        /// Counterparty bearer lineage identifier.
        pub counterparty_lineage_id: String,
        /// Counterparty controller account identifier.
        pub counterparty_account_id: String,
        /// Counterparty device identifier.
        pub counterparty_device_id: String,
        /// Counterparty offline public key.
        pub counterparty_offline_public_key: String,
        /// Transfer amount.
        pub amount: String,
        /// Sender authorization snapshot.
        #[norito(default)]
        pub authorization: Option<OfflineSpendAuthorization>,
        /// Device attestation snapshot bound to this receipt.
        pub attestation: OfflineDeviceAttestation,
        /// Optional outgoing payload used to prove source continuity to the receiver.
        #[norito(default)]
        pub source_payload: Option<String>,
        /// Sender signature over the canonical unsigned receipt payload.
        pub sender_signature_base64: String,
        /// Receipt creation timestamp (unix ms).
        pub created_at_ms: u64,
    }

    /// Outgoing transfer payload shared with a receiver for continuity validation.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineOutgoingTransferPayload {
        /// Wire-format version.
        pub version: i32,
        /// Latest authoritative lineage anchor known to the sender.
        pub anchor: OfflineLineageState,
        /// Optional ancestry receipts bridging from the anchor to the current sender state.
        #[norito(default)]
        pub ancestry_receipts: Vec<OfflineTransferReceipt>,
        /// The outgoing receipt being handed to the receiver.
        pub receipt: OfflineTransferReceipt,
    }

    /// Persisted App Attest binding captured during lineage setup.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineAppleAppAttestBinding {
        /// Original App Attest report used to bind the device lineage.
        pub attestation_report_base64: String,
        /// Expected iOS team identifier.
        pub ios_team_id: String,
        /// Expected iOS bundle identifier.
        pub ios_bundle_id: String,
        /// Expected iOS environment label.
        pub ios_environment: String,
    }

    /// Shared lineage record stored in the Iroha world state.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineLineageRecord {
        /// Current authoritative lineage anchor.
        pub lineage_state: OfflineLineageState,
        /// Device attestation key bound to this bearer lineage.
        pub app_attest_key_id: String,
        /// Highest attestation counter observed per device key identifier.
        #[norito(default)]
        pub counter_book: BTreeMap<String, u64>,
        /// Transfer ids already folded into the lineage anchor.
        #[norito(default)]
        pub seen_transfer_ids: BTreeSet<String>,
        /// Sender-state revisions already folded into the lineage anchor.
        #[norito(default)]
        pub seen_sender_states: BTreeSet<String>,
        /// Persisted iOS App Attest binding, when applicable.
        #[norito(default)]
        pub apple_app_attest_binding: Option<OfflineAppleAppAttestBinding>,
    }

    /// Completed lineage operation result keyed by `kind:operation_id`.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineLineageOperationResult {
        /// Stable operation key (`kind:operation_id`).
        pub operation_key: String,
        /// Operation kind (`load`, `refresh`, `sync`, `redeem`).
        pub kind: String,
        /// Request hash bound to the original client request.
        pub request_hash_hex: String,
        /// Bearer lineage identifier mutated by this operation.
        pub lineage_id: String,
        /// Reserve envelope returned after the successful mutation.
        pub envelope: OfflineLineageEnvelope,
        /// Completion timestamp (unix ms).
        pub completed_at_ms: u64,
    }

    /// Ledger-maintained monotonic counter checkpoints per platform.
    #[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineCounterState {
        /// Highest App Attest hardware counter observed per key id.
        #[norito(default)]
        pub apple_key_counters: BTreeMap<String, u64>,
        /// Highest Android marker-key alias counter observed per series identifier.
        #[norito(default)]
        pub android_series_counters: BTreeMap<String, u64>,
    }

    /// Derived summary exposing the latest platform counter checkpoints for a wallet certificate.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineCounterSummary {
        /// Certificate identifier this summary corresponds to.
        pub certificate_id: Hash,
        /// Controller account associated with the certificate.
        pub controller: AccountId,
        /// Highest App Attest hardware counter observed per key id.
        #[norito(default)]
        pub apple_key_counters: BTreeMap<String, u64>,
        /// Highest Android marker-key alias counter observed per series identifier.
        #[norito(default)]
        pub android_series_counters: BTreeMap<String, u64>,
        /// Deterministic hash over the counter maps for integrity verification.
        pub summary_hash: Hash,
    }

    /// Identifier for an [`OfflineCounterSummary`].
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema, Hash)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineCounterSummaryId {
        /// Certificate identifier.
        pub certificate_id: Hash,
        /// Controller account.
        pub controller: AccountId,
    }

    /// Backend root key descriptor distributed to POS devices.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflinePosBackendRoot {
        /// Human-readable label for the backend key.
        pub label: String,
        /// Logical role describing how the backend key is used (e.g., admission signer).
        pub role: String,
        /// Public key that POS devices pin.
        pub public_key: PublicKey,
        /// Timestamp when the key becomes valid (unix ms).
        pub valid_from_ms: u64,
        /// Timestamp when the key expires (unix ms).
        pub valid_until_ms: u64,
        /// Additional metadata surfaced to tooling (issuer notes, jurisdiction, etc.).
        #[norito(default)]
        pub metadata: Metadata,
    }

    /// Signed manifest describing the backend keys POS devices must trust.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflinePosProvisionManifest {
        /// Logical identifier for the manifest (e.g., `pos-retail-v1`).
        pub manifest_id: String,
        /// Rotation sequence number (monotonic).
        pub sequence: u32,
        /// Timestamp when the manifest was published (unix ms).
        pub published_at_ms: u64,
        /// Timestamp when the manifest becomes active (unix ms).
        pub valid_from_ms: u64,
        /// Timestamp when the manifest expires (unix ms).
        pub valid_until_ms: u64,
        /// Optional hint for the next rotation or refresh deadline (unix ms).
        #[norito(default)]
        pub rotation_hint_ms: Option<u64>,
        /// Operator account that produced the manifest.
        pub operator: AccountId,
        /// Backend root entries pinned by the manifest.
        pub backend_roots: Vec<OfflinePosBackendRoot>,
        /// Additional metadata surfaced to POS tooling.
        #[norito(default)]
        pub metadata: Metadata,
        /// Operator signature over the canonical manifest payload.
        pub operator_signature: Signature,
    }
}

impl OfflineAllowanceRecord {
    /// Deterministic identifier derived from the embedded certificate.
    pub fn certificate_id(&self) -> Hash {
        self.certificate.certificate_id()
    }
}

impl OfflineCounterSummary {
    /// Identifier referencing the controller and certificate the summary belongs to.
    #[must_use]
    pub fn id(&self) -> OfflineCounterSummaryId {
        OfflineCounterSummaryId {
            certificate_id: self.certificate_id,
            controller: self.controller.clone(),
        }
    }

    /// Build a summary directly from an allowance record.
    #[must_use]
    pub fn from_allowance(record: &OfflineAllowanceRecord) -> Self {
        let certificate_id = record.certificate_id();
        let controller = record.certificate.controller.clone();
        let apple_key_counters = record.counter_state.apple_key_counters.clone();
        let android_series_counters = record.counter_state.android_series_counters.clone();
        let summary_hash = Self::compute_hash(&apple_key_counters, &android_series_counters);
        Self {
            certificate_id,
            controller,
            apple_key_counters,
            android_series_counters,
            summary_hash,
        }
    }

    fn compute_hash(
        apple_key_counters: &BTreeMap<String, u64>,
        android_series_counters: &BTreeMap<String, u64>,
    ) -> Hash {
        let payload = (apple_key_counters.clone(), android_series_counters.clone());
        let bytes = to_bytes(&payload).expect("offline counter summary serialization must succeed");
        Hash::new(bytes)
    }
}

impl From<&OfflineAllowanceRecord> for OfflineCounterSummary {
    fn from(record: &OfflineAllowanceRecord) -> Self {
        Self::from_allowance(record)
    }
}

#[cfg(test)]
mod android_metadata_tests {
    use core::str::FromStr;
    use std::iter::FromIterator;

    use iroha_primitives::json::Json;

    use super::*;

    fn name(key: &str) -> Name {
        Name::from_str(key).expect("metadata key")
    }

    fn metadata_with(entries: &[(&str, Json)]) -> Metadata {
        let mut metadata = Metadata::default();
        for (key, value) in entries {
            metadata.insert(name(key), value.clone());
        }
        metadata
    }

    fn put_metadata<T: norito::json::JsonSerialize>(metadata: &mut Metadata, key: &str, value: T) {
        metadata.insert(name(key), Json::new(value));
    }

    #[test]
    fn marker_key_metadata_parses_and_normalizes() {
        let metadata = metadata_with(&[
            (
                AndroidIntegrityPolicy::METADATA_KEY,
                Json::new("marker_key"),
            ),
            (
                ANDROID_PACKAGE_NAMES_KEY,
                Json::new(vec!["Tech.App".to_string()]),
            ),
            (
                ANDROID_SIGNATURE_DIGESTS_KEY,
                Json::new(vec!["11:22:33:44".to_string()]),
            ),
            (ANDROID_REQUIRE_STRONGBOX_KEY, Json::new(true)),
        ]);
        let parsed = AndroidIntegrityMetadata::from_metadata(&metadata)
            .expect("metadata parsed")
            .expect("marker metadata present");
        let AndroidIntegrityMetadata::MarkerKey(marker) = parsed else {
            panic!("expected marker key metadata");
        };
        assert!(marker.package_names.contains("tech.app"));
        assert_eq!(marker.signing_digests_sha256.len(), 1);
        assert_eq!(
            marker
                .signing_digests_sha256
                .iter()
                .next()
                .expect("digest")
                .as_slice(),
            &[0x11, 0x22, 0x33, 0x44]
        );
        assert!(marker.require_strongbox);
        assert!(marker.require_rollback_resistance);
    }

    #[test]
    fn marker_key_metadata_detects_without_slug() {
        let metadata = metadata_with(&[
            (
                ANDROID_PACKAGE_NAMES_KEY,
                Json::new(vec!["tech.app".to_string()]),
            ),
            (
                ANDROID_SIGNATURE_DIGESTS_KEY,
                Json::new(vec!["AQID".to_string()]),
            ),
        ]);
        let parsed = AndroidIntegrityMetadata::from_metadata(&metadata)
            .expect("metadata parsed")
            .expect("marker metadata present");
        assert!(matches!(parsed, AndroidIntegrityMetadata::MarkerKey(_)));
    }

    #[test]
    fn play_integrity_metadata_parses() {
        let metadata = metadata_with(&[
            (
                AndroidIntegrityPolicy::METADATA_KEY,
                Json::new("play_integrity"),
            ),
            (ANDROID_PLAY_PROJECT_KEY, Json::new("1234")),
            (ANDROID_PLAY_ENVIRONMENT_KEY, Json::new("Production")),
            (
                ANDROID_PLAY_PACKAGE_NAMES_KEY,
                Json::new(vec!["tech.app".to_string()]),
            ),
            (
                ANDROID_PLAY_DIGESTS_KEY,
                Json::new(vec![
                    "778899aabbccddeeff00112233445566778899aabbccddeeff00112233445566".to_string(),
                ]),
            ),
            (
                ANDROID_PLAY_APP_VERDICTS_KEY,
                Json::new(vec!["play_recognized".to_string(), "licensed".to_string()]),
            ),
            (
                ANDROID_PLAY_DEVICE_VERDICTS_KEY,
                Json::new(vec!["strong".to_string(), "device".to_string()]),
            ),
            (ANDROID_PLAY_MAX_AGE_KEY, Json::new(5_000_u64)),
        ]);
        let parsed = AndroidIntegrityMetadata::from_metadata(&metadata)
            .expect("metadata parsed")
            .expect("play metadata present");
        let AndroidIntegrityMetadata::PlayIntegrity(play) = parsed else {
            panic!("expected play integrity metadata");
        };
        assert_eq!(play.cloud_project_number, 1234);
        assert_eq!(play.environment.as_str(), "production");
        assert!(
            play.allowed_app_verdicts
                .contains(&PlayIntegrityAppVerdict::PlayRecognized)
        );
        assert!(
            play.allowed_device_verdicts
                .contains(&PlayIntegrityDeviceVerdict::Strong)
        );
        assert_eq!(play.max_token_age_ms, Some(5_000));
    }

    #[test]
    fn marker_key_metadata_accepts_defaults() {
        const DIGEST: &str = "11223344556677889900aabbccddeeff11223344556677889900aabbccddeeff";
        let mut metadata = Metadata::default();
        put_metadata(
            &mut metadata,
            AndroidIntegrityPolicy::METADATA_KEY,
            "marker_key",
        );
        put_metadata(
            &mut metadata,
            ANDROID_PACKAGE_NAMES_KEY,
            vec!["tech.iroha.retail".to_string()],
        );
        put_metadata(
            &mut metadata,
            ANDROID_SIGNATURE_DIGESTS_KEY,
            vec![DIGEST.to_string()],
        );
        put_metadata(&mut metadata, ANDROID_REQUIRE_STRONGBOX_KEY, true);
        put_metadata(&mut metadata, ANDROID_REQUIRE_ROLLBACK_KEY, false);

        let parsed = AndroidIntegrityMetadata::from_metadata(&metadata)
            .expect("metadata parsing must succeed")
            .expect("metadata present");
        let AndroidIntegrityMetadata::MarkerKey(marker) = parsed else {
            panic!("unexpected metadata variant: {parsed:?}");
        };
        let expected_names = BTreeSet::from_iter([String::from("tech.iroha.retail")]);
        assert_eq!(marker.package_names, expected_names);
        assert!(marker.require_strongbox);
        assert!(!marker.require_rollback_resistance);

        let expected_digest = hex::decode(DIGEST).expect("digest fixture must decode as hex");
        let expected_digests = BTreeSet::from_iter([expected_digest]);
        assert_eq!(marker.signing_digests_sha256, expected_digests);
    }

    #[test]
    fn play_integrity_slug_variants_parse() {
        const PLAY_DIGEST: &str =
            "aabbccddeeff00112233445566778899ffeeddccbbaa99887766554433221100";
        let mut metadata = Metadata::default();
        put_metadata(
            &mut metadata,
            AndroidIntegrityPolicy::METADATA_KEY,
            " Play-Integrity ",
        );
        put_metadata(&mut metadata, ANDROID_PLAY_PROJECT_KEY, 42_u64);
        put_metadata(&mut metadata, ANDROID_PLAY_ENVIRONMENT_KEY, "Testing");
        put_metadata(
            &mut metadata,
            ANDROID_PLAY_PACKAGE_NAMES_KEY,
            vec!["tech.iroha.play".to_string()],
        );
        put_metadata(
            &mut metadata,
            ANDROID_PLAY_DIGESTS_KEY,
            vec![PLAY_DIGEST.to_string()],
        );
        put_metadata(
            &mut metadata,
            ANDROID_PLAY_APP_VERDICTS_KEY,
            vec!["PlayRecognized".to_string(), "licensed".to_string()],
        );
        put_metadata(
            &mut metadata,
            ANDROID_PLAY_DEVICE_VERDICTS_KEY,
            vec!["Strong".to_string(), "device".to_string()],
        );
        put_metadata(&mut metadata, ANDROID_PLAY_MAX_AGE_KEY, 86_400_u64);

        let parsed = AndroidIntegrityMetadata::from_metadata(&metadata)
            .expect("metadata parsing must succeed")
            .expect("metadata present");
        let AndroidIntegrityMetadata::PlayIntegrity(play) = parsed else {
            panic!("unexpected metadata variant: {parsed:?}");
        };
        assert_eq!(play.cloud_project_number, 42);
        assert_eq!(play.environment, PlayIntegrityEnvironment::Testing);
        let expected_names = BTreeSet::from_iter([String::from("tech.iroha.play")]);
        assert_eq!(play.package_names, expected_names);
        let expected_digest = hex::decode(PLAY_DIGEST).expect("digest fixture must decode as hex");
        let expected_digests = BTreeSet::from_iter([expected_digest]);
        assert_eq!(play.signing_digests_sha256, expected_digests);
        let expected_app = BTreeSet::from_iter([
            PlayIntegrityAppVerdict::PlayRecognized,
            PlayIntegrityAppVerdict::Licensed,
        ]);
        assert_eq!(play.allowed_app_verdicts, expected_app);
        let expected_device = BTreeSet::from_iter([
            PlayIntegrityDeviceVerdict::Strong,
            PlayIntegrityDeviceVerdict::Device,
        ]);
        assert_eq!(play.allowed_device_verdicts, expected_device);
        assert_eq!(play.max_token_age_ms, Some(86_400));
    }

    #[test]
    fn unsupported_policy_errors() {
        let metadata = metadata_with(&[(
            AndroidIntegrityPolicy::METADATA_KEY,
            Json::new("unknown_policy"),
        )]);
        let err = AndroidIntegrityMetadata::from_metadata(&metadata).unwrap_err();
        assert!(matches!(
            err,
            AndroidIntegrityMetadataError::UnsupportedPolicy { .. }
        ));
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_primitives::numeric::Numeric;
    use norito::decode_from_bytes;

    use super::*;
    use crate::{asset::AssetDefinitionId, domain::DomainId};

    fn sample_signature(seed: u8) -> Signature {
        let mut payload = [0u8; 64];
        for (idx, byte) in payload.iter_mut().enumerate() {
            let offset = u8::try_from(idx).expect("index fits into u8");
            *byte = seed.wrapping_add(offset);
        }
        Signature::from_bytes(&payload)
    }

    fn sample_public_key(seed: u8) -> PublicKey {
        let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        key_pair.public_key().clone()
    }

    fn account_from_key(key: &PublicKey, domain: &str) -> AccountId {
        let _domain_id = DomainId::from_str(domain).expect("domain id");
        AccountId::new(key.clone())
    }

    fn sample_account(seed: u8, domain: &str) -> AccountId {
        let key = sample_public_key(seed);
        account_from_key(&key, domain)
    }

    fn sample_asset(domain: &str) -> AssetId {
        let definition = AssetDefinitionId::new(
            domain.parse().expect("domain id"),
            "usd".parse().expect("asset name"),
        );
        AssetId::new(definition, sample_account(0xD4, domain))
    }

    fn sample_commitment(tag: u8) -> OfflineAllowanceCommitment {
        OfflineAllowanceCommitment {
            asset: sample_asset("acme"),
            amount: Numeric::new(1_000, 0),
            commitment: vec![tag; 32],
        }
    }

    fn sample_receipt() -> OfflineSpendReceipt {
        let sender_key = sample_public_key(0xA1);
        let receiver_key = sample_public_key(0xB2);
        let certificate = OfflineWalletCertificate {
            controller: account_from_key(&sender_key, "acme"),
            operator: account_from_key(&sender_key, "acme"),
            allowance: sample_commitment(0x11),
            spend_public_key: sender_key.clone(),
            attestation_report: vec![0x01, 0x02],
            issued_at_ms: 1_700_000_000,
            expires_at_ms: 1_800_000_000,
            policy: OfflineWalletPolicy {
                max_balance: Numeric::new(5_000, 0),
                max_tx_value: Numeric::new(1_000, 0),
                expires_at_ms: 1_800_000_000,
            },
            operator_signature: sample_signature(0xA0),
            metadata: Metadata::default(),
            verdict_id: None,
            attestation_nonce: None,
            refresh_at_ms: None,
        };
        OfflineSpendReceipt {
            tx_id: Hash::new(b"offline-tx"),
            from: account_from_key(&sender_key, "acme"),
            to: account_from_key(&receiver_key, "acme"),
            asset: sample_asset("acme"),
            amount: Numeric::new(250, 0),
            issued_at_ms: 1_700_000_500,
            invoice_id: "inv-001".into(),
            platform_proof: OfflinePlatformProof::AppleAppAttest(AppleAppAttestProof {
                key_id: BASE64_STANDARD.encode(b"AA_KEY"),
                counter: 42,
                assertion: vec![0xAA, 0xBB],
                challenge_hash: Hash::new(b"challenge"),
            }),
            platform_snapshot: None,
            sender_certificate_id: certificate.certificate_id(),
            sender_signature: sample_signature(0x55),
            build_claim: None,
        }
    }

    #[test]
    fn invoice_id_accessor_returns_value() {
        let receipt = sample_receipt();
        assert_eq!(receipt.invoice_id(), "inv-001");
    }

    fn sample_receipt_with_counter(counter: u64) -> OfflineSpendReceipt {
        let mut receipt = sample_receipt();
        if let OfflinePlatformProof::AppleAppAttest(proof) = &mut receipt.platform_proof {
            proof.counter = counter;
        }
        receipt
    }

    fn sample_transfer_record(counter: u64) -> OfflineTransferRecord {
        let receipt = sample_receipt_with_counter(counter);
        let snapshot_certificate = OfflineWalletCertificate {
            controller: receipt.from.clone(),
            operator: receipt.from.clone(),
            allowance: sample_commitment(0x11),
            spend_public_key: sample_public_key(0xA1),
            attestation_report: vec![0x01, 0x02],
            issued_at_ms: 1_700_000_000,
            expires_at_ms: 1_800_000_000,
            policy: OfflineWalletPolicy {
                max_balance: Numeric::new(5_000, 0),
                max_tx_value: Numeric::new(1_000, 0),
                expires_at_ms: 1_800_000_000,
            },
            operator_signature: sample_signature(0xA0),
            metadata: Metadata::default(),
            verdict_id: None,
            attestation_nonce: None,
            refresh_at_ms: None,
        };
        let bundle_id = Hash::new(b"bundle");
        let balance_proof = OfflineBalanceProof {
            initial_commitment: sample_commitment(0x22),
            resulting_commitment: vec![0u8; 32],
            claimed_delta: receipt.amount.clone(),
            zk_proof: None,
        };
        let transfer = OfflineToOnlineTransfer {
            bundle_id,
            receiver: receipt.to.clone(),
            deposit_account: receipt.to.clone(),
            receipts: vec![receipt],
            balance_proof,
            balance_proofs: None,
            aggregate_proof: None,
            attachments: None,
            platform_snapshot: None,
        };
        let pos_verdict_snapshots =
            OfflineTransferRecord::collect_pos_verdict_snapshots(&transfer, &snapshot_certificate);
        OfflineTransferRecord {
            transfer,
            controller: sample_account(0xA1, "acme"),
            status: OfflineTransferStatus::Settled,
            rejection_reason: None,
            recorded_at_ms: 1,
            recorded_at_height: 1,
            archived_at_height: None,
            history: Vec::new(),
            pos_verdict_snapshots,
            verdict_snapshot: Some(OfflineVerdictSnapshot::from_certificate(
                &snapshot_certificate,
            )),
            platform_snapshot: None,
        }
    }

    #[test]
    fn counter_checkpoint_hint_derives_previous_value() {
        let record = sample_transfer_record(41);
        assert_eq!(record.counter_checkpoint_hint().unwrap(), 40);
    }

    #[test]
    fn counter_checkpoint_hint_rejects_zero() {
        let record = sample_transfer_record(0);
        assert!(matches!(
            record.counter_checkpoint_hint(),
            Err(OfflineProofRequestError::InvalidCounterSequence)
        ));
    }

    #[test]
    fn counter_checkpoint_hint_orders_receipts() {
        let mut receipt_a = sample_receipt_with_counter(10);
        receipt_a.tx_id = Hash::new(b"tx-a");
        let mut receipt_b = sample_receipt_with_counter(8);
        receipt_b.tx_id = Hash::new(b"tx-b");
        let transfer = OfflineToOnlineTransfer {
            bundle_id: Hash::new(b"bundle-order"),
            receiver: receipt_a.to.clone(),
            deposit_account: receipt_a.to.clone(),
            receipts: vec![receipt_a, receipt_b],
            balance_proof: OfflineBalanceProof {
                initial_commitment: sample_commitment(0x22),
                resulting_commitment: vec![0u8; 32],
                claimed_delta: Numeric::new(0, 0),
                zk_proof: None,
            },
            balance_proofs: None,
            aggregate_proof: None,
            attachments: None,
            platform_snapshot: None,
        };
        assert_eq!(transfer.counter_checkpoint_hint().unwrap(), 7);
    }

    #[test]
    fn proof_request_header_rejects_mixed_certificates() {
        let mut receipt_a = sample_receipt_with_counter(8);
        receipt_a.sender_certificate_id = Hash::new(b"cert-a");
        let mut receipt_b = sample_receipt_with_counter(9);
        receipt_b.sender_certificate_id = Hash::new(b"cert-b");
        let transfer = OfflineToOnlineTransfer {
            bundle_id: Hash::new(b"bundle-mixed-cert"),
            receiver: receipt_a.to.clone(),
            deposit_account: receipt_a.to.clone(),
            receipts: vec![receipt_a, receipt_b],
            balance_proof: OfflineBalanceProof {
                initial_commitment: sample_commitment(0x22),
                resulting_commitment: vec![0x44; 32],
                claimed_delta: Numeric::new(200, 0),
                zk_proof: None,
            },
            balance_proofs: None,
            aggregate_proof: None,
            attachments: None,
            platform_snapshot: None,
        };

        assert!(matches!(
            transfer.to_proof_request_header(),
            Err(OfflineProofRequestError::MixedCertificates)
        ));
    }

    #[test]
    fn balance_proof_lookup_prefers_per_certificate_entries() {
        let receipt = sample_receipt_with_counter(1);
        let certificate_id = receipt.sender_certificate_id;
        let mapped_proof = OfflineBalanceProof {
            initial_commitment: sample_commitment(0x66),
            resulting_commitment: vec![0x77; 32],
            claimed_delta: Numeric::new(10, 0),
            zk_proof: None,
        };
        let transfer = OfflineToOnlineTransfer {
            bundle_id: Hash::new(b"bundle-proof-map"),
            receiver: receipt.to.clone(),
            deposit_account: receipt.to.clone(),
            receipts: vec![receipt],
            balance_proof: OfflineBalanceProof {
                initial_commitment: sample_commitment(0x22),
                resulting_commitment: vec![0x33; 32],
                claimed_delta: Numeric::new(10, 0),
                zk_proof: None,
            },
            balance_proofs: Some(vec![OfflineCertificateBalanceProof::new(
                certificate_id,
                mapped_proof.clone(),
            )]),
            aggregate_proof: None,
            attachments: None,
            platform_snapshot: None,
        };

        let resolved = transfer
            .balance_proof_for_certificate(certificate_id)
            .expect("mapped proof");
        assert_eq!(resolved, &mapped_proof);
    }

    #[test]
    fn canonical_app_attest_key_id_accepts_standard_base64() {
        let key_id = BASE64_STANDARD.encode(b"app-attest-key");
        let canonical = canonical_app_attest_key_id(&key_id).expect("canonical key id");
        assert_eq!(canonical, key_id);
    }

    #[test]
    fn canonical_app_attest_key_id_rejects_non_canonical() {
        let err = canonical_app_attest_key_id("AA_KEY").expect_err("non-canonical key id");
        assert!(matches!(
            err,
            OfflineProofRequestError::InvalidCounterScope {
                platform: OfflineTransferRejectionPlatform::Apple,
                ..
            }
        ));
    }

    #[test]
    fn marker_series_derives_prefix() {
        let series = marker_series_from_public_key(&[0x01, 0x02, 0x03]).expect("series");
        assert!(series.starts_with(MARKER_COUNTER_PREFIX));
    }

    #[test]
    fn receipts_canonical_ordering_helpers() {
        let receipt_a = sample_receipt_with_counter(2);
        let receipt_b = sample_receipt_with_counter(1);
        assert!(!receipts_are_canonical(&[
            receipt_a.clone(),
            receipt_b.clone()
        ]));
        let receipts = [receipt_a, receipt_b];
        let ordered = canonical_receipts(&receipts);
        assert_eq!(ordered.first().unwrap().platform_proof.counter(), 1);
    }

    #[test]
    fn ensure_single_counter_scope_rejects_mixed_keys() {
        let receipt_a = sample_receipt_with_counter(1);
        let mut receipt_b = sample_receipt_with_counter(2);
        if let OfflinePlatformProof::AppleAppAttest(proof) = &mut receipt_b.platform_proof {
            proof.key_id = BASE64_STANDARD.encode(b"OTHER_KEY");
        }
        assert!(matches!(
            ensure_single_counter_scope(&[receipt_a, receipt_b]),
            Err(OfflineProofRequestError::MixedCounterScopes)
        ));
    }

    #[test]
    fn proof_requests_build_payloads() {
        let record = sample_transfer_record(50);
        let sum = record.to_proof_request_sum().expect("sum request");
        assert_eq!(sum.receipt_amounts.len(), 1);
        assert_eq!(sum.blinding_seeds.len(), 1);

        let checkpoint = record.counter_checkpoint_hint().expect("checkpoint");
        let counter = record
            .to_proof_request_counter(checkpoint)
            .expect("counter request");
        assert_eq!(counter.counters, vec![50]);

        let replay = record
            .to_proof_request_replay(Hash::new(b"head"), Hash::new(b"tail"))
            .expect("replay request");
        assert_eq!(replay.tx_ids.len(), 1);
    }

    #[test]
    fn transfer_proof_requests_build_payloads() {
        let record = sample_transfer_record(50);
        let transfer = record.transfer.clone();
        assert_eq!(transfer.counter_checkpoint_hint().unwrap(), 49);

        let sum = transfer.to_proof_request_sum().expect("sum request");
        assert_eq!(sum.receipt_amounts.len(), 1);
        assert_eq!(sum.blinding_seeds.len(), 1);

        let counter = transfer
            .to_proof_request_counter(49)
            .expect("counter request");
        assert_eq!(counter.counters, vec![50]);

        let replay = transfer
            .to_proof_request_replay(Hash::new(b"head"), Hash::new(b"tail"))
            .expect("replay request");
        assert_eq!(replay.tx_ids.len(), 1);
    }

    #[test]
    fn proof_kind_parses_lowercase() {
        assert_eq!(
            OfflineProofRequestKind::from_str("counter").unwrap(),
            OfflineProofRequestKind::Counter
        );
        assert!(OfflineProofRequestKind::from_str("invalid").is_err());
    }

    #[test]
    fn receipt_challenge_bytes_roundtrip() {
        let receipt = sample_receipt();
        let bytes = receipt.challenge_bytes().expect("challenge bytes");
        let decoded: OfflineReceiptChallengePreimage =
            decode_from_bytes(&bytes).expect("decode preimage");
        assert_eq!(decoded, receipt.challenge_preimage());
    }

    #[test]
    fn offline_to_online_transfer_roundtrip() {
        let transfer = OfflineToOnlineTransfer {
            bundle_id: Hash::new(b"bundle"),
            receiver: sample_account(0xB2, "acme"),
            deposit_account: sample_account(0xC3, "acme"),
            receipts: vec![sample_receipt()],
            balance_proof: OfflineBalanceProof {
                initial_commitment: sample_commitment(0x22),
                resulting_commitment: vec![0x33; 32],
                claimed_delta: Numeric::new(250, 0),
                zk_proof: Some(vec![0xDE, 0xAD, 0xBE, 0xEF]),
            },
            balance_proofs: None,
            aggregate_proof: None,
            attachments: None,
            platform_snapshot: Some(OfflinePlatformTokenSnapshot {
                policy: AndroidIntegrityPolicy::PlayIntegrity.as_str().into(),
                attestation_jws_b64: "token".into(),
            }),
        };

        let buf = transfer.encode();
        let mut cursor = buf.as_slice();
        let decoded = OfflineToOnlineTransfer::decode(&mut cursor).expect("decode transfer");
        assert_eq!(decoded, transfer);
    }

    #[test]
    fn counter_summary_hash_matches_tuple_encoding() {
        let mut apple = BTreeMap::new();
        apple.insert("app.attest:k1".into(), 17);
        apple.insert("app.attest:k9".into(), 23);

        let mut android = BTreeMap::new();
        android.insert("pixel-7a".into(), 5);
        android.insert("pixel-8".into(), 9);

        let receipt = sample_receipt();
        let certificate = OfflineWalletCertificate {
            controller: receipt.from.clone(),
            operator: receipt.from.clone(),
            allowance: sample_commitment(0x11),
            spend_public_key: sample_public_key(0xA1),
            attestation_report: vec![0x01, 0x02],
            issued_at_ms: 1_700_000_000,
            expires_at_ms: 1_800_000_000,
            policy: OfflineWalletPolicy {
                max_balance: Numeric::new(5_000, 0),
                max_tx_value: Numeric::new(1_000, 0),
                expires_at_ms: 1_800_000_000,
            },
            operator_signature: sample_signature(0xA0),
            metadata: Metadata::default(),
            verdict_id: None,
            attestation_nonce: None,
            refresh_at_ms: None,
        };
        let record = OfflineAllowanceRecord {
            certificate: certificate.clone(),
            current_commitment: certificate.allowance.commitment.clone(),
            registered_at_ms: 1_700_000_123,
            remaining_amount: Numeric::new(750, 0),
            counter_state: OfflineCounterState {
                apple_key_counters: apple.clone(),
                android_series_counters: android.clone(),
            },
            verdict_id: None,
            attestation_nonce: None,
            refresh_at_ms: None,
        };

        let summary = OfflineCounterSummary::from_allowance(&record);
        let payload = (apple.clone(), android.clone());
        let expected_hash =
            Hash::new(to_bytes(&payload).expect("tuple serialization must succeed"));

        assert_eq!(expected_hash, summary.summary_hash);
        assert_eq!(
            expected_hash,
            OfflineCounterSummary::compute_hash(&apple, &android)
        );
    }

    #[test]
    fn lifecycle_history_tracks_transitions() {
        let mut record = sample_transfer_record(21);
        assert!(record.history.is_empty());
        let expected_snapshot = record.verdict_snapshot.clone();
        let snapshot_ref = expected_snapshot.as_ref();
        record.push_history_entry(OfflineTransferStatus::Settled, 1_001, snapshot_ref);
        record.push_history_entry(OfflineTransferStatus::Archived, 2_002, snapshot_ref);
        assert_eq!(record.history.len(), 2);
        assert_eq!(record.history[0].status, OfflineTransferStatus::Settled);
        assert_eq!(record.history[0].transitioned_at_ms, 1_001);
        assert_eq!(record.history[0].verdict_snapshot.as_ref(), snapshot_ref);
        assert_eq!(record.history[1].status, OfflineTransferStatus::Archived);
        assert_eq!(record.history[1].transitioned_at_ms, 2_002);
        assert_eq!(record.history[1].verdict_snapshot.as_ref(), snapshot_ref);
    }

    #[test]
    fn verdict_snapshot_captures_certificate_metadata() {
        let record = sample_transfer_record(33);
        let snapshot = record.verdict_snapshot.as_ref().expect("snapshot");
        assert_eq!(
            snapshot.certificate_id,
            record.transfer.primary_certificate_id().unwrap()
        );
        assert_eq!(
            snapshot.certificate_expires_at_ms,
            record
                .verdict_snapshot
                .as_ref()
                .unwrap()
                .certificate_expires_at_ms
        );
        assert_eq!(
            snapshot.policy_expires_at_ms,
            record
                .verdict_snapshot
                .as_ref()
                .unwrap()
                .policy_expires_at_ms
        );
        assert_eq!(snapshot.verdict_id, None);
    }

    #[test]
    fn platform_snapshot_norito_roundtrip() {
        let snapshot = OfflinePlatformTokenSnapshot {
            policy: AndroidIntegrityPolicy::PlayIntegrity.as_str().to_string(),
            attestation_jws_b64: "token".to_string(),
        };
        let bytes = to_bytes(&snapshot).expect("serialize platform snapshot");
        let decoded: OfflinePlatformTokenSnapshot =
            decode_from_bytes(&bytes).expect("decode platform snapshot");
        assert_eq!(decoded, snapshot);
    }

    #[test]
    fn receipt_platform_snapshot_roundtrip() {
        let mut receipt = sample_receipt();
        receipt.platform_snapshot = Some(OfflinePlatformTokenSnapshot {
            policy: AndroidIntegrityPolicy::HmsSafetyDetect.as_str().to_string(),
            attestation_jws_b64: "per-receipt-token".to_string(),
        });
        let bytes = receipt.encode();
        let mut cursor = bytes.as_slice();
        let decoded = OfflineSpendReceipt::decode(&mut cursor).expect("decode receipt");
        assert_eq!(decoded.platform_snapshot, receipt.platform_snapshot);
    }
}

#[cfg(test)]
mod pos_manifest_tests {
    use core::str::FromStr;

    use super::*;
    use crate::domain::DomainId;

    #[test]
    fn operator_signing_payload_roundtrips() {
        let public_key = PublicKey::from_str(
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03",
        )
        .expect("public key");
        let _domain = DomainId::from_str("wonderland").expect("domain id");
        let operator = AccountId::new(public_key.clone());
        let backend_root = OfflinePosBackendRoot {
            label: "torii-admission".to_string(),
            role: "offline_admission_signer".to_string(),
            public_key,
            valid_from_ms: 1_730_314_876_000,
            valid_until_ms: 1_740_314_876_000,
            metadata: Metadata::default(),
        };
        let manifest = OfflinePosProvisionManifest {
            manifest_id: "pos-retail-v1".to_string(),
            sequence: 7,
            published_at_ms: 1_730_314_876_000,
            valid_from_ms: 1_730_314_876_000,
            valid_until_ms: 1_740_314_876_000,
            rotation_hint_ms: Some(1_735_000_000_000),
            operator,
            backend_roots: vec![backend_root.clone()],
            metadata: Metadata::default(),
            operator_signature: Signature::from_bytes(&[0; 64]),
        };

        let payload_bytes = manifest
            .operator_signing_bytes()
            .expect("serialize payload");
        let expected = OfflinePosProvisionManifestPayload::from(&manifest);
        let expected_bytes = to_bytes(&expected).expect("encode manifest payload for comparison");
        assert_eq!(payload_bytes, expected_bytes);
    }
}

#[cfg(test)]
mod receipt_challenge_tests {
    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use iroha_primitives::numeric::Numeric;

    use super::*;
    use crate::{
        ChainId,
        asset::{AssetDefinitionId, AssetId},
        domain::DomainId,
        metadata::Metadata,
    };

    fn sample_account() -> AccountId {
        let key_pair = KeyPair::from_seed(vec![0xA1; 32], Algorithm::Ed25519);
        let _domain = DomainId::from_str("wonderland").expect("domain id");
        AccountId::new(key_pair.public_key().clone())
    }

    fn sample_receiver() -> AccountId {
        let key_pair = KeyPair::from_seed(vec![0xB2; 32], Algorithm::Ed25519);
        let _domain = DomainId::from_str("soramitsu").expect("domain id");
        AccountId::new(key_pair.public_key().clone())
    }

    fn sample_asset(owner: &AccountId) -> AssetId {
        let definition =
            AssetDefinitionId::new("wonderland".parse().unwrap(), "xor".parse().unwrap());
        AssetId::new(definition, owner.clone())
    }

    fn sample_certificate(controller: &AccountId, asset: &AssetId) -> OfflineWalletCertificate {
        OfflineWalletCertificate {
            controller: controller.clone(),
            operator: controller.clone(),
            allowance: OfflineAllowanceCommitment {
                asset: asset.clone(),
                amount: Numeric::from(500_u32),
                commitment: vec![0x42; 32],
            },
            spend_public_key: PublicKey::from_hex(
                Algorithm::Ed25519,
                "1509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4",
            )
            .expect("public key"),
            attestation_report: vec![0x01, 0x02, 0x03],
            issued_at_ms: 1_700_000_000_000,
            expires_at_ms: 1_800_000_000_000,
            policy: OfflineWalletPolicy {
                max_balance: Numeric::from(1_000_u32),
                max_tx_value: Numeric::from(200_u32),
                expires_at_ms: 1_800_000_000_000,
            },
            operator_signature: Signature::from_bytes(&[0xAB; 64]),
            metadata: Metadata::default(),
            verdict_id: None,
            attestation_nonce: None,
            refresh_at_ms: None,
        }
    }

    fn sample_platform_proof(challenge_hash: Hash) -> OfflinePlatformProof {
        OfflinePlatformProof::AppleAppAttest(AppleAppAttestProof {
            key_id: BASE64_STANDARD.encode(b"TEST_KEY"),
            counter: 42,
            assertion: vec![0xCA, 0xFE],
            challenge_hash,
        })
    }

    fn sample_receipt() -> OfflineSpendReceipt {
        let sender = sample_account();
        let receiver = sample_receiver();
        let asset = sample_asset(&sender);
        let certificate = sample_certificate(&sender, &asset);
        OfflineSpendReceipt {
            tx_id: Hash::new(vec![0x22; 32]),
            from: sender.clone(),
            to: receiver,
            asset: asset.clone(),
            amount: Numeric::from(75_u32),
            issued_at_ms: 1_700_000_500_000,
            invoice_id: "INV-42".into(),
            platform_proof: sample_platform_proof(Hash::new(vec![0x33; 32])),
            platform_snapshot: None,
            sender_certificate_id: certificate.certificate_id(),
            sender_signature: Signature::from_bytes(&[0xCD; 64]),
            build_claim: None,
        }
    }

    #[test]
    fn preimage_hash_matches_manual_hash() {
        let preimage = OfflineReceiptChallengePreimage {
            invoice_id: "INV-7".into(),
            receiver: sample_receiver(),
            asset: sample_asset(&sample_account()),
            amount: Numeric::from(123_u32),
            issued_at_ms: 1_700_000_400_000,
            sender_certificate_id: Hash::new(vec![0x44; 32]),
            nonce: Hash::new(vec![0x10; 32]),
        };
        let raw = preimage.to_bytes().expect("serialize preimage");
        let manual = Hash::new(raw);
        assert_eq!(preimage.hash().expect("hash preimage"), manual);
    }

    #[test]
    fn receipt_helpers_match_preimage() {
        let receipt = sample_receipt();
        let expected_preimage = OfflineReceiptChallengePreimage::from_receipt(&receipt);
        assert_eq!(receipt.challenge_preimage(), expected_preimage);
        let expected_hash = expected_preimage.hash().expect("hash preimage");
        assert_eq!(
            receipt.challenge_hash().expect("hash receipt"),
            expected_hash
        );
    }

    #[test]
    fn receipt_hash_binds_chain_id() {
        let receipt = sample_receipt();
        let chain_a: ChainId = "alpha".parse().expect("chain id");
        let chain_b: ChainId = "beta".parse().expect("chain id");
        let hash_a = receipt
            .challenge_hash_with_chain_id(&chain_a)
            .expect("hash receipt alpha");
        let hash_b = receipt
            .challenge_hash_with_chain_id(&chain_b)
            .expect("hash receipt beta");
        assert_ne!(hash_a, hash_b, "chain context must affect receipt hash");
    }

    #[test]
    fn certificate_controller_getter_returns_controller() {
        let controller = sample_account();
        let asset = sample_asset(&controller);
        let certificate = sample_certificate(&controller, &asset);
        assert_eq!(certificate.controller(), &controller);
    }

    #[test]
    fn aggregate_proof_getter_exposes_payload() {
        let controller = sample_account();
        let asset = sample_asset(&controller);
        let balance_proof = OfflineBalanceProof {
            initial_commitment: OfflineAllowanceCommitment {
                asset: asset.clone(),
                amount: Numeric::from(500_u32),
                commitment: vec![0xAA; 32],
            },
            resulting_commitment: vec![0xBB; 32],
            claimed_delta: Numeric::from(75_u32),
            zk_proof: None,
        };
        let aggregate = AggregateProofEnvelope {
            version: AGGREGATE_PROOF_VERSION_V1,
            receipts_root: poseidon::PoseidonDigest::zero(),
            proof_sum: None,
            proof_counter: None,
            proof_replay: None,
            metadata: Metadata::default(),
        };
        let transfer = OfflineToOnlineTransfer {
            bundle_id: Hash::new(vec![0xAA; 32]),
            receiver: sample_receiver(),
            deposit_account: controller,
            receipts: vec![sample_receipt()],
            balance_proof,
            balance_proofs: None,
            aggregate_proof: Some(aggregate.clone()),
            attachments: None,
            platform_snapshot: None,
        };

        let proof = transfer.aggregate_proof().expect("aggregate proof present");
        assert_eq!(proof.version, AGGREGATE_PROOF_VERSION_V1);
        assert_eq!(proof.receipts_root, aggregate.receipts_root);
    }
}
