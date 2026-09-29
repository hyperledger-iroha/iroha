//! SCCP v1 bridge-key state (`specs/sccp.md` §4.2, §4.11).
//!
//! Every Taira validator holds a secp256k1 bridge key that signs SCCP attestations. World state
//! keeps, per peer, the active and pending key, a bounded list of retired tombstones, the binding
//! nonce that makes every binding single-use, the fee-exemption rate limit and the newest fault
//! bar. The consensus-key consent a peer signs for a binding is [`SccpBridgeKeyBindingV1`].
//! Recorded equivocation evidence is [`SccpAttestationFaultRecordV1`], keyed by
//! [`SccpFaultRefV1`].

use crate::{DeriveJsonDeserialize, DeriveJsonSerialize, NetworkId};
use iroha_model_base::peer::PeerId;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

/// Protocol domain of [`SccpBridgeKeyBindingV1`]; separates the peer's consent signature from
/// every other message its consensus key signs.
pub const SCCP_BRIDGE_KEY_BINDING_DOMAIN_V1: &str = "iroha.sccp.bridge_key.v1";
/// Maximum retired-key tombstones kept per peer; the oldest is dropped from the list only (its
/// address stays burned in `sccp_bridge_key_owners`).
pub const SCCP_BRIDGE_KEY_RETIRED_MAX_V1: usize = 16;

/// Reference to one recorded attestation fault: the key of `sccp_attestation_faults`.
///
/// A peer's `barred` field holds its newest fault; only the Parliament-enacted
/// `ClearBridgeKeyFault` action clears it, and only when it still names this exact fault.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sccp::keys::SccpFaultRefV1")]
pub struct SccpFaultRefV1 {
    /// Ethereum-style address of the faulted bridge key (§3.8).
    pub address: [u8; 20],
    /// Taira height at which the equivocating attestation was signed.
    pub height: u64,
}

impl SccpFaultRefV1 {
    /// Return the fault height when it exceeds `maximum` (the exact JSON integer bound).
    #[must_use]
    pub fn first_json_u64_violation(&self, maximum: u64) -> Option<&'static str> {
        (self.height > maximum)
            .then_some("SCCP bridge-key fault height exceeds the exact JSON integer maximum")
    }
}

/// One registered bridge key (§4.2.1).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sccp::keys::SccpBridgeKeyV1")]
pub struct SccpBridgeKeyV1 {
    /// Compressed secp256k1 public key.
    pub public_key: [u8; 33],
    /// Address of the key (§3.8): `keccak256(X ‖ Y)[12..32]` of the uncompressed point.
    pub address: [u8; 20],
    /// First epoch in which the key is active.
    pub activation_epoch: u64,
    /// Taira height at which the binding was recorded.
    pub registered_at_height: u64,
    /// Set by recorded equivocation evidence (§4.11); a faulted key never signs again.
    pub faulted: bool,
}

/// Bridge-key state of one peer (`sccp_bridge_keys`, §4.2.1).
///
/// At most one of [`pending`](Self::pending) and
/// [`pending_revocation_epoch`](Self::pending_revocation_epoch) is set: a later binding replaces
/// the earlier one, whatever its kind.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sccp::keys::SccpBridgeKeyStateV1")]
pub struct SccpBridgeKeyStateV1 {
    /// Key that signs for the peer in the current epoch.
    #[norito(required)]
    pub active: Option<SccpBridgeKeyV1>,
    /// Key that becomes active at its `activation_epoch`.
    #[norito(required)]
    pub pending: Option<SccpBridgeKeyV1>,
    /// Epoch from which the active key is revoked (a binding with `public_key: None`).
    ///
    /// The spec's state (§4.2.1) has no slot for a pending revocation, which §4.2.2 requires to
    /// take effect "at activation"; this field records it.
    #[norito(required)]
    pub pending_revocation_epoch: Option<u64>,
    /// Retired tombstones, oldest first, at most [`SCCP_BRIDGE_KEY_RETIRED_MAX_V1`].
    pub retired: Vec<SccpBridgeKeyV1>,
    /// Binding nonce the next `SetSccpBridgeKeyV1` must carry.
    pub next_binding_nonce: u64,
    /// Epoch of the last fee-exempt registration (§4.2.3).
    #[norito(required)]
    pub last_exempt_binding_epoch: Option<u64>,
    /// Newest recorded fault (§4.11); cleared only by the Parliament.
    #[norito(required)]
    pub barred: Option<SccpFaultRefV1>,
}

impl SccpBridgeKeyStateV1 {
    /// Return whether a recorded fault bars the peer from registering keys.
    #[must_use]
    pub const fn is_barred(&self) -> bool {
        self.barred.is_some()
    }

    /// Stage `key` as the pending key, replacing any pending key or revocation.
    pub fn stage_key(&mut self, key: SccpBridgeKeyV1) {
        self.pending = Some(key);
        self.pending_revocation_epoch = None;
    }

    /// Stage a revocation of the active key from `epoch`, replacing any pending key or
    /// revocation.
    pub fn stage_revocation(&mut self, epoch: u64) {
        self.pending = None;
        self.pending_revocation_epoch = Some(epoch);
    }

    /// Append `key` to the retired tombstones, dropping the oldest beyond
    /// [`SCCP_BRIDGE_KEY_RETIRED_MAX_V1`].
    pub fn retire(&mut self, key: SccpBridgeKeyV1) {
        self.retired.push(key);
        if self.retired.len() > SCCP_BRIDGE_KEY_RETIRED_MAX_V1 {
            let excess = self.retired.len() - SCCP_BRIDGE_KEY_RETIRED_MAX_V1;
            self.retired.drain(..excess);
        }
    }

    /// Apply the pending binding if it activates at or before `epoch` (§4.2.2 promotion).
    ///
    /// A pending key becomes active and the previous active key is retired; a pending
    /// revocation retires the active key. Returns whether anything changed.
    pub fn promote_for_epoch(&mut self, epoch: u64) -> bool {
        if let Some(key) = self.pending.filter(|key| key.activation_epoch <= epoch) {
            self.pending = None;
            if let Some(previous) = self.active.replace(key) {
                self.retire(previous);
            }
            return true;
        }
        if self
            .pending_revocation_epoch
            .is_some_and(|revocation| revocation <= epoch)
        {
            self.pending_revocation_epoch = None;
            if let Some(previous) = self.active.take() {
                self.retire(previous);
            }
            return true;
        }
        false
    }
}

/// Consent of a peer's consensus key to one bridge-key binding (§4.2.2).
///
/// `SetSccpBridgeKeyV1.peer_signature` is a `SignatureOf` this value by the peer's consensus
/// key (the `PublicLaneCandidateAuthorization` pattern). The domain is fixed by
/// [`SccpBridgeKeyBindingV1::new`]; core rebuilds the binding from the instruction and the live
/// `NetworkId` before verifying.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sccp::keys::SccpBridgeKeyBindingV1")]
pub struct SccpBridgeKeyBindingV1 {
    /// Always [`SCCP_BRIDGE_KEY_BINDING_DOMAIN_V1`] for values built by [`Self::new`].
    domain: String,
    /// Live Taira `NetworkId`.
    pub network_id: NetworkId,
    /// Peer whose bridge key is bound.
    pub peer: PeerId,
    /// Compressed secp256k1 key, or `None` for a revocation.
    #[norito(required)]
    pub public_key: Option<[u8; 33]>,
    /// Epoch from which the binding applies.
    pub activation_epoch: u64,
    /// Must equal the peer's `next_binding_nonce`.
    pub binding_nonce: u64,
}

impl SccpBridgeKeyBindingV1 {
    /// Build the binding message with the canonical domain.
    #[must_use]
    pub fn new(
        network_id: NetworkId,
        peer: PeerId,
        public_key: Option<[u8; 33]>,
        activation_epoch: u64,
        binding_nonce: u64,
    ) -> Self {
        Self {
            domain: SCCP_BRIDGE_KEY_BINDING_DOMAIN_V1.to_owned(),
            network_id,
            peer,
            public_key,
            activation_epoch,
            binding_nonce,
        }
    }

    /// Return the protocol domain carried by this binding.
    #[must_use]
    pub fn domain(&self) -> &str {
        &self.domain
    }
}

/// Recorded equivocation evidence (`sccp_attestation_faults[(address, height)]`, §4.11).
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sccp::keys::SccpAttestationFaultRecordV1")]
pub struct SccpAttestationFaultRecordV1 {
    /// Peer that owns the faulted key.
    pub peer: PeerId,
    /// §3.6 EIP-712 attestation digest of the faulty statement under the live domain.
    pub statement_hash: [u8; 32],
    /// Taira height at which the evidence was recorded.
    pub reported_at_height: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sccp::test_support::{assert_rejects_unknown_field, roundtrip};
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use norito::codec::DecodeAll as _;

    const JSON_MAX: u64 = (1_u64 << 53) - 1;

    fn peer(seed: u8) -> PeerId {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("deterministic Ed25519 seed");
        PeerId::new(key_pair.public_key().clone())
    }

    fn network_id(seed: u8) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<crate::block::BlockHeader>::from_untyped_unchecked(
            Hash::new([seed; Hash::LENGTH]),
        ))
    }

    fn key(seed: u8, activation_epoch: u64) -> SccpBridgeKeyV1 {
        let mut public_key = [seed; 33];
        public_key[0] = 0x02;
        SccpBridgeKeyV1 {
            public_key,
            address: [seed; 20],
            activation_epoch,
            registered_at_height: u64::from(seed) * 10,
            faulted: false,
        }
    }

    #[test]
    fn binary_and_json_roundtrip() {
        for fault in [
            SccpFaultRefV1 {
                address: [0x5a; 20],
                height: 42,
            },
            SccpFaultRefV1 {
                address: [0xff; 20],
                height: u64::MAX,
            },
        ] {
            let encoded = fault.encode();
            assert_eq!(
                SccpFaultRefV1::decode_all(&mut encoded.as_slice()).expect("decode"),
                fault
            );
            roundtrip(&fault);
        }
        roundtrip(&key(7, 3));
        roundtrip(&SccpBridgeKeyV1 {
            faulted: true,
            ..key(8, u64::MAX)
        });
        roundtrip(&SccpBridgeKeyStateV1::default());
        roundtrip(&SccpBridgeKeyStateV1 {
            active: Some(key(1, 0)),
            pending: Some(key(2, 5)),
            pending_revocation_epoch: None,
            retired: vec![key(3, 0), key(4, 1)],
            next_binding_nonce: 9,
            last_exempt_binding_epoch: Some(4),
            barred: Some(SccpFaultRefV1 {
                address: [3; 20],
                height: 77,
            }),
        });
        roundtrip(&SccpBridgeKeyStateV1 {
            pending_revocation_epoch: Some(12),
            ..SccpBridgeKeyStateV1::default()
        });
        let binding = SccpBridgeKeyBindingV1::new(network_id(1), peer(2), Some([2; 33]), 6, 3);
        roundtrip(&binding);
        roundtrip(&SccpBridgeKeyBindingV1::new(
            network_id(1),
            peer(2),
            None,
            6,
            4,
        ));
        roundtrip(&SccpAttestationFaultRecordV1 {
            peer: peer(3),
            statement_hash: [0xab; 32],
            reported_at_height: 1_000,
        });
    }

    #[test]
    fn json_is_closed_and_options_are_explicit() {
        assert_rejects_unknown_field(&key(1, 1), &[]);
        let state = SccpBridgeKeyStateV1 {
            active: Some(key(1, 0)),
            ..SccpBridgeKeyStateV1::default()
        };
        assert_rejects_unknown_field(&state, &[]);
        assert_rejects_unknown_field(&state, &["active"]);
        for field in [
            "active",
            "pending",
            "pending_revocation_epoch",
            "last_exempt_binding_epoch",
            "barred",
        ] {
            let mut value = norito::json::to_value(&state).expect("value");
            value
                .as_object_mut()
                .expect("object")
                .remove(field)
                .expect("field present");
            let json = norito::json::to_json(&value).expect("json");
            assert!(
                norito::json::from_json::<SccpBridgeKeyStateV1>(&json).is_err(),
                "{field} must be explicit"
            );
        }
        let binding = SccpBridgeKeyBindingV1::new(network_id(1), peer(2), None, 6, 3);
        assert_rejects_unknown_field(&binding, &[]);
    }

    #[test]
    fn json_u64_violation_is_exact_at_the_bound() {
        let at = SccpFaultRefV1 {
            address: [1; 20],
            height: JSON_MAX,
        };
        assert_eq!(at.first_json_u64_violation(JSON_MAX), None);
        let over = SccpFaultRefV1 {
            height: JSON_MAX + 1,
            ..at
        };
        assert!(over.first_json_u64_violation(JSON_MAX).is_some());
    }

    #[test]
    fn binding_domain_is_fixed_by_the_constructor() {
        assert_eq!(
            SCCP_BRIDGE_KEY_BINDING_DOMAIN_V1,
            "iroha.sccp.bridge_key.v1"
        );
        let binding = SccpBridgeKeyBindingV1::new(network_id(9), peer(9), Some([3; 33]), 1, 0);
        assert_eq!(binding.domain(), SCCP_BRIDGE_KEY_BINDING_DOMAIN_V1);
        let json = norito::json::to_json(&binding).expect("json");
        assert!(
            json.contains("\"domain\":\"iroha.sccp.bridge_key.v1\""),
            "{json}"
        );
        let forged = json.replace("iroha.sccp.bridge_key.v1", "iroha.sccp.other.v1");
        let decoded = norito::json::from_json::<SccpBridgeKeyBindingV1>(&forged).expect("decodes");
        assert_ne!(decoded.domain(), SCCP_BRIDGE_KEY_BINDING_DOMAIN_V1);
        assert_ne!(decoded, binding);
    }

    #[test]
    fn binding_frame_binds_every_field() {
        let base = SccpBridgeKeyBindingV1::new(network_id(1), peer(1), Some([2; 33]), 5, 7);
        let frame = norito::to_bytes(&base).expect("frame");
        for variant in [
            SccpBridgeKeyBindingV1::new(network_id(2), peer(1), Some([2; 33]), 5, 7),
            SccpBridgeKeyBindingV1::new(network_id(1), peer(2), Some([2; 33]), 5, 7),
            SccpBridgeKeyBindingV1::new(network_id(1), peer(1), None, 5, 7),
            SccpBridgeKeyBindingV1::new(network_id(1), peer(1), Some([2; 33]), 6, 7),
            SccpBridgeKeyBindingV1::new(network_id(1), peer(1), Some([2; 33]), 5, 8),
        ] {
            assert_ne!(norito::to_bytes(&variant).expect("frame"), frame);
        }
    }

    #[test]
    fn staging_replaces_the_other_kind() {
        let mut state = SccpBridgeKeyStateV1::default();
        assert!(!state.is_barred());
        state.stage_revocation(4);
        assert_eq!(state.pending_revocation_epoch, Some(4));
        state.stage_key(key(1, 5));
        assert_eq!(state.pending, Some(key(1, 5)));
        assert_eq!(state.pending_revocation_epoch, None);
        state.stage_revocation(6);
        assert_eq!(state.pending, None);
        assert_eq!(state.pending_revocation_epoch, Some(6));
        state.barred = Some(SccpFaultRefV1 {
            address: [1; 20],
            height: 1,
        });
        assert!(state.is_barred());
    }

    #[test]
    fn retired_tombstones_are_bounded_oldest_first() {
        let mut state = SccpBridgeKeyStateV1::default();
        for seed in 0..20_u8 {
            state.retire(key(seed, u64::from(seed)));
        }
        assert_eq!(state.retired.len(), SCCP_BRIDGE_KEY_RETIRED_MAX_V1);
        assert_eq!(state.retired.first(), Some(&key(4, 4)));
        assert_eq!(state.retired.last(), Some(&key(19, 19)));
    }

    #[test]
    fn promotion_applies_keys_and_revocations_at_their_epoch() {
        let mut state = SccpBridgeKeyStateV1::default();
        state.stage_key(key(1, 0));
        assert!(state.promote_for_epoch(0), "genesis promotes immediately");
        assert_eq!(state.active, Some(key(1, 0)));
        assert_eq!(state.pending, None);

        state.stage_key(key(2, 3));
        assert!(!state.promote_for_epoch(2));
        assert!(state.promote_for_epoch(3));
        assert_eq!(state.active, Some(key(2, 3)));
        assert_eq!(state.retired, vec![key(1, 0)]);

        state.stage_revocation(5);
        assert!(!state.promote_for_epoch(4));
        assert!(state.promote_for_epoch(5));
        assert_eq!(state.active, None);
        assert_eq!(state.pending_revocation_epoch, None);
        assert_eq!(state.retired, vec![key(1, 0), key(2, 3)]);
        assert!(!state.promote_for_epoch(9), "nothing left to promote");

        state.stage_revocation(6);
        assert!(
            state.promote_for_epoch(6),
            "revoking without an active key clears it"
        );
        assert_eq!(state.retired.len(), 2);
    }
}
