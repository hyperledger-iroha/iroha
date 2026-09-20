// Real execution and signed durable-finality controls for linear output publication.
use iroha_data_model::block::consensus::{ExecKv, ExecWitness};

fn executed_and_certified(
    sut: &SystemUnderTest,
    candidate: NewBlock,
    overlay: &mut crate::state::StateBlock<'_>,
) -> (CommittedBlock, ExecWitness) {
    let mut context = sut.finality_context(&candidate.header());
    let mut signed = candidate.into();
    crate::block::ValidBlock::execute_block_outputs_and_capture_for_test(
        &mut signed,
        overlay,
        Some(&sut.account_id),
        &mut context,
    )
    .expect("execute actual genesis sources and retain the original witness");
    crate::block::check_genesis_block(&signed, &sut.account_id)
        .expect("fixture preserves authenticated genesis admission and results");
    sut.certify_block(signed, overlay, context)
}

#[tokio::test]
async fn actual_witness_and_durable_finality_authorize_publication_exactly_once() {
    let sut = SystemUnderTest::new();
    let candidate = sut.create_block();
    let mut overlay = sut.state.block(candidate.header());
    let (committed, witness) = executed_and_certified(&sut, candidate, &mut overlay);
    let mut changed = witness.clone();
    changed.writes.push(ExecKv {
        key: b"substituted execution evidence".to_vec(),
        value: vec![1],
    });
    let error = overlay
        .authorize_execution_output_publication(&committed, &changed)
        .expect_err("foreign witness must not replace the actual captured evidence");
    assert!(error.contains("captured witness"), "{error}");
    sut.kura
        .store_block(committed.clone())
        .expect("durable canonical block without finality");
    let error = overlay
        .authorize_execution_output_publication(&committed, &witness)
        .expect_err("a valid in-memory QC is insufficient without exact durable finality");
    assert!(error.contains("durably stored"), "{error}");
    assert_eq!(sut.state.committed_height(), 0);
    sut.persist_finality(&committed);
    overlay
        .authorize_execution_output_publication(&committed, &witness)
        .expect("exact original witness rejoins its durable finality");
    assert!(
        overlay
            .authorize_execution_output_publication(&committed, &witness)
            .is_err(),
        "authorization must consume the sealed owner exactly once"
    );
    let events = overlay
        .apply_without_execution_with_verified_v2_finality(&committed)
        .expect("only authenticated metadata preparation can complete the publication owner");
    assert!(
        !events.is_empty(),
        "actual carrier publication retains its events"
    );
    overlay
        .commit()
        .expect("finalized exact publication succeeds");
    sut.promote_finality(&committed);
    assert_eq!(sut.state.committed_height(), 1);
    assert_eq!(
        sut.state.latest_block_hash_fast(),
        Some(committed.as_ref().hash())
    );
}

#[tokio::test]
async fn mere_output_seal_cannot_commit_even_with_durable_finality() {
    let sut = SystemUnderTest::new();
    let candidate = sut.create_block();
    let mut overlay = sut.state.block(candidate.header());
    let (committed, _) = executed_and_certified(&sut, candidate, &mut overlay);
    sut.persist_finality(&committed);
    overlay
        .commit()
        .expect_err("seal and durable QC do not perform authorization or metadata preparation");
    assert_eq!(sut.state.committed_height(), 0);
    assert_eq!(sut.state.latest_block_hash_fast(), None);
}

#[tokio::test]
async fn verified_finality_cannot_publish_a_fresh_unexecuted_overlay() {
    let sut = SystemUnderTest::new();
    let candidate = sut.create_block();
    let header = candidate.header();
    let mut executed = sut.state.block(header);
    let (committed, _) = executed_and_certified(&sut, candidate, &mut executed);
    sut.persist_finality(&committed);
    drop(executed);

    let mut unexecuted = sut.state.block(header);
    let error = unexecuted
        .apply_without_execution_with_verified_v2_finality(&committed)
        .expect_err("a durable genuine QC cannot replace this overlay's execution owner");
    assert!(error.to_string().contains("authorization"), "{error}");
    assert!(
        unexecuted.commit().is_err(),
        "failed preparation poisons the fresh overlay"
    );
    assert_eq!(sut.state.committed_height(), 0);
}

#[tokio::test]
async fn output_authorization_rejects_a_commit_without_verified_finality() {
    let sut = SystemUnderTest::new();
    let candidate = sut.create_block();
    let mut overlay = sut.state.block(candidate.header());
    let (committed, witness) = executed_and_certified(&sut, candidate, &mut overlay);
    sut.persist_finality(&committed);
    let unverified = crate::block::ValidBlock::new_unverified_for_tests(committed.as_ref().clone())
        .commit_unchecked()
        .unpack(|_| {});
    let error = overlay
        .authorize_execution_output_publication(&unverified, &witness)
        .expect_err("equal bytes and an unchecked commit cannot replace verified authority");
    assert!(error.contains("verified finality"), "{error}");
    assert!(overlay.commit().is_err());
    assert_eq!(sut.state.committed_height(), 0);
}

#[derive(Clone, Copy, Debug)]
enum PublicationMutation {
    World,
    Event,
    Topology,
}

fn mutate_publication(
    sut: &SystemUnderTest,
    overlay: &mut crate::state::StateBlock<'_>,
    header: BlockHeader,
    mutation: PublicationMutation,
) {
    match mutation {
        PublicationMutation::World => {
            overlay.world.accounts.remove(sut.account_id.clone());
        }
        PublicationMutation::Event => {
            overlay
                .world
                .push_pipeline_warning(header, "substituted", "unowned publication event");
        }
        PublicationMutation::Topology => {
            overlay
                .commit_topology
                .get_mut()
                .push(PeerId::new(sut.account_keypair.public_key().clone()));
        }
    }
}

#[tokio::test]
async fn authorized_output_refuses_world_event_and_topology_drift_before_metadata() {
    for mutation in [
        PublicationMutation::World,
        PublicationMutation::Event,
        PublicationMutation::Topology,
    ] {
        let sut = SystemUnderTest::new();
        let candidate = sut.create_block();
        let header = candidate.header();
        let mut overlay = sut.state.block(header);
        let (committed, witness) = executed_and_certified(&sut, candidate, &mut overlay);
        sut.persist_finality(&committed);
        overlay
            .authorize_execution_output_publication(&committed, &witness)
            .unwrap();
        mutate_publication(&sut, &mut overlay, header, mutation);
        assert!(
            overlay
                .apply_without_execution_with_verified_v2_finality(&committed)
                .is_err(),
            "{mutation:?} drift after authorization cannot enter metadata preparation"
        );
        assert!(
            overlay.commit().is_err(),
            "failed preparation must poison its publication owner"
        );
        assert_eq!(sut.state.committed_height(), 0);
        assert!(
            sut.state
                .world_view()
                .accounts()
                .get(&sut.account_id)
                .is_some()
        );
    }
}

#[tokio::test]
async fn finalized_output_refuses_world_event_and_topology_drift_before_commit() {
    for mutation in [
        PublicationMutation::World,
        PublicationMutation::Event,
        PublicationMutation::Topology,
    ] {
        let sut = SystemUnderTest::new();
        let candidate = sut.create_block();
        let header = candidate.header();
        let mut overlay = sut.state.block(header);
        let (committed, witness) = executed_and_certified(&sut, candidate, &mut overlay);
        sut.persist_finality(&committed);
        overlay
            .authorize_execution_output_publication(&committed, &witness)
            .unwrap();
        let _events = overlay
            .apply_without_execution_with_verified_v2_finality(&committed)
            .unwrap();
        mutate_publication(&sut, &mut overlay, header, mutation);
        assert!(
            overlay.commit().is_err(),
            "{mutation:?} drift cannot reuse finalized publication"
        );
        assert_eq!(sut.state.committed_height(), 0);
        assert!(
            sut.state
                .world_view()
                .accounts()
                .get(&sut.account_id)
                .is_some()
        );
    }
}
