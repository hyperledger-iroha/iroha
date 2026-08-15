use super::*;
use crate::vega::zk_ams::mkhe::ZkAmsMkhePartyIdV1;

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
