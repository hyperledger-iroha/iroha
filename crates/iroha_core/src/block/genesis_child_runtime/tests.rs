//! Original signed-recipe controls and pristine unpublished foundation installation.

use super::*;
use crate::{block::authenticate_genesis_block_intents, state::World};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    isi::InstructionBox,
    runtime::child_admin::ChildRuntimeAdminPolicyV1,
    transaction::{FeePaymentIntent, TransactionBuilder},
};
use iroha_model_base::topology::DataSpaceId;
use iroha_test_samples::{SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR};

fn scope() -> SumeragiRootScope {
    SumeragiRootScope::Dataspace {
        parent_network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"foundation-parent",
        ))),
        dataspace_id: DataSpaceId::new((1_u64 << 42) + 19),
    }
}

fn recipe() -> ChildRuntimeFoundationV1 {
    ChildRuntimeFoundationV1 {
        selection: crate::runtime::release_catalog()
            .unwrap()
            .child_foundation()
            .unwrap()
            .0
            .id()
            .unwrap(),
        policy: ChildRuntimeAdminPolicyV1 {
            revision: 1,
            admins: vec![iroha_test_samples::ALICE_KEYPAIR.public_key().clone()],
            threshold: 1,
            notice: Default::default(),
            provenance: None,
        },
    }
}

fn source(scope: SumeragiRootScope, recipes: &[ChildRuntimeFoundationV1]) -> SignedBlock {
    let instructions: Vec<InstructionBox> = std::iter::once(
        SetParameter::new(crate::sumeragi::lanes::routing::test_support::metadata(
            scope,
        ))
        .into(),
    )
    .chain(recipes.iter().map(|recipe| {
        SetParameter::new(Parameter::Custom(recipe.to_custom_parameter().unwrap())).into()
    }))
    .collect();
    let transactions = instructions
        .into_iter()
        .map(|instruction| {
            TransactionBuilder::new_genesis(
                SAMPLE_GENESIS_ACCOUNT_ID.clone(),
                FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([instruction])
            .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
        })
        .collect();
    SignedBlock::genesis(
        transactions,
        SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key(),
        None,
        None,
    )
    .canonical_resultless_proposal()
    .unwrap()
}

fn state(block: &SignedBlock) -> State {
    let world = World::new();
    State::try_new_with_chain_and_network_id_with_default_telemetry(
        world.sccp_committee_generations.allocation_budget().clone(),
        world,
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
        "child-foundation".into(),
        NetworkId::from_genesis_hash(block.hash()),
    )
    .unwrap()
}

#[test]
fn only_the_unique_original_signed_child_recipe_selects_foundation() {
    let value = recipe();
    let block = source(scope(), std::slice::from_ref(&value));
    let original = authenticate_genesis_block_intents(&block, &SAMPLE_GENESIS_ACCOUNT_ID).unwrap();
    let selected = original.initial_child_runtime(&block).unwrap().unwrap();
    assert_eq!(selected.recipe, value);
    assert_eq!(selected.scope, scope());
    assert!(
        original
            .initial_child_runtime(&source(scope(), &[]))
            .is_err()
    );
    assert!(authenticate_genesis_block_intents(&block, &iroha_test_samples::ALICE_ID).is_err());
    for (scope, recipes, accepted) in [
        (scope(), vec![], true),
        (scope(), vec![value.clone(), value.clone()], false),
        (SumeragiRootScope::Global, vec![value], false),
    ] {
        let block = source(scope, &recipes);
        let original =
            authenticate_genesis_block_intents(&block, &SAMPLE_GENESIS_ACCOUNT_ID).unwrap();
        assert_eq!(original.initial_child_runtime(&block).is_ok(), accepted);
    }
}

#[test]
fn installation_binds_full_child_identity_and_rolls_back_with_failed_genesis() {
    let value = recipe();
    let block = source(scope(), std::slice::from_ref(&value));
    let original = authenticate_genesis_block_intents(&block, &SAMPLE_GENESIS_ACCOUNT_ID).unwrap();
    let selected = original.initial_child_runtime(&block).unwrap().unwrap();
    let state = state(&block);
    let before = crate::snapshot::canonical_state_snapshot_bytes(&state);
    let generation = state.state_view_generation();
    let result = state.block_with_recorded_pristine_carrier_stage(
        &block,
        |overlay| {
            let prepared = selected.prepare_installation(&block, overlay, &state, generation)?;
            assert_eq!(prepared.authority.root.scope(), scope());
            assert_eq!(
                prepared.authority.root.network,
                NetworkId::from_genesis_hash(block.hash())
            );
            assert_eq!(prepared.authority.next_nonce, 0);
            prepared.install(overlay);
            overlay
                .world
                .set_parameter_for_testing(Parameter::Custom(value.to_custom_parameter().unwrap()))
                .unwrap();
            {
                let parameter = value.to_custom_parameter().unwrap();
                let signed = block
                    .external_transactions()
                    .nth(selected.coordinate.0)
                    .unwrap();
                let Executable::Instructions(instructions) = signed.instructions() else {
                    panic!("native original recipe");
                };
                let mut transaction = overlay
                    .transaction_for_original_genesis_testing(
                        &block,
                        selected.coordinate.0,
                        &SAMPLE_GENESIS_ACCOUNT_ID,
                        &instructions[selected.coordinate.1],
                    )
                    .unwrap();
                selected
                    .require_parameter(&transaction, &parameter)
                    .unwrap();
                let original_position = transaction.current_entrypoint_index;
                transaction.current_entrypoint_index = Some(99);
                assert!(
                    selected
                        .require_parameter(&transaction, &parameter)
                        .is_err()
                );
                transaction.current_entrypoint_index = original_position;
                let scope = transaction.genesis_execution_scope.take();
                assert!(
                    selected
                        .require_parameter(&transaction, &parameter)
                        .is_err()
                );
                transaction.genesis_execution_scope = scope;
                let authority = transaction.world.runtime_child_admin_state.get_mut().take();
                assert!(
                    selected
                        .require_parameter(&transaction, &parameter)
                        .is_err()
                );
                *transaction.world.runtime_child_admin_state.get_mut() = authority;
                let replacement = CustomParameter::new(
                    parameter.id().clone(),
                    iroha_primitives::json::Json::new("executor replacement"),
                );
                assert!(
                    selected
                        .require_parameter(&transaction, &replacement)
                        .is_err()
                );
                let mut model = iroha_data_model::executor::ExecutorDataModel::default();
                model.parameters.insert(replacement);
                transaction.world.apply_executor_data_model(model).unwrap();
                assert_eq!(
                    transaction
                        .world
                        .parameters
                        .get()
                        .custom()
                        .get(parameter.id()),
                    Some(&parameter)
                );
                assert!(
                    transaction
                        .world
                        .executor_data_model
                        .get()
                        .parameters()
                        .get(parameter.id())
                        .is_none()
                );
                transaction
                    .world
                    .apply_executor_data_model(Default::default())
                    .unwrap();
                assert_eq!(
                    transaction
                        .world
                        .parameters
                        .get()
                        .custom()
                        .get(parameter.id()),
                    Some(&parameter)
                );
                selected
                    .require_parameter(&transaction, &parameter)
                    .unwrap();
            }
            selected.require_unchanged(overlay)?;
            assert!(
                selected
                    .prepare_installation(&block, overlay, &state, generation)
                    .is_err()
            );
            overlay
                .world
                .runtime_child_admin_state
                .get_mut()
                .as_mut()
                .unwrap()
                .next_nonce = 1;
            assert!(selected.require_unchanged(overlay).is_err());
            Err::<(), BlockValidationError>(invalid("deliberate later genesis refusal"))
        },
        invalid,
    );
    assert!(result.is_err());
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_bytes(&state),
        before
    );
    assert_eq!(state.state_view_generation(), generation);
}

#[test]
fn a_signed_unknown_foundation_is_rejected_before_any_overlay_write() {
    let mut value = recipe();
    value.selection.0[0] ^= 1;
    let block = source(scope(), &[value]);
    let original = authenticate_genesis_block_intents(&block, &SAMPLE_GENESIS_ACCOUNT_ID).unwrap();
    let selected = original.initial_child_runtime(&block).unwrap().unwrap();
    let state = state(&block);
    let generation = state.state_view_generation();
    let result = state.block_with_recorded_pristine_carrier_stage(
        &block,
        |overlay| {
            assert!(
                selected
                    .prepare_installation(&block, overlay, &state, generation)
                    .is_err()
            );
            assert!(overlay.world.runtime_child_admin_state.get().is_none());
            assert_eq!(
                *overlay.world.runtime_selection.get(),
                crate::runtime::release_catalog().unwrap().baseline
            );
            Err::<(), BlockValidationError>(invalid("checked refusal"))
        },
        invalid,
    );
    assert!(result.is_err());
}

#[test]
fn recipe_decoder_refusal_retains_the_original_source_for_retry() {
    let block = source(scope(), &[recipe()]);
    let original = authenticate_genesis_block_intents(&block, &SAMPLE_GENESIS_ACCOUNT_ID).unwrap();
    let zero = norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
        usize::MAX,
        usize::MAX,
        usize::MAX,
        0,
        usize::MAX,
    ));
    let error = zero
        .with(|| original.initial_child_runtime(&block))
        .unwrap_err();
    let BlockValidationError::ExecutionDeferred(refusal) = error else {
        panic!("decoder capacity refusal must remain local");
    };
    assert!(refusal.decode_refusal().is_some());
    assert!(original.initial_child_runtime(&block).unwrap().is_some());
}

#[test]
fn signed_scope_and_selection_encoding_preserve_original_local_refusals() {
    let block = source(scope(), &[recipe()]);
    let selection = crate::runtime::release_catalog()
        .unwrap()
        .child_foundation()
        .unwrap()
        .0;
    assert_eq!(
        selection_identity(&selection).unwrap(),
        selection.id().unwrap()
    );
    assert_eq!(signed_scope(&block).unwrap(), scope());
    let attempts: [&dyn Fn() -> Result<(), BlockValidationError>; 2] =
        [&|| signed_scope(&block).map(|_| ()), &|| {
            selection_identity(&selection).map(|_| ())
        }];
    for attempt in attempts {
        let zero = norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            0,
            usize::MAX,
        ));
        let error = zero.with(attempt).unwrap_err();
        let BlockValidationError::ExecutionDeferred(refusal) = error else {
            panic!("local encoding/metadata refusal must not reject genesis");
        };
        assert!(refusal.decode_refusal().is_some());
        attempt().unwrap();
    }
}

#[test]
fn child_model_boundary_preserves_resource_owner_and_still_rejects_invalid_shape() {
    let parameter = recipe().to_custom_parameter().unwrap();
    let zero = norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
        usize::MAX,
        usize::MAX,
        usize::MAX,
        0,
        usize::MAX,
    ));
    let original = zero
        .with(|| ChildRuntimeFoundationV1::from_custom_parameter(&parameter))
        .unwrap_err();
    let BlockValidationError::ExecutionDeferred(refusal) = child_admin_error(original) else {
        panic!("original child model refusal remains deferred after its scope unwinds");
    };
    assert!(refusal.decode_refusal().is_some());
    assert!(!matches!(
        child_admin_error(ChildAdminErrorV1::Shape("invalid initial policy")),
        BlockValidationError::ExecutionDeferred(_)
    ));
    ChildRuntimeFoundationV1::from_custom_parameter(&parameter).unwrap();
}
