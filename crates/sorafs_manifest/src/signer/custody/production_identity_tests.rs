//! Public identity grammar remains separate from actual custody and trust qualification.
use super::*;

fn binding() -> SignerCustodyBindingV1 {
    SignerCustodyBindingV1 {
        chain_id: "promotion-chain".into(),
        network_id: [0x11; 32],
        runtime_handle: "hsm://sorafs/attestation/latest".into(),
        key_handle: "pkcs11:production/contest/key-7".into(),
        service_id: "account-attester".into(),
        administrator_id: "attestation-security".into(),
        role: SignerRoleV1::FinalPromotionAccountTransaction,
        purpose: SignerPurposeBindingV1::FinalPromotionAccountTransaction {
            deployment_id: "latest-contest".into(),
        },
        algorithm: SignerKeyAlgorithmV1::Ed25519,
        public_key: iroha_crypto::KeyPair::try_from_seed(vec![0x21; 32], Algorithm::Ed25519)
            .unwrap()
            .public_key()
            .clone(),
        key_revision: 7,
        policy_revision: 9,
        policy_digest: [0x41; 32],
    }
}

#[test]
fn custody_binding_and_authority_accept_real_words_without_weakening_independence() {
    let original = binding();
    original.validate().unwrap();
    let authority = SignerCustodyAuthorityV1 {
        service_id: "signer-authority".into(),
        administrator_id: "latest-attestation".into(),
        key_revision: 1,
        policy_revision: 1,
        policy_digest: [0x52; 32],
    };
    assert!(valid_authority(&authority));
    let mut duplicate = original.clone();
    duplicate.administrator_id = duplicate.service_id.clone();
    assert!(duplicate.validate().is_err());
    let mut duplicate = authority.clone();
    duplicate.administrator_id = duplicate.service_id.clone();
    assert!(!valid_authority(&duplicate));
    for reserved in [
        "null",
        "mock",
        "test",
        "dev",
        "demo",
        "fake",
        "dummy",
        "placeholder",
    ] {
        for administrator in [false, true] {
            let marked = format!("production-{}-primary", reserved.to_ascii_uppercase());
            let mut changed = original.clone();
            let mut changed_authority = authority.clone();
            if administrator {
                changed.administrator_id = marked.clone();
                changed_authority.administrator_id = marked;
            } else {
                changed.service_id = marked.clone();
                changed_authority.service_id = marked;
            }
            assert!(changed.validate().is_err());
            assert!(!valid_authority(&changed_authority));
        }
    }
}

#[test]
fn signer_handles_use_exact_reserved_components_and_keep_credential_and_size_rules() {
    for scheme in ["software", "signer", "hsm", "kms", "pkcs11"] {
        for word in ["attester", "attestation", "latest", "contest"] {
            assert!(valid_custody_handle(&format!(
                "{scheme}://production/{word}"
            )));
        }
        for reserved in [
            "null",
            "mock",
            "test",
            "dev",
            "demo",
            "fake",
            "dummy",
            "placeholder",
        ] {
            assert!(!valid_custody_handle(&format!(
                "{scheme}://production/{}",
                reserved.to_ascii_uppercase()
            )));
        }
        let boundary = format!(
            "{scheme}:{}",
            "a".repeat(MAX_HANDLE_BYTES_V1 - scheme.len() - 1)
        );
        assert!(valid_custody_handle(&boundary));
        assert!(!valid_custody_handle(&format!("{boundary}a")));
    }
    for value in [
        "hsm://operator:secret@attester",
        "hsm://attester?key=secret",
        "hsm://attester%2fkey",
        "HSM://attester",
        "hsm://attester//key",
    ] {
        assert!(!valid_custody_handle(value));
    }
}
