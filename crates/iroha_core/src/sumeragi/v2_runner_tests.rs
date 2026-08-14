//! Integration-heavy unit cases for the Sumeragi v2 runner.
use super::super::FairV2IngressPushError;
use super::*;
use crate::{
    NetworkMessage,
    merge_sidecar::{
        CERTIFIED_MERGE_SIDECAR_VERSION_V1, CertifiedMergeSidecarChunkV1,
        CertifiedMergeSidecarCloseV1, CertifiedMergeSidecarMessage,
        CertifiedMergeSidecarSemanticSequenceV1, CertifiedMergeSidecarServiceGenerationV1,
        CertifiedMergeSidecarStreamEpochV1,
    },
    sumeragi::{LaneRelayMessage, v2_effects::v2_payload_is_terminal_reducer_control},
};
use iroha_config::parameters::actual::{NodeRole, SumeragiV2KeyPolicy, SumeragiV2Limits};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
use iroha_data_model::{
    account::AccountId,
    block::decode_framed_signed_block,
    isi::Log,
    peer::PeerId,
    transaction::{TransactionBuilder, signed::TransactionResultInner},
    trigger::DataTriggerSequence,
};
use iroha_logger::Level;
use iroha_p2p::network::{
    NetworkActorAdmissionError, NetworkReplyFlushAckTestFixture, NetworkReplyRouteTestFixture,
    NetworkReplyRoutes,
};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    sync::{Mutex, atomic::AtomicUsize},
};
use tempfile::TempDir;
include!("tests/v2_runner_unsealed_00.rs");
include!("tests/v2_runner_unsealed_01.rs");
include!("tests/v2_runner_unsealed_02.rs");
include!("tests/v2_runner_upstream_recovery.rs");
include!("tests/v2_runner_lifecycle_startup_order.rs");
#[test]
fn recovered_lifecycle_factory_dependency_permit_retains_exact_signer_and_cadence() {
    let local_signer = KeyPair::random();
    let expected = local_signer.public_key().clone();
    let expected_cadence = Duration::from_millis(777);
    let permit =
        RecoveredLifecycleOwnerFactoryDependencyPermitV1::for_test(local_signer, expected_cadence);
    let (local_signer, block_cadence) = permit.into_factory_dependencies();
    assert_eq!(local_signer.public_key(), &expected);
    assert_eq!(block_cadence, expected_cadence);

    let _guard = super::super::status::rbc_status_test_guard();
    super::super::status::clear_v2_status();
    let (context, _) = context();
    let ingress_ready = Arc::new(AtomicBool::new(false));
    let ingress = Arc::new(FairV2Ingress::new(1, 1024 * 1024, 1024 * 1024, 0, 0));
    ingress
        .configure_roster(std::iter::empty())
        .expect("configure lifecycle activation ingress");
    let activation = ProductionLifecycleRunnerActivationV1::current_height_for_test(
        Arc::clone(&ingress_ready),
        Arc::clone(&ingress),
    );
    let activated = activation
        .open_and_publish(&ingress, runner_status(&context))
        .expect("current-height lifecycle activation opens and publishes exactly once");
    assert!(ingress_ready.load(Ordering::Acquire));
    assert_eq!(
        super::super::status::v2_status()
            .expect("current-height activation publishes status")
            .height_context_id,
        context.id()
    );
    drop(activated);
    assert!(!ingress_ready.load(Ordering::Acquire));
    assert!(!ingress.state.lock().open);
    super::super::status::clear_v2_status();

    let exact_ingress = Arc::new(FairV2Ingress::new(1, 1024 * 1024, 1024 * 1024, 0, 0));
    exact_ingress
        .configure_roster(std::iter::empty())
        .expect("configure exact lifecycle activation ingress");
    let foreign_ingress = Arc::new(FairV2Ingress::new(1, 1024 * 1024, 1024 * 1024, 0, 0));
    foreign_ingress
        .configure_roster(std::iter::empty())
        .expect("configure foreign lifecycle activation ingress");
    let exact_ready = Arc::new(AtomicBool::new(true));
    exact_ingress.open().expect("open the stale exact ingress");
    let activation = ProductionLifecycleRunnerActivationV1::current_height_for_test(
        Arc::clone(&exact_ready),
        Arc::clone(&exact_ingress),
    );
    assert!(matches!(
        activation.open_and_publish(&foreign_ingress, runner_status(&context)),
        Err(V2RunnerError::LifecycleActivationIngressMismatch)
    ));
    assert!(!exact_ready.load(Ordering::Acquire));
    assert!(!exact_ingress.state.lock().open);
    assert!(!foreign_ingress.state.lock().open);
}
#[test]
fn outer_ingress_cursor_preserves_sequence_and_attests_runner_reach() {
    let context_id = wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
        b"outer-ingress-cursor-context",
    )));
    let height = 7;
    let mut turns = outer_ingress_turns(2, context_id, height);
    let snapshot = turns
        .lifecycle_rank_snapshot(LifecycleRunnerRankTarget::Ingress)
        .expect("the live cursor must attest the requested outer turn");
    assert_eq!(snapshot.context_id(), context_id);
    assert_eq!(snapshot.height(), height);
    assert_eq!(snapshot.target(), LifecycleRunnerRankTarget::Ingress);
    assert_eq!(snapshot.debt(), 2);
    assert_eq!(turns.reach_debt(OuterIngressTurn::Completion), Some(0));
    assert_eq!(turns.reach_debt(OuterIngressTurn::Runtime), Some(1));
    assert_eq!(turns.reach_debt(OuterIngressTurn::Ingress), Some(2));
    {
        let turn = turns.next_current().expect("Completion turn");
        assert_eq!(turn.turn(), OuterIngressTurn::Completion);
    }
    assert_eq!(turns.reach_debt(OuterIngressTurn::Completion), Some(2));
    {
        let turn = turns.next_current().expect("Runtime turn");
        assert_eq!(turn.turn(), OuterIngressTurn::Runtime);
    }
    assert_eq!(turns.reach_debt(OuterIngressTurn::Completion), Some(1));
    {
        let turn = turns.next_current().expect("Ingress turn");
        assert_eq!(turn.turn(), OuterIngressTurn::Ingress);
    }
    assert_eq!(turns.reach_debt(OuterIngressTurn::Completion), Some(0));
    let mut remaining = Vec::new();
    while let Some(turn) = turns.next_current() {
        remaining.push(turn.turn());
    }
    assert_eq!(
        remaining,
        vec![
            OuterIngressTurn::Completion,
            OuterIngressTurn::Runtime,
            OuterIngressTurn::Ingress,
        ]
    );
    let mut minimum = outer_ingress_turns(0, context_id, height);
    let mut count = 0;
    while minimum.next_current().is_some() {
        count += 1;
    }
    assert_eq!(count, 3);
    assert_eq!(minimum.reach_debt(OuterIngressTurn::Ingress), None);
}
