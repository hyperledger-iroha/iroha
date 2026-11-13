//! Signature verification helper and syscall tests using TLVs.
use crate::common;
use common::assemble;
use ivm::{IVM, Memory, PointerType, VMError, encoding};
const ED25519_SMALL_ORDER_POINT: [u8; 32] = [
    1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
const ED25519_NON_CANONICAL_IDENTITY: [u8; 32] = [
    0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f,
];
const ED25519_NON_CANONICAL_NON_SMALL_ORDER_POINT: [u8; 32] = [
    0xf0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f,
];
fn make_tlv(type_id: u16, payload: &[u8]) -> Vec<u8> {
    use iroha_crypto::Hash;
    let mut out = Vec::with_capacity(7 + payload.len() + 32);
    out.extend_from_slice(&type_id.to_be_bytes());
    out.push(1); // version
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload.as_ref());
    let h: [u8; 32] = Hash::new(payload).into();
    out.extend_from_slice(&h);
    out
}
fn ed25519_test_key(tag: u8) -> ed25519_dalek::SigningKey {
    ed25519_dalek::SigningKey::from_bytes(&[tag; 32])
}
fn ed25519_signature_with_replacement_r(
    signing_key: &ed25519_dalek::SigningKey,
    message: &[u8],
    replacement_r: &[u8; 32],
) -> [u8; 64] {
    use ed25519_dalek::Signer;
    let mut signature = signing_key.sign(message).to_bytes();
    signature[..32].copy_from_slice(replacement_r);
    signature
}
fn run_syscall_verify_signature_blob(
    message: &[u8],
    signature: &[u8],
    public_key: &[u8],
    scheme: u64,
) -> u64 {
    let msg_tlv = make_tlv(PointerType::Blob as u16, message);
    let sig_tlv = make_tlv(PointerType::Blob as u16, signature);
    let pk_tlv = make_tlv(PointerType::Blob as u16, public_key);
    let mut vm = IVM::new(10_000);
    vm.memory.preload_input(0, &msg_tlv).expect("preload input");
    let p_msg = Memory::INPUT_START;
    let p_sig = p_msg + msg_tlv.len() as u64 + 8;
    let p_pk = p_sig + sig_tlv.len() as u64 + 8;
    vm.memory
        .preload_input(msg_tlv.len() as u64 + 8, &sig_tlv)
        .expect("preload input");
    vm.memory
        .preload_input((msg_tlv.len() + sig_tlv.len()) as u64 + 16, &pk_tlv)
        .expect("preload input");
    vm.set_register(10, p_msg);
    vm.set_register(11, p_sig);
    vm.set_register(12, p_pk);
    vm.set_register(13, scheme);
    let halt = encoding::wide::encode_halt();
    let syscall = encoding::wide::encode_scall(ivm::syscalls::SYSCALL_VERIFY_SIGNATURE);
    let mut prog = Vec::new();
    prog.extend_from_slice(&syscall.to_le_bytes());
    prog.extend_from_slice(&halt.to_le_bytes());
    let prog = assemble(&prog);
    vm.load_program(&prog).unwrap();
    vm.run().unwrap();
    vm.register(10)
}
fn run_syscall_verify_signature_ed25519(
    message_type: PointerType,
    message: &[u8],
    key_tag: u8,
) -> Result<u64, VMError> {
    use ed25519_dalek::Signer;
    let sk = ed25519_test_key(key_tag);
    let pk_bytes = sk.verifying_key().to_bytes();
    let sig = sk.sign(message);
    let msg_payload = match message_type {
        PointerType::Json => {
            let raw = std::str::from_utf8(message).expect("json message utf8");
            let json = iroha_primitives::json::Json::from_str_norito(raw).expect("valid json");
            norito::to_bytes(&json).expect("encode json payload")
        }
        _ => message.to_vec(),
    };
    let msg_tlv = make_tlv(message_type as u16, &msg_payload);
    let sig_tlv = make_tlv(PointerType::Blob as u16, &sig.to_bytes());
    let pk_tlv = make_tlv(PointerType::Blob as u16, &pk_bytes);
    let mut vm = IVM::new(10_000);
    vm.memory.preload_input(0, &msg_tlv).expect("preload input");
    let p_msg = Memory::INPUT_START;
    let p_sig = p_msg + msg_tlv.len() as u64 + 8;
    let p_pk = p_sig + sig_tlv.len() as u64 + 8;
    vm.memory
        .preload_input(msg_tlv.len() as u64 + 8, &sig_tlv)
        .expect("preload input");
    vm.memory
        .preload_input((msg_tlv.len() + sig_tlv.len()) as u64 + 16, &pk_tlv)
        .expect("preload input");
    vm.set_register(10, p_msg);
    vm.set_register(11, p_sig);
    vm.set_register(12, p_pk);
    vm.set_register(13, 1); // scheme 1 = Ed25519
    let halt = encoding::wide::encode_halt();
    let syscall = encoding::wide::encode_scall(ivm::syscalls::SYSCALL_VERIFY_SIGNATURE);
    let mut prog = Vec::new();
    prog.extend_from_slice(&syscall.to_le_bytes());
    prog.extend_from_slice(&halt.to_le_bytes());
    let prog = assemble(&prog);
    vm.load_program(&prog).unwrap();
    vm.run()?;
    Ok(vm.register(10))
}
#[test]
fn verify_signature_helper_rejects_all_zero_signature_material() {
    use ivm::signature::{SignatureScheme, verify_signature};
    let ed_key = ed25519_test_key(0x41);
    assert!(
        !verify_signature(
            SignatureScheme::Ed25519,
            b"ivm-ed25519-zero",
            &[0u8; 64],
            &ed_key.verifying_key().to_bytes(),
        ),
        "all-zero Ed25519 signature material must fail"
    );
    let keypair =
        iroha_crypto::KeyPair::try_from_seed(vec![0x65; 32], iroha_crypto::Algorithm::MlDsa)
            .expect("seeded ML-DSA fixture");
    let ml_public_key = keypair
        .public_key()
        .try_to_bytes()
        .expect("ML-DSA public key")
        .1;
    let _ = keypair.private_key();
    let ml_signature = vec![0u8; iroha_crypto::ML_DSA_65_SIGNATURE_BYTES];
    assert!(
        !verify_signature(
            SignatureScheme::MlDsa,
            b"ivm-mldsa-zero",
            &ml_signature,
            ml_public_key,
        ),
        "all-zero ML-DSA signature material must fail"
    );
}
#[test]
fn verify_signature_helper_rejects_all_zero_mldsa_public_key_material() {
    use ivm::signature::{SignatureScheme, verify_signature};
    let keypair =
        iroha_crypto::KeyPair::try_from_seed(vec![0x66; 32], iroha_crypto::Algorithm::MlDsa)
            .expect("seeded ML-DSA fixture");
    let secret_key = keypair.private_key();
    let message = b"ivm-mldsa-zero-public-key";
    let signature =
        iroha_crypto::Signature::try_new(secret_key, message).expect("sign ML-DSA fixture");
    let public_key = vec![0u8; iroha_crypto::ML_DSA_65_PUBLIC_KEY_BYTES];
    assert!(
        !verify_signature(
            SignatureScheme::MlDsa,
            message,
            signature.payload(),
            &public_key,
        ),
        "all-zero ML-DSA public-key material must fail"
    );
}
#[test]
fn syscall_verify_signature_ed25519_rejects_weak_public_key_material() {
    use ed25519_dalek::Signer;
    let message = b"ivm-ed25519-weak-public-key";
    let signing_key = ed25519_test_key(0x42);
    let signature = signing_key.sign(message);
    assert_eq!(
        run_syscall_verify_signature_blob(message, &signature.to_bytes(), &[0u8; 32], 1),
        0
    );
}
#[test]
fn syscall_verify_signature_ed25519_rejects_malformed_public_key_material() {
    use ed25519_dalek::Signer;
    let message = b"ivm-ed25519-malformed-public-key";
    let signing_key = ed25519_test_key(0x47);
    let signature = signing_key.sign(message).to_bytes();
    for (label, public_key) in [
        ("small-order", ED25519_SMALL_ORDER_POINT),
        ("noncanonical", ED25519_NON_CANONICAL_IDENTITY),
        (
            "noncanonical-non-small-order",
            ED25519_NON_CANONICAL_NON_SMALL_ORDER_POINT,
        ),
    ] {
        assert_eq!(
            run_syscall_verify_signature_blob(message, &signature, &public_key, 1),
            0,
            "{label} public key must reject"
        );
    }
}
#[test]
fn syscall_verify_signature_ed25519_rejects_malformed_signature_r() {
    let message = b"ivm-ed25519-malformed-r";
    let signing_key = ed25519_test_key(0x45);
    let public_key = signing_key.verifying_key().to_bytes();
    for (label, replacement_r) in [
        ("small-order", ED25519_SMALL_ORDER_POINT),
        ("noncanonical", ED25519_NON_CANONICAL_IDENTITY),
    ] {
        let signature = ed25519_signature_with_replacement_r(&signing_key, message, &replacement_r);
        assert_eq!(
            run_syscall_verify_signature_blob(message, &signature, &public_key, 1),
            0,
            "{label} signature R must reject"
        );
    }
}
#[test]
fn syscall_verify_signature_rejects_wide_scheme_code_without_truncation() {
    use ed25519_dalek::Signer;
    let message = b"ivm-ed25519-wide-scheme-code";
    let signing_key = ed25519_test_key(0x46);
    let public_key = signing_key.verifying_key().to_bytes();
    let signature = signing_key.sign(message).to_bytes();
    assert_eq!(
        run_syscall_verify_signature_blob(message, &signature, &public_key, 1),
        1,
        "control signature must verify under the canonical scheme code"
    );
    assert_eq!(
        run_syscall_verify_signature_blob(message, &signature, &public_key, 0x101),
        0,
        "a wide register value must not alias the Ed25519 scheme after narrowing"
    );
}
#[test]
fn syscall_verify_signature_secp256k1_via_tlv() {
    use iroha_crypto::{EcdsaSecp256k1Sha256, KeyGenOption};
    let (pk, sk) = EcdsaSecp256k1Sha256::keypair(KeyGenOption::UseSeed(vec![0x11; 32]));
    let pk_bytes = pk.to_sec1_bytes();
    let msg = b"ivm-secp256k1";
    let sig_bytes = EcdsaSecp256k1Sha256::sign(msg, &sk);
    let msg_tlv = make_tlv(PointerType::Blob as u16, msg);
    let sig_tlv = make_tlv(PointerType::Blob as u16, &sig_bytes);
    let pk_tlv = make_tlv(PointerType::Blob as u16, &pk_bytes);
    let mut vm = IVM::new(10_000);
    vm.memory.preload_input(0, &msg_tlv).expect("preload input");
    let p_msg = Memory::INPUT_START;
    let p_sig = p_msg + msg_tlv.len() as u64 + 8;
    let p_pk = p_sig + sig_tlv.len() as u64 + 8;
    vm.memory
        .preload_input(msg_tlv.len() as u64 + 8, &sig_tlv)
        .expect("preload input");
    vm.memory
        .preload_input((msg_tlv.len() + sig_tlv.len()) as u64 + 16, &pk_tlv)
        .expect("preload input");
    vm.set_register(10, p_msg);
    vm.set_register(11, p_sig);
    vm.set_register(12, p_pk);
    vm.set_register(13, 2); // scheme 2 = Secp256k1
    let halt = encoding::wide::encode_halt();
    let syscall = encoding::wide::encode_scall(ivm::syscalls::SYSCALL_VERIFY_SIGNATURE);
    let mut prog = Vec::new();
    prog.extend_from_slice(&syscall.to_le_bytes());
    prog.extend_from_slice(&halt.to_le_bytes());
    let prog = assemble(&prog);
    vm.load_program(&prog).unwrap();
    vm.run().unwrap();
    assert_eq!(vm.register(10), 1);
}
#[test]
fn syscall_verify_signature_rejects_all_zero_ed25519_signature_material() {
    let key = ed25519_test_key(0x42);
    let result = run_syscall_verify_signature_blob(
        b"ivm-ed25519-zero-syscall",
        &[0u8; 64],
        &key.verifying_key().to_bytes(),
        1,
    );
    assert_eq!(result, 0);
}
#[test]
fn syscall_verify_signature_rejects_all_zero_mldsa_signature_material() {
    let keypair =
        iroha_crypto::KeyPair::try_from_seed(vec![0x65; 32], iroha_crypto::Algorithm::MlDsa)
            .expect("seeded ML-DSA fixture");
    let public_key = keypair
        .public_key()
        .try_to_bytes()
        .expect("ML-DSA public key")
        .1;
    let _ = keypair.private_key();
    let signature = vec![0u8; iroha_crypto::ML_DSA_65_SIGNATURE_BYTES];
    let result =
        run_syscall_verify_signature_blob(b"ivm-mldsa-zero-syscall", &signature, public_key, 3);
    assert_eq!(result, 0);
}
#[test]
fn syscall_verify_signature_rejects_all_zero_mldsa_public_key_material() {
    let keypair =
        iroha_crypto::KeyPair::try_from_seed(vec![0x66; 32], iroha_crypto::Algorithm::MlDsa)
            .expect("seeded ML-DSA fixture");
    let secret_key = keypair.private_key();
    let message = b"ivm-mldsa-zero-public-key-syscall";
    let signature =
        iroha_crypto::Signature::try_new(secret_key, message).expect("sign ML-DSA fixture");
    let public_key = vec![0u8; iroha_crypto::ML_DSA_65_PUBLIC_KEY_BYTES];
    let result = run_syscall_verify_signature_blob(message, signature.payload(), &public_key, 3);
    assert_eq!(result, 0);
}
#[test]
fn syscall_verify_signature_dilithium_via_tlv() {
    let keypair =
        iroha_crypto::KeyPair::try_from_seed(vec![0x65; 32], iroha_crypto::Algorithm::MlDsa)
            .expect("seeded ML-DSA fixture");
    let pk = keypair
        .public_key()
        .try_to_bytes()
        .expect("ML-DSA public key")
        .1;
    let sk = keypair.private_key();
    let msg = b"ivm-dilithium";
    let sig = iroha_crypto::Signature::try_new(sk, msg).expect("sign ML-DSA fixture");
    let msg_tlv = make_tlv(PointerType::Blob as u16, msg);
    let sig_tlv = make_tlv(PointerType::Blob as u16, sig.payload());
    let pk_tlv = make_tlv(PointerType::Blob as u16, pk);
    let mut vm = IVM::new(10_000);
    vm.memory.preload_input(0, &msg_tlv).expect("preload input");
    let p_msg = Memory::INPUT_START;
    let p_sig = p_msg + msg_tlv.len() as u64 + 8;
    let p_pk = p_sig + sig_tlv.len() as u64 + 8;
    vm.memory
        .preload_input(msg_tlv.len() as u64 + 8, &sig_tlv)
        .expect("preload input");
    vm.memory
        .preload_input((msg_tlv.len() + sig_tlv.len()) as u64 + 16, &pk_tlv)
        .expect("preload input");
    vm.set_register(10, p_msg);
    vm.set_register(11, p_sig);
    vm.set_register(12, p_pk);
    vm.set_register(13, 3); // scheme 3 = Dilithium
    let halt = encoding::wide::encode_halt();
    let syscall = encoding::wide::encode_scall(ivm::syscalls::SYSCALL_VERIFY_SIGNATURE);
    let mut prog = Vec::new();
    prog.extend_from_slice(&syscall.to_le_bytes());
    prog.extend_from_slice(&halt.to_le_bytes());
    let prog = assemble(&prog);
    vm.load_program(&prog).unwrap();
    vm.run().unwrap();
    assert_eq!(vm.register(10), 1);
}
#[test]
fn secp256k1_verify_rejects_high_s_signature() {
    use iroha_crypto::{EcdsaSecp256k1Sha256, KeyGenOption};
    use ivm::signature::{SignatureScheme, verify_signature};
    use k256::ecdsa::Signature;
    let (pk, sk) = EcdsaSecp256k1Sha256::keypair(KeyGenOption::UseSeed(vec![0x33; 32]));
    let msg = b"ivm-high-s";
    let sig = EcdsaSecp256k1Sha256::sign(msg, &sk);
    let signature = Signature::from_slice(&sig).expect("signature parse");
    let high_s = Signature::from_scalars(signature.r(), -signature.s()).expect("high-S signature");
    let high_s_bytes = high_s.to_vec();
    assert!(
        !verify_signature(
            SignatureScheme::Secp256k1,
            msg,
            &high_s_bytes,
            pk.to_sec1_bytes().as_ref()
        ),
        "high-S signatures must be rejected"
    );
}
#[test]
fn syscall_verify_signature_ed25519_via_tlv() {
    let msg = b"ivm-ed25519";
    assert_eq!(
        run_syscall_verify_signature_ed25519(PointerType::Blob, msg, 5)
            .expect("canonical Blob message must verify"),
        1
    );
}
#[test]
fn syscall_verify_signature_rejects_norito_message_tlv() {
    let message = br#"{"amount":123,"dpn_id":"dpn-live"}"#;
    assert!(matches!(
        run_syscall_verify_signature_ed25519(PointerType::NoritoBytes, message, 6),
        Err(VMError::NoritoInvalid)
    ));
}
#[test]
fn syscall_verify_signature_rejects_json_message_tlv() {
    let message = br#"{"amount":456,"dpn_id":"dpn-json"}"#;
    assert!(matches!(
        run_syscall_verify_signature_ed25519(PointerType::Json, message, 7),
        Err(VMError::NoritoInvalid)
    ));
}
