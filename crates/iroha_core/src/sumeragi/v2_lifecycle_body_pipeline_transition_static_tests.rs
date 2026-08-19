#[cfg(test)]
fn digest_from_hash(hash: &iroha_crypto::Hash) -> super::LifecycleDigest {
    let mut bytes = [0_u8; 32];
    bytes.copy_from_slice(hash.as_ref());
    super::LifecycleDigest::new(bytes)
}
#[cfg(test)]
mod static_tests {
    use super::super::{LifecycleDigest, LifecycleKey, LifecycleRound, LifecycleStage};
    use super::*;
    fn key(
        phase: LifecyclePhase,
        proposal_round: bool,
        subject: bool,
        commitment: Option<LifecycleDigest>,
    ) -> LifecycleKey {
        LifecycleKey::new(
            LifecycleDigest::new([1; 32]),
            LifecycleRound::new(7, 3),
            proposal_round.then_some(LifecycleRound::new(7, 3)),
            subject.then_some(LifecycleDigest::new([2; 32])),
            phase,
            commitment,
        )
    }
    fn stage(kind: LifecycleStageKind, scope: PredecessorScope) -> LifecycleStage {
        LifecycleStage::new(kind, scope)
    }
    #[test]
    fn durable_successor_relation_covers_all_ten_exact_continuation_edges() {
        let commitment = LifecycleDigest::new([3; 32]);
        let exact = [
            (
                DurableContinuationEdge::FetchToStore,
                LifecycleWorkClass::Fetch,
                key(LifecyclePhase::Fetch, true, true, Some(commitment)),
                LifecycleStageKind::FetchBody,
                LifecycleWorkClass::Store,
                key(LifecyclePhase::Store, true, true, Some(commitment)),
                LifecycleStageKind::StoreBody,
            ),
            (
                DurableContinuationEdge::StoreToValidate,
                LifecycleWorkClass::Store,
                key(LifecyclePhase::Store, true, true, Some(commitment)),
                LifecycleStageKind::StoreBody,
                LifecycleWorkClass::Validate,
                key(LifecyclePhase::Validate, true, true, Some(commitment)),
                LifecycleStageKind::ValidateBody,
            ),
            (
                DurableContinuationEdge::ValidateToApply,
                LifecycleWorkClass::Validate,
                key(LifecyclePhase::Validate, true, true, None),
                LifecycleStageKind::ValidateBody,
                LifecycleWorkClass::Apply,
                key(LifecyclePhase::Apply, true, true, Some(commitment)),
                LifecycleStageKind::ApplyDecision,
            ),
            (
                DurableContinuationEdge::ValidateToInvalidBodyReport,
                LifecycleWorkClass::Validate,
                key(LifecyclePhase::Validate, true, true, None),
                LifecycleStageKind::ValidateBody,
                LifecycleWorkClass::InvalidBodyReport,
                key(
                    LifecyclePhase::DiagnosticInvalidBody,
                    true,
                    true,
                    Some(commitment),
                ),
                LifecycleStageKind::ReportInvalidBody,
            ),
            (
                DurableContinuationEdge::ValidateToSignPrepare,
                LifecycleWorkClass::Validate,
                key(LifecyclePhase::Validate, true, true, None),
                LifecycleStageKind::ValidateBody,
                LifecycleWorkClass::SignVote,
                key(LifecyclePhase::Prepare, true, true, Some(commitment)),
                LifecycleStageKind::SignPrepareVote,
            ),
            (
                DurableContinuationEdge::ValidateToSignCommit,
                LifecycleWorkClass::Validate,
                key(LifecyclePhase::Validate, true, true, None),
                LifecycleStageKind::ValidateBody,
                LifecycleWorkClass::SignVote,
                key(LifecyclePhase::Commit, true, true, Some(commitment)),
                LifecycleStageKind::SignCommitVote,
            ),
            (
                DurableContinuationEdge::SignProposalToBroadcast,
                LifecycleWorkClass::SignProposal,
                key(LifecyclePhase::Proposal, true, true, None),
                LifecycleStageKind::SignProposal,
                LifecycleWorkClass::Broadcast,
                key(LifecyclePhase::BroadcastProposal, true, true, None),
                LifecycleStageKind::BroadcastProposal,
            ),
            (
                DurableContinuationEdge::SignPrepareToBroadcast,
                LifecycleWorkClass::SignVote,
                key(LifecyclePhase::Prepare, true, true, Some(commitment)),
                LifecycleStageKind::SignPrepareVote,
                LifecycleWorkClass::Broadcast,
                key(
                    LifecyclePhase::BroadcastPrepareVote,
                    true,
                    true,
                    Some(commitment),
                ),
                LifecycleStageKind::BroadcastPrepareVote,
            ),
            (
                DurableContinuationEdge::SignCommitToBroadcast,
                LifecycleWorkClass::SignVote,
                key(LifecyclePhase::Commit, true, true, Some(commitment)),
                LifecycleStageKind::SignCommitVote,
                LifecycleWorkClass::Broadcast,
                key(
                    LifecyclePhase::BroadcastCommitVote,
                    true,
                    true,
                    Some(commitment),
                ),
                LifecycleStageKind::BroadcastCommitVote,
            ),
            (
                DurableContinuationEdge::SignTimeoutToBroadcast,
                LifecycleWorkClass::SignTimeout,
                key(LifecyclePhase::Timeout, false, false, None),
                LifecycleStageKind::SignTimeoutVote,
                LifecycleWorkClass::Broadcast,
                key(LifecyclePhase::BroadcastTimeoutVote, false, false, None),
                LifecycleStageKind::BroadcastTimeoutVote,
            ),
        ];
        for (edge, parent_class, parent_key, parent_kind, child_class, child_key, child_kind) in
            exact
        {
            assert!(durable_continuation_successor_is_exact(
                edge,
                parent_class,
                parent_key,
                stage(parent_kind, PredecessorScope::Independent),
                child_class,
                child_key,
                stage(child_kind, PredecessorScope::Independent),
            ));
        }
        assert!(!durable_continuation_successor_is_exact(
            DurableContinuationEdge::ValidateToApply,
            LifecycleWorkClass::Validate,
            key(LifecyclePhase::Validate, false, true, None),
            stage(
                LifecycleStageKind::ValidateBody,
                PredecessorScope::Independent,
            ),
            LifecycleWorkClass::Apply,
            key(LifecyclePhase::Apply, false, true, Some(commitment)),
            stage(
                LifecycleStageKind::ApplyDecision,
                PredecessorScope::Independent,
            ),
        ));
        assert!(!durable_continuation_successor_is_exact(
            DurableContinuationEdge::ValidateToApply,
            LifecycleWorkClass::Validate,
            key(LifecyclePhase::Validate, true, false, None),
            stage(
                LifecycleStageKind::ValidateBody,
                PredecessorScope::Independent,
            ),
            LifecycleWorkClass::Apply,
            key(LifecyclePhase::Apply, true, false, Some(commitment)),
            stage(
                LifecycleStageKind::ApplyDecision,
                PredecessorScope::Independent,
            ),
        ));
        assert!(!durable_continuation_successor_is_exact(
            DurableContinuationEdge::StoreToValidate,
            LifecycleWorkClass::Store,
            key(LifecyclePhase::Store, true, true, Some(commitment)),
            stage(LifecycleStageKind::StoreBody, PredecessorScope::Independent,),
            LifecycleWorkClass::Validate,
            key(LifecyclePhase::Validate, true, true, Some(commitment)),
            stage(
                LifecycleStageKind::ValidateBody,
                PredecessorScope::ReadyOrdinalPrefix,
            ),
        ));
        assert!(!durable_continuation_successor_is_exact(
            DurableContinuationEdge::FetchToStore,
            LifecycleWorkClass::Fetch,
            key(LifecyclePhase::Fetch, true, true, Some(commitment)),
            stage(LifecycleStageKind::FetchBody, PredecessorScope::Independent,),
            LifecycleWorkClass::Store,
            key(
                LifecyclePhase::Store,
                true,
                true,
                Some(LifecycleDigest::new([4; 32])),
            ),
            stage(LifecycleStageKind::StoreBody, PredecessorScope::Independent,),
        ));
        assert!(!durable_continuation_successor_is_exact(
            DurableContinuationEdge::SignTimeoutToBroadcast,
            LifecycleWorkClass::SignTimeout,
            key(LifecyclePhase::Timeout, false, false, None),
            stage(
                LifecycleStageKind::SignTimeoutVote,
                PredecessorScope::Independent,
            ),
            LifecycleWorkClass::Broadcast,
            key(
                LifecyclePhase::BroadcastTimeoutVote,
                true,
                true,
                Some(commitment),
            ),
            stage(
                LifecycleStageKind::BroadcastTimeoutVote,
                PredecessorScope::Independent,
            ),
        ));
    }
    #[test]
    fn durable_successor_payload_relation_rejects_body_frame_substitution() {
        let round = LifecycleRound::new(7, 3);
        let frame = DurablePayloadReference::BodyFrame(
            super::super::schema::DurableBodyFrameReference::new(
                LifecycleDigest::new([1; 32]),
                round,
                LifecycleDigest::new([2; 32]),
                LifecycleDigest::new([3; 32]),
                LifecycleDigest::new([4; 32]),
            ),
        );
        let foreign = DurablePayloadReference::BodyFrame(
            super::super::schema::DurableBodyFrameReference::new(
                LifecycleDigest::new([1; 32]),
                round,
                LifecycleDigest::new([2; 32]),
                LifecycleDigest::new([3; 32]),
                LifecycleDigest::new([5; 32]),
            ),
        );
        assert!(durable_continuation_payload_is_exact(
            DurableContinuationEdge::FetchToStore,
            frame,
            frame,
        ));
        assert!(!durable_continuation_payload_is_exact(
            DurableContinuationEdge::FetchToStore,
            DurablePayloadReference::None,
            frame,
        ));
        for edge in [
            DurableContinuationEdge::StoreToValidate,
            DurableContinuationEdge::ValidateToApply,
        ] {
            assert!(durable_continuation_payload_is_exact(edge, frame, frame));
            assert!(!durable_continuation_payload_is_exact(
                edge,
                DurablePayloadReference::None,
                DurablePayloadReference::None,
            ));
            assert!(!durable_continuation_payload_is_exact(edge, frame, foreign,));
            assert!(!durable_continuation_payload_is_exact(
                edge,
                frame,
                DurablePayloadReference::None,
            ));
            assert!(!durable_continuation_payload_is_exact(
                edge,
                DurablePayloadReference::None,
                frame,
            ));
        }
        assert!(durable_continuation_payload_is_exact(
            DurableContinuationEdge::ValidateToSignPrepare,
            frame,
            DurablePayloadReference::None,
        ));
        assert!(!durable_continuation_payload_is_exact(
            DurableContinuationEdge::ValidateToSignPrepare,
            frame,
            frame,
        ));
        assert!(!durable_continuation_payload_is_exact(
            DurableContinuationEdge::FetchToStore,
            DurablePayloadReference::None,
            DurablePayloadReference::None,
        ));
        assert!(!durable_continuation_payload_is_exact(
            DurableContinuationEdge::ValidateToSignPrepare,
            DurablePayloadReference::None,
            DurablePayloadReference::None,
        ));
        for edge in [
            DurableContinuationEdge::SignProposalToBroadcast,
            DurableContinuationEdge::SignPrepareToBroadcast,
            DurableContinuationEdge::SignCommitToBroadcast,
            DurableContinuationEdge::SignTimeoutToBroadcast,
        ] {
            assert!(durable_continuation_payload_is_exact(
                edge,
                DurablePayloadReference::None,
                DurablePayloadReference::None,
            ));
            assert!(!durable_continuation_payload_is_exact(
                edge,
                frame,
                DurablePayloadReference::None,
            ));
        }
    }
    #[cfg(feature = "bls")]
    #[test]
    fn recovered_broadcast_and_next_sign_relation_accepts_only_adjacent_wal_vote() {
        let commitment = LifecycleDigest::new([0x63; 32]);
        let mut broadcast = super::tests::fetch_store_fixture(4).store_candidate;
        broadcast.key = key(LifecyclePhase::BroadcastProposal, true, true, None);
        broadcast.work_class = LifecycleWorkClass::Broadcast;
        broadcast.stage = stage(
            LifecycleStageKind::BroadcastProposal,
            PredecessorScope::Independent,
        );
        let mut next_sign = broadcast.clone();
        next_sign.key = key(LifecyclePhase::Prepare, true, true, Some(commitment));
        next_sign.work_class = LifecycleWorkClass::SignVote;
        next_sign.stage = stage(
            LifecycleStageKind::SignPrepareVote,
            PredecessorScope::Independent,
        );
        assert!(recovered_broadcast_and_next_sign_are_exact(
            &broadcast, &next_sign
        ));
        let mut prepare_broadcast = broadcast.clone();
        prepare_broadcast.key = key(
            LifecyclePhase::BroadcastPrepareVote,
            true,
            true,
            Some(commitment),
        );
        prepare_broadcast.stage = stage(
            LifecycleStageKind::BroadcastPrepareVote,
            PredecessorScope::Independent,
        );
        let mut commit_sign = next_sign.clone();
        commit_sign.key = key(LifecyclePhase::Commit, true, true, Some(commitment));
        commit_sign.stage = stage(
            LifecycleStageKind::SignCommitVote,
            PredecessorScope::Independent,
        );
        assert!(recovered_broadcast_and_next_sign_are_exact(
            &prepare_broadcast,
            &commit_sign
        ));
        let mut foreign = commit_sign.clone();
        foreign.key.execution_commitment = Some(LifecycleDigest::new([0x64; 32]));
        assert!(!recovered_broadcast_and_next_sign_are_exact(
            &prepare_broadcast,
            &foreign
        ));
        foreign = next_sign.clone();
        foreign.key.round = LifecycleRound::new(7, 4);
        assert!(!recovered_broadcast_and_next_sign_are_exact(
            &broadcast, &foreign
        ));
    }
    crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
        combined_recovered_sign_staging_is_two_child_affine_and_inert
    );
    crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
        transition_surface_is_ordered_borrow_bound_and_published
    );
    crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
        ordinary_certified_body_pipeline_reserves_executes_and_publishes_ready_validate
    );
}
