//! Nexus endorsement configuration parsing tests.

use super::*;

fn committee_key(seed: u8) -> PublicKey {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .expect("derive deterministic committee key")
        .public_key()
        .clone()
}

#[test]
fn parse_canonicalizes_committee_as_a_unique_key_set() {
    let first = committee_key(0x31);
    let second = committee_key(0x32);
    let config = NexusEndorsement {
        committee_keys: vec![format!(" {second} "), first.to_string(), first.to_string()],
        quorum: 2,
    };
    let mut emitter = Emitter::new();
    let parsed = config.parse(&mut emitter).expect("valid committee");
    assert!(emitter.into_result().is_ok());
    let expected = BTreeSet::from([first, second]);
    assert_eq!(parsed.committee_keys, expected);
    assert_eq!(parsed.quorum, 2);
}

#[test]
fn parse_rejects_invalid_committee_keys() {
    let config = NexusEndorsement {
        committee_keys: vec!["not-a-public-key".to_owned()],
        quorum: 0,
    };
    let mut emitter = Emitter::new();
    assert!(config.parse(&mut emitter).is_none());
    assert!(emitter.into_result().is_err());
}

#[test]
fn parse_accepts_empty_committee_when_enforcement_is_disabled() {
    let config = NexusEndorsement {
        committee_keys: Vec::new(),
        quorum: 0,
    };
    let mut emitter = Emitter::new();
    let parsed = config.parse(&mut emitter).expect("disabled committee");
    assert!(emitter.into_result().is_ok());
    assert!(parsed.committee_keys.is_empty());
    assert_eq!(parsed.quorum, 0);
}

#[test]
fn parse_rejects_nonzero_quorum_above_unique_member_count() {
    let member = committee_key(0x33);
    let config = NexusEndorsement {
        committee_keys: vec![member.to_string(), member.to_string()],
        quorum: 2,
    };
    let mut emitter = Emitter::new();
    assert!(config.parse(&mut emitter).is_none());
    assert!(emitter.into_result().is_err());
}
