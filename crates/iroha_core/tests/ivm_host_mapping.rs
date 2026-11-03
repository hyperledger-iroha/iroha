//! Host mapping parity tests: ensure SCALLs bridge to native ISIs with identical effects.
//!
//! Host effects reach State only under the exact signed root of the transaction that ran the
//! program. Programs whose effects are applied therefore run as signed generic IVM
//! transactions through the node executor over an authenticated signed genesis; their pointer
//! arguments are the program's canonical literal TLVs. The same program run on a detached
//! `CoreHost` exposes the effects its syscalls queue.

#![allow(
    clippy::cast_possible_truncation,
    clippy::redundant_closure_for_method_calls,
    clippy::too_many_lines,
    clippy::map_unwrap_or
)]
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::ivm::{cache::IvmCache, host::CoreHost},
    state::{State, StateBlock, World, WorldReadOnly},
    tx::AcceptedTransaction,
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    account::NewAccount,
    isi::error::{InstructionExecutionError, MathError},
    nft::NftId,
    prelude::*,
};
use iroha_model_base::domain::DomainId;
use iroha_model_base::metadata::Metadata;
use iroha_model_base::name::Name;
use ivm::{IVM, PointerType, ProgramMetadata, encoding, instruction, syscalls as ivm_sys};
use mv::storage::StorageReadOnly;
use norito::NoritoSerialize;
const AMPLE_TEST_GAS_LIMIT: u64 = 1_000_000;
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
fn quantity_tlv(value: &Quantity) -> Vec<u8> {
    ivm_abi::numeric_tlv::encode_quantity(value).expect("encode quantity pointer envelope")
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
    let keypair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .expect("seeded IVM host mapping account keypair should be valid");
    let _domain = DomainId::try_new(domain_name, "universal").unwrap();
    AccountId::new(keypair.public_key().clone())
}
/// A seeded account together with the key that signs its transactions.
fn seeded_signer(seed: u8) -> (AccountId, KeyPair) {
    let keypair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .expect("seeded IVM host mapping signer keypair should be valid");
    (AccountId::new(keypair.public_key().clone()), keypair)
}
fn new_account_in_domain(account_id: &AccountId) -> NewAccount {
    NewAccount::new(account_id.clone())
}
fn built_account_in_domain(account_id: &AccountId) -> Account {
    new_account_in_domain(account_id).build(account_id)
}
fn make_header() -> Vec<u8> {
    ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: 4,
        max_cycles: 1_000_000,
        abi_version: 1,
    }
    .encode()
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
/// Generic program whose pointer arguments are its authenticated literal-table TLVs.
fn literal_program(literals: &[Vec<u8>], code: &[u32]) -> Vec<u8> {
    let mut program = make_header();
    let data_offset = 16 + literals.len() * 8;
    let data_len: usize = literals.iter().map(Vec::len).sum();
    let pad = (4 - (data_offset + data_len) % 4) % 4;
    program.extend_from_slice(b"LTLB");
    for field in [literals.len(), pad, data_len] {
        program.extend_from_slice(
            &u32::try_from(field)
                .expect("literal table field fits u32")
                .to_le_bytes(),
        );
    }
    let mut offset = data_offset;
    for literal in literals {
        let descriptor = ivm::encode_literal_descriptor(
            ivm::LiteralKindV1::PointerTlv,
            u64::try_from(offset).expect("literal offset fits u64"),
        )
        .expect("literal offset fits its descriptor");
        program.extend_from_slice(&descriptor.to_le_bytes());
        offset += literal.len();
    }
    for literal in literals {
        program.extend_from_slice(literal);
    }
    program.extend(std::iter::repeat_n(0, pad));
    for word in code.iter().copied().chain([encoding::wide::encode_halt()]) {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}
/// Load each `(register, literal)` pointer argument, then invoke `syscall`.
fn literal_call(syscall: u32, arguments: &[(u8, u16)]) -> Vec<u32> {
    arguments
        .iter()
        .map(|&(register, literal)| {
            encoding::wide::encode_literal(instruction::wide::memory::LDLIT, register, literal)
        })
        .chain([encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            u8::try_from(syscall).expect("syscall id fits in u8"),
        )])
        .collect()
}
/// Effects the program's syscalls queue on a detached `CoreHost`.
fn queued_effects(program: &[u8], authority: &AccountId) -> Vec<InstructionBox> {
    let mut vm = IVM::new(AMPLE_TEST_GAS_LIMIT);
    vm.set_host(CoreHost::new(authority.clone()));
    vm.load_program(program).expect("load literal program");
    vm.run()
        .unwrap_or_else(|err| panic!("run literal program: {err:?}"));
    with_core_host(&mut vm, |host| {
        host.drain_instructions()
            .expect("ordinary instruction inspection retains no attachment owner")
    })
}
/// The ordinary block after the applied signed genesis (height one).
///
/// Committing a fixture block's World overlay publishes no block, so every fixture block
/// executes at the height the authenticated consensus schedule admits next.
fn ordinary_header(state: &State) -> iroha_data_model::block::BlockHeader {
    iroha_data_model::block::BlockHeader::new(
        core::num::NonZeroU64::new(2).expect("ordinary height"),
        state.view().latest_block_hash(),
        None,
        0,
        0,
    )
}
/// Statelessly admit `program` as a generic IVM transaction signed by `key`.
fn signed_program(state: &State, key: &KeyPair, program: Vec<u8>) -> AcceptedTransaction<'static> {
    let transaction = TransactionBuilder::new(
        *state.network_id_ref(),
        AccountId::new(key.public_key().clone()),
        FeePaymentIntent::authority(Vec::new(), core::num::NonZeroU64::new(AMPLE_TEST_GAS_LIMIT)),
    )
    .with_executable(Executable::Ivm(IvmBytecode::from_compiled(program)))
    .sign(key.private_key());
    let parameters = state.transaction_admission_parameters();
    AcceptedTransaction::accept(
        transaction,
        state.network_id_ref(),
        parameters.sumeragi().max_clock_drift(),
        parameters.transaction(),
        parameters
            .require_crypto_policy()
            .expect("signed genesis crypto policy"),
    )
    .expect("stateless transaction admission")
}
/// Execute an admitted transaction in `block`; its signed root owns the host effects.
fn execute(
    block: &mut StateBlock<'_>,
    transaction: AcceptedTransaction<'static>,
) -> Result<(), TransactionRejectionReason> {
    iroha_core::tx::execute_component_transaction_for_testing(
        block,
        transaction,
        &mut IvmCache::new(),
        None,
    )
    .map(drop)
}
#[test]
fn host_bridges_nft_mint_and_transfer() {
    // Accounts and NFT id
    let (owner, owner_key) = seeded_signer(1);
    let recipient = seeded_account(2);
    let nft_id: NftId = "n0$wonder".parse().unwrap();
    // Program: NFT_MINT_ASSET(&nft, &owner)
    let mint_program = literal_program(
        &[
            tlv_blob(&nft_id, PointerType::NftId as u16),
            tlv_blob(&owner, PointerType::AccountId as u16),
        ],
        &literal_call(ivm_sys::SYSCALL_NFT_MINT_ASSET, &[(10, 0), (11, 1)]),
    );
    assert_eq!(queued_effects(&mint_program, &owner).len(), 1);
    // Minimal world setup: the owner's domain + accounts
    let domain_id: DomainId = DomainId::try_new("wonder", "universal").unwrap();
    let world = World::with(
        [Domain::new(domain_id).build(&owner)],
        [
            built_account_in_domain(&owner),
            built_account_in_domain(&recipient),
        ],
        [],
    );
    let state = crate::native_genesis::certified_state(world);
    {
        let mint = signed_program(&state, &owner_key, mint_program);
        let mut block = state.block(ordinary_header(&state));
        execute(&mut block, mint).expect("apply the queued NFT mint under its signed root");
        block.commit_world_overlay_for_testing().unwrap();
    }
    // NFT exists and is owned by owner
    {
        let view = state.view();
        let entry = view.world.nfts().get(&nft_id).expect("nft exists");
        assert_eq!(entry.owned_by, owner);
    }
    // Now transfer NFT to recipient via SCALL NFT_TRANSFER_ASSET
    let transfer_program = literal_program(
        &[
            tlv_blob(&owner, PointerType::AccountId as u16),
            tlv_blob(&nft_id, PointerType::NftId as u16),
            tlv_blob(&recipient, PointerType::AccountId as u16),
        ],
        &literal_call(
            ivm_sys::SYSCALL_NFT_TRANSFER_ASSET,
            &[(10, 0), (11, 1), (12, 2)],
        ),
    );
    assert_eq!(queued_effects(&transfer_program, &owner).len(), 1);
    // Apply the queued NFT_TRANSFER_ASSET in a new block scope
    {
        let transfer = signed_program(&state, &owner_key, transfer_program);
        let mut block = state.block(ordinary_header(&state));
        execute(&mut block, transfer).expect("apply the queued NFT transfer under its signed root");
        block.commit_world_overlay_for_testing().unwrap();
    }
    // Owner changed to recipient
    let view = state.view();
    let entry2 = view.world.nfts().get(&nft_id).expect("nft exists");
    assert_eq!(entry2.owned_by, recipient);
}
#[test]
fn host_rejects_insufficient_asset_transfer() {
    // Setup accounts and asset def
    let (from, from_key) = seeded_signer(3);
    let to = seeded_account(4);
    let asset_def: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonder", "universal").unwrap(),
            "coin".parse().unwrap(),
        );
    // Build program: TRANSFER_ASSET_SCOPED(&from, &to, &asset_def, 1000, &dataspace) -> expect rejection when applying its queued ISI
    let program = literal_program(
        &[
            tlv_blob(&from, PointerType::AccountId as u16),
            tlv_blob(&to, PointerType::AccountId as u16),
            tlv_blob(&asset_def, PointerType::AssetDefinitionId as u16),
            quantity_tlv(&Quantity::from(1000_u64)),
            tlv_blob(
                &iroha_model_base::topology::DataSpaceId::UNIVERSAL,
                PointerType::DataSpaceId as u16,
            ),
        ],
        &literal_call(
            ivm_sys::SYSCALL_TRANSFER_ASSET_SCOPED,
            &[(10, 0), (11, 1), (12, 2), (13, 3), (14, 4)],
        ),
    );
    // Setup world: domain, accounts, asset def, only 100 held by the source
    let domain_id: DomainId = DomainId::try_new("wonder", "universal").unwrap();
    let new_asset_def = AssetDefinition::numeric(
        asset_def.clone(),
        "coin".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        Some(domain_id.clone()),
    )
    .build(&from);
    let world = World::with_assets(
        [Domain::new(domain_id).build(&from)],
        [built_account_in_domain(&from), built_account_in_domain(&to)],
        [new_asset_def],
        [Asset::new(
            AssetId::of(asset_def.clone(), from.clone()),
            Quantity::from(100_u64),
        )],
        [],
    );
    let state = crate::native_genesis::certified_state(world);
    // Apply the queued transfer: should be rejected due to insufficient funds
    let transfer = signed_program(&state, &from_key, program);
    let mut block = state.block(ordinary_header(&state));
    let err = execute(&mut block, transfer)
        .expect_err("transfer must reject spending more than the source balance");
    assert!(
        matches!(
            err,
            TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
                InstructionExecutionError::Math(MathError::NotEnoughQuantity)
            ))
        ),
        "expected insufficient balance rejection, got {err:?}"
    );
    assert_eq!(
        block
            .world
            .assets()
            .get(&AssetId::of(asset_def.clone(), from.clone()))
            .map(|value| value.as_ref().clone()),
        Some(Quantity::from(100_u64)),
        "rejected transfer must preserve the source balance"
    );
    assert!(
        block
            .world
            .assets()
            .get(&AssetId::of(asset_def, to))
            .is_none(),
        "rejected transfer must not create a destination balance"
    );
}
#[test]
fn host_batches_transfer_v1_calls() {
    let (from, from_key) = seeded_signer(5);
    let to_a = seeded_account(6);
    let to_b = seeded_account(7);
    let domain_id: DomainId = DomainId::try_new("wonder", "universal").unwrap();
    let asset_def_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonder", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
    let domain = Domain::new(domain_id.clone()).build(&from);
    let from_account = built_account_in_domain(&from);
    let first_recipient_account = built_account_in_domain(&to_a);
    let second_recipient_account = built_account_in_domain(&to_b);
    let asset_def = AssetDefinition::numeric(
        asset_def_id.clone(),
        "rose".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&from);
    let from_asset = Asset::new(
        AssetId::new(asset_def_id.clone(), from.clone()),
        Quantity::from(25_u32),
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
    let state = crate::native_genesis::certified_state(world);
    // Literals: from, first recipient, second recipient, asset, first amount, second amount.
    let literals = [
        tlv_blob(&from, PointerType::AccountId as u16),
        tlv_blob(&to_a, PointerType::AccountId as u16),
        tlv_blob(&to_b, PointerType::AccountId as u16),
        tlv_blob(&asset_def_id, PointerType::AssetDefinitionId as u16),
        quantity_tlv(&Quantity::from(7_u64)),
        quantity_tlv(&Quantity::from(4_u64)),
    ];
    let code = [
        literal_call(ivm_sys::SYSCALL_TRANSFER_V1_BATCH_BEGIN, &[]),
        literal_call(
            ivm_sys::SYSCALL_TRANSFER_V1,
            &[(10, 0), (11, 1), (12, 3), (13, 4)],
        ),
        literal_call(
            ivm_sys::SYSCALL_TRANSFER_V1,
            &[(10, 0), (11, 2), (12, 3), (13, 5)],
        ),
        literal_call(ivm_sys::SYSCALL_TRANSFER_V1_BATCH_END, &[]),
    ]
    .concat();
    let program = literal_program(&literals, &code);
    let queued = queued_effects(&program, &from);
    assert_eq!(queued.len(), 1, "single batch instruction enqueued");
    let batch = queued[0]
        .as_any()
        .downcast_ref::<TransferAssetBatch>()
        .expect("queued instruction is a TransferAssetBatch");
    assert_eq!(batch.entries().len(), 2, "two entries batched");
    let transaction = signed_program(&state, &from_key, program);
    let mut block = state.block(ordinary_header(&state));
    execute(&mut block, transaction).unwrap();
    let from_asset_id = AssetId::new(asset_def_id.clone(), from.clone());
    let from_balance = block.world.asset(&from_asset_id).expect("authority asset");
    assert_eq!(**from_balance, Quantity::from(14_u32));
    let first_recipient_asset_id = AssetId::new(asset_def_id.clone(), to_a.clone());
    let first_recipient_balance = block
        .world
        .asset(&first_recipient_asset_id)
        .expect("recipient a asset");
    assert_eq!(**first_recipient_balance, Quantity::from(7_u32));
    let second_recipient_asset_id = AssetId::new(asset_def_id.clone(), to_b.clone());
    let second_recipient_balance = block
        .world
        .asset(&second_recipient_asset_id)
        .expect("recipient b asset");
    assert_eq!(**second_recipient_balance, Quantity::from(4_u32));
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
    let mut vm = IVM::new(AMPLE_TEST_GAS_LIMIT);
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
    // Seed a valid ledger fixture directly. This test exercises transfer
    // authorization, not account/NFT registration authorization.
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let domain_id: DomainId = DomainId::try_new("wonder", "universal").unwrap();
    let domain = Domain::new(domain_id).build(&alice);
    let alice_account = Account::new(alice.clone()).build(&alice);
    let bob_account = Account::new(bob.clone()).build(&bob);
    let charlie_account = Account::new(charlie.clone()).build(&charlie);
    let nft = Nft::new(nft_id.clone(), Metadata::default()).build(&bob);
    let world = World::with_assets(
        [domain],
        [alice_account, bob_account, charlie_account],
        [],
        [],
        [nft],
    );
    let state = State::new_for_testing(world, kura, query_handle);
    let header = iroha_data_model::block::BlockHeader::new(
        core::num::NonZeroU64::new(1).unwrap(),
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut tx = block.transaction();
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
    let (authority, authority_key) = seeded_signer(11);
    let program = literal_program(
        &[
            tlv_blob(&authority, PointerType::AccountId as u16),
            tlv_blob(&key, PointerType::Name as u16),
            tlv_blob(&val, PointerType::Json as u16),
        ],
        &literal_call(
            ivm_sys::SYSCALL_SET_ACCOUNT_DETAIL,
            &[(10, 0), (11, 1), (12, 2)],
        ),
    );
    assert_eq!(queued_effects(&program, &authority).len(), 1);
    // Minimal world setup: domain and account
    let domain_id: DomainId = DomainId::try_new("wonder", "universal").unwrap();
    let world = World::with(
        [Domain::new(domain_id).build(&authority)],
        [built_account_in_domain(&authority)],
        [],
    );
    let state = crate::native_genesis::certified_state(world);
    {
        // Apply the queued detail set under its signed root
        let detail = signed_program(&state, &authority_key, program);
        let mut block = state.block(ordinary_header(&state));
        execute(&mut block, detail).unwrap();
        block.commit_world_overlay_for_testing().unwrap();
    } // drop block before taking a read-only view
    // Check metadata present
    let view = state.view();
    let acc = view.world.accounts().get(&authority).unwrap();
    assert_eq!(
        acc.metadata()
            .get(&key)
            .map(<iroha_primitives::json::Json as AsRef<str>>::as_ref),
        Some("1")
    );
}
#[test]
fn host_bridges_mint_asset() {
    // Build program: mint_asset(authority(), asset_definition("coin#wonder"), 123); HALT
    let (authority, authority_key) = seeded_signer(12);
    let asset_def: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonder", "universal").unwrap(),
            "coin".parse().unwrap(),
        );
    let program = literal_program(
        &[
            tlv_blob(&authority, PointerType::AccountId as u16),
            tlv_blob(&asset_def, PointerType::AssetDefinitionId as u16),
            quantity_tlv(&Quantity::from(123_u64)),
        ],
        &literal_call(ivm_sys::SYSCALL_MINT_ASSET, &[(10, 0), (11, 1), (12, 2)]),
    );
    assert_eq!(queued_effects(&program, &authority).len(), 1);
    // Minimal world setup: domain, account, asset def
    let domain_id: DomainId = DomainId::try_new("wonder", "universal").unwrap();
    let new_asset_def = AssetDefinition::numeric(
        asset_def.clone(),
        "coin".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        Some(domain_id.clone()),
    )
    .build(&authority);
    let world = World::with(
        [Domain::new(domain_id).build(&authority)],
        [built_account_in_domain(&authority)],
        [new_asset_def],
    );
    let state = crate::native_genesis::certified_state(world);
    {
        // Apply the queued mint under its signed root
        let mint = signed_program(&state, &authority_key, program);
        let mut block = state.block(ordinary_header(&state));
        execute(&mut block, mint).unwrap();
        block.commit_world_overlay_for_testing().unwrap();
    }
    let balance = state
        .view()
        .world
        .assets()
        .get(&AssetId::of(asset_def.clone(), authority.clone()))
        .map_or_else(Quantity::zero, |v| v.clone().into_inner());
    assert_eq!(balance, Quantity::from(123_u32));
}
#[test]
fn host_bridges_nft_set_metadata_and_burn() {
    // Setup an owner and NFT id
    let (owner, owner_key) = seeded_signer(13);
    let nft_id: NftId = "n0$wonder".parse().unwrap();
    // Build program: NFT_MINT_ASSET, NFT_SET_METADATA(nft_id, "flag", true), NFT_BURN_ASSET(nft_id)
    let key: Name = "flag".parse().unwrap();
    let val: iroha_primitives::json::Json = true.into();
    let literals = [
        tlv_blob(&nft_id, PointerType::NftId as u16),
        tlv_blob(&owner, PointerType::AccountId as u16),
        tlv_blob(&key, PointerType::Name as u16),
        tlv_blob(&val, PointerType::Json as u16),
    ];
    let code = [
        literal_call(ivm_sys::SYSCALL_NFT_MINT_ASSET, &[(10, 0), (11, 1)]),
        literal_call(
            ivm_sys::SYSCALL_NFT_SET_METADATA,
            &[(10, 0), (11, 2), (12, 3)],
        ),
        literal_call(ivm_sys::SYSCALL_NFT_BURN_ASSET, &[(10, 0)]),
    ]
    .concat();
    let program = literal_program(&literals, &code);
    assert_eq!(queued_effects(&program, &owner).len(), 3);
    // Minimal world: domain + owner
    let domain_id: DomainId = DomainId::try_new("wonder", "universal").unwrap();
    let world = World::with(
        [Domain::new(domain_id).build(&owner)],
        [built_account_in_domain(&owner)],
        [],
    );
    let state = crate::native_genesis::certified_state(world);
    {
        // Apply queued NFT_MINT_ASSET, NFT_SET_METADATA, NFT_BURN_ASSET under their signed root
        let transaction = signed_program(&state, &owner_key, program);
        let mut block = state.block(ordinary_header(&state));
        execute(&mut block, transaction).unwrap();
        block.commit_world_overlay_for_testing().unwrap();
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
    let asset_def_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonder", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
    let batch = TransferAssetBatch::new(vec![
        TransferAssetBatchEntry::new(from.clone(), to_a.clone(), asset_def_id.clone(), 7_u32),
        TransferAssetBatchEntry::new(from.clone(), to_b.clone(), asset_def_id.clone(), 4_u32),
    ]);
    let encoded_batch = norito::to_bytes(&batch).expect("encode batch");
    let decoded: TransferAssetBatch =
        norito::decode_from_bytes(&encoded_batch).expect("batch roundtrip");
    assert_eq!(decoded.entries().len(), 2, "encode/decode sanity check");
    let batch_tlv = norito_bytes_tlv(&batch);
    let mut vm = IVM::new(AMPLE_TEST_GAS_LIMIT);
    vm.set_host(CoreHost::new(from.clone()));
    let mut cursor = 0;
    let ptr_batch = load_input_blob(&mut vm, &mut cursor, &batch_tlv);
    run_syscall(
        &mut vm,
        ivm_sys::SYSCALL_TRANSFER_V1_BATCH_APPLY,
        &[(10, ptr_batch)],
    );
    let queued = with_core_host(&mut vm, |host| {
        host.drain_instructions()
            .expect("ordinary instruction inspection retains no attachment owner")
    });
    assert_eq!(queued.len(), 1, "transfer batch apply enqueues instruction");
    let batch_instr = queued[0]
        .as_any()
        .downcast_ref::<TransferAssetBatch>()
        .expect("queued instruction is a TransferAssetBatch");
    assert_eq!(batch_instr.entries().len(), 2, "two entries preserved");
    assert_eq!(batch_instr.entries()[0].to(), &to_a);
    assert_eq!(batch_instr.entries()[1].to(), &to_b);
}
