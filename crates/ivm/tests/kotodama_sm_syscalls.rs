//! Kotodama integration tests for SM3/SM2 syscalls.

use hex::decode;
use iroha_crypto::{Hash, Sm2PrivateKey, Sm2PublicKey, Sm3Digest};
use ivm::{Memory, PointerType, kotodama::compiler::Compiler as KotodamaCompiler};

fn new_sm_host() -> ivm::host::DefaultHost {
    ivm::host::DefaultHost::new().with_sm_enabled(true)
}

fn make_blob_tlv(payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(7 + payload.len() + 32);
    out.extend_from_slice(&(PointerType::Blob as u16).to_be_bytes());
    out.push(1);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(&payload);
    let h: [u8; 32] = Hash::new(&payload).into();
    out.extend_from_slice(&h);
    out
}

fn preload_blob(vm: &mut ivm::IVM, offset: &mut u64, payload: &[u8]) -> u64 {
    let tlv = make_blob_tlv(payload);
    let ptr = vm.alloc_input_tlv(&tlv).expect("preload blob into INPUT");
    // Keep caller cursor in step with the VM bump pointer to avoid overlaps during INPUT_PUBLISH_TLV.
    let ptr_off = ptr
        .checked_sub(Memory::INPUT_START)
        .expect("pointer must lie within INPUT");
    *offset = ptr_off + tlv.len() as u64;
    ptr
}

#[test]
fn kotodama_sm3_hash_returns_expected_digest() {
    let src = r#"
        fn sm_hash(msg: Blob) -> Blob {
            return sm::hash(msg);
        }
    "#;
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile sm3 hash contract");

    let message = b"kotodama-sm3";
    let expected = Sm3Digest::hash(message);

    let mut vm = ivm::IVM::new(10_000);
    vm.set_host(new_sm_host());
    vm.load_program(&code).expect("load program");

    let mut offset = 0;
    let p_msg = preload_blob(&mut vm, &mut offset, message);
    vm.set_register(10, p_msg);

    vm.run().expect("vm run");

    let out_ptr = vm.register(10);
    assert_ne!(out_ptr, 0, "sm::hash should return Blob pointer");
    let tlv = vm
        .memory
        .validate_tlv(out_ptr)
        .expect("validate digest pointer");
    assert_eq!(tlv.type_id, PointerType::Blob);
    assert_eq!(tlv.payload, expected.as_bytes());
}

fn compile_sm2_verify() -> Vec<u8> {
    let src = r#"
        fn verify(msg: Blob, sig: Blob, pk: Blob) -> bool {
            return sm::verify(msg, sig, pk);
        }
    "#;
    KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile sm2 verify contract")
}

fn compile_sm2_verify_with_distid() -> Vec<u8> {
    let src = r#"
        fn verify_with_distid(msg: Blob, sig: Blob, pk: Blob, distid: Blob) -> bool {
            return sm::verify(msg, sig, pk, distid);
        }
    "#;
    KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile sm2 verify with distid contract")
}

fn compile_sm4_gcm_seal() -> Vec<u8> {
    let src = r#"
        fn seal(key: Blob, nonce: Blob, aad: Blob, pt: Blob) -> Blob {
            return sm::seal_gcm(key, nonce, aad, pt);
        }
    "#;
    KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile sm4 gcm seal contract")
}

fn compile_sm4_gcm_open() -> Vec<u8> {
    let src = r#"
        fn open(key: Blob, nonce: Blob, aad: Blob, ct: Blob) -> Blob {
            return sm::open_gcm(key, nonce, aad, ct);
        }
    "#;
    KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile sm4 gcm open contract")
}

fn compile_sm4_ccm_seal() -> Vec<u8> {
    let src = r#"
        fn seal(key: Blob, nonce: Blob, aad: Blob, pt: Blob, tag_len: int) -> Blob {
            return sm::seal_ccm(key, nonce, aad, pt, tag_len);
        }
    "#;
    KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile sm4 ccm seal contract")
}

fn compile_sm4_ccm_open() -> Vec<u8> {
    let src = r#"
        fn open(key: Blob, nonce: Blob, aad: Blob, ct: Blob, tag_len: int) -> Blob {
            return sm::open_ccm(key, nonce, aad, ct, tag_len);
        }
    "#;
    KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile sm4 ccm open contract")
}

fn new_sm2_key() -> (Sm2PrivateKey, Sm2PublicKey) {
    let secret = [0x42u8; 32];
    let private =
        Sm2PrivateKey::new(Sm2PublicKey::DEFAULT_DISTID, secret).expect("construct private key");
    let public = private.public_key();
    (private, public)
}

#[test]
fn kotodama_sm2_verify_accepts_valid_signature() {
    let code = compile_sm2_verify();
    let (private, public) = new_sm2_key();

    let message = b"kotodama-sm2";
    let sig_bytes = private.sign(message).to_bytes();
    let pk_bytes = public.to_sec1_bytes(false);

    let mut vm = ivm::IVM::new(10_000);
    vm.set_host(new_sm_host());
    vm.load_program(&code).expect("load program");

    let mut offset = 0;
    let p_msg = preload_blob(&mut vm, &mut offset, message);
    let p_sig = preload_blob(&mut vm, &mut offset, sig_bytes.as_ref());
    let p_pk = preload_blob(&mut vm, &mut offset, &pk_bytes);

    vm.set_register(10, p_msg);
    vm.set_register(11, p_sig);
    vm.set_register(12, p_pk);
    vm.set_register(13, 0);

    vm.run().expect("vm run");
    assert_eq!(vm.register(10), 1, "sm::verify should succeed");
}

#[test]
fn kotodama_sm2_verify_rejects_malformed_signature() {
    let code = compile_sm2_verify();
    let (private, public) = new_sm2_key();

    let message = b"kotodama-sm2";
    let mut sig_bytes = private.sign(message).to_bytes();
    sig_bytes[0] ^= 0xFF;
    let pk_bytes = public.to_sec1_bytes(false);

    let mut vm = ivm::IVM::new(10_000);
    vm.set_host(new_sm_host());
    vm.load_program(&code).expect("load program");

    let mut offset = 0;
    let p_msg = preload_blob(&mut vm, &mut offset, message);
    let p_sig = preload_blob(&mut vm, &mut offset, sig_bytes.as_ref());
    let p_pk = preload_blob(&mut vm, &mut offset, &pk_bytes);

    vm.set_register(10, p_msg);
    vm.set_register(11, p_sig);
    vm.set_register(12, p_pk);
    vm.set_register(13, 0);

    vm.run().expect("vm run");
    assert_eq!(vm.register(10), 0, "malformed signature must fail");
}

#[test]
fn kotodama_sm2_verify_rejects_signature_for_other_message() {
    let code = compile_sm2_verify();
    let (private, public) = new_sm2_key();

    let message = b"kotodama-sm2";
    let sig_bytes = private.sign(message).to_bytes();
    let pk_bytes = public.to_sec1_bytes(false);
    let other_message = b"kotodama-sm2-nonce-reuse";

    let mut vm = ivm::IVM::new(10_000);
    vm.set_host(new_sm_host());
    vm.load_program(&code).expect("load program");

    let mut offset = 0;
    let p_msg = preload_blob(&mut vm, &mut offset, other_message);
    let p_sig = preload_blob(&mut vm, &mut offset, sig_bytes.as_ref());
    let p_pk = preload_blob(&mut vm, &mut offset, &pk_bytes);

    vm.set_register(10, p_msg);
    vm.set_register(11, p_sig);
    vm.set_register(12, p_pk);
    vm.set_register(13, 0);

    vm.run().expect("vm run");
    assert_eq!(
        vm.register(10),
        0,
        "signature tied to different message must be rejected"
    );
}

#[test]
fn kotodama_sm2_verify_with_distid_enforces_identifier() {
    let code = compile_sm2_verify_with_distid();
    let distid = "kotodama-dist-0001";
    let private =
        Sm2PrivateKey::new(distid.to_string(), [0x24u8; 32]).expect("construct private key");
    let public = private.public_key();

    let message = b"kotodama-sm2-dist";
    let sig_bytes = private.sign(message).to_bytes();
    let pk_bytes = public.to_sec1_bytes(false);
    let dist_bytes = distid.as_bytes();
    let wrong_dist = b"other-dist";

    // Success with matching distid
    let mut vm = ivm::IVM::new(10_000);
    vm.set_host(new_sm_host());
    vm.load_program(&code).expect("load program");
    let mut offset = 0;
    let p_msg = preload_blob(&mut vm, &mut offset, message);
    let p_sig = preload_blob(&mut vm, &mut offset, sig_bytes.as_ref());
    let p_pk = preload_blob(&mut vm, &mut offset, &pk_bytes);
    let p_dist = preload_blob(&mut vm, &mut offset, dist_bytes);

    vm.set_register(10, p_msg);
    vm.set_register(11, p_sig);
    vm.set_register(12, p_pk);
    vm.set_register(13, p_dist);

    vm.run().expect("vm run");
    assert_eq!(vm.register(10), 1, "matching distid should verify");

    // Failure with mismatched distid
    let mut vm_fail = ivm::IVM::new(10_000);
    vm_fail.set_host(new_sm_host());
    vm_fail.load_program(&code).expect("load program");
    let mut offset_fail = 0;
    let p_msg_fail = preload_blob(&mut vm_fail, &mut offset_fail, message);
    let p_sig_fail = preload_blob(&mut vm_fail, &mut offset_fail, sig_bytes.as_ref());
    let p_pk_fail = preload_blob(&mut vm_fail, &mut offset_fail, &pk_bytes);
    let p_dist_fail = preload_blob(&mut vm_fail, &mut offset_fail, wrong_dist);

    vm_fail.set_register(10, p_msg_fail);
    vm_fail.set_register(11, p_sig_fail);
    vm_fail.set_register(12, p_pk_fail);
    vm_fail.set_register(13, p_dist_fail);

    vm_fail.run().expect("vm run");
    assert_eq!(
        vm_fail.register(10),
        0,
        "mismatched distid must cause verification failure"
    );
}

#[test]
fn kotodama_sm4_gcm_seal_matches_vector() {
    let code = compile_sm4_gcm_seal();
    let key = decode("0123456789abcdeffedcba9876543210").expect("hex key");
    let nonce = decode("00001234567800000000abcd").expect("hex nonce");
    let aad = decode("feedfacedeadbeeffeedfacedeadbeefabaddad2").expect("hex aad");
    let plaintext = decode("d9313225f88406e5a55909c5aff5269a").expect("hex plaintext");
    let expected_cipher = decode("6468017fde4979a107326ee77d8a265c").expect("hex cipher");
    let expected_tag = decode("cadf422b1af7ec6df46004dc8d3ba855").expect("hex tag");

    let mut vm = ivm::IVM::new(10_000);
    vm.set_host(new_sm_host());
    vm.load_program(&code).expect("load program");

    let mut offset = 0;
    let p_key = preload_blob(&mut vm, &mut offset, &key);
    let p_nonce = preload_blob(&mut vm, &mut offset, &nonce);
    let p_aad = preload_blob(&mut vm, &mut offset, &aad);
    let p_pt = preload_blob(&mut vm, &mut offset, &plaintext);

    vm.set_register(10, p_key);
    vm.set_register(11, p_nonce);
    vm.set_register(12, p_aad);
    vm.set_register(13, p_pt);

    vm.run().expect("vm run");

    let out_ptr = vm.register(10);
    assert_ne!(out_ptr, 0, "seal should produce output blob");
    let tlv = vm
        .memory
        .validate_tlv(out_ptr)
        .expect("validate ciphertext blob");
    assert_eq!(tlv.type_id, PointerType::Blob);
    assert_eq!(
        &tlv.payload[..expected_cipher.len()],
        expected_cipher.as_slice()
    );
    assert_eq!(
        &tlv.payload[expected_cipher.len()..],
        expected_tag.as_slice()
    );
}

#[test]
fn kotodama_sm4_gcm_open_returns_plaintext() {
    let code = compile_sm4_gcm_open();
    let key = decode("0123456789abcdeffedcba9876543210").expect("hex key");
    let nonce = decode("00001234567800000000abcd").expect("hex nonce");
    let aad = decode("feedfacedeadbeeffeedfacedeadbeefabaddad2").expect("hex aad");
    let plaintext = decode("d9313225f88406e5a55909c5aff5269a").expect("hex plaintext");
    let cipher = decode("6468017fde4979a107326ee77d8a265c").expect("hex cipher");
    let tag = decode("cadf422b1af7ec6df46004dc8d3ba855").expect("hex tag");
    let mut cipher_tag = cipher.clone();
    cipher_tag.extend_from_slice(&tag);

    let mut vm = ivm::IVM::new(10_000);
    vm.set_host(new_sm_host());
    vm.load_program(&code).expect("load program");

    let mut offset = 0;
    let p_key = preload_blob(&mut vm, &mut offset, &key);
    let p_nonce = preload_blob(&mut vm, &mut offset, &nonce);
    let p_aad = preload_blob(&mut vm, &mut offset, &aad);
    let p_ct = preload_blob(&mut vm, &mut offset, &cipher_tag);

    vm.set_register(10, p_key);
    vm.set_register(11, p_nonce);
    vm.set_register(12, p_aad);
    vm.set_register(13, p_ct);

    vm.run().expect("vm run");

    let out_ptr = vm.register(10);
    assert_ne!(out_ptr, 0, "open should return plaintext blob");
    let tlv = vm
        .memory
        .validate_tlv(out_ptr)
        .expect("validate plaintext blob");
    assert_eq!(tlv.type_id, PointerType::Blob);
    assert_eq!(tlv.payload, plaintext.as_slice());
}

#[test]
fn kotodama_sm4_gcm_open_rejects_bad_tag() {
    let code = compile_sm4_gcm_open();
    let key = decode("0123456789abcdeffedcba9876543210").expect("hex key");
    let nonce = decode("00001234567800000000abcd").expect("hex nonce");
    let aad = decode("feedfacedeadbeeffeedfacedeadbeefabaddad2").expect("hex aad");
    let cipher = decode("6468017fde4979a107326ee77d8a265c").expect("hex cipher");
    let mut tag = decode("cadf422b1af7ec6df46004dc8d3ba855").expect("hex tag");
    tag[0] ^= 0xFF;
    let mut cipher_tag = cipher.clone();
    cipher_tag.extend_from_slice(&tag);

    let mut vm = ivm::IVM::new(10_000);
    vm.set_host(new_sm_host());
    vm.load_program(&code).expect("load program");

    let mut offset = 0;
    let p_key = preload_blob(&mut vm, &mut offset, &key);
    let p_nonce = preload_blob(&mut vm, &mut offset, &nonce);
    let p_aad = preload_blob(&mut vm, &mut offset, &aad);
    let p_ct = preload_blob(&mut vm, &mut offset, &cipher_tag);

    vm.set_register(10, p_key);
    vm.set_register(11, p_nonce);
    vm.set_register(12, p_aad);
    vm.set_register(13, p_ct);

    vm.run().expect("vm run");
    assert_eq!(vm.register(10), 0, "open must fail for tampered tag");
}

#[test]
fn kotodama_sm4_ccm_seal_matches_vector() {
    let code = compile_sm4_ccm_seal();
    let key = decode("404142434445464748494a4b4c4d4e4f").expect("hex key");
    let nonce = decode("10111213141516").expect("hex nonce");
    let aad = decode("000102030405060708090a0b0c0d0e0f").expect("hex aad");
    let plaintext = decode("202122232425262728292a2b2c2d2e2f").expect("hex plaintext");
    let expected_cipher = decode("a9550cebab5f227d9590e8979caafd1f").expect("hex cipher");
    let expected_tag = decode("03a1f305").expect("hex tag");

    let mut vm = ivm::IVM::new(10_000);
    vm.set_host(new_sm_host());
    vm.load_program(&code).expect("load program");

    let mut offset = 0;
    let p_key = preload_blob(&mut vm, &mut offset, &key);
    let p_nonce = preload_blob(&mut vm, &mut offset, &nonce);
    let p_aad = preload_blob(&mut vm, &mut offset, &aad);
    let p_pt = preload_blob(&mut vm, &mut offset, &plaintext);

    vm.set_register(10, p_key);
    vm.set_register(11, p_nonce);
    vm.set_register(12, p_aad);
    vm.set_register(13, p_pt);
    vm.set_register(14, expected_tag.len() as u64);

    vm.run().expect("vm run");

    let out_ptr = vm.register(10);
    assert_ne!(out_ptr, 0, "seal should produce output blob");
    let tlv = vm
        .memory
        .validate_tlv(out_ptr)
        .expect("validate ciphertext blob");
    assert_eq!(tlv.type_id, PointerType::Blob);
    assert_eq!(
        &tlv.payload[..expected_cipher.len()],
        expected_cipher.as_slice()
    );
    assert_eq!(
        &tlv.payload[expected_cipher.len()..],
        expected_tag.as_slice()
    );
}

#[test]
fn kotodama_sm4_ccm_open_returns_plaintext() {
    let code = compile_sm4_ccm_open();
    let key = decode("404142434445464748494a4b4c4d4e4f").expect("hex key");
    let nonce = decode("10111213141516").expect("hex nonce");
    let aad = decode("000102030405060708090a0b0c0d0e0f").expect("hex aad");
    let plaintext = decode("202122232425262728292a2b2c2d2e2f").expect("hex plaintext");
    let cipher = decode("a9550cebab5f227d9590e8979caafd1f").expect("hex cipher");
    let tag = decode("03a1f305").expect("hex tag");
    let mut cipher_tag = cipher.clone();
    cipher_tag.extend_from_slice(&tag);

    let mut vm = ivm::IVM::new(10_000);
    vm.set_host(new_sm_host());
    vm.load_program(&code).expect("load program");

    let mut offset = 0;
    let p_key = preload_blob(&mut vm, &mut offset, &key);
    let p_nonce = preload_blob(&mut vm, &mut offset, &nonce);
    let p_aad = preload_blob(&mut vm, &mut offset, &aad);
    let p_ct = preload_blob(&mut vm, &mut offset, &cipher_tag);

    vm.set_register(10, p_key);
    vm.set_register(11, p_nonce);
    vm.set_register(12, p_aad);
    vm.set_register(13, p_ct);
    vm.set_register(14, tag.len() as u64);

    vm.run().expect("vm run");

    let out_ptr = vm.register(10);
    assert_ne!(out_ptr, 0, "open should produce plaintext blob");
    let tlv = vm
        .memory
        .validate_tlv(out_ptr)
        .expect("validate plaintext blob");
    assert_eq!(tlv.type_id, PointerType::Blob);
    assert_eq!(tlv.payload, plaintext.as_slice());
}

#[test]
fn kotodama_sm4_ccm_open_rejects_bad_tag() {
    let code = compile_sm4_ccm_open();
    let key = decode("404142434445464748494a4b4c4d4e4f").expect("hex key");
    let nonce = decode("10111213141516").expect("hex nonce");
    let aad = decode("000102030405060708090a0b0c0d0e0f").expect("hex aad");
    let cipher = decode("a9550cebab5f227d9590e8979caafd1f").expect("hex cipher");
    let mut tag = decode("03a1f305").expect("hex tag");
    tag[0] ^= 0x01;
    let mut cipher_tag = cipher.clone();
    cipher_tag.extend_from_slice(&tag);

    let mut vm = ivm::IVM::new(10_000);
    vm.set_host(new_sm_host());
    vm.load_program(&code).expect("load program");

    let mut offset = 0;
    let p_key = preload_blob(&mut vm, &mut offset, &key);
    let p_nonce = preload_blob(&mut vm, &mut offset, &nonce);
    let p_aad = preload_blob(&mut vm, &mut offset, &aad);
    let p_ct = preload_blob(&mut vm, &mut offset, &cipher_tag);

    vm.set_register(10, p_key);
    vm.set_register(11, p_nonce);
    vm.set_register(12, p_aad);
    vm.set_register(13, p_ct);
    vm.set_register(14, tag.len() as u64);

    vm.run().expect("vm run");
    assert_eq!(
        vm.register(10),
        0,
        "open must fail for tampered CCM authentication tag"
    );
}
