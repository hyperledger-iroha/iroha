use super::*;
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_primitives::json::Json;

fn account(seed: u8) -> AccountId {
    AccountId::new(
        KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    )
}

fn permission(name: &str) -> Permission {
    Permission::new(name.into(), Json::new(norito::json!(null)))
}

fn network(seed: u8) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed(
        [seed; Hash::LENGTH],
    )))
}

fn payload() -> SorafsBootstrapPayloadV1 {
    let mut grants = vec![
        SorafsBootstrapGrantV1 {
            account: account(10),
            permission: permission("CanSetSorafsReservePolicy"),
        },
        SorafsBootstrapGrantV1 {
            account: account(11),
            permission: permission("CanBindSorafsAlias"),
        },
    ];
    grants.sort();
    SorafsBootstrapPayloadV1 {
        council: vec![1, 2, 3],
        providers: vec![SorafsBootstrapProviderV1 {
            owner: account(12),
            envelope: vec![4, 5, 6],
        }],
        grants,
        compliance_operators: vec![account(13)],
    }
}

#[test]
fn payload_roundtrips_as_canonical_public_state() {
    let payload = payload();
    assert_eq!(payload.validate(), Ok(()));
    let bytes = payload.to_public_state().expect("encode");
    assert_eq!(
        SorafsBootstrapPayloadV1::from_public_state(&bytes).expect("decode"),
        payload
    );
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_eq!(
        SorafsBootstrapPayloadV1::from_public_state(&trailing),
        Err(SorafsBootstrapPayloadErrorV1::Encoding)
    );
    let json = norito::json::to_json(&payload).expect("json");
    let from_json: SorafsBootstrapPayloadV1 = norito::json::from_str(&json).expect("from json");
    assert_eq!(from_json, payload);
}

#[test]
fn session_identity_binds_the_network_and_every_payload_field() {
    let payload = payload();
    let original = payload.session_id(&network(0x21)).expect("session");
    assert_ne!(
        payload.session_id(&network(0x22)).expect("session"),
        original
    );
    let mutations: Vec<Box<dyn Fn(&mut SorafsBootstrapPayloadV1)>> = vec![
        Box::new(|p| p.council.push(7)),
        Box::new(|p| p.providers[0].owner = account(14)),
        Box::new(|p| p.providers[0].envelope.push(7)),
        Box::new(|p| {
            p.grants.pop();
        }),
        Box::new(|p| p.compliance_operators.clear()),
    ];
    for mutate in mutations {
        let mut changed = payload.clone();
        mutate(&mut changed);
        assert_ne!(
            changed.canonical_digest().expect("digest"),
            payload.canonical_digest().expect("digest")
        );
        assert_ne!(
            changed.session_id(&network(0x21)).expect("session"),
            original
        );
    }
}

#[test]
fn payload_admits_only_allowlisted_ordered_entries() {
    for name in [
        "CanManageParliament",
        "CanManageRoles",
        "CanUpgradeExecutor",
        "CanReadAllLedgerData",
        "CanProposeRuntimeChangeSet",
    ] {
        let mut changed = payload();
        changed.grants = vec![SorafsBootstrapGrantV1 {
            account: account(21),
            permission: permission(name),
        }];
        assert_eq!(
            changed.validate(),
            Err(SorafsBootstrapPayloadErrorV1::Permission),
            "{name}"
        );
    }
    let mut unordered = payload();
    unordered.grants.reverse();
    assert_eq!(
        unordered.validate(),
        Err(SorafsBootstrapPayloadErrorV1::Shape)
    );
    let mut duplicate = payload();
    duplicate
        .compliance_operators
        .push(duplicate.compliance_operators[0].clone());
    assert_eq!(
        duplicate.validate(),
        Err(SorafsBootstrapPayloadErrorV1::Shape)
    );
    let mut empty = payload();
    empty.providers.clear();
    assert_eq!(empty.validate(), Err(SorafsBootstrapPayloadErrorV1::Shape));
    let mut no_council = payload();
    no_council.council.clear();
    assert_eq!(
        no_council.validate(),
        Err(SorafsBootstrapPayloadErrorV1::Shape)
    );
    let mut too_many = payload();
    too_many.compliance_operators = (0..=SORAFS_BOOTSTRAP_MAX_COMPLIANCE_OPERATORS_V1)
        .map(|seed| account(u8::try_from(seed + 40).expect("small seed")))
        .collect();
    too_many.compliance_operators.sort();
    assert_eq!(
        too_many.validate(),
        Err(SorafsBootstrapPayloadErrorV1::Shape)
    );
}

#[test]
fn record_decoding_rejects_trailing_bytes() {
    let record = SorafsBootstrapRecordV1 {
        network_id: [1; 32],
        height: 7,
        recorded_at_unix_ms: 8,
        session_id: [2; 32],
        payload_digest: [3; 32],
        roster_hash: [4; 32],
    };
    let mut bytes = norito::encode_canonical(&record).expect("encode");
    assert_eq!(
        SorafsBootstrapRecordV1::decode_frame(&bytes).expect("canonical record"),
        record
    );
    bytes.push(0);
    assert!(SorafsBootstrapRecordV1::decode_frame(&bytes).is_err());
}
