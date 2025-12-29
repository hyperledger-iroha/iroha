//! `CoreHost` test for `ZK_VOTE_GET_TALLY`: ensure it returns finalized and tally from snapshot.
#![cfg(feature = "zk-tests")]

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::ivm::host::CoreHost,
    state::{State, World, WorldReadOnly},
};
use iroha_data_model::prelude::*;
use ivm::{IVMHost, Memory, PointerType, syscalls, zk_verify};
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

fn make_tlv(type_id: u16, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(7 + payload.len() + 32);
    out.extend_from_slice(&type_id.to_be_bytes());
    out.push(1);
    let payload_len = u32::try_from(payload.len()).expect("payload length fits u32");
    out.extend_from_slice(&payload_len.to_be_bytes());
    out.extend_from_slice(payload);
    let h: [u8; 32] = iroha_crypto::Hash::new(payload).into();
    out.extend_from_slice(&h);
    out
}

#[test]
fn zk_vote_get_tally_roundtrip_from_snapshot() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!("Skipping: zk vote tally from snapshot gated. Set IROHA_RUN_IGNORED=1 to run.");
        return;
    }
    // Build minimal state
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let state = State::new(
        World::new(),
        kura,
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let state = State::new(World::new(), kura, query);

    // Begin block and transaction
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    // Create a simple election via ISIs and finalize it
    let election_id = "e1".to_string();
    let create = iroha_data_model::isi::zk::CreateElection {
        election_id: election_id.clone(),
        options: 3,
        eligible_root: [0u8; 32],
        start_ts: 0,
        end_ts: 10,
        vk_ballot: iroha_data_model::proof::VerifyingKeyId::new("halo2/ipa", "vk_ballot"),
        vk_tally: iroha_data_model::proof::VerifyingKeyId::new("halo2/ipa", "vk_tally"),
        domain_tag: "ballot-domain".to_string(),
    };
    let owner: AccountId = "alice@zkd".parse().unwrap();
    stx.world
        .executor()
        .clone()
        .execute_instruction(&mut stx, &owner, InstructionBox::from(create))
        .expect("create election");
    let finalize_fixture =
        iroha_core::zk::test_utils::halo2_fixture_envelope("halo2/ipa:tiny-add-v1", [0u8; 32]);
    let finalize_vk = finalize_fixture
        .vk_box("halo2/ipa")
        .expect("fixture verifying key");
    let finalize = iroha_data_model::isi::zk::FinalizeElection {
        election_id: election_id.clone(),
        tally: vec![5, 3, 1],
        tally_proof: iroha_data_model::proof::ProofAttachment::new_inline(
            "halo2/ipa".into(),
            finalize_fixture.proof_box("halo2/ipa"),
            finalize_vk,
        ),
    };
    stx.world
        .executor()
        .clone()
        .execute_instruction(&mut stx, &owner, InstructionBox::from(finalize))
        .expect("finalize election");
    stx.apply();

    // Snapshot elections into CoreHost and query via syscall
    let mut vm = ivm::IVM::new(1_000_000);
    let mut host = CoreHost::new(owner.clone());
    {
        use std::collections::BTreeMap;
        let mut esnap: BTreeMap<String, (bool, Vec<u64>)> = BTreeMap::new();
        let view = state.view();
        let e = view.world.elections().get(&election_id).unwrap();
        esnap.insert(election_id.clone(), (e.finalized, e.tally.clone()));
        host.set_zk_elections_snapshot(esnap);
    }
    let mut host = host;

    // Build request TLV and call syscall
    let req = zk_verify::VoteGetTallyRequest { election_id };
    let payload = norito::to_bytes(&req).expect("encode req");
    let tlv = make_tlv(PointerType::NoritoBytes as u16, &payload);
    vm.memory.preload_input(0, &tlv).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    host.syscall(syscalls::SYSCALL_ZK_VOTE_GET_TALLY, &mut vm)
        .expect("syscall ok");
    let ptr = vm.register(10);
    let tlv_out = vm.memory.validate_tlv(ptr).expect("valid tlv");
    assert_eq!(tlv_out.type_id, PointerType::NoritoBytes);
    let resp: zk_verify::VoteGetTallyResponse =
        norito::decode_from_bytes(tlv_out.payload).expect("decode resp");
    assert!(resp.finalized);
    assert_eq!(resp.tally, vec![5, 3, 1]);
}
