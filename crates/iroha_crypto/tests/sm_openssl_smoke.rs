//! Smoke tests for the OpenSSL-backed SM crypto provider.
#![cfg(all(feature = "sm", feature = "sm-ffi-openssl"))]

use hex::decode;
use iroha_crypto::sm::{
    OpenSslProvider, OpenSslSmBackend, Sm2PublicKey, Sm2Signature, Sm3Digest, Sm4Key,
    openssl_sm::OpenSslSmError,
};

const ANNEX_PUBLIC_KEY_HEX: &str = "040AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857";
const ANNEX_SIGNATURE_HEX: &str = "40F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D16FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7";
const ANNEX_DISTID: &str = "ALICE123@YAHOO.COM";
const ANNEX_MESSAGE: &[u8] = b"message digest";

struct PreviewGuard {
    previous: bool,
}

impl PreviewGuard {
    fn enable() -> Self {
        let previous = OpenSslProvider::is_enabled();
        OpenSslProvider::set_preview_enabled(true);
        Self { previous }
    }
}

impl Drop for PreviewGuard {
    fn drop(&mut self) {
        OpenSslProvider::set_preview_enabled(self.previous);
    }
}

#[test]
fn openssl_sm3_digest_matches_pure_rust() {
    if !OpenSslProvider::is_available() {
        eprintln!("skipping OpenSSL SM3 smoke: OpenSSL runtime symbols unavailable");
        return;
    }

    let _preview = PreviewGuard::enable();

    let message = b"iroha sm openssl smoke test";
    let expected = Sm3Digest::hash(message);
    let first = OpenSslSmBackend::sm3_digest(message).expect("OpenSSL SM3 digest must succeed");
    let second =
        OpenSslSmBackend::sm3_digest(message).expect("second OpenSSL SM3 digest must succeed");

    assert_eq!(
        first,
        *expected.as_bytes(),
        "OpenSSL SM3 digest must match pure-Rust implementation"
    );
    assert_eq!(
        first, second,
        "OpenSSL SM3 digest must be deterministic across calls"
    );
}

#[test]
fn openssl_sm2_verify_smoke() {
    if !OpenSslProvider::is_available() {
        eprintln!("skipping OpenSSL SM2 smoke: OpenSSL runtime symbols unavailable");
        return;
    }

    let _preview = PreviewGuard::enable();
    let public_key_bytes =
        decode(ANNEX_PUBLIC_KEY_HEX).expect("Annex Example public key hex must decode");
    let signature =
        Sm2Signature::from_hex(ANNEX_SIGNATURE_HEX).expect("Annex Example signature must decode");

    let public_key = match Sm2PublicKey::from_sec1_bytes(ANNEX_DISTID, &public_key_bytes) {
        Ok(key) => key,
        Err(err) => {
            eprintln!(
                "skipping OpenSSL SM2 smoke: unable to parse Annex Example public key ({err})"
            );
            return;
        }
    };

    public_key
        .verify(ANNEX_MESSAGE, &signature)
        .expect("pure-Rust SM2 verification path must accept Annex Example 1");

    let result =
        OpenSslSmBackend::sm2_verify(&public_key_bytes, ANNEX_DISTID, ANNEX_MESSAGE, &signature)
            .expect("OpenSSL SM2 verification must succeed");

    assert!(
        result,
        "OpenSSL SM2 verification must accept Annex Example 1 when available"
    );

    let repeat =
        OpenSslSmBackend::sm2_verify(&public_key_bytes, ANNEX_DISTID, ANNEX_MESSAGE, &signature)
            .expect("repeated OpenSSL SM2 verification must succeed");
    assert_eq!(
        result, repeat,
        "OpenSSL SM2 verification must be deterministic across runs"
    );
}

#[test]
fn openssl_sm4_gcm_roundtrip() {
    if !OpenSslProvider::is_available() {
        eprintln!("skipping OpenSSL SM4 smoke: OpenSSL runtime symbols unavailable");
        return;
    }

    let _preview = PreviewGuard::enable();

    match openssl::cipher::Cipher::fetch(None, "SM4-GCM", None) {
        Ok(_) => {}
        Err(err) => eprintln!("OpenSSL Cipher::fetch(SM4-GCM) failed: {err}"),
    }

    let key_bytes = [0x11; 16];
    let key = Sm4Key::new(key_bytes);
    let nonce = [0x22; 12];
    let aad = b"sm4-openssl-aad";
    let plaintext = b"preview-open-ssl-sm4-gcm";

    let (expected_ciphertext, expected_tag) = key
        .encrypt_gcm(&nonce, aad, plaintext)
        .expect("Rust SM4 GCM encrypt must succeed");

    let (ciphertext, tag) =
        match OpenSslSmBackend::sm4_gcm_encrypt(&key_bytes, &nonce, aad, plaintext) {
            Ok(pair) => pair,
            Err(OpenSslSmError::Sm4GcmNotImplemented) => {
                eprintln!("skipping OpenSSL SM4 GCM smoke: SM4-GCM not implemented in provider");
                return;
            }
            Err(err) => panic!("OpenSSL SM4 GCM encrypt failed: {err}"),
        };

    assert_eq!(
        ciphertext, expected_ciphertext,
        "OpenSSL SM4 GCM ciphertext must match Rust implementation"
    );
    assert_eq!(
        tag, expected_tag,
        "OpenSSL SM4 GCM tag must match Rust implementation"
    );

    let decrypted =
        match OpenSslSmBackend::sm4_gcm_decrypt(&key_bytes, &nonce, aad, &ciphertext, &tag) {
            Ok(plaintext_out) => plaintext_out,
            Err(OpenSslSmError::Sm4GcmNotImplemented) => {
                eprintln!("skipping OpenSSL SM4 GCM smoke: SM4-GCM not implemented in provider");
                return;
            }
            Err(err) => panic!("OpenSSL SM4 GCM decrypt failed: {err}"),
        };

    assert_eq!(
        decrypted, plaintext,
        "OpenSSL SM4 GCM decrypt must recover original plaintext"
    );
}

#[test]
fn openssl_sm4_gcm_rejects_invalid_params() {
    if !OpenSslProvider::is_available() {
        eprintln!("skipping OpenSSL SM4 error smoke: OpenSSL runtime symbols unavailable");
        return;
    }

    let _preview = PreviewGuard::enable();
    let nonce = [0x33; 12];
    let aad = b"sm4-openssl-invalid-params";
    let plaintext = b"invalid-parameters";

    let err = OpenSslSmBackend::sm4_gcm_encrypt(&[0xAA; 15], &nonce, aad, plaintext)
        .expect_err("SM4-GCM encrypt must reject short key");
    assert!(matches!(err, OpenSslSmError::InvalidKeyLength(15)));

    let err = OpenSslSmBackend::sm4_gcm_encrypt(&[0xAA; 16], &[0x44; 11], aad, plaintext)
        .expect_err("SM4-GCM encrypt must reject short nonce");
    assert!(matches!(err, OpenSslSmError::InvalidNonceLength(11)));

    let (ciphertext, tag) =
        match OpenSslSmBackend::sm4_gcm_encrypt(&[0xAA; 16], &nonce, aad, plaintext) {
            Ok(pair) => pair,
            Err(OpenSslSmError::Sm4GcmNotImplemented) => {
                eprintln!("skipping OpenSSL SM4 error smoke: SM4-GCM not implemented in provider");
                return;
            }
            Err(err) => panic!("OpenSSL SM4 GCM encrypt failed: {err}"),
        };
    let mut bad_tag = tag;
    bad_tag[0] ^= 0xFF;
    let err = OpenSslSmBackend::sm4_gcm_decrypt(&[0xAA; 16], &nonce, aad, &ciphertext, &bad_tag)
        .expect_err("SM4-GCM decrypt must reject modified tag");
    assert!(matches!(err, OpenSslSmError::OpenSsl(_)));
}

#[cfg(feature = "sm-ccm")]
#[test]
fn openssl_sm4_ccm_roundtrip() {
    if !OpenSslProvider::is_available() {
        eprintln!("skipping OpenSSL SM4 CCM smoke: OpenSSL runtime symbols unavailable");
        return;
    }

    let _preview = PreviewGuard::enable();

    match openssl::cipher::Cipher::fetch(None, "SM4-CCM", None) {
        Ok(_) => {}
        Err(err) => eprintln!("OpenSSL Cipher::fetch(SM4-CCM) failed: {err}"),
    }

    let key_bytes = [0x33; 16];
    let key = Sm4Key::new(key_bytes);
    let nonce = [0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16];
    let aad = b"sm4-openssl-ccm-aad";
    let plaintext = b"preview-openssl-sm4-ccm";
    let tag_len = 10usize;

    let (expected_ciphertext, expected_tag) = key
        .encrypt_ccm(&nonce, aad, plaintext, tag_len)
        .expect("Rust SM4 CCM encrypt must succeed");

    let (ciphertext, tag) =
        match OpenSslSmBackend::sm4_ccm_encrypt(&key_bytes, &nonce, aad, plaintext, tag_len) {
            Ok(pair) => pair,
            Err(OpenSslSmError::Sm4CcmNotImplemented) => {
                eprintln!("skipping OpenSSL SM4 CCM smoke: SM4-CCM not implemented in provider");
                return;
            }
            Err(err) => panic!("OpenSSL SM4 CCM encrypt failed: {err}"),
        };

    assert_eq!(
        ciphertext, expected_ciphertext,
        "OpenSSL SM4 CCM ciphertext must match Rust implementation"
    );
    assert_eq!(
        tag, expected_tag,
        "OpenSSL SM4 CCM tag must match Rust implementation"
    );

    let decrypted =
        match OpenSslSmBackend::sm4_ccm_decrypt(&key_bytes, &nonce, aad, &ciphertext, &tag) {
            Ok(plaintext_out) => plaintext_out,
            Err(OpenSslSmError::Sm4CcmNotImplemented) => {
                eprintln!("skipping OpenSSL SM4 CCM smoke: SM4-CCM not implemented in provider");
                return;
            }
            Err(err) => panic!("OpenSSL SM4 CCM decrypt failed: {err}"),
        };

    assert_eq!(
        decrypted, plaintext,
        "OpenSSL SM4 CCM decrypt must recover original plaintext"
    );
}

#[cfg(feature = "sm-ccm")]
#[test]
fn openssl_sm4_ccm_rejects_invalid_params() {
    if !OpenSslProvider::is_available() {
        eprintln!("skipping OpenSSL SM4 CCM error smoke: OpenSSL runtime symbols unavailable");
        return;
    }

    let _preview = PreviewGuard::enable();
    let aad = b"sm4-openssl-ccm-invalid";
    let plaintext = b"invalid-params-sm4-ccm";
    let nonce = [0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26];

    let err = OpenSslSmBackend::sm4_ccm_encrypt(&[0x55; 16], &[0x44; 6], aad, plaintext, 10)
        .expect_err("SM4-CCM encrypt must reject nonce shorter than 7 bytes");
    assert!(matches!(err, OpenSslSmError::InvalidCcmNonceLength(6)));

    let err = OpenSslSmBackend::sm4_ccm_encrypt(&[0x55; 16], &nonce, aad, plaintext, 5)
        .expect_err("SM4-CCM encrypt must reject unsupported tag length");
    assert!(matches!(err, OpenSslSmError::InvalidCcmTagLength(5)));

    let (ciphertext, tag) =
        match OpenSslSmBackend::sm4_ccm_encrypt(&[0x55; 16], &nonce, aad, plaintext, 12) {
            Ok(pair) => pair,
            Err(OpenSslSmError::Sm4CcmNotImplemented) => {
                eprintln!(
                    "skipping OpenSSL SM4 CCM error smoke: SM4-CCM not implemented in provider"
                );
                return;
            }
            Err(err) => panic!("OpenSSL SM4 CCM encrypt failed: {err}"),
        };

    let mut bad_tag = tag.clone();
    bad_tag[0] ^= 0xAA;
    let err = OpenSslSmBackend::sm4_ccm_decrypt(&[0x55; 16], &nonce, aad, &ciphertext, &bad_tag)
        .expect_err("SM4-CCM decrypt must reject modified tag");
    assert!(matches!(err, OpenSslSmError::OpenSsl(_)));
}
