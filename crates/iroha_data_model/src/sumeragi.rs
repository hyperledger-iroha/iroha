//! Node-facing Sumeragi status (`specs/sumeragi.md` §12.1 `status()`).
//!
//! [`SumeragiStatus`] is the Norito/JSON projection of the consensus core's read-only
//! diagnostics (`iroha_sumeragi::api::CoreStatus`) that the node serves on its status endpoint.
//! The driver fills it from `Core::status()`; hashes become 32-byte arrays and core keys the
//! validators' consensus [`PublicKey`]s. The data model does not depend on the core crate.

/// Sole first-release native consensus wire version, including mandatory epoch context.
pub const PROTOCOL_VERSION: u16 = 1;

/// Canonical validator generations, scheduling epochs, and frozen boundary results.
pub mod epoch;

/// Canonical native finality source frames and explicit bounded decoding.
pub mod finality;

use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_crypto::PublicKey;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

/// Why a Sumeragi instance halted (§12.5); mirrors `iroha_sumeragi::api::HaltReason`.
///
/// A halted instance only keeps serving blocks; the operator must restart the node after
/// repairing its storage.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(
    tag = "reason",
    content = "details",
    rename_all = "snake_case",
    deny_unknown_fields
)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sumeragi::SumeragiHaltReason")]
pub enum SumeragiHaltReason {
    /// The local safety record is corrupt or belongs to another instance or key (§7.4 R1).
    SafetyRecordCorrupt,
    /// The safety record or a committed body contradicts the block store (local storage
    /// corruption).
    SafetyRecordInconsistent,
    /// A valid `CommitQC` conflicts with a committed block at this height (§7.6).
    SafetyViolation(u64),
    /// Local apply disagrees with a certified result at this height (O3).
    ApplyDiverged(u64),
    /// Original publication was consumed, possibly visible, or lost; this node needs recovery.
    PublicationRecoveryRequired(u64),
    /// The driver violated its contract with the core (§6.13).
    DriverAnomaly,
}

impl SumeragiHaltReason {
    /// The height named by the reason, if it names one.
    #[must_use]
    pub const fn height(self) -> Option<u64> {
        match self {
            Self::SafetyViolation(height)
            | Self::ApplyDiverged(height)
            | Self::PublicationRecoveryRequired(height) => Some(height),
            Self::SafetyRecordCorrupt | Self::SafetyRecordInconsistent | Self::DriverAnomaly => {
                None
            }
        }
    }
}

/// Memory footprint counters of a core (§8.4); mirrors `iroha_sumeragi::api::Footprint` with
/// every counter widened to `u64`.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sumeragi::SumeragiFootprint")]
pub struct SumeragiFootprint {
    /// Votes in all pools.
    pub votes: u64,
    /// Stored timeout votes.
    pub timeouts: u64,
    /// Block bodies in memory.
    pub blocks: u64,
    /// Execution entries.
    pub exec_entries: u64,
    /// Outstanding wants (bodies being fetched).
    pub wants: u64,
    /// Committed blocks waiting to be applied.
    pub pending_apply: u64,
    /// Buffered sync entries.
    pub sync_entries: u64,
    /// Bytes of buffered sync entries.
    pub sync_bytes: u64,
    /// Peer table entries.
    pub peers: u64,
    /// Recent committed headers.
    pub recent_headers: u64,
    /// Height configurations held.
    pub configs: u64,
    /// Verified-certificate cache entries.
    pub cert_cache: u64,
    /// Evidence deduplication keys.
    pub evidence_keys: u64,
    /// Probe table entries (§7.4 R2).
    pub probe: u64,
}

/// Status of one Sumeragi instance as served by the node (§12.1 `status()`); mirrors
/// `iroha_sumeragi::api::CoreStatus`.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sumeragi::SumeragiStatus")]
pub struct SumeragiStatus {
    /// Exact native wire revision of the running owner.
    pub protocol_version: u16,
    /// Signed-genesis native consensus configuration fingerprint, excluding local resources.
    pub config_fingerprint: iroha_crypto::Hash,
    /// Same-applied-cut native beacon readiness, absent while its observation is unavailable.
    #[norito(required)]
    pub beacon_horizon: Option<BeaconHorizonStatusV1>,
    /// Instance id `I` (§1.8).
    #[norito(
        with = "crate::json_helpers::fixed_bytes_hex",
        bounded_with = "crate::json_helpers::fixed_bytes_hex::serialize_bounded"
    )]
    pub instance: [u8; 32],
    /// Height of the current round.
    pub height: u64,
    /// View of the current round.
    pub view: u64,
    /// Routing stage of the round (0, 1 or 2, §5.2).
    pub stage: u8,
    /// Leader of the current round (`None` while awaiting the next configuration).
    #[norito(required)]
    pub leader: Option<PublicKey>,
    /// Proxy tail of the current round (`None` while awaiting the next configuration).
    #[norito(required)]
    pub proxy_tail: Option<PublicKey>,
    /// View of the lock (`high_pqc`) at the current height, if any.
    #[norito(required)]
    pub high_qc_view: Option<u64>,
    /// Pacemaker level of the current view (§9.1).
    pub level: u32,
    /// Start level of the current height (§9.2).
    pub start_level: u32,
    /// Current retransmission interval `t_retx` in milliseconds.
    pub t_retx_ms: u64,
    /// Committed tip height.
    pub committed_height: u64,
    /// Highest applied height.
    pub applied_height: u64,
    /// Committed, but waiting for the next height's configuration.
    pub awaiting: bool,
    /// Key signing at this height (`None`: the node is an observer here).
    #[norito(required)]
    pub signer: Option<PublicKey>,
    /// Some key is unanchored (§7.4 R2): the node probes and signs nothing with it.
    pub unanchored: bool,
    /// The node is not a signing member at its height; for liveness it counts as faulty there.
    pub abstaining: bool,
    /// Halt reason, if the instance halted.
    #[norito(required)]
    pub halted: Option<SumeragiHaltReason>,
    /// Memory footprint counters.
    pub footprint: SumeragiFootprint,
}

/// Native beacon readiness facts from the sole production pulse owner.
/// This local observation confers no finality or signing authority.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    norito::Encode,
    norito::Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::sumeragi::BeaconHorizonStatusV1")]
pub struct BeaconHorizonStatusV1 {
    /// Frozen scheduling interval; zero for permissioned consensus.
    pub epoch_length_blocks: u64,
    /// Earliest mandatory pulse within the currently authenticated epoch.
    #[norito(required)]
    pub next_required_pulse_height: Option<u64>,
    /// Actual committed active session pointer, if installed.
    #[norito(required)]
    pub active_session_id: Option<[u8; 32]>,
    /// The authenticated active transcript covers the required pulse.
    pub session_covers_next_pulse: bool,
    /// Actual current validator custody probe succeeded; always false for observers.
    pub local_provider_ready: bool,
}

impl SumeragiStatus {
    /// Whether the instance halted and the node must be restarted after repair.
    #[must_use]
    pub const fn is_halted(&self) -> bool {
        self.halted.is_some()
    }

    /// Whether the node signs at its current height (a validator key is configured, anchored
    /// and a member of the height's committee).
    #[must_use]
    pub const fn is_signing(&self) -> bool {
        self.signer.is_some() && !self.abstaining && !self.unanchored
    }

    /// Committed heights not yet durably applied.
    #[must_use]
    pub const fn apply_lag(&self) -> u64 {
        self.committed_height.saturating_sub(self.applied_height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::KeyPair;
    use norito::codec::DecodeAll as _;

    fn sample_status() -> SumeragiStatus {
        let leader = KeyPair::from_seed(vec![1; 32], iroha_crypto::Algorithm::Ed25519)
            .public_key()
            .clone();
        let tail = KeyPair::from_seed(vec![2; 32], iroha_crypto::Algorithm::Ed25519)
            .public_key()
            .clone();
        SumeragiStatus {
            protocol_version: PROTOCOL_VERSION,
            config_fingerprint: iroha_crypto::Hash::new(b"native status configuration fixture"),
            beacon_horizon: None,
            instance: [7; 32],
            height: 12,
            view: 1,
            stage: 2,
            leader: Some(leader.clone()),
            proxy_tail: Some(tail),
            high_qc_view: Some(0),
            level: 1,
            start_level: 0,
            t_retx_ms: 250,
            committed_height: 11,
            applied_height: 10,
            awaiting: false,
            signer: Some(leader),
            unanchored: false,
            abstaining: false,
            halted: Some(SumeragiHaltReason::ApplyDiverged(9)),
            footprint: SumeragiFootprint {
                votes: 3,
                timeouts: 1,
                blocks: 2,
                exec_entries: 2,
                wants: 0,
                pending_apply: 1,
                sync_entries: 0,
                sync_bytes: 0,
                peers: 4,
                recent_headers: 10,
                configs: 3,
                cert_cache: 2,
                evidence_keys: 0,
                probe: 0,
            },
        }
    }

    #[test]
    fn status_codec_round_trip() {
        let status = sample_status();
        let bytes = status.encode();
        let decoded = SumeragiStatus::decode_all(&mut bytes.as_slice()).expect("decode");
        assert_eq!(decoded, status);
        let framed = norito::to_bytes(&status).expect("framed encode");
        let decoded: SumeragiStatus = norito::decode_from_bytes(&framed).expect("framed decode");
        assert_eq!(decoded, status);
    }

    #[test]
    fn status_json_round_trip() {
        let status = sample_status();
        let json = norito::json::to_json(&status).expect("json");
        assert!(json.contains(&format!("\"instance\":\"{}\"", "07".repeat(32))));
        assert!(json.contains(r#""halted":{"reason":"apply_diverged","details":9}"#));
        let parsed: SumeragiStatus = norito::json::from_str(&json).expect("parse");
        assert_eq!(parsed, status);
        let observer = SumeragiStatus {
            leader: None,
            proxy_tail: None,
            high_qc_view: None,
            signer: None,
            halted: None,
            ..status
        };
        let json = norito::json::to_json(&observer).expect("json");
        let parsed: SumeragiStatus = norito::json::from_str(&json).expect("parse");
        assert_eq!(parsed, observer);
        let encoded = norito::json::to_value(&observer).expect("status value");
        for field in ["leader", "proxy_tail", "high_qc_view", "signer", "halted"] {
            let mut omitted = encoded.clone();
            assert!(omitted.as_object_mut().unwrap().remove(field).is_some());
            assert!(
                norito::json::from_value::<SumeragiStatus>(omitted.clone()).is_err(),
                "missing nullable status field {field} must not default to null"
            );
            assert!(
                norito::json::from_str::<SumeragiStatus>(&norito::json::to_json(&omitted).unwrap())
                    .is_err(),
                "streaming decode must require nullable status field {field}"
            );
        }
    }

    #[test]
    fn halt_reason_json_and_codec_cover_every_variant() {
        for reason in [
            SumeragiHaltReason::SafetyRecordCorrupt,
            SumeragiHaltReason::SafetyRecordInconsistent,
            SumeragiHaltReason::SafetyViolation(5),
            SumeragiHaltReason::ApplyDiverged(6),
            SumeragiHaltReason::PublicationRecoveryRequired(7),
            SumeragiHaltReason::DriverAnomaly,
        ] {
            let json = norito::json::to_json(&reason).expect("json");
            let parsed: SumeragiHaltReason = norito::json::from_str(&json).expect("parse");
            assert_eq!(parsed, reason);
            let bytes = reason.encode();
            let decoded = SumeragiHaltReason::decode_all(&mut bytes.as_slice()).expect("decode");
            assert_eq!(decoded, reason);
        }
        assert_eq!(
            norito::json::to_json(&SumeragiHaltReason::DriverAnomaly).expect("json"),
            r#"{"reason":"driver_anomaly","details":null}"#
        );
    }

    #[test]
    fn halt_reason_height() {
        assert_eq!(SumeragiHaltReason::SafetyViolation(3).height(), Some(3));
        assert_eq!(SumeragiHaltReason::ApplyDiverged(4).height(), Some(4));
        assert_eq!(
            SumeragiHaltReason::PublicationRecoveryRequired(7).height(),
            Some(7)
        );
        assert_eq!(SumeragiHaltReason::SafetyRecordCorrupt.height(), None);
        assert_eq!(SumeragiHaltReason::SafetyRecordInconsistent.height(), None);
        assert_eq!(SumeragiHaltReason::DriverAnomaly.height(), None);
    }

    #[test]
    fn footprint_default_is_zero_and_round_trips() {
        let footprint = SumeragiFootprint::default();
        assert_eq!(footprint.votes + footprint.probe + footprint.sync_bytes, 0);
        let json = norito::json::to_json(&footprint).expect("json");
        let parsed: SumeragiFootprint = norito::json::from_str(&json).expect("parse");
        assert_eq!(parsed, footprint);
    }

    #[test]
    fn status_helpers() {
        let status = sample_status();
        assert!(status.is_halted());
        assert!(status.is_signing());
        assert_eq!(status.apply_lag(), 1);
        let abstaining = SumeragiStatus {
            abstaining: true,
            halted: None,
            ..status.clone()
        };
        assert!(!abstaining.is_halted());
        assert!(!abstaining.is_signing());
        let unanchored = SumeragiStatus {
            unanchored: true,
            ..status.clone()
        };
        assert!(!unanchored.is_signing());
        let observer = SumeragiStatus {
            signer: None,
            ..status.clone()
        };
        assert!(!observer.is_signing());
        let behind = SumeragiStatus {
            committed_height: 3,
            applied_height: 5,
            ..status
        };
        assert_eq!(behind.apply_lag(), 0);
    }
    #[test]
    fn native_readiness_fields_roundtrip_and_reject_missing_first_release_fields() {
        let mut status = sample_status();
        status.beacon_horizon = Some(BeaconHorizonStatusV1 {
            epoch_length_blocks: 64,
            next_required_pulse_height: Some(63),
            active_session_id: Some([9; 32]),
            session_covers_next_pulse: true,
            local_provider_ready: true,
        });
        let bytes = norito::encode_canonical(&status).unwrap();
        assert_eq!(
            norito::decode_canonical::<SumeragiStatus>(&bytes).unwrap(),
            status
        );
        let text = norito::json::to_json(&status).unwrap();
        assert_eq!(
            norito::json::from_str::<SumeragiStatus>(&text).unwrap(),
            status
        );
        let value: norito::json::Value = norito::json::from_str(&text).unwrap();
        for key in [
            "protocol_version",
            "config_fingerprint",
            "beacon_horizon",
            "leader",
            "proxy_tail",
            "high_qc_view",
            "signer",
            "halted",
        ] {
            let mut changed = value.clone();
            changed.as_object_mut().unwrap().remove(key);
            assert!(
                norito::json::from_value::<SumeragiStatus>(changed).is_err(),
                "missing {key}"
            );
        }
    }
}
