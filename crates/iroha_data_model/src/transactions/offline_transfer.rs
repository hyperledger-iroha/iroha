//! Offline transfer query responses shared with Torii and SDKs.

use std::convert::TryFrom;

use hex::encode as encode_hex;
use iroha_primitives::numeric::Numeric;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{
    account::AccountId,
    offline::{
        OfflinePlatformTokenSnapshot, OfflineToOnlineTransfer, OfflineTransferLifecycleEntry,
        OfflineTransferRecord, OfflineTransferStatus, OfflineVerdictSnapshot,
    },
};

/// Canonical list response returned by the offline transfer endpoints.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OfflineTransferList {
    /// Items captured by the query or list operation.
    pub items: Vec<OfflineTransferSummary>,
    /// Total number of records that matched the filter prior to pagination.
    pub total: u64,
}

/// Flattened view of an [`OfflineTransferRecord`] surfaced to HTTP consumers.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OfflineTransferSummary {
    /// Controller that owns the originating allowance.
    pub controller: AccountId,
    /// Canonical bundle identifier (hex).
    pub bundle_id_hex: String,
    /// Receiver account literal that initiated the deposit.
    pub receiver: AccountId,
    /// Online account that received the ledger deposit.
    pub deposit_account: AccountId,
    /// Canonical asset identifier derived from the first receipt.
    #[norito(default)]
    pub asset_id: Option<String>,
    /// Number of receipts included in the bundle.
    pub receipt_count: u64,
    /// Sum of all receipt amounts included in the bundle.
    pub total_amount: Numeric,
    /// Lifecycle status tracked on-ledger.
    pub status: OfflineTransferStatus,
    /// Stable rejection code when `status=rejected`.
    #[norito(default)]
    pub rejection_reason: Option<String>,
    /// Unix timestamp (ms) when the bundle settled on-ledger.
    pub recorded_at_ms: u64,
    /// Height where the bundle settled.
    pub recorded_at_height: u64,
    /// Optional height when the bundle moved to the archived tier.
    #[norito(default)]
    pub archived_at_height: Option<u64>,
    /// Optional certificate identifier (hex, lowercase).
    #[norito(default)]
    pub certificate_id_hex: Option<String>,
    /// Optional timestamp when the certificate expires.
    #[norito(default)]
    pub certificate_expires_at_ms: Option<u64>,
    /// Optional timestamp when the wallet policy expires.
    #[norito(default)]
    pub policy_expires_at_ms: Option<u64>,
    /// Optional timestamp when the attestation/verdict must be refreshed.
    #[norito(default)]
    pub refresh_at_ms: Option<u64>,
    /// Optional attestation verdict identifier (hex, lowercase).
    #[norito(default)]
    pub verdict_id_hex: Option<String>,
    /// Optional attestation nonce (hex, lowercase).
    #[norito(default)]
    pub attestation_nonce_hex: Option<String>,
    /// Full transfer bundle submitted by the receiver.
    pub transfer: OfflineToOnlineTransfer,
    /// Lifecycle history recorded for the bundle.
    #[norito(default)]
    pub history: Vec<OfflineTransferLifecycleEntry>,
    /// POS importer verdict snapshots for the receipts in this bundle.
    #[norito(default)]
    pub pos_verdict_snapshots: Vec<OfflineVerdictSnapshot>,
    /// Snapshot of the certificate verdict metadata captured at settlement.
    #[norito(default)]
    pub verdict_snapshot: Option<OfflineVerdictSnapshot>,
    /// Snapshot of the platform-specific attestation token captured at settlement.
    #[norito(default)]
    pub platform_snapshot: Option<OfflinePlatformTokenSnapshot>,
}

impl From<OfflineTransferRecord> for OfflineTransferSummary {
    fn from(record: OfflineTransferRecord) -> Self {
        let bundle_id_hex = encode_hex(record.bundle_id().as_ref());
        let asset_id = first_receipt_asset_literal(&record);
        let receipt_count =
            u64::try_from(record.transfer.receipts.len()).expect("receipt count fits into u64");
        let total_amount = sum_receipt_amounts(&record);
        let certificate_id_hex = transfer_certificate_id_hex(&record);
        let certificate_expires_at_ms = transfer_certificate_expires_at_ms(&record);
        let policy_expires_at_ms = transfer_policy_expires_at_ms(&record);
        let refresh_at_ms = transfer_refresh_at_ms(&record);
        let verdict_id_hex = transfer_verdict_hex(&record);
        let attestation_nonce_hex = transfer_attestation_nonce_hex(&record);
        let OfflineTransferRecord {
            transfer,
            controller,
            status,
            rejection_reason,
            recorded_at_ms,
            recorded_at_height,
            archived_at_height,
            history,
            pos_verdict_snapshots,
            verdict_snapshot,
            platform_snapshot,
            ..
        } = record;
        Self {
            controller,
            bundle_id_hex,
            receiver: transfer.receiver.clone(),
            deposit_account: transfer.deposit_account.clone(),
            asset_id,
            receipt_count,
            total_amount,
            status,
            rejection_reason,
            recorded_at_ms,
            recorded_at_height,
            archived_at_height,
            certificate_id_hex,
            certificate_expires_at_ms,
            policy_expires_at_ms,
            refresh_at_ms,
            verdict_id_hex,
            attestation_nonce_hex,
            transfer,
            history,
            pos_verdict_snapshots,
            verdict_snapshot,
            platform_snapshot,
        }
    }
}

impl OfflineTransferSummary {
    /// Canonical integrity policy backing the transfer, if available.
    #[must_use]
    pub fn platform_policy_label(&self) -> Option<String> {
        resolve_platform_policy_label(
            self.platform_snapshot.as_ref(),
            self.transfer.platform_snapshot.as_ref(),
        )
    }
}

fn first_receipt_asset_literal(record: &OfflineTransferRecord) -> Option<String> {
    record
        .transfer
        .receipts
        .first()
        .map(|receipt| receipt.asset.to_string())
}

fn sum_receipt_amounts(record: &OfflineTransferRecord) -> Numeric {
    record
        .transfer
        .receipts
        .iter()
        .fold(Numeric::zero(), |total, receipt| {
            total
                .checked_add(receipt.amount().clone())
                .expect("receipt totals exceed Numeric bounds")
        })
}

fn primary_pos_snapshot(record: &OfflineTransferRecord) -> Option<&OfflineVerdictSnapshot> {
    record.pos_verdict_snapshots.first()
}

/// Canonical bundle identifier associated with an offline transfer.
#[must_use]
pub fn transfer_certificate_id_hex(record: &OfflineTransferRecord) -> Option<String> {
    record
        .certificate_id()
        .map(|hash| encode_hex(hash.as_ref()))
}

/// Canonical verdict identifier (hex) tied to the bundle.
#[must_use]
pub fn transfer_verdict_hex(record: &OfflineTransferRecord) -> Option<String> {
    record
        .verdict_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.verdict_id)
        .map(|hash| encode_hex(hash.as_ref()))
        .or_else(|| {
            primary_pos_snapshot(record)
                .and_then(|snapshot| snapshot.verdict_id)
                .map(|hash| encode_hex(hash.as_ref()))
        })
}

/// Canonical attestation nonce (hex) tied to the bundle.
#[must_use]
pub fn transfer_attestation_nonce_hex(record: &OfflineTransferRecord) -> Option<String> {
    record
        .verdict_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.attestation_nonce)
        .map(|hash| encode_hex(hash.as_ref()))
        .or_else(|| {
            primary_pos_snapshot(record)
                .and_then(|snapshot| snapshot.attestation_nonce)
                .map(|hash| encode_hex(hash.as_ref()))
        })
}

/// Timestamp when the attestation/verdict must be refreshed.
#[must_use]
pub fn transfer_refresh_at_ms(record: &OfflineTransferRecord) -> Option<u64> {
    record
        .verdict_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.refresh_at_ms)
        .or_else(|| primary_pos_snapshot(record).and_then(|snapshot| snapshot.refresh_at_ms))
}

/// Timestamp when the originating certificate expires.
#[must_use]
pub fn transfer_certificate_expires_at_ms(record: &OfflineTransferRecord) -> Option<u64> {
    record
        .verdict_snapshot
        .as_ref()
        .map(|snapshot| snapshot.certificate_expires_at_ms)
        .or_else(|| primary_pos_snapshot(record).map(|snapshot| snapshot.certificate_expires_at_ms))
}

/// Timestamp when the wallet policy expires.
#[must_use]
pub fn transfer_policy_expires_at_ms(record: &OfflineTransferRecord) -> Option<u64> {
    record
        .verdict_snapshot
        .as_ref()
        .map(|snapshot| snapshot.policy_expires_at_ms)
        .or_else(|| primary_pos_snapshot(record).map(|snapshot| snapshot.policy_expires_at_ms))
}

/// Policy slug recorded in the platform snapshot.
#[must_use]
pub fn transfer_platform_policy_label(record: &OfflineTransferRecord) -> Option<String> {
    resolve_platform_policy_label(
        record.platform_snapshot.as_ref(),
        record.transfer.platform_snapshot.as_ref(),
    )
}

fn resolve_platform_policy_label(
    record_snapshot: Option<&OfflinePlatformTokenSnapshot>,
    transfer_snapshot: Option<&OfflinePlatformTokenSnapshot>,
) -> Option<String> {
    record_snapshot
        .map(|snapshot| snapshot.policy_label().to_string())
        .or_else(|| transfer_snapshot.map(|snapshot| snapshot.policy_label().to_string()))
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
    use iroha_crypto::{Algorithm, Hash, KeyPair, PublicKey, Signature};
    use iroha_primitives::numeric::Numeric;
    use norito::json::{self, Value};

    use super::*;
    use crate::{
        asset::AssetDefinitionId,
        domain::DomainId,
        metadata::Metadata,
        offline::{
            AndroidIntegrityPolicy, AppleAppAttestProof, OfflineAllowanceCommitment,
            OfflineBalanceProof, OfflinePlatformProof, OfflinePlatformTokenSnapshot,
            OfflineSpendReceipt, OfflineWalletCertificate, OfflineWalletPolicy,
        },
    };

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

    fn sample_account(seed: u8, domain: &str) -> AccountId {
        let key = sample_public_key(seed);
        let domain_id = DomainId::from_str(domain).expect("domain");
        AccountId::new(domain_id, key)
    }

    fn sample_asset(domain: &str) -> crate::asset::AssetId {
        let definition =
            AssetDefinitionId::from_str(&format!("usd#{domain}")).expect("definition id");
        crate::asset::AssetId::new(definition, sample_account(0xD4, domain))
    }

    fn sample_commitment(tag: u8) -> OfflineAllowanceCommitment {
        OfflineAllowanceCommitment {
            asset: sample_asset("sbp"),
            amount: Numeric::new(1_000, 0),
            commitment: vec![tag; 32],
        }
    }

    fn sample_certificate() -> OfflineWalletCertificate {
        let controller = sample_account(0xA1, "sbp");
        let policy = OfflineWalletPolicy {
            max_balance: Numeric::new(5_000, 0),
            max_tx_value: Numeric::new(1_000, 0),
            expires_at_ms: 1_800_000_000,
        };
        OfflineWalletCertificate {
            controller,
            operator: sample_account(0xA1, "sbp"),
            allowance: sample_commitment(0x11),
            spend_public_key: sample_public_key(0xA1),
            attestation_report: vec![0xAA, 0xBB],
            issued_at_ms: 1_700_000_000,
            expires_at_ms: 1_800_000_000,
            policy,
            operator_signature: sample_signature(0x01),
            metadata: Metadata::default(),
            verdict_id: Some(Hash::new(b"verdict")),
            attestation_nonce: Some(Hash::new(b"nonce")),
            refresh_at_ms: Some(1_750_000_000),
        }
    }

    fn sample_transfer_record() -> OfflineTransferRecord {
        let certificate = sample_certificate();
        let receipt = OfflineSpendReceipt {
            tx_id: Hash::new(b"offline-tx"),
            from: sample_account(0xA1, "sbp"),
            to: sample_account(0xB2, "sbp"),
            asset: sample_asset("sbp"),
            amount: Numeric::new(250, 0),
            issued_at_ms: 1_700_000_250,
            invoice_id: "inv-001".into(),
            platform_proof: OfflinePlatformProof::AppleAppAttest(AppleAppAttestProof {
                key_id: BASE64_STANDARD.encode(b"AA_KEY"),
                counter: 42,
                assertion: vec![0xAA],
                challenge_hash: Hash::new(b"challenge"),
            }),
            platform_snapshot: None,
            sender_certificate_id: certificate.certificate_id(),
            sender_signature: sample_signature(0x55),
            build_claim: None,
        };
        let claimed_delta = receipt.amount.clone();
        let mut pos_snapshot = OfflineVerdictSnapshot::from_certificate(&certificate);
        pos_snapshot.certificate_id = receipt.sender_certificate_id;
        let balance_proof = OfflineBalanceProof {
            initial_commitment: sample_commitment(0x22),
            resulting_commitment: vec![0xCC; 32],
            claimed_delta,
            zk_proof: None,
        };
        let transfer = OfflineToOnlineTransfer {
            bundle_id: Hash::new(b"bundle"),
            receiver: receipt.to.clone(),
            deposit_account: receipt.to.clone(),
            receipts: vec![receipt],
            balance_proof,
            balance_proofs: None,
            aggregate_proof: None,
            attachments: None,
            platform_snapshot: Some(OfflinePlatformTokenSnapshot {
                policy: AndroidIntegrityPolicy::PlayIntegrity.as_str().into(),
                attestation_jws_b64: "token".into(),
            }),
        };
        OfflineTransferRecord {
            transfer,
            controller: sample_account(0xC1, "sbp"),
            status: OfflineTransferStatus::Settled,
            rejection_reason: None,
            recorded_at_ms: 1_700_000_500,
            recorded_at_height: 123,
            archived_at_height: None,
            history: Vec::new(),
            pos_verdict_snapshots: vec![pos_snapshot],
            verdict_snapshot: None,
            platform_snapshot: None,
        }
    }

    #[test]
    fn summary_derives_bundle_metadata() {
        let record = sample_transfer_record();
        let summary = OfflineTransferSummary::from(record);
        assert_eq!(summary.receipt_count, 1_u64);
        assert_eq!(summary.total_amount, Numeric::new(250, 0));
        let expected_asset = sample_asset("sbp").to_string();
        assert_eq!(summary.asset_id.as_deref(), Some(expected_asset.as_str()));
        assert!(summary.certificate_id_hex.is_some());
        assert!(summary.verdict_id_hex.is_some());
        assert!(summary.attestation_nonce_hex.is_some());
        assert_eq!(summary.status, OfflineTransferStatus::Settled);
    }

    #[test]
    fn helper_functions_surface_metadata() {
        let record = sample_transfer_record();
        assert!(transfer_certificate_id_hex(&record).is_some());
        assert!(transfer_verdict_hex(&record).is_some());
        assert!(transfer_attestation_nonce_hex(&record).is_some());
        assert!(transfer_certificate_expires_at_ms(&record).is_some());
        assert!(transfer_policy_expires_at_ms(&record).is_some());
        assert!(transfer_refresh_at_ms(&record).is_some());
    }

    #[test]
    fn summary_accumulates_multiple_receipts() {
        let mut record = sample_transfer_record();
        let mut extra = record
            .transfer
            .receipts
            .first()
            .cloned()
            .expect("sample receipt");
        extra.tx_id = Hash::new(b"offline-tx-2");
        extra.invoice_id = "inv-002".into();
        extra.amount = Numeric::new(750, 0);
        record.transfer.receipts.push(extra);

        let summary = OfflineTransferSummary::from(record);
        assert_eq!(summary.receipt_count, 2);
        assert_eq!(summary.total_amount, Numeric::new(1_000, 0));
    }

    #[test]
    fn platform_policy_uses_transfer_snapshot_when_present() {
        let record = sample_transfer_record();
        let expected = AndroidIntegrityPolicy::PlayIntegrity.as_str();

        assert_eq!(
            transfer_platform_policy_label(&record).as_deref(),
            Some(expected)
        );

        let summary = OfflineTransferSummary::from(record.clone());
        assert_eq!(summary.platform_policy_label().as_deref(), Some(expected));
    }

    #[test]
    fn platform_policy_prefers_record_snapshot_over_transfer_snapshot() {
        let mut record = sample_transfer_record();
        record.platform_snapshot = Some(OfflinePlatformTokenSnapshot {
            policy: AndroidIntegrityPolicy::HmsSafetyDetect.to_string(),
            attestation_jws_b64: "record-token".into(),
        });
        let expected = AndroidIntegrityPolicy::HmsSafetyDetect.as_str();

        assert_eq!(
            transfer_platform_policy_label(&record).as_deref(),
            Some(expected)
        );

        let summary = OfflineTransferSummary::from(record.clone());
        assert_eq!(summary.platform_policy_label().as_deref(), Some(expected));
    }

    #[test]
    fn platform_policy_absent_without_snapshots() {
        let mut record = sample_transfer_record();
        record.platform_snapshot = None;
        record.transfer.platform_snapshot = None;

        assert_eq!(transfer_platform_policy_label(&record), None);

        let summary = OfflineTransferSummary::from(record);
        assert_eq!(summary.platform_policy_label(), None);
    }

    #[test]
    fn summary_round_trips_through_norito_json() {
        let summary = OfflineTransferSummary::from(sample_transfer_record());
        let encoded = json::to_value(&summary).expect("serialize summary to JSON");
        let decoded: OfflineTransferSummary =
            json::from_value(encoded).expect("deserialize summary from JSON");

        assert_eq!(decoded, summary);
    }

    #[test]
    fn summary_defaults_optional_fields_when_missing_in_json() {
        let summary = OfflineTransferSummary::from(sample_transfer_record());
        let mut encoded = json::to_value(&summary).expect("serialize summary to JSON for mutation");
        let object = encoded
            .as_object_mut()
            .expect("summary must serialize to a JSON object");

        for key in [
            "asset_id",
            "history",
            "archived_at_height",
            "rejection_reason",
            "certificate_id_hex",
            "certificate_expires_at_ms",
            "policy_expires_at_ms",
            "refresh_at_ms",
            "verdict_id_hex",
            "attestation_nonce_hex",
            "verdict_snapshot",
            "platform_snapshot",
        ] {
            object.remove(key);
        }

        let decoded: OfflineTransferSummary =
            json::from_value(Value::Object(object.clone())).expect("decode summary defaults");

        assert!(decoded.asset_id.is_none());
        assert!(decoded.archived_at_height.is_none());
        assert!(decoded.rejection_reason.is_none());
        assert!(decoded.certificate_id_hex.is_none());
        assert!(decoded.certificate_expires_at_ms.is_none());
        assert!(decoded.policy_expires_at_ms.is_none());
        assert!(decoded.refresh_at_ms.is_none());
        assert!(decoded.verdict_id_hex.is_none());
        assert!(decoded.attestation_nonce_hex.is_none());
        assert!(decoded.verdict_snapshot.is_none());
        assert!(decoded.platform_snapshot.is_none());
        assert!(decoded.history.is_empty());
        assert_eq!(decoded.total_amount, summary.total_amount);
        assert_eq!(decoded.transfer.bundle_id, summary.transfer.bundle_id);
        assert_eq!(decoded.status, summary.status);
    }
}
