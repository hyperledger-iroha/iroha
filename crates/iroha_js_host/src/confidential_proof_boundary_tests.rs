//! Exercise canonical confidential entrypoints without generating a proof.
use super::*;

#[test]
fn canonical_confidential_wallet_rejects_invalid_network_before_proving() {
    let transfer = confidential_wallet::prove_confidential_transfer(
        Uint8Array::from(vec![0; 31]),
        String::new(),
        Uint8Array::from(vec![0; 32]),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        String::new(),
    )
    .map(|_| ());
    let redemption = confidential_wallet::prove_confidential_redemption(
        Uint8Array::from(vec![0; 31]),
        String::new(),
        Uint8Array::from(vec![0; 32]),
        Vec::new(),
        Vec::new(),
        String::new(),
        String::new(),
        None,
    )
    .map(|_| ());
    let change = confidential_wallet::prove_confidential_redemption(
        Uint8Array::from(vec![0; 31]),
        String::new(),
        Uint8Array::from(vec![0; 32]),
        Vec::new(),
        Vec::new(),
        String::new(),
        String::new(),
        Some(JsConfidentialUnshieldOutputV3 {
            amount: String::new(),
            rho_hex: String::new(),
        }),
    )
    .map(|_| ());
    for result in [transfer, redemption, change] {
        let error = result.expect_err("invalid network must fail before proof construction");
        assert_eq!(error.status, napi::Status::InvalidArg);
        assert_eq!(
            error.reason,
            "networkId must contain exactly 32 genesis-header hash bytes"
        );
    }
}
