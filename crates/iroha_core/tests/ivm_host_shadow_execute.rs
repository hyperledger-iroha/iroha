//! `CoreHost` shadow-execute parity tests (IVM syscalls vs native Execute).
//!
//! Both paths execute as signed transactions through the node executor over the same
//! authenticated signed genesis: the native path carries the instructions, the host path a
//! generic IVM program whose syscalls queue them. Host effects apply only under that exact
//! signed root; the program's pointer arguments are its canonical literal TLVs.

use iroha_core::{
    smartcontracts::ivm::cache::IvmCache,
    state::{State, StateBlock, World, WorldReadOnly},
    tx::AcceptedTransaction,
};
use iroha_data_model::prelude::*;
use iroha_model_base::domain::DomainId;
use iroha_model_base::name::Name;
use iroha_primitives::nonzero;
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
use ivm::{PointerType, ProgramMetadata, encoding, instruction, syscalls as ivm_sys};
use mv::storage::StorageReadOnly;
use norito::NoritoSerialize;
const AMPLE_TEST_GAS_LIMIT: u64 = 1_000_000;
fn tlv_blob<T: NoritoSerialize>(val: &T, ty: PointerType) -> Vec<u8> {
    let payload = norito::to_bytes(val).expect("encode payload");
    let mut blob = Vec::with_capacity(2 + 1 + 4 + payload.len() + 32);
    blob.extend_from_slice(&(ty as u16).to_be_bytes());
    blob.push(1);
    blob.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_be_bytes());
    blob.extend_from_slice(&payload);
    let hash: [u8; 32] = iroha_crypto::Hash::new(&payload).into();
    blob.extend_from_slice(&hash);
    blob
}
fn quantity_tlv(value: &Quantity) -> Vec<u8> {
    ivm_abi::numeric_tlv::encode_quantity(value).expect("encode quantity pointer envelope")
}
/// Generic program whose pointer arguments are its authenticated literal-table TLVs.
fn literal_program(literals: &[Vec<u8>], code: &[u32]) -> Vec<u8> {
    let mut program = ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: 0,
        max_cycles: 1_000_000,
        abi_version: 1,
    }
    .encode();
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
fn setup_state(
    authority: &AccountId,
    asset_def: &AssetDefinitionId,
    asset_domain: &DomainId,
    asset_name: &Name,
) -> State {
    let domain = Domain::new(asset_domain.clone()).build(authority);
    let account = Account::new(authority.clone()).build(authority);
    let asset_definition = AssetDefinition::numeric(
        asset_def.clone(),
        asset_name.to_string(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        Some(asset_domain.clone()),
    )
    .build(authority);
    // The authenticated signed genesis gives the asset its incarnation before
    // executing the height-two native and IVM transactions.
    crate::native_genesis::certified_state(World::with([domain], [account], [asset_definition]))
}
/// Execute `executable` signed by ALICE in the first ordinary block of `state`, commit its
/// World overlay and return the events the transaction emitted.
fn execute_signed(
    state: &State,
    executable: Executable,
) -> Vec<iroha_data_model::events::EventBox> {
    let transaction = TransactionBuilder::new(
        *state.network_id_ref(),
        ALICE_ID.clone(),
        FeePaymentIntent::authority(Vec::new(), core::num::NonZeroU64::new(AMPLE_TEST_GAS_LIMIT)),
    )
    .with_executable(executable)
    .sign(ALICE_KEYPAIR.private_key());
    let parameters = state.transaction_admission_parameters();
    let accepted = AcceptedTransaction::accept(
        transaction,
        state.network_id_ref(),
        parameters.sumeragi().max_clock_drift(),
        parameters.transaction(),
        parameters
            .require_crypto_policy()
            .expect("signed genesis crypto policy"),
    )
    .expect("stateless transaction admission");
    let header = BlockHeader::new(
        nonzero!(2_u64),
        state.view().latest_block_hash(),
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    execute(&mut block, accepted).expect("signed execution applies");
    let events = block.world.take_external_events();
    block
        .commit_world_overlay_for_testing()
        .expect("commit executed block");
    events
}
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
fn data_event_debug(events: Vec<iroha_data_model::events::EventBox>) -> Vec<String> {
    events
        .into_iter()
        .filter_map(|ev| match ev {
            iroha_data_model::events::EventBox::Data(d) => Some(d),
            _ => None,
        })
        .map(|ev| format!("{:?}", ev.as_ref()))
        .collect()
}
#[test]
fn ivm_host_shadow_execute_matches_native_execute() {
    let authority = ALICE_ID.clone();
    let asset_domain = DomainId::try_new("wonderland", "universal").unwrap();
    let asset_name: Name = "coin".parse().unwrap();
    let asset_def_seed: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            asset_domain.clone(),
            asset_name.clone(),
        );
    let asset_def = AssetDefinitionId::parse_address_literal(&asset_def_seed.canonical_address())
        .expect("canonical asset definition literal");
    let key: Name = "parity_key".parse().unwrap();
    let value = iroha_primitives::json::Json::new("shadow");
    let amount = Quantity::from(100_u64);
    let direct_state = setup_state(&authority, &asset_def, &asset_domain, &asset_name);
    let host_state = setup_state(&authority, &asset_def, &asset_domain, &asset_name);
    // Direct Execute path.
    let direct_events = {
        let set_detail: InstructionBox =
            SetKeyValue::account(authority.clone(), key.clone(), value.clone()).into();
        let mint: InstructionBox = Mint::asset_quantity(
            amount.clone(),
            AssetId::of(asset_def.clone(), authority.clone()),
        )
        .into();
        execute_signed(&direct_state, [set_detail, mint].into_iter().collect())
    };
    // CoreHost path via syscalls.
    let host_events = {
        let literals = [
            tlv_blob(&authority, PointerType::AccountId),
            tlv_blob(&key, PointerType::Name),
            tlv_blob(&value, PointerType::Json),
            tlv_blob(&asset_def, PointerType::AssetDefinitionId),
            quantity_tlv(&amount),
        ];
        let code = [
            literal_call(
                ivm_sys::SYSCALL_SET_ACCOUNT_DETAIL,
                &[(10, 0), (11, 1), (12, 2)],
            ),
            literal_call(ivm_sys::SYSCALL_MINT_ASSET, &[(10, 0), (11, 3), (12, 4)]),
        ]
        .concat();
        execute_signed(
            &host_state,
            Executable::Ivm(IvmBytecode::from_compiled(literal_program(
                &literals, &code,
            ))),
        )
    };
    let direct_data = data_event_debug(direct_events);
    let host_data = data_event_debug(host_events);
    assert_eq!(direct_data, host_data, "data events must match");
    let asset_id = AssetId::of(asset_def.clone(), authority.clone());
    let direct_view = direct_state.view();
    let host_view = host_state.view();
    let direct_acc = direct_view.world.accounts().get(&authority).unwrap();
    let host_acc = host_view.world.accounts().get(&authority).unwrap();
    assert_eq!(
        direct_acc.metadata().get(&key).map(AsRef::as_ref),
        host_acc.metadata().get(&key).map(AsRef::as_ref)
    );
    let direct_balance = direct_view
        .world
        .assets()
        .get(&asset_id)
        .map_or_else(Quantity::zero, |v| v.clone().into_inner());
    let host_balance = host_view
        .world
        .assets()
        .get(&asset_id)
        .map_or_else(Quantity::zero, |v| v.clone().into_inner());
    assert_eq!(direct_balance, host_balance);
    assert_eq!(direct_balance, amount);
}
