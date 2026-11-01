//! Live escrows and AMX participations, the steps that act on them and the selections that would
//! strand them.

use std::collections::BTreeMap;

use iroha_crypto::Hash;
use iroha_data_model::{
    asset::{AssetBalanceScope, AssetDefinitionId, AssetId},
    escrow::{AssetEscrowKind, AssetEscrowRecord},
    isi::{
        Log,
        escrow::{CancelAssetLock, OpenAssetEscrow},
        sumeragi_amx::{
            BeginAmxV1, PrepareAmxV1, RegisterAmxDataspaceV1, RegisterAmxParticipantV1,
        },
    },
    level::Level,
    runtime::definition::RestrictionEntryV1,
    sumeragi_amx::{
        AmxBeginV1, AmxCertifiedBlockV1, AmxDecidedV1, AmxDecisionV1, AmxForeignInstanceV1,
        AmxGlobalEntryV1, AmxHandoffProofV1, AmxLegV1, AmxOutcomeV1, AmxParticipantStateV1,
        AmxPreparedV1, AmxRecordProofV1, AmxTransactionV1, AmxTransferEscrowV1, AmxTransferLegV1,
        AmxWriteProofV1, native_transfer_effects_hash,
    },
    sumeragi_finality::{
        CertifiedResultScope, authenticated_genesis, test_fixtures::NativeFinalityFixture,
    },
};
use iroha_primitives::numeric::Quantity;

use super::*;
use crate::runtime::{ReleaseCatalog, release_catalog};

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

/// A State for testing, with nothing live.
fn blank_state() -> crate::state::State {
    crate::state::State::new_for_testing(
        crate::state::World::default(),
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
    )
}

fn first_header() -> iroha_data_model::block::BlockHeader {
    iroha_data_model::block::BlockHeader::new(
        std::num::NonZeroU64::new(1).expect("height"),
        None,
        None,
        0,
        0,
    )
}

/// Published definitions with a replacement of the baseline implementation bound to `wire_id`,
/// and the closed baseline successor that rebinds `wire_id` to that replacement.
fn rebound(
    catalog: &ReleaseCatalog,
    wire_id: &str,
) -> (
    BTreeMap<DefinitionId, DefinitionDescriptorV1>,
    RootSelectionV1,
) {
    use iroha_data_model::runtime::definition::ImplementationCodeV1;

    let original = binding(&catalog.baseline, wire_id).expect("baseline binding");
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
    let mut successor = catalog.baseline.clone();
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
    (published, successor)
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
    let state = blank_state();
    {
        let mut escrows = state.world.asset_escrows.block();
        escrows.insert(escrow_id(1), record(1, AssetEscrowStatus::Locked));
        escrows.insert(escrow_id(2), record(2, AssetEscrowStatus::Released));
        escrows.commit();
    }
    let mut block = state.block(first_header());
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
    let catalog = release_catalog().expect("catalog");
    let baseline = &catalog.baseline;
    let live = LiveObligations::with_escrows(1);
    for wire_id in [
        wire_id_of::<ReleaseAssetEscrow>().expect("escrow release"),
        wire_id_of::<Log>().expect("a declared native callee"),
    ] {
        let (published, successor) = rebound(catalog, wire_id);
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
    for live in [
        LiveObligations::with_escrows(1),
        LiveObligations::with_amx(1, 1),
    ] {
        assert_eq!(
            stranded(
                &catalog.baseline,
                &applied.selection,
                &catalog.definitions,
                &live,
            ),
            None,
            "new interfaces and implementations do not change an existing completion"
        );
    }
}

#[test]
fn what_the_predecessor_already_left_unbound_or_disabled_blocks_no_change_set() {
    let catalog = release_catalog().expect("catalog");
    let baseline = &catalog.baseline;
    let live = LiveObligations::with_escrows(1);
    let log = wire_id_of::<Log>().expect("a declared native callee");
    let log_interface = binding(baseline, log).expect("baseline binding").interface;
    let mut disabled = baseline.clone();
    disabled.restrictions = vec![RestrictionEntryV1 {
        definition: log_interface,
        restriction: RestrictionV1::Disabled(5),
    }];
    assert_eq!(
        stranded(&disabled, &disabled, &catalog.definitions, &live),
        None,
        "a selection that keeps the predecessor's disabled callee strands nothing"
    );
    assert_eq!(
        stranded(&disabled, baseline, &catalog.definitions, &live),
        None,
        "lifting the restriction restores the same code"
    );
    let (published, rebound_log) = rebound(catalog, log);
    let reason = stranded(&disabled, &rebound_log, &published, &live)
        .expect("a disabled callee still cannot be rebound to new code");
    assert!(reason.contains("rebound"), "{reason}");

    let release = wire_id_of::<ReleaseAssetEscrow>().expect("escrow release");
    let mut unbound = baseline.clone();
    unbound
        .bindings
        .retain(|binding| binding.wire_id != release);
    assert_eq!(
        stranded(&unbound, &unbound, &catalog.definitions, &live),
        None,
        "a completion the predecessor left unbound blocks no change set"
    );
    let reason = stranded(&unbound, baseline, &catalog.definitions, &live)
        .expect("binding a completion again would be new settlement authority");
    assert!(
        reason.contains("is bound without a captured completion capability"),
        "{reason}"
    );
}

/// The dataspace whose native participant the AMX tests install.
const PARTICIPANT: DataSpaceId = DataSpaceId::new(42);
/// The other dataspace of every AMX transaction the tests begin.
const PEER: DataSpaceId = DataSpaceId::new(41);

/// Identity of the AMX transaction of `seed`.
fn tx(seed: u8) -> [u8; 32] {
    [seed; 32]
}

/// The global chain's entry of the transaction of `seed` between `PEER` and `PARTICIPANT`,
/// decided with `outcome` if one is given.
fn global_entry(seed: u8, outcome: Option<AmxOutcomeV1>) -> AmxGlobalEntryV1 {
    AmxGlobalEntryV1 {
        begin: AmxBeginV1 {
            tx: tx(seed),
            participants: vec![PEER, PARTICIPANT],
            deadline: 10,
            expires_at_ms: 1_000,
        },
        begun_at: 1,
        begun_time_ms: 1,
        original_cadence_ms: 100,
        yes: Vec::new(),
        decided: outcome.map(|outcome| AmxDecidedV1 {
            outcome,
            height: 2,
            time_ms: 2,
        }),
    }
}

/// Coordinator state holding `entries`, ascending by transaction.
fn coordinator(entries: Vec<AmxGlobalEntryV1>) -> SumeragiAmxState {
    SumeragiAmxState {
        dataspaces: Vec::new(),
        transactions: entries,
    }
}

/// A certified block that is never verified: classification reads only the instruction.
fn certified_block() -> AmxCertifiedBlockV1 {
    AmxCertifiedBlockV1 {
        consensus_header: vec![1],
        commit_qc: vec![2],
        result_preimage: vec![3],
    }
}

fn proof(record: AmxRecordV1) -> AmxRecordProofV1 {
    AmxRecordProofV1 {
        block: certified_block(),
        record,
        write: AmxWriteProofV1 {
            present: [0; 32],
            siblings: Vec::new(),
        },
    }
}

fn prepared(seed: u8) -> AmxRecordV1 {
    AmxRecordV1::Prepared(AmxPreparedV1 {
        tx: tx(seed),
        participant: PARTICIPANT,
        vote: AmxVoteV1::No,
    })
}

fn decision(seed: u8) -> AmxRecordV1 {
    AmxRecordV1::Decision(AmxDecisionV1 {
        tx: tx(seed),
        outcome: AmxOutcomeV1::Commit,
    })
}

fn relay_prepared(seed: u8) -> InstructionBox {
    RelayAmxPreparedV1 {
        proof: proof(prepared(seed)),
    }
    .into()
}

fn expire(seed: u8) -> InstructionBox {
    ExpireAmxV1 { tx: tx(seed) }.into()
}

fn handoff(dataspace: DataSpaceId) -> InstructionBox {
    RelayAmxHandoffV1 {
        dataspace,
        proof: AmxHandoffProofV1 {
            block: certified_block(),
        },
    }
    .into()
}

fn settle(dataspace: DataSpaceId, seed: u8) -> InstructionBox {
    SettleAmxV1 {
        dataspace,
        decision: proof(decision(seed)),
    }
    .into()
}

fn global_handoff(dataspace: DataSpaceId) -> InstructionBox {
    RelayGlobalAmxHandoffV1 {
        dataspace,
        proof: AmxHandoffProofV1 {
            block: certified_block(),
        },
    }
    .into()
}

fn transaction() -> AmxTransactionV1 {
    AmxTransactionV1 {
        legs: [PEER, PARTICIPANT]
            .into_iter()
            .map(|dataspace| AmxLegV1 {
                dataspace,
                payload: vec![7],
            })
            .collect(),
        deadline: 10,
        expires_at_ms: 1_000,
        nonce: [9; 32],
    }
}

/// Every AMX instruction that starts new work.
fn new_amx_work() -> [InstructionBox; 4] {
    [
        BeginAmxV1 {
            transaction: transaction(),
        }
        .into(),
        PrepareAmxV1 {
            dataspace: PARTICIPANT,
            transaction: transaction(),
            begin: proof(AmxRecordV1::Begin(AmxBeginV1 {
                tx: tx(1),
                participants: vec![PEER, PARTICIPANT],
                deadline: 10,
                expires_at_ms: 1_000,
            })),
        }
        .into(),
        RegisterAmxDataspaceV1 {
            dataspace: PEER,
            instance: [4; 32],
            anchor: vec![5],
        }
        .into(),
        RegisterAmxParticipantV1 {
            dataspace: PARTICIPANT,
            global_chain_id: "amx-obligations".into(),
            global_genesis: vec![6],
            global_successor: vec![7],
        }
        .into(),
    ]
}

/// The native participant `PARTICIPANT` of a genuine signed global genesis, holding one escrow of
/// a `Yes` vote per `(seed, settled)`, seeds ascending.
fn native_participant(escrows: &[(u8, Option<AmxOutcomeV1>)]) -> NativeAmxParticipantStateV1 {
    let fixture = NativeFinalityFixture::start("amx-obligations");
    let epoch = authenticated_genesis(fixture.genesis())
        .expect("signed genesis")
        .into_parts()
        .0;
    let global = AmxForeignInstanceV1::new(
        fixture.verifier().instance().0,
        epoch,
        CertifiedResultScope::TairaGlobal,
    )
    .expect("global tracker");
    let begun_at = global.current.authorization.first_height;
    let genesis = fixture.genesis().encode_wire().expect("genesis wire");
    let custody = iroha_test_samples::CARPENTER_ID.clone();
    let leg = AmxTransferLegV1 {
        source: AssetId::with_scope(
            asset(),
            iroha_test_samples::ALICE_ID.clone(),
            AssetBalanceScope::Dataspace(PARTICIPANT),
        ),
        destination: iroha_test_samples::BOB_ID.clone(),
        amount: Quantity::one(),
    };
    let effects_hash = native_transfer_effects_hash(&leg).expect("effects of the leg");
    let mut state = NativeAmxParticipantStateV1 {
        global_genesis: genesis.clone(),
        global_successor: genesis,
        global_chain_label: b"amx-obligations".to_vec(),
        participant: AmxParticipantStateV1::new(PARTICIPANT, global),
        custody: custody.clone(),
        escrows: Vec::new(),
    };
    for &(seed, settled) in escrows {
        state.participant.prepared.push(AmxPreparedEntryV1 {
            tx: tx(seed),
            begun_at,
            begun_time_ms: 1,
            deadline: begun_at + 10,
            expires_at_ms: 1_000,
            vote: AmxVoteV1::Yes(effects_hash),
            settled,
        });
        state.escrows.push(AmxTransferEscrowV1 {
            tx: tx(seed),
            effects_hash,
            leg: leg.clone(),
            custody: custody.clone(),
            settled,
        });
    }
    state
}

#[test]
fn amx_instructions_complete_live_participations_and_new_work_acts_on_nothing() {
    for work in new_amx_work() {
        assert_eq!(
            amx_step(&work),
            None,
            "registering, beginning and preparing"
        );
    }
    let complete = |participation| Some((participation, Step::Complete));
    assert_eq!(
        amx_step(&relay_prepared(1)),
        complete(AmxParticipation::Decision(tx(1)))
    );
    assert_eq!(
        amx_step(&expire(2)),
        complete(AmxParticipation::Decision(tx(2)))
    );
    assert_eq!(
        amx_step(&handoff(PEER)),
        complete(AmxParticipation::DecisionsOf(PEER))
    );
    assert_eq!(
        amx_step(&settle(PARTICIPANT, 3)),
        complete(AmxParticipation::Settlement {
            dataspace: PARTICIPANT,
            tx: tx(3),
        })
    );
    assert_eq!(
        amx_step(&global_handoff(PARTICIPANT)),
        complete(AmxParticipation::SettlementsOf(PARTICIPANT))
    );
    let misdirected_relay: InstructionBox = RelayAmxPreparedV1 {
        proof: proof(decision(1)),
    }
    .into();
    assert_eq!(amx_step(&misdirected_relay), None, "a relay carries a vote");
    let misdirected_settle: InstructionBox = SettleAmxV1 {
        dataspace: PARTICIPANT,
        decision: proof(prepared(1)),
    }
    .into();
    assert_eq!(
        amx_step(&misdirected_settle),
        None,
        "a settlement carries a decision"
    );
    assert_eq!(escrow_step(&settle(PARTICIPANT, 3)), None);
    assert_eq!(amx_step(&release(1)), None);
    for wire_id in AMX_DECISION_COMPLETIONS
        .into_iter()
        .chain(AMX_SETTLEMENT_COMPLETIONS)
    {
        assert!(
            binding(&release_catalog().expect("catalog").baseline, wire_id).is_some(),
            "the baseline binds {wire_id}"
        );
    }
    assert_eq!(wire_id_of::<SettleAmxV1>(), Some(SettleAmxV1::WIRE_ID));
}

#[test]
fn amx_participations_are_live_until_decided_or_settled() {
    let coordinator = coordinator(vec![
        global_entry(1, None),
        global_entry(2, Some(AmxOutcomeV1::Commit)),
    ]);
    let mut native = native_participant(&[(3, None), (4, Some(AmxOutcomeV1::Abort))]);
    let no = AmxPreparedEntryV1 {
        tx: tx(5),
        vote: AmxVoteV1::No,
        settled: None,
        ..native.participant.prepared[0]
    };
    native.participant.prepared.push(no);
    let live = |participation: AmxParticipation| participation.is_live(&coordinator, Some(&native));
    assert!(live(AmxParticipation::Decision(tx(1))), "undecided");
    assert!(!live(AmxParticipation::Decision(tx(2))), "decided");
    assert!(!live(AmxParticipation::Decision(tx(9))), "unknown");
    assert!(live(AmxParticipation::DecisionsOf(PARTICIPANT)));
    assert!(
        !live(AmxParticipation::DecisionsOf(DataSpaceId::new(77))),
        "takes part in nothing undecided"
    );
    let settlement = |seed| AmxParticipation::Settlement {
        dataspace: PARTICIPANT,
        tx: tx(seed),
    };
    assert!(live(settlement(3)), "unsettled");
    assert!(!live(settlement(4)), "settled");
    assert!(!live(settlement(5)), "a No vote escrows nothing");
    assert!(!live(settlement(9)), "never prepared");
    assert!(
        !live(AmxParticipation::Settlement {
            dataspace: PEER,
            tx: tx(3),
        }),
        "another participant's"
    );
    assert!(live(AmxParticipation::SettlementsOf(PARTICIPANT)));
    assert!(!live(AmxParticipation::SettlementsOf(PEER)));
    assert!(
        !AmxParticipation::SettlementsOf(PARTICIPANT).is_live(&coordinator, None),
        "no participant is installed"
    );
    assert_eq!(
        LiveObligations::amx(&coordinator, Some(&native)),
        LiveObligations::with_amx(1, 1)
    );

    native.escrows[0].settled = Some(AmxOutcomeV1::Commit);
    native.participant.prepared[0].settled = Some(AmxOutcomeV1::Commit);
    assert!(
        !AmxParticipation::SettlementsOf(PARTICIPANT).is_live(&coordinator, Some(&native)),
        "every escrow is settled"
    );
    assert_eq!(
        LiveObligations::amx(&SumeragiAmxState::default(), Some(&native)),
        LiveObligations::default()
    );
}

#[test]
fn only_a_live_amx_participation_of_the_world_is_acted_on() {
    let state = blank_state();
    let mut block = state.block(first_header());
    let mut stx = block.transaction();
    assert_eq!(LiveObligations::of(&stx.world), LiveObligations::default());
    assert_eq!(step(&expire(1), &stx.world), None, "nothing is begun");
    assert_eq!(
        step(&settle(PARTICIPANT, 3), &stx.world),
        None,
        "no participant is installed"
    );

    *stx.world.sumeragi_amx.get_mut() = coordinator(vec![
        global_entry(1, None),
        global_entry(2, Some(AmxOutcomeV1::Abort)),
    ]);
    let native = native_participant(&[(3, None), (4, Some(AmxOutcomeV1::Commit))]);
    *stx.world.sumeragi_amx_participant.get_mut() = crate::sumeragi::amx::RetainedNativeAmx::admit(
        &native,
        &iroha_allocation::AllocationBudget::new(64 << 20),
    )
    .expect("a valid participant");

    for completion in [
        relay_prepared(1),
        expire(1),
        handoff(PARTICIPANT),
        settle(PARTICIPANT, 3),
        global_handoff(PARTICIPANT),
    ] {
        assert_eq!(step(&completion, &stx.world), Some(Step::Complete));
    }
    assert_eq!(step(&expire(2), &stx.world), None, "a decided transaction");
    assert_eq!(
        step(&settle(PARTICIPANT, 4), &stx.world),
        None,
        "a settled escrow"
    );
    assert_eq!(
        step(&settle(PEER, 3), &stx.world),
        None,
        "another participant's escrow"
    );
    for work in new_amx_work() {
        assert_eq!(step(&work, &stx.world), None, "new work");
    }
    assert_eq!(
        LiveObligations::of(&stx.world),
        LiveObligations::with_amx(1, 1)
    );
}

#[test]
fn a_successor_keeps_every_live_amx_participation_completable() {
    let catalog = release_catalog().expect("catalog");
    let baseline = &catalog.baseline;
    let check = |successor: &RootSelectionV1, live: &LiveObligations| {
        stranded(baseline, successor, &catalog.definitions, live)
    };
    let deciding = LiveObligations::with_amx(3, 0);
    let settling = LiveObligations::with_amx(0, 2);
    assert_eq!(check(baseline, &LiveObligations::with_amx(3, 2)), None);

    let mut unbound = baseline.clone();
    unbound
        .bindings
        .retain(|binding| binding.wire_id != SettleAmxV1::WIRE_ID);
    let reason = check(&unbound, &settling).expect("an unbound settlement strands escrows");
    assert!(
        reason.contains("2 live AmxParticipation obligations awaiting settlement"),
        "{reason}"
    );
    assert!(reason.contains("is unbound"), "{reason}");

    let restricted = |wire_id: &str, restriction| {
        let mut restricted = baseline.clone();
        restricted.restrictions = vec![RestrictionEntryV1 {
            definition: binding(baseline, wire_id)
                .expect("baseline binding")
                .interface,
            restriction,
        }];
        restricted
    };
    for wire_id in AMX_SETTLEMENT_COMPLETIONS {
        let reason = check(&restricted(wire_id, RestrictionV1::Disabled(9)), &settling)
            .expect("a disabled settlement step strands escrows");
        assert!(reason.contains("inactive or disabled"), "{reason}");
        assert_eq!(
            check(
                &restricted(wire_id, RestrictionV1::CompletionOnly(9)),
                &settling
            ),
            None,
            "completion-only still settles"
        );
    }
    for wire_id in AMX_DECISION_COMPLETIONS {
        let reason = check(&restricted(wire_id, RestrictionV1::Disabled(9)), &deciding)
            .expect("a disabled decision step strands undecided transactions");
        assert!(
            reason.contains("3 live AmxParticipation obligations awaiting a decision"),
            "{reason}"
        );
    }
    assert_eq!(
        check(
            &restricted(SettleAmxV1::WIRE_ID, RestrictionV1::Disabled(9)),
            &LiveObligations::default()
        ),
        None,
        "nothing is stranded once every participation completed"
    );
}

#[test]
fn an_amx_completion_cannot_rebind_while_participations_are_live() {
    let catalog = release_catalog().expect("catalog");
    let baseline = &catalog.baseline;
    for (wire_id, live) in [
        (SettleAmxV1::WIRE_ID, LiveObligations::with_amx(0, 1)),
        (RelayAmxPreparedV1::WIRE_ID, LiveObligations::with_amx(1, 0)),
    ] {
        let (published, successor) = rebound(catalog, wire_id);
        let reason = stranded(baseline, &successor, &published, &live)
            .expect("an enabled replacement has not inherited the participation");
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
            "rebinding is no longer blocked once participations complete"
        );
    }
}

#[test]
fn amx_completions_are_native_and_retain_no_host_profile() {
    let catalog = release_catalog().expect("catalog");
    let baseline = &catalog.baseline;
    let host_profile = baseline
        .active
        .iter()
        .copied()
        .find(|id| {
            matches!(
                catalog.definitions[id],
                DefinitionDescriptorV1::HostProfile(_)
            )
        })
        .expect("the baseline admits a host profile");
    let mut successor = baseline.clone();
    successor.restrictions = vec![RestrictionEntryV1 {
        definition: host_profile,
        restriction: RestrictionV1::Disabled(9),
    }];
    assert_eq!(
        stranded(
            baseline,
            &successor,
            &catalog.definitions,
            &LiveObligations::with_amx(1, 1)
        ),
        None,
        "AMX completions invoke no contract"
    );
    let reason = stranded(
        baseline,
        &successor,
        &catalog.definitions,
        &LiveObligations::with_escrows(1),
    )
    .expect("controlled locks invoke contracts");
    assert!(reason.contains(&host_profile.to_string()), "{reason}");
}

#[test]
fn live_obligations_are_empty_only_without_any_class() {
    assert!(LiveObligations::default().is_empty());
    for live in [
        LiveObligations::with_escrows(1),
        LiveObligations::with_amx(1, 0),
        LiveObligations::with_amx(0, 1),
    ] {
        assert!(!live.is_empty(), "{live:?}");
    }
}
