#![doc = "End-to-end test for the native STARK (FRI single-fold) verifier."]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(feature = "zk-tests")]
//! End-to-end test for the native STARK (FRI single-fold) verifier.

#![cfg(feature = "zk-stark")]
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use iroha_core::{
    zk::verify_backend,
    zk_stark::{
        StarkCompositionTermV1, StarkFriParamsV1, StarkFriVerifyingKeyV1, StarkVerifierLimits,
        StarkVerifyEnvelopeV1, prove_stark_fri_air_envelope_bytes,
        prove_stark_fri_composition_envelope_bytes, verify_stark_fri_envelope,
        verify_stark_fri_envelope_with_limits,
    },
};
fn test_digest(word: u64) -> iroha_data_model::privacy::GoldilocksDigest384V1 {
    iroha_data_model::privacy::GoldilocksDigest384V1::new([word; 6])
        .expect("canonical native STARK test digest")
}
fn mutate_digest(digest: &mut iroha_data_model::privacy::GoldilocksDigest384V1) {
    let mut words = digest.words();
    words[0] ^= 1;
    *digest = iroha_data_model::privacy::GoldilocksDigest384V1::new(words)
        .expect("mutated native STARK test digest remains canonical");
}
fn mutate_fp4(value: &mut iroha_core::zk_stark::GoldilocksFp4V1) {
    let mut coefficients = value.coefficients();
    coefficients[0] ^= 1;
    *value = iroha_core::zk_stark::GoldilocksFp4V1::new(coefficients)
        .expect("mutated native STARK Fp4 value remains canonical");
}
fn stark_open_verify_domain_tag_current(
    backend: &str,
    circuit_id: &str,
    vk_hash: [u8; 32],
    env_public_inputs: &[u8],
    public_inputs: &[Vec<[u8; 32]>],
) -> String {
    let mut preimage = Vec::new();
    preimage.extend_from_slice(b"iroha:zk:stark-binding-air:v1");
    preimage.extend_from_slice(&(backend.len() as u64).to_le_bytes());
    preimage.extend_from_slice(backend.as_bytes());
    preimage.extend_from_slice(&(circuit_id.len() as u64).to_le_bytes());
    preimage.extend_from_slice(circuit_id.as_bytes());
    preimage.extend_from_slice(&vk_hash);
    preimage.extend_from_slice(&(env_public_inputs.len() as u64).to_le_bytes());
    preimage.extend_from_slice(env_public_inputs);
    preimage.extend_from_slice(&(public_inputs.len() as u64).to_le_bytes());
    let mut cell_count = 0_u64;
    for column in public_inputs {
        preimage.extend_from_slice(&(column.len() as u64).to_le_bytes());
        cell_count = cell_count.saturating_add(column.len() as u64);
        for value in column {
            preimage.extend_from_slice(value);
        }
    }
    preimage.extend_from_slice(&cell_count.to_le_bytes());
    let digest = iroha_core::zk_stark::stark_open_verify_domain_digest_v1(&preimage)
        .expect("derive six-lane OpenVerify domain digest");
    URL_SAFE_NO_PAD.encode(digest.to_le_bytes())
}
fn build_sample_envelope_with_domain_tag(domain_tag: String) -> StarkVerifyEnvelopeV1 {
    let mut envelope = build_sample_air_envelope_with_domain_tag(domain_tag);
    envelope.proof.air = None;
    envelope
}
fn build_sample_envelope() -> StarkVerifyEnvelopeV1 {
    build_sample_envelope_with_domain_tag("fastpq:v1:fri".to_owned())
}
fn sample_air_params(domain_tag: String) -> StarkFriParamsV1 {
    StarkFriParamsV1 {
        version: 1,
        n_log2: 6,
        blowup_log2: 3,
        fold_arity: 2,
        queries: iroha_core::zk_stark::STARK_FRI_CONSENSUS_MIN_QUERIES,
        merkle_arity: 2,
        domain_tag,
    }
}
fn build_sample_air_envelope_with_domain_tag(domain_tag: String) -> StarkVerifyEnvelopeV1 {
    let bytes = prove_stark_fri_air_envelope_bytes(
        sample_air_params(domain_tag),
        "TEST-STARK".to_string(),
        "stark/fri/poseidon-x7-goldilocks-6x64-v1:test".to_owned(),
        test_digest(0x42),
    )
    .expect("build sample AIR envelope");
    norito::decode_from_bytes(&bytes).expect("decode sample AIR envelope")
}
fn build_sample_air_envelope() -> StarkVerifyEnvelopeV1 {
    build_sample_air_envelope_with_domain_tag("fastpq:v1:fri".to_owned())
}
fn sample_composition_terms() -> Vec<StarkCompositionTermV1> {
    vec![
        StarkCompositionTermV1 {
            wire_index: 0,
            value: 11,
            coeff: 3,
        },
        StarkCompositionTermV1 {
            wire_index: 1,
            value: 17,
            coeff: 5,
        },
    ]
}
fn build_sample_air_composition_envelope_with_domain_tag(
    domain_tag: String,
) -> StarkVerifyEnvelopeV1 {
    let bytes = prove_stark_fri_composition_envelope_bytes(
        sample_air_params(domain_tag),
        "TEST-STARK".to_string(),
        7,
        2,
        sample_composition_terms(),
    )
    .expect("build sample AIR composition envelope");
    norito::decode_from_bytes(&bytes).expect("decode sample AIR composition envelope")
}
fn build_sample_air_composition_envelope() -> StarkVerifyEnvelopeV1 {
    build_sample_air_composition_envelope_with_domain_tag("fastpq:v1:fri".to_string())
}
fn build_stark_open_verify_envelope_bytes_for_columns(
    backend: &str,
    circuit_id: &str,
    vk_hash: [u8; 32],
    schema_descriptor: &[u8],
    public_inputs: Vec<Vec<[u8; 32]>>,
) -> Vec<u8> {
    use iroha_data_model::zk::{BackendTag, OpenVerifyEnvelope, StarkFriOpenProofV1};
    let domain_tag = stark_open_verify_domain_tag_current(
        backend,
        circuit_id,
        vk_hash,
        schema_descriptor,
        &public_inputs,
    );
    let inner = build_sample_envelope_with_domain_tag(domain_tag);
    let envelope_bytes = norito::to_bytes(&inner).expect("encode STARK inner envelope");
    let open = StarkFriOpenProofV1 {
        version: 1,
        public_inputs,
        envelope_bytes,
    };
    let proof_bytes = norito::to_bytes(&open).expect("encode STARK open proof");
    let env = OpenVerifyEnvelope {
        backend: BackendTag::Stark,
        circuit_id: circuit_id.to_string(),
        vk_hash,
        public_inputs: schema_descriptor.to_vec(),
        proof_bytes,
        aux: Vec::new(),
    };
    norito::to_bytes(&env).expect("encode OpenVerifyEnvelope")
}
fn derive_ballot_nullifier_for_test(
    domain_tag: &str,
    network_id: &iroha_data_model::NetworkId,
    election_id: &str,
    commit: &[u8; 32],
) -> [u8; 32] {
    use blake2::{Blake2b512, Digest as _};
    let mut input = Vec::with_capacity(
        domain_tag.len() + network_id.as_bytes().len() + election_id.len() + commit.len() + 24,
    );
    let push_len = |buf: &mut Vec<u8>, len: usize| {
        let len_u64 = len as u64;
        buf.extend_from_slice(&len_u64.to_le_bytes());
    };
    push_len(&mut input, domain_tag.len());
    input.extend_from_slice(domain_tag.as_bytes());
    push_len(&mut input, network_id.as_bytes().len());
    input.extend_from_slice(network_id.as_bytes());
    push_len(&mut input, election_id.len());
    input.extend_from_slice(election_id.as_bytes());
    input.extend_from_slice(commit);
    let digest = Blake2b512::digest(&input);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest[..32]);
    out
}
fn sample_stark_vk_box(
    backend: &str,
    circuit_id: &str,
) -> iroha_data_model::proof::VerifyingKeyBox {
    let payload = StarkFriVerifyingKeyV1 {
        version: 1,
        circuit_id: circuit_id.to_string(),
        n_log2: 6,
        blowup_log2: 3,
        fold_arity: 2,
        queries: iroha_core::zk_stark::STARK_FRI_CONSENSUS_MIN_QUERIES,
        merkle_arity: 2,
    };
    let bytes = norito::to_bytes(&payload).expect("encode STARK verifying key payload");
    iroha_data_model::proof::VerifyingKeyBox::new(backend.into(), bytes)
}
#[test]
fn stark_single_fold_roundtrip_ok_and_fail() {
    let env = build_sample_air_composition_envelope();
    let bytes = norito::to_bytes(&env).expect("encode");
    let native_ok = iroha_core::zk_stark::verify_stark_fri_envelope(&bytes);
    assert!(native_ok, "native STARK verifier rejected sample envelope");
    // Tamper auxiliary term and expect rejection
    let mut env_bad_comp = env.clone();
    if let Some(ref mut entries) = env_bad_comp.proof.comp_values {
        entries[0].aux_terms[0].coeff = entries[0].aux_terms[0].coeff.wrapping_add(1);
    }
    let bytes_bad_comp = norito::to_bytes(&env_bad_comp).expect("encode");
    assert!(
        !verify_stark_fri_envelope(&bytes_bad_comp),
        "tampered composition term should fail"
    );
    // Tamper with the derived index and expect rejection
    let mut env_bad_index = env.clone();
    env_bad_index.proof.queries[0][0].j = env_bad_index.proof.queries[0][0].j.wrapping_add(1);
    let bytes_bad_index = norito::to_bytes(&env_bad_index).expect("encode");
    assert!(
        !verify_stark_fri_envelope(&bytes_bad_index),
        "tampered query index should fail"
    );
    // Corrupt one z1 value and expect failure
    let mut env_bad = env.clone();
    mutate_fp4(&mut env_bad.proof.queries[0][1].z);
    let bytes_bad = norito::to_bytes(&env_bad).expect("encode");
    assert!(
        !verify_stark_fri_envelope(&bytes_bad),
        "tampered STARK proof should fail"
    );
    // Wrong root should fail deterministically
    let mut env_bad_root = env.clone();
    mutate_digest(&mut env_bad_root.proof.commits.roots[0]);
    let bytes_bad_root = norito::to_bytes(&env_bad_root).expect("encode");
    assert!(
        !verify_stark_fri_envelope(&bytes_bad_root),
        "tampered root must fail"
    );
    // Broken Merkle path should fail
    let mut env_bad_path = env.clone();
    mutate_digest(&mut env_bad_path.proof.queries[0][0].path_y0.siblings[0]);
    let bytes_bad_path = norito::to_bytes(&env_bad_path).expect("encode");
    assert!(
        !verify_stark_fri_envelope(&bytes_bad_path),
        "broken Merkle path should fail"
    );
    // Round-count/roots mismatch should fail
    let mut env_bad_rounds = env.clone();
    env_bad_rounds.proof.commits.roots.pop();
    env_bad_rounds.proof.queries[0].pop();
    let bytes_bad_rounds = norito::to_bytes(&env_bad_rounds).expect("encode");
    assert!(
        !verify_stark_fri_envelope(&bytes_bad_rounds),
        "mismatched round count should fail validation"
    );
    // Query-count/header mismatch should fail
    let mut env_bad_query_header = env.clone();
    env_bad_query_header.params.queries = 2;
    let bytes_bad_query_header = norito::to_bytes(&env_bad_query_header).expect("encode");
    assert!(
        !verify_stark_fri_envelope(&bytes_bad_query_header),
        "mismatched query count in params should fail"
    );
}
#[test]
fn stark_rejects_duplicate_auxiliary_composition_wires() {
    let mut env = build_sample_air_composition_envelope();
    let comp_values = env
        .proof
        .comp_values
        .as_mut()
        .expect("sample composition envelope has composition values");
    let first_wire = comp_values[0].aux_terms[0].wire_index;
    comp_values[0].aux_terms[1].wire_index = first_wire;
    let bytes = norito::to_bytes(&env).expect("encode duplicate auxiliary wires");
    assert!(
        !verify_stark_fri_envelope(&bytes),
        "duplicate auxiliary composition wires must be rejected"
    );
}
#[test]
fn stark_rejects_auxiliary_composition_wire_retarget_without_digest_match() {
    let mut env = build_sample_air_composition_envelope();
    let comp_values = env
        .proof
        .comp_values
        .as_mut()
        .expect("sample composition envelope has composition values");
    comp_values[0].aux_terms[1].wire_index =
        comp_values[0].aux_terms[1].wire_index.saturating_add(1);
    let bytes = norito::to_bytes(&env).expect("encode retargeted auxiliary wire");
    assert!(
        !verify_stark_fri_envelope(&bytes),
        "auxiliary wire-index retargeting must remain bound to the AIR public digest"
    );
}
#[test]
fn stark_composition_constructor_requires_strict_auxiliary_wire_order() {
    let params = sample_air_params("fastpq:v1:fri".to_string());
    let duplicate_terms = vec![
        StarkCompositionTermV1 {
            wire_index: 1,
            value: 11,
            coeff: 3,
        },
        StarkCompositionTermV1 {
            wire_index: 1,
            value: 17,
            coeff: 5,
        },
    ];
    let duplicate_err = prove_stark_fri_composition_envelope_bytes(
        params.clone(),
        "TEST-STARK".to_string(),
        7,
        2,
        duplicate_terms,
    )
    .expect_err("duplicate auxiliary wires must fail before proof construction");
    assert!(
        duplicate_err.contains("strictly ordered"),
        "unexpected duplicate-wire error: {duplicate_err}"
    );
    let unsorted_terms = vec![
        StarkCompositionTermV1 {
            wire_index: 2,
            value: 11,
            coeff: 3,
        },
        StarkCompositionTermV1 {
            wire_index: 1,
            value: 17,
            coeff: 5,
        },
    ];
    let unsorted_err = prove_stark_fri_composition_envelope_bytes(
        params,
        "TEST-STARK".to_string(),
        7,
        2,
        unsorted_terms,
    )
    .expect_err("unsorted auxiliary wires must fail before proof construction");
    assert!(
        unsorted_err.contains("strictly ordered"),
        "unexpected unsorted-wire error: {unsorted_err}"
    );
}
#[test]
fn stark_low_level_envelope_requires_air_section() {
    let env = build_sample_envelope();
    let bytes = norito::to_bytes(&env).expect("encode");
    assert!(
        !verify_stark_fri_envelope(&bytes),
        "native STARK verifier must reject V1 envelopes without AIR openings"
    );
}
#[test]
fn stark_six_lane_fp4_roundtrip_ok() {
    let env = build_sample_air_envelope();
    let bytes = norito::to_bytes(&env).expect("encode");
    assert!(
        verify_stark_fri_envelope(&bytes),
        "native STARK verifier rejected the canonical six-lane Fp4 envelope"
    );
}
#[test]
fn stark_rejects_mismatched_merkle_indices() {
    let mut env = build_sample_air_envelope_with_domain_tag("index-test".to_string());
    let first = &mut env.proof.queries[0][0];
    core::mem::swap(&mut first.path_y0, &mut first.path_y1);
    let bytes = norito::to_bytes(&env).expect("encode");
    assert!(
        !verify_stark_fri_envelope(&bytes),
        "index-mismatched Merkle openings must be rejected"
    );
}
#[test]
fn stark_rejects_unbound_air_composition_root() {
    let mut env = build_sample_air_composition_envelope();
    mutate_digest(
        &mut env
            .proof
            .air
            .as_mut()
            .expect("AIR section")
            .composition_root,
    );
    let bytes = norito::to_bytes(&env).expect("encode");
    assert!(
        !verify_stark_fri_envelope(&bytes),
        "AIR composition root must authenticate the base-field composition openings"
    );
}
#[test]
fn stark_rejects_tampered_air_trace_root() {
    let mut env = build_sample_air_composition_envelope();
    mutate_digest(&mut env.proof.air.as_mut().expect("AIR section").trace_root);
    let bytes = norito::to_bytes(&env).expect("encode");
    assert!(
        !verify_stark_fri_envelope(&bytes),
        "AIR trace root must authenticate sampled trace rows"
    );
}
#[test]
fn stark_rejects_tampered_air_public_digest() {
    let mut env = build_sample_air_composition_envelope();
    mutate_digest(&mut env.proof.air.as_mut().expect("AIR section").public_digest);
    let bytes = norito::to_bytes(&env).expect("encode");
    assert!(
        !verify_stark_fri_envelope(&bytes),
        "AIR public digest must remain bound to sampled rows and composition openings"
    );
}
#[test]
fn stark_rejects_air_trace_width_mismatch() {
    let mut env = build_sample_air_composition_envelope();
    let air = env.proof.air.as_mut().expect("AIR section");
    air.trace_width = air.trace_width.saturating_add(1);
    let bytes = norito::to_bytes(&env).expect("encode");
    assert!(
        !verify_stark_fri_envelope(&bytes),
        "AIR trace width must match the V1 AIR layout"
    );
}
#[test]
fn stark_rejects_air_opening_count_mismatch() {
    let mut env = build_sample_air_composition_envelope();
    let air = env.proof.air.as_mut().expect("AIR section");
    assert_eq!(air.openings.len(), env.proof.queries.len());
    air.openings.clear();
    let bytes = norito::to_bytes(&env).expect("encode");
    assert!(
        !verify_stark_fri_envelope(&bytes),
        "AIR opening count must match verifier query count"
    );
}
#[test]
fn stark_air_width_limit_is_enforced() {
    let env = build_sample_air_composition_envelope();
    let bytes = norito::to_bytes(&env).expect("encode");
    let trace_width = env.proof.air.as_ref().expect("AIR section").trace_width as usize;
    assert!(trace_width > 1, "sample AIR trace must have width");
    let mut limits = StarkVerifierLimits::default();
    limits.max_air_width = trace_width - 1;
    assert!(
        !verify_stark_fri_envelope_with_limits(&bytes, &limits),
        "AIR trace width must respect verifier limits"
    );
}
#[test]
fn stark_open_verify_envelope_rejects_synthetic_air_proof() {
    use iroha_data_model::{
        proof::ProofBox,
        zk::{BackendTag, OpenVerifyEnvelope, StarkFriOpenProofV1},
    };
    let backend = "stark/fri/poseidon-x7-goldilocks-6x64-v1";
    let circuit_id = "ivm-execution-v1";
    let vk_box = sample_stark_vk_box(backend, circuit_id);
    let vk_hash = iroha_core::zk::hash_vk(&vk_box);
    // Two columns, one row each (matches the instance-column shape used by other backends).
    let public_inputs = vec![vec![[0xAA; 32]], vec![[0xBB; 32]]];
    let env_public_inputs = b"schema:test".to_vec();
    let domain_tag = stark_open_verify_domain_tag_current(
        backend,
        circuit_id,
        vk_hash,
        &env_public_inputs,
        &public_inputs,
    );
    let inner = build_sample_envelope_with_domain_tag(domain_tag);
    let envelope_bytes = norito::to_bytes(&inner).expect("encode stark envelope");
    let open = StarkFriOpenProofV1 {
        version: 1,
        public_inputs: public_inputs.clone(),
        envelope_bytes,
    };
    let proof_bytes = norito::to_bytes(&open).expect("encode open proof");
    let env = OpenVerifyEnvelope {
        backend: BackendTag::Stark,
        circuit_id: circuit_id.to_string(),
        vk_hash,
        public_inputs: env_public_inputs,
        proof_bytes,
        aux: Vec::new(),
    };
    let proof = ProofBox::new(
        backend.into(),
        norito::to_bytes(&env).expect("encode OpenVerifyEnvelope"),
    );
    assert!(
        !verify_backend(backend, &proof, Some(&vk_box)),
        "wrapped STARK OpenVerifyEnvelope must fail closed when the AIR section is missing"
    );
    // Changing circuit_id without updating the inner envelope's `domain_tag` must fail.
    let mut env_bad = env;
    env_bad.circuit_id = "other-circuit".to_string();
    let proof_bad = ProofBox::new(
        backend.into(),
        norito::to_bytes(&env_bad).expect("encode tampered OpenVerifyEnvelope"),
    );
    assert!(
        !verify_backend(backend, &proof_bad, Some(&vk_box)),
        "STARK OpenVerifyEnvelope must bind circuit_id via domain_tag"
    );
}
#[test]
fn retired_poseidon2_backend_label_is_rejected() {
    use iroha_data_model::proof::ProofBox;
    let retired_backend = "stark/fri/poseidon2-goldilocks";
    let proof = ProofBox::new(retired_backend.to_owned(), vec![0xA5]);
    assert!(
        !verify_backend(retired_backend, &proof, None),
        "the pre-release Poseidon2 backend label must remain rejection-only"
    );
}
fn hash_to_u64_limbs_le(hash: &iroha_crypto::Hash) -> [u64; 4] {
    let bytes: &[u8; 32] = hash.as_ref();
    let mut limbs = [0u64; 4];
    for (i, limb) in limbs.iter_mut().enumerate() {
        let start = i * 8;
        let end = start + 8;
        *limb = u64::from_le_bytes(bytes[start..end].try_into().expect("slice len = 8"));
    }
    limbs
}
fn limb_as_instance_bytes(limb: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[..8].copy_from_slice(&limb.to_le_bytes());
    out
}
fn expected_ivm_exec_public_inputs(
    code_hash: iroha_crypto::Hash,
    overlay_hash: iroha_crypto::Hash,
    events_commitment: iroha_crypto::Hash,
    gas_policy_commitment: iroha_crypto::Hash,
) -> Vec<[u8; 32]> {
    let code_limbs = hash_to_u64_limbs_le(&code_hash);
    let overlay_limbs = hash_to_u64_limbs_le(&overlay_hash);
    let events_limbs = hash_to_u64_limbs_le(&events_commitment);
    let gas_limbs = hash_to_u64_limbs_le(&gas_policy_commitment);
    code_limbs
        .into_iter()
        .chain(overlay_limbs)
        .chain(events_limbs)
        .chain(gas_limbs)
        .map(limb_as_instance_bytes)
        .collect()
}
#[test]
fn stark_ivm_proved_execution_admission_rejects_synthetic_air_proof() {
    use iroha_crypto::Hash;
    use iroha_data_model::{
        Registrable,
        account::Account,
        confidential::ConfidentialStatus,
        domain::Domain,
        prelude::{AccountId, IvmBytecode, TransactionBuilder},
        proof::{
            ProofAttachment, ProofAttachmentList, ProofBox, VerifyingKeyId, VerifyingKeyRecord,
        },
        transaction::{Executable, IvmProved},
        zk::{BackendTag, OpenVerifyEnvelope, StarkFriOpenProofV1},
    };
    use std::sync::Arc;
    let backend = "stark/fri/poseidon-x7-goldilocks-6x64-v1";
    let circuit_id = "ivm-execution-v1";
    // Minimal ZK-mode IVM program: metadata + `HALT`.
    let meta = ivm::ProgramMetadata {
        max_cycles: 1,
        mode: ivm::ivm_mode::ZK,
        ..ivm::ProgramMetadata::default()
    };
    let mut program = meta.encode();
    program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let bytecode = IvmBytecode::from_compiled(program);
    let kp = checked_zk_stark_keypair();
    let authority = AccountId::new(kp.public_key().clone());
    let domain_id: iroha_data_model::domain::DomainId =
        iroha_data_model::domain::DomainId::try_new("wonderland", "universal").unwrap();
    let domain = Domain::new(domain_id.clone()).build(&authority);
    let account = Account::new(authority.clone()).build(&authority);
    let world = iroha_core::state::World::with([domain], [account], []);
    let vk_id = VerifyingKeyId::new(backend, "ivm_execution_stark");
    let vk_box = sample_stark_vk_box(backend, circuit_id);
    let vk_hash = iroha_core::zk::hash_vk(&vk_box);
    let mut vk_record = VerifyingKeyRecord::new(
        1,
        format!("{backend}:{circuit_id}"),
        BackendTag::Stark,
        "goldilocks",
        iroha_core::zk::ivm_execution_public_inputs_schema_hash(),
        vk_hash,
    );
    vk_record.status = ConfidentialStatus::Active;
    vk_record.gas_schedule_id = Some("sched_0".to_owned());
    vk_record.max_proof_bytes = 8 * 1024 * 1024;
    vk_record.key = Some(vk_box.clone());
    {
        let mut wb = world.block();
        wb.verifying_keys_mut_for_testing()
            .insert(vk_id.clone(), vk_record.clone());
        wb.verifying_keys_by_circuit_mut_for_testing().insert(
            (vk_record.circuit_id.clone(), vk_record.version),
            vk_id.clone(),
        );
        wb.commit();
    }
    let kura = Arc::new(iroha_core::kura::Kura::blank_kura_for_testing());
    let query = iroha_core::query::store::LiveQueryStore::start_test();
    let mut state = iroha_core::state::State::new_for_testing(world, Arc::clone(&kura), query);
    state.zk.halo2.enabled = false;
    state.zk.stark.enabled = true;
    const TEST_GAS_LIMIT: u64 = 50_000_000;
    // Derive the proved payload by executing the IVM program once.
    let tx = TransactionBuilder::new(
        *state.network_id_ref(),
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(
            Vec::new(),
            core::num::NonZeroU64::new(TEST_GAS_LIMIT),
        ),
    )
    .with_executable(Executable::Ivm(bytecode.clone()))
    .sign(kp.private_key());
    let proved = iroha_core::pipeline::overlay::derive_ivm_proved_payload_from_ivm_execution(
        &state.view(),
        &tx,
        &vk_record,
    )
    .expect("derive proved payload");
    // Compute the ivm-execution-v1 public inputs and package them as STARK wrapper columns.
    let mut ivm_cache = iroha_core::smartcontracts::ivm::cache::IvmCache::new();
    let summary = ivm_cache
        .summarize_program(proved.bytecode.as_ref())
        .expect("summarize IVM program");
    let overlay_hash = {
        let bytes = norito::to_bytes(&proved.overlay).expect("encode overlay");
        Hash::new(&bytes)
    };
    let inputs = expected_ivm_exec_public_inputs(
        summary.code_hash,
        overlay_hash,
        proved.events_commitment,
        proved.gas_policy_commitment,
    );
    let public_inputs = inputs.into_iter().map(|v| vec![v]).collect::<Vec<_>>();
    // Public-input schema descriptor is the same for both Halo2 and STARK wrappers.
    let env_public_inputs =
        iroha_core::zk::ivm_execution_public_inputs_schema_descriptor().to_vec();
    let domain_tag = stark_open_verify_domain_tag_current(
        backend,
        circuit_id,
        vk_hash,
        &env_public_inputs,
        &public_inputs,
    );
    let inner = build_sample_envelope_with_domain_tag(domain_tag);
    let envelope_bytes = norito::to_bytes(&inner).expect("encode stark envelope");
    let open = StarkFriOpenProofV1 {
        version: 1,
        public_inputs: public_inputs.clone(),
        envelope_bytes,
    };
    let proof_bytes = norito::to_bytes(&open).expect("encode open proof");
    let env = OpenVerifyEnvelope {
        backend: iroha_data_model::zk::BackendTag::Stark,
        circuit_id: circuit_id.to_string(),
        vk_hash,
        public_inputs: env_public_inputs,
        proof_bytes,
        aux: Vec::new(),
    };
    let proof_box = ProofBox::new(
        backend.into(),
        norito::to_bytes(&env).expect("encode OpenVerifyEnvelope"),
    );
    let attachment = ProofAttachment::new_ref(backend.into(), proof_box, vk_id);
    let attachments = ProofAttachmentList::try_from(vec![attachment])
        .expect("one attachment is a valid bounded proof list");
    let tx_proved = TransactionBuilder::new(
        *state.network_id_ref(),
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(
            Vec::new(),
            core::num::NonZeroU64::new(TEST_GAS_LIMIT),
        ),
    )
    .with_executable(Executable::IvmProved(IvmProved {
        bytecode: proved.bytecode.clone(),
        overlay: proved.overlay.clone(),
        events_commitment: proved.events_commitment,
        gas_policy_commitment: proved.gas_policy_commitment,
    }))
    .with_attachments(attachments)
    .sign(kp.private_key());
    let err =
        iroha_core::pipeline::overlay::build_overlay_for_transaction(&tx_proved, &state.view())
            .expect_err("synthetic STARK proved execution must be rejected");
    let err_text = format!("{err:?}");
    assert!(
        err_text.contains("proof rejected"),
        "unexpected proved execution rejection: {err:?}"
    );
}
#[test]
fn create_election_rejects_generic_stark_vote_role_labels() {
    use core::num::NonZeroU64;
    use iroha_core::{
        kura::Kura,
        query::store::LiveQueryStore,
        smartcontracts::Execute,
        state::{State, World},
    };
    use iroha_data_model::{
        Registrable,
        account::Account,
        block::BlockHeader,
        confidential::ConfidentialStatus,
        domain::Domain,
        isi::{Grant, verifying_keys, zk::CreateElection},
        permission::Permission,
        proof::{VerifyingKeyId, VerifyingKeyRecord},
        zk::BackendTag,
    };
    use iroha_executor_data_model::permission::governance::CanManageParliament;
    use iroha_primitives::json::Json;
    use iroha_test_samples::ALICE_ID;
    let backend = "stark/fri/poseidon-x7-goldilocks-6x64-v1";
    let ballot_circuit_id = "stark/fri/poseidon-x7-goldilocks-6x64-v1:vote-ballot";
    let tally_circuit_id = "stark/fri/poseidon-x7-goldilocks-6x64-v1:vote-tally";
    let election_id = "stark-vote-e2e".to_string();
    let nullifier_domain = "gov:ballot:v1";
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let domain_id: iroha_data_model::domain::DomainId =
        iroha_data_model::domain::DomainId::try_new("wonderland", "universal").expect("domain");
    let domain: Domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
    let account: Account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let world = World::with([domain], [account], Vec::new());
    let mut state = State::new_for_testing(world, kura, query);
    state.zk.stark.enabled = true;
    state.zk.halo2.enabled = false;
    state.zk.verify_timeout = std::time::Duration::ZERO;
    state.gov.citizenship_bond_amount = 0_u64.into();
    state.gov.min_bond_amount = 0_u64.into();
    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let perm_vk = Permission::new("CanManageVerifyingKeys".to_string(), Json::new(()));
    Grant::account_permission(perm_vk, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant CanManageVerifyingKeys");
    let perm_parliament: Permission = CanManageParliament.into();
    Grant::account_permission(perm_parliament, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant CanManageParliament");
    let ballot_vk_id = VerifyingKeyId::new(backend, "vote_ballot");
    let ballot_vk_box = sample_stark_vk_box(backend, ballot_circuit_id);
    let ballot_vk_hash = iroha_core::zk::hash_vk(&ballot_vk_box);
    let ballot_schema = b"gov:vote:ballot:schema:v1".to_vec();
    let ballot_schema_hash: [u8; 32] = iroha_crypto::Hash::new(&ballot_schema).into();
    let mut ballot_vk_record = VerifyingKeyRecord::new(
        1,
        ballot_circuit_id,
        BackendTag::Stark,
        "goldilocks",
        ballot_schema_hash,
        ballot_vk_hash,
    );
    ballot_vk_record.status = ConfidentialStatus::Active;
    ballot_vk_record.gas_schedule_id = Some("sched_ballot".to_string());
    ballot_vk_record.key = Some(ballot_vk_box.clone());
    verifying_keys::RegisterVerifyingKey {
        id: ballot_vk_id.clone(),
        record: ballot_vk_record,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("register ballot vk");
    let tally_vk_id = VerifyingKeyId::new(backend, "vote_tally");
    let tally_vk_box = sample_stark_vk_box(backend, tally_circuit_id);
    let tally_vk_hash = iroha_core::zk::hash_vk(&tally_vk_box);
    let tally_schema = b"gov:vote:tally:schema:v1".to_vec();
    let tally_schema_hash: [u8; 32] = iroha_crypto::Hash::new(&tally_schema).into();
    let mut tally_vk_record = VerifyingKeyRecord::new(
        1,
        tally_circuit_id,
        BackendTag::Stark,
        "goldilocks",
        tally_schema_hash,
        tally_vk_hash,
    );
    tally_vk_record.status = ConfidentialStatus::Active;
    tally_vk_record.gas_schedule_id = Some("sched_tally".to_string());
    tally_vk_record.key = Some(tally_vk_box.clone());
    verifying_keys::RegisterVerifyingKey {
        id: tally_vk_id.clone(),
        record: tally_vk_record,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("register tally vk");
    let eligible_root = [0x22; 32];
    let err = CreateElection {
        election_id: election_id.clone(),
        options: 2,
        eligible_root,
        start_ts: 0,
        end_ts: 0,
        vk_ballot: ballot_vk_id.clone(),
        vk_tally: tally_vk_id.clone(),
        domain_tag: nullifier_domain.to_string(),
    }
    .execute(&ALICE_ID, &mut stx)
    .expect_err("generic STARK Binding AIR must not be admitted as a ballot circuit");
    let err_text = format!("{err:?}");
    assert!(
        err_text.contains("ballot verifying key circuit mismatch"),
        "unexpected generic STARK vote-role rejection: {err:?}"
    );
}
#[test]
fn create_election_rejects_stark_vk_with_wrong_vote_circuit_role() {
    use core::num::NonZeroU64;
    use iroha_core::{
        kura::Kura, query::store::LiveQueryStore, smartcontracts::Execute, state::State,
    };
    use iroha_data_model::{
        Registrable,
        account::Account,
        block::BlockHeader,
        confidential::ConfidentialStatus,
        domain::Domain,
        isi::{Grant, verifying_keys, zk::CreateElection},
        permission::Permission,
        proof::{VerifyingKeyId, VerifyingKeyRecord},
        zk::BackendTag,
    };
    use iroha_executor_data_model::permission::governance::CanManageParliament;
    use iroha_primitives::json::Json;
    use iroha_test_samples::ALICE_ID;
    let backend = "stark/fri/poseidon-x7-goldilocks-6x64-v1";
    let bad_ballot_circuit_id = "stark/fri/poseidon-x7-goldilocks-6x64-v1:not-a-ballot-circuit";
    let tally_circuit_id = "stark/fri/poseidon-x7-goldilocks-6x64-v1:vote-tally";
    let ballot_schema_hash: [u8; 32] = iroha_crypto::Hash::new(b"gov:vote:ballot:schema:v1").into();
    let tally_schema_hash: [u8; 32] = iroha_crypto::Hash::new(b"gov:vote:tally:schema:v1").into();
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let domain_id: iroha_data_model::domain::DomainId =
        iroha_data_model::domain::DomainId::try_new("wonderland", "universal").expect("domain");
    let domain: Domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
    let account: Account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let mut state = State::new_for_testing(
        iroha_core::state::World::with([domain], [account], Vec::new()),
        kura,
        query,
    );
    state.zk.stark.enabled = true;
    state.zk.halo2.enabled = false;
    state.zk.verify_timeout = std::time::Duration::ZERO;
    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let perm_vk = Permission::new("CanManageVerifyingKeys".to_string(), Json::new(()));
    Grant::account_permission(perm_vk, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant CanManageVerifyingKeys");
    let perm_parliament: Permission = CanManageParliament.into();
    Grant::account_permission(perm_parliament, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant CanManageParliament");
    let ballot_vk_id = VerifyingKeyId::new(backend, "bad_vote_ballot");
    let ballot_vk_box = sample_stark_vk_box(backend, bad_ballot_circuit_id);
    let mut ballot_vk_record = VerifyingKeyRecord::new(
        1,
        bad_ballot_circuit_id,
        BackendTag::Stark,
        "goldilocks",
        ballot_schema_hash,
        iroha_core::zk::hash_vk(&ballot_vk_box),
    );
    ballot_vk_record.status = ConfidentialStatus::Active;
    ballot_vk_record.gas_schedule_id = Some("sched_bad_ballot".to_owned());
    ballot_vk_record.key = Some(ballot_vk_box);
    verifying_keys::RegisterVerifyingKey {
        id: ballot_vk_id.clone(),
        record: ballot_vk_record,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("register bad ballot vk");
    let tally_vk_id = VerifyingKeyId::new(backend, "vote_tally");
    let tally_vk_box = sample_stark_vk_box(backend, tally_circuit_id);
    let mut tally_vk_record = VerifyingKeyRecord::new(
        1,
        tally_circuit_id,
        BackendTag::Stark,
        "goldilocks",
        tally_schema_hash,
        iroha_core::zk::hash_vk(&tally_vk_box),
    );
    tally_vk_record.status = ConfidentialStatus::Active;
    tally_vk_record.gas_schedule_id = Some("sched_tally".to_owned());
    tally_vk_record.key = Some(tally_vk_box);
    verifying_keys::RegisterVerifyingKey {
        id: tally_vk_id.clone(),
        record: tally_vk_record,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("register tally vk");
    let err = CreateElection {
        election_id: "stark-vote-role-check".to_owned(),
        options: 2,
        eligible_root: [0x22; 32],
        start_ts: 0,
        end_ts: 0,
        vk_ballot: ballot_vk_id,
        vk_tally: tally_vk_id,
        domain_tag: "gov:ballot:v1".to_owned(),
    }
    .execute(&ALICE_ID, &mut stx)
    .expect_err("create election must reject wrong STARK ballot role");
    let err_text = format!("{err:?}");
    assert!(
        err_text.contains("ballot verifying key circuit mismatch"),
        "unexpected error: {err:?}"
    );
}
#[test]
fn create_election_rejects_generic_stark_ballot_before_tally_resolution() {
    use core::num::NonZeroU64;
    use iroha_core::{
        kura::Kura, query::store::LiveQueryStore, smartcontracts::Execute, state::State,
    };
    use iroha_data_model::{
        Registrable,
        account::Account,
        block::BlockHeader,
        confidential::ConfidentialStatus,
        domain::Domain,
        isi::{Grant, verifying_keys, zk::CreateElection},
        permission::Permission,
        proof::{VerifyingKeyId, VerifyingKeyRecord},
        zk::BackendTag,
    };
    use iroha_executor_data_model::permission::governance::CanManageParliament;
    use iroha_primitives::json::Json;
    use iroha_test_samples::ALICE_ID;
    let backend = "stark/fri/poseidon-x7-goldilocks-6x64-v1";
    let ballot_circuit_id = "stark/fri/poseidon-x7-goldilocks-6x64-v1:vote-ballot";
    let bad_tally_circuit_id = "stark/fri/poseidon-x7-goldilocks-6x64-v1:not-a-tally-circuit";
    let ballot_schema_hash: [u8; 32] = iroha_crypto::Hash::new(b"gov:vote:ballot:schema:v1").into();
    let tally_schema_hash: [u8; 32] = iroha_crypto::Hash::new(b"gov:vote:tally:schema:v1").into();
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let domain_id: iroha_data_model::domain::DomainId =
        iroha_data_model::domain::DomainId::try_new("wonderland", "universal").expect("domain");
    let domain: Domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
    let account: Account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let mut state = State::new_for_testing(
        iroha_core::state::World::with([domain], [account], Vec::new()),
        kura,
        query,
    );
    state.zk.stark.enabled = true;
    state.zk.halo2.enabled = false;
    state.zk.verify_timeout = std::time::Duration::ZERO;
    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let perm_vk = Permission::new("CanManageVerifyingKeys".to_string(), Json::new(()));
    Grant::account_permission(perm_vk, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant CanManageVerifyingKeys");
    let perm_parliament: Permission = CanManageParliament.into();
    Grant::account_permission(perm_parliament, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant CanManageParliament");
    let ballot_vk_id = VerifyingKeyId::new(backend, "vote_ballot");
    let ballot_vk_box = sample_stark_vk_box(backend, ballot_circuit_id);
    let mut ballot_vk_record = VerifyingKeyRecord::new(
        1,
        ballot_circuit_id,
        BackendTag::Stark,
        "goldilocks",
        ballot_schema_hash,
        iroha_core::zk::hash_vk(&ballot_vk_box),
    );
    ballot_vk_record.status = ConfidentialStatus::Active;
    ballot_vk_record.gas_schedule_id = Some("sched_ballot".to_owned());
    ballot_vk_record.key = Some(ballot_vk_box);
    verifying_keys::RegisterVerifyingKey {
        id: ballot_vk_id.clone(),
        record: ballot_vk_record,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("register ballot vk");
    let tally_vk_id = VerifyingKeyId::new(backend, "bad_vote_tally");
    let tally_vk_box = sample_stark_vk_box(backend, bad_tally_circuit_id);
    let mut tally_vk_record = VerifyingKeyRecord::new(
        1,
        bad_tally_circuit_id,
        BackendTag::Stark,
        "goldilocks",
        tally_schema_hash,
        iroha_core::zk::hash_vk(&tally_vk_box),
    );
    tally_vk_record.status = ConfidentialStatus::Active;
    tally_vk_record.gas_schedule_id = Some("sched_bad_tally".to_owned());
    tally_vk_record.key = Some(tally_vk_box);
    verifying_keys::RegisterVerifyingKey {
        id: tally_vk_id.clone(),
        record: tally_vk_record,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("register bad tally vk");
    let err = CreateElection {
        election_id: "stark-vote-role-check-tally".to_owned(),
        options: 2,
        eligible_root: [0x22; 32],
        start_ts: 0,
        end_ts: 0,
        vk_ballot: ballot_vk_id,
        vk_tally: tally_vk_id,
        domain_tag: "gov:ballot:v1".to_owned(),
    }
    .execute(&ALICE_ID, &mut stx)
    .expect_err("create election must reject the generic STARK ballot role");
    let err_text = format!("{err:?}");
    assert!(
        err_text.contains("ballot verifying key circuit mismatch"),
        "unexpected error: {err:?}"
    );
}
#[test]
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn governance_accepts_halo2_and_rejects_synthetic_stark_ballot() {
    use core::num::NonZeroU64;
    use iroha_core::{
        kura::Kura,
        query::store::LiveQueryStore,
        smartcontracts::Execute,
        state::{State, World, WorldReadOnly},
        zk::test_utils::halo2_fixture_envelope,
    };
    use iroha_data_model::{
        Registrable,
        account::Account,
        block::BlockHeader,
        confidential::ConfidentialStatus,
        domain::Domain,
        isi::{
            Grant, verifying_keys,
            zk::{CreateElection, SubmitBallot},
        },
        permission::Permission,
        proof::{ProofAttachment, ProofBox, VerifyingKeyId, VerifyingKeyRecord},
        zk::BackendTag,
    };
    use iroha_executor_data_model::permission::governance::{
        CanManageParliament, CanSubmitGovernanceBallot,
    };
    use iroha_primitives::json::Json;
    use iroha_test_samples::ALICE_ID;
    use mv::storage::StorageReadOnly;
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let domain_id: iroha_data_model::domain::DomainId =
        iroha_data_model::domain::DomainId::try_new("wonderland", "universal").expect("domain");
    let domain: Domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
    let account: Account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let world = World::with([domain], [account], Vec::new());
    let mut state = State::new_for_testing(world, kura, query);
    state.zk.stark.enabled = true;
    state.zk.halo2.enabled = true;
    state.zk.verify_timeout = std::time::Duration::ZERO;
    state.gov.citizenship_bond_amount = 0_u64.into();
    state.gov.min_bond_amount = 0_u64.into();
    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let perm_vk = Permission::new("CanManageVerifyingKeys".to_string(), Json::new(()));
    Grant::account_permission(perm_vk, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant CanManageVerifyingKeys");
    let perm_parliament: Permission = CanManageParliament.into();
    Grant::account_permission(perm_parliament, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant CanManageParliament");
    let halo2_election_id = "mixed-backend-halo2".to_string();
    let stark_election_id = "mixed-backend-stark".to_string();
    let perm_halo2_ballot: Permission = CanSubmitGovernanceBallot {
        referendum_id: halo2_election_id.clone(),
    }
    .into();
    Grant::account_permission(perm_halo2_ballot, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant halo2 ballot permission");
    let perm_stark_ballot: Permission = CanSubmitGovernanceBallot {
        referendum_id: stark_election_id.clone(),
    }
    .into();
    Grant::account_permission(perm_stark_ballot, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant stark ballot permission");
    // Register a Halo2 VK/circuit pair and submit a valid Halo2 ballot.
    let halo2_backend = "halo2/ipa";
    let halo2_circuit_id = "halo2/ipa:tiny-add2inst-public";
    let halo2_vk_id = VerifyingKeyId::new(halo2_backend, "mixed_halo2_ballot");
    let halo2_fixture = halo2_fixture_envelope(halo2_circuit_id, [0u8; 32]);
    let halo2_vk_box = halo2_fixture
        .vk_box(halo2_backend)
        .expect("halo2 fixture must include vk bytes");
    let halo2_vk_hash = halo2_fixture
        .vk_hash(halo2_backend)
        .expect("halo2 fixture must include vk hash");
    let mut halo2_vk_record = VerifyingKeyRecord::new(
        1,
        halo2_circuit_id,
        BackendTag::Halo2IpaPasta,
        "pallas",
        halo2_fixture.schema_hash,
        halo2_vk_hash,
    );
    halo2_vk_record.status = ConfidentialStatus::Active;
    halo2_vk_record.gas_schedule_id = Some("sched_halo2_ballot".to_string());
    halo2_vk_record.key = Some(halo2_vk_box);
    verifying_keys::RegisterVerifyingKey {
        id: halo2_vk_id.clone(),
        record: halo2_vk_record,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("register halo2 ballot vk");
    let mut halo2_commit = [0u8; 32];
    halo2_commit.copy_from_slice(&halo2_fixture.public_inputs[..32]);
    let mut halo2_root = [0u8; 32];
    halo2_root.copy_from_slice(&halo2_fixture.public_inputs[32..64]);
    CreateElection {
        election_id: halo2_election_id.clone(),
        options: 2,
        eligible_root: halo2_root,
        start_ts: 0,
        end_ts: 0,
        vk_ballot: halo2_vk_id.clone(),
        vk_tally: halo2_vk_id.clone(),
        domain_tag: "gov:ballot:v1".to_string(),
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("create halo2 election");
    let halo2_ballot_attachment = ProofAttachment::new_ref(
        halo2_backend.to_string(),
        ProofBox::new(halo2_backend.to_string(), halo2_fixture.proof_bytes.clone()),
        halo2_vk_id.clone(),
    );
    let halo2_nullifier = derive_ballot_nullifier_for_test(
        "gov:ballot:v1",
        state.network_id_ref(),
        &halo2_election_id,
        &halo2_commit,
    );
    SubmitBallot {
        election_id: halo2_election_id.clone(),
        ciphertext: halo2_commit.to_vec(),
        ballot_proof: halo2_ballot_attachment,
        nullifier: halo2_nullifier,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("submit halo2 ballot");
    // Register a STARK VK/circuit pair and reject a synthetic STARK ballot.
    let stark_backend = "stark/fri/poseidon-x7-goldilocks-6x64-v1";
    let stark_ballot_circuit_id = "stark/fri/poseidon-x7-goldilocks-6x64-v1:vote-ballot";
    let stark_tally_circuit_id = "stark/fri/poseidon-x7-goldilocks-6x64-v1:vote-tally";
    let stark_ballot_vk_id = VerifyingKeyId::new(stark_backend, "mixed_stark_ballot");
    let stark_ballot_vk_box = sample_stark_vk_box(stark_backend, stark_ballot_circuit_id);
    let stark_ballot_vk_hash = iroha_core::zk::hash_vk(&stark_ballot_vk_box);
    let stark_ballot_schema = b"gov:vote:ballot:schema:v1".to_vec();
    let stark_ballot_schema_hash: [u8; 32] = iroha_crypto::Hash::new(&stark_ballot_schema).into();
    let mut stark_ballot_vk_record = VerifyingKeyRecord::new(
        1,
        stark_ballot_circuit_id,
        BackendTag::Stark,
        "goldilocks",
        stark_ballot_schema_hash,
        stark_ballot_vk_hash,
    );
    stark_ballot_vk_record.status = ConfidentialStatus::Active;
    stark_ballot_vk_record.gas_schedule_id = Some("sched_stark_ballot".to_string());
    stark_ballot_vk_record.key = Some(stark_ballot_vk_box.clone());
    verifying_keys::RegisterVerifyingKey {
        id: stark_ballot_vk_id.clone(),
        record: stark_ballot_vk_record,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("register stark ballot vk");
    let stark_tally_vk_id = VerifyingKeyId::new(stark_backend, "mixed_stark_tally");
    let stark_tally_vk_box = sample_stark_vk_box(stark_backend, stark_tally_circuit_id);
    let stark_tally_vk_hash = iroha_core::zk::hash_vk(&stark_tally_vk_box);
    let stark_tally_schema = b"gov:vote:tally:schema:v1".to_vec();
    let stark_tally_schema_hash: [u8; 32] = iroha_crypto::Hash::new(&stark_tally_schema).into();
    let mut stark_tally_vk_record = VerifyingKeyRecord::new(
        1,
        stark_tally_circuit_id,
        BackendTag::Stark,
        "goldilocks",
        stark_tally_schema_hash,
        stark_tally_vk_hash,
    );
    stark_tally_vk_record.status = ConfidentialStatus::Active;
    stark_tally_vk_record.gas_schedule_id = Some("sched_stark_tally".to_string());
    stark_tally_vk_record.key = Some(stark_tally_vk_box);
    verifying_keys::RegisterVerifyingKey {
        id: stark_tally_vk_id.clone(),
        record: stark_tally_vk_record,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("register stark tally vk");
    let stark_commit = [0x11; 32];
    let stark_root = [0x22; 32];
    CreateElection {
        election_id: stark_election_id.clone(),
        options: 2,
        eligible_root: stark_root,
        start_ts: 0,
        end_ts: 0,
        vk_ballot: stark_ballot_vk_id.clone(),
        vk_tally: stark_tally_vk_id,
        domain_tag: "gov:ballot:v1".to_string(),
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("create stark election");
    let stark_ballot_proof_bytes = build_stark_open_verify_envelope_bytes_for_columns(
        stark_backend,
        stark_ballot_circuit_id,
        stark_ballot_vk_hash,
        &stark_ballot_schema,
        vec![vec![stark_commit], vec![stark_root]],
    );
    let stark_ballot_attachment = ProofAttachment::new_ref(
        stark_backend.to_string(),
        ProofBox::new(stark_backend.to_string(), stark_ballot_proof_bytes),
        stark_ballot_vk_id,
    );
    let stark_nullifier = derive_ballot_nullifier_for_test(
        "gov:ballot:v1",
        state.network_id_ref(),
        &stark_election_id,
        &stark_commit,
    );
    let err = SubmitBallot {
        election_id: stark_election_id.clone(),
        ciphertext: stark_commit.to_vec(),
        ballot_proof: stark_ballot_attachment,
        nullifier: stark_nullifier,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect_err("synthetic STARK ballot must be rejected");
    let err_text = format!("{err:?}");
    assert!(
        err_text.contains("invalid ballot proof"),
        "unexpected stark ballot rejection: {err:?}"
    );
    let halo2_election = stx
        .world
        .elections()
        .get(&halo2_election_id)
        .cloned()
        .expect("halo2 election exists");
    assert_eq!(
        halo2_election.ciphertexts.len(),
        1,
        "halo2 ballot must be accepted"
    );
    let stark_election = stx
        .world
        .elections()
        .get(&stark_election_id)
        .cloned()
        .expect("stark election exists");
    assert_eq!(
        stark_election.ciphertexts.len(),
        0,
        "synthetic stark ballot must be rejected"
    );
}
#[test]
fn stark_envelope_respects_limits() {
    let env = build_sample_air_composition_envelope();
    let bytes = norito::to_bytes(&env).expect("encode");
    assert!(
        verify_stark_fri_envelope(&bytes),
        "default limits should accept the sample envelope"
    );
    let default_limits = StarkVerifierLimits::default();
    // Apply a stricter domain-tag limit to force rejection.
    let mut tight_limits = default_limits;
    tight_limits.max_domain_tag_len = 4;
    let mut env_bad_tag = env.clone();
    env_bad_tag.params.domain_tag = "TOO-LONG-TAG".into();
    let bytes_bad_tag = norito::to_bytes(&env_bad_tag).expect("encode");
    assert!(
        !verify_stark_fri_envelope_with_limits(&bytes_bad_tag, &tight_limits),
        "envelope with oversized domain tag must fail under stricter limits"
    );
    // Apply envelope byte budget lower than payload size to confirm size guard triggers.
    tight_limits.max_envelope_bytes = bytes.len().saturating_sub(1);
    assert!(
        !verify_stark_fri_envelope_with_limits(&bytes, &tight_limits),
        "envelope larger than allowed byte budget must fail"
    );
    let mut relaxed_limits = default_limits;
    relaxed_limits.max_domain_tag_len = default_limits.max_domain_tag_len + 1;
    relaxed_limits.max_transcript_label_len = default_limits.max_transcript_label_len + 1;
    relaxed_limits.max_envelope_bytes = default_limits.max_envelope_bytes + 1;
    let oversized_envelope_bytes = vec![0_u8; default_limits.max_envelope_bytes + 1];
    assert!(
        !verify_stark_fri_envelope_with_limits(&oversized_envelope_bytes, &relaxed_limits),
        "raised public limits must not relax the native encoded-envelope byte cap"
    );
    let over_canonical_domain_tag = "d".repeat(default_limits.max_domain_tag_len + 1);
    let err = prove_stark_fri_composition_envelope_bytes(
        sample_air_params(over_canonical_domain_tag.clone()),
        "TEST-STARK".to_string(),
        7,
        2,
        sample_composition_terms(),
    )
    .expect_err("public STARK prover must reject over-canonical domain tags");
    assert!(
        err.contains("domain tag"),
        "domain-tag rejection should be explicit, got: {err}"
    );
    let over_canonical_transcript_label = "T".repeat(default_limits.max_transcript_label_len + 1);
    let err = prove_stark_fri_composition_envelope_bytes(
        sample_air_params("fastpq:v1:fri".to_string()),
        over_canonical_transcript_label.clone(),
        7,
        2,
        sample_composition_terms(),
    )
    .expect_err("public STARK prover must reject over-canonical transcript labels");
    assert!(
        err.contains("transcript label"),
        "transcript-label rejection should be explicit, got: {err}"
    );
    let mut env_over_canonical_tag = env.clone();
    env_over_canonical_tag.params.domain_tag = over_canonical_domain_tag;
    let bytes_over_canonical_tag =
        norito::to_bytes(&env_over_canonical_tag).expect("encode over-canonical domain tag");
    assert!(
        !verify_stark_fri_envelope_with_limits(&bytes_over_canonical_tag, &relaxed_limits),
        "raised public limits must not relax the canonical domain-tag cap"
    );
    let mut env_over_canonical_label = env;
    env_over_canonical_label.transcript_label = over_canonical_transcript_label;
    let bytes_over_canonical_label = norito::to_bytes(&env_over_canonical_label)
        .expect("encode over-canonical transcript label");
    assert!(
        !verify_stark_fri_envelope_with_limits(&bytes_over_canonical_label, &relaxed_limits),
        "raised public limits must not relax the canonical transcript-label cap"
    );
}
#[test]
fn stark_six_lane_wire_is_deterministic_and_uses_48_byte_roots() {
    let envelope = build_sample_air_composition_envelope();
    let first = norito::to_bytes(&envelope).expect("encode canonical six-lane envelope");
    let second = norito::to_bytes(&envelope).expect("re-encode canonical six-lane envelope");
    assert_eq!(
        first, second,
        "native STARK V1 encoding must be deterministic"
    );
    assert!(
        envelope
            .proof
            .commits
            .roots
            .iter()
            .all(|root| root.to_le_bytes().len() == 48),
        "every native STARK commitment root must use the canonical 48-byte digest"
    );
}
