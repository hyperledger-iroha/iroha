//! SCCP v1 instructions (`specs/sccp.md` §4).
//!
//! These are the wire structs of the ten SCCP v1 instructions. Core enforces every SCCP rule;
//! the default executor only allows visitors through. Per-chain proofs, advances and evidence
//! are the bounded opaque wrappers of [`crate::sccp`], decoded by `iroha_sccp::light_client`.
// TODO(ws20): register these instructions (wire ids, record inventory, instruction-enum and
// executor visitors, universal-dataspace routing).

use crate::{
    DeriveJsonDeserialize, DeriveJsonSerialize, NetworkId,
    bridge::SccpNetworkV1,
    sccp::{
        attestation::{SccpAttestationSignatureV1, SccpAttestationStatementV1},
        inbound::SccpSourceProofBytesV1,
        keys::SccpBridgeKeyBindingV1,
        light_client::{SccpLcAdvanceBytesV1, SccpLcEvidenceBytesV1},
        params::SccpParametersV1,
    },
};
use iroha_crypto::SignatureOf;
use iroha_model_base::peer::PeerId;
use iroha_primitives::numeric::Numeric;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

/// Genesis-only: store the SCCP parameters, create the route escrows and an empty registry
/// (§4.1).
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
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::isi::sccp::InitializeSccpV1")]
pub struct InitializeSccpV1 {
    /// Complete SCCP parameters; every §4.1 rule must hold.
    pub parameters: SccpParametersV1,
    /// Fresh nonzero randomness per Taira genesis (§4.18).
    pub reset_nonce: [u8; 32],
}
impl crate::seal::Instruction for InitializeSccpV1 {}

/// Register or revoke a peer's bridge key (§4.2.2).
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::isi::sccp::SetSccpBridgeKeyV1")]
pub struct SetSccpBridgeKeyV1 {
    /// Registered peer whose key changes.
    pub peer: PeerId,
    /// Compressed secp256k1 key; `None` revokes from `activation_epoch`.
    #[norito(required)]
    pub public_key: Option<[u8; 33]>,
    /// Epoch from which the binding applies (0 exactly in genesis).
    pub activation_epoch: u64,
    /// Must equal the peer's `next_binding_nonce`.
    pub binding_nonce: u64,
    /// Consent of the peer's consensus key to [`Self::binding`].
    pub peer_signature: SignatureOf<SccpBridgeKeyBindingV1>,
    /// §3.8 proof of possession over the `SccpBridgeKey` digest; required iff `public_key` is
    /// present.
    #[norito(required)]
    pub key_pop: Option<[u8; 65]>,
}
impl crate::seal::Instruction for SetSccpBridgeKeyV1 {}

impl SetSccpBridgeKeyV1 {
    /// Return the binding message `peer_signature` must sign under `network_id`.
    #[must_use]
    pub fn binding(&self, network_id: NetworkId) -> SccpBridgeKeyBindingV1 {
        SccpBridgeKeyBindingV1::new(
            network_id,
            self.peer.clone(),
            self.public_key,
            self.activation_epoch,
            self.binding_nonce,
        )
    }

    /// Return whether `key_pop` is present exactly when `public_key` is.
    #[must_use]
    pub const fn pop_matches_key_presence(&self) -> bool {
        self.public_key.is_some() == self.key_pop.is_some()
    }
}

/// Store bridge-key signatures of attestation statements (§4.8).
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
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::isi::sccp::SubmitSccpAttestationsV1")]
pub struct SubmitSccpAttestationsV1 {
    /// Entries sorted by `(height, signer_index)` without duplicates.
    pub entries: Vec<SccpAttestationSignatureV1>,
}
impl crate::seal::Instruction for SubmitSccpAttestationsV1 {}

impl SubmitSccpAttestationsV1 {
    /// Return whether the entries are strictly ascending by `(height, signer_index)` (§4.8
    /// check 1, without the count bound).
    #[must_use]
    pub fn entries_strictly_ascending(&self) -> bool {
        self.entries
            .windows(2)
            .all(|pair| pair[0].key() < pair[1].key())
    }
}

/// Submit equivocation evidence against a bridge key (§4.11).
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
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::isi::sccp::SubmitSccpAttestationFaultV1")]
pub struct SubmitSccpAttestationFaultV1 {
    /// Faulty statement (all ten §3.6.1 fields).
    pub statement: SccpAttestationStatementV1,
    /// Bridge-key signature over the statement's §3.6 digest.
    pub signature: [u8; 65],
}
impl crate::seal::Instruction for SubmitSccpAttestationFaultV1 {}

/// Lock XOR and record an outbound message to an external network (§4.4).
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
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::isi::sccp::RecordSccpMessage")]
pub struct RecordSccpMessage {
    /// External target network.
    pub network: SccpNetworkV1,
    /// Revision the wallet verified; must be the route's `Bidirectional` revision.
    pub expected_revision: u32,
    /// XOR amount (§0); `taira_units(amount)` must be exact.
    pub amount: Numeric,
    /// Recipient bytes of the target's codec (§3.1).
    #[norito(json = "crate::json_helpers::base64_vec")]
    pub recipient: Vec<u8>,
}
impl crate::seal::Instruction for RecordSccpMessage {}

/// Prove an inbound burn, then attempt settlement (§4.12.1).
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
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::isi::sccp::SubmitSccpInboundMessageV1")]
pub struct SubmitSccpInboundMessageV1 {
    /// External source network.
    pub network: SccpNetworkV1,
    /// Route revision whose deployment emitted the burn.
    pub revision: u32,
    /// Canonical §3.2 payload.
    #[norito(json = "crate::json_helpers::base64_vec")]
    pub payload: Vec<u8>,
    /// Per-chain source proof.
    pub proof: SccpSourceProofBytesV1,
}
impl crate::seal::Instruction for SubmitSccpInboundMessageV1 {}

/// `SettleSccpV1::Inbound` target.
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
#[norito_schema(name = "iroha_data_model::isi::sccp::SccpSettleInboundV1")]
pub struct SccpSettleInboundV1 {
    /// Pending inbound message.
    pub message_id: [u8; 32],
}

/// `SettleSccpV1::Refund` target.
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
#[norito_schema(name = "iroha_data_model::isi::sccp::SccpSettleRefundV1")]
pub struct SccpSettleRefundV1 {
    /// External network.
    pub network: SccpNetworkV1,
    /// Route revision.
    pub revision: u32,
    /// Voided outbound nonce.
    pub nonce: u64,
}

/// What `SettleSccpV1` retries.
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
#[norito(tag = "target", content = "detail")]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::isi::sccp::SccpSettleTargetV1")]
pub enum SccpSettleTargetV1 {
    /// A `Pending` inbound settlement.
    #[codec(index = 0)]
    #[norito(rename = "inbound")]
    Inbound(SccpSettleInboundV1),
    /// A pending outbound refund.
    #[codec(index = 1)]
    #[norito(rename = "refund")]
    Refund(SccpSettleRefundV1),
}

/// Retry a pending inbound settlement or outbound refund without a proof (§4.12.1).
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
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::isi::sccp::SettleSccpV1")]
pub struct SettleSccpV1 {
    /// Record to settle.
    pub target: SccpSettleTargetV1,
}
impl crate::seal::Instruction for SettleSccpV1 {}

impl SettleSccpV1 {
    /// Retry the inbound settlement of `message_id`.
    #[must_use]
    pub const fn inbound(message_id: [u8; 32]) -> Self {
        Self {
            target: SccpSettleTargetV1::Inbound(SccpSettleInboundV1 { message_id }),
        }
    }

    /// Retry the refund of outbound `(network, revision, nonce)`.
    #[must_use]
    pub const fn refund(network: SccpNetworkV1, revision: u32, nonce: u64) -> Self {
        Self {
            target: SccpSettleTargetV1::Refund(SccpSettleRefundV1 {
                network,
                revision,
                nonce,
            }),
        }
    }
}

/// Prove destination voids of outbound nonces and attempt refunds (§4.16).
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
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::isi::sccp::SubmitSccpOutboundVoidV1")]
pub struct SubmitSccpOutboundVoidV1 {
    /// External network of the destination.
    pub network: SccpNetworkV1,
    /// Route revision whose deployment emitted the void.
    pub revision: u32,
    /// Per-chain source proof of the void event.
    pub proof: SccpSourceProofBytesV1,
}
impl crate::seal::Instruction for SubmitSccpOutboundVoidV1 {}

/// Permissionlessly advance a light client (§4.13.2).
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
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::isi::sccp::AdvanceSccpLightClientV1")]
pub struct AdvanceSccpLightClientV1 {
    /// Source chain.
    pub network: SccpNetworkV1,
    /// Compare-and-swap guard on the light client's `state_hash`.
    #[norito(required)]
    pub expected_state_hash: Option<[u8; 32]>,
    /// Per-chain advance (including `Backfill`).
    pub advance: SccpLcAdvanceBytesV1,
}
impl crate::seal::Instruction for AdvanceSccpLightClientV1 {}

/// Freeze a light client with two conflicting quorum-valid records (§4.13.2).
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
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::isi::sccp::ReportSccpLightClientEquivocationV1")]
pub struct ReportSccpLightClientEquivocationV1 {
    /// Source chain.
    pub network: SccpNetworkV1,
    /// First record.
    pub a: SccpLcEvidenceBytesV1,
    /// Conflicting record.
    pub b: SccpLcEvidenceBytesV1,
}
impl crate::seal::Instruction for ReportSccpLightClientEquivocationV1 {}

fn sccp_decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}

macro_rules! impl_sccp_decode_from_slice {
    ($ty:ty { $($field:ident : $field_ty:ty),+ $(,)? }) => {
        impl<'a> norito::core::DecodeFromSlice<'a> for $ty {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                let flags = sccp_decode_flags();
                if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
                    return super::decode_packed_instruction_payload::<Self>(bytes);
                }
                let mut offset = 0usize;
                $(
                    let $field = super::decode_aos_canonical_field::<$field_ty>(
                        super::read_aos_field(bytes, &mut offset, flags)?,
                        flags,
                    )?;
                )+
                if offset != bytes.len() {
                    return Err(norito::core::Error::LengthMismatch);
                }
                norito::core::note_payload_access(bytes, offset);
                Ok((Self { $($field),+ }, offset))
            }
        }
    };
}

impl_sccp_decode_from_slice!(InitializeSccpV1 {
    parameters: SccpParametersV1,
    reset_nonce: [u8; 32],
});
impl_sccp_decode_from_slice!(SetSccpBridgeKeyV1 {
    peer: PeerId,
    public_key: Option<[u8; 33]>,
    activation_epoch: u64,
    binding_nonce: u64,
    peer_signature: SignatureOf<SccpBridgeKeyBindingV1>,
    key_pop: Option<[u8; 65]>,
});
impl_sccp_decode_from_slice!(SubmitSccpAttestationsV1 {
    entries: Vec<SccpAttestationSignatureV1>,
});
impl_sccp_decode_from_slice!(SubmitSccpAttestationFaultV1 {
    statement: SccpAttestationStatementV1,
    signature: [u8; 65],
});
impl_sccp_decode_from_slice!(RecordSccpMessage {
    network: SccpNetworkV1,
    expected_revision: u32,
    amount: Numeric,
    recipient: Vec<u8>,
});
impl_sccp_decode_from_slice!(SubmitSccpInboundMessageV1 {
    network: SccpNetworkV1,
    revision: u32,
    payload: Vec<u8>,
    proof: SccpSourceProofBytesV1,
});
impl_sccp_decode_from_slice!(SettleSccpV1 {
    target: SccpSettleTargetV1,
});
impl_sccp_decode_from_slice!(SubmitSccpOutboundVoidV1 {
    network: SccpNetworkV1,
    revision: u32,
    proof: SccpSourceProofBytesV1,
});
impl_sccp_decode_from_slice!(AdvanceSccpLightClientV1 {
    network: SccpNetworkV1,
    expected_state_hash: Option<[u8; 32]>,
    advance: SccpLcAdvanceBytesV1,
});
impl_sccp_decode_from_slice!(ReportSccpLightClientEquivocationV1 {
    network: SccpNetworkV1,
    a: SccpLcEvidenceBytesV1,
    b: SccpLcEvidenceBytesV1,
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        isi::test_support::assert_slice_roundtrip,
        sccp::test_support::{assert_rejects_unknown_field, roundtrip},
    };
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use norito::{codec::DecodeAll, core::DecodeFromSlice as _};

    const ETH: SccpNetworkV1 = SccpNetworkV1::EthereumMainnet;
    const TON: SccpNetworkV1 = SccpNetworkV1::TonMainnet;

    fn key_pair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("deterministic Ed25519 seed")
    }

    fn network_id(seed: u8) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<crate::block::BlockHeader>::from_untyped_unchecked(
            Hash::new([seed; Hash::LENGTH]),
        ))
    }

    fn set_bridge_key(public_key: Option<[u8; 33]>) -> SetSccpBridgeKeyV1 {
        let peer_keys = key_pair(1);
        let peer = PeerId::new(peer_keys.public_key().clone());
        let binding = SccpBridgeKeyBindingV1::new(network_id(7), peer.clone(), public_key, 3, 0);
        SetSccpBridgeKeyV1 {
            peer,
            public_key,
            activation_epoch: 3,
            binding_nonce: 0,
            peer_signature: SignatureOf::new(peer_keys.private_key(), &binding),
            key_pop: public_key.map(|_| [0x1b; 65]),
        }
    }

    fn statement() -> SccpAttestationStatementV1 {
        SccpAttestationStatementV1 {
            height: 9,
            epoch: 1,
            timestamp_ms: 1_758_000_000_000,
            block_hash: [1; 32],
            sccp_root: [2; 32],
            message_count: 1,
            history_root: [3; 32],
            history_size: 1,
            roster_digest: [4; 32],
            next_roster_digest: [0; 32],
        }
    }

    fn entry(height: u64, signer_index: u8) -> SccpAttestationSignatureV1 {
        SccpAttestationSignatureV1 {
            height,
            signer_index,
            signature: [0x1c; 65],
        }
    }

    fn proof() -> SccpSourceProofBytesV1 {
        SccpSourceProofBytesV1::new(vec![0x4e, 0x52, 0x54, 0x30, 0x01]).expect("bounded")
    }

    fn check<T>(value: &T)
    where
        T: Clone
            + Encode
            + DecodeAll
            + norito::NoritoSerialize
            + for<'a> norito::NoritoDeserialize<'a>
            + for<'a> norito::core::DecodeFromSlice<'a>
            + norito::json::JsonSerialize
            + norito::json::JsonDeserialize
            + PartialEq
            + core::fmt::Debug,
    {
        roundtrip(value);
        assert_slice_roundtrip(value.clone());
        assert_rejects_unknown_field(value, &[]);
    }

    #[test]
    fn every_instruction_roundtrips() {
        check(&InitializeSccpV1 {
            parameters: SccpParametersV1::taira_default(),
            reset_nonce: [0x5a; 32],
        });
        check(&set_bridge_key(Some([2; 33])));
        check(&set_bridge_key(None));
        check(&SubmitSccpAttestationsV1 {
            entries: vec![entry(9, 0), entry(9, 4), entry(10, 1)],
        });
        check(&SubmitSccpAttestationFaultV1 {
            statement: statement(),
            signature: [0x1b; 65],
        });
        check(&RecordSccpMessage {
            network: ETH,
            expected_revision: 1,
            amount: Numeric::new(1_500_000_000_u64, 9),
            recipient: vec![0x22; 20],
        });
        check(&SubmitSccpInboundMessageV1 {
            network: TON,
            revision: 2,
            payload: vec![2, 1, 0, 0, 0, 4],
            proof: proof(),
        });
        check(&SettleSccpV1::inbound([6; 32]));
        check(&SettleSccpV1::refund(ETH, 1, 7));
        check(&SubmitSccpOutboundVoidV1 {
            network: ETH,
            revision: 1,
            proof: proof(),
        });
        check(&AdvanceSccpLightClientV1 {
            network: ETH,
            expected_state_hash: Some([8; 32]),
            advance: SccpLcAdvanceBytesV1::new(vec![1; 64]).expect("bounded"),
        });
        check(&AdvanceSccpLightClientV1 {
            network: ETH,
            expected_state_hash: None,
            advance: SccpLcAdvanceBytesV1::new(vec![1]).expect("bounded"),
        });
        check(&ReportSccpLightClientEquivocationV1 {
            network: TON,
            a: SccpLcEvidenceBytesV1::new(vec![1, 2]).expect("bounded"),
            b: SccpLcEvidenceBytesV1::new(vec![3, 4]).expect("bounded"),
        });
    }

    #[test]
    fn instruction_ids_are_the_sccp_module_paths() {
        let ids = [
            crate::isi::Instruction::id(&InitializeSccpV1 {
                parameters: SccpParametersV1::taira_default(),
                reset_nonce: [1; 32],
            }),
            crate::isi::Instruction::id(&SettleSccpV1::inbound([0; 32])),
            crate::isi::Instruction::id(&RecordSccpMessage {
                network: ETH,
                expected_revision: 1,
                amount: Numeric::from(1_u64),
                recipient: vec![1; 20],
            }),
        ];
        assert_eq!(
            ids,
            [
                "iroha_data_model::isi::sccp::InitializeSccpV1",
                "iroha_data_model::isi::sccp::SettleSccpV1",
                "iroha_data_model::isi::sccp::RecordSccpMessage",
            ]
        );
    }

    #[test]
    fn binding_is_rebuilt_from_the_instruction_and_verifies() {
        let instruction = set_bridge_key(Some([2; 33]));
        let binding = instruction.binding(network_id(7));
        assert!(binding.has_canonical_domain());
        assert_eq!(binding.peer, instruction.peer);
        assert_eq!(binding.public_key, Some([2; 33]));
        assert_eq!(binding.activation_epoch, 3);
        assert_eq!(binding.binding_nonce, 0);
        let peer_key = key_pair(1).public_key().clone();
        instruction
            .peer_signature
            .verify(&peer_key, &binding)
            .expect("peer consent verifies");
        assert!(
            instruction
                .peer_signature
                .verify(&peer_key, &instruction.binding(network_id(8)))
                .is_err(),
            "consent is bound to the NetworkId"
        );
    }

    #[test]
    fn pop_presence_must_match_key_presence() {
        assert!(set_bridge_key(Some([2; 33])).pop_matches_key_presence());
        assert!(set_bridge_key(None).pop_matches_key_presence());
        let mut missing_pop = set_bridge_key(Some([2; 33]));
        missing_pop.key_pop = None;
        assert!(!missing_pop.pop_matches_key_presence());
        let mut stray_pop = set_bridge_key(None);
        stray_pop.key_pop = Some([0; 65]);
        assert!(!stray_pop.pop_matches_key_presence());
    }

    #[test]
    fn attestation_entries_must_be_strictly_ascending() {
        let batch = |entries| SubmitSccpAttestationsV1 { entries };
        assert!(batch(vec![]).entries_strictly_ascending());
        assert!(batch(vec![entry(1, 0)]).entries_strictly_ascending());
        assert!(batch(vec![entry(1, 0), entry(1, 1), entry(2, 0)]).entries_strictly_ascending());
        assert!(!batch(vec![entry(1, 1), entry(1, 1)]).entries_strictly_ascending());
        assert!(!batch(vec![entry(1, 2), entry(1, 1)]).entries_strictly_ascending());
        assert!(!batch(vec![entry(2, 0), entry(1, 5)]).entries_strictly_ascending());
    }

    #[test]
    fn settle_constructors_build_each_target() {
        assert_eq!(
            SettleSccpV1::inbound([3; 32]).target,
            SccpSettleTargetV1::Inbound(SccpSettleInboundV1 {
                message_id: [3; 32]
            })
        );
        assert_eq!(
            SettleSccpV1::refund(TON, 2, 9).target,
            SccpSettleTargetV1::Refund(SccpSettleRefundV1 {
                network: TON,
                revision: 2,
                nonce: 9,
            })
        );
        let json = norito::json::to_json(&SettleSccpV1::refund(TON, 2, 9)).expect("json");
        assert!(json.contains("\"target\":\"refund\""), "{json}");
    }

    #[test]
    fn instructions_reject_out_of_bounds_wrappers_and_trailing_bytes() {
        let valid = SubmitSccpOutboundVoidV1 {
            network: ETH,
            revision: 1,
            proof: proof(),
        };
        let mut json = norito::json::to_value(&valid).expect("value");
        json.as_object_mut()
            .expect("object")
            .insert("proof".to_owned(), norito::json!({ "bytes": "" }));
        let empty = norito::json::to_json(&json).expect("json");
        assert!(norito::json::from_json::<SubmitSccpOutboundVoidV1>(&empty).is_err());

        let mut encoded = valid.encode();
        encoded.push(0);
        assert!(SubmitSccpOutboundVoidV1::decode_from_slice(&encoded).is_err());
        assert!(SubmitSccpOutboundVoidV1::decode_all(&mut encoded.as_slice()).is_err());

        let evidence = ReportSccpLightClientEquivocationV1 {
            network: ETH,
            a: SccpLcEvidenceBytesV1::new(vec![1]).expect("bounded"),
            b: SccpLcEvidenceBytesV1::new(vec![2]).expect("bounded"),
        };
        let mut json = norito::json::to_value(&evidence).expect("value");
        json.as_object_mut().expect("object").remove("b");
        let missing = norito::json::to_json(&json).expect("json");
        assert!(norito::json::from_json::<ReportSccpLightClientEquivocationV1>(&missing).is_err());
    }
}
