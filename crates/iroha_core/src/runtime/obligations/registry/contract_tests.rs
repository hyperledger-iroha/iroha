//! Contract-owned kernel registration preserves exact invocation identity and ordinary admission.

use super::*;
use crate::{
    executor::ContractRuntimeExecutionContext,
    runtime::{catalog::additions, release_catalog},
    smartcontracts::{
        Execute,
        code::{self, ContractSubjectBinding},
    },
};
use iroha_crypto::Hash;
use iroha_data_model::{
    account::{Account, AccountDetails, AccountValue},
    block::BlockHeader,
    isi::{Log, runtime_obligation::RegisterContractRuntimeObligationV1},
    level::Level,
    runtime::{
        definition::{RestrictionEntryV1, RestrictionV1},
        lifecycle::ObligationKindV1,
    },
    smart_contract::{ContractArtifactId, manifest::ContractManifest},
};
use iroha_test_samples::{ALICE_ID, BOB_ID};

const NOW: u64 = 1_000;

fn state() -> crate::state::State {
    let world = crate::state::World::with(
        [],
        [
            Account::new(ALICE_ID.clone()).build(&ALICE_ID),
            Account::new(BOB_ID.clone()).build(&ALICE_ID),
        ],
        [],
    );
    crate::runtime::activate_additions_for_testing(&world, additions::RUNTIME_LIFECYCLE.module)
        .unwrap();
    let state = crate::state::State::new_for_testing(
        world,
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
    );
    let catalog = release_catalog().unwrap();
    let mut block = state.world.block();
    let original = block.runtime_selection.get().clone();
    let published = PublishedDefinitions(&block.runtime_definitions);
    let change = catalog
        .activation(
            additions::CONTRACT_LIFECYCLE.module,
            &original,
            &published,
            state.network_id,
            1,
            [0; 32],
        )
        .unwrap();
    let applied = change.apply(&original, &published).unwrap();
    for (id, descriptor) in applied.published {
        block.runtime_definitions.insert(id, descriptor);
    }
    *block.runtime_selection.get_mut() = applied.selection;
    block.commit();
    state
}
fn header() -> BlockHeader {
    BlockHeader::new(std::num::NonZeroU64::new(1).unwrap(), None, None, NOW, 0)
}
fn effect(message: &str) -> InstructionBox {
    Log::new(Level::INFO, message.to_owned()).into()
}
fn registration() -> RegisterContractRuntimeObligationV1 {
    RegisterContractRuntimeObligationV1 {
        kind: ObligationKindV1::Escrow,
        terminal_effects: vec![effect("release original contract obligation")],
        expires_ms: NOW + 60_000,
    }
}
fn seed_contract(
    tx: &mut StateTransaction<'_, '_>,
    parliament: bool,
) -> ContractRuntimeExecutionContext {
    let address = ContractAddress::derive(
        &tx.network_id,
        &ALICE_ID,
        7,
        iroha_model_base::topology::DataSpaceId::UNIVERSAL,
    )
    .unwrap();
    let subject = address.subject_id();
    let code_hash = Hash::new(b"contract kernel registration fixture");
    let artifact = ContractArtifactId::for_address(&address, code_hash).unwrap();
    let manifest = ContractManifest {
        seiyaku_name: Some("KernelRegistrationFixture".into()),
        code_hash: Some(code_hash),
        abi_hash: Some(Hash::prehashed(ivm::syscalls::compute_abi_hash(
            ivm::SyscallPolicy::AbiV1RuntimeLifecycle,
        ))),
        compiler_fingerprint: None,
        features_bitmap: Some(0),
        access_set_hints: None,
        permissions: Vec::new(),
        events: Vec::new(),
        entrypoints: None,
        states: None,
        error_types: None,
        enum_types: Vec::new(),
        error_messages: None,
        kotoba: None,
        provenance: None,
    };
    tx.world.accounts.insert(
        subject.clone(),
        AccountValue::new(AccountDetails::default()),
    );
    tx.world
        .contract_instances
        .insert(address.clone(), code_hash);
    tx.world
        .contract_subject_addresses
        .insert(subject.clone(), address.clone());
    let binding = if parliament {
        ContractSubjectBinding::new_parliament(&address, ALICE_ID.clone(), [1; 32], [2; 32])
    } else {
        ContractSubjectBinding::new_direct(&address, ALICE_ID.clone())
    };
    tx.world
        .contract_subject_bindings
        .insert(address.clone(), binding.with_active_code_hash(code_hash));
    tx.world.contract_manifests.insert(artifact, manifest);
    ContractRuntimeExecutionContext {
        contract_address: address,
        contract_subject: subject,
        contract_alias: None,
        entrypoint: "finish".into(),
    }
}
fn artifact(
    tx: &StateTransaction<'_, '_>,
    context: &ContractRuntimeExecutionContext,
) -> ContractArtifactId {
    ContractArtifactId::for_address(
        &context.contract_address,
        *tx.world
            .contract_instances
            .get(&context.contract_address)
            .unwrap(),
    )
    .unwrap()
}
fn restrict(tx: &mut StateTransaction<'_, '_>, wire: &str) {
    let definition = tx
        .world
        .runtime_selection
        .canonical()
        .bindings
        .iter()
        .find(|binding| binding.wire_id == wire)
        .unwrap()
        .interface;
    tx.world
        .runtime_selection
        .get_mut()
        .restrictions
        .push(RestrictionEntryV1 {
            definition,
            restriction: RestrictionV1::CompletionOnly(1),
        });
    tx.world
        .runtime_selection
        .get_mut()
        .restrictions
        .sort_by_key(|entry| entry.definition);
}

#[test]
fn contract_registration_requires_original_active_context_and_exact_subject() {
    let state = state();
    let mut block = state.block(header());
    let mut tx = block.transaction();
    let context = seed_contract(&mut tx, false);
    let request = registration();
    let subject = context.contract_subject.clone();
    assert!(register_contract(&mut tx, &subject, &request, None).is_err());
    assert!(
        request.clone().execute(&subject, &mut tx).is_err(),
        "a signed/native instruction has no opaque invocation"
    );
    assert!(
        register_contract(&mut tx, &ALICE_ID, &request, Some(&context)).is_err(),
        "lifecycle controller is not executing subject"
    );
    let mut foreign = context.clone();
    foreign.contract_subject = BOB_ID.clone();
    assert!(register_contract(&mut tx, &BOB_ID, &request, Some(&foreign)).is_err());
    assert_eq!(tx.world.runtime_obligations.iter().count(), 0);
    register_contract(&mut tx, &subject, &request, Some(&context)).unwrap();
    let id = ObligationId::derive(&subject).unwrap();
    let registered = tx.world.runtime_obligations.get(&id).unwrap();
    assert_eq!(registered.owner, subject);
    assert_eq!(registered.contract, Some(context.contract_address));
    assert_eq!(registered.created_at, 1);
    assert_eq!(
        registered.terminal_effects,
        terminal_effects(&request.terminal_effects).unwrap().0
    );
    assert_eq!(registered.expires_ms, request.expires_ms);
    assert!(!registered.capture.host_profiles.is_empty());
}

#[test]
fn contract_registration_requires_exact_manifest_and_refuses_pending_or_inactive_lifecycle() {
    let state = state();
    let mut block = state.block(header());
    let mut tx = block.transaction();
    let context = seed_contract(&mut tx, false);
    let request = registration();
    let subject = context.contract_subject.clone();
    let artifact = artifact(&tx, &context);
    let original = tx.world.contract_manifests.get(&artifact).unwrap().clone();
    for hash in [
        None,
        Some(Hash::prehashed(ivm::syscalls::compute_abi_hash(
            ivm::SyscallPolicy::AbiV1,
        ))),
    ] {
        tx.world
            .contract_manifests
            .get_mut(&artifact)
            .unwrap()
            .abi_hash = hash;
        assert!(register_contract(&mut tx, &subject, &request, Some(&context)).is_err());
    }
    tx.world.contract_manifests.insert(artifact, original);
    tx.world
        .contract_manifests
        .get_mut(&artifact)
        .unwrap()
        .code_hash = Some(Hash::prehashed([0x77; 32]));
    assert!(register_contract(&mut tx, &subject, &request, Some(&context)).is_err());
    tx.world
        .contract_manifests
        .get_mut(&artifact)
        .unwrap()
        .code_hash = Some(artifact.code_hash);
    code::set_pending_contract_lifecycle(
        &mut tx,
        &context.contract_address,
        Some(code::PendingContractLifecycle::Hajimari {
            transition_id: Hash::prehashed([4; 32]),
            code_hash: artifact.code_hash,
        }),
    );
    assert!(register_contract(&mut tx, &subject, &request, Some(&context)).is_err());
    code::set_pending_contract_lifecycle(&mut tx, &context.contract_address, None);
    tx.world
        .contract_instances
        .remove(context.contract_address.clone());
    tx.world
        .contract_subject_bindings
        .get_mut(&context.contract_address)
        .unwrap()
        .lifecycle
        .active_code_hash = None;
    assert!(register_contract(&mut tx, &subject, &request, Some(&context)).is_err());
    assert_eq!(tx.world.runtime_obligations.iter().count(), 0);
}

#[test]
fn contract_registration_is_new_work_under_canonical_admission_and_cannot_borrow_a_scope() {
    let state = state();
    let mut block = state.block(header());
    let mut tx = block.transaction();
    let context = seed_contract(&mut tx, false);
    let request = registration();
    let subject = context.contract_subject.clone();
    restrict(&mut tx, RegisterContractRuntimeObligationV1::WIRE_ID);
    assert!(register_contract(&mut tx, &subject, &request, Some(&context)).is_err());
    tx.world.runtime_selection.get_mut().restrictions.clear();
    restrict(&mut tx, iroha_data_model::isi::wire_id_of::<Log>().unwrap());
    assert!(
        register_contract(&mut tx, &subject, &request, Some(&context)).is_err(),
        "a new terminal effect needs ordinary admission too"
    );
    tx.world.runtime_selection.get_mut().restrictions.clear();
    let canonical = tx.world.runtime_selection.canonical().clone();
    let mut retained = canonical.clone();
    retained.completion_closures.clear();
    retained.bindings.clear();
    tx.world.runtime_selection.set_executing(Some(retained));
    assert!(register_contract(&mut tx, &subject, &request, Some(&context)).is_err());
    assert_eq!(tx.world.runtime_selection.canonical(), &canonical);
    tx.world.runtime_selection.set_executing(None);
    register_contract(&mut tx, &subject, &request, Some(&context)).unwrap();
    let id = ObligationId::derive(&subject).unwrap();
    let before = tx.world.runtime_obligations.get(&id).unwrap().clone();
    let mut invalid = request.clone();
    invalid.expires_ms = NOW;
    assert!(register_contract(&mut tx, &subject, &invalid, Some(&context)).is_err());
    invalid = request.clone();
    invalid.terminal_effects.clear();
    assert!(register_contract(&mut tx, &subject, &invalid, Some(&context)).is_err());
    assert_eq!(tx.world.runtime_obligations.get(&id), Some(&before));
}

#[test]
fn contract_registration_and_frozen_controller_registration_keep_distinct_authority() {
    for parliament in [false, true] {
        let state = state();
        let mut block = state.block(header());
        let mut tx = block.transaction();
        let context = seed_contract(&mut tx, parliament);
        let request = registration();
        let subject = context.contract_subject.clone();
        let controller = RegisterRuntimeObligationV1 {
            contract: Some(context.contract_address.clone()),
            kind: request.kind,
            terminal_effects: request.terminal_effects.clone(),
            expires_ms: request.expires_ms,
        };
        assert_eq!(
            register(&mut tx, &ALICE_ID, &controller).is_ok(),
            !parliament
        );
        assert!(register(&mut tx, &BOB_ID, &controller).is_err());
        register_contract(&mut tx, &subject, &request, Some(&context)).unwrap();
        let id = ObligationId::derive(&subject).unwrap();
        let expected = terminal_effect_id(&request.terminal_effects[0]).unwrap();
        assert_eq!(
            tx.world
                .runtime_obligations
                .get(&id)
                .unwrap()
                .pending_effect(&expected),
            Some(0)
        );
        restrict(&mut tx, iroha_data_model::isi::wire_id_of::<Log>().unwrap());
        performing(&mut tx, &subject, &request.terminal_effects[0], |tx| {
            crate::runtime::resolve_executing(tx, &request.terminal_effects[0])
                .map(|_| ())
                .map_err(ValidationFail::InstructionFailed)
        })
        .unwrap();
        assert_eq!(
            tx.world
                .runtime_obligations
                .get(&id)
                .unwrap()
                .pending_effect(&expected),
            None
        );
        complete(
            &mut tx,
            &subject,
            &CompleteRuntimeObligationV1 {
                owner: subject.clone(),
            },
        )
        .unwrap();
        assert!(tx.world.runtime_obligations.get(&id).is_none());
    }
}

#[test]
fn closed_named_host_profile_refuses_account_and_contract_registrations_before_replacement() {
    let state = state();
    let mut block = state.block(header());
    let mut tx = block.transaction();
    let context = seed_contract(&mut tx, false);
    let account_request = RegisterRuntimeObligationV1 {
        contract: None,
        kind: ObligationKindV1::Escrow,
        terminal_effects: vec![effect("account obligation")],
        expires_ms: NOW + 60_000,
    };
    register(&mut tx, &ALICE_ID, &account_request).unwrap();
    let account_id = ObligationId::derive(&ALICE_ID).unwrap();
    let before = tx
        .world
        .runtime_obligations
        .get(&account_id)
        .unwrap()
        .clone();
    let hash = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1RuntimeLifecycle);
    let profile = *tx.world.runtime_selection.get().active.iter().find(|id| {
        matches!(tx.world.runtime_definitions.get(id),
            Some(iroha_data_model::runtime::definition::DefinitionDescriptorV1::HostProfile(profile))
                if profile.abi_hash == hash)
    }).unwrap();
    for restriction in [
        RestrictionV1::AdmissionClosed(1),
        RestrictionV1::CompletionOnly(1),
        RestrictionV1::Disabled(1),
    ] {
        tx.world.runtime_selection.get_mut().restrictions = vec![RestrictionEntryV1 {
            definition: profile,
            restriction,
        }];
        assert!(register(&mut tx, &ALICE_ID, &account_request).is_err());
        assert!(
            register_contract(
                &mut tx,
                &context.contract_subject,
                &registration(),
                Some(&context)
            )
            .is_err()
        );
        assert_eq!(tx.world.runtime_obligations.get(&account_id), Some(&before));
        assert_eq!(tx.world.runtime_obligations.iter().count(), 1);
    }
}
