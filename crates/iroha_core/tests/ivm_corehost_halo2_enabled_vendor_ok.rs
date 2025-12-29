#![doc = "Positive gating path: with Halo2 enabled and within `max_k`, a prior\n`ZK_VOTE_VERIFY_BALLOT` sets the `CoreHost` latch and allows enqueued\n`SubmitBallot` (via vendor bridge) to apply."]
#![cfg(feature = "zk-ipa-native")]
#![allow(clippy::cast_possible_truncation, clippy::too_many_lines)]

use iroha_config::parameters::defaults;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::{Execute, ivm::host::CoreHost},
    state::State,
};
use iroha_crypto::Hash;
use iroha_data_model::isi::verifying_keys;
use iroha_data_model::{
    prelude::*,
    proof::{VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord},
    zk::BackendTag,
};
use iroha_executor_data_model::permission::governance::{
    CanManageParliament, CanSubmitGovernanceBallot,
};
use iroha_primitives::json::Json;
use iroha_test_samples::ALICE_ID;
use ivm::{IVM, PointerType, ProgramMetadata, encoding, instruction, syscalls as ivm_sys};
use nonzero_ext::nonzero;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc};

fn make_tlv(type_id: u16, payload: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(7 + payload.len() + 32);
    v.extend_from_slice(&type_id.to_be_bytes());
    v.push(1);
    v.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    v.extend_from_slice(payload);
    let h: [u8; 32] = iroha_crypto::Hash::new(payload).into();
    v.extend_from_slice(&h);
    v
}

fn store_tlv(vm: &mut IVM, cursor: &mut u64, tlv: &[u8]) -> u64 {
    vm.memory
        .input_write_aligned(cursor, tlv, 8)
        .expect("write TLV into INPUT")
}

#[allow(clippy::too_many_arguments)]
fn compute_domain_tag(
    chain_id_bytes: &[u8],
    backend_label: &str,
    curve_label: &str,
    vk_commitment: [u8; 32],
    schema_hash: [u8; 32],
    manifest: &str,
    namespace: &str,
    syscall_label: &str,
) -> [u8; 32] {
    let mut buf = Vec::new();
    buf.extend_from_slice(b"iroha.zk.domain/v1");
    buf.extend_from_slice(chain_id_bytes);
    buf.extend_from_slice(backend_label.as_bytes());
    buf.extend_from_slice(curve_label.as_bytes());
    buf.extend_from_slice(&vk_commitment);
    buf.extend_from_slice(&schema_hash);
    buf.extend_from_slice(syscall_label.as_bytes());
    buf.extend_from_slice(manifest.as_bytes());
    buf.extend_from_slice(namespace.as_bytes());
    *Hash::new(&buf).as_ref()
}

fn build_open_verify_envelope_bytes(
    k: u32,
    vk_commitment: [u8; 32],
    schema_hash: [u8; 32],
    domain_tag: [u8; 32],
) -> Vec<u8> {
    use h2::norito_helpers as nh;
    use iroha_zkp_halo2::{self as h2, backend::pallas::PallasBackend};
    let params = h2::Params::new(k.try_into().unwrap()).expect("params");
    // Build a trivial polynomial and opening proof; the coefficient vector must match params.n().
    let coeffs: Vec<h2::PrimeField64> = vec![0u64.into(); params.n()];
    let poly = h2::Polynomial::from_coeffs(coeffs);
    let mut tr = h2::Transcript::new(ivm::host::LABEL_VOTE_BALLOT);
    let p_g = poly.commit(&params).expect("commit");
    let z = h2::PrimeField64::from(1u64);
    let (proof, t) = poly.open(&params, &mut tr, z, p_g).expect("open");
    let env = h2::OpenVerifyEnvelope {
        params: nh::params_to_wire(&params),
        public: nh::poly_open_public::<PallasBackend>(params.n(), z, t, p_g),
        proof: nh::proof_to_wire(&proof),
        transcript_label: ivm::host::LABEL_VOTE_BALLOT.to_string(),
        vk_commitment: Some(vk_commitment),
        public_inputs_schema_hash: Some(schema_hash),
        domain_tag: Some(domain_tag),
    };
    norito::to_bytes(&env).expect("encode env")
}

#[test]
fn verify_then_vendor_submit_ballot_applies() {
    // Minimal node state
    let authority: AccountId = ALICE_ID.clone();
    let domain = Domain::new(authority.domain().clone()).build(&authority);
    let account = Account::new(authority.clone()).build(&authority);
    let world = iroha_core::state::World::with([domain], [account], Vec::<AssetDefinition>::new());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let mut state = State::new(
        world,
        kura,
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let mut state = State::new(world, kura, query);
    let mut gov_cfg = state.gov.clone();
    gov_cfg.citizenship_bond_amount = 0;
    state.set_gov(gov_cfg);
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    // Authority and VM/host configured for Halo2 IPA (ToyP61), max_k >= 8
    let mut vm = IVM::new(1_000_000);
    let mut host = CoreHost::with_accounts(authority.clone(), Arc::new(vec![authority.clone()]));
    let chain_id_bytes = state.chain_id.to_string().into_bytes();
    host.set_chain_id_bytes(chain_id_bytes.clone());
    let backend_label = "halo2/ipa";
    let vk_bytes = vec![0x01, 0x02, 0x03];
    let mut hasher = Sha256::new();
    hasher.update(backend_label.as_bytes());
    hasher.update(&vk_bytes);
    let vk_commitment: [u8; 32] = hasher.finalize().into();
    let schema_hash: [u8; 32] = Hash::new(b"ballot-schema").into();
    let domain_tag = compute_domain_tag(
        &chain_id_bytes,
        backend_label,
        "pallas",
        vk_commitment,
        schema_hash,
        "core",
        "ballot",
        "zk_verify_ballot/v1",
    );
    let mut vk_record = VerifyingKeyRecord::new_with_owner(
        1,
        "zk_vote_ballot",
        None,
        "ballot",
        BackendTag::Halo2IpaPasta,
        "pallas",
        schema_hash,
        vk_commitment,
    );
    vk_record.status = ConfidentialStatus::Active;
    vk_record.key = Some(VerifyingKeyBox::new(backend_label.into(), vk_bytes.clone()));
    vk_record.vk_len = u32::try_from(vk_bytes.len()).expect("vk length fits in u32");
    vk_record.max_proof_bytes = u32::MAX;
    vk_record.gas_schedule_id = Some("halo2_default".into());
    let vk_record_for_state = vk_record.clone();
    let mut vk_map = BTreeMap::new();
    vk_map.insert(VerifyingKeyId::new(backend_label, "vk_ballot"), vk_record);
    host.set_verifying_keys(vk_map).expect("set registry");
    host.set_halo2_config(&iroha_config::parameters::actual::Halo2 {
        enabled: true,
        curve: iroha_config::parameters::actual::ZkCurve::Pallas,
        backend: iroha_config::parameters::actual::Halo2Backend::Ipa,
        max_k: 18,
        verifier_budget_ms: 200,
        verifier_max_batch: 8,
        max_envelope_bytes: defaults::zk::halo2::MAX_ENVELOPE_BYTES,
        max_proof_bytes: usize::MAX,
        max_transcript_label_len: defaults::zk::halo2::MAX_TRANSCRIPT_LABEL_LEN,
        enforce_transcript_label_ascii: defaults::zk::halo2::ENFORCE_TRANSCRIPT_LABEL_ASCII,
    });
    vm.set_host(host);

    // Grant authority permissions and register verifying keys in the WSV for governance plumbing.
    let perm_vk = Permission::new("CanManageVerifyingKeys".to_string(), Json::new(()));
    let perm_parliament: Permission = CanManageParliament.into();
    let perm_submit: Permission = CanSubmitGovernanceBallot {
        referendum_id: "e1".to_string(),
    }
    .into();
    Grant::account_permission(perm_vk, authority.clone())
        .execute(&authority, &mut stx)
        .expect("grant vk permission");
    Grant::account_permission(perm_parliament, authority.clone())
        .execute(&authority, &mut stx)
        .expect("grant parliament permission");
    Grant::account_permission(perm_submit, authority.clone())
        .execute(&authority, &mut stx)
        .expect("grant submit ballot permission");
    let vk_ballot = VerifyingKeyId::new("halo2/ipa", "vk_ballot");
    let vk_tally = vk_ballot.clone();
    verifying_keys::RegisterVerifyingKey {
        id: vk_ballot.clone(),
        record: vk_record_for_state.clone(),
    }
    .execute(&authority, &mut stx)
    .expect("register ballot vk");

    // 1) Verify ballot: prepare envelope TLV in INPUT and run SCALL
    let env_bytes = build_open_verify_envelope_bytes(8, vk_commitment, schema_hash, domain_tag);
    let tlv = make_tlv(PointerType::NoritoBytes as u16, &env_bytes);
    let mut cursor = 0;
    let ptr = store_tlv(&mut vm, &mut cursor, &tlv);
    vm.set_register(10, ptr);
    let mut code = Vec::new();
    code.extend_from_slice(
        &encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            ivm_sys::SYSCALL_ZK_VOTE_VERIFY_BALLOT as u8,
        )
        .to_le_bytes(),
    );
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let mut prog = ProgramMetadata {
        vector_length: 4,
        ..ProgramMetadata::default()
    }
    .encode();
    prog.extend_from_slice(&code);
    vm.load_program(&prog).expect("load verify");
    vm.run().expect("run verify");
    let verify_res = vm.register(10);
    let verify_err = vm.register(11);
    assert_ne!(
        verify_res, 0,
        "verify must succeed under enabled config (err code {verify_err})"
    );

    // Seed an election so SubmitBallot can record ciphertexts
    let create = iroha_data_model::isi::zk::CreateElection {
        election_id: "e1".to_string(),
        options: 2,
        eligible_root: [0u8; 32],
        start_ts: 0,
        end_ts: 0,
        vk_ballot,
        vk_tally,
        domain_tag: "zkvote".to_string(),
    };
    create
        .execute(&authority, &mut stx)
        .expect("create election in WSV");

    // 2) Enqueue SubmitBallot via the vendor bridge
    let mut code2 = Vec::new();
    code2.extend_from_slice(
        &encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION as u8,
        )
        .to_le_bytes(),
    );
    code2.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let mut prog2 = ProgramMetadata {
        vector_length: 4,
        ..ProgramMetadata::default()
    }
    .encode();
    prog2.extend_from_slice(&code2);
    let sb = iroha_data_model::isi::zk::SubmitBallot {
        election_id: "e1".to_string(),
        ciphertext: vec![0u8; 8],
        ballot_proof: iroha_data_model::proof::ProofAttachment::new_inline(
            "halo2/ipa".into(),
            iroha_data_model::proof::ProofBox::new("halo2/ipa".into(), vec![0xAA]),
            iroha_data_model::proof::VerifyingKeyBox::new("halo2/ipa".into(), vk_bytes.clone()),
        ),
        nullifier: [7u8; 32],
    };
    let sb_box: iroha_data_model::isi::InstructionBox = sb.into();
    let sb_bytes = norito::to_bytes(&sb_box).expect("encode SubmitBallot InstructionBox to Norito");
    let tlv2 = make_tlv(PointerType::NoritoBytes as u16, &sb_bytes);
    let ptr2 = store_tlv(&mut vm, &mut cursor, &tlv2);
    vm.set_register(10, ptr2);
    vm.load_program(&prog2).expect("load vendor2");
    vm.run().expect("run vendor2");

    // 3) Apply queued instructions; should succeed thanks to verify latch
    CoreHost::with_host(&mut vm, |host| host.apply_queued(&mut stx, &authority))
        .expect("apply queued after verify");
}
