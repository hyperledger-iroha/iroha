//! Canonical receipt for a bounded offline operator-preseed session.

use std::{collections::BTreeSet, path::Path};

use norito::derive::{JsonDeserialize, JsonSerialize};

/// Sole first-release operator-preseed session receipt schema.
pub const OPERATOR_PRESEED_SESSION_RECEIPT_VERSION_V1: u16 = 1;
/// Exact acknowledgment emitted only after the helper observes release EOF.
pub const OPERATOR_PRESEED_SESSION_RELEASE_ACK_V1: &[u8] =
    b"{\"schema_version\":1,\"status\":\"released\"}\n";
/// Canonical per-store qualification directory installed by operator-preseed sessions.
pub const OPERATOR_PRESEED_STORE_RECEIPT_DIR_V1: &str = ".operator-preseed-v1";
/// Maximum simultaneously locked stores admitted by the first-release session.
pub const OPERATOR_PRESEED_SESSION_MAX_STORES_V1: usize = 4;
/// Maximum ordered artifacts admitted by one first-release session.
pub const OPERATOR_PRESEED_SESSION_MAX_ARTIFACTS_V1: usize = 256;

/// Exact artifact identity verified in every locked store of one preseed session.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct OperatorPreseedArtifactReceiptV1 {
    /// Canonical manifest digest (lowercase BLAKE3-256 hex).
    pub manifest_digest_blake3: String,
    /// Exact payload digest (lowercase BLAKE3-256 hex).
    pub payload_digest_blake3: String,
    /// Exact payload length.
    pub content_length: u64,
    /// Number of simultaneously locked stores that verified this artifact.
    pub store_count: u32,
}

/// Identity-bound offline store that participated in one preseed session.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct OperatorPreseedTargetReceiptV1 {
    /// Canonical validator account identity admitted for placement.
    pub validator_account_id: String,
    /// Exact peer identity bound to the validator account.
    pub peer_id: String,
    /// Canonical absolute root of that validator's offline store.
    pub store_root: String,
}

/// Ready barrier emitted after every artifact is exact in every offline store.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct OperatorPreseedSessionReceiptV1 {
    /// Receipt schema version.
    pub schema_version: u16,
    /// Exact barrier state; V1 admits only `ready`.
    pub status: String,
    /// Session behavior: `ingest` admits missing artifacts, `verify_only` never writes them.
    pub mode: String,
    /// Exact common capacity used to open every store.
    pub max_capacity_bytes: u64,
    /// Identity-bound offline stores, in canonical validator/peer/root order.
    pub targets: Vec<OperatorPreseedTargetReceiptV1>,
    /// Artifact receipts, in canonical manifest-digest order.
    pub artifacts: Vec<OperatorPreseedArtifactReceiptV1>,
}

impl OperatorPreseedSessionReceiptV1 {
    /// Validate the closed first-release receipt contract.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != OPERATOR_PRESEED_SESSION_RECEIPT_VERSION_V1 {
            return Err(format!(
                "unsupported operator-preseed receipt schema {}; expected {}",
                self.schema_version, OPERATOR_PRESEED_SESSION_RECEIPT_VERSION_V1
            ));
        }
        if self.status != "ready" {
            return Err("operator-preseed receipt status must be exactly `ready`".to_owned());
        }
        if self.mode != "ingest" && self.mode != "verify_only" {
            return Err(
                "operator-preseed receipt mode must be `ingest` or `verify_only`".to_owned(),
            );
        }
        if self.max_capacity_bytes == 0 {
            return Err("operator-preseed receipt capacity must be nonzero".to_owned());
        }
        if self.targets.is_empty() {
            return Err("operator-preseed receipt must name at least one target".to_owned());
        }
        if self.targets.len() > OPERATOR_PRESEED_SESSION_MAX_STORES_V1 {
            return Err(format!(
                "operator-preseed receipt exceeds the V1 limit of {OPERATOR_PRESEED_SESSION_MAX_STORES_V1} targets"
            ));
        }
        let mut roots = BTreeSet::new();
        let mut validators = BTreeSet::new();
        let mut peers = BTreeSet::new();
        for target in &self.targets {
            if target.validator_account_id.is_empty()
                || target.validator_account_id.trim() != target.validator_account_id
                || target.peer_id.is_empty()
                || target.peer_id.trim() != target.peer_id
            {
                return Err(
                    "operator-preseed receipt validator and peer identities must be exact nonblank strings"
                        .to_owned(),
                );
            }
            if !validators.insert(target.validator_account_id.as_str())
                || !peers.insert(target.peer_id.as_str())
            {
                return Err(
                    "operator-preseed receipt validator and peer identities must each be distinct"
                        .to_owned(),
                );
            }
            let root = &target.store_root;
            if root.is_empty() || root.trim() != root || !Path::new(root).is_absolute() {
                return Err(
                    "operator-preseed receipt store roots must be exact absolute paths".to_owned(),
                );
            }
            if !roots.insert(root.as_str()) {
                return Err("operator-preseed receipt store roots must be distinct".to_owned());
            }
        }
        if self.targets.windows(2).any(|pair| {
            let left = (
                pair[0].validator_account_id.as_str(),
                pair[0].peer_id.as_str(),
                pair[0].store_root.as_str(),
            );
            let right = (
                pair[1].validator_account_id.as_str(),
                pair[1].peer_id.as_str(),
                pair[1].store_root.as_str(),
            );
            left >= right
        }) {
            return Err(
                "operator-preseed receipt targets must be strictly ordered by validator, peer, then store root"
                    .to_owned(),
            );
        }
        for (index, target) in self.targets.iter().enumerate() {
            let root = Path::new(&target.store_root);
            if self.targets[..index].iter().any(|existing| {
                let existing = Path::new(&existing.store_root);
                root.starts_with(existing) || existing.starts_with(root)
            }) {
                return Err("operator-preseed receipt store roots must not overlap".to_owned());
            }
        }
        if self.artifacts.is_empty() {
            return Err("operator-preseed receipt must bind at least one artifact".to_owned());
        }
        if self.artifacts.len() > OPERATOR_PRESEED_SESSION_MAX_ARTIFACTS_V1 {
            return Err(format!(
                "operator-preseed receipt exceeds the V1 limit of {OPERATOR_PRESEED_SESSION_MAX_ARTIFACTS_V1} artifacts"
            ));
        }
        let store_count = u32::try_from(self.targets.len())
            .map_err(|_| "operator-preseed receipt store count exceeds u32".to_owned())?;
        let mut manifest_digests = BTreeSet::new();
        for artifact in &self.artifacts {
            validate_digest_hex(
                &artifact.manifest_digest_blake3,
                "operator-preseed manifest digest",
            )?;
            validate_digest_hex(
                &artifact.payload_digest_blake3,
                "operator-preseed payload digest",
            )?;
            if !manifest_digests.insert(&artifact.manifest_digest_blake3) {
                return Err(
                    "operator-preseed receipt artifact manifest digests must be distinct"
                        .to_owned(),
                );
            }
            if artifact.content_length == 0 {
                return Err("operator-preseed artifact content length must be nonzero".to_owned());
            }
            if artifact.store_count != store_count {
                return Err(format!(
                    "operator-preseed artifact store_count {} does not match {} receipt roots",
                    artifact.store_count, store_count
                ));
            }
        }
        if self.artifacts.windows(2).any(|pair| {
            pair[0].manifest_digest_blake3.as_str() >= pair[1].manifest_digest_blake3.as_str()
        }) {
            return Err(
                "operator-preseed receipt artifacts must be strictly ordered by manifest digest"
                    .to_owned(),
            );
        }
        Ok(())
    }
}

fn validate_digest_hex(value: &str, label: &str) -> Result<(), String> {
    let decoded = hex::decode(value).map_err(|_| format!("{label} must be lowercase hex"))?;
    if decoded.len() != 32 || hex::encode(decoded) != value {
        return Err(format!("{label} must be exactly 32 lowercase hex bytes"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_acknowledgement_is_one_exact_v1_json_line() {
        assert_eq!(
            OPERATOR_PRESEED_SESSION_RELEASE_ACK_V1,
            b"{\"schema_version\":1,\"status\":\"released\"}\n"
        );
        let line = OPERATOR_PRESEED_SESSION_RELEASE_ACK_V1
            .strip_suffix(b"\n")
            .expect("release acknowledgment newline");
        let value: norito::json::Value =
            norito::json::from_slice(line).expect("release acknowledgment JSON");
        assert_eq!(
            norito::json::to_json(&value).expect("canonical release acknowledgment"),
            "{\"schema_version\":1,\"status\":\"released\"}"
        );
    }

    #[test]
    fn receipt_validation_is_closed_and_exact() {
        let receipt = OperatorPreseedSessionReceiptV1 {
            schema_version: OPERATOR_PRESEED_SESSION_RECEIPT_VERSION_V1,
            status: "ready".to_owned(),
            mode: "ingest".to_owned(),
            max_capacity_bytes: 64,
            targets: vec![
                OperatorPreseedTargetReceiptV1 {
                    validator_account_id: "validator-a".to_owned(),
                    peer_id: "peer-a".to_owned(),
                    store_root: "/tmp/store-a".to_owned(),
                },
                OperatorPreseedTargetReceiptV1 {
                    validator_account_id: "validator-b".to_owned(),
                    peer_id: "peer-b".to_owned(),
                    store_root: "/tmp/store-b".to_owned(),
                },
            ],
            artifacts: vec![OperatorPreseedArtifactReceiptV1 {
                manifest_digest_blake3: hex::encode([0x11; 32]),
                payload_digest_blake3: hex::encode([0x22; 32]),
                content_length: 7,
                store_count: 2,
            }],
        };
        receipt.validate().expect("valid exact V1 receipt");
        let mut malformed = receipt;
        malformed.artifacts[0].store_count = 1;
        assert!(malformed.validate().is_err());
    }

    #[test]
    fn receipt_rejects_duplicate_manifest_identity() {
        let artifact = OperatorPreseedArtifactReceiptV1 {
            manifest_digest_blake3: hex::encode([0x11; 32]),
            payload_digest_blake3: hex::encode([0x22; 32]),
            content_length: 7,
            store_count: 1,
        };
        let receipt = OperatorPreseedSessionReceiptV1 {
            schema_version: OPERATOR_PRESEED_SESSION_RECEIPT_VERSION_V1,
            status: "ready".to_owned(),
            mode: "ingest".to_owned(),
            max_capacity_bytes: 64,
            targets: vec![OperatorPreseedTargetReceiptV1 {
                validator_account_id: "validator-a".to_owned(),
                peer_id: "peer-a".to_owned(),
                store_root: "/tmp/store-a".to_owned(),
            }],
            artifacts: vec![artifact.clone(), artifact],
        };
        assert!(
            receipt
                .validate()
                .expect_err("duplicate manifest identity must fail")
                .contains("must be distinct")
        );
    }

    #[test]
    fn receipt_rejects_reordered_targets() {
        let mut receipt = OperatorPreseedSessionReceiptV1 {
            schema_version: OPERATOR_PRESEED_SESSION_RECEIPT_VERSION_V1,
            status: "ready".to_owned(),
            mode: "ingest".to_owned(),
            max_capacity_bytes: 64,
            targets: vec![
                OperatorPreseedTargetReceiptV1 {
                    validator_account_id: "validator-a".to_owned(),
                    peer_id: "peer-a".to_owned(),
                    store_root: "/tmp/store-a".to_owned(),
                },
                OperatorPreseedTargetReceiptV1 {
                    validator_account_id: "validator-b".to_owned(),
                    peer_id: "peer-b".to_owned(),
                    store_root: "/tmp/store-b".to_owned(),
                },
            ],
            artifacts: vec![OperatorPreseedArtifactReceiptV1 {
                manifest_digest_blake3: hex::encode([0x11; 32]),
                payload_digest_blake3: hex::encode([0x22; 32]),
                content_length: 7,
                store_count: 2,
            }],
        };
        receipt.targets.reverse();
        assert!(
            receipt
                .validate()
                .expect_err("reordered targets must fail")
                .contains("strictly ordered")
        );
    }

    #[test]
    fn receipt_rejects_reordered_artifacts() {
        let mut receipt = OperatorPreseedSessionReceiptV1 {
            schema_version: OPERATOR_PRESEED_SESSION_RECEIPT_VERSION_V1,
            status: "ready".to_owned(),
            mode: "ingest".to_owned(),
            max_capacity_bytes: 64,
            targets: vec![OperatorPreseedTargetReceiptV1 {
                validator_account_id: "validator-a".to_owned(),
                peer_id: "peer-a".to_owned(),
                store_root: "/tmp/store-a".to_owned(),
            }],
            artifacts: vec![
                OperatorPreseedArtifactReceiptV1 {
                    manifest_digest_blake3: hex::encode([0x11; 32]),
                    payload_digest_blake3: hex::encode([0x22; 32]),
                    content_length: 7,
                    store_count: 1,
                },
                OperatorPreseedArtifactReceiptV1 {
                    manifest_digest_blake3: hex::encode([0x33; 32]),
                    payload_digest_blake3: hex::encode([0x44; 32]),
                    content_length: 9,
                    store_count: 1,
                },
            ],
        };
        receipt.artifacts.reverse();
        assert!(
            receipt
                .validate()
                .expect_err("reordered artifacts must fail")
                .contains("strictly ordered")
        );
    }
}
