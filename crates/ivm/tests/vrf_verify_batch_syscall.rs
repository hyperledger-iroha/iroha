#![cfg(feature = "ivm_vrf_tests")]
use ivm::{IVM, Memory, PointerType, host::DefaultHost};

mod common;
use blstrs::{G1Affine, G1Projective, G2Affine, G2Projective, Scalar};
use common::assemble_syscalls;
use group::{Curve, Group, prime::PrimeCurveAffine};
use ivm::vrf::{VrfVerifyBatchRequest, VrfVerifyRequest};

fn hash_to_g1(msg: &[u8]) -> G1Affine {
    const DST: &[u8] = b"BLS12381G1_XMD:SHA-256_SSWU_RO_IROHA_VRF_V1";
    G1Projective::hash_to_curve(msg, DST, &[]).to_affine()
}
fn hash_to_g2(msg: &[u8]) -> G2Affine {
    const DST: &[u8] = b"BLS12381G2_XMD:SHA-256_SSWU_RO_IROHA_VRF_V1";
    G2Projective::hash_to_curve(msg, DST, &[]).to_affine()
}

fn make_tlv(type_id: u16, payload: &[u8]) -> Vec<u8> {
    use iroha_crypto::Hash;
    let mut out = Vec::with_capacity(7 + payload.len() + 32);
    out.extend_from_slice(&type_id.to_be_bytes());
    out.push(1);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload.as_ref());
    let h: [u8; 32] = Hash::new(payload).into();
    out.extend_from_slice(&h);
    out
}

#[test]
fn syscall_vrf_verify_batch_two_items_ok() {
    // Item 1: Normal (SigInG2)
    let sk1 = {
        let mut b = [0u8; 32];
        for (i, x) in b.iter_mut().enumerate() {
            *x = (i as u8) + 1;
        }
        Scalar::from_bytes_be(&b).unwrap()
    };
    let pk1 = (G1Projective::generator() * sk1)
        .to_affine()
        .to_compressed();
    let chain = b"test-chain";
    let input1 = b"in1";
    let mut in1 = Vec::new();
    in1.extend_from_slice(b"iroha:vrf:v1:input|");
    in1.extend_from_slice(chain);
    in1.push(b'|');
    in1.extend_from_slice(input1);
    let msg1: [u8; 32] = iroha_crypto::Hash::new(&in1).into();
    let sig1 = (G2Projective::from(hash_to_g2(&msg1)) * sk1)
        .to_affine()
        .to_compressed();
    let mut y1buf = Vec::new();
    y1buf.extend_from_slice(b"iroha:vrf:v1:output");
    y1buf.extend_from_slice(&sig1);
    let exp1: [u8; 32] = iroha_crypto::Hash::new(&y1buf).into();

    // Item 2: Small (SigInG1)
    let sk2 = {
        let mut b = [0u8; 32];
        for (i, x) in b.iter_mut().enumerate() {
            *x = (i as u8) + 11;
        }
        Scalar::from_bytes_be(&b).unwrap()
    };
    let pk2 = (G2Projective::generator() * sk2)
        .to_affine()
        .to_compressed();
    let input2 = b"in2";
    let mut in2 = Vec::new();
    in2.extend_from_slice(b"iroha:vrf:v1:input|");
    in2.extend_from_slice(chain);
    in2.push(b'|');
    in2.extend_from_slice(input2);
    let msg2: [u8; 32] = iroha_crypto::Hash::new(&in2).into();
    let sig2 = (G1Projective::from(hash_to_g1(&msg2)) * sk2)
        .to_affine()
        .to_compressed();
    let mut y2buf = Vec::new();
    y2buf.extend_from_slice(b"iroha:vrf:v1:output");
    y2buf.extend_from_slice(&sig2);
    let exp2: [u8; 32] = iroha_crypto::Hash::new(&y2buf).into();

    let req = VrfVerifyBatchRequest {
        items: vec![
            VrfVerifyRequest {
                variant: 1,
                pk: pk1.to_vec(),
                proof: sig1.to_vec(),
                chain_id: chain.to_vec(),
                input: input1.to_vec(),
            },
            VrfVerifyRequest {
                variant: 2,
                pk: pk2.to_vec(),
                proof: sig2.to_vec(),
                chain_id: chain.to_vec(),
                input: input2.to_vec(),
            },
        ],
    };
    let body = norito::to_bytes(&req).expect("encode batch");
    let tlv_env = make_tlv(PointerType::NoritoBytes as u16, &body);

    let mut vm = IVM::new(10_000);
    vm.memory.preload_input(0, &tlv_env).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);

    let prog = assemble_syscalls(&[ivm::syscalls::SYSCALL_VRF_VERIFY_BATCH as u8]);
    vm.load_program(&prog).unwrap();
    vm.run().unwrap();

    assert_eq!(vm.register(11), 0, "status ok");
    let p = vm.register(10);
    let tlv = vm.memory.validate_tlv(p).expect("valid tlv");
    assert_eq!(tlv.type_id, PointerType::NoritoBytes);
    let outs: Vec<[u8; 32]> = norito::decode_from_bytes(tlv.payload).expect("decode outs");
    assert_eq!(outs.len(), 2);
    assert_eq!(outs[0], exp1);
    assert_eq!(outs[1], exp2);
}

#[test]
fn syscall_vrf_verify_batch_fail_index_is_reported() {
    // One good, one bad (mismatched variant/length), expect r11!=0 and r12==1
    let sk1 = {
        let mut b = [0u8; 32];
        for (i, x) in b.iter_mut().enumerate() {
            *x = (i as u8) + 1;
        }
        Scalar::from_bytes_be(&b).unwrap()
    };
    let pk1 = (G1Projective::generator() * sk1)
        .to_affine()
        .to_compressed();
    let chain = b"test-chain";
    let input1 = b"in1";
    let mut in1 = Vec::new();
    in1.extend_from_slice(b"iroha:vrf:v1:input|");
    in1.extend_from_slice(chain);
    in1.push(b'|');
    in1.extend_from_slice(input1);
    let msg1: [u8; 32] = iroha_crypto::Hash::new(&in1).into();
    let sig1 = (G2Projective::from(hash_to_g2(&msg1)) * sk1)
        .to_affine()
        .to_compressed();

    // Bad item: use G1 sig but claim variant 1 (expects G2 sig). Build a fake but valid-length G1 encoding.
    let bad_sig_g1 = G1Affine::generator().to_compressed();

    let req = VrfVerifyBatchRequest {
        items: vec![
            VrfVerifyRequest {
                variant: 1,
                pk: pk1.to_vec(),
                proof: sig1.to_vec(),
                chain_id: chain.to_vec(),
                input: input1.to_vec(),
            },
            VrfVerifyRequest {
                variant: 1,
                pk: pk1.to_vec(),
                proof: bad_sig_g1.to_vec(),
                chain_id: chain.to_vec(),
                input: input1.to_vec(),
            },
        ],
    };
    let body = norito::to_bytes(&req).expect("encode batch");
    let tlv_env = make_tlv(PointerType::NoritoBytes as u16, &body);

    let mut vm = IVM::new(10_000);
    vm.memory.preload_input(0, &tlv_env).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);

    let prog = assemble_syscalls(&[ivm::syscalls::SYSCALL_VRF_VERIFY_BATCH as u8]);
    vm.load_program(&prog).unwrap();
    vm.run().unwrap();

    assert_ne!(vm.register(11), 0, "status must be error");
    assert_eq!(vm.register(12), 1, "failing index must be 1");
}

#[test]
fn syscall_vrf_verify_batch_chain_mismatch_reports_index() {
    // Host enforces a fixed chain id; batch contains a mismatched item.
    // Expect: r11 = 8 (ERR_CHAIN) and r12 = index of first mismatch.
    let host_chain = b"chain-A";
    let bad_chain = b"chain-B";

    // Item 1: Valid (SigInG2) with correct chain id
    let sk1 = {
        let mut b = [0u8; 32];
        for (i, x) in b.iter_mut().enumerate() {
            *x = (i as u8) + 3;
        }
        Scalar::from_bytes_be(&b).unwrap()
    };
    let pk1 = (G1Projective::generator() * sk1)
        .to_affine()
        .to_compressed();
    let input1 = b"in-ok";
    let mut in1 = Vec::new();
    in1.extend_from_slice(b"iroha:vrf:v1:input|");
    in1.extend_from_slice(host_chain);
    in1.push(b'|');
    in1.extend_from_slice(input1);
    let msg1: [u8; 32] = iroha_crypto::Hash::new(&in1).into();
    let sig1 = (G2Projective::from(hash_to_g2(&msg1)) * sk1)
        .to_affine()
        .to_compressed();

    // Item 2: Any item with mismatched chain id; proof content is irrelevant because
    // the host rejects on chain check before verification.
    let sk2 = {
        let mut b = [0u8; 32];
        for (i, x) in b.iter_mut().enumerate() {
            *x = (i as u8) + 17;
        }
        Scalar::from_bytes_be(&b).unwrap()
    };
    let pk2 = (G2Projective::generator() * sk2)
        .to_affine()
        .to_compressed();
    let input2 = b"in-bad";
    let mut in2 = Vec::new();
    in2.extend_from_slice(b"iroha:vrf:v1:input|");
    in2.extend_from_slice(bad_chain);
    in2.push(b'|');
    in2.extend_from_slice(input2);
    let msg2: [u8; 32] = iroha_crypto::Hash::new(&in2).into();
    let sig2 = (G1Projective::from(hash_to_g1(&msg2)) * sk2)
        .to_affine()
        .to_compressed();

    let req = VrfVerifyBatchRequest {
        items: vec![
            VrfVerifyRequest {
                variant: 1,
                pk: pk1.to_vec(),
                proof: sig1.to_vec(),
                chain_id: host_chain.to_vec(),
                input: input1.to_vec(),
            },
            VrfVerifyRequest {
                variant: 2,
                pk: pk2.to_vec(),
                proof: sig2.to_vec(),
                chain_id: bad_chain.to_vec(),
                input: input2.to_vec(),
            },
        ],
    };
    let body = norito::to_bytes(&req).expect("encode batch");
    let tlv_env = make_tlv(PointerType::NoritoBytes as u16, &body);

    let mut vm = IVM::new(10_000);
    // Configure host to enforce chain id = host_chain
    vm.set_host(DefaultHost::new().with_chain_id(host_chain.to_vec()));
    vm.memory.preload_input(0, &tlv_env).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);

    let prog = assemble_syscalls(&[ivm::syscalls::SYSCALL_VRF_VERIFY_BATCH as u8]);
    vm.load_program(&prog).unwrap();
    vm.run().unwrap();

    assert_eq!(vm.register(11), 8, "status must be ERR_CHAIN (8)");
    assert_eq!(vm.register(12), 1, "failing index must be 1");
    assert_eq!(vm.register(10), 0, "output pointer must be 0 on error");
}
