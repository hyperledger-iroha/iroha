//! Recognition, activation and retirement of the committed content and router policies at the
//! level the kernel drives them: governed change sets activated at block start, `SetParameter`
//! executed in blocks, and the values blocks, views and digests read.

use std::num::NonZeroU64;

use iroha_config::parameters::actual::Settlement;
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, PublicKey};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    block::BlockHeader,
    isi::SetParameter,
    parameter::{CustomParameter, Parameter},
    runtime::{
        definition::{DefinitionDescriptorV1, RestrictionEntryV1, RestrictionV1},
        lifecycle::{
            ChangeSetRecordV1, ChangeSetStatusV1, ChangeSetV1, RestrictionKindV1, RootScopeV1,
            RuntimeNoticeV1, SelectionTransitionV1, SupportAttestationV1, committee_digest,
        },
    },
};
use iroha_primitives::json::Json;

use super::*;
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    runtime::{
        change_set::{PublishedDefinitions, activate_due},
        release_catalog,
    },
    smartcontracts::Execute as _,
    state::{State, World},
};

fn network() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([0x21; 32])))
}

fn validators() -> Vec<KeyPair> {
    (1..=4)
        .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519))
        .collect()
}

fn committee() -> Vec<PublicKey> {
    validators()
        .iter()
        .map(|pair| pair.public_key().clone())
        .collect()
}

fn authority() -> AccountId {
    AccountId::new(
        KeyPair::from_seed(vec![0x51; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    )
}

fn state() -> State {
    State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    )
}

/// An overlay commit does not advance the chain, so every block here is a height-one block
/// over the committed World.
fn header() -> BlockHeader {
    BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero height"),
        None,
        None,
        0,
        0,
    )
}

fn content() -> ContentPolicyV1 {
    ContentPolicyV1 {
        max_bundle_bytes: 2_097_152,
        max_files: 64,
        max_path_len: 128,
        max_retention_blocks: 5_000,
        chunk_size_bytes: 4_096,
        default_cache_max_age_secs: 60,
        max_cache_max_age_secs: 120,
        immutable_bundles: false,
        stripe_total_stripes: 4,
        stripe_shards_per_stripe: 16,
        stripe_row_parity_stripes: 1,
    }
}

fn router() -> SettlementRouterPolicyV1 {
    SettlementRouterPolicyV1 {
        twap_window_seconds: 90,
        epsilon_bps: 40,
        buffer_alert_pct: 80,
        buffer_throttle_pct: 30,
        buffer_xor_only_pct: 12,
        buffer_halt_pct: 3,
        buffer_horizon_hours: 48,
    }
}

fn opaque(name: &str, payload: Json) -> CustomParameter {
    CustomParameter::new(name.parse().expect("parameter identifier"), payload)
}

/// Execute `SetParameter` for each of `parameters` in one block and commit it.
fn set(state: &State, parameters: impl IntoIterator<Item = CustomParameter>) -> Result<(), String> {
    let mut block = state.block(header());
    let mut transaction = block.transaction();
    for parameter in parameters {
        SetParameter::new(Parameter::Custom(parameter))
            .execute(&authority(), &mut transaction)
            .map_err(|error| error.to_string())?;
    }
    transaction.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("the overlay commits");
    Ok(())
}

fn stored(state: &State, name: &str) -> Option<CustomParameter> {
    state
        .world
        .parameters
        .view()
        .custom()
        .get(&name.parse().expect("parameter identifier"))
        .cloned()
}

/// Record `change` as scheduled and attested by every validator.
fn schedule(world: &World, change: &ChangeSetV1) {
    let committee = committee();
    let mut record = ChangeSetRecordV1 {
        change_set: change.clone(),
        status: ChangeSetStatusV1::Scheduled,
        proposed_ms: 0,
        approved_height: 1,
        approved_ms: 0,
        notice: RuntimeNoticeV1 {
            blocks: 1,
            ms: 0,
            approval_cushion_ms: 0,
        },
        attestations: Vec::new(),
    };
    for pair in validators() {
        let attestation = SupportAttestationV1 {
            network: change.network,
            root: change.root,
            change_set: change.id().unwrap(),
            predecessor: change.predecessor,
            successor: change.successor,
            committee: committee_digest(&committee).unwrap(),
            tested_checkpoint: 1,
            valid_from: 0,
            valid_until: u64::MAX,
            release: [0x44; 32],
            validator: pair.public_key().clone(),
        }
        .sign(pair.private_key())
        .unwrap();
        crate::runtime::change_set::record_attestation(&mut record, attestation);
    }
    let mut block = world.block();
    block
        .runtime_change_sets
        .insert(change.id().unwrap(), record);
    block.commit();
}

/// Run the block start of `height` on `world` as the kernel does and return the status the
/// change set `change` reached.
fn start_block(world: &World, height: u64, change: &ChangeSetV1) -> ChangeSetStatusV1 {
    let mut block = world.block();
    let mut transaction = block
        .transaction_without_telemetry(iroha_config::parameters::actual::LaneConfig::default(), 0);
    activate_due(&mut transaction, height, 1_000, &committee()).expect("block start");
    transaction.apply();
    block.commit();
    world
        .view()
        .runtime_change_sets()
        .get(&change.id().unwrap())
        .expect("recorded")
        .status
}

/// Draft, schedule and run the activation of both parameters at `height`.
fn activate(state: &State, height: u64) {
    let change = {
        let view = state.world.view();
        crate::runtime::draft_addition(
            &view,
            network(),
            super::super::catalog::parameters::MODULE,
            height,
            [0x31; 32],
        )
        .expect("draft the activation")
    };
    schedule(&state.world, &change);
    assert_eq!(
        start_block(&state.world, height, &change),
        ChangeSetStatusV1::Activated(height)
    );
}

/// The change set that disables and deactivates the content policy definition at `height`.
fn retire_content(state: &State, height: u64) -> ChangeSetV1 {
    let view = state.world.view();
    let current = view.runtime_selection().clone();
    let definition = CONTENT_POLICY.definition_id().unwrap();
    let mut successor = current.clone();
    successor.active.retain(|id| *id != definition);
    let change = ChangeSetV1 {
        network: network(),
        root: RootScopeV1::Universal,
        predecessor: current.id().unwrap(),
        successor: successor.id().unwrap(),
        effective_height: height,
        publish: Vec::new(),
        transitions: vec![
            SelectionTransitionV1::Restrict {
                definition,
                restriction: RestrictionKindV1::Disabled,
            },
            SelectionTransitionV1::Deactivate(definition),
        ],
        qualification: [0x32; 32],
    };
    change
        .apply(&current, &PublishedDefinitions(view.runtime_definitions()))
        .expect("a closed retirement");
    change
}

fn selection(state: &State) -> RootSelectionV1 {
    state.world.view().runtime_selection().clone()
}

#[test]
fn an_unrecognizing_root_stores_either_identifier_opaquely_and_changes_nothing() {
    let state = state();
    let digest = state.execution_policy_digest_v1().unwrap();
    let installed_content = state.content_snapshot();
    let installed_settlement = state.settlement_snapshot();
    assert!(!content_recognized(&selection(&state)));
    assert!(!settlement_router_recognized(&selection(&state)));

    // Valid and malformed values are stored exactly as the frozen baseline stores them.
    set(
        &state,
        [
            opaque(ContentPolicyV1::PARAMETER_ID_STR, Json::new("not a policy")),
            content().into_custom_parameter(),
            opaque(SettlementRouterPolicyV1::PARAMETER_ID_STR, Json::new(7_u32)),
        ],
    )
    .expect("an unrecognized identifier is an ordinary custom parameter");
    assert_eq!(
        stored(&state, ContentPolicyV1::PARAMETER_ID_STR),
        Some(content().into_custom_parameter())
    );
    assert_eq!(
        stored(&state, SettlementRouterPolicyV1::PARAMETER_ID_STR),
        Some(opaque(
            SettlementRouterPolicyV1::PARAMETER_ID_STR,
            Json::new(7_u32)
        ))
    );

    // No reader applies them.
    let view_selection = selection(&state);
    let parameters = state.world.parameters.view();
    assert_eq!(content_policy(&view_selection, parameters.get()), None);
    assert_eq!(
        settlement_router_policy(&view_selection, parameters.get()),
        None
    );
    drop(parameters);
    for name in [
        ContentPolicyV1::PARAMETER_ID_STR,
        SettlementRouterPolicyV1::PARAMETER_ID_STR,
    ] {
        assert!(!consensus_owned(&view_selection, &name.parse().unwrap()));
    }
    assert_eq!(state.execution_policy_digest_v1().unwrap(), digest);
    assert_eq!(
        content_policy_of(&state.content_snapshot()),
        content_policy_of(&installed_content)
    );
    assert_eq!(
        settlement_router_policy_of(&state.settlement_snapshot()),
        settlement_router_policy_of(&installed_settlement)
    );
    let view = state.view();
    assert_eq!(
        content_policy_of(&view.content),
        content_policy_of(&installed_content)
    );
    assert_eq!(
        settlement_router_policy_of(&view.settlement),
        settlement_router_policy_of(&installed_settlement)
    );
    drop(view);
    let block = state.block(header());
    assert_eq!(
        content_policy_of(&block.content),
        content_policy_of(&installed_content)
    );
    assert_eq!(
        settlement_router_policy_of(&block.settlement),
        settlement_router_policy_of(&installed_settlement)
    );
}

#[test]
fn activation_overwrites_the_opaque_value_and_the_next_block_applies_the_committed_policy() {
    let state = state();
    let digest = state.execution_policy_digest_v1().unwrap();
    let installed = state.content_snapshot();
    // Values a holder stored while the identifiers were opaque.
    set(
        &state,
        [
            content().into_custom_parameter(),
            opaque(
                SettlementRouterPolicyV1::PARAMETER_ID_STR,
                Json::new("halt"),
            ),
        ],
    )
    .unwrap();

    activate(&state, 2);
    let current = selection(&state);
    assert!(content_recognized(&current));
    assert!(settlement_router_recognized(&current));
    // Activation wrote the initial values over the opaque ones.
    assert_eq!(
        stored(&state, ContentPolicyV1::PARAMETER_ID_STR),
        Some(ContentPolicyV1::protocol_default().into_custom_parameter())
    );
    assert_eq!(
        stored(&state, SettlementRouterPolicyV1::PARAMETER_ID_STR),
        Some(SettlementRouterPolicyV1::protocol_default().into_custom_parameter())
    );
    // The initial values are the constants, so nothing a chain commits to moves.
    assert_eq!(state.execution_policy_digest_v1().unwrap(), digest);
    assert_eq!(
        content_policy_of(&state.content_snapshot()),
        ContentPolicyV1::protocol_default()
    );
    assert_eq!(
        settlement_router_policy_of(&state.settlement_snapshot()),
        SettlementRouterPolicyV1::protocol_default()
    );
    for name in [
        ContentPolicyV1::PARAMETER_ID_STR,
        SettlementRouterPolicyV1::PARAMETER_ID_STR,
    ] {
        assert!(consensus_owned(&current, &name.parse().unwrap()));
    }

    // A recognized identifier admits only a valid policy.
    let invalid = ContentPolicyV1 {
        chunk_size_bytes: 0,
        ..content()
    };
    assert!(set(&state, [invalid.into_custom_parameter()]).is_err());
    assert!(
        set(
            &state,
            [opaque(
                SettlementRouterPolicyV1::PARAMETER_ID_STR,
                Json::new(7_u32)
            )]
        )
        .is_err()
    );
    assert_eq!(
        stored(&state, ContentPolicyV1::PARAMETER_ID_STR),
        Some(ContentPolicyV1::protocol_default().into_custom_parameter())
    );

    // The block that commits a value still executes under its predecessor's.
    {
        let mut block = state.block(header());
        let mut transaction = block.transaction();
        for parameter in [
            content().into_custom_parameter(),
            router().into_custom_parameter(),
        ] {
            SetParameter::new(Parameter::Custom(parameter))
                .execute(&authority(), &mut transaction)
                .expect("a valid policy is committed");
        }
        transaction.apply();
        assert_eq!(
            content_policy_of(&block.content),
            ContentPolicyV1::protocol_default()
        );
        assert_eq!(
            settlement_router_policy_of(&block.settlement),
            SettlementRouterPolicyV1::protocol_default()
        );
        assert!(
            block
                .settlement_engine
                .matches_router_config(&Settlement::default().router)
        );
        block.commit_world_overlay_for_testing().unwrap();
    }
    // Every later block, view and digest applies the committed value.
    let next = state.block(header());
    assert_eq!(content_policy_of(&next.content), content());
    assert_eq!(settlement_router_policy_of(&next.settlement), router());
    assert!(
        next.settlement_engine
            .matches_router_config(&settlement_of(&router()).router)
    );
    // Gateway resources stay the node's own.
    assert_eq!(next.content.pow.header_name, installed.pow.header_name);
    drop(next);
    let view = state.view();
    assert_eq!(content_policy_of(&view.content), content());
    assert_eq!(settlement_router_policy_of(&view.settlement), router());
    assert!(
        view.settlement_engine
            .matches_router_config(&settlement_of(&router()).router)
    );
    drop(view);
    assert_eq!(content_policy_of(&state.content_snapshot()), content());
    assert_ne!(state.execution_policy_digest_v1().unwrap(), digest);
}

#[test]
fn retirement_waits_for_the_initial_value_and_reactivation_reinstalls_it() {
    let state = state();
    activate(&state, 2);
    set(&state, [content().into_custom_parameter()]).unwrap();

    // A retirement that would leave its own block under a non-default value lapses.
    let early = retire_content(&state, 3);
    schedule(&state.world, &early);
    assert_eq!(
        start_block(&state.world, 3, &early),
        ChangeSetStatusV1::Lapsed(3)
    );
    assert!(content_recognized(&selection(&state)));

    // Governance first sets the value back; then the retirement applies.
    set(
        &state,
        [ContentPolicyV1::protocol_default().into_custom_parameter()],
    )
    .unwrap();
    let retirement = retire_content(&state, 4);
    schedule(&state.world, &retirement);
    assert_eq!(
        start_block(&state.world, 4, &retirement),
        ChangeSetStatusV1::Activated(4)
    );
    let retired = selection(&state);
    assert!(!content_recognized(&retired));
    assert!(settlement_router_recognized(&retired));
    assert!(
        !retired
            .restrictions
            .iter()
            .any(|RestrictionEntryV1 { definition, .. }| {
                *definition == CONTENT_POLICY.definition_id().unwrap()
            }),
        "deactivation consumes the restriction"
    );

    // The identifier is opaque again: any value is stored and none applies.
    set(&state, [content().into_custom_parameter()]).unwrap();
    assert_eq!(
        content_policy_of(&state.content_snapshot()),
        ContentPolicyV1::protocol_default()
    );
    assert!(!consensus_owned(
        &retired,
        &ContentPolicyV1::PARAMETER_ID_STR.parse().unwrap()
    ));

    // Reactivation writes the initial value over the opaque one.
    activate(&state, 5);
    assert_eq!(
        stored(&state, ContentPolicyV1::PARAMETER_ID_STR),
        Some(ContentPolicyV1::protocol_default().into_custom_parameter())
    );
    assert_eq!(
        content_policy_of(&state.block(header()).content),
        ContentPolicyV1::protocol_default()
    );
}

#[test]
fn a_disabled_definition_is_not_recognized_and_other_restrictions_change_nothing() {
    let catalog = release_catalog().unwrap();
    let definition = CONTENT_POLICY.definition_id().unwrap();
    let mut active = catalog.baseline.clone();
    active.active.extend(catalog.parameter_definitions());
    active.active.sort_unstable();
    assert!(content_recognized(&active));
    for (restriction, recognized) in [
        (RestrictionV1::Deprecated(1), true),
        (RestrictionV1::AdmissionClosed(1), true),
        (RestrictionV1::CompletionOnly(1), true),
        (RestrictionV1::Disabled(1), false),
    ] {
        let mut restricted = active.clone();
        restricted.restrictions.push(RestrictionEntryV1 {
            definition,
            restriction,
        });
        restricted
            .restrictions
            .sort_by_key(|entry| entry.definition);
        assert_eq!(
            content_recognized(&restricted),
            recognized,
            "{restriction:?}"
        );
        assert!(settlement_router_recognized(&restricted));
    }
}

#[test]
fn readers_treat_absent_and_malformed_values_of_a_recognizing_root_as_the_default() {
    let catalog = release_catalog().unwrap();
    let mut active = catalog.baseline.clone();
    active.active.extend(catalog.parameter_definitions());
    active.active.sort_unstable();
    let mut parameters = Parameters::default();
    assert_eq!(
        content_policy(&active, &parameters),
        Some(ContentPolicyV1::protocol_default())
    );
    parameters.set_parameter(Parameter::Custom(opaque(
        ContentPolicyV1::PARAMETER_ID_STR,
        Json::new("not a policy"),
    )));
    assert_eq!(
        content_policy(&active, &parameters),
        Some(ContentPolicyV1::protocol_default())
    );
    parameters.set_parameter(Parameter::Custom(router().into_custom_parameter()));
    assert_eq!(
        settlement_router_policy(&active, &parameters),
        Some(router())
    );
    assert_eq!(
        settlement_router_policy_of(&effective_settlement(
            &Settlement::default(),
            &active,
            &parameters
        )),
        router()
    );
    assert_eq!(
        settlement_router_policy_of(&effective_settlement(
            &Settlement::default(),
            &catalog.baseline,
            &parameters
        )),
        settlement_router_policy_of(&Settlement::default())
    );
}

#[test]
fn the_protocol_defaults_are_the_constants_an_unrecognizing_root_applies() {
    // Activation and retirement leave every committed byte as it was only because these agree.
    let state = state();
    assert_eq!(
        content_policy_of(&state.content_snapshot()),
        ContentPolicyV1::protocol_default()
    );
    assert_eq!(
        settlement_router_policy_of(&state.settlement_snapshot()),
        SettlementRouterPolicyV1::protocol_default()
    );
    assert_eq!(
        settlement_router_policy_of(&Settlement::default()),
        SettlementRouterPolicyV1::protocol_default()
    );
    assert_eq!(
        settlement_of(&SettlementRouterPolicyV1::protocol_default())
            .router
            .twap_window,
        Settlement::default().router.twap_window
    );
    let mut installed = state.content_snapshot();
    apply_content(&ContentPolicyV1::protocol_default(), &mut installed);
    assert_eq!(
        content_policy_of(&installed),
        content_policy_of(&state.content_snapshot())
    );
    assert_eq!(
        installed.pow.header_name,
        state.content_snapshot().pow.header_name
    );
    // A policy round-trips through the settings values exactly.
    assert_eq!(
        settlement_router_policy_of(&settlement_of(&router())),
        router()
    );
    let mut changed = state.content_snapshot();
    apply_content(&content(), &mut changed);
    assert_eq!(content_policy_of(&changed), content());
}

#[test]
fn the_parameter_definitions_never_join_the_published_baseline() {
    let state = state();
    let view = state.world.view();
    let published = view.runtime_definitions();
    for definition in release_catalog().unwrap().parameter_definitions() {
        assert!(
            published.get(&definition).is_none(),
            "a founded root publishes only the baseline"
        );
        assert!(matches!(
            release_catalog().unwrap().definitions.get(&definition),
            Some(DefinitionDescriptorV1::Parameter(_))
        ));
    }
}
