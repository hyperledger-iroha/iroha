//! Exercise retained confidential native entrypoints without generating a proof.

use super::*;

#[test]
fn retained_confidential_builders_reject_invalid_network_before_proving() {
    let transfer = super::build_confidential_transfer_proof_v2(
        Uint8Array::from(vec![0; 31]),
        String::new(),
        Uint8Array::from(vec![0; 32]),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        String::new(),
        String::new(),
        String::new(),
        Uint8Array::from(Vec::new()),
    )
    .map(|_| ());
    let unshield = super::build_confidential_unshield_proof_v2(
        Uint8Array::from(vec![0; 31]),
        String::new(),
        Uint8Array::from(vec![0; 32]),
        Vec::new(),
        Vec::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        Uint8Array::from(Vec::new()),
    )
    .map(|_| ());
    let unshield_with_change = super::build_confidential_unshield_proof_v3(
        Uint8Array::from(vec![0; 31]),
        String::new(),
        Uint8Array::from(vec![0; 32]),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        Uint8Array::from(Vec::new()),
    )
    .map(|_| ());
    for result in [transfer, unshield, unshield_with_change] {
        let error = result.expect_err("invalid network must fail before proof construction");
        assert_eq!(error.status, napi::Status::InvalidArg);
        assert_eq!(
            error.reason,
            "networkId must contain exactly 32 genesis-header hash bytes"
        );
    }
}
