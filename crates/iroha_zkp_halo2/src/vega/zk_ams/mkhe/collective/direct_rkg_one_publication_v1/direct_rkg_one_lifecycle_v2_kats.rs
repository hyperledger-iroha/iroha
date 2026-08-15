use super::{
    DirectRkgOnePublicationScopeV1, RECORD_BYTES_V2, record_v2,
    tests::{fixture_records_v2, fixture_scope_v2},
};
use crate::vega::zk_ams::mkhe::ZkAmsMkhePartyIdV1;
use std::collections::BTreeSet;

const STORAGE_KEY_KAT_V2: &str = "3b0e7afc152418fd7158c31aaf3a8e2c13a18be6da045cb2a7abe5b44485e992";
const FRESH_PREFIX_KAT_V2: &str = "52314c320102000000000000000000003b0e7afc152418fd7158c31aaf3a8e2c13a18be6da045cb2a7abe5b44485e992111111111111111111111111111111111111111111111111111111111111111103072222222222222222222222222222222222222222222222222222222222222222";
const FRESH_DIGEST_KAT_V2: &str =
    "bfb6a3fb2312ba90259ce2fe87e1efa8ff9e6487af2dd9a68d4d6003bc8cad01";
const PUBLISHED_DIGEST_KAT_V2: &str =
    "91f99a3b83ff0a1ad1dd545dcdb988a169e1defd5a8e9f654e241d369b679570";
const PROOF_DIGEST_KAT_V2: &str =
    "c12c8bbdc303e823f777dc5cee6b61319fa8a4e83d1f12373176a74087bdab9f";

#[test]
fn fresh_lifecycle_storage_key_and_record_digest_are_stable() {
    let scope = DirectRkgOnePublicationScopeV1 {
        context_digest: [0x11; 32],
        party_index: 2,
        digit_index: 3,
        party: ZkAmsMkhePartyIdV1::new([0x22; 32]).expect("nonzero party"),
    };
    let key = record_v2::stable_storage_key_v2(scope).expect("stable key");
    assert_eq!(
        hex::encode(key),
        "d35114e0762cb1ee4986859e8380738d6c0f052ed70cf00c111dcc159e2efd5b"
    );
    let mut record = [0; RECORD_BYTES_V2];
    record_v2::encode_fresh_v2(scope, key, &mut record).expect("fresh record");
    assert_eq!(
        hex::encode(&record[608..]),
        "b6be7850c6526c05df47678a9a1bb5ff93e36a1736b19d677e5d75310fa6763f"
    );
}

#[test]
fn stable_key_and_all_lifecycle_record_checksums_are_literal() {
    let scope = fixture_scope_v2();
    let key = record_v2::stable_storage_key_v2(scope).expect("stable key");
    assert_eq!(hex::encode(key), STORAGE_KEY_KAT_V2);

    let (fresh, published, proof) = fixture_records_v2();
    assert_eq!(fresh.len(), 640);
    assert_eq!(hex::encode(&fresh[..114]), FRESH_PREFIX_KAT_V2);
    assert_eq!(fresh[114..608], [0; 494]);
    for (record, digest) in [
        (&fresh, FRESH_DIGEST_KAT_V2),
        (&published, PUBLISHED_DIGEST_KAT_V2),
        (&proof, PROOF_DIGEST_KAT_V2),
    ] {
        assert_eq!(hex::encode(&record[608..]), digest);
        assert_eq!(record[608..], record_v2::record_digest_v2(record));
    }
}

#[test]
fn stable_key_binds_context_party_slot_digit_and_party_identity() {
    let scope = fixture_scope_v2();
    let mut scopes = [scope; 5];
    scopes[1].context_digest[0] ^= 1;
    scopes[2].party_index += 1;
    scopes[3].digit_index += 1;
    scopes[4].party = ZkAmsMkhePartyIdV1::new([0x23; 32]).expect("nonzero party");
    let keys = scopes
        .map(|value| record_v2::stable_storage_key_v2(value).expect("valid scope"))
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(keys.len(), scopes.len());
}
