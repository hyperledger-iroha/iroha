//! Complete immutable public pin identity and programmatic configuration admission.
use super::hardware_test_support::{CHAIN, NETWORK, SignedFixture, TestSignerMode};
use super::*;

type ConfigMutation = fn(&mut actual::SorafsStreamTokenHardwareConfig);
fn replacement_key() -> [u8; 32] {
    iroha_crypto::KeyPair::try_from_seed(vec![0xe7; 32], iroha_crypto::Algorithm::Ed25519)
        .unwrap()
        .public_key()
        .try_to_bytes()
        .unwrap()
        .1
        .try_into()
        .unwrap()
}
#[test]
fn every_immutable_hardware_attester_observer_pin_changes_the_catalog_digest() {
    let fixture = SignedFixture::new(3, TestSignerMode::Sign);
    let baseline = fixture.pins.config_digest();
    let mutations: &[ConfigMutation] = &[
        |h| h.runtime_handle.push_str("-other"),
        |h| h.key_handle.push_str("-other"),
        |h| h.service_id.push_str("-other"),
        |h| h.administrator_id.push_str("-other"),
        |h| h.public_key = replacement_key(),
        |h| h.key_revision += 1,
        |h| h.policy_revision += 1,
        |h| h.policy_digest[0] ^= 1,
        |h| h.attester.authority.service_id.push_str("-other"),
        |h| h.attester.authority.administrator_id.push_str("-other"),
        |h| h.attester.authority.public_key = replacement_key(),
        |h| h.attester.authority.key_revision += 1,
        |h| h.attester.authority.policy_revision += 1,
        |h| h.attester.authority.policy_digest[0] ^= 1,
        |h| h.attester.authority.active_from_unix_ms -= 1,
        |h| h.attester.authority.active_until_unix_ms += 1,
        |h| h.attester.max_validity_ms -= 1,
        |h| h.attester.max_anchor_age_ms -= 1,
        |h| h.observer.runtime_handle.push_str("-other"),
        |h| h.observer.authority.service_id.push_str("-other"),
        |h| h.observer.authority.administrator_id.push_str("-other"),
        |h| h.observer.authority.public_key = replacement_key(),
        |h| h.observer.authority.key_revision += 1,
        |h| h.observer.authority.policy_revision += 1,
        |h| h.observer.authority.policy_digest[0] ^= 1,
        |h| h.observer.authority.active_from_unix_ms -= 1,
        |h| h.observer.authority.active_until_unix_ms += 1,
        |h| h.observer.max_state_age_ms -= 1,
    ];
    for (index, mutate) in mutations.iter().enumerate() {
        let mut storage = fixture.storage.clone();
        mutate(storage.stream_tokens.hardware.as_mut().unwrap());
        let changed = StreamTokenHardwarePinsV1::from_config(&storage, CHAIN, NETWORK)
            .unwrap()
            .unwrap();
        assert_ne!(
            changed.config_digest(),
            baseline,
            "public pin {index} omitted from identity"
        );
    }
    let mut storage = fixture.storage.clone();
    storage.provider_id.as_mut().unwrap().0[0] ^= 1;
    for (storage, chain, network) in [
        (&storage, CHAIN, NETWORK),
        (&fixture.storage, "other-chain", NETWORK),
        (&fixture.storage, CHAIN, [0x9a; 32]),
    ] {
        assert_ne!(
            StreamTokenHardwarePinsV1::from_config(storage, chain, network)
                .unwrap()
                .unwrap()
                .config_digest(),
            baseline
        );
    }
    use norito::core::header_flags::{COMPACT_LEN, FIELD_BITSET, PACKED_SEQ, PACKED_STRUCT};
    for flags in [
        0,
        COMPACT_LEN,
        PACKED_SEQ,
        PACKED_SEQ | COMPACT_LEN,
        PACKED_STRUCT,
        PACKED_STRUCT | COMPACT_LEN,
        PACKED_SEQ | PACKED_STRUCT,
        PACKED_SEQ | PACKED_STRUCT | COMPACT_LEN,
        PACKED_STRUCT | COMPACT_LEN | FIELD_BITSET,
        PACKED_SEQ | PACKED_STRUCT | COMPACT_LEN | FIELD_BITSET,
    ] {
        let _layout = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(
            StreamTokenHardwarePinsV1::from_config(&fixture.storage, CHAIN, NETWORK)
                .unwrap()
                .unwrap()
                .config_digest(),
            baseline
        );
    }
}
#[test]
fn programmatic_hardware_config_cannot_bypass_independence_or_bounded_eligibility() {
    let fixture = SignedFixture::new(3, TestSignerMode::Sign);
    let mutations: &[ConfigMutation] = &[
        |h| h.key_revision = u64::from(u32::MAX) + 1,
        |h| h.observer.runtime_handle = h.runtime_handle.clone(),
        |h| h.observer.runtime_handle = "software:prod/observer".into(),
        |h| h.observer.authority.public_key = h.public_key,
        |h| h.attester.authority.public_key = h.public_key,
        |h| h.observer.authority.public_key = h.attester.authority.public_key,
        |h| h.observer.authority.service_id = h.administrator_id.clone(),
        |h| h.observer.authority.administrator_id = h.attester.authority.service_id.clone(),
        |h| h.attester.authority.service_id = h.service_id.clone(),
        |h| h.attester.authority.administrator_id = h.attester.authority.service_id.clone(),
        |h| h.observer.authority.service_id = "a".repeat(129),
        |h| h.attester.authority.active_from_unix_ms = 0,
        |h| h.observer.authority.active_until_unix_ms = h.observer.authority.active_from_unix_ms,
        |h| h.attester.max_validity_ms = 86_400_001,
        |h| h.attester.max_anchor_age_ms = 0,
        |h| h.observer.max_state_age_ms = 300_001,
    ];
    for mutate in mutations {
        let mut storage = fixture.storage.clone();
        mutate(storage.stream_tokens.hardware.as_mut().unwrap());
        assert!(matches!(
            StreamTokenHardwarePinsV1::from_config(&storage, CHAIN, NETWORK),
            Err(StreamTokenIssuerError::InvalidHardwareConfig)
        ));
    }
    let mut storage = fixture.storage.clone();
    storage
        .stream_tokens
        .hardware
        .as_mut()
        .unwrap()
        .key_revision = u64::from(u32::MAX);
    assert_eq!(
        StreamTokenHardwarePinsV1::from_config(&storage, CHAIN, NETWORK)
            .unwrap()
            .unwrap()
            .binding()
            .key_revision,
        u64::from(u32::MAX)
    );
    storage.provider_id.as_mut().unwrap().0 = [0; 32];
    assert!(StreamTokenHardwarePinsV1::from_config(&storage, CHAIN, NETWORK).is_err());
    assert!(!format!("{:?}", fixture.pins).contains(&fixture.pins.binding().key_handle));
}
