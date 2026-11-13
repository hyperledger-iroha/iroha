//! Selected AMX captures over real signed roots, original proofs and funded monetary custody.

use super::*;
use crate::runtime::{
    activate_additions_for_testing,
    catalog::additions::{NATIVE_AMX_CAPTURE, RESTRICTED_DATASPACE_READ},
    native_amx_capture,
};
use iroha_data_model::{
    block::BlockHeader,
    isi::sumeragi_amx::ExpireAmxV1,
    runtime::definition::{DefinitionDescriptorV1, ImplementationCodeV1},
};

fn captured_roots() -> Roots {
    Roots::new_with_start(|config| {
        activate_additions_for_testing(&config.world, NATIVE_AMX_CAPTURE.module).unwrap();
        CertifiedTestChain::start(config).unwrap()
    })
}
fn header(chain: &CertifiedTestChain, time_ms: u64) -> BlockHeader {
    BlockHeader::new(
        std::num::NonZeroU64::new(chain.height() + 1).unwrap(),
        Some(chain.committed(chain.height()).block().hash()),
        None,
        time_ms,
        0,
    )
}
fn rebind(tx: &mut StateTransaction<'_, '_>) {
    let profile = tx.world.runtime_selection.canonical().services_profile;
    let catalog = crate::runtime::release_catalog().unwrap();
    let (id,descriptor)=catalog.definitions.iter().find(|(_,descriptor)|matches!(descriptor,
        DefinitionDescriptorV1::Implementation(implementation)
            if implementation.services_profile==profile
            && matches!(&implementation.code,ImplementationCodeV1::Native{module,..} if module==RESTRICTED_DATASPACE_READ.module)
    )).expect("real compiled permission successor for AMX profile");
    tx.world.runtime_definitions.insert(*id, descriptor.clone());
    let selection = tx.world.runtime_selection.get_mut();
    selection.active.push(*id);
    selection.active.sort();
    selection.active.dedup();
    for binding in &mut selection.bindings {
        if RESTRICTED_DATASPACE_READ
            .rebinds
            .contains(&binding.wire_id.as_str())
        {
            binding.implementation = *id;
        }
    }
    assert_eq!(
        catalog.unsupported(tx.world.runtime_selection.canonical()),
        None
    );
}
fn amount(tx: &StateTransaction<'_, '_>, owner: AccountId) -> Quantity {
    tx.world
        .assets
        .get(&AssetId::with_scope(
            principal(),
            owner,
            AssetBalanceScope::Dataspace(FIRST),
        ))
        .map_or_else(Quantity::zero, |value| value.as_ref().clone())
}

#[test]
fn selected_amx_begin_and_expiry_writer_refusals_leave_original_rows_and_captures_retryable() {
    let roots = captured_roots();
    let transaction = roots.transaction(50, 0xC1);
    let id = transaction.id().unwrap();
    let mut block = roots.global.state().block(header(&roots.global, 3_000));
    let mut tx = block.transaction();
    let original = tx.world.sumeragi_amx().clone();
    let cadence = tx.world.parameters().sumeragi().block_cadence_ms.get();
    let height = tx.block_height();
    let refused = super::super::super::captured::begin_with(
        &mut tx,
        &transaction,
        height,
        3_000,
        cadence,
        |_| Err(AmxError::State("original witness refusal")),
    );
    assert!(refused.is_err());
    assert_eq!(tx.world.sumeragi_amx(), &original);
    assert_eq!(tx.world.runtime_amx_captures.iter().count(), 0);
    super::super::super::captured::begin_with(
        &mut tx,
        &transaction,
        height,
        3_000,
        cadence,
        |_| Ok(()),
    )
    .unwrap();
    let opened = tx.world.sumeragi_amx().clone();
    let capture = tx
        .world
        .runtime_amx_captures
        .iter()
        .next()
        .unwrap()
        .1
        .clone();
    assert_eq!(capture.tx, id);
    native_amx_capture::validate_persistence(&*tx.world).unwrap();
    let due = transaction.deadline + 1;
    assert!(
        super::super::super::captured::expire_with(&mut tx, id, due, 3_001, |_| Err(
            AmxError::State("same terminal witness refusal")
        ))
        .is_err()
    );
    assert_eq!(tx.world.sumeragi_amx(), &opened);
    assert_eq!(
        tx.world.runtime_amx_captures.iter().next().unwrap().1,
        &capture
    );
    super::super::super::captured::expire_with(&mut tx, id, due, 3_001, |_| Ok(())).unwrap();
    assert_eq!(
        tx.world
            .sumeragi_amx()
            .transaction(&id)
            .unwrap()
            .decided
            .unwrap()
            .outcome,
        AmxOutcomeV1::Abort
    );
    assert_eq!(tx.world.runtime_amx_captures.iter().count(), 0);
}

#[test]
fn selected_amx_begin_keeps_the_original_local_codec_refusal_outside_protocol_results() {
    let roots = captured_roots();
    let transaction = roots.transaction(50, 0xD1);
    let original = norito::with_decode_limits_scope(
        norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 64),
        || norito::core::classify_decode_attempt(|| norito::core::reserve_decode_allocation(1)),
    )
    .unwrap_err();
    let expected = original.local_refusal().unwrap();
    {
        let mut block = roots.global.state().block(header(&roots.global, 3_000));
        let mut tx = block.transaction();
        let before = tx.world.sumeragi_amx().clone();
        let cadence = tx.world.parameters().sumeragi().block_cadence_ms.get();
        let height = tx.block_height();
        assert!(
            super::super::super::captured::begin_with(
                &mut tx,
                &transaction,
                height,
                3_000,
                cadence,
                |_| Err(AmxError::Resource(original)),
            )
            .is_err()
        );
        assert_eq!(tx.world.sumeragi_amx(), &before);
        assert_eq!(tx.world.runtime_amx_captures.iter().count(), 0);
        assert_eq!(
            tx.require_storage_admission(),
            Err(crate::state::StateStorageAdmissionError::AmxDecode(
                expected
            ))
        );
    }
    // The failed attempt is discarded, and the same authenticated opening can be retried.
    let mut block = roots.global.state().block(header(&roots.global, 3_000));
    let mut tx = block.transaction();
    let cadence = tx.world.parameters().sumeragi().block_cadence_ms.get();
    let height = tx.block_height();
    super::super::super::captured::begin_with(
        &mut tx,
        &transaction,
        height,
        3_000,
        cadence,
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(tx.world.runtime_amx_captures.iter().count(), 1);
    tx.require_storage_admission().unwrap();
}

#[test]
fn selected_amx_genuine_last_vote_retries_after_witness_refusal_without_losing_its_capture() {
    let mut roots = captured_roots();
    let transaction = roots.transaction(50, 0xC2);
    let begin = roots.begin(&transaction);
    let first = roots.prepare(0, &transaction, &begin);
    let second = roots.prepare(1, &transaction, &begin);
    let mut block = roots.global.state().block(header(&roots.global, 5_000));
    let mut tx = block.transaction();
    let height = tx.block_height();
    super::super::super::captured::relay_with(&mut tx, first.canonical(), height, 5_000, |_| {
        panic!("first Yes does not decide")
    })
    .unwrap();
    let before = tx.world.sumeragi_amx().clone();
    let capture = tx
        .world
        .runtime_amx_captures
        .iter()
        .next()
        .unwrap()
        .1
        .clone();
    assert!(
        super::super::super::captured::relay_with(
            &mut tx,
            second.canonical(),
            height,
            5_000,
            |_| Err(AmxError::State("original last-vote witness refusal"))
        )
        .is_err()
    );
    assert_eq!(tx.world.sumeragi_amx(), &before);
    assert_eq!(
        tx.world.runtime_amx_captures.iter().next().unwrap().1,
        &capture
    );
    super::super::super::captured::relay_with(&mut tx, second.canonical(), height, 5_000, |_| {
        Ok(())
    })
    .unwrap();
    assert_eq!(
        tx.world
            .sumeragi_amx()
            .transaction(&transaction.id().unwrap())
            .unwrap()
            .decided
            .unwrap()
            .outcome,
        AmxOutcomeV1::Commit
    );
    assert_eq!(tx.world.runtime_amx_captures.iter().count(), 0);
}

#[test]
fn selected_amx_funded_settlement_uses_captured_executor_context_after_compiled_rebinding() {
    let mut roots = captured_roots();
    let transaction = roots.transaction(100, 0xC3);
    let begin = roots.begin(&transaction);
    let first = roots.prepare(0, &transaction, &begin);
    let second = roots.prepare(1, &transaction, &begin);
    let decision = roots.decide([first, second], transaction.id().unwrap());
    let chain = &roots.participants[0];
    let budget = chain.state().ivm_execution_budget();
    let instruction = decision
        .try_copy(&budget)
        .unwrap()
        .into_settle(FIRST)
        .complete(&budget)
        .unwrap();
    let mut block = chain.state().block(header(chain, 6_000));
    let mut tx = block.transaction();
    tx.current_dataspace_id = Some(FIRST);
    tx.world.current_dataspace_id = Some(FIRST);
    assert_eq!(amount(&tx, account(&payer())), 900_u32.into());
    let before = tx.world.sumeragi_amx_participant.get().clone();
    let capture = tx
        .world
        .runtime_amx_captures
        .iter()
        .next()
        .unwrap()
        .1
        .clone();
    rebind(&mut tx);
    let settlement_interface = tx
        .world
        .runtime_selection
        .canonical()
        .bindings
        .iter()
        .find(|binding| {
            binding.wire_id == iroha_data_model::isi::wire_id_of::<SettleAmxV1>().unwrap()
        })
        .unwrap()
        .interface;
    tx.world.runtime_selection.get_mut().restrictions.push(
        iroha_data_model::runtime::definition::RestrictionEntryV1 {
            definition: settlement_interface,
            restriction: iroha_data_model::runtime::definition::RestrictionV1::CompletionOnly(1),
        },
    );
    tx.world
        .runtime_selection
        .get_mut()
        .restrictions
        .sort_by_key(|entry| entry.definition);
    let canonical = tx.world.runtime_selection.canonical().clone();
    let raw = instruction.as_any().downcast_ref::<SettleAmxV1>().unwrap();
    assert!(execute_settle_original(raw, &account(&payer()), &mut tx).is_err());
    assert!(std::ptr::eq(
        before.canonical().unwrap(),
        tx.world.sumeragi_amx_participant.get().canonical().unwrap()
    ));
    assert_eq!(
        tx.world.runtime_amx_captures.iter().next().unwrap().1,
        &capture
    );
    let executor = tx.world.executor.clone();
    let mut unowned = SettleAmxV1 {
        dataspace: FIRST,
        decision: decision.canonical().clone(),
    };
    let AmxRecordV1::Decision(ref mut unowned_decision) = unowned.decision.record else {
        panic!("real Decision");
    };
    unowned_decision.tx = [0xFD; 32];
    let unowned: InstructionBox = unowned.into();
    crate::runtime::execution::with_native_obligation(&mut tx, &instruction, |tx| {
        assert_eq!(crate::runtime::execution::native_step(tx, &instruction), Some(crate::runtime::obligations::Step::Complete));
        let original_scope = tx.world.runtime_selection.get().clone();
        let error = native_amx_capture::prepare_execution(tx, &unowned).unwrap_err();
        assert!(matches!(error, Error::Runtime(iroha_data_model::runtime::lifecycle::RuntimeRejectionV1::DefinitionRestricted { definition, .. }) if definition == settlement_interface), "{error}");
        assert_eq!(tx.world.runtime_selection.get(), &original_scope);
        Ok(())
    }).unwrap();
    assert_eq!(tx.world.runtime_selection.get(), &canonical);
    let mut forged = SettleAmxV1 {
        dataspace: FIRST,
        decision: decision.canonical().clone(),
    };
    let AmxRecordV1::Decision(ref mut altered) = forged.decision.record else {
        panic!("real Decision")
    };
    altered.outcome = AmxOutcomeV1::Abort;
    assert!(
        executor
            .execute_instruction(&mut tx, &account(&payer()), &forged.into())
            .is_err()
    );
    assert_eq!(amount(&tx, account(&payer())), 900_u32.into());
    assert_eq!(amount(&tx, account(&receiver())), Quantity::zero());
    assert_eq!(
        tx.world.runtime_amx_captures.iter().next().unwrap().1,
        &capture
    );
    assert!(std::ptr::eq(
        before.canonical().unwrap(),
        tx.world.sumeragi_amx_participant.get().canonical().unwrap()
    ));
    assert_eq!(tx.world.runtime_selection.get(), &canonical);
    executor
        .execute_instruction(&mut tx, &account(&payer()), &instruction)
        .unwrap();
    assert_eq!(amount(&tx, account(&receiver())), 100_u32.into());
    assert_eq!(
        tx.world
            .sumeragi_amx_participant
            .get()
            .canonical()
            .unwrap()
            .escrows[0]
            .settled,
        Some(AmxOutcomeV1::Commit)
    );
    assert_eq!(tx.world.runtime_amx_captures.iter().count(), 0);
    assert_eq!(tx.world.runtime_selection.canonical(), &canonical);
    assert_eq!(tx.world.runtime_selection.get(), &canonical);
}

#[test]
fn selected_amx_rebound_creators_cannot_change_shared_trackers_or_fund_new_custody() {
    let mut roots = captured_roots();
    let transaction = roots.transaction(100, 0xC4);
    let begin = roots.begin(&transaction);
    roots.prepare(0, &transaction, &begin);
    // Begin shares the exact participant trackers with the existing undecided transaction.
    let another = roots.transaction(60, 0xC5);
    {
        let mut block = roots.global.state().block(header(&roots.global, 3_500));
        let mut tx = block.transaction();
        rebind(&mut tx);
        let before = tx.world.sumeragi_amx().clone();
        let capture = tx
            .world
            .runtime_amx_captures
            .iter()
            .next()
            .unwrap()
            .1
            .clone();
        assert!(
            BeginAmxV1 {
                transaction: another
            }
            .execute(&account(&global_config().genesis_key), &mut tx)
            .is_err()
        );
        assert_eq!(tx.world.sumeragi_amx(), &before);
        assert_eq!(
            tx.world.runtime_amx_captures.iter().next().unwrap().1,
            &capture
        );
    }
    // Even a repeated/held Prepare cannot mutate the shared participant tracker from a new context.
    let chain = &roots.participants[0];
    let budget = chain.state().ivm_execution_budget();
    let instruction = begin
        .try_copy(&budget)
        .unwrap()
        .into_prepare(FIRST, &transaction)
        .complete(&budget)
        .unwrap();
    let mut block = chain.state().block(header(chain, 4_500));
    let mut tx = block.transaction();
    rebind(&mut tx);
    let before = tx.world.sumeragi_amx_participant.get().clone();
    let balance = amount(&tx, account(&payer()));
    assert!(
        execute_prepare_original(
            instruction.as_any().downcast_ref().unwrap(),
            &account(&payer()),
            &mut tx
        )
        .is_err()
    );
    assert!(std::ptr::eq(
        before.canonical().unwrap(),
        tx.world.sumeragi_amx_participant.get().canonical().unwrap()
    ));
    assert_eq!(amount(&tx, account(&payer())), balance);
    assert_eq!(tx.world.runtime_amx_captures.iter().count(), 1);
}

#[test]
fn selected_amx_automatic_expiry_prechecks_every_capture_and_preserves_all_on_writer_refusal() {
    let mut roots = captured_roots();
    let first = roots.transaction(20, 0xC6);
    roots.begin(&first);
    let second = roots.transaction(30, 0xC7);
    let signed = roots.global.sign(
        &global_config().genesis_key,
        [BeginAmxV1 {
            transaction: second.clone(),
        }
        .into()],
        3_499,
    );
    assert_eq!(roots.global.commit_at(3_500, vec![signed]), vec![true]);
    let mut block = roots.global.state().block(header(
        &roots.global,
        first.expires_at_ms.max(second.expires_at_ms) + 1,
    ));
    let before = block.world.sumeragi_amx.get().clone();
    let captures: Vec<_> = block
        .world
        .runtime_amx_captures
        .iter()
        .map(|(id, record)| (*id, record.clone()))
        .collect();
    assert_eq!(captures.len(), 2);
    let expiry_interface = captures[0]
        .1
        .capture
        .terminal
        .iter()
        .find(|binding| {
            binding.wire_id == iroha_data_model::isi::wire_id_of::<ExpireAmxV1>().unwrap()
        })
        .unwrap()
        .interface;
    block.world.runtime_selection.get_mut().restrictions.push(
        iroha_data_model::runtime::definition::RestrictionEntryV1 {
            definition: expiry_interface,
            restriction: iroha_data_model::runtime::definition::RestrictionV1::Disabled(1),
        },
    );
    block
        .world
        .runtime_selection
        .get_mut()
        .restrictions
        .sort_by_key(|entry| entry.definition);
    let mut disabled_writes = 0;
    assert!(
        block
            .advance_sumeragi_amx_with(|_| {
                disabled_writes += 1;
                Ok(())
            })
            .is_err()
    );
    assert_eq!(
        disabled_writes, 0,
        "disabled original authority refuses before any witness"
    );
    assert_eq!(block.world.sumeragi_amx.get(), &before);
    for (id, record) in &captures {
        assert_eq!(block.world.runtime_amx_captures.get(id), Some(record));
    }
    block
        .world
        .runtime_selection
        .get_mut()
        .restrictions
        .retain(|entry| entry.definition != expiry_interface);
    let mut wrong = captures[1].1.clone();
    wrong.opening[0] ^= 1;
    block
        .world
        .runtime_amx_captures
        .insert(captures[1].0, wrong);
    let mut writes = 0;
    assert!(
        block
            .advance_sumeragi_amx_with(|_| {
                writes += 1;
                Ok(())
            })
            .is_err()
    );
    assert_eq!(
        writes, 0,
        "all immutable opening identities checked before the first witness"
    );
    assert_eq!(block.world.sumeragi_amx.get(), &before);
    block
        .world
        .runtime_amx_captures
        .insert(captures[1].0, captures[1].1.clone());
    let original = norito::with_decode_limits_scope(
        norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 64),
        || norito::core::classify_decode_attempt(|| norito::core::reserve_decode_allocation(1)),
    )
    .unwrap_err();
    let expected = original.local_refusal().unwrap();
    let mut original = Some(original);
    let rejected = block
        .advance_sumeragi_amx_with(|_| {
            writes += 1;
            if writes == 2 {
                Err(AmxError::Resource(original.take().unwrap()))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    let AmxError::Resource(actual) = rejected else {
        panic!("the finalizer must retain the original codec refusal");
    };
    assert_eq!(actual.local_refusal(), Some(expected));
    assert_eq!(writes, 2);
    assert_eq!(block.world.sumeragi_amx.get(), &before);
    for (id, record) in &captures {
        assert_eq!(block.world.runtime_amx_captures.get(id), Some(record));
    }
    assert_eq!(block.advance_sumeragi_amx_with(|_| Ok(())).unwrap(), 2);
    assert!(block.world.sumeragi_amx.get().transactions.is_empty());
    assert_eq!(block.world.runtime_amx_captures.iter().count(), 0);
}

#[test]
fn selected_amx_restore_and_shared_cohorts_refuse_missing_or_substituted_original_contexts() {
    use iroha_data_model::runtime::obligation::CompletionCaptureV1;
    let mut roots = captured_roots();
    let first = roots.transaction(20, 0xC8);
    roots.begin(&first);
    let second = roots.transaction(30, 0xC9);
    let signed = roots.global.sign(
        &global_config().genesis_key,
        [BeginAmxV1 {
            transaction: second,
        }
        .into()],
        3_499,
    );
    assert_eq!(roots.global.commit_at(3_500, vec![signed]), vec![true]);
    let mut block = roots.global.state().block(header(&roots.global, 4_000));
    let mut tx = block.transaction();
    native_amx_capture::validate_persistence(&*tx.world).unwrap();
    let captures: Vec<_> = tx
        .world
        .runtime_amx_captures
        .iter()
        .map(|(id, record)| (*id, record.clone()))
        .collect();
    tx.world.runtime_amx_captures.remove(captures[0].0);
    assert!(native_amx_capture::validate_persistence(&*tx.world).is_err());
    assert!(native_amx_capture::guard_global_handoff(&mut tx, FIRST).is_err());
    tx.world
        .runtime_amx_captures
        .insert(captures[0].0, captures[0].1.clone());
    let before = tx.world.sumeragi_amx().clone();
    rebind(&mut tx);
    // Deliberately corrupt one side-table context in this negative component control.
    // Real constructors refuse the same mixed cohort before any certificate or custody exists.
    let mut wrong = captures[1].1.clone();
    wrong.capture = CompletionCaptureV1::capture(
        tx.world.runtime_selection.canonical(),
        &crate::runtime::change_set::PublishedDefinitions(&tx.world.runtime_definitions),
        &wrong
            .capture
            .terminal
            .iter()
            .map(|binding| binding.wire_id.clone())
            .collect::<Vec<_>>(),
        tx.block_height(),
    )
    .unwrap();
    tx.world.runtime_amx_captures.insert(captures[1].0, wrong);
    assert!(native_amx_capture::validate_persistence(&*tx.world).is_err());
    assert!(native_amx_capture::guard_global_handoff(&mut tx, FIRST).is_err());
    assert_eq!(tx.world.sumeragi_amx(), &before);
}

#[test]
fn selected_native_amx_capture_does_not_retain_an_unrelated_ivm_host_profile() {
    use crate::runtime::obligations::{LiveObligations, stranded};
    let mut roots = captured_roots();
    let transaction = roots.transaction(50, 0xCA);
    roots.begin(&transaction);
    let mut block = roots.global.state().block(header(&roots.global, 4_000));
    let mut tx = block.transaction();
    let before = tx.world.runtime_selection.canonical().clone();
    let capture = tx
        .world
        .runtime_amx_captures
        .iter()
        .next()
        .unwrap()
        .1
        .capture
        .clone();
    assert!(capture.host_profiles.is_empty());
    let lookup = crate::runtime::change_set::PublishedDefinitions(&tx.world.runtime_definitions);
    let closure = capture.closure(&lookup).unwrap();
    let mut after = before.clone();
    let mut removed = 0;
    after.active.retain(|id| {
        if matches!(
            tx.world.runtime_definitions.get(id),
            Some(DefinitionDescriptorV1::HostProfile(_))
        ) {
            assert!(
                !closure.contains(id),
                "native completion has no transitive host dependency"
            );
            removed += 1;
            false
        } else {
            true
        }
    });
    assert!(removed > 0);
    after.check_closure(&lookup).unwrap();
    assert_eq!(
        stranded(&before, &after, &lookup, &LiveObligations::of(&tx.world)),
        None
    );
    *tx.world.runtime_selection.get_mut() = after;
    native_amx_capture::validate_persistence(&*tx.world).unwrap();
    super::super::super::captured::expire_with(
        &mut tx,
        transaction.id().unwrap(),
        transaction.deadline + 1,
        4_000,
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(tx.world.runtime_amx_captures.iter().count(), 0);
}

#[test]
fn native_amx_activation_drains_legacy_decisions_and_yes_custody_without_synthesizing_captures() {
    let mut roots = Roots::new();
    let transaction = roots.transaction(50, 0xCB);
    let begin = roots.begin(&transaction);
    roots.prepare(0, &transaction, &begin);
    let catalog = crate::runtime::release_catalog().unwrap();
    for chain in [&roots.global, &roots.participants[0]] {
        let mut block = chain.state().block(header(chain, 5_000));
        let tx = block.transaction();
        let before = tx.world.runtime_selection.canonical();
        let change = catalog
            .activation(
                NATIVE_AMX_CAPTURE.module,
                before,
                &catalog.definitions,
                chain.network_id(),
                tx.block_height(),
                [0; 32],
            )
            .unwrap();
        let after = change
            .apply(before, &catalog.definitions)
            .unwrap()
            .selection;
        let error = native_amx_capture::check_transition(&*tx.world, before, &after).unwrap_err();
        assert!(error.contains("drain"), "{error}");
        assert_eq!(tx.world.runtime_amx_captures.iter().count(), 0);
        assert!(!native_amx_capture::enabled(before).unwrap());
    }
}
