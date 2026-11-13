//! Canonical spellings and bounded streaming for standalone persisted-map keys.

use crate::{
    account::AccountId,
    consensus::{ConsensusKeyId, ConsensusKeyRole},
    nexus::{
        PrivateSettlementNullifierKeyV1, PrivateSettlementOutputKeyV1, PrivateSettlementPoolKeyV1,
        PrivateSettlementRouteV1,
    },
    privacy::{
        PrivacyCommitmentV1, PrivacyNullifierV1, PrivacyPoolIdV1, PrivacyProtocolPolicyKeyV1,
        PrivacyRuntimeProtocolIdV1,
    },
    repo::RepoAgreementId,
    role::RoleIdWithOwner,
    rwa::RwaId,
    smart_contract::ContractArtifactId,
    sorafs::pin_registry::ManifestDigest,
};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_model_base::{
    domain::DomainId,
    topology::{DataSpaceId, LaneId},
};
use norito::json::{self, BoundedJsonError, JsonKeyCodec, JsonSerialize, JsonWriteSink};

struct KeyJson<'a, T>(&'a T);
impl<T: JsonKeyCodec> JsonSerialize for KeyJson<'_, T> {
    fn json_serialize(&self, output: &mut String) {
        self.0.encode_json_key(output);
    }
    fn json_serialize_to(&self, output: &mut dyn JsonWriteSink) -> Result<(), BoundedJsonError> {
        self.0.encode_json_key_to(output)
    }
}

fn parity<T: JsonKeyCodec + PartialEq + core::fmt::Debug>(key: T) {
    let value = KeyJson(&key);
    let expected = json::to_json(&value).unwrap();
    assert_eq!(
        json::to_json_bounded(&value, expected.len()),
        Ok(expected.clone())
    );
    for cap in [0, 1, expected.len() / 2, expected.len() - 1] {
        assert_eq!(
            json::to_json_bounded(&value, cap),
            Err(BoundedJsonError::BodyTooLarge)
        );
    }
    let text: String = json::from_str(&expected).unwrap();
    assert_eq!(T::decode_json_key(&text).unwrap(), key);
}

#[test]
fn bounded_json_keys_preserve_named_hash_and_composite_identifiers() {
    parity(RwaId::new(
        DomainId::try_new("vault", "universal").unwrap(),
        Hash::new(b"rwa"),
    ));
    for text in ["repo", "quote\"\\雪|role"] {
        parity(text.parse::<RepoAgreementId>().unwrap());
        parity(RoleIdWithOwner {
            account: AccountId::new(
                KeyPair::from_seed(vec![7; 32], Algorithm::Ed25519)
                    .public_key()
                    .clone(),
            ),
            id: text.parse().unwrap(),
        });
    }
    for role in [
        ConsensusKeyRole::Validator,
        ConsensusKeyRole::Committee,
        ConsensusKeyRole::Endorsement,
    ] {
        parity(ConsensusKeyId::new(role, "validator-key"));
    }
    parity(ManifestDigest::new([0xAB; 32]));
    for scope in [0, u64::MAX] {
        parity(ContractArtifactId::new(
            DataSpaceId::new(scope),
            Hash::new(b"artifact"),
        ));
    }
}

#[test]
fn bounded_json_keys_preserve_protocol_policy_frame_and_object_key() {
    let key =
        PrivacyProtocolPolicyKeyV1::new(PrivacyRuntimeProtocolIdV1::IrohaIvmPrivateNoteStarkV1);
    parity(key);
    let map = std::collections::BTreeMap::from([(key, 1_u64)]);
    let expected = json::to_json(&map).unwrap();
    assert_eq!(
        json::to_json_bounded(&map, expected.len()),
        Ok(expected.clone())
    );
    assert_eq!(
        json::to_json_bounded(&map, expected.len() - 1),
        Err(BoundedJsonError::BodyTooLarge)
    );
}

#[test]
fn bounded_json_keys_preserve_all_private_settlement_frames() {
    let pool = PrivateSettlementPoolKeyV1 {
        route: PrivateSettlementRouteV1 {
            dataspace_id: DataSpaceId::new(u64::MAX),
            lane_id: LaneId::new(u32::MAX),
            lane_incarnation: Hash::new(b"incarnation"),
        },
        pool_id: PrivacyPoolIdV1::new([0xAB; 32]),
    };
    parity(pool);
    parity(PrivateSettlementNullifierKeyV1 {
        pool,
        nullifier: PrivacyNullifierV1::new([0xCD; 32]),
    });
    parity(PrivateSettlementOutputKeyV1 {
        pool,
        commitment: PrivacyCommitmentV1::new([0xEF; 32]),
    });
}
