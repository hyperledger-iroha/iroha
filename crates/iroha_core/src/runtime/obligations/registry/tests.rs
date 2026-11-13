//! Registered obligations: registration, the completions restrictions leave them, and the change
//! sets that would alter their closure.

use iroha_data_model::{
    isi::{InstructionBox, Log},
    level::Level,
    runtime::{
        definition::{RestrictionEntryV1, RestrictionV1},
        lifecycle::ObligationKindV1,
    },
};
use iroha_test_samples::{ALICE_ID, BOB_ID};

use super::*;
use crate::runtime::{
    catalog::additions::RUNTIME_LIFECYCLE,
    obligations::{LiveObligations, Step, step, stranded},
    release_catalog, resolve_executing,
};

const NOW_MS: u64 = 1_000;

/// A State whose root has activated the kernel's lifecycle State.
fn lifecycle_state() -> crate::state::State {
    let state = crate::state::State::new_for_testing(
        crate::state::World::default(),
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
    );
    crate::runtime::activate_additions_for_testing(&state.world, RUNTIME_LIFECYCLE.module)
        .expect("activate the lifecycle addition");
    state
}

fn header() -> iroha_data_model::block::BlockHeader {
    iroha_data_model::block::BlockHeader::new(
        std::num::NonZeroU64::new(1).expect("height"),
        None,
        None,
        NOW_MS,
        0,
    )
}

fn log(message: &str) -> InstructionBox {
    Log::new(Level::INFO, message.to_owned()).into()
}

fn registration(effects: Vec<InstructionBox>) -> RegisterRuntimeObligationV1 {
    RegisterRuntimeObligationV1 {
        contract: None,
        kind: ObligationKindV1::Escrow,
        terminal_effects: effects,
        expires_ms: NOW_MS + 60_000,
    }
}

fn log_interface() -> iroha_data_model::runtime::definition::DefinitionId {
    let catalog = release_catalog().expect("catalog");
    let wire_id = iroha_data_model::isi::wire_id_of::<Log>().expect("registered");
    catalog
        .baseline
        .bindings
        .iter()
        .find(|binding| binding.wire_id == wire_id)
        .expect("the baseline binds Log")
        .interface
}

/// Restrict the interface that answers `Log` in the executing selection.
fn restrict_log(stx: &mut StateTransaction<'_, '_>, restriction: RestrictionV1) {
    let selection = stx.world.runtime_selection.get_mut();
    selection
        .restrictions
        .retain(|entry| entry.definition != log_interface());
    selection.restrictions.push(RestrictionEntryV1 {
        definition: log_interface(),
        restriction,
    });
    selection.restrictions.sort_by_key(|entry| entry.definition);
}

/// Whether `instruction`, issued by `authority`, resolves where it executes.
fn resolves(
    stx: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    instruction: &InstructionBox,
) -> bool {
    performing(stx, authority, instruction, |stx| {
        resolve_executing(stx, instruction)
            .map(|_| ())
            .map_err(ValidationFail::InstructionFailed)
    })
    .is_ok()
}

#[test]
fn an_owner_registers_one_obligation_and_ends_it() {
    let state = lifecycle_state();
    let mut block = state.block(header());
    let mut stx = block.transaction();
    let end = InstructionBox::from(CompleteRuntimeObligationV1 {
        owner: ALICE_ID.clone(),
    });
    assert_eq!(step(&end, &stx.world), None, "no obligation is registered");

    register(
        &mut stx,
        &ALICE_ID,
        &registration(vec![log("release"), log("refund")]),
    )
    .expect("register");
    let id = ObligationId::derive(&ALICE_ID).unwrap();
    let record = stx.world.runtime_obligations.get(&id).cloned().unwrap();
    assert_eq!(record.owner, *ALICE_ID);
    assert_eq!(record.contract, None);
    assert_eq!(record.created_at, 1);
    assert_eq!(record.terminal_effects.len(), 2);
    assert!(record.is_live(NOW_MS));
    assert_eq!(step(&end, &stx.world), Some(Step::Complete));
    assert!(
        !LiveObligations::of(&stx.world)
            .with_registered(&stx.world, NOW_MS)
            .is_empty()
    );
    assert!(
        LiveObligations::of(&stx.world)
            .with_registered(&stx.world, record.expires_ms)
            .is_empty(),
        "an expired obligation keeps no closure"
    );

    // Registering again replaces the owner's one obligation.
    register(&mut stx, &ALICE_ID, &registration(vec![log("only")])).expect("replace");
    assert_eq!(
        stx.world
            .runtime_obligations
            .get(&id)
            .unwrap()
            .terminal_effects
            .len(),
        1
    );
    assert_eq!(stx.world.runtime_obligations.iter().count(), 1);

    // Only the owner ends a live obligation.
    let end = CompleteRuntimeObligationV1 {
        owner: ALICE_ID.clone(),
    };
    assert!(complete(&mut stx, &BOB_ID, &end).is_err());
    complete(&mut stx, &ALICE_ID, &end).expect("the owner ends it");
    assert!(stx.world.runtime_obligations.get(&id).is_none());
    assert!(
        complete(&mut stx, &ALICE_ID, &end).is_err(),
        "nothing is held"
    );
}

#[test]
fn anyone_ends_an_obligation_that_is_no_longer_live() {
    let state = lifecycle_state();
    let mut block = state.block(header());
    let mut stx = block.transaction();
    register(&mut stx, &ALICE_ID, &registration(vec![log("release")])).expect("register");
    let end = CompleteRuntimeObligationV1 {
        owner: ALICE_ID.clone(),
    };
    assert!(complete(&mut stx, &BOB_ID, &end).is_err());
    assert!(resolves(&mut stx, &ALICE_ID, &log("release")));
    complete(&mut stx, &BOB_ID, &end).expect("every committed instruction is performed");
}

#[test]
fn a_registration_is_bounded_and_new_work() {
    let state = lifecycle_state();
    let mut block = state.block(header());
    let mut stx = block.transaction();
    assert!(register(&mut stx, &ALICE_ID, &registration(Vec::new())).is_err());
    assert!(
        register(
            &mut stx,
            &ALICE_ID,
            &registration(vec![log("same"), log("same")])
        )
        .is_err()
    );
    let lifecycle = InstructionBox::from(CompleteRuntimeObligationV1 {
        owner: ALICE_ID.clone(),
    });
    assert!(register(&mut stx, &ALICE_ID, &registration(vec![lifecycle])).is_err());
    let mut expired = registration(vec![log("release")]);
    expired.expires_ms = NOW_MS;
    assert!(register(&mut stx, &ALICE_ID, &expired).is_err());
    let mut kernel_kind = registration(vec![log("release")]);
    kernel_kind.kind = ObligationKindV1::MigrationCursor;
    assert!(register(&mut stx, &ALICE_ID, &kernel_kind).is_err());

    let unknown = iroha_data_model::smart_contract::ContractAddress::derive(
        &stx.network_id,
        &ALICE_ID,
        7,
        iroha_model_base::topology::DataSpaceId::UNIVERSAL,
    )
    .expect("contract address");
    let mut for_contract = registration(vec![log("release")]);
    for_contract.contract = Some(unknown);
    assert!(
        register(&mut stx, &ALICE_ID, &for_contract).is_err(),
        "only the lifecycle controller of a deployed contract registers for it"
    );

    restrict_log(&mut stx, RestrictionV1::AdmissionClosed(1));
    assert!(
        register(&mut stx, &ALICE_ID, &registration(vec![log("release")])).is_err(),
        "registering is new work"
    );
    assert_eq!(stx.world.runtime_obligations.iter().count(), 0);
}

#[test]
fn a_committed_instruction_completes_once_whatever_is_restricted() {
    let state = lifecycle_state();
    let mut block = state.block(header());
    let mut stx = block.transaction();
    register(
        &mut stx,
        &ALICE_ID,
        &registration(vec![log("release"), log("refund")]),
    )
    .expect("register");
    for restriction in [
        RestrictionV1::AdmissionClosed(1),
        RestrictionV1::CompletionOnly(1),
    ] {
        restrict_log(&mut stx, restriction);
        assert!(
            !resolves(&mut stx, &ALICE_ID, &log("new work")),
            "an instruction the obligation did not commit to is new work under {restriction:?}"
        );
        assert!(
            !resolves(&mut stx, &BOB_ID, &log("release")),
            "only the owner completes its obligation"
        );
    }
    assert!(resolves(&mut stx, &ALICE_ID, &log("release")));
    assert!(
        stx.completing_obligation.is_none(),
        "the capability ends with its instruction"
    );
    assert!(
        !resolves(&mut stx, &ALICE_ID, &log("release")),
        "a committed instruction lends its capability once"
    );

    // Disabling stops every execution; an activation that would do so lapses instead.
    restrict_log(&mut stx, RestrictionV1::Disabled(1));
    assert!(!resolves(&mut stx, &ALICE_ID, &log("refund")));
    let id = ObligationId::derive(&ALICE_ID).unwrap();
    let record = stx.world.runtime_obligations.get(&id).cloned().unwrap();
    assert_eq!(
        record
            .terminal_effects
            .iter()
            .filter(|effect| effect.performed)
            .count(),
        1,
        "the rejected refund remains pending even if its caller catches the error"
    );
}

#[test]
fn a_change_set_that_alters_a_live_closure_lapses() {
    let state = lifecycle_state();
    let mut block = state.block(header());
    let mut stx = block.transaction();
    let predecessor = stx.world.runtime_selection.get().clone();
    let mut disabled = predecessor.clone();
    disabled.restrictions.push(RestrictionEntryV1 {
        definition: log_interface(),
        restriction: RestrictionV1::Disabled(5),
    });
    disabled.restrictions.sort_by_key(|entry| entry.definition);
    let mut contained = predecessor.clone();
    contained.restrictions.push(RestrictionEntryV1 {
        definition: log_interface(),
        restriction: RestrictionV1::CompletionOnly(5),
    });
    contained.restrictions.sort_by_key(|entry| entry.definition);

    let live = |stx: &StateTransaction<'_, '_>, now_ms| {
        LiveObligations::default().with_registered(&stx.world, now_ms)
    };
    assert_eq!(
        stranded(
            &predecessor,
            &disabled,
            &PublishedDefinitions(&stx.world.runtime_definitions),
            &live(&stx, NOW_MS)
        ),
        None,
        "no obligation is registered"
    );
    register(&mut stx, &ALICE_ID, &registration(vec![log("release")])).expect("register");
    let published = PublishedDefinitions(&stx.world.runtime_definitions);
    let reason = stranded(&predecessor, &disabled, &published, &live(&stx, NOW_MS))
        .expect("disabling a captured definition strands the obligation");
    assert!(reason.contains("registered"), "{reason}");
    assert_eq!(
        stranded(&predecessor, &contained, &published, &live(&stx, NOW_MS)),
        None,
        "completion-only keeps the closure executing"
    );
    assert_eq!(
        stranded(&predecessor, &predecessor, &published, &live(&stx, NOW_MS)),
        None
    );
    assert_eq!(
        stranded(
            &predecessor,
            &disabled,
            &published,
            &live(&stx, NOW_MS + 60_000)
        ),
        None,
        "an expired obligation blocks no change set"
    );
}

#[test]
fn a_completion_only_lends_its_captured_call_graph() {
    use iroha_data_model::isi::account_signing_key::SetAccountSigningKeyV1;

    let state = lifecycle_state();
    crate::runtime::activate_additions_for_testing(
        &state.world,
        crate::runtime::catalog::additions::ACCOUNT_SIGNING_KEYS.module,
    )
    .expect("activate an independent interface");
    let mut block = state.block(header());
    let mut stx = block.transaction();
    let terminal = log("release");
    let unrelated = InstructionBox::from(SetAccountSigningKeyV1::revoke(ALICE_ID.clone()));
    register(&mut stx, &ALICE_ID, &registration(vec![terminal.clone()])).unwrap();
    let binding = stx
        .world
        .runtime_selection
        .get()
        .bindings
        .iter()
        .find(|binding| binding.wire_id == SetAccountSigningKeyV1::WIRE_ID)
        .unwrap()
        .clone();
    let selection = stx.world.runtime_selection.get_mut();
    selection.restrictions.push(RestrictionEntryV1 {
        definition: binding.interface,
        restriction: RestrictionV1::CompletionOnly(1),
    });
    selection.restrictions.sort_by_key(|entry| entry.definition);
    restrict_log(&mut stx, RestrictionV1::CompletionOnly(1));

    performing(&mut stx, &ALICE_ID, &terminal, |stx| {
        assert!(captures_executing(stx, &terminal));
        assert!(resolve_executing(stx, &terminal).is_ok());
        // The baseline bundle declares Log as a nested call. Its captured binding may
        // continue, while an independently activated interface receives no such capability.
        assert!(captures_executing(stx, &log("nested")));
        assert!(resolve_executing(stx, &log("nested")).is_ok());
        assert!(!captures_executing(stx, &unrelated));
        assert!(resolve_executing(stx, &unrelated).is_err());
        performing(stx, &ALICE_ID, &unrelated, |stx| {
            assert!(!captures_executing(stx, &unrelated));
            assert!(resolve_executing(stx, &unrelated).is_err());
            Ok(())
        })?;
        assert!(captures_executing(stx, &terminal));
        Ok(())
    })
    .unwrap();
    assert!(stx.completing_obligation.is_none());
    assert!(!captures_executing(&stx, &terminal));
}

#[test]
fn a_failed_terminal_effect_clears_its_execution_capability() {
    let state = lifecycle_state();
    let mut block = state.block(header());
    let terminal = log("release");
    {
        let mut stx = block.transaction();
        register(&mut stx, &ALICE_ID, &registration(vec![terminal.clone()])).unwrap();
        stx.apply();
    }
    {
        let mut stx = block.transaction();
        let failure = ValidationFail::NotPermitted("terminal effect failed".into());
        let result: Result<(), _> = performing(&mut stx, &ALICE_ID, &terminal, |stx| {
            assert!(captures_executing(stx, &terminal));
            let id = ObligationId::derive(&ALICE_ID).unwrap();
            assert!(
                stx.world
                    .runtime_obligations
                    .get(&id)
                    .unwrap()
                    .is_live(NOW_MS),
                "the registration remains live until its terminal effect succeeds"
            );
            Err(failure.clone())
        });
        assert_eq!(result, Err(failure));
        assert!(stx.completing_obligation.is_none());
        let id = ObligationId::derive(&ALICE_ID).unwrap();
        assert!(
            stx.world
                .runtime_obligations
                .get(&id)
                .unwrap()
                .is_live(NOW_MS)
        );
        // A contract may catch the rejection and apply its surrounding transaction.
        stx.apply();
    }
    let mut stx = block.transaction();
    restrict_log(&mut stx, RestrictionV1::CompletionOnly(1));
    assert!(resolves(&mut stx, &ALICE_ID, &terminal));
    assert!(!resolves(&mut stx, &ALICE_ID, &terminal));
}

#[test]
fn an_unwind_cannot_leak_completion_authority_into_later_instructions() {
    let state = lifecycle_state();
    let mut block = state.block(header());
    let mut stx = block.transaction();
    let terminal = log("release");
    register(&mut stx, &ALICE_ID, &registration(vec![terminal.clone()])).unwrap();
    restrict_log(&mut stx, RestrictionV1::CompletionOnly(1));
    let canonical = stx.world.runtime_selection.canonical().clone();
    let unwind = iroha_panic_hook::catch_unwind_suppressed(std::panic::AssertUnwindSafe(|| {
        let _: Result<(), ValidationFail> = performing(&mut stx, &ALICE_ID, &terminal, |stx| {
            assert!(captures_executing(stx, &terminal));
            panic!("execution owner catches this callback failure");
        });
    }));
    assert!(unwind.is_err());
    assert!(stx.completing_obligation.is_none());
    assert_eq!(stx.world.runtime_selection.get(), &canonical);
    assert!(resolve_executing(&stx, &log("unregistered work")).is_err());
    assert!(resolves(&mut stx, &ALICE_ID, &terminal));
    assert!(!resolves(&mut stx, &ALICE_ID, &terminal));
}

#[test]
fn a_captured_binding_executes_beside_its_replacement_without_mutating_state() {
    use iroha_data_model::{isi::Revoke, isi::RevokeBox, permission::Permission};
    let state = lifecycle_state();
    let mut block = state.block(iroha_data_model::block::BlockHeader::new(
        std::num::NonZeroU64::new(5).unwrap(),
        None,
        None,
        NOW_MS,
        0,
    ));
    let mut stx = block.transaction();
    let terminal = InstructionBox::from(Revoke::account_permission(
        Permission::from(
            iroha_executor_data_model::permission::query::CanReadRestrictedDataspace {
                dataspace: iroha_model_base::topology::DataSpaceId::new(7),
            },
        ),
        ALICE_ID.clone(),
    ));
    register(&mut stx, &ALICE_ID, &registration(vec![terminal.clone()])).unwrap();
    let predecessor = stx.world.runtime_selection.get().clone();
    let old = resolve_executing(&stx, &terminal).unwrap();
    let catalog = release_catalog().unwrap();
    let change = catalog
        .activation(
            crate::runtime::catalog::additions::RESTRICTED_DATASPACE_READ.module,
            &predecessor,
            &PublishedDefinitions(&stx.world.runtime_definitions),
            stx.network_id,
            5,
            [0; 32],
        )
        .unwrap();
    let applied = change
        .apply(
            &predecessor,
            &PublishedDefinitions(&stx.world.runtime_definitions),
        )
        .unwrap();
    for (id, descriptor) in applied.published {
        stx.world.runtime_definitions.insert(id, descriptor);
    }
    *stx.world.runtime_selection.get_mut() = applied.selection.clone();
    let successor = stx.world.runtime_selection.get().clone();
    let new = resolve_executing(&stx, &terminal).unwrap();
    assert_ne!(new, old);
    assert!(
        crate::executor::validate_selected_restricted_read_mutation(&stx, &ALICE_ID, &terminal)
            .is_err()
    );
    let captures = live_captures(&stx.world, NOW_MS);
    assert_eq!(
        closure_gap(
            &predecessor,
            &successor,
            &PublishedDefinitions(&stx.world.runtime_definitions),
            &captures
        ),
        None
    );
    performing(&mut stx, &ALICE_ID, &terminal, |stx| {
        use crate::state::WorldReadOnly as _;
        assert_eq!(resolve_executing(stx, &terminal).unwrap(), old);
        assert!(
            crate::executor::validate_selected_restricted_read_mutation(stx, &ALICE_ID, &terminal)
                .is_ok()
        );
        assert_eq!(
            stx.world
                .runtime_selection()
                .bindings
                .iter()
                .find(|binding| binding.wire_id
                    == iroha_data_model::isi::wire_id_of::<RevokeBox>().unwrap())
                .unwrap()
                .implementation,
            old.unwrap()
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(stx.world.runtime_selection.get(), &successor);
    assert_eq!(resolve_executing(&stx, &terminal).unwrap(), new);
    stx.apply();
    assert_eq!(block.world.runtime_selection.get(), &successor);
}
