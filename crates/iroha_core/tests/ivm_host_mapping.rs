//! Host mapping parity tests: ensure SCALLs bridge to native ISIs with identical effects.
#![allow(
    clippy::cast_possible_truncation,
    clippy::redundant_closure_for_method_calls,
    clippy::too_many_lines,
    clippy::map_unwrap_or
)]

#[cfg(feature = "telemetry")]
use iroha_core::telemetry::StateTelemetry;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::ivm::host::CoreHost,
    state::{State, World, WorldReadOnly},
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{account::NewAccount, metadata::Metadata, nft::NftId, prelude::*};
use ivm::{IVM, PointerType, encoding, instruction, syscalls as ivm_sys};
use mv::storage::StorageReadOnly;
use norito::NoritoSerialize;

fn with_core_host<R>(vm: &mut IVM, f: impl FnOnce(&mut CoreHost) -> R) -> R {
    CoreHost::with_host(vm, f)
}

fn tlv_blob<T: NoritoSerialize>(val: &T, type_id: u16) -> Vec<u8> {
    let payload = norito::to_bytes(val).expect("encode payload");
    tlv_from_payload(&payload, type_id)
}

fn norito_bytes_tlv<T: norito::core::NoritoSerialize>(val: &T) -> Vec<u8> {
    let payload = norito::to_bytes(val).expect("encode NoritoBytes payload");
    tlv_from_payload(&payload, PointerType::NoritoBytes as u16)
}

fn tlv_from_payload(payload: &[u8], type_id: u16) -> Vec<u8> {
    let mut blob = Vec::with_capacity(2 + 1 + 4 + payload.len() + 32);
    blob.extend_from_slice(&type_id.to_be_bytes());
    blob.push(1u8); // version
    blob.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_be_bytes());
    blob.extend_from_slice(payload);
    let hash: [u8; 32] = iroha_crypto::Hash::new(payload).into();
    blob.extend_from_slice(&hash);
    blob
}

fn seeded_account(seed: u8) -> AccountId {
    seeded_account_in(seed, "wonder")
}

fn seeded_account_in(seed: u8, domain_name: &str) -> AccountId {
    let keypair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
    let domain: DomainId = domain_name.parse().unwrap();
    AccountId::new(domain, keypair.public_key().clone())
}

fn make_header() -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(b"IVM\0");
    v.extend_from_slice(&[1, 0, 0, 4]);
    v.extend_from_slice(&0u64.to_le_bytes());
    v.push(1);
    v
}

fn scall_program(syscall: u32) -> Vec<u8> {
    let mut code = Vec::new();
    code.extend_from_slice(
        &encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            u8::try_from(syscall).expect("syscall id fits in u8"),
        )
        .to_le_bytes(),
    );
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let mut program = make_header();
    program.extend_from_slice(&code);
    program
}

fn load_input_blob(vm: &mut IVM, cursor: &mut u64, blob: &[u8]) -> u64 {
    vm.memory
        .input_write_aligned(cursor, blob, 8)
        .expect("write INPUT blob")
}

fn run_syscall(vm: &mut IVM, syscall: u32, regs: &[(u8, u64)]) {
    let program = scall_program(syscall);
    vm.load_program(&program).expect("load program");
    for &(reg, value) in regs {
        vm.set_register(usize::from(reg), value);
    }
    vm.run()
        .unwrap_or_else(|err| panic!("run syscall 0x{syscall:02X}: {err:?}"));
}

#[test]
fn host_bridges_nft_mint_and_transfer() {
    // Accounts and NFT id
    let owner = seeded_account(1);
    let recipient = seeded_account(2);
    let nft_id: NftId = "n0$wonder".parse().unwrap();

    let nft_blob = tlv_blob(&nft_id, PointerType::NftId as u16);
    let owner_blob = tlv_blob(&owner, PointerType::AccountId as u16);
    let mut vm = IVM::new(50_000);
    vm.set_host(CoreHost::new(owner.clone()));
    let mut cursor = 0;
    let ptr_nft = load_input_blob(&mut vm, &mut cursor, &nft_blob);
    let ptr_owner = load_input_blob(&mut vm, &mut cursor, &owner_blob);
    run_syscall(
        &mut vm,
        ivm_sys::SYSCALL_NFT_MINT_ASSET,
        &[(10, ptr_nft), (11, ptr_owner)],
    );

    // Minimal world setup: domain + accounts
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let state = State::new(World::new(), kura, query_handle, StateTelemetry::default());
    #[cfg(not(feature = "telemetry"))]
    let state = State::new(World::new(), kura, query_handle);
    let header = iroha_data_model::block::BlockHeader::new(
        core::num::NonZeroU64::new(1).unwrap(),
        None,
        None,
        None,
        0,
        0,
    );
    {
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let domain_id: DomainId = "wonder".parse().unwrap();
        let new_domain = Domain::new(domain_id.clone());
        let reg_domain = RegisterBox::from(Register::domain(new_domain));
        let reg_owner = RegisterBox::from(Register::account(NewAccount::new(owner.clone())));
        let reg_recipient =
            RegisterBox::from(Register::account(NewAccount::new(recipient.clone())));
        let executor = tx.world.executor().clone();
        for instr in [
            InstructionBox::from(reg_domain),
            InstructionBox::from(reg_owner),
            InstructionBox::from(reg_recipient),
        ] {
            executor
                .execute_instruction(&mut tx, &owner, instr)
                .unwrap();
        }

        // Apply queued NFT_MINT_ASSET
        let queued =
            CoreHost::with_host(&mut vm, |host| host.apply_queued(&mut tx, &owner)).unwrap();
        assert_eq!(queued.len(), 1);
        tx.apply();
        block.commit().unwrap();
    }

    // NFT exists and is owned by owner
    {
        let view = state.view();
        let entry = view.world.nfts().get(&nft_id).expect("nft exists");
        assert_eq!(entry.owned_by, owner);
    }

    // Now transfer NFT to recipient via SCALL NFT_TRANSFER_ASSET
    let nft_blob2 = tlv_blob(&nft_id, PointerType::NftId as u16);
    let to_blob = tlv_blob(&recipient, PointerType::AccountId as u16);
    let mut vm2 = IVM::new(50_000);
    vm2.set_host(CoreHost::new(owner.clone()));
    let mut cursor2 = 0;
    let ptr_nft2 = load_input_blob(&mut vm2, &mut cursor2, &nft_blob2);
    let ptr_to = load_input_blob(&mut vm2, &mut cursor2, &to_blob);
    run_syscall(
        &mut vm2,
        ivm_sys::SYSCALL_NFT_TRANSFER_ASSET,
        &[(10, 0), (11, ptr_nft2), (12, ptr_to)],
    );

    // Apply queued NFT_TRANSFER_ASSET in a new block scope
    let header_transfer = iroha_data_model::block::BlockHeader::new(
        core::num::NonZeroU64::new(2).unwrap(),
        None,
        None,
        None,
        0,
        0,
    );
    {
        let mut block = state.block(header_transfer);
        let mut tx = block.transaction();
        let queued2 =
            CoreHost::with_host(&mut vm2, |host| host.apply_queued(&mut tx, &owner)).unwrap();
        assert_eq!(queued2.len(), 1);
        tx.apply();
        block.commit().unwrap();
    }
    // Owner changed to recipient
    let view = state.view();
    let entry2 = view.world.nfts().get(&nft_id).expect("nft exists");
    assert_eq!(entry2.owned_by, recipient);
}

#[test]
fn host_rejects_insufficient_asset_transfer() {
    // Setup accounts and asset def
    let from = seeded_account(3);
    let to = seeded_account(4);
    let asset_def: AssetDefinitionId = "coin#wonder".parse().unwrap();

    // Build program: TRANSFER_ASSET(&from, &to, &asset_def, 1000) -> expect rejection when applying queued ISIs
    let from_tlv = tlv_blob(&from, PointerType::AccountId as u16);
    let to_tlv = tlv_blob(&to, PointerType::AccountId as u16);
    let asset_tlv = tlv_blob(&asset_def, PointerType::AssetDefinitionId as u16);
    let amount_tlv = norito_bytes_tlv(&Numeric::from(1000_u64));

    let mut vm = IVM::new(50_000);
    vm.set_host(CoreHost::new(from.clone()));
    let mut cursor = 0;
    let ptr_from = load_input_blob(&mut vm, &mut cursor, &from_tlv);
    let ptr_to = load_input_blob(&mut vm, &mut cursor, &to_tlv);
    let ptr_asset = load_input_blob(&mut vm, &mut cursor, &asset_tlv);
    let ptr_amount = load_input_blob(&mut vm, &mut cursor, &amount_tlv);
    run_syscall(
        &mut vm,
        ivm_sys::SYSCALL_TRANSFER_ASSET,
        &[
            (10, ptr_from),
            (11, ptr_to),
            (12, ptr_asset),
            (13, ptr_amount),
        ],
    );

    // Setup world: domain, accounts, asset def, mint only 100
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let state = State::new(World::new(), kura, query_handle, StateTelemetry::default());
    #[cfg(not(feature = "telemetry"))]
    let state = State::new(World::new(), kura, query_handle);
    let header = iroha_data_model::block::BlockHeader::new(
        core::num::NonZeroU64::new(1).unwrap(),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut tx = block.transaction();
    let domain_id: DomainId = "wonder".parse().unwrap();
    let new_domain = Domain::new(domain_id.clone());
    let reg_domain = RegisterBox::from(Register::domain(new_domain));
    let reg_from = RegisterBox::from(Register::account(NewAccount::new(from.clone())));
    let reg_to = RegisterBox::from(Register::account(NewAccount::new(to.clone())));
    let new_asset_def = AssetDefinition::numeric(asset_def.clone());
    let reg_asset_def = RegisterBox::from(Register::asset_definition(new_asset_def));
    let mint = MintBox::from(Mint::asset_numeric(
        100u64,
        AssetId::of(asset_def.clone(), from.clone()),
    ));
    let executor = tx.world.executor().clone();
    for instr in [
        InstructionBox::from(reg_domain),
        InstructionBox::from(reg_from),
        InstructionBox::from(reg_to),
        InstructionBox::from(reg_asset_def),
        InstructionBox::from(mint),
    ] {
        executor.execute_instruction(&mut tx, &from, instr).unwrap();
    }

    // Apply queued transfer: should be rejected due to insufficient funds
    let result = with_core_host(&mut vm, |host| host.apply_queued(&mut tx, &from));
    result.expect_err("should reject");
    // We don't assert exact error kind to avoid tight coupling, just that it rejects.
}

#[test]
fn host_batches_transfer_v1_calls() {
    let from = seeded_account(5);
    let to_a = seeded_account(6);
    let to_b = seeded_account(7);
    let domain_id: DomainId = "wonder".parse().unwrap();
    let asset_def_id: AssetDefinitionId = "rose#wonder".parse().unwrap();
    let domain = Domain::new(domain_id.clone()).build(&from);
    let from_account = Account::new(from.clone()).build(&from);
    let first_recipient_account = Account::new(to_a.clone()).build(&from);
    let second_recipient_account = Account::new(to_b.clone()).build(&from);
    let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&from);
    let from_asset = Asset::new(
        AssetId::new(asset_def_id.clone(), from.clone()),
        Numeric::from(25_u32),
    );
    let world = World::with_assets(
        [domain],
        [
            from_account,
            first_recipient_account,
            second_recipient_account,
        ],
        [asset_def],
        [from_asset],
        [],
    );
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query);
    let header = iroha_data_model::block::BlockHeader::new(
        core::num::NonZeroU64::new(1).unwrap(),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut tx = block.transaction();
    tx.tx_call_hash = Some(Hash::prehashed([0x55; Hash::LENGTH]));

    let from_tlv = tlv_blob(&from, PointerType::AccountId as u16);
    let first_recipient_tlv = tlv_blob(&to_a, PointerType::AccountId as u16);
    let second_recipient_tlv = tlv_blob(&to_b, PointerType::AccountId as u16);
    let asset_tlv = tlv_blob(&asset_def_id, PointerType::AssetDefinitionId as u16);
    let amount_a_tlv = norito_bytes_tlv(&Numeric::from(7_u64));
    let amount_b_tlv = norito_bytes_tlv(&Numeric::from(4_u64));
    let mut vm = IVM::new(50_000);
    vm.set_host(CoreHost::new(from.clone()));
    let mut cursor = 0;
    let ptr_from = load_input_blob(&mut vm, &mut cursor, &from_tlv);
    let ptr_to_a = load_input_blob(&mut vm, &mut cursor, &first_recipient_tlv);
    let ptr_to_b = load_input_blob(&mut vm, &mut cursor, &second_recipient_tlv);
    let ptr_asset = load_input_blob(&mut vm, &mut cursor, &asset_tlv);
    let ptr_amount_a = load_input_blob(&mut vm, &mut cursor, &amount_a_tlv);
    let ptr_amount_b = load_input_blob(&mut vm, &mut cursor, &amount_b_tlv);

    run_syscall(&mut vm, ivm_sys::SYSCALL_TRANSFER_V1_BATCH_BEGIN, &[]);
    run_syscall(
        &mut vm,
        ivm_sys::SYSCALL_TRANSFER_ASSET,
        &[
            (10, ptr_from),
            (11, ptr_to_a),
            (12, ptr_asset),
            (13, ptr_amount_a),
        ],
    );
    run_syscall(
        &mut vm,
        ivm_sys::SYSCALL_TRANSFER_ASSET,
        &[
            (10, ptr_from),
            (11, ptr_to_b),
            (12, ptr_asset),
            (13, ptr_amount_b),
        ],
    );
    run_syscall(&mut vm, ivm_sys::SYSCALL_TRANSFER_V1_BATCH_END, &[]);

    let queued = with_core_host(&mut vm, |host| host.apply_queued(&mut tx, &from)).unwrap();
    assert_eq!(queued.len(), 1, "single batch instruction enqueued");
    let batch = queued[0]
        .as_any()
        .downcast_ref::<TransferAssetBatch>()
        .expect("queued instruction is a TransferAssetBatch");
    assert_eq!(batch.entries().len(), 2, "two entries batched");
    tx.apply();

    let from_asset_id = AssetId::new(asset_def_id.clone(), from.clone());
    let from_balance = block.world.asset(&from_asset_id).expect("authority asset");
    assert_eq!(**from_balance, Numeric::from(14_u32));
    let first_recipient_asset_id = AssetId::new(asset_def_id.clone(), to_a.clone());
    let first_recipient_balance = block
        .world
        .asset(&first_recipient_asset_id)
        .expect("recipient a asset");
    assert_eq!(**first_recipient_balance, Numeric::from(7_u32));
    let second_recipient_asset_id = AssetId::new(asset_def_id.clone(), to_b.clone());
    let second_recipient_balance = block
        .world
        .asset(&second_recipient_asset_id)
        .expect("recipient b asset");
    assert_eq!(**second_recipient_balance, Numeric::from(4_u32));

    let transcripts = block.drain_transfer_transcripts();
    assert_eq!(transcripts.len(), 1);
    let (_, batches) = transcripts.into_iter().next().unwrap();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].deltas.len(), 2);
    assert!(batches[0].poseidon_preimage_digest.is_none());
}

#[test]
fn host_rejects_nft_transfer_from_non_owner() {
    // Setup accounts and NFT owned by bob; alice attempts transfer -> reject
    let alice = seeded_account(8);
    let bob = seeded_account(9);
    let charlie = seeded_account(10);
    let nft_id: NftId = "n0$wonder".parse().unwrap();

    // Build program: TRANSFER_NFT(from=alice, nft_id, to=charlie)
    let from_tlv = tlv_blob(&alice, PointerType::AccountId as u16);
    let nft_tlv = tlv_blob(&nft_id, PointerType::NftId as u16);
    let to_tlv = tlv_blob(&charlie, PointerType::AccountId as u16);

    let mut vm = IVM::new(50_000);
    vm.set_host(CoreHost::new(alice.clone()));
    let mut cursor = 0;
    let ptr_from = load_input_blob(&mut vm, &mut cursor, &from_tlv);
    let ptr_nft = load_input_blob(&mut vm, &mut cursor, &nft_tlv);
    let ptr_to = load_input_blob(&mut vm, &mut cursor, &to_tlv);
    run_syscall(
        &mut vm,
        ivm_sys::SYSCALL_NFT_TRANSFER_ASSET,
        &[(10, ptr_from), (11, ptr_nft), (12, ptr_to)],
    );

    // World: domain, register alice, bob, charlie, and register NFT owned by bob
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let state = State::new(World::new(), kura, query_handle, StateTelemetry::default());
    #[cfg(not(feature = "telemetry"))]
    let state = State::new(World::new(), kura, query_handle);
    let header = iroha_data_model::block::BlockHeader::new(
        core::num::NonZeroU64::new(1).unwrap(),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut tx = block.transaction();
    let domain_id: DomainId = "wonder".parse().unwrap();
    let reg_domain = RegisterBox::from(Register::domain(Domain::new(domain_id)));
    let reg_alice = RegisterBox::from(Register::account(NewAccount::new(alice.clone())));
    let reg_bob = RegisterBox::from(Register::account(NewAccount::new(bob.clone())));
    let reg_charlie = RegisterBox::from(Register::account(NewAccount::new(charlie.clone())));
    // Prepare NewNft to be registered by bob as the owner (authority = bob)
    let new_nft = Nft::new(nft_id.clone(), Metadata::default());
    let reg_nft = RegisterBox::from(Register::nft(new_nft));
    let executor = tx.world.executor().clone();
    for instr in [
        InstructionBox::from(reg_domain),
        InstructionBox::from(reg_alice),
        InstructionBox::from(reg_bob),
        InstructionBox::from(reg_charlie),
    ] {
        executor
            .execute_instruction(&mut tx, &alice, instr)
            .unwrap();
    }
    // Register NFT with bob as the authority so the owner is bob
    executor
        .execute_instruction(&mut tx, &bob, InstructionBox::from(reg_nft))
        .unwrap();

    // Apply queued transfer: should be rejected since alice is not the owner
    let err = with_core_host(&mut vm, |host| host.apply_queued(&mut tx, &alice))
        .expect_err("should reject");
    drop(err);
}

#[test]
fn host_bridges_set_account_detail() {
    // Build program: set_account_detail(authority(), name("cursor"), json("1")); HALT
    let key: Name = "cursor".parse().unwrap();
    let val: iroha_primitives::json::Json = "1".parse().unwrap();
    let authority = seeded_account(11);
    let key_tlv = tlv_blob(&key, PointerType::Name as u16);
    let val_tlv = tlv_blob(&val, PointerType::Json as u16);
    let authority_tlv = tlv_blob(&authority, PointerType::AccountId as u16);

    let mut vm = IVM::new(100_000);
    vm.set_host(CoreHost::new(authority.clone()));
    let mut cursor = 0;
    let ptr_account = load_input_blob(&mut vm, &mut cursor, &authority_tlv);
    let ptr_key = load_input_blob(&mut vm, &mut cursor, &key_tlv);
    let ptr_val = load_input_blob(&mut vm, &mut cursor, &val_tlv);
    run_syscall(
        &mut vm,
        ivm_sys::SYSCALL_SET_ACCOUNT_DETAIL,
        &[(10, ptr_account), (11, ptr_key), (12, ptr_val)],
    );

    // Minimal world setup: register domain and account
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let state = State::new(World::new(), kura, query_handle, StateTelemetry::default());
    #[cfg(not(feature = "telemetry"))]
    let state = State::new(World::new(), kura, query_handle);
    let header = iroha_data_model::block::BlockHeader::new(
        core::num::NonZeroU64::new(1).unwrap(),
        None,
        None,
        None,
        0,
        0,
    );
    {
        let mut block = state.block(header);
        let mut tx = block.transaction();

        let domain_id: DomainId = "wonder".parse().unwrap();
        let new_domain = Domain::new(domain_id.clone());
        let reg_domain = RegisterBox::from(Register::domain(new_domain));
        let reg_acc = RegisterBox::from(Register::account(NewAccount::new(authority.clone())));
        let executor = tx.world.executor().clone();
        for instr in [
            InstructionBox::from(reg_domain),
            InstructionBox::from(reg_acc),
        ] {
            executor
                .execute_instruction(&mut tx, &authority, instr)
                .unwrap();
        }

        // Apply queued detail set via host
        let queued =
            with_core_host(&mut vm, |host| host.apply_queued(&mut tx, &authority)).unwrap();
        assert_eq!(queued.len(), 1);
        tx.apply();
        block.commit().unwrap();
    } // drop block + tx before taking a read-only view

    // Check metadata present
    let view = state.view();
    let acc = view.world.accounts().get(&authority).unwrap();
    assert_eq!(acc.metadata().get(&key).map(|v| v.as_ref()), Some("1"));
}

#[test]
fn host_bridges_mint_asset() {
    // Build program: mint_asset(authority(), asset_definition("coin#wonder"), 123); HALT
    let authority = seeded_account(12);
    let asset_def: AssetDefinitionId = "coin#wonder".parse().unwrap();
    let authority_tlv = tlv_blob(&authority, PointerType::AccountId as u16);
    let asset_tlv = tlv_blob(&asset_def, PointerType::AssetDefinitionId as u16);
    let amount_tlv = norito_bytes_tlv(&Numeric::from(123_u64));

    let mut vm = IVM::new(100_000);
    vm.set_host(CoreHost::new(authority.clone()));
    let mut cursor = 0;
    let ptr_authority = load_input_blob(&mut vm, &mut cursor, &authority_tlv);
    let ptr_asset = load_input_blob(&mut vm, &mut cursor, &asset_tlv);
    let ptr_amount = load_input_blob(&mut vm, &mut cursor, &amount_tlv);
    run_syscall(
        &mut vm,
        ivm_sys::SYSCALL_MINT_ASSET,
        &[(10, ptr_authority), (11, ptr_asset), (12, ptr_amount)],
    );

    // Minimal world setup: domain, account, asset def
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let state = State::new(World::new(), kura, query_handle, StateTelemetry::default());
    #[cfg(not(feature = "telemetry"))]
    let state = State::new(World::new(), kura, query_handle);
    let header = iroha_data_model::block::BlockHeader::new(
        core::num::NonZeroU64::new(1).unwrap(),
        None,
        None,
        None,
        0,
        0,
    );
    {
        let mut block = state.block(header);
        let mut tx = block.transaction();

        let domain_id: DomainId = "wonder".parse().unwrap();
        let new_domain = Domain::new(domain_id.clone());
        let reg_domain = RegisterBox::from(Register::domain(new_domain));
        let reg_acc = RegisterBox::from(Register::account(NewAccount::new(authority.clone())));
        let new_asset_def = AssetDefinition::numeric(asset_def.clone());
        let reg_asset_def = RegisterBox::from(Register::asset_definition(new_asset_def));
        let executor = tx.world.executor().clone();
        for instr in [
            InstructionBox::from(reg_domain),
            InstructionBox::from(reg_acc),
            InstructionBox::from(reg_asset_def),
        ] {
            executor
                .execute_instruction(&mut tx, &authority, instr)
                .unwrap();
        }

        // Apply queued mint via host
        let queued =
            with_core_host(&mut vm, |host| host.apply_queued(&mut tx, &authority)).unwrap();
        assert_eq!(queued.len(), 1);
        tx.apply();
        block.commit().unwrap();
    }

    let balance = state
        .view()
        .world
        .assets()
        .get(&AssetId::of(asset_def.clone(), authority.clone()))
        .map_or_else(|| Numeric::from(0u32), |v| v.clone().into_inner());
    assert_eq!(balance, 123u32.into());
}

#[test]
fn host_bridges_nft_set_metadata_and_burn() {
    // Setup an owner and NFT id
    let owner = seeded_account(13);
    let nft_id: NftId = "n0$wonder".parse().unwrap();
    // Build program: NFT_MINT_ASSET, NFT_SET_METADATA(nft_id, "flag", true), NFT_BURN_ASSET(nft_id)
    let nft_tlv = tlv_blob(&nft_id, PointerType::NftId as u16);
    let key: Name = "flag".parse().unwrap();
    let key_tlv = tlv_blob(&key, PointerType::Name as u16);
    let val: iroha_primitives::json::Json = true.into();
    let val_tlv = tlv_blob(&val, PointerType::Json as u16);
    let owner_tlv = tlv_blob(&owner, PointerType::AccountId as u16);

    // VM
    let mut vm = IVM::new(100_000);
    vm.set_host(CoreHost::new(owner.clone()));
    let mut cursor = 0;
    let ptr_nft = load_input_blob(&mut vm, &mut cursor, &nft_tlv);
    let ptr_owner = load_input_blob(&mut vm, &mut cursor, &owner_tlv);
    let ptr_key = load_input_blob(&mut vm, &mut cursor, &key_tlv);
    let ptr_val = load_input_blob(&mut vm, &mut cursor, &val_tlv);
    run_syscall(
        &mut vm,
        ivm_sys::SYSCALL_NFT_MINT_ASSET,
        &[(10, ptr_nft), (11, ptr_owner)],
    );
    run_syscall(
        &mut vm,
        ivm_sys::SYSCALL_NFT_SET_METADATA,
        &[(10, ptr_nft), (11, ptr_key), (12, ptr_val)],
    );
    run_syscall(&mut vm, ivm_sys::SYSCALL_NFT_BURN_ASSET, &[(10, ptr_nft)]);

    // Minimal world: domain + owner
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let state = State::new(World::new(), kura, query_handle, StateTelemetry::default());
    #[cfg(not(feature = "telemetry"))]
    let state = State::new(World::new(), kura, query_handle);
    let header = iroha_data_model::block::BlockHeader::new(
        core::num::NonZeroU64::new(1).unwrap(),
        None,
        None,
        None,
        0,
        0,
    );
    {
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let domain_id: DomainId = "wonder".parse().unwrap();
        let reg_domain = RegisterBox::from(Register::domain(Domain::new(domain_id.clone())));
        let reg_owner = RegisterBox::from(Register::account(NewAccount::new(owner.clone())));
        let executor = tx.world.executor().clone();
        for instr in [
            InstructionBox::from(reg_domain),
            InstructionBox::from(reg_owner),
        ] {
            executor
                .execute_instruction(&mut tx, &owner, instr)
                .unwrap();
        }

        // Apply queued NFT_MINT_ASSET, NFT_SET_METADATA, NFT_BURN_ASSET
        let queued = with_core_host(&mut vm, |host| host.apply_queued(&mut tx, &owner)).unwrap();
        assert_eq!(queued.len(), 3);
        tx.apply();
        block.commit().unwrap();
    }

    // NFT should not exist after burn; before burn, metadata should have been set correctly
    let view = state.view();
    assert!(view.world.nfts().get(&nft_id).is_none());
}

#[test]
fn transfer_batch_apply_syscall_enqueues_batch() {
    let from = seeded_account(14);
    let to_a = seeded_account(15);
    let to_b = seeded_account(16);
    let asset_def_id: AssetDefinitionId = "rose#wonder".parse().unwrap();

    let batch = TransferAssetBatch::new(vec![
        TransferAssetBatchEntry::new(
            from.clone(),
            to_a.clone(),
            asset_def_id.clone(),
            Numeric::from(7_u32),
        ),
        TransferAssetBatchEntry::new(
            from.clone(),
            to_b.clone(),
            asset_def_id.clone(),
            Numeric::from(4_u32),
        ),
    ]);
    let encoded_batch = norito::to_bytes(&batch).expect("encode batch");
    let decoded: TransferAssetBatch =
        norito::decode_from_bytes(&encoded_batch).expect("batch roundtrip");
    assert_eq!(decoded.entries().len(), 2, "encode/decode sanity check");
    let batch_tlv = norito_bytes_tlv(&batch);

    let mut vm = IVM::new(50_000);
    vm.set_host(CoreHost::new(from.clone()));
    let mut cursor = 0;
    let ptr_batch = load_input_blob(&mut vm, &mut cursor, &batch_tlv);
    run_syscall(
        &mut vm,
        ivm_sys::SYSCALL_TRANSFER_V1_BATCH_APPLY,
        &[(10, ptr_batch)],
    );

    let queued = with_core_host(&mut vm, |host| host.drain_instructions());
    assert_eq!(queued.len(), 1, "transfer batch apply enqueues instruction");
    let batch_instr = queued[0]
        .as_any()
        .downcast_ref::<TransferAssetBatch>()
        .expect("queued instruction is a TransferAssetBatch");
    assert_eq!(batch_instr.entries().len(), 2, "two entries preserved");
    assert_eq!(batch_instr.entries()[0].to(), &to_a);
    assert_eq!(batch_instr.entries()[1].to(), &to_b);
}
