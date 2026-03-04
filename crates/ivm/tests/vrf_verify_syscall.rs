use ivm::{IVM, Memory, PointerType};

mod common;
// Helpers: BLS Hash-to-curve mirroring host logic
use blstrs::{G1Affine, G1Projective, G2Affine, G2Projective, Scalar};
use common::assemble_syscalls;
use group::{Curve, Group};
use ivm::vrf::VrfVerifyRequest;

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
fn syscall_vrf_verify_normal_returns_expected_output() {
    // Deterministic secret key from seed 0x01..0x20
    let mut seed32 = [0u8; 32];
    for (i, b) in seed32.iter_mut().enumerate() {
        *b = (i as u8) + 1;
    }
    let sk = Scalar::from_bytes_be(&seed32).expect("scalar from seed");
    let pk = (G1Projective::generator() * sk).to_affine();
    let pk_bytes = pk.to_compressed();

    // Input and message prehash (with chain id)
    let input = b"ivm:vrf:test";
    let chain = b"test-chain";
    let mut in_buf =
        Vec::with_capacity(b"iroha:vrf:v1:input|".len() + chain.len() + 1 + input.len());
    in_buf.extend_from_slice(b"iroha:vrf:v1:input|");
    in_buf.extend_from_slice(chain);
    in_buf.push(b'|');
    in_buf.extend_from_slice(input);
    let msg: [u8; 32] = iroha_crypto::Hash::new(&in_buf).into();

    // Signature in G2: sigma = H2(msg)^sk
    let h = hash_to_g2(&msg);
    let sigma = (G2Projective::from(h) * sk).to_affine().to_compressed();

    // Expected output: y = Hash("iroha:vrf:v1:output" || sigma)
    let mut out_buf = Vec::with_capacity(b"iroha:vrf:v1:output".len() + sigma.len());
    out_buf.extend_from_slice(b"iroha:vrf:v1:output");
    out_buf.extend_from_slice(&sigma);
    let y_exp: [u8; 32] = iroha_crypto::Hash::new(&out_buf).into();

    // Build Norito envelope and TLV
    let req = VrfVerifyRequest {
        variant: 1,
        pk: pk_bytes.to_vec(),
        proof: sigma.to_vec(),
        chain_id: chain.to_vec(),
        input: input.to_vec(),
    };
    let body = norito::to_bytes(&req).expect("encode vrf request");
    let tlv_env = make_tlv(PointerType::NoritoBytes as u16, &body);

    let mut vm = IVM::new(10_000);
    vm.memory.preload_input(0, &tlv_env).expect("preload input");
    let p_env = Memory::INPUT_START;
    vm.set_register(10, p_env);

    let prog = assemble_syscalls(&[ivm::syscalls::SYSCALL_VRF_VERIFY as u8]);
    vm.load_program(&prog).unwrap();
    vm.run().unwrap();
    let p_out = vm.register(10);
    let status = vm.register(11);
    assert_eq!(status, 0, "status must be OK");
    assert!(p_out != 0, "must return output pointer on success");

    // Validate returned TLV
    let tlv = vm.memory.validate_tlv(p_out).expect("valid tlv");
    assert_eq!(tlv.type_id, PointerType::Blob);
    assert_eq!(tlv.payload.len(), 32);
    assert_eq!(tlv.payload, &y_exp);
}

#[test]
fn syscall_vrf_verify_chain_mismatch_rejected() {
    use blstrs::{G2Projective, Scalar};
    // Prover side: build a valid tuple for chain "A"
    let sk = {
        let mut b = [0u8; 32];
        for (i, x) in b.iter_mut().enumerate() {
            *x = (i as u8) + 7;
        }
        Scalar::from_bytes_be(&b).unwrap()
    };
    let pk_g1 = (G1Projective::generator() * sk).to_affine().to_compressed();
    let input = b"in";
    let chain_a = b"chain-A";
    let mut m = Vec::new();
    m.extend_from_slice(b"iroha:vrf:v1:input|");
    m.extend_from_slice(chain_a);
    m.push(b'|');
    m.extend_from_slice(input);
    let msg: [u8; 32] = iroha_crypto::Hash::new(&m).into();
    let sig = (G2Projective::from(hash_to_g2(&msg)) * sk)
        .to_affine()
        .to_compressed();

    // Host configured with chain "B" must reject when envelope carries chain "A"
    let req = VrfVerifyRequest {
        variant: 1,
        pk: pk_g1.to_vec(),
        proof: sig.to_vec(),
        chain_id: chain_a.to_vec(),
        input: input.to_vec(),
    };
    let body = norito::to_bytes(&req).expect("encode req");
    let tlv_env = make_tlv(PointerType::NoritoBytes as u16, &body);

    let mut vm = IVM::new(10_000);
    vm.set_host(ivm::host::DefaultHost::new().with_chain_id(b"chain-B".to_vec()));
    vm.memory.preload_input(0, &tlv_env).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);

    let prog = assemble_syscalls(&[ivm::syscalls::SYSCALL_VRF_VERIFY as u8]);
    vm.load_program(&prog).unwrap();
    vm.run().unwrap();

    assert_eq!(vm.register(10), 0, "no output pointer on chain mismatch");
    assert_eq!(vm.register(11), 8, "ERR_CHAIN=8");
}

#[test]
fn syscall_vrf_verify_rejects_wrong_proof_length() {
    // Deterministic secret key
    let mut seed32 = [0u8; 32];
    for (i, b) in seed32.iter_mut().enumerate() {
        *b = (i as u8) + 3;
    }
    let sk = Scalar::from_bytes_be(&seed32).expect("scalar from seed");
    let pk = (G1Projective::generator() * sk).to_affine();
    let pk_bytes = pk.to_compressed();

    // Input prehash
    let input = b"ivm:vrf:test:neg";
    let mut in_buf =
        Vec::with_capacity(b"iroha:vrf:v1:input|".len() + b"test-chain".len() + 1 + input.len());
    in_buf.extend_from_slice(b"iroha:vrf:v1:input|");
    in_buf.extend_from_slice(b"test-chain");
    in_buf.push(b'|');
    in_buf.extend_from_slice(input);
    let msg: [u8; 32] = iroha_crypto::Hash::new(&in_buf).into();

    // Construct a G1 signature (48 bytes), but claim variant=1 (SigInG2 required)
    let h1 = hash_to_g1(&msg);
    let sig_g1 = (G1Projective::from(h1) * sk).to_affine().to_compressed();

    let req = VrfVerifyRequest {
        variant: 1,
        pk: pk_bytes.to_vec(),
        proof: sig_g1.to_vec(),
        chain_id: b"test-chain".to_vec(),
        input: input.to_vec(),
    };
    let body = norito::to_bytes(&req).expect("encode");
    let tlv_env = make_tlv(PointerType::NoritoBytes as u16, &body);

    let mut vm = IVM::new(10_000);
    vm.memory.preload_input(0, &tlv_env).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);

    let prog = assemble_syscalls(&[ivm::syscalls::SYSCALL_VRF_VERIFY as u8]);
    vm.load_program(&prog).unwrap();
    vm.run().unwrap();

    assert_eq!(vm.register(10), 0, "no output on failure");
    let status = vm.register(11);
    // Expect ERR_PROOF (5) or ERR_VERIFY (6) depending on exact failure point
    assert!(status == 5 || status == 6, "unexpected status: {status}");
}
