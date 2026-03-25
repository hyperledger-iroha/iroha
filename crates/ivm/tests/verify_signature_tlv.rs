//! Signature verification opcode/syscall tests using TLVs.

use ivm::signature::{Ed25519BatchEntry, Ed25519BatchRequest};
use ivm::{IVM, Memory, PointerType, encoding, instruction};

mod common;
use common::assemble;

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
    let syscall = encoding::wide::encode_sys(
        instruction::wide::system::SCALL,
        ivm::syscalls::SYSCALL_VERIFY_SIGNATURE as u8,
    );
    let mut prog = Vec::new();
    prog.extend_from_slice(&syscall.to_le_bytes());
    prog.extend_from_slice(&halt.to_le_bytes());
    let prog = assemble(&prog);
    vm.load_program(&prog).unwrap();
    vm.run().unwrap();
    assert_eq!(vm.register(10), 1);
}

#[test]
fn syscall_verify_signature_dilithium_via_tlv() {
    use pqcrypto_mldsa::mldsa65 as dilithium;
    use pqcrypto_traits::sign::{DetachedSignature, PublicKey};
    let (pk, sk) = dilithium::keypair();
    let msg = b"ivm-dilithium";
    let sig = dilithium::detached_sign(msg, &sk);

    let msg_tlv = make_tlv(PointerType::Blob as u16, msg);
    let sig_tlv = make_tlv(PointerType::Blob as u16, sig.as_bytes());
    let pk_tlv = make_tlv(PointerType::Blob as u16, pk.as_bytes());

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
    let syscall = encoding::wide::encode_sys(
        instruction::wide::system::SCALL,
        ivm::syscalls::SYSCALL_VERIFY_SIGNATURE as u8,
    );
    let mut prog = Vec::new();
    prog.extend_from_slice(&syscall.to_le_bytes());
    prog.extend_from_slice(&halt.to_le_bytes());
    let prog = assemble(&prog);
    vm.load_program(&prog).unwrap();
    vm.run().unwrap();
    assert_eq!(vm.register(10), 1);
}

#[test]
fn opcode_verify_ed25519_via_tlv() {
    use ed25519_dalek::Signer;
    let sk = ed25519_test_key(1);
    let pk_bytes = sk.verifying_key().to_bytes();
    let msg = b"ivm-op-ed25519";
    let sig = sk.sign(msg);
    let msg_tlv = make_tlv(PointerType::Blob as u16, msg);
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
    vm.set_register(1, p_msg);
    vm.set_register(2, p_sig);
    vm.set_register(3, p_pk);
    let op = instruction::wide::crypto::ED25519VERIFY;
    let word = encoding::wide::encode_rr(op, 3, 1, 2); // rd holds pk pointer initially
    let halt = encoding::wide::encode_halt().to_le_bytes();
    let mut code = Vec::new();
    code.extend_from_slice(&word.to_le_bytes());
    code.extend_from_slice(&halt);
    vm.memory.load_code(&code);
    vm.run().unwrap();
    assert_eq!(vm.register(3), 1);
}

#[test]
fn opcode_verify_ed25519_batch_via_tlv_success() {
    use ed25519_dalek::Signer;
    let sk = ed25519_test_key(2);
    let pk_bytes = sk.verifying_key().to_bytes();
    let entries = ["entry-a", "entry-b"]
        .iter()
        .map(|msg| {
            let msg_bytes = msg.as_bytes();
            let sig = sk.sign(msg_bytes);
            Ed25519BatchEntry {
                message: msg_bytes.to_vec(),
                signature: sig.to_bytes().to_vec(),
                public_key: pk_bytes.to_vec(),
            }
        })
        .collect();
    let request = Ed25519BatchRequest {
        seed: [0u8; 32],
        entries,
    };
    let payload = norito::to_bytes(&request).expect("encode request");
    let tlv = make_tlv(PointerType::NoritoBytes as u16, &payload);

    let mut vm = IVM::new(10_000);
    let ptr = vm.alloc_input_tlv(&tlv).expect("alloc request");
    vm.set_register(1, ptr);
    vm.set_register(2, 9); // failure index register
    let word = encoding::wide::encode_rr(instruction::wide::crypto::ED25519BATCHVERIFY, 5, 1, 2);
    let halt = encoding::wide::encode_halt().to_le_bytes();
    let mut code = Vec::new();
    code.extend_from_slice(&word.to_le_bytes());
    code.extend_from_slice(&halt);
    vm.memory.load_code(&code);
    vm.run().unwrap();
    assert_eq!(vm.register(5), 1, "batch should verify");
    assert_eq!(vm.register(2), 0, "failure index cleared on success");
}

#[test]
fn opcode_verify_ed25519_batch_via_tlv_reports_failure_index() {
    use ed25519_dalek::Signer;
    let sk = ed25519_test_key(3);
    let pk_bytes = sk.verifying_key().to_bytes();
    let mut bad_sig = sk.sign(b"entry-bad").to_bytes().to_vec();
    bad_sig[0] ^= 0x42;
    let entries = vec![
        Ed25519BatchEntry {
            message: b"ok-a".to_vec(),
            signature: sk.sign(b"ok-a").to_bytes().to_vec(),
            public_key: pk_bytes.to_vec(),
        },
        Ed25519BatchEntry {
            message: b"bad-b".to_vec(),
            signature: bad_sig,
            public_key: pk_bytes.to_vec(),
        },
    ];
    let request = Ed25519BatchRequest {
        seed: [1u8; 32],
        entries,
    };
    let payload = norito::to_bytes(&request).expect("encode request");
    let tlv = make_tlv(PointerType::NoritoBytes as u16, &payload);

    let mut vm = IVM::new(10_000);
    let ptr = vm.alloc_input_tlv(&tlv).expect("alloc request");
    vm.set_register(1, ptr);
    vm.set_register(2, 123);
    let word = encoding::wide::encode_rr(instruction::wide::crypto::ED25519BATCHVERIFY, 6, 1, 2);
    let halt = encoding::wide::encode_halt().to_le_bytes();
    let mut code = Vec::new();
    code.extend_from_slice(&word.to_le_bytes());
    code.extend_from_slice(&halt);
    vm.memory.load_code(&code);
    vm.run().unwrap();
    assert_eq!(vm.register(6), 0, "batch should fail");
    assert_eq!(vm.register(2), 1, "second entry flagged as failing");
}

#[test]
fn opcode_verify_secp256k1_via_tlv() {
    use iroha_crypto::{EcdsaSecp256k1Sha256, KeyGenOption};
    let (pk, sk) = EcdsaSecp256k1Sha256::keypair(KeyGenOption::UseSeed(vec![0x22; 32]));
    let pk_bytes = pk.to_sec1_bytes();
    let msg = b"ivm-op-secp256k1";
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
    vm.set_register(1, p_msg);
    vm.set_register(2, p_sig);
    vm.set_register(3, p_pk);
    let op = instruction::wide::crypto::ECDSAVERIFY;
    let word = encoding::wide::encode_rr(op, 3, 1, 2);
    let halt = encoding::wide::encode_halt().to_le_bytes();
    let mut code = Vec::new();
    code.extend_from_slice(&word.to_le_bytes());
    code.extend_from_slice(&halt);
    vm.memory.load_code(&code);
    vm.run().unwrap();
    assert_eq!(vm.register(3), 1);
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
fn opcode_verify_dilithium_via_tlv() {
    use pqcrypto_mldsa::mldsa65 as dilithium;
    use pqcrypto_traits::sign::{DetachedSignature, PublicKey};
    let (pk, sk) = dilithium::keypair();
    let msg = b"ivm-op-dilithium";
    let sig = dilithium::detached_sign(msg, &sk);
    let msg_tlv = make_tlv(PointerType::Blob as u16, msg);
    let sig_tlv = make_tlv(PointerType::Blob as u16, sig.as_bytes());
    let pk_tlv = make_tlv(PointerType::Blob as u16, pk.as_bytes());
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
    vm.set_register(1, p_msg);
    vm.set_register(2, p_sig);
    vm.set_register(3, p_pk);
    let op = instruction::wide::crypto::DILITHIUMVERIFY;
    let word = encoding::wide::encode_rr(op, 3, 1, 2);
    let halt = encoding::wide::encode_halt().to_le_bytes();
    let mut code = Vec::new();
    code.extend_from_slice(&word.to_le_bytes());
    code.extend_from_slice(&halt);
    vm.memory.load_code(&code);
    vm.run().unwrap();
    assert_eq!(vm.register(3), 1);
}

#[test]
fn syscall_verify_signature_ed25519_via_tlv() {
    use ed25519_dalek::Signer;

    // Generate key and signature
    let sk = ed25519_test_key(5);
    let pk_bytes = sk.verifying_key().to_bytes();
    let msg = b"ivm-ed25519";
    let sig = sk.sign(msg);

    // Prepare TLVs in INPUT: message, signature, public key
    let msg_tlv = make_tlv(PointerType::Blob as u16, msg);
    let sig_tlv = make_tlv(PointerType::Blob as u16, &sig.to_bytes());
    let pk_tlv = make_tlv(PointerType::Blob as u16, &pk_bytes);

    let mut vm = IVM::new(10_000);
    // Preload TLVs with spacing
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

    // Place pointers in registers
    vm.set_register(10, p_msg);
    vm.set_register(11, p_sig);
    vm.set_register(12, p_pk);
    vm.set_register(13, 1); // scheme 1 = Ed25519

    // Program: SCALL VERIFY_SIGNATURE; HALT
    let halt = encoding::wide::encode_halt();
    let syscall = encoding::wide::encode_sys(
        instruction::wide::system::SCALL,
        ivm::syscalls::SYSCALL_VERIFY_SIGNATURE as u8,
    );
    let mut prog = Vec::new();
    prog.extend_from_slice(&syscall.to_le_bytes());
    prog.extend_from_slice(&halt.to_le_bytes());
    let prog = assemble(&prog);
    vm.load_program(&prog).unwrap();
    vm.run().unwrap();
    assert_eq!(vm.register(10), 1);
}
