//! Every release stops at the same envelope execution can never apply
//! (`specs/live_runtime_upgrades.md` §5.2).

use iroha_crypto::Hash;
use iroha_data_model::{
    Registrable as _,
    account::Account,
    block::BlockHeader,
    isi::{InstructionBox, Log, OpaqueInstructionV1, SetKeyValue},
    level::Level,
    runtime::definition::{BindingV1, RestrictionEntryV1, RestrictionV1},
    transaction::{
        Executable, ExecutableBatchItem, FeePaymentIntent, SignedTransaction, TransactionBuilder,
        executable::ContractInvocation,
    },
};
use iroha_model_base::topology::DataSpaceId;
use iroha_primitives::json::Json;
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};

use super::*;
use crate::{
    executor::Executor,
    smartcontracts::ivm::cache::IvmCache,
    state::{State, StateTransaction, World, WorldReadOnly as _},
};

/// A wire ID no release registers.
const UNKNOWN: &str = "test.unreleased.instruction.v1";
/// A block height after genesis.
const HEIGHT: u64 = 7;

fn baseline() -> RootSelectionV1 {
    crate::runtime::founded_selection().expect("baseline")
}

fn log() -> InstructionBox {
    Log::new(Level::INFO, "envelope".into()).into()
}

fn log_wire_id() -> &'static str {
    iroha_data_model::isi::wire_id_of::<Log>().expect("registered")
}

fn opaque(wire_id: &str) -> InstructionBox {
    OpaqueInstructionV1::new(wire_id, vec![0xA5; 24])
        .expect("well-formed wire id")
        .into()
}

fn write(key: &str) -> InstructionBox {
    SetKeyValue::account(ALICE_ID.clone(), key.parse().expect("key"), Json::new(true)).into()
}

fn inactive(wire_id: &str) -> RuntimeRejectionV1 {
    RuntimeRejectionV1::DefinitionInactive {
        wire_id: wire_id.to_owned(),
    }
}

fn malformed(wire_id: &str) -> RuntimeRejectionV1 {
    RuntimeRejectionV1::MalformedPayload {
        wire_id: wire_id.to_owned(),
    }
}

/// The committed failure of a stable rejection.
fn failed(rejection: RuntimeRejectionV1) -> ValidationFail {
    ValidationFail::InstructionFailed(InstructionExecutionError::Runtime(rejection))
}

fn without_log(mut selection: RootSelectionV1) -> RootSelectionV1 {
    selection
        .bindings
        .retain(|binding| binding.wire_id != log_wire_id());
    selection
}

/// The binding that answers `Log`.
fn log_binding(selection: &RootSelectionV1) -> BindingV1 {
    selection
        .bindings
        .iter()
        .find(|binding| binding.wire_id == log_wire_id())
        .expect("bound")
        .clone()
}

/// `selection` with `wire_id` bound to the interface and implementation that answer `Log`.
fn with_binding(mut selection: RootSelectionV1, wire_id: &str) -> RootSelectionV1 {
    let template = log_binding(&selection);
    let index = selection
        .bindings
        .binary_search_by(|binding| binding.wire_id.as_str().cmp(wire_id))
        .expect_err("not yet bound");
    selection.bindings.insert(
        index,
        BindingV1 {
            wire_id: wire_id.to_owned(),
            ..template
        },
    );
    selection
}

fn contract_call() -> ContractInvocation {
    ContractInvocation {
        contract_address: iroha_data_model::smart_contract::ContractAddress::derive(
            &iroha_data_model::NetworkId::from_genesis_hash(
                iroha_crypto::HashOf::from_untyped_unchecked(Hash::prehashed([0x41; 32])),
            ),
            &ALICE_ID,
            0,
            DataSpaceId::UNIVERSAL,
        )
        .expect("contract address"),
        expected_code_hash: Hash::new(b"envelope test contract"),
        entrypoint: "main".to_owned(),
        arguments: None,
    }
}

#[test]
fn bound_decoded_envelopes_are_admitted() {
    let selection = baseline();
    assert_eq!(inactive_envelope(&selection, &log()), None);
    assert_eq!(
        refuse_unexecutable_envelope(&selection, HEIGHT, &write("a")),
        Ok(())
    );
    let executable = Executable::Instructions(vec![log(), write("a")].into());
    assert_eq!(admit_envelopes(&selection, HEIGHT, &executable), Ok(()));
    assert_eq!(first_unadmitted_envelope(&selection, &executable), None);
}

#[test]
fn the_first_unbound_envelope_is_refused_in_order() {
    let selection = baseline();
    // An envelope this release kept opaque names a wire ID no selection of this release binds.
    assert_eq!(
        inactive_envelope(&selection, &opaque(UNKNOWN)),
        Some(inactive(UNKNOWN))
    );
    let executable = Executable::Instructions(
        vec![log(), opaque(UNKNOWN), opaque("test.unreleased.second.v1")].into(),
    );
    assert_eq!(
        admit_envelopes(&selection, HEIGHT, &executable),
        Err(failed(inactive(UNKNOWN)))
    );
    assert_eq!(
        first_unadmitted_envelope(&selection, &executable),
        Some(inactive(UNKNOWN))
    );
    // A wire ID this release registers is refused alike while the selection does not bind it:
    // a release that knows the instruction stops where one that kept it opaque stops.
    let selection = without_log(selection);
    let executable = Executable::Instructions(vec![write("a"), log(), opaque(UNKNOWN)].into());
    assert_eq!(
        admit_envelopes(&selection, HEIGHT, &executable),
        Err(failed(inactive(log_wire_id())))
    );
    assert_eq!(
        first_unadmitted_envelope(&selection, &executable),
        Some(inactive(log_wire_id()))
    );
}

#[test]
fn a_bound_opaque_envelope_is_a_malformed_payload_after_its_restrictions() {
    let selection = with_binding(baseline(), UNKNOWN);
    assert_eq!(inactive_envelope(&selection, &opaque(UNKNOWN)), None);
    assert_eq!(
        refuse_unexecutable_envelope(&selection, HEIGHT, &opaque(UNKNOWN)),
        Err(InstructionExecutionError::Runtime(malformed(UNKNOWN)))
    );
    let executable = Executable::Instructions(vec![write("a"), opaque(UNKNOWN), log()].into());
    assert_eq!(
        admit_envelopes(&selection, HEIGHT, &executable),
        Err(failed(malformed(UNKNOWN)))
    );
    assert_eq!(
        first_unadmitted_envelope(&selection, &executable),
        Some(malformed(UNKNOWN))
    );
    // Its restrictions come first, exactly as resolution checks them at execution.
    let binding = log_binding(&selection);
    let restriction = RestrictionV1::Disabled(HEIGHT);
    let mut restricted = selection;
    restricted.restrictions = vec![RestrictionEntryV1 {
        definition: binding.interface,
        restriction,
    }];
    assert_eq!(
        refuse_unexecutable_envelope(&restricted, HEIGHT, &opaque(UNKNOWN)),
        Err(InstructionExecutionError::Runtime(
            RuntimeRejectionV1::DefinitionRestricted {
                definition: binding.interface,
                restriction,
            }
        ))
    );
    assert_eq!(
        refuse_unexecutable_envelope(&restricted, HEIGHT - 1, &opaque(UNKNOWN)),
        Err(InstructionExecutionError::Runtime(malformed(UNKNOWN)))
    );
    // A decoded envelope's restrictions wait for execution and its obligation context.
    assert_eq!(
        refuse_unexecutable_envelope(&restricted, HEIGHT, &log()),
        Ok(())
    );
}

#[test]
fn batch_instruction_items_are_top_level_envelopes() {
    let selection = baseline();
    let batch = Executable::Batch(
        vec![
            ExecutableBatchItem::ContractCall(contract_call()),
            ExecutableBatchItem::Instruction(log()),
            ExecutableBatchItem::Instruction(opaque(UNKNOWN)),
        ]
        .into(),
    );
    assert_eq!(
        admit_envelopes(&selection, HEIGHT, &batch),
        Err(failed(inactive(UNKNOWN)))
    );
    assert_eq!(
        first_unadmitted_envelope(&selection, &batch),
        Some(inactive(UNKNOWN))
    );
    // A contract call carries no top-level envelope: what it emits is refused at execution.
    let call = Executable::ContractCall(contract_call());
    assert_eq!(admit_envelopes(&selection, HEIGHT, &call), Ok(()));
    assert_eq!(first_unadmitted_envelope(&selection, &call), None);
}

#[test]
fn only_an_opaque_envelope_has_an_opaque_rejection() {
    assert_eq!(opaque_rejection(&opaque(UNKNOWN)), Some(malformed(UNKNOWN)));
    assert_eq!(opaque_rejection(&log()), None);
}

/// One post-genesis state where Alice may write her own metadata.
fn fixture() -> State {
    crate::pipeline::overlay::test_support::state_after_genesis(World::with(
        [],
        [Account::new(ALICE_ID.clone()).build(&ALICE_ID)],
        [],
    ))
}

fn next_header(state: &State) -> BlockHeader {
    BlockHeader::new(
        core::num::NonZeroU64::new(state.committed_height() as u64 + 1)
            .expect("post-genesis height"),
        state.view().latest_block_hash(),
        None,
        0,
        0,
    )
}

fn signed(state: &State, executable: Executable) -> SignedTransaction {
    TransactionBuilder::new(
        state.network_id,
        ALICE_ID.clone(),
        FeePaymentIntent::authority(Vec::new(), core::num::NonZeroU64::new(50_000_000)),
    )
    .with_executable(executable)
    .sign(ALICE_KEYPAIR.private_key())
}

/// Execute `instructions` signed by Alice; `prepare` adjusts the transaction's State first.
fn execute(
    instructions: Vec<InstructionBox>,
    prepare: impl FnOnce(&mut StateTransaction<'_, '_>),
    check: impl FnOnce(&StateTransaction<'_, '_>, Result<(), ValidationFail>),
) {
    let state = fixture();
    let source = signed(&state, Executable::Instructions(instructions.into()));
    let mut block = state.block(next_header(&state));
    let mut tx = block.transaction_for_fastpq_testing(Hash::from(source.hash_as_entrypoint()));
    tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    prepare(&mut tx);
    let result = Executor::Initial
        .execute_transaction(&mut tx, &ALICE_ID, &source, &mut IvmCache::new())
        .map_err(crate::execution_attempt::expect_completed_rejection);
    check(&tx, result);
}

fn wrote(tx: &StateTransaction<'_, '_>, key: &str) -> bool {
    tx.world
        .account(&ALICE_ID)
        .expect("Alice")
        .metadata()
        .get(key)
        .is_some()
}

/// The transaction stopped before its first instruction ran or any instruction work was metered.
fn untouched(tx: &StateTransaction<'_, '_>, key: &str) {
    assert!(!wrote(tx, key), "the first instruction never ran");
    assert_eq!(tx.last_tx_gas_used, 0, "no instruction work was metered");
}

#[test]
fn a_transaction_stops_at_its_unknown_envelope_before_any_effect() {
    execute(
        vec![write("before_unknown"), opaque(UNKNOWN)],
        |_| {},
        |tx, result| {
            assert_eq!(result, Err(failed(inactive(UNKNOWN))));
            untouched(tx, "before_unknown");
        },
    );
}

#[test]
fn a_transaction_stops_at_a_registered_envelope_the_selection_does_not_bind() {
    execute(
        vec![write("before_unbound"), log()],
        |tx| *tx.world.runtime_selection.get_mut() = without_log(baseline()),
        |tx, result| {
            assert_eq!(result, Err(failed(inactive(log_wire_id()))));
            untouched(tx, "before_unbound");
        },
    );
    // The same transaction executes under the baseline.
    execute(
        vec![write("bound"), log()],
        |_| {},
        |tx, result| {
            assert_eq!(result, Ok(()));
            assert!(wrote(tx, "bound"));
        },
    );
}

#[test]
fn a_transaction_stops_at_a_bound_opaque_envelope_before_any_effect() {
    execute(
        vec![write("before_opaque"), opaque(UNKNOWN)],
        |tx| *tx.world.runtime_selection.get_mut() = with_binding(baseline(), UNKNOWN),
        |tx, result| {
            assert_eq!(result, Err(failed(malformed(UNKNOWN))));
            untouched(tx, "before_opaque");
        },
    );
}

#[test]
fn a_nested_envelope_is_refused_where_it_executes() {
    // Contract output, trigger bodies and approved proposals reach execution one instruction at
    // a time; an envelope execution can never apply is refused there with the same rejection,
    // before any executor's own admission.
    let state = fixture();
    let mut block = state.block(next_header(&state));
    let mut tx = block.transaction_for_callback_testing();
    tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    assert_eq!(
        Executor::Initial.execute_instruction(&mut tx, &ALICE_ID, opaque(UNKNOWN)),
        Err(failed(inactive(UNKNOWN)))
    );
    assert_eq!(
        crate::smartcontracts::isi::execute_borrowed_instruction(
            &opaque(UNKNOWN),
            &ALICE_ID,
            &mut tx
        ),
        Err(InstructionExecutionError::Runtime(inactive(UNKNOWN)))
    );
    *tx.world.runtime_selection.get_mut() = without_log(baseline());
    assert_eq!(
        Executor::Initial.execute_instruction(&mut tx, &ALICE_ID, log()),
        Err(failed(inactive(log_wire_id())))
    );
    // Once an interface answers its wire ID, an envelope this release still keeps opaque (a frame
    // the interface's decoder does not accept canonically) is refused with `malformed_payload`.
    *tx.world.runtime_selection.get_mut() = with_binding(baseline(), UNKNOWN);
    assert_eq!(
        Executor::Initial.execute_instruction(&mut tx, &ALICE_ID, opaque(UNKNOWN)),
        Err(failed(malformed(UNKNOWN)))
    );
    assert_eq!(
        crate::smartcontracts::isi::execute_borrowed_instruction(
            &opaque(UNKNOWN),
            &ALICE_ID,
            &mut tx
        ),
        Err(InstructionExecutionError::Runtime(malformed(UNKNOWN)))
    );
}

#[test]
fn a_restriction_holds_on_paths_outside_the_native_dispatch_table() {
    // Linked-FI transfers, controlled locks and multisig execute without the native dispatch
    // table; resolution applies the root's restrictions before any of them (§7.1).
    let transfer: InstructionBox = iroha_data_model::isi::asset_fi::TransferToLinkedFiV1 {
        asset_definition_id: iroha_config::parameters::defaults::nexus::fees::fee_asset_id()
            .parse()
            .expect("fee asset definition"),
        account_id: ALICE_ID.clone(),
        amount: iroha_primitives::numeric::Quantity::one(),
        expected_link_revision: 0,
        expected_policy_revision: 0,
    }
    .into();
    let wire_id = instruction_wire_id(&transfer)
        .expect("registered")
        .to_owned();
    let binding = baseline()
        .bindings
        .into_iter()
        .find(|binding| binding.wire_id == wire_id)
        .expect("baseline binding");
    let restriction = RestrictionV1::AdmissionClosed(0);
    let mut restricted = baseline();
    restricted.restrictions = vec![RestrictionEntryV1 {
        definition: binding.interface,
        restriction,
    }];
    let rejection = RuntimeRejectionV1::DefinitionRestricted {
        definition: binding.interface,
        restriction,
    };
    assert_eq!(
        resolve_envelope(&restricted, HEIGHT, &transfer, || None),
        Err(InstructionExecutionError::Runtime(rejection.clone()))
    );
    assert_eq!(
        resolve_envelope(&baseline(), HEIGHT, &transfer, || None),
        Ok(Some(binding.implementation))
    );

    let state = fixture();
    let mut block = state.block(next_header(&state));
    let mut tx = block.transaction_for_callback_testing();
    tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    *tx.world.runtime_selection.get_mut() = restricted;
    assert_eq!(
        Executor::Initial.execute_instruction(&mut tx, &ALICE_ID, transfer),
        Err(failed(rejection)),
        "the linked-FI path runs only after resolution"
    );
}
