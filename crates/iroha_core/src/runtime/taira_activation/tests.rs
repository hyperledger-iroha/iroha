//! The certified activation stays exact, bounded and exclusive to its one predecessor.

use std::collections::BTreeMap;

use iroha_crypto::{Hash, HashOf, Signature};
use iroha_data_model::{
    Registrable as _,
    asset::{AssetBalancePolicy, AssetBalanceScope, AssetDefinition, AssetDefinitionId},
    domain::Domain,
    isi::privacy::BootstrapPrivacyIronwoodPoolV1,
    privacy::PrivacyPoolIdV1,
    runtime::{
        definition::{DefinitionDescriptorV1, DefinitionId, RootSelectionV1},
        lifecycle::{
            ChangeSetV1, RuntimeNoticeV1, RuntimeRejectionV1, SupportAttestationV1,
            committee_digest,
        },
        taira_activation::{
            TAIRA_ACTIVATION_SIGNATURE_DOMAIN_V1, TAIRA_ACTIVATION_VERSION_V1,
            TairaActivationPlanV1,
        },
    },
    transaction::SignedTransaction,
};

use iroha_model_base::domain::DomainId;

use super::*;
use crate::{
    runtime::{catalog::ReleaseCatalog, ironwood, lifecycle_activation},
    smartcontracts::isi::{
        sorafs_bootstrap::tests::{Fixture, account},
        sorafs_provider_admission::test_fixture::key,
    },
    state::{State, world_projection::world_state_accumulator::selection_schema},
    sumeragi::test_chain::fixture_keys,
};

/// Taira's selection as read from its authenticated runtime route and replayed State at H12509.
const LIVE_REPAIRED_SELECTION: &str =
    "f4f73c1261d087963bdfdd045b09551e9dbefbc889b9e735b62f72d9e49e07bb";

fn network() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([0x25; 32])))
}

fn baseline_definitions(
    catalog: &ReleaseCatalog,
) -> BTreeMap<DefinitionId, DefinitionDescriptorV1> {
    catalog
        .baseline_definitions
        .iter()
        .map(|id| (*id, catalog.definitions[id].clone()))
        .collect()
}

/// The compiled repair successor and everything a chain has published once it is selected.
fn repaired(
    catalog: &ReleaseCatalog,
) -> (
    RootSelectionV1,
    BTreeMap<DefinitionId, DefinitionDescriptorV1>,
) {
    let mut published = baseline_definitions(catalog);
    let applied = catalog
        .sorafs_repair_activation(&catalog.baseline, &published, network(), 2, [0x26; 32])
        .expect("repair successor")
        .apply(&catalog.baseline, &published)
        .expect("apply repair");
    published.extend(applied.published);
    (applied.selection, published)
}

fn change_set(
    previous: &RootSelectionV1,
    published: &BTreeMap<DefinitionId, DefinitionDescriptorV1>,
) -> ChangeSetV1 {
    ironwood::taira_time_ironwood_change_set(network(), 100, [0x27; 32], previous, published)
        .expect("exact composition")
}

#[test]
fn this_release_reconstructs_the_selection_taira_runs() {
    let catalog = crate::runtime::release_catalog().expect("catalog");
    let (selection, _) = repaired(catalog);
    assert_eq!(
        hex::encode(selection.id().expect("selection id").0),
        LIVE_REPAIRED_SELECTION,
    );
}

#[test]
fn the_successor_is_the_compiled_composition_with_the_repair_kernel() {
    let catalog = crate::runtime::release_catalog().expect("catalog");
    let (previous, published) = repaired(catalog);
    let change = change_set(&previous, &published);
    let applied = change.apply(&previous, &published).expect("change applies");
    let successor = &applied.selection;
    assert_eq!(change.predecessor, previous.id().unwrap());
    assert_eq!(change.successor, successor.id().unwrap());

    let extension = catalog.ironwood.as_ref().expect("Ironwood");
    let mut builder = catalog.clone();
    let combined = builder
        .lifecycle_successor(&extension.selection)
        .expect("combined composition");
    assert_eq!(successor.kernel, previous.kernel);
    assert_ne!(successor.kernel, combined.kernel);
    assert_eq!(successor.commitment_profile, combined.commitment_profile);
    assert_eq!(successor.services_profile, combined.services_profile);
    assert_eq!(successor.installed_storage, combined.installed_storage);
    assert_eq!(successor.active, combined.active);
    assert_eq!(successor.bindings, combined.bindings);
    assert!(successor.restrictions.is_empty());
    assert!(successor.completion_closures.is_empty());
    assert!(successor.bindings.iter().all(|binding| binding.wire_id
        != ScheduleTairaTimeIronwoodV1::WIRE_ID
        && binding.wire_id != iroha_data_model::isi::sorafs_repair::ApplySorafsRepairV1::WIRE_ID),);
    assert_eq!(catalog.unsupported(successor), None);
    assert_eq!(
        crate::runtime::trigger_lifecycle_enabled(successor),
        Ok(true)
    );
    let mut definitions = published.clone();
    definitions.extend(applied.published.clone());
    successor
        .check_closure(&definitions)
        .expect("complete successor closure");
    assert!(ironwood::supports_identity_cutover(
        &previous,
        successor,
        &published,
        &applied.published,
    ));
    assert!(
        definitions.contains_key(&extension.migration),
        "the cutover certificate is published",
    );
}

#[test]
fn readers_keep_their_world_schema_and_supported_pair() {
    let catalog = crate::runtime::release_catalog().expect("catalog");
    let (previous, published) = repaired(catalog);
    let successor = change_set(&previous, &published)
        .apply(&previous, &published)
        .expect("change applies")
        .selection;
    let extension = catalog.ironwood.as_ref().expect("Ironwood");
    let schema = selection_schema(&successor).expect("schema");
    assert_eq!(
        schema,
        selection_schema(&extension.selection).expect("Ironwood schema"),
    );
    assert!(
        State::native_world_schema_hashes_v1()
            .expect("family")
            .contains(&schema),
    );
    assert_eq!(
        selection_schema(&previous).expect("repaired schema"),
        selection_schema(&catalog.baseline).expect("baseline schema"),
    );
    let mut builder = catalog.clone();
    let combined = builder
        .lifecycle_successor(&extension.selection)
        .expect("combined composition");
    assert_eq!(
        (
            successor.services_profile,
            successor.bindings[0].implementation
        ),
        (
            combined.services_profile,
            combined.bindings[0].implementation
        ),
    );
}

#[test]
fn only_the_exact_repaired_selection_is_a_predecessor() {
    let catalog = crate::runtime::release_catalog().expect("catalog");
    let (repaired_selection, published) = repaired(catalog);
    let extension = catalog.ironwood.as_ref().expect("Ironwood");
    for other in [&catalog.baseline, &extension.selection, &catalog.lifecycle] {
        assert!(
            ironwood::taira_time_ironwood_change_set(
                network(),
                100,
                [0x27; 32],
                other,
                &published,
            )
            .is_err()
        );
    }
    let successor = change_set(&repaired_selection, &published)
        .apply(&repaired_selection, &published)
        .expect("change applies")
        .selection;
    assert!(
        ironwood::taira_time_ironwood_change_set(
            network(),
            100,
            [0x27; 32],
            &successor,
            &published,
        )
        .is_err(),
        "the activation cannot be repeated from its own successor",
    );
}

/// A certified chain standing where Taira stands: repaired, with the retained repair certificate.
fn repaired_chain() -> Fixture {
    let domain = Domain::new(DomainId::try_new("privacy", "universal").unwrap()).build(&account(1));
    let definition = AssetDefinition::numeric(
        asset_definition(),
        "Ironwood Core fixture",
        AssetBalancePolicy::Global,
        None,
    )
    .build(&account(1));
    let mut fixture = Fixture::with_world(crate::state::World::with([domain], [], [definition]));
    let payload = fixture.payload();
    let certificate = fixture.certificate(&payload);
    assert_eq!(fixture.submit(certificate), None);
    fixture
}

fn pool() -> PrivacyIronwoodPoolBootstrapV1 {
    PrivacyIronwoodPoolBootstrapV1::new(
        PrivacyPoolIdV1::new([0x41; 32]),
        asset_definition(),
        AssetBalanceScope::Global,
        account(1),
    )
    .expect("pool")
}

fn asset_definition() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("privacy", "universal").unwrap(),
        "ironwood_core".parse().unwrap(),
    )
}

fn plan(fixture: &Fixture) -> TairaActivationPlanV1 {
    let height = fixture.chain.height() + 5;
    let view = fixture.chain.state().view();
    let change_set = ironwood::taira_time_ironwood_change_set(
        fixture.chain.network_id(),
        height,
        [0x42; 32],
        view.world().runtime_selection(),
        &PublishedDefinitions(view.world().runtime_definitions()),
    )
    .expect("exact change set");
    TairaActivationPlanV1 {
        version: TAIRA_ACTIVATION_VERSION_V1,
        chain_id: view.chain_id.clone(),
        network_id: fixture.chain.network_id(),
        operation_id: [0x43; 32],
        change_set,
        notice: RuntimeNoticeV1 {
            blocks: 1,
            ms: 0,
            approval_cushion_ms: 0,
        },
        committee: crate::state::threshold_key_lifecycle_roster_v1(
            view.world(),
            fixture.chain.height() + 1,
        )
        .expect("roster")
        .iter()
        .map(|peer| peer.public_key().clone())
        .collect::<Vec<_>>()
        .try_into()
        .expect("four validators"),
        expires_at_unix_ms: u64::MAX,
        authorization_digest: AUTHORIZATION_DIGEST,
        release_digest: [0x44; 32],
        source_closure_digest: [0x45; 32],
        pool: pool(),
    }
}

fn sign(plan: TairaActivationPlanV1) -> TairaActivationCertificateV1 {
    let frame = norito::encode_canonical(&plan).expect("plan frame");
    let bytes = [TAIRA_ACTIVATION_SIGNATURE_DOMAIN_V1, frame.as_slice()].concat();
    let signatures = std::array::from_fn(|index| {
        let signer = fixture_keys()
            .into_iter()
            .find(|key| key.public_key() == &plan.committee[index])
            .expect("fixture validator");
        Signature::new(signer.private_key(), &bytes)
    });
    TairaActivationCertificateV1 { plan, signatures }
}

/// Open the next block's transaction, as far as this fixture needs one.
fn with_transaction<T>(
    fixture: &mut Fixture,
    body: impl FnOnce(&mut StateTransaction<'_, '_>) -> T,
) -> T {
    use iroha_data_model::block::BlockHeader;
    let time = fixture.next_time_ms();
    let state = std::sync::Arc::clone(fixture.chain.state());
    let view = state.view();
    let header = BlockHeader::new(
        std::num::NonZeroU64::new(fixture.chain.height() + 1).unwrap(),
        view.latest_block_hash(),
        None,
        time,
        0,
    );
    drop(view);
    let mut block = state.block(header);
    let mut tx = block.transaction();
    let result = body(&mut tx);
    tx.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("fixture World effects publish");
    fixture.chain.commit_at(time, Vec::new());
    result
}

fn validate(
    fixture: &Fixture,
    certificate: &TairaActivationCertificateV1,
) -> Result<[u8; 32], ValidationFail> {
    let view = fixture.chain.state().view();
    let height = fixture.chain.height() + 1;
    let committee: Vec<_> = crate::state::threshold_key_lifecycle_roster_v1(view.world(), height)
        .expect("roster")
        .iter()
        .map(|peer| peer.public_key().clone())
        .collect();
    validate_fixture_boundary(
        certificate,
        view.world(),
        &view.chain_id,
        fixture.chain.network_id(),
        height,
        1,
        &committee,
    )
}

/// Execute the handler the way the signed boundary does after it validated the certificate.
fn schedule(
    fixture: &mut Fixture,
    certificate: TairaActivationCertificateV1,
) -> Result<(), InstructionExecutionError> {
    let id = validate(fixture, &certificate).expect("valid boundary");
    with_transaction(fixture, |tx| {
        tx.taira_activation_entrypoint = Some(id);
        (ScheduleTairaTimeIronwoodV1 { certificate }).execute(&account(6), tx)
    })
}

fn attest_all(fixture: &mut Fixture, change: &ChangeSetV1) {
    let id = change.id().unwrap();
    let committee: Vec<_> = fixture
        .chain
        .validators()
        .iter()
        .map(|(peer, _)| peer.public_key().clone())
        .collect();
    let height = fixture.chain.height();
    let time = fixture.next_time_ms();
    let view = fixture.chain.state().view();
    let mut record = view
        .world()
        .runtime_change_sets()
        .get(&id)
        .cloned()
        .expect("scheduled record");
    drop(view);
    for key in fixture_keys() {
        let signed = SupportAttestationV1 {
            network: change.network,
            root: change.root,
            change_set: id,
            predecessor: change.predecessor,
            successor: change.successor,
            committee: committee_digest(&committee).unwrap(),
            tested_checkpoint: height,
            valid_from: height,
            valid_until: change.effective_height + 1,
            release: [0x46; 32],
            validator: key.public_key().clone(),
        }
        .sign(key.private_key())
        .unwrap();
        crate::runtime::change_set::record_attestation(&mut record, signed);
    }
    fixture.chain.setup_world_at(time, |tx| {
        tx.world.runtime_change_sets.insert(id, record);
    });
}

#[test]
fn the_boundary_refuses_every_changed_context() {
    let fixture = repaired_chain();
    let good = plan(&fixture);
    validate(&fixture, &sign(good.clone())).expect("the exact plan is valid");
    let refused = |plan: TairaActivationPlanV1| {
        assert!(validate(&fixture, &sign(plan)).is_err());
    };
    refused(TairaActivationPlanV1 {
        authorization_digest: [0x99; 32],
        ..good.clone()
    });
    refused(TairaActivationPlanV1 {
        chain_id: ChainId::from("another-chain"),
        ..good.clone()
    });
    refused(TairaActivationPlanV1 {
        expires_at_unix_ms: 0,
        ..good.clone()
    });
    let mut reordered = good.clone();
    reordered.committee.swap(0, 1);
    refused(reordered);
    let mut late = good.clone();
    late.notice.blocks = 100;
    refused(late);
    let mut smaller = good.clone();
    smaller.change_set.publish.pop();
    refused(smaller);
    let mut extra = good.clone();
    extra.change_set.qualification = [0x99; 32];
    assert!(
        validate(&fixture, &sign(extra.clone())).is_ok(),
        "qualification is the plan's own evidence commitment",
    );
    extra.change_set.transitions.pop();
    refused(extra);
}

#[test]
fn the_deployment_check_refuses_every_other_network() {
    let fixture = repaired_chain();
    let certificate = sign(plan(&fixture));
    let view = fixture.chain.state().view();
    let height = fixture.chain.height() + 1;
    let committee = certificate.plan.committee.to_vec();
    let error = validate_boundary(
        &certificate,
        view.world(),
        &ChainId::from("a-synthetic-chain"),
        fixture.chain.network_id(),
        height,
        1,
        &committee,
    )
    .expect_err("a synthetic chain is not Taira");
    assert!(format!("{error:?}").contains("exact Taira network"));
}

#[test]
fn the_handler_refuses_without_the_certified_boundary() {
    let mut fixture = repaired_chain();
    let certificate = sign(plan(&fixture));
    let result = with_transaction(&mut fixture, |tx| {
        (ScheduleTairaTimeIronwoodV1 { certificate }).execute(&account(6), tx)
    });
    assert!(result.is_err());
    let view = fixture.chain.state().view();
    assert_eq!(view.world().runtime_change_sets().iter().count(), 0);
    assert!(retained_certificate(view.world()).unwrap().is_none());
}

#[test]
fn certified_activation_applies_at_a_block_start_and_authorizes_one_pool() {
    let mut fixture = repaired_chain();
    let view = fixture.chain.state().view();
    let policy = lifecycle_activation::committed_policy(
        view.world(),
        fixture.chain.network_id(),
        fixture.chain.height(),
    )
    .expect("the repair's retained certificate is the provenance of its one step");
    assert!(!policy.active);
    assert_eq!(policy.activation, None);
    drop(view);

    let certificate = sign(plan(&fixture));
    let change = certificate.plan.change_set.clone();
    let id = change.id().unwrap();
    let predecessor = change.predecessor;
    schedule(&mut fixture, certificate.clone()).expect("scheduled");

    // Scheduling selects nothing, and the plan cannot be scheduled twice.
    let view = fixture.chain.state().view();
    assert_eq!(view.world().runtime_selection().id().unwrap(), predecessor);
    assert_eq!(
        view.world().runtime_change_sets().get(&id).unwrap().status,
        ChangeSetStatusV1::Scheduled,
    );
    assert!(retained_certificate(view.world()).unwrap().is_some());
    drop(view);
    assert!(validate(&fixture, &certificate).is_err());

    // Before activation the certificate grants the pool nothing.
    let refused = with_transaction(&mut fixture, |tx| !certified_pool_bootstrap(tx, &pool()));
    assert!(refused);

    attest_all(&mut fixture, &change);
    while fixture.chain.height() < change.effective_height {
        let time = fixture.next_time_ms();
        fixture.chain.commit_at(time, Vec::new());
    }
    let view = fixture.chain.state().view();
    assert_eq!(
        view.world().runtime_selection().id().unwrap(),
        change.successor,
    );
    assert_eq!(
        view.world().runtime_change_sets().get(&id).unwrap().status,
        ChangeSetStatusV1::Activated(change.effective_height),
    );
    let policy = lifecycle_activation::committed_policy(
        view.world(),
        fixture.chain.network_id(),
        fixture.chain.height(),
    )
    .expect("complete lineage through the repair");
    assert!(policy.active);
    assert_eq!(
        policy.activation.map(|(id, height)| (id, height.get())),
        Some((id, change.effective_height)),
    );
    drop(view);
    fixture
        .chain
        .state()
        .verify_world_state_accumulator()
        .expect("cold capture authenticates the activated World");

    // The exact pool needs no governance permission; any other still does; it happens once.
    let different = PrivacyIronwoodPoolBootstrapV1::new(
        PrivacyPoolIdV1::new([0x51; 32]),
        asset_definition(),
        AssetBalanceScope::Global,
        account(1),
    )
    .expect("pool");
    let result = with_transaction(&mut fixture, |tx| {
        BootstrapPrivacyIronwoodPoolV1::new(different).execute(&account(6), tx)
    });
    assert!(format!("{result:?}").contains("CanEnactGovernance"));
    with_transaction(&mut fixture, |tx| {
        BootstrapPrivacyIronwoodPoolV1::new(pool()).execute(&account(6), tx)
    })
    .expect("the certified pool bootstraps");
    let repeated = with_transaction(&mut fixture, |tx| {
        BootstrapPrivacyIronwoodPoolV1::new(pool()).execute(&account(6), tx)
    });
    assert!(format!("{repeated:?}").contains("already initialized"));
}

#[test]
fn only_a_lapsed_attempt_accepts_a_new_certificate_and_the_selection_changes_once() {
    let mut fixture = repaired_chain();
    let first = sign(plan(&fixture));
    let first_id = first.plan.change_set.id().unwrap();
    let predecessor = first.plan.change_set.predecessor;
    schedule(&mut fixture, first.clone()).expect("scheduled");

    // While the first attempt is scheduled nothing else can start, whatever its height.
    let mut competing = first.plan.clone();
    competing.change_set = ironwood::taira_time_ironwood_change_set(
        fixture.chain.network_id(),
        first.plan.change_set.effective_height + 1,
        first.plan.change_set.qualification,
        fixture.chain.state().view().world().runtime_selection(),
        &PublishedDefinitions(fixture.chain.state().view().world().runtime_definitions()),
    )
    .expect("exact change set");
    let refused = validate(&fixture, &sign(competing)).expect_err("first attempt is scheduled");
    assert!(format!("{refused:?}").contains("already scheduled"));

    // Without attestations its height passes and it lapses, selecting nothing.
    while fixture.chain.height() < first.plan.change_set.effective_height {
        let time = fixture.next_time_ms();
        fixture.chain.commit_at(time, Vec::new());
    }
    {
        let view = fixture.chain.state().view();
        assert_eq!(
            view.world()
                .runtime_change_sets()
                .get(&first_id)
                .unwrap()
                .status,
            ChangeSetStatusV1::Lapsed(first.plan.change_set.effective_height),
        );
        assert_eq!(view.world().runtime_selection().id().unwrap(), predecessor);
    }
    assert!(with_transaction(&mut fixture, |tx| {
        !certified_pool_bootstrap(tx, &pool())
    }));

    // Its own certificate stays refused: its height has passed.
    assert!(validate(&fixture, &first).is_err());

    // A new certificate replaces the receipt and activates through the same checks.
    let second = sign(plan(&fixture));
    let second_id = second.plan.change_set.id().unwrap();
    assert_ne!(second_id, first_id);
    schedule(&mut fixture, second.clone()).expect("a lapse leaves the activation open");
    {
        let view = fixture.chain.state().view();
        assert_eq!(
            retained_certificate(view.world()).unwrap().as_ref(),
            Some(&second),
        );
    }
    attest_all(&mut fixture, &second.plan.change_set);
    while fixture.chain.height() < second.plan.change_set.effective_height {
        let time = fixture.next_time_ms();
        fixture.chain.commit_at(time, Vec::new());
    }
    {
        let view = fixture.chain.state().view();
        assert_eq!(
            view.world()
                .runtime_change_sets()
                .get(&second_id)
                .unwrap()
                .status,
            ChangeSetStatusV1::Activated(second.plan.change_set.effective_height),
        );
        assert_eq!(
            view.world().runtime_selection().id().unwrap(),
            second.plan.change_set.successor,
        );
        let policy = lifecycle_activation::committed_policy(
            view.world(),
            fixture.chain.network_id(),
            fixture.chain.height(),
        )
        .expect("a lapsed attempt leaves no mark on the lineage");
        assert_eq!(
            policy.activation.map(|(id, _)| id),
            Some(second_id),
            "the lineage follows only the activated change set",
        );
    }
    assert!(with_transaction(&mut fixture, |tx| {
        certified_pool_bootstrap(tx, &pool())
    }));

    // Activation ends the authorization, before the selection check would refuse anything.
    let mut third = second.plan.clone();
    third.change_set.effective_height = fixture.chain.height() + 5;
    let refused = validate(&fixture, &sign(third)).expect_err("the activation already happened");
    assert!(format!("{refused:?}").contains("already scheduled"));
}

#[test]
fn the_rehearsal_activates_in_a_discarded_overlay_and_changes_nothing() {
    let fixture = repaired_chain();
    let state = fixture.chain.state();
    let tip = state.view().native_execution_tip().unwrap();
    let before = state.verify_world_state_accumulator().unwrap();

    let report = state.rehearse_taira_activation(pool()).expect("rehearsal");
    assert!(report.activated, "{}", report.status);
    assert_eq!(report.predecessor, LIVE_REPAIRED_SELECTION);
    assert_eq!(report.resulting_kernel, report.predecessor_kernel);
    assert!(report.lifecycle_enabled);
    assert!(
        report.lifecycle_lineage.starts_with("active=true"),
        "{}",
        report.lifecycle_lineage,
    );
    assert!(report.published_definitions > 0);
    assert_eq!(report.published_digest.len(), 64);
    assert_eq!(report.lane_policy_schedule, "admitted");
    assert!(
        report
            .lane_policy_attestation_unscheduled
            .contains("requires a governance module"),
        "{}",
        report.lane_policy_attestation_unscheduled,
    );
    assert_eq!(report.lane_policy_attestation_scheduled, "admitted");
    assert_eq!(report.lane_policy_pool, "admitted");
    assert_eq!(report.trigger_capture_error, None);
    assert!(report.trigger_capture.is_some());
    assert!(!report.reserve_registered_in_overlay);
    assert_eq!(report.pool_bootstrap, "bootstrapped");
    assert_ne!(report.bootstrapped_world, report.activated_world);
    assert_eq!(report.committed_world_after, report.committed_world_before);
    assert!(!report.state_published);

    // A reserve the chain has not registered yet is registered in the same discarded overlay.
    let unregistered = PrivacyIronwoodPoolBootstrapV1::new(
        PrivacyPoolIdV1::new([0x41; 32]),
        asset_definition(),
        AssetBalanceScope::Global,
        account(7),
    )
    .expect("pool");
    let report = state
        .rehearse_taira_activation(unregistered)
        .expect("rehearsal");
    assert!(report.reserve_registered_in_overlay);
    assert_eq!(report.pool_bootstrap, "bootstrapped");

    assert_eq!(state.verify_world_state_accumulator().unwrap(), before);
    assert_eq!(state.view().native_execution_tip(), Some(tip));
    let view = state.view();
    assert_eq!(
        view.world().runtime_selection().id().unwrap().to_string(),
        LIVE_REPAIRED_SELECTION,
    );
    assert!(retained_certificate(view.world()).unwrap().is_none());
}

/// The universal root's lane policy refuses runtime-upgrade instructions without a Parliament
/// module, and no such module is configured here, as on Taira. Only an attestation of the change
/// set the retained certificate scheduled is admitted, through ordinary signed transactions.
#[test]
fn validators_attest_only_the_certified_change_set_by_ordinary_transactions() {
    use crate::runtime::attestation::sign_support;
    use iroha_data_model::{
        isi::runtime_upgrade::AttestRuntimeSupportV1, runtime::lifecycle::ChangeSetId,
    };

    let mut fixture = repaired_chain();
    let chain_id = fixture.chain.state().view().chain_id.clone();
    fixture_deployment::allow(&chain_id, fixture.chain.network_id());
    let certificate = sign(plan(&fixture));
    let change = certificate.plan.change_set.clone();
    let id = change.id().unwrap();
    let attest_with = |fixture: &mut Fixture, attestation, time: u64| {
        fixture.chain.sign(
            &key(6),
            [AttestRuntimeSupportV1 { attestation }.into()],
            time - 1,
        )
    };
    let committee: Vec<_> = fixture
        .chain
        .validators()
        .iter()
        .map(|(peer, _)| peer.public_key().clone())
        .collect();

    // Before a certificate is retained the policy refuses, as every older release does.
    let forged = SupportAttestationV1 {
        network: change.network,
        root: change.root,
        change_set: id,
        predecessor: change.predecessor,
        successor: change.successor,
        committee: committee_digest(&committee).unwrap(),
        tested_checkpoint: fixture.chain.height(),
        valid_from: change.effective_height,
        valid_until: change.effective_height + 1,
        release: [0x46; 32],
        validator: committee[0].clone(),
    }
    .sign(fixture_keys()[0].private_key())
    .unwrap();
    let time = fixture.next_time_ms();
    let early = attest_with(&mut fixture, forged, time);
    assert_eq!(fixture.chain.commit_at(time, vec![early]), [false]);

    let (time, good) = schedule_transaction(&mut fixture, certificate.clone());
    assert_eq!(fixture.chain.commit_at(time, vec![good]), [true]);

    // An attestation of any other change set is still refused.
    let other_id = ChangeSetId([0x77; 32]);
    let other = SupportAttestationV1 {
        network: change.network,
        root: change.root,
        change_set: other_id,
        predecessor: change.predecessor,
        successor: change.successor,
        committee: committee_digest(&committee).unwrap(),
        tested_checkpoint: fixture.chain.height(),
        valid_from: change.effective_height,
        valid_until: change.effective_height + 1,
        release: [0x46; 32],
        validator: committee[0].clone(),
    }
    .sign(fixture_keys()[0].private_key())
    .unwrap();
    let time = fixture.next_time_ms();
    let unrelated = attest_with(&mut fixture, other, time);
    assert_eq!(fixture.chain.commit_at(time, vec![unrelated]), [false]);

    // All four validators' own attestations commit as ordinary transactions and it activates.
    let height = fixture.chain.height();
    let time = fixture.next_time_ms();
    let mut attestations = Vec::new();
    for validator in fixture_keys() {
        let signed = {
            let view = fixture.chain.state().view();
            sign_support(view.world(), &committee, height, id, &validator, [0x46; 32])
                .expect("the validator supports the scheduled change set")
        };
        attestations.push(attest_with(&mut fixture, signed, time));
    }
    assert_eq!(
        fixture.chain.commit_at(time, attestations),
        [true, true, true, true],
    );
    while fixture.chain.height() < change.effective_height {
        let time = fixture.next_time_ms();
        fixture.chain.commit_at(time, Vec::new());
    }
    let view = fixture.chain.state().view();
    assert_eq!(
        view.world().runtime_change_sets().get(&id).unwrap().status,
        ChangeSetStatusV1::Activated(change.effective_height),
    );
    assert_eq!(
        view.world().runtime_selection().id().unwrap(),
        change.successor
    );
}

fn schedule_transaction(
    fixture: &mut Fixture,
    certificate: TairaActivationCertificateV1,
) -> (u64, SignedTransaction) {
    let time = fixture.next_time_ms();
    let signed = fixture.chain.sign(
        &key(6),
        [ScheduleTairaTimeIronwoodV1 { certificate }.into()],
        time - 1,
    );
    (time, signed)
}

#[test]
fn a_signed_schedule_passes_ordinary_admission_once_and_a_bad_one_stays_unbound() {
    let mut fixture = repaired_chain();
    let chain_id = fixture.chain.state().view().chain_id.clone();
    fixture_deployment::allow(&chain_id, fixture.chain.network_id());
    let certificate = sign(plan(&fixture));

    // A signed but changed plan is the ordinary unbound envelope, on every release: the queue
    // refuses it with the same rejection a release that kept the payload opaque commits.
    let mut tampered = certificate.clone();
    tampered.plan.operation_id = [0x99; 32];
    let (_, bad) = schedule_transaction(&mut fixture, tampered);
    {
        let view = fixture.chain.state().view();
        assert!(!queue_admits(&view, &bad));
        assert_eq!(
            crate::runtime::envelope::first_unadmitted_envelope(
                view.world().runtime_selection(),
                bad.instructions(),
            ),
            Some(RuntimeRejectionV1::DefinitionInactive {
                wire_id: ScheduleTairaTimeIronwoodV1::WIRE_ID.to_owned(),
            }),
        );
    }

    let (time, good) = schedule_transaction(&mut fixture, certificate.clone());
    {
        let view = fixture.chain.state().view();
        assert!(queue_admits(&view, &good));
    }
    assert_eq!(fixture.chain.commit_at(time, vec![good]), [true]);
    let id = certificate.plan.change_set.id().unwrap();
    {
        let view = fixture.chain.state().view();
        assert_eq!(
            view.world().runtime_change_sets().get(&id).unwrap().status,
            ChangeSetStatusV1::Scheduled,
        );
        assert_eq!(
            retained_certificate(view.world()).unwrap().as_ref(),
            Some(&certificate),
        );
    }

    // The same plan cannot be scheduled again: it is an unbound envelope from now on.
    let (_, again) = schedule_transaction(&mut fixture, certificate.clone());
    {
        let view = fixture.chain.state().view();
        assert!(!queue_admits(&view, &again));
    }

    // An ordinary transaction after it finds no leftover boundary.
    attest_all(&mut fixture, &certificate.plan.change_set);
    while fixture.chain.height() < certificate.plan.change_set.effective_height {
        let time = fixture.next_time_ms();
        fixture.chain.commit_at(time, Vec::new());
    }
    let time = fixture.next_time_ms();
    let bootstrap = fixture.chain.sign(
        &key(6),
        [BootstrapPrivacyIronwoodPoolV1::new(pool()).into()],
        time - 1,
    );
    assert_eq!(fixture.chain.commit_at(time, vec![bootstrap]), [true]);
    fixture
        .chain
        .state()
        .verify_world_state_accumulator()
        .expect("cold capture authenticates the bootstrapped World");
}
