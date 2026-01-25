//! Ensure structured Data events emitted as a result of VM syscalls (OUTPUT → ISIs)
//! preserve syscall order within a single transaction overlay.
#![allow(clippy::cast_possible_truncation, clippy::items_after_statements)]

use std::{str::FromStr, sync::Arc};

use iroha_core::{
    kura::Kura, query::store::LiveQueryStore, smartcontracts::ivm::host::CoreHost, state::State,
};
use iroha_crypto::Hash;
use iroha_data_model::{block::BlockHeader, prelude::*};
use ivm::{IVM, Memory, PointerType, encoding, syscalls};
use nonzero_ext::nonzero;

fn make_tlv(type_id: PointerType, payload: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(7 + payload.len() + 32);
    v.extend_from_slice(&(type_id as u16).to_be_bytes());
    v.push(1);
    v.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    v.extend_from_slice(payload);
    let h: [u8; 32] = Hash::new(payload).into();
    v.extend_from_slice(&h);
    v
}

fn store_tlv(vm: &mut IVM, cursor: &mut u64, type_id: PointerType, payload: &[u8]) -> u64 {
    let tlv = make_tlv(type_id, payload);
    vm.memory
        .preload_input(*cursor, &tlv)
        .expect("preload tlv into INPUT");
    let ptr = Memory::INPUT_START + *cursor;
    let len = tlv.len() as u64;
    // maintain 8-byte alignment between TLVs
    let aligned = (len + 7) & !7;
    *cursor += aligned;
    ptr
}

fn encode_prog_syscall(num: u32) -> Vec<u8> {
    let scall = ivm::instruction::wide::system::SCALL;
    let mut code = Vec::new();
    code.extend_from_slice(&encoding::wide::encode_sys(scall, num as u8).to_le_bytes());
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

    let mut prog = Vec::new();
    prog.extend_from_slice(b"IVM\0");
    prog.push(2);
    prog.push(0);
    prog.push(0);
    prog.push(0);
    prog.extend_from_slice(&0u64.to_le_bytes());
    prog.push(1);
    prog.extend_from_slice(&code);
    prog
}

#[test]
#[allow(clippy::too_many_lines)]
fn ivm_syscall_data_events_follow_order() {
    // World with domain/account and asset definition for mint
    let (authority_id, _) = iroha_test_samples::gen_account_in("wonderland");
    let domain_id: DomainId = "wonderland".parse().expect("domain id");
    let domain = Domain::new(domain_id.clone()).build(&authority_id);
    let account = Account::new(authority_id.clone()).build(&authority_id);
    let asset_def = AssetDefinition::new(
        AssetDefinitionId::from_str("rose#wonderland").expect("asset def"),
        NumericSpec::default(),
    )
    .build(&authority_id);
    let world = iroha_core::state::World::with_assets([domain], [account], [asset_def], [], []);
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let state = State::new(
        world,
        kura,
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let state = State::new(world, kura, query);

    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    // VM + host for syscall execution
    let mut vm = IVM::new(1_000_000);
    let host = CoreHost::with_accounts(authority_id.clone(), Arc::new(vec![authority_id.clone()]));
    vm.set_host(host);

    // Prepare TLVs for account id, metadata keys/values, and asset definition id.
    let mut cursor = 0u64;
    let acct_bytes = norito::to_bytes(&authority_id).expect("encode account id");
    let ptr_account = store_tlv(&mut vm, &mut cursor, PointerType::AccountId, &acct_bytes);

    let key1 = Name::from_str("account_key").expect("name");
    let key1_bytes = norito::to_bytes(&key1).expect("encode key");
    let ptr_key1 = store_tlv(&mut vm, &mut cursor, PointerType::Name, &key1_bytes);
    let val1 = Json::new(norito::json!({"kind": "alpha"}));
    let val1_bytes = norito::to_bytes(&val1).expect("encode json");
    let ptr_val1 = store_tlv(&mut vm, &mut cursor, PointerType::Json, &val1_bytes);

    let key2 = Name::from_str("account_second_key").expect("name");
    let key2_bytes = norito::to_bytes(&key2).expect("encode key");
    let ptr_key2 = store_tlv(&mut vm, &mut cursor, PointerType::Name, &key2_bytes);
    let val2 = Json::new(norito::json!({"kind": "beta"}));
    let val2_bytes = norito::to_bytes(&val2).expect("encode json");
    let ptr_val2 = store_tlv(&mut vm, &mut cursor, PointerType::Json, &val2_bytes);

    let asset_def_id: AssetDefinitionId = "rose#wonderland".parse().expect("asset def id");
    let asset_bytes = norito::to_bytes(&asset_def_id).expect("encode asset definition");
    let ptr_asset_def = store_tlv(
        &mut vm,
        &mut cursor,
        PointerType::AssetDefinitionId,
        &asset_bytes,
    );
    let amount_payload = norito::to_bytes(&Numeric::from(5_u64)).expect("encode amount");
    let ptr_amount = store_tlv(
        &mut vm,
        &mut cursor,
        PointerType::NoritoBytes,
        &amount_payload,
    );

    let prog_set_detail = encode_prog_syscall(syscalls::SYSCALL_SET_ACCOUNT_DETAIL);
    let prog_mint = encode_prog_syscall(syscalls::SYSCALL_MINT_ASSET);

    // 1) Account metadata insert
    vm.set_register(10, ptr_account);
    vm.set_register(11, ptr_key1);
    vm.set_register(12, ptr_val1);
    vm.load_program(&prog_set_detail)
        .expect("load detail program");
    vm.run().expect("set account detail #1");

    // 2) Second metadata insert
    vm.set_register(10, ptr_account);
    vm.set_register(11, ptr_key2);
    vm.set_register(12, ptr_val2);
    vm.load_program(&prog_set_detail)
        .expect("reload detail program");
    vm.run().expect("set account detail #2");

    // 3) Mint asset into the authority account
    vm.set_register(10, ptr_account);
    vm.set_register(11, ptr_asset_def);
    vm.set_register(12, ptr_amount);
    vm.load_program(&prog_mint).expect("load mint program");
    vm.run().expect("mint asset");

    // Apply queued ISIs to the state transaction
    CoreHost::with_host(&mut vm, |host| {
        host.apply_queued(&mut stx, &authority_id)
            .expect("apply queued syscalls");
    });

    // Collect Data events emitted during execution
    let events = stx.world.take_external_events();
    let data_events: Vec<_> = events
        .into_iter()
        .filter_map(|ev| match ev {
            iroha_data_model::events::EventBox::Data(d) => Some(d),
            _ => None,
        })
        .collect();

    assert!(
        data_events.len() >= 3,
        "expected multiple data events from syscalls"
    );

    let debug_events: Vec<String> = data_events
        .iter()
        .map(|ev| format!("{:?}", ev.as_ref()))
        .collect();
    let idx_key1 = debug_events
        .iter()
        .position(|ev| ev.contains("account_key"))
        .expect("account_key metadata event not found");
    let idx_key2 = debug_events
        .iter()
        .position(|ev| ev.contains("account_second_key"))
        .expect("account_second_key metadata event not found");
    let idx_asset_created = debug_events
        .iter()
        .position(|ev| ev.contains("AssetEvent::Created") || ev.contains("Asset(Created"))
        .expect("asset creation event not found");

    assert!(
        idx_key1 < idx_key2 && idx_key2 < idx_asset_created,
        "expected metadata events followed by asset creation, got {debug_events:?}"
    );
}
