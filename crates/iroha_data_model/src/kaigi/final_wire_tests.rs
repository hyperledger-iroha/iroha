//! Final Kaigi wire carriers reject retired hints and preserve exact field bytes.

use super::*;
use crate::kaigi::scalar::KaigiAuthorizationScalarV1;

fn scalar(byte: u8) -> KaigiAuthorizationScalarV1 {
    KaigiAuthorizationScalarV1::from_le_bytes([byte; 32]).expect("canonical test scalar")
}

#[test]
fn final_artifact_carriers_preserve_unmarked_scalars_and_reject_retired_hints() {
    let commitment = KaigiParticipantCommitment {
        commitment: scalar(0x24),
    };
    let nullifier = KaigiParticipantNullifier {
        digest: scalar(0x26),
    };
    assert_eq!(
        KaigiParticipantCommitment::decode(&mut commitment.encode().as_slice()).unwrap(),
        commitment
    );
    assert_eq!(
        KaigiParticipantNullifier::decode(&mut nullifier.encode().as_slice()).unwrap(),
        nullifier
    );
    assert_eq!(commitment.commitment.to_le_bytes(), [0x24; 32]);
    assert_eq!(nullifier.digest.to_le_bytes(), [0x26; 32]);

    let commitment_json = norito::json::to_value(&commitment).unwrap();
    let nullifier_json = norito::json::to_value(&nullifier).unwrap();
    for retired in [
        norito::json::Value::Null,
        norito::json::Value::String("participant".into()),
    ] {
        let mut value = commitment_json.clone();
        value
            .as_object_mut()
            .unwrap()
            .insert("alias_tag".into(), retired);
        assert!(norito::json::from_value::<KaigiParticipantCommitment>(value).is_err());
    }
    for retired in [0_u64, 1, u64::MAX] {
        let mut value = nullifier_json.clone();
        value
            .as_object_mut()
            .unwrap()
            .insert("issued_at_ms".into(), retired.into());
        assert!(norito::json::from_value::<KaigiParticipantNullifier>(value).is_err());
    }
    for name in ["commitment", "digest"] {
        let mut value = if name == "commitment" {
            commitment_json.clone()
        } else {
            nullifier_json.clone()
        };
        value.as_object_mut().unwrap().insert(
            name.into(),
            norito::json::Value::Array((0..32).map(|_| 255_u64.into()).collect()),
        );
        if name == "commitment" {
            assert!(norito::json::from_value::<KaigiParticipantCommitment>(value).is_err());
        } else {
            assert!(norito::json::from_value::<KaigiParticipantNullifier>(value).is_err());
        }
    }
}

#[test]
fn record_wire_requires_original_participation_ownership() {
    use iroha_crypto::{Algorithm, KeyPair};
    let host = AccountId::new(
        KeyPair::from_seed(vec![1; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    let original = AccountId::new(
        KeyPair::from_seed(vec![2; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    let id = KaigiId::new(
        DomainId::try_new("kaigi", "universal").unwrap(),
        "wire".parse().unwrap(),
    );
    let mut template = NewKaigi::with_defaults(id, host);
    template.privacy_mode = KaigiPrivacyMode::ZkRosterV1;
    let mut record = KaigiRecord::from_new(&template, 1);
    assert!(record.private_participation.entries().is_empty());
    record.host_commitment = Some(KaigiParticipantCommitment {
        commitment: scalar(3),
    });
    record
        .private_participation
        .commit_join(&original, 1, scalar(4))
        .unwrap();
    record.push_commitment(KaigiParticipantCommitment {
        commitment: scalar(4),
    });
    record.push_nullifier(KaigiParticipantNullifier { digest: scalar(5) });
    record.push_usage_commitment(scalar(6));
    record.segments_recorded = 1;
    assert_eq!(
        KaigiRecord::decode(&mut record.encode().as_slice()).unwrap(),
        record
    );
    let mut json = norito::json::to_value(&record).unwrap();
    assert_eq!(
        norito::json::from_value::<KaigiRecord>(json.clone()).unwrap(),
        record
    );
    json.as_object_mut()
        .unwrap()
        .remove("private_participation");
    assert!(norito::json::from_value::<KaigiRecord>(json).is_err());
}
