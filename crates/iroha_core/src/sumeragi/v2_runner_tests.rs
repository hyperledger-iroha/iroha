//! Integration-heavy unit cases for the Sumeragi v2 runner.

use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    sync::{Mutex, atomic::AtomicUsize},
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
use tempfile::TempDir;

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
    sumeragi::LaneRelayMessage,
};

include!("tests/v2_runner_unsealed_00.rs");
include!("tests/v2_runner_unsealed_01.rs");
include!("tests/v2_runner_unsealed_02.rs");
include!("tests/v2_runner_upstream_recovery.rs");
include!("tests/v2_runner_lifecycle_startup_order.rs");

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

    assert_eq!(turns.next(), Some(OuterIngressTurn::Completion));
    assert_eq!(turns.reach_debt(OuterIngressTurn::Completion), Some(2));
    assert_eq!(turns.next(), Some(OuterIngressTurn::Runtime));
    assert_eq!(turns.reach_debt(OuterIngressTurn::Completion), Some(1));
    assert_eq!(turns.next(), Some(OuterIngressTurn::Ingress));
    assert_eq!(turns.reach_debt(OuterIngressTurn::Completion), Some(0));
    assert_eq!(
        turns.collect::<Vec<_>>(),
        vec![
            OuterIngressTurn::Completion,
            OuterIngressTurn::Runtime,
            OuterIngressTurn::Ingress,
        ]
    );

    let mut minimum = outer_ingress_turns(0, context_id, height);
    assert_eq!(minimum.by_ref().count(), 3);
    assert_eq!(minimum.reach_debt(OuterIngressTurn::Ingress), None);
}
