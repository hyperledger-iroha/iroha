//! Canonical SCCP V1 network, lane, and governed source-identity commitments.
//!
//! The data-model types deliberately expose a closed profile inventory. This
//! module gives those profiles an explicit canonical byte layout so protocol
//! hashes never depend on Rust discriminants, JSON spelling, or a platform ABI.

pub use iroha_data_model::bridge::{
    SccpEvmSourceEmitterV1, SccpLaneIdV1, SccpNetworkV1, SccpOutboundMessageContextV1,
    SccpOutboundMessageKeyV1, SccpOutboundPendingMessageRecordV1, SccpSourceEmitterV1,
    SccpSourceIdentityV1, SccpTronSourceEmitterV1, canonical_sccp_lane_id_bytes_v1,
    canonical_sccp_network_bytes_v1, canonical_sccp_source_emitter_bytes_v1,
    canonical_sccp_source_identity_bytes_v1, sccp_lane_id_hash_v1, sccp_network_identity_hash_v1,
    sccp_network_tag_v1, sccp_source_emitter_identity_hash_v1, sccp_source_identity_hash_v1,
};
use tiny_keccak::{Hasher as _, Keccak};

use crate::H256;

/// Keccak-256 source-event domain separator used by every native route contract.
pub const SCCP_SOURCE_EVENT_DIGEST_PREFIX_V1: &[u8] = b"sccp:source:event:v1";

fn keccak256(prefix: &[u8], payload: &[u8]) -> H256 {
    let mut hasher = Keccak::v256();
    hasher.update(prefix);
    hasher.update(payload);
    let mut output = [0u8; 32];
    hasher.finalize(&mut output);
    output
}

/// Decode one stable V1 network-profile tag.
#[must_use]
pub const fn sccp_network_from_tag_v1(tag: u8) -> Option<SccpNetworkV1> {
    match tag {
        1 => Some(SccpNetworkV1::SoraTaira),
        2 => Some(SccpNetworkV1::EthereumMainnet),
        3 => Some(SccpNetworkV1::EthereumSepolia),
        4 => Some(SccpNetworkV1::BscMainnet),
        5 => Some(SccpNetworkV1::BscTestnet),
        10 => Some(SccpNetworkV1::TronMainnet),
        11 => Some(SccpNetworkV1::TronNile),
        12 => Some(SccpNetworkV1::TronShasta),
        _ => None,
    }
}

/// Return the canonical V1 preimage for an SCCP event emitted on one exact lane.
///
/// Binding the exact lane hash, rather than only the two numeric protocol
/// domains, prevents a proof from being replayed between mainnet and testnet
/// profiles that intentionally share a protocol domain.
pub fn canonical_sccp_lane_source_event_bytes_v1(
    lane: SccpLaneIdV1,
    message_id: H256,
    payload_hash: H256,
) -> Option<Vec<u8>> {
    let lane_hash = sccp_lane_id_hash_v1(lane)?;
    if [lane_hash, message_id, payload_hash]
        .iter()
        .any(|hash| hash.iter().all(|byte| *byte == 0))
        || lane_hash == message_id
        || lane_hash == payload_hash
        || message_id == payload_hash
    {
        return None;
    }
    let mut out = Vec::with_capacity(97);
    out.push(1);
    out.extend_from_slice(&lane_hash);
    out.extend_from_slice(&message_id);
    out.extend_from_slice(&payload_hash);
    Some(out)
}

/// Hash an SCCP event emitted on one exact V1 lane.
///
/// The contract-computable preimage is exactly
/// `"sccp:source:event:v1" || 0x01 || lane_hash || message_id || payload_hash`.
/// All three hash roles must be nonzero and pairwise distinct. Keccak-256 is
/// used here because EVM and TVM route contracts must recompute the value
/// without accepting a caller-supplied digest.
pub fn sccp_lane_source_event_digest_v1(
    lane: SccpLaneIdV1,
    message_id: H256,
    payload_hash: H256,
) -> Option<H256> {
    let preimage = canonical_sccp_lane_source_event_bytes_v1(lane, message_id, payload_hash)?;
    Some(keccak256(SCCP_SOURCE_EVENT_DIGEST_PREFIX_V1, &preimage))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    const NETWORKS: [SccpNetworkV1; 8] = [
        SccpNetworkV1::SoraTaira,
        SccpNetworkV1::EthereumMainnet,
        SccpNetworkV1::EthereumSepolia,
        SccpNetworkV1::BscMainnet,
        SccpNetworkV1::BscTestnet,
        SccpNetworkV1::TronMainnet,
        SccpNetworkV1::TronNile,
        SccpNetworkV1::TronShasta,
    ];

    fn sample_identity() -> SccpSourceIdentityV1 {
        SccpSourceIdentityV1 {
            lane: SccpLaneIdV1 {
                source: SccpNetworkV1::EthereumMainnet,
                target: SccpNetworkV1::SoraTaira,
            },
            emitter: SccpSourceEmitterV1::Evm(SccpEvmSourceEmitterV1 {
                address: [0x11; 20],
                runtime_code_hash: [0x22; 32],
                route_config_hash: [0x33; 32],
            }),
        }
    }

    #[test]
    fn closed_network_profiles_have_distinct_canonical_bytes_and_hashes() {
        let bytes = NETWORKS
            .into_iter()
            .map(canonical_sccp_network_bytes_v1)
            .collect::<BTreeSet<_>>();
        let hashes = NETWORKS
            .into_iter()
            .map(sccp_network_identity_hash_v1)
            .collect::<BTreeSet<_>>();
        assert_eq!(bytes.len(), NETWORKS.len());
        assert_eq!(hashes.len(), NETWORKS.len());
    }

    #[test]
    fn canonical_network_bytes_bind_exact_native_identity() {
        assert_eq!(
            canonical_sccp_network_bytes_v1(SccpNetworkV1::EthereumMainnet),
            [1, 2, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(
            canonical_sccp_network_bytes_v1(SccpNetworkV1::TronMainnet),
            [1, 10, 5, 0, 0, 0, 0xdc, 0x53, 0x66, 0x2b]
        );
        assert_ne!(
            sccp_network_identity_hash_v1(SccpNetworkV1::BscMainnet),
            sccp_network_identity_hash_v1(SccpNetworkV1::BscTestnet)
        );
    }

    #[test]
    fn source_identity_hashes_match_independent_golden_vectors() {
        let identity = sample_identity();
        let lane_hash = sccp_lane_id_hash_v1(identity.lane).expect("valid lane");
        let emitter_hash =
            sccp_source_emitter_identity_hash_v1(&identity.emitter).expect("valid emitter");
        let identity_hash = sccp_source_identity_hash_v1(&identity).expect("valid identity");

        assert_eq!(
            sccp_network_identity_hash_v1(SccpNetworkV1::EthereumMainnet),
            crate::decode_fixed_hex_bytes::<32>(
                "e8fe46a73f87245767d448aff69467a3723ad25ab9b73d7a8d018e5abd8eae89",
            )
            .expect("network hash vector"),
        );
        assert_eq!(
            lane_hash,
            crate::decode_fixed_hex_bytes::<32>(
                "647fa252d0f3a6a52574ff713b02f80a35542af4abc77de6d36eef2b18a62819",
            )
            .expect("lane hash vector"),
        );
        assert_eq!(
            emitter_hash,
            crate::decode_fixed_hex_bytes::<32>(
                "9483fb04d05f7e03331f8a6e123f47cdd81f1431770f8e2b2a11ce29e3431674",
            )
            .expect("emitter hash vector"),
        );
        assert_eq!(
            identity_hash,
            crate::decode_fixed_hex_bytes::<32>(
                "10ee71fd8d22ce7ba84f72eefe154cd21cf5414fdd7168719338a22d905ec27f",
            )
            .expect("source identity hash vector"),
        );
    }

    #[test]
    fn lane_hash_is_directional_and_rejects_invalid_topologies() {
        let inbound = SccpLaneIdV1 {
            source: SccpNetworkV1::EthereumMainnet,
            target: SccpNetworkV1::SoraTaira,
        };
        let outbound = SccpLaneIdV1 {
            source: SccpNetworkV1::SoraTaira,
            target: SccpNetworkV1::EthereumMainnet,
        };
        assert_ne!(
            sccp_lane_id_hash_v1(inbound),
            sccp_lane_id_hash_v1(outbound)
        );
        for invalid in [
            SccpLaneIdV1 {
                source: SccpNetworkV1::SoraTaira,
                target: SccpNetworkV1::SoraTaira,
            },
            SccpLaneIdV1 {
                source: SccpNetworkV1::EthereumMainnet,
                target: SccpNetworkV1::BscMainnet,
            },
        ] {
            assert!(canonical_sccp_lane_id_bytes_v1(invalid).is_none());
            assert!(sccp_lane_id_hash_v1(invalid).is_none());
        }
    }

    #[test]
    fn network_profile_tags_are_bijective_and_reject_unknown_values() {
        let networks = [
            SccpNetworkV1::SoraTaira,
            SccpNetworkV1::EthereumMainnet,
            SccpNetworkV1::EthereumSepolia,
            SccpNetworkV1::BscMainnet,
            SccpNetworkV1::BscTestnet,
            SccpNetworkV1::TronMainnet,
            SccpNetworkV1::TronNile,
            SccpNetworkV1::TronShasta,
        ];
        let tags = networks.map(sccp_network_tag_v1);
        for (network, tag) in networks.into_iter().zip(tags) {
            assert_eq!(sccp_network_from_tag_v1(tag), Some(network));
        }
        for (index, tag) in tags.iter().enumerate() {
            assert!(tags[index + 1..].iter().all(|other| tag != other));
        }
        for unknown in core::iter::once(0).chain(6..=9).chain(13..=u8::MAX) {
            assert!(sccp_network_from_tag_v1(unknown).is_none());
        }
    }

    #[test]
    fn source_event_digest_binds_exact_profiles_and_rejects_sentinels() {
        let message_id = [0x71; 32];
        let payload_hash = [0x72; 32];
        let mainnet_taira = SccpLaneIdV1 {
            source: SccpNetworkV1::EthereumMainnet,
            target: SccpNetworkV1::SoraTaira,
        };
        let sepolia_taira = SccpLaneIdV1 {
            source: SccpNetworkV1::EthereumSepolia,
            target: SccpNetworkV1::SoraTaira,
        };
        let expected = sccp_lane_source_event_digest_v1(mainnet_taira, message_id, payload_hash)
            .expect("well-formed exact lane event");
        assert_eq!(
            expected,
            crate::decode_fixed_hex_bytes::<32>(
                "dd71a2bb21c6213d7b07c9c45197c4ff41db5cc4c059c41342b490e2879fd9a4",
            )
            .expect("lane-bound event digest vector")
        );
        assert_ne!(
            Some(expected),
            sccp_lane_source_event_digest_v1(sepolia_taira, message_id, payload_hash)
        );
        assert!(sccp_lane_source_event_digest_v1(mainnet_taira, [0; 32], payload_hash).is_none());
        assert!(sccp_lane_source_event_digest_v1(mainnet_taira, message_id, [0; 32]).is_none());
        assert!(sccp_lane_source_event_digest_v1(mainnet_taira, message_id, message_id).is_none());
        let lane_hash = sccp_lane_id_hash_v1(mainnet_taira).unwrap();
        assert!(sccp_lane_source_event_digest_v1(mainnet_taira, lane_hash, payload_hash).is_none());
        assert!(sccp_lane_source_event_digest_v1(mainnet_taira, message_id, lane_hash).is_none());
        assert!(
            sccp_lane_source_event_digest_v1(
                SccpLaneIdV1 {
                    source: SccpNetworkV1::EthereumMainnet,
                    target: SccpNetworkV1::BscMainnet,
                },
                message_id,
                payload_hash,
            )
            .is_none()
        );
    }

    #[test]
    fn identity_hash_commits_to_every_emitter_role() {
        let identity = sample_identity();
        let expected = sccp_source_identity_hash_v1(&identity).expect("valid source identity");

        let mut mutations = Vec::new();
        for field in 0..3 {
            let mut mutated = identity;
            let SccpSourceEmitterV1::Evm(SccpEvmSourceEmitterV1 {
                address,
                runtime_code_hash,
                route_config_hash,
            }) = &mut mutated.emitter
            else {
                unreachable!("sample is EVM")
            };
            match field {
                0 => address[0] ^= 1,
                1 => runtime_code_hash[0] ^= 1,
                2 => route_config_hash[0] ^= 1,
                _ => unreachable!(),
            }
            mutations.push(mutated);
        }
        for mutated in mutations {
            assert_ne!(
                sccp_source_identity_hash_v1(&mutated),
                Some(expected),
                "every governed identity role must affect the hash"
            );
        }
    }

    #[test]
    fn malformed_emitters_fail_before_hashing() {
        for emitter in [
            SccpSourceEmitterV1::Evm(SccpEvmSourceEmitterV1 {
                address: [0; 20],
                runtime_code_hash: [0x22; 32],
                route_config_hash: [0x33; 32],
            }),
            SccpSourceEmitterV1::Tron(SccpTronSourceEmitterV1 {
                address: [1; 20],
                runtime_code_hash: [2; 32],
                route_config_hash: [2; 32],
            }),
        ] {
            assert!(canonical_sccp_source_emitter_bytes_v1(&emitter).is_none());
            assert!(sccp_source_emitter_identity_hash_v1(&emitter).is_none());
        }
    }

    #[test]
    fn shared_native_transfer_event_vectors_match_exact_rust_wire() {
        fn object(value: &norito::json::Value) -> &norito::json::Map {
            value.as_object().expect("fixture object")
        }
        fn text<'a>(value: &'a norito::json::Value, key: &str) -> &'a str {
            object(value)
                .get(key)
                .and_then(norito::json::Value::as_str)
                .unwrap_or_else(|| panic!("fixture text {key}"))
        }
        fn hex(value: &str) -> Vec<u8> {
            crate::decode_hex_bytes(&format!("0x{value}")).expect("fixture lowercase hexadecimal")
        }

        let fixture = norito::json::from_str::<norito::json::Value>(include_str!(
            "../../../fixtures/sccp/native_transfer_event_v1.json"
        ))
        .expect("parse native transfer fixture");
        assert_eq!(
            object(&fixture).get("version").and_then(|v| v.as_u64()),
            Some(1)
        );
        let vectors = object(&fixture)
            .get("vectors")
            .and_then(norito::json::Value::as_array)
            .expect("fixture vectors");
        assert_eq!(vectors.len(), 7);
        for vector in vectors {
            let lane = SccpLaneIdV1 {
                source: SccpNetworkV1::from_profile_key(text(vector, "source_profile"))
                    .expect("source profile"),
                target: SccpNetworkV1::from_profile_key(text(vector, "target_profile"))
                    .expect("target profile"),
            };
            let canonical_lane = hex(text(vector, "canonical_lane_hex"));
            assert_eq!(
                canonical_sccp_lane_id_bytes_v1(lane).as_deref(),
                Some(canonical_lane.as_slice())
            );
            let lane_hash = sccp_lane_id_hash_v1(lane).expect("lane hash");
            assert_eq!(lane_hash.to_vec(), hex(text(vector, "lane_hash_hex")));
            let canonical_payload = hex(text(vector, "canonical_payload_hex"));
            let payload = crate::decode_canonical_sccp_payload_bytes(&canonical_payload)
                .expect("canonical transfer payload");
            assert!(matches!(payload, crate::SccpPayloadV1::Transfer(_)));
            let payload_hash = crate::payload_hash(&canonical_payload);
            assert_eq!(payload_hash.to_vec(), hex(text(vector, "payload_hash_hex")));
            let message_id = crate::sccp_message_id(lane, &payload).expect("message id");
            assert_eq!(message_id.to_vec(), hex(text(vector, "message_id_hex")));
            assert_eq!(
                sccp_lane_source_event_digest_v1(lane, message_id, payload_hash)
                    .expect("source event")
                    .to_vec(),
                hex(text(vector, "source_event_digest_hex"))
            );
        }
    }
}
