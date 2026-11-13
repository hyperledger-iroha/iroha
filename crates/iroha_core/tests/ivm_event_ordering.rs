//! Ensure structured Data events emitted as a result of VM syscalls (OUTPUT → ISIs)

//! preserve syscall order within a single transaction overlay.
//!
//! Host effects reach State only under the exact signed root of the transaction that ran the
//! program, so the syscalls run as one signed generic IVM transaction over an authenticated
//! signed genesis; their pointer arguments are the program's canonical literal TLVs.
#![allow(clippy::cast_possible_truncation, clippy::items_after_statements)]
use iroha_core::{
    smartcontracts::ivm::cache::IvmCache,
    state::{State, StateBlock},
    tx::AcceptedTransaction,
};
use iroha_crypto::{Hash, KeyPair};
use iroha_data_model::{block::BlockHeader, prelude::*};
use iroha_model_base::domain::DomainId;
use iroha_model_base::name::Name;
use iroha_primitives::nonzero;
use ivm::{PointerType, ProgramMetadata, encoding, instruction, syscalls};
use std::str::FromStr;
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
fn encode_prog_syscall(num: u32) -> Vec<u8> {
    let mut code = Vec::new();
    code.extend_from_slice(&encoding::wide::encode_scall(num).to_le_bytes());
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let mut prog = ProgramMetadata::default().encode();
    prog.extend_from_slice(&code);
    prog
}
/// Generic program whose pointer arguments are its authenticated literal-table TLVs.
fn literal_program(literals: &[Vec<u8>], code: &[u32]) -> Vec<u8> {
    // Signed generic programs declare their cycle bound in the authenticated header.
    let mut program = ProgramMetadata::default().encode();
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
        .chain([encoding::wide::encode_scall(syscall)])
        .collect()
}
/// Statelessly admit `program` as a generic IVM transaction signed by `key`.
fn signed_program(state: &State, key: &KeyPair, program: Vec<u8>) -> AcceptedTransaction<'static> {
    let transaction = TransactionBuilder::new(
        *state.network_id_ref(),
        AccountId::new(key.public_key().clone()),
        FeePaymentIntent::authority(Vec::new(), core::num::NonZeroU64::new(1_000_000)),
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
fn encode_prog_syscall_uses_valid_metadata_header() {
    let program = encode_prog_syscall(syscalls::SYSCALL_SET_ACCOUNT_DETAIL);
    let parsed = ProgramMetadata::parse(&program).expect("metadata should parse");
    assert_eq!(parsed.metadata.version_major, 1);
    assert_eq!(parsed.metadata.version_minor, 1);
    assert_eq!(parsed.metadata.abi_version, 1);
    assert_eq!(parsed.code_offset, parsed.header_len);
    assert!(
        parsed.code_offset < program.len(),
        "program should contain code"
    );
}
#[test]
#[allow(clippy::too_many_lines)]
fn ivm_syscall_data_events_follow_order() {
    // World with domain/account and asset definition for mint
    let (authority_id, authority_key) = iroha_test_samples::gen_account_in("wonderland");
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
    let domain = Domain::new(domain_id.clone()).build(&authority_id);
    let account = Account::new(authority_id.clone()).build(&authority_id);
    let asset_def = AssetDefinition::new(
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        ),
        "rose".to_owned(),
        NumericSpec::default(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&authority_id);
    let world = iroha_core::state::World::with_assets([domain], [account], [asset_def], [], []);
    let state = crate::native_genesis::certified_state(world);
    let header = BlockHeader::new(
        nonzero!(2_u64),
        state.view().latest_block_hash(),
        None,
        0,
        0,
    );
    // Prepare literal TLVs for account id, metadata keys/values, and asset definition id.
    let acct_bytes = norito::to_bytes(&authority_id).expect("encode account id");
    let key1 = Name::from_str("account_key").expect("name");
    let key1_bytes = norito::to_bytes(&key1).expect("encode key");
    let val1 = Json::new(norito::json!({"kind": "alpha"}));
    let val1_bytes = norito::to_bytes(&val1).expect("encode json");
    let key2 = Name::from_str("account_second_key").expect("name");
    let key2_bytes = norito::to_bytes(&key2).expect("encode key");
    let val2 = Json::new(norito::json!({"kind": "beta"}));
    let val2_bytes = norito::to_bytes(&val2).expect("encode json");
    let asset_def_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
    let asset_bytes = norito::to_bytes(&asset_def_id).expect("encode asset definition");
    let literals = [
        make_tlv(PointerType::AccountId, &acct_bytes),
        make_tlv(PointerType::Name, &key1_bytes),
        make_tlv(PointerType::Json, &val1_bytes),
        make_tlv(PointerType::Name, &key2_bytes),
        make_tlv(PointerType::Json, &val2_bytes),
        make_tlv(PointerType::AssetDefinitionId, &asset_bytes),
        ivm_abi::numeric_tlv::encode_quantity(&Quantity::from(5_u64))
            .expect("encode quantity pointer envelope"),
    ];
    let code = [
        // 1) Account metadata insert
        literal_call(
            syscalls::SYSCALL_SET_ACCOUNT_DETAIL,
            &[(10, 0), (11, 1), (12, 2)],
        ),
        // 2) Second metadata insert
        literal_call(
            syscalls::SYSCALL_SET_ACCOUNT_DETAIL,
            &[(10, 0), (11, 3), (12, 4)],
        ),
        // 3) Mint asset into the authority account
        literal_call(syscalls::SYSCALL_MINT_ASSET, &[(10, 0), (11, 5), (12, 6)]),
    ]
    .concat();
    let transaction = signed_program(&state, &authority_key, literal_program(&literals, &code));
    // Apply the queued ISIs under the program's signed root
    let mut block = state.block(header);
    execute(&mut block, transaction).expect("apply queued syscalls");
    // Collect Data events emitted during execution
    let events = block.world.take_external_events();
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
    let debug_events: Vec<String> = data_events.iter().map(|ev| format!("{ev:?}")).collect();
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
