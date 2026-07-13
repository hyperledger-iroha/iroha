//! Durable runtime helpers for governed pricing and authenticated hedging feeds.

use norito::derive::{NoritoDeserialize, NoritoSerialize};
use sorafs_manifest::{
    hedging::signed::{
        HedgingFeedTrustPolicyV1, MAX_SIGNED_HEDGING_FEED_LEDGER_BYTES, SignedHedgingError,
        SignedHedgingFeedLedgerV1,
    },
    pricing::signed::{
        GovernedPricingError, GovernedPricingSeriesV1, MAX_GOVERNED_PRICING_SERIES_BYTES,
        PricingTrustPolicyV1,
    },
};
use thiserror::Error;

/// Version of the local governed-pricing checkpoint wrapper.
pub(crate) const PRICING_RUNTIME_CHECKPOINT_VERSION_V1: u8 = 1;
/// Version of the local signed-hedging-feed checkpoint wrapper.
pub(crate) const HEDGING_RUNTIME_CHECKPOINT_VERSION_V1: u8 = 1;

// The wrapper contains only a version byte and an optional bounded manifest
// state machine. Keep a small, explicit allowance for Norito framing while the
// protocol-owned inner state remains subject to its stricter bound.
const CHECKPOINT_WRAPPER_OVERHEAD_BYTES: usize = 4 * 1024;

/// Maximum canonical bytes accepted for the local governed-pricing checkpoint.
pub(crate) const MAX_PRICING_RUNTIME_CHECKPOINT_BYTES: usize =
    MAX_GOVERNED_PRICING_SERIES_BYTES + CHECKPOINT_WRAPPER_OVERHEAD_BYTES;
/// Maximum canonical bytes accepted for the local signed-feed checkpoint.
pub(crate) const MAX_HEDGING_RUNTIME_CHECKPOINT_BYTES: usize =
    MAX_SIGNED_HEDGING_FEED_LEDGER_BYTES + CHECKPOINT_WRAPPER_OVERHEAD_BYTES;

/// Runtime admission, query, and durability failures for SoraFS economics.
#[derive(Debug, Error)]
pub enum EconomicsRuntimeError {
    /// No external pricing trust policy was configured for this node.
    #[error("governed pricing is not configured on this node")]
    PricingNotConfigured,
    /// No external hedging-feed trust policy was configured for this node.
    #[error("signed hedging feeds are not configured on this node")]
    HedgingNotConfigured,
    /// Governed pricing verification or replay failed.
    #[error(transparent)]
    Pricing(#[from] GovernedPricingError),
    /// Signed feed verification, freshness, or replay protection failed.
    #[error(transparent)]
    Hedging(#[from] SignedHedgingError),
    /// A runtime state lock was poisoned.
    #[error("SoraFS economics runtime state lock poisoned")]
    StateLockPoisoned,
    /// Durable checkpointing failed.
    #[error("SoraFS economics checkpoint failed: {0}")]
    Checkpoint(String),
}

/// Durable result of admitting one threshold-governed pricing manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GovernedPricingAdmissionOutcome {
    /// Domain-separated identifier of the admitted pricing manifest.
    pub pricing_id: [u8; 32],
    /// Unix second at which the schedule becomes effective.
    pub effective_from_unix: u64,
    /// Server admission clock durably bound to this transition.
    pub admitted_at_unix: u64,
    /// Number of retained admissions after the commit.
    pub admission_count: usize,
}

/// Durable result of admitting one authenticated hedging-feed sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedHedgingFeedAdmissionOutcome {
    /// Canonical governed feed identifier.
    pub feed_id: String,
    /// Canonical external source identifier.
    pub source: String,
    /// Observation time carried by the signed feed.
    pub observed_at_unix: u64,
    /// Highest durable admission clock after the commit.
    pub admitted_at_unix: u64,
    /// Number of retained per-feed high-water marks after the commit.
    pub feed_count: usize,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct PricingRuntimeCheckpointV1 {
    version: u8,
    series: Option<GovernedPricingSeriesV1>,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct HedgingRuntimeCheckpointV1 {
    version: u8,
    ledger: Option<SignedHedgingFeedLedgerV1>,
}

/// Encode one exact-canonical governed-pricing checkpoint.
pub(crate) fn encode_pricing_checkpoint(
    policy: Option<&PricingTrustPolicyV1>,
    series: Option<&GovernedPricingSeriesV1>,
) -> Result<Vec<u8>, EconomicsRuntimeError> {
    validate_pricing_configuration(policy, series)?;
    let checkpoint = PricingRuntimeCheckpointV1 {
        version: PRICING_RUNTIME_CHECKPOINT_VERSION_V1,
        series: series.cloned(),
    };
    encode_checkpoint_bounded(
        "governed pricing runtime",
        &checkpoint,
        MAX_PRICING_RUNTIME_CHECKPOINT_BYTES,
    )
}

/// Decode, canonicalize, and replay one governed-pricing checkpoint.
pub(crate) fn decode_pricing_checkpoint(
    bytes: &[u8],
    policy: Option<&PricingTrustPolicyV1>,
) -> Result<Option<GovernedPricingSeriesV1>, EconomicsRuntimeError> {
    let checkpoint: PricingRuntimeCheckpointV1 = crate::decode_local_checkpoint_canonical(
        bytes,
        u64::try_from(MAX_PRICING_RUNTIME_CHECKPOINT_BYTES).unwrap_or(u64::MAX),
        2_000_000,
    )
    .map_err(|error| {
        EconomicsRuntimeError::Checkpoint(format!(
            "decode governed pricing runtime checkpoint: {error}"
        ))
    })?;
    if checkpoint.version != PRICING_RUNTIME_CHECKPOINT_VERSION_V1 {
        return Err(EconomicsRuntimeError::Checkpoint(format!(
            "unsupported governed-pricing checkpoint version {}",
            checkpoint.version
        )));
    }
    validate_pricing_configuration(policy, checkpoint.series.as_ref())?;
    Ok(checkpoint.series)
}

/// Encode one exact-canonical signed-feed checkpoint.
pub(crate) fn encode_hedging_checkpoint(
    policy: Option<&HedgingFeedTrustPolicyV1>,
    ledger: Option<&SignedHedgingFeedLedgerV1>,
) -> Result<Vec<u8>, EconomicsRuntimeError> {
    validate_hedging_configuration(policy, ledger)?;
    let checkpoint = HedgingRuntimeCheckpointV1 {
        version: HEDGING_RUNTIME_CHECKPOINT_VERSION_V1,
        ledger: ledger.cloned(),
    };
    encode_checkpoint_bounded(
        "signed hedging-feed runtime",
        &checkpoint,
        MAX_HEDGING_RUNTIME_CHECKPOINT_BYTES,
    )
}

/// Decode, canonicalize, and replay one signed-feed checkpoint.
pub(crate) fn decode_hedging_checkpoint(
    bytes: &[u8],
    policy: Option<&HedgingFeedTrustPolicyV1>,
) -> Result<Option<SignedHedgingFeedLedgerV1>, EconomicsRuntimeError> {
    let checkpoint: HedgingRuntimeCheckpointV1 = crate::decode_local_checkpoint_canonical(
        bytes,
        u64::try_from(MAX_HEDGING_RUNTIME_CHECKPOINT_BYTES).unwrap_or(u64::MAX),
        1_000_000,
    )
    .map_err(|error| {
        EconomicsRuntimeError::Checkpoint(format!(
            "decode signed hedging-feed runtime checkpoint: {error}"
        ))
    })?;
    if checkpoint.version != HEDGING_RUNTIME_CHECKPOINT_VERSION_V1 {
        return Err(EconomicsRuntimeError::Checkpoint(format!(
            "unsupported signed-feed checkpoint version {}",
            checkpoint.version
        )));
    }
    validate_hedging_configuration(policy, checkpoint.ledger.as_ref())?;
    Ok(checkpoint.ledger)
}

fn validate_pricing_configuration(
    policy: Option<&PricingTrustPolicyV1>,
    series: Option<&GovernedPricingSeriesV1>,
) -> Result<(), EconomicsRuntimeError> {
    match (policy, series) {
        (Some(policy), Some(series)) => series.validate(policy).map_err(Into::into),
        (None, None) => Ok(()),
        (Some(_), None) => Err(EconomicsRuntimeError::Checkpoint(
            "configured pricing policy is missing its durable series".to_owned(),
        )),
        (None, Some(_)) => Err(EconomicsRuntimeError::Checkpoint(
            "pricing checkpoint contains state but no external policy is configured".to_owned(),
        )),
    }
}

fn validate_hedging_configuration(
    policy: Option<&HedgingFeedTrustPolicyV1>,
    ledger: Option<&SignedHedgingFeedLedgerV1>,
) -> Result<(), EconomicsRuntimeError> {
    match (policy, ledger) {
        (Some(policy), Some(ledger)) => ledger.validate(policy).map_err(Into::into),
        (None, None) => Ok(()),
        (Some(_), None) => Err(EconomicsRuntimeError::Checkpoint(
            "configured hedging policy is missing its durable feed ledger".to_owned(),
        )),
        (None, Some(_)) => Err(EconomicsRuntimeError::Checkpoint(
            "hedging checkpoint contains state but no external policy is configured".to_owned(),
        )),
    }
}

fn encode_checkpoint_bounded<T: norito::core::NoritoSerialize>(
    label: &'static str,
    checkpoint: &T,
    max_bytes: usize,
) -> Result<Vec<u8>, EconomicsRuntimeError> {
    let exact = checkpoint.encoded_len_exact().ok_or_else(|| {
        EconomicsRuntimeError::Checkpoint(format!(
            "{label} checkpoint does not expose an exact encoded length"
        ))
    })?;
    if exact > max_bytes {
        return Err(EconomicsRuntimeError::Checkpoint(format!(
            "{label} checkpoint length {exact} exceeds maximum {max_bytes}"
        )));
    }
    let bytes = norito::to_bytes(checkpoint).map_err(|error| {
        EconomicsRuntimeError::Checkpoint(format!("encode {label} checkpoint: {error}"))
    })?;
    // `encoded_len_exact` describes the archived payload; `to_bytes` adds the
    // canonical Norito header and alignment padding. Enforce both bounds, but
    // do not compare those two deliberately different lengths.
    if bytes.len() > max_bytes {
        return Err(EconomicsRuntimeError::Checkpoint(format!(
            "{label} checkpoint encoded length {} exceeds maximum {max_bytes}",
            bytes.len()
        )));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::Write as _,
        sync::{Arc, Barrier},
        thread,
    };

    use ed25519_dalek::{Signer as _, SigningKey};
    use iroha_crypto::numeric::Quantity;
    use sorafs_manifest::{
        deal::XorQuantity,
        hedging::signed::{
            HEDGING_FEED_BINDING_VERSION_V1, HEDGING_FEED_TRUST_POLICY_VERSION_V1,
            HEDGING_TRUSTED_SIGNER_VERSION_V1, HedgingFeedBindingV1, HedgingTrustedSignerV1,
            SIGNED_HEDGING_PRICE_FEED_VERSION_V1, SignedHedgingError, SignedHedgingPriceFeedV1,
        },
        hedging::{HEDGING_PRICE_FEED_VERSION_V1, HedgingFeedStatusV1, HedgingPriceFeedV1},
        pricing::signed::{
            GOVERNED_PRICING_MANIFEST_VERSION_V1, GovernedPricingManifestV1,
            PRICING_MANIFEST_SIGNATURE_VERSION_V1, PRICING_TRUST_POLICY_VERSION_V1,
            PRICING_TRUSTED_SIGNER_VERSION_V1, PricingManifestSignatureV1, PricingTrustedSignerV1,
            derive_pricing_id,
        },
        pricing::{
            BondPolicyV1, CreditPolicyV1, PRICING_MANIFEST_VERSION_V1, PricingManifestV1,
            PricingMicropaymentPolicyV1, PricingTierV1,
        },
    };
    use tempfile::TempDir;

    use super::*;
    use crate::{NodeHandle, NodeInitError, config::RuntimeRetentionPolicy, config::StorageConfig};

    const NOW: u64 = 1_800_000_000;

    fn xor(value: &str) -> XorQuantity {
        value.parse().expect("canonical XOR quantity")
    }

    fn quantity_from_micro(value: u64) -> Quantity {
        format!("{}.{:06}", value / 1_000_000, value % 1_000_000)
            .parse()
            .expect("canonical micro-unit quantity")
    }

    fn pricing_keys() -> [SigningKey; 2] {
        [
            SigningKey::from_bytes(&[31; 32]),
            SigningKey::from_bytes(&[32; 32]),
        ]
    }

    fn pricing_policy(policy_byte: u8) -> PricingTrustPolicyV1 {
        PricingTrustPolicyV1 {
            version: PRICING_TRUST_POLICY_VERSION_V1,
            policy_id: [policy_byte; 32],
            valid_from_unix: NOW - 1_000,
            valid_until_unix: NOW + 10_000,
            currency: "xor".into(),
            max_future_activation_secs: 600,
            min_signatures: 2,
            signers: pricing_keys()
                .iter()
                .enumerate()
                .map(|(index, key)| PricingTrustedSignerV1 {
                    version: PRICING_TRUSTED_SIGNER_VERSION_V1,
                    signer_id: format!("council-{}", index + 1),
                    public_key: key.verifying_key().to_bytes(),
                })
                .collect(),
            revoked_signer_ids: Vec::new(),
        }
    }

    fn pricing_manifest(effective_from_unix: u64) -> PricingManifestV1 {
        PricingManifestV1 {
            version: PRICING_MANIFEST_VERSION_V1,
            currency: "xor".into(),
            effective_from_unix,
            tiers: vec![PricingTierV1 {
                tier_id: "hot".into(),
                storage_price_per_gib_hour: xor("0.5"),
                egress_price_per_gib: xor("0.05"),
                min_collateral_ratio_bps: Some(15_000),
                notes: None,
            }],
            credit_policy: CreditPolicyV1 {
                settlement_window_secs: 86_400,
                auto_top_up_threshold_bps: 2_000,
            },
            bond_policy: BondPolicyV1 {
                collateral_ratio_bps: 30_000,
                new_provider_grace_days: 30,
            },
            micropayment_policy: Some(PricingMicropaymentPolicyV1 {
                payout_probability_bps: 100,
                max_voucher_value: xor("5"),
                notes: None,
            }),
        }
    }

    fn signed_pricing(
        policy: &PricingTrustPolicyV1,
        effective_from_unix: u64,
        previous_pricing_id: Option<[u8; 32]>,
    ) -> GovernedPricingManifestV1 {
        let manifest = pricing_manifest(effective_from_unix);
        let mut governed = GovernedPricingManifestV1 {
            version: GOVERNED_PRICING_MANIFEST_VERSION_V1,
            policy_digest: policy.canonical_digest().expect("pricing policy digest"),
            pricing_id: derive_pricing_id(&manifest, previous_pricing_id).expect("pricing id"),
            previous_pricing_id,
            manifest,
            signatures: Vec::new(),
        };
        let digest = governed.signing_digest().expect("pricing signing digest");
        governed.signatures = pricing_keys()
            .iter()
            .enumerate()
            .map(|(index, key)| PricingManifestSignatureV1 {
                version: PRICING_MANIFEST_SIGNATURE_VERSION_V1,
                signer_id: format!("council-{}", index + 1),
                signature: key.sign(&digest).to_bytes(),
            })
            .collect();
        governed
    }

    fn hedging_key() -> SigningKey {
        SigningKey::from_bytes(&[11; 32])
    }

    fn hedging_policy(policy_byte: u8) -> HedgingFeedTrustPolicyV1 {
        HedgingFeedTrustPolicyV1 {
            version: HEDGING_FEED_TRUST_POLICY_VERSION_V1,
            policy_id: [policy_byte; 32],
            valid_from_unix: NOW - 1_000,
            valid_until_unix: NOW + 10_000,
            max_sample_age_secs: 300,
            max_future_skew_secs: 30,
            signers: vec![HedgingTrustedSignerV1 {
                version: HEDGING_TRUSTED_SIGNER_VERSION_V1,
                signer_id: "collector-1".into(),
                public_key: hedging_key().verifying_key().to_bytes(),
                authorized_feeds: vec![HedgingFeedBindingV1 {
                    version: HEDGING_FEED_BINDING_VERSION_V1,
                    feed_id: "primary".into(),
                    source: "primary-source".into(),
                }],
            }],
            revoked_signer_ids: Vec::new(),
        }
    }

    fn signed_feed(
        policy: &HedgingFeedTrustPolicyV1,
        observed_at_unix: u64,
        evidence_byte: u8,
    ) -> SignedHedgingPriceFeedV1 {
        let mut envelope = SignedHedgingPriceFeedV1 {
            version: SIGNED_HEDGING_PRICE_FEED_VERSION_V1,
            policy_digest: policy.canonical_digest().expect("hedging policy digest"),
            feed: HedgingPriceFeedV1 {
                version: HEDGING_PRICE_FEED_VERSION_V1,
                feed_id: "primary".into(),
                source: "primary-source".into(),
                observed_at_unix,
                xor_usd_price: quantity_from_micro(2_000_000 + u64::from(evidence_byte)),
                weight_bps: 10_000,
                evidence_digest: [evidence_byte; 32],
                status: HedgingFeedStatusV1::Ok,
            },
            signer_id: "collector-1".into(),
            signature: [0; ed25519_dalek::SIGNATURE_LENGTH],
        };
        envelope.signature = hedging_key()
            .sign(&envelope.signing_digest().expect("feed signing digest"))
            .to_bytes();
        envelope
    }

    fn write_policy(path: &std::path::Path, bytes: &[u8]) {
        crate::write_local_checkpoint_atomic(path, bytes).expect("write canonical policy");
    }

    fn configured_node_with_retention(
        retention: RuntimeRetentionPolicy,
    ) -> (
        NodeHandle,
        StorageConfig,
        TempDir,
        PricingTrustPolicyV1,
        HedgingFeedTrustPolicyV1,
    ) {
        let temp_dir = tempfile::tempdir().expect("create economics temp dir");
        let root = temp_dir.path().canonicalize().expect("canonical temp dir");
        let pricing = pricing_policy(0xB5);
        let hedging = hedging_policy(0xA5);
        let pricing_path = root.join("pricing-policy.to");
        let hedging_path = root.join("hedging-policy.to");
        write_policy(
            &pricing_path,
            &pricing.canonical_bytes().expect("encode pricing policy"),
        );
        write_policy(
            &hedging_path,
            &hedging.canonical_bytes().expect("encode hedging policy"),
        );
        let config = StorageConfig::builder()
            .enabled(true)
            .data_dir(root.join("storage"))
            .runtime_retention(retention)
            .pricing_trust_policy_path(Some(pricing_path))
            .hedging_feed_trust_policy_path(Some(hedging_path))
            .build();
        let node = NodeHandle::try_new(config.clone()).expect("initialize economics runtime");
        (node, config, temp_dir, pricing, hedging)
    }

    fn configured_node() -> (
        NodeHandle,
        StorageConfig,
        TempDir,
        PricingTrustPolicyV1,
        HedgingFeedTrustPolicyV1,
    ) {
        configured_node_with_retention(RuntimeRetentionPolicy::new(16, 64, 64 * 1024 * 1024))
    }

    #[test]
    fn durable_pricing_and_feed_state_survive_restart_and_drive_queries() {
        let (node, config, temp_dir, pricing, hedging) = configured_node();
        let governed = signed_pricing(&pricing, NOW + 100, None);
        let outcome = node
            .admit_governed_pricing_manifest(
                &governed.canonical_bytes().expect("pricing envelope"),
                NOW,
            )
            .expect("admit pricing");
        assert_eq!(outcome.pricing_id, governed.pricing_id);
        assert_eq!(outcome.admitted_at_unix, NOW);
        assert!(
            node.active_governed_pricing(NOW + 99)
                .expect("query before activation")
                .is_none()
        );
        assert_eq!(
            node.active_governed_pricing(NOW + 100)
                .expect("query active pricing"),
            Some(governed.clone())
        );

        let feed = signed_feed(&hedging, NOW - 5, 1);
        node.admit_signed_hedging_feed(&feed.canonical_bytes().expect("signed feed envelope"), NOW)
            .expect("admit signed feed");
        assert_eq!(
            node.hedging_max_sample_age_secs()
                .expect("configured hedging maximum age"),
            hedging.max_sample_age_secs
        );
        let decision = node
            .derive_latest_hedging_reference_price(NOW, NOW, 60, 500)
            .expect("derive governed reference price");
        assert_eq!(decision.decision.feeds.len(), 1);
        assert_eq!(decision.decision.xor_usd_price, feed.feed.xor_usd_price);

        drop(node);
        let restored = NodeHandle::try_new(config).expect("restore economics checkpoints");
        assert_eq!(
            restored
                .governed_pricing_series()
                .expect("restored pricing")
                .len(),
            1
        );
        assert_eq!(
            restored
                .latest_signed_hedging_feeds()
                .expect("restored feeds"),
            vec![feed]
        );
        drop(restored);
        drop(temp_dir);
    }

    #[test]
    fn replays_rollbacks_policy_substitution_and_clock_skew_are_atomic() {
        let (node, _config, _temp_dir, pricing, hedging) = configured_node();
        let governed = signed_pricing(&pricing, NOW + 100, None);
        let governed_bytes = governed.canonical_bytes().expect("pricing envelope");
        node.admit_governed_pricing_manifest(&governed_bytes, NOW)
            .expect("first pricing admission");
        assert!(
            node.admit_governed_pricing_manifest(&governed_bytes, NOW + 1)
                .is_err()
        );
        let substituted =
            signed_pricing(&pricing_policy(0xC5), NOW + 200, Some(governed.pricing_id));
        assert!(
            node.admit_governed_pricing_manifest(
                &substituted.canonical_bytes().expect("substituted pricing"),
                NOW + 1,
            )
            .is_err()
        );
        let successor = signed_pricing(&pricing, NOW + 200, Some(governed.pricing_id));
        assert!(
            node.admit_governed_pricing_manifest(
                &successor.canonical_bytes().expect("pricing successor"),
                NOW - 1,
            )
            .is_err()
        );
        assert_eq!(
            node.governed_pricing_series()
                .expect("pricing series")
                .len(),
            1
        );

        let first = signed_feed(&hedging, NOW - 5, 1);
        let first_bytes = first.canonical_bytes().expect("feed envelope");
        node.admit_signed_hedging_feed(&first_bytes, NOW)
            .expect("first feed admission");
        assert!(matches!(
            node.admit_signed_hedging_feed(&first_bytes, NOW + 1),
            Err(EconomicsRuntimeError::Hedging(
                SignedHedgingError::FeedReplay { .. }
            ))
        ));
        let rollback = signed_feed(&hedging, NOW - 6, 2);
        assert!(matches!(
            node.admit_signed_hedging_feed(
                &rollback.canonical_bytes().expect("rollback feed"),
                NOW + 1,
            ),
            Err(EconomicsRuntimeError::Hedging(
                SignedHedgingError::FeedObservationRollback { .. }
            ))
        ));
        let admission_clock_rollback = signed_feed(&hedging, NOW + 1, 5);
        assert!(matches!(
            node.admit_signed_hedging_feed(
                &admission_clock_rollback
                    .canonical_bytes()
                    .expect("admission-clock rollback feed"),
                NOW - 1,
            ),
            Err(EconomicsRuntimeError::Hedging(
                SignedHedgingError::FeedAdmissionTimeRollback { .. }
            ))
        ));
        let future = signed_feed(&hedging, NOW + 32, 3);
        assert!(matches!(
            node.admit_signed_hedging_feed(
                &future.canonical_bytes().expect("future feed"),
                NOW + 1,
            ),
            Err(EconomicsRuntimeError::Hedging(
                SignedHedgingError::FeedFromFuture
            ))
        ));
        let stale = signed_feed(&hedging, NOW - 301, 6);
        assert!(matches!(
            node.admit_signed_hedging_feed(&stale.canonical_bytes().expect("stale feed"), NOW + 1,),
            Err(EconomicsRuntimeError::Hedging(
                SignedHedgingError::FeedTooOld
            ))
        ));
        let substituted_feed = signed_feed(&hedging_policy(0xB5), NOW + 1, 4);
        assert!(matches!(
            node.admit_signed_hedging_feed(
                &substituted_feed
                    .canonical_bytes()
                    .expect("substituted feed"),
                NOW + 1,
            ),
            Err(EconomicsRuntimeError::Hedging(
                SignedHedgingError::PolicyDigestMismatch
            ))
        ));
        assert_eq!(
            node.signed_hedging_feed_ledger()
                .expect("feed ledger")
                .len(),
            1
        );
    }

    #[test]
    fn malformed_trailing_and_archive_bomb_envelopes_fail_without_mutation() {
        let (node, _config, _temp_dir, pricing, hedging) = configured_node();
        let mut pricing_bytes = signed_pricing(&pricing, NOW + 100, None)
            .canonical_bytes()
            .expect("pricing envelope");
        pricing_bytes.push(0);
        assert!(
            node.admit_governed_pricing_manifest(&pricing_bytes, NOW)
                .is_err()
        );
        assert!(
            node.governed_pricing_series()
                .expect("pricing series")
                .is_empty()
        );

        let mut feed_bytes = signed_feed(&hedging, NOW, 1)
            .canonical_bytes()
            .expect("feed envelope");
        feed_bytes.push(0);
        assert!(node.admit_signed_hedging_feed(&feed_bytes, NOW).is_err());
        assert!(
            node.signed_hedging_feed_ledger()
                .expect("feed ledger")
                .is_empty()
        );

        let pricing_bomb =
            vec![0_u8; sorafs_manifest::pricing::signed::MAX_GOVERNED_PRICING_MANIFEST_BYTES + 1];
        let feed_bomb =
            vec![0_u8; sorafs_manifest::hedging::signed::MAX_SIGNED_HEDGING_FEED_BYTES + 1];
        assert!(
            node.admit_governed_pricing_manifest(&pricing_bomb, NOW)
                .is_err()
        );
        assert!(node.admit_signed_hedging_feed(&feed_bomb, NOW).is_err());
    }

    #[test]
    fn economics_checkpoint_persistence_failures_roll_back_memory() {
        let (node, _config, _temp_dir, pricing, hedging) = configured_node();
        let path = node
            .pricing_checkpoint_path
            .clone()
            .expect("pricing checkpoint path");
        fs::remove_file(&path).expect("remove pricing checkpoint");
        fs::create_dir(&path).expect("replace pricing checkpoint with directory");
        let governed = signed_pricing(&pricing, NOW + 100, None);
        assert!(matches!(
            node.admit_governed_pricing_manifest(
                &governed.canonical_bytes().expect("pricing envelope"),
                NOW,
            ),
            Err(EconomicsRuntimeError::Checkpoint(_))
        ));
        assert!(
            node.governed_pricing_series()
                .expect("rolled-back pricing series")
                .is_empty()
        );
        assert!(node.durability_failure_reason().is_none());

        let hedging_path = node
            .hedging_checkpoint_path
            .clone()
            .expect("hedging checkpoint path");
        fs::remove_file(&hedging_path).expect("remove hedging checkpoint");
        fs::create_dir(&hedging_path).expect("replace hedging checkpoint with directory");
        let feed = signed_feed(&hedging, NOW, 1);
        assert!(matches!(
            node.admit_signed_hedging_feed(
                &feed.canonical_bytes().expect("signed feed envelope"),
                NOW,
            ),
            Err(EconomicsRuntimeError::Checkpoint(_))
        ));
        assert!(
            node.signed_hedging_feed_ledger()
                .expect("rolled-back feed ledger")
                .is_empty()
        );
        assert!(node.durability_failure_reason().is_none());
    }

    #[test]
    fn concurrent_pricing_replay_has_one_durable_winner() {
        let (node, _config, _temp_dir, pricing, _hedging) = configured_node();
        let bytes = Arc::new(
            signed_pricing(&pricing, NOW + 100, None)
                .canonical_bytes()
                .expect("pricing envelope"),
        );
        let node = Arc::new(node);
        let barrier = Arc::new(Barrier::new(8));
        let joins = (0..8)
            .map(|_| {
                let node = Arc::clone(&node);
                let bytes = Arc::clone(&bytes);
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    node.admit_governed_pricing_manifest(bytes.as_slice(), NOW)
                        .is_ok()
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            joins
                .into_iter()
                .map(|join| join.join().expect("join admission worker"))
                .filter(|succeeded| *succeeded)
                .count(),
            1
        );
        assert_eq!(
            node.governed_pricing_series()
                .expect("pricing series")
                .len(),
            1
        );
    }

    #[test]
    fn concurrent_hedging_replay_has_one_durable_winner() {
        let (node, _config, _temp_dir, _pricing, hedging) = configured_node();
        let bytes = Arc::new(
            signed_feed(&hedging, NOW, 1)
                .canonical_bytes()
                .expect("signed feed envelope"),
        );
        let node = Arc::new(node);
        let barrier = Arc::new(Barrier::new(8));
        let joins = (0..8)
            .map(|_| {
                let node = Arc::clone(&node);
                let bytes = Arc::clone(&bytes);
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    node.admit_signed_hedging_feed(bytes.as_slice(), NOW)
                        .is_ok()
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            joins
                .into_iter()
                .map(|join| join.join().expect("join feed admission worker"))
                .filter(|succeeded| *succeeded)
                .count(),
            1
        );
        assert_eq!(
            node.signed_hedging_feed_ledger()
                .expect("signed feed ledger")
                .len(),
            1
        );
    }

    #[test]
    fn pricing_checkpoint_trailing_bytes_fail_restart() {
        let (node, config, _temp_dir, pricing, _hedging) = configured_node();
        let governed = signed_pricing(&pricing, NOW + 100, None);
        node.admit_governed_pricing_manifest(
            &governed.canonical_bytes().expect("pricing envelope"),
            NOW,
        )
        .expect("admit pricing");
        let checkpoint = node
            .pricing_checkpoint_path
            .clone()
            .expect("pricing checkpoint path");
        drop(node);
        fs::OpenOptions::new()
            .append(true)
            .open(&checkpoint)
            .expect("open checkpoint")
            .write_all(&[0])
            .expect("append trailing byte");
        assert!(matches!(
            NodeHandle::try_new(config),
            Err(NodeInitError::Checkpoint {
                component: "governed pricing runtime",
                ..
            })
        ));
    }

    #[test]
    fn checkpoint_replay_rejects_external_policy_substitution() {
        let (node, config, _temp_dir, pricing, _hedging) = configured_node();
        let governed = signed_pricing(&pricing, NOW + 100, None);
        node.admit_governed_pricing_manifest(
            &governed.canonical_bytes().expect("pricing envelope"),
            NOW,
        )
        .expect("admit pricing");
        drop(node);
        let policy_path = config
            .pricing_trust_policy_path()
            .expect("pricing policy path");
        write_policy(
            policy_path,
            &pricing_policy(0xC5)
                .canonical_bytes()
                .expect("substituted policy bytes"),
        );
        assert!(matches!(
            NodeHandle::try_new(config),
            Err(NodeInitError::Checkpoint {
                component: "governed pricing runtime",
                ..
            })
        ));
    }

    #[test]
    fn hedging_checkpoint_replay_rejects_external_policy_substitution() {
        let (node, config, _temp_dir, _pricing, hedging) = configured_node();
        let feed = signed_feed(&hedging, NOW, 1);
        node.admit_signed_hedging_feed(&feed.canonical_bytes().expect("signed feed envelope"), NOW)
            .expect("admit signed feed");
        drop(node);
        let policy_path = config
            .hedging_feed_trust_policy_path()
            .expect("hedging policy path");
        write_policy(
            policy_path,
            &hedging_policy(0xC5)
                .canonical_bytes()
                .expect("substituted hedging policy bytes"),
        );
        assert!(matches!(
            NodeHandle::try_new(config),
            Err(NodeInitError::Checkpoint {
                component: "signed hedging-feed runtime",
                ..
            })
        ));
    }

    #[test]
    fn policy_files_reject_trailing_bytes_and_oversized_archives() {
        let (node, config, _temp_dir, pricing, _hedging) = configured_node();
        drop(node);
        let pricing_path = config
            .pricing_trust_policy_path()
            .expect("pricing policy path");
        let canonical_pricing = pricing.canonical_bytes().expect("pricing policy bytes");
        let mut trailing = canonical_pricing.clone();
        trailing.push(0);
        write_policy(pricing_path, &trailing);
        assert!(matches!(
            NodeHandle::try_new(config.clone()),
            Err(NodeInitError::PricingTrustPolicy { .. })
        ));

        write_policy(pricing_path, &canonical_pricing);
        let hedging_path = config
            .hedging_feed_trust_policy_path()
            .expect("hedging policy path");
        let oversized =
            vec![0_u8; sorafs_manifest::hedging::signed::MAX_HEDGING_TRUST_POLICY_BYTES + 1];
        write_policy(hedging_path, &oversized);
        assert!(matches!(
            NodeHandle::try_new(config),
            Err(NodeInitError::HedgingFeedTrustPolicy { .. })
        ));
    }

    #[test]
    fn auxiliary_checkpoint_rejects_trailing_bytes_and_sequence_bombs() {
        let retention = RuntimeRetentionPolicy::new(2, 2, 2 * 1024 * 1024);
        let (node, config, _temp_dir, _pricing, _hedging) =
            configured_node_with_retention(retention);
        let auxiliary_path = node
            .auxiliary_runtime_checkpoint_path
            .clone()
            .expect("auxiliary checkpoint path");
        drop(node);
        let original = fs::read(&auxiliary_path).expect("read auxiliary checkpoint");
        let mut trailing = original.clone();
        trailing.push(0);
        crate::write_local_checkpoint_atomic(&auxiliary_path, &trailing)
            .expect("write trailing checkpoint");
        assert!(matches!(
            NodeHandle::try_new(config.clone()),
            Err(NodeInitError::Checkpoint {
                component: "auxiliary runtime",
                ..
            })
        ));

        crate::write_local_checkpoint_atomic(&auxiliary_path, &original)
            .expect("restore auxiliary checkpoint");
        let mut checkpoint: crate::AuxiliaryRuntimeCheckpointV1 =
            crate::decode_local_checkpoint_canonical(
                &original,
                retention.checkpoint_max_bytes(),
                1_024,
            )
            .expect("decode auxiliary checkpoint for adversarial fixture");
        checkpoint.por_history = (0..7)
            .map(|index| crate::PorHistoryCheckpointEntryV1 {
                manifest_digest: [index; 32],
                provider_id: [index.saturating_add(1); 32],
                last_success_unix: None,
                last_failure_unix: None,
                failures_total: 0,
                consecutive_failures: 0,
                last_slash_unix: None,
            })
            .collect();
        let bomb = norito::to_bytes(&checkpoint).expect("encode sequence bomb");
        crate::write_local_checkpoint_atomic(&auxiliary_path, &bomb).expect("write sequence bomb");
        assert!(matches!(
            NodeHandle::try_new(config),
            Err(NodeInitError::Checkpoint {
                component: "auxiliary runtime",
                ..
            })
        ));
    }
}
