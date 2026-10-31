//! Live escrows, the steps that act on them and the selections that would strand them.

use iroha_crypto::Hash;
use iroha_data_model::{
    asset::AssetDefinitionId,
    escrow::{AssetEscrowKind, AssetEscrowRecord},
    isi::{
        Log,
        escrow::{CancelAssetLock, OpenAssetEscrow},
    },
    level::Level,
    runtime::definition::RestrictionEntryV1,
};
use iroha_primitives::numeric::Quantity;

use super::*;
use crate::runtime::release_catalog;

fn escrow_id(seed: u8) -> EscrowId {
    EscrowId(Hash::prehashed([seed; 32]))
}

fn asset() -> AssetDefinitionId {
    iroha_config::parameters::defaults::nexus::fees::fee_asset_id()
        .parse()
        .expect("fee asset definition")
}

/// An asset lock of `seed` in `status`.
fn record(seed: u8, status: AssetEscrowStatus) -> AssetEscrowRecord {
    let seller = iroha_test_samples::ALICE_ID.clone();
    AssetEscrowRecord {
        id: escrow_id(seed),
        seller: seller.clone(),
        buyer: None,
        asset_definition: asset(),
        amount: Quantity::one(),
        custody: seller,
        status,
        kind: AssetEscrowKind::Lock,
        remaining_amount: Quantity::one(),
        release_authority: None,
        expires_at_ms: None,
        evidence_hashes: Vec::new(),
        conditions: Vec::new(),
        created_at_ms: 1,
        accepted_at_ms: None,
        payment_sent_at_ms: None,
        disputed_at_ms: None,
        closed_at_ms: None,
        resolution: None,
    }
}

fn release(seed: u8) -> InstructionBox {
    ReleaseAssetEscrow {
        escrow_id: escrow_id(seed),
    }
    .into()
}

#[test]
fn escrow_instructions_continue_or_complete_and_openings_act_on_nothing() {
    let open: InstructionBox = OpenAssetEscrow::new(escrow_id(1), asset(), 1_u32).into();
    assert_eq!(escrow_step(&open), None, "opening custody is new work");
    let accept: InstructionBox = AcceptAssetEscrow {
        escrow_id: escrow_id(1),
    }
    .into();
    assert_eq!(escrow_step(&accept), Some((escrow_id(1), Step::Continue)));
    assert_eq!(
        escrow_step(&release(2)),
        Some((escrow_id(2), Step::Complete))
    );
    let cancel: InstructionBox = CancelControlledAssetLockV1 {
        cancel: CancelAssetLock {
            escrow_id: escrow_id(3),
            expected_remaining_amount: Quantity::one(),
        },
        waterfall: None,
    }
    .into();
    assert_eq!(escrow_step(&cancel), Some((escrow_id(3), Step::Complete)));
    let log: InstructionBox = Log::new(Level::INFO, "unrelated".into()).into();
    assert_eq!(escrow_step(&log), None);
}

#[test]
fn custody_is_live_until_paid_out_refunded_or_resolved() {
    for status in [
        AssetEscrowStatus::Open,
        AssetEscrowStatus::Accepted,
        AssetEscrowStatus::PaymentSent,
        AssetEscrowStatus::Disputed,
        AssetEscrowStatus::Locked,
    ] {
        assert!(is_live(status), "{status:?}");
    }
    for status in [
        AssetEscrowStatus::Released,
        AssetEscrowStatus::Cancelled,
        AssetEscrowStatus::Resolved,
        AssetEscrowStatus::DrawnDown,
        AssetEscrowStatus::Expired,
    ] {
        assert!(!is_live(status), "{status:?}");
    }
}

#[test]
fn admission_closure_lets_work_continue_and_completion_only_lets_it_complete() {
    for (restriction, continues, completes) in [
        (RestrictionV1::Deprecated(1), true, true),
        (RestrictionV1::AdmissionClosed(1), true, true),
        (RestrictionV1::CompletionOnly(1), false, true),
        (RestrictionV1::Disabled(1), false, false),
    ] {
        assert_eq!(Step::Continue.runs_under(restriction), continues);
        assert_eq!(Step::Complete.runs_under(restriction), completes);
    }
}

#[test]
fn only_a_live_escrow_of_the_world_is_acted_on() {
    let state = crate::state::State::new_for_testing(
        crate::state::World::default(),
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
    );
    {
        let mut escrows = state.world.asset_escrows.block();
        escrows.insert(escrow_id(1), record(1, AssetEscrowStatus::Locked));
        escrows.insert(escrow_id(2), record(2, AssetEscrowStatus::Released));
        escrows.commit();
    }
    let header = iroha_data_model::block::BlockHeader::new(
        std::num::NonZeroU64::new(1).expect("height"),
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let stx = block.transaction();
    assert_eq!(step(&release(1), &stx.world), Some(Step::Complete));
    assert_eq!(step(&release(2), &stx.world), None, "a released escrow");
    assert_eq!(step(&release(3), &stx.world), None, "an unknown escrow");
    assert_eq!(
        LiveObligations::of(&stx.world),
        LiveObligations::with_escrows(1)
    );
}

#[test]
fn a_successor_keeps_every_live_escrow_completable() {
    let catalog = release_catalog().expect("catalog");
    let baseline = catalog.baseline.clone();
    let live = LiveObligations::with_escrows(2);
    let check = |successor: &RootSelectionV1, live: &LiveObligations| {
        stranded(&baseline, successor, &catalog.definitions, live)
    };
    assert_eq!(check(&baseline, &live), None);
    let wire_id = wire_id_of::<ReleaseAssetEscrow>().expect("registered");
    let binding = baseline
        .bindings
        .iter()
        .find(|binding| binding.wire_id == wire_id)
        .expect("the baseline binds escrow release")
        .clone();

    let mut unbound = baseline.clone();
    unbound
        .bindings
        .retain(|binding| binding.wire_id != wire_id);
    let reason = check(&unbound, &live).expect("an unbound completion strands escrows");
    assert!(reason.contains("2 live Escrow obligations"), "{reason}");
    assert!(reason.contains("is unbound"), "{reason}");

    let restricted = |restriction| {
        let mut restricted = baseline.clone();
        restricted.restrictions = vec![RestrictionEntryV1 {
            definition: binding.interface,
            restriction,
        }];
        restricted
    };
    let reason = check(&restricted(RestrictionV1::Disabled(9)), &live)
        .expect("a disabled completion strands escrows");
    assert!(reason.contains("inactive or disabled"), "{reason}");
    assert_eq!(
        check(&restricted(RestrictionV1::CompletionOnly(9)), &live),
        None,
        "completion-only still completes"
    );
    assert_eq!(
        check(
            &restricted(RestrictionV1::Disabled(9)),
            &LiveObligations::default()
        ),
        None,
        "nothing is stranded once every escrow completed"
    );
}

#[test]
fn a_completion_or_declared_callee_cannot_rebind_while_custody_is_live() {
    use iroha_data_model::runtime::definition::ImplementationCodeV1;

    let catalog = release_catalog().expect("catalog");
    let baseline = &catalog.baseline;
    let live = LiveObligations::with_escrows(1);
    for wire_id in [
        wire_id_of::<ReleaseAssetEscrow>().expect("escrow release"),
        wire_id_of::<Log>().expect("a declared native callee"),
    ] {
        let original = binding(baseline, wire_id).expect("baseline binding");
        let mut replacement = catalog.definitions[&original.implementation].clone();
        let DefinitionDescriptorV1::Implementation(implementation) = &mut replacement else {
            panic!("native implementation");
        };
        implementation.code = ImplementationCodeV1::Native {
            module: "iroha.test.rebound".into(),
            reference_closure: [0x71; 32],
        };
        let id = replacement.id().expect("replacement identity");
        let mut published = catalog.definitions.clone();
        published.insert(id, replacement);
        let mut successor = baseline.clone();
        successor.active.push(id);
        successor.active.sort_unstable();
        successor
            .bindings
            .iter_mut()
            .find(|binding| binding.wire_id == wire_id)
            .expect("retained binding")
            .implementation = id;
        successor
            .check_closure(&published)
            .expect("closed replacement");

        let reason = stranded(baseline, &successor, &published, &live)
            .expect("an enabled replacement has not inherited custody");
        assert!(reason.contains(wire_id), "{reason}");
        assert!(reason.contains("rebound"), "{reason}");
        assert_eq!(
            stranded(
                baseline,
                &successor,
                &published,
                &LiveObligations::default()
            ),
            None,
            "rebinding is no longer blocked after custody drains"
        );
    }
}

#[test]
fn completion_dependencies_cannot_be_disabled_or_removed_while_custody_is_live() {
    let catalog = release_catalog().expect("catalog");
    let baseline = &catalog.baseline;
    let live = LiveObligations::with_escrows(1);
    for (id, descriptor) in &catalog.definitions {
        if !matches!(
            descriptor,
            DefinitionDescriptorV1::Event(_)
                | DefinitionDescriptorV1::Permission(_)
                | DefinitionDescriptorV1::Parameter(_)
                | DefinitionDescriptorV1::HostProfile(_)
        ) {
            continue;
        }
        let mut successor = baseline.clone();
        successor.restrictions.push(RestrictionEntryV1 {
            definition: *id,
            restriction: RestrictionV1::Disabled(9),
        });
        successor
            .check_closure(&catalog.definitions)
            .expect("a closed selection can restrict a dependency");
        let reason = stranded(baseline, &successor, &catalog.definitions, &live)
            .expect("completion must retain every enabled dependency");
        assert!(reason.contains(&id.to_string()), "{reason}");
    }
    for storage in &baseline.installed_storage {
        let mut successor = baseline.clone();
        successor.installed_storage.retain(|id| id != storage);
        let reason = stranded(baseline, &successor, &catalog.definitions, &live)
            .expect("completion storage remains installed");
        assert!(reason.contains(&storage.to_string()), "{reason}");
    }

    let mut successor = baseline.clone();
    successor.kernel = DefinitionId([0x72; 32]);
    assert!(stranded(baseline, &successor, &catalog.definitions, &live).is_some());
    let mut successor = baseline.clone();
    successor.commitment_profile = DefinitionId([0x74; 32]);
    assert!(
        stranded(baseline, &successor, &catalog.definitions, &live).is_some(),
        "completion retains the original proof-verification context"
    );
}

#[test]
fn an_unrelated_addition_keeps_the_original_completion_closure() {
    let catalog = release_catalog().expect("catalog");
    let change = crate::runtime::change_set::test_support::add_instruction(
        iroha_data_model::NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::from_untyped_unchecked(Hash::prehashed([0x73; 32])),
        ),
        9,
    );
    let applied = change
        .apply(&catalog.baseline, &catalog.definitions)
        .expect("closed additive change");
    assert_eq!(
        stranded(
            &catalog.baseline,
            &applied.selection,
            &catalog.definitions,
            &LiveObligations::with_escrows(1),
        ),
        None,
        "new interfaces and implementations do not change an existing completion"
    );
}
