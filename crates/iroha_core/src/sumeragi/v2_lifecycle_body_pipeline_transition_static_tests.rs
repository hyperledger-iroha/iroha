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
    use crate::sumeragi::v2_lifecycle_coordinator::{
        reviewed_lifecycle_ledger_source_for_test, reviewed_lifecycle_work_registry_source_for_test,
    };

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

    #[test]
    fn combined_recovered_sign_staging_is_two_child_affine_and_inert() {
        let source = include_str!("v2_lifecycle_body_pipeline_transition.rs");
        let adapter_source = include_str!("v2.rs");
        let registry_source = reviewed_lifecycle_work_registry_source_for_test();
        let registry_recovery_source =
            include_str!("v2_lifecycle_work_registry_validate_recovery.rs");
        let reducer = source
            .split_once("fn stage_recovered_lifecycle_sign_broadcast_and_sign_transition(")
            .expect("locate combined Sign reducer")
            .1
            .split_once("#[allow(clippy::too_many_arguments")
            .expect("locate end of combined Sign reducer")
            .0;
        let broadcast = reducer
            .find("stage_recovered_lifecycle_sign_broadcast_transition(")
            .expect("stage inherited Broadcast first");
        let next_ordinal = reducer
            .find(".checked_add(1)")
            .expect("derive adjacent next-Sign ordinal");
        let next_admission = reducer
            .find("staged.reduce_admit(AdmissionRequest::Candidate(next_sign))")
            .expect("admit next WAL Sign in the same copy");
        assert!(broadcast < next_ordinal && next_ordinal < next_admission);
        for required in [
            "coordinator.owner_index.contains_key(&next_sign.causal_root)",
            "next_sign_owner.causal_root() != next_sign_candidate.causal_root",
            "staged.records.len() != records_before.saturating_add(2)",
            "staged.high_water != next_sign_ordinal",
            "capacity_used_before[&CapacityClass::Consensus].saturating_add(1)",
        ] {
            assert!(
                reducer.contains(required),
                "combined Sign reducer omitted {required}"
            );
        }
        assert!(!reducer.contains("persist_exact"));

        let entry = source
            .split_once(
                "pub(super) fn prepare_recovered_lifecycle_sign_broadcast_and_sign_transition",
            )
            .expect("locate sealed combined Sign entrypoint")
            .1
            .split_once("/// Stage the sole live post-WAL Validate-to-Sign transaction.")
            .expect("locate end of sealed combined Sign entrypoint")
            .0;
        for required in [
            "RecoveredLifecycleBroadcastAndSignTransitionProjectionPermitV1::new()",
            ".publication_is_vote()",
            "stage_recovered_lifecycle_sign_broadcast_and_sign_transition(",
            ".bind_staged_children(",
            "successor",
            "broadcast_wait: WaitToken::new(",
        ] {
            assert!(
                entry.contains(required),
                "sealed entrypoint omitted {required}"
            );
        }
        for forbidden in ["persist_exact", "commit_after", "into_registry_children"] {
            assert!(
                !entry.contains(forbidden),
                "preparation entrypoint exposed {forbidden}"
            );
        }

        let publication = source
            .split_once("impl PreparedRecoveredLifecycleSignBroadcastAndSignTransition<'_, '_, '_>")
            .expect("locate combined transition publication")
            .1
            .split_once("fn map_sealed_successor_projection_error(")
            .expect("locate end of combined transition publication")
            .0;
        let persist = publication
            .find("persist_exact_staged_successor(&self.staged)")
            .expect("fsync the exact combined successor");
        let registry_commit = publication
            .find("successor.commit_after_publication()")
            .expect("split the registry pair only after fsync");
        let coordinator_commit = publication
            .find("*coordinator = staged")
            .expect("publish the exact staged coordinator");
        let mode = publication
            .find("if publication_is_vote")
            .expect("separate Vote debt from pre-reserved Proposal output");
        let vote_ready = publication
            .find("ready_index.contains(&broadcast_ordinal)")
            .expect("leave a Vote Broadcast Ready for typed refanout");
        let park = publication
            .find("LifecycleState::Waiting(broadcast_wait)")
            .expect("park only a Proposal Broadcast behind output ownership");
        let next_ready = publication
            .find("ready_index.contains(&next_sign_ordinal)")
            .expect("leave the WAL-backed Sign Ready");
        let vote_adapter_commit = publication
            .find("adapter.commit_after_durable_vote_broadcast_and_sign()")
            .expect("advance the Vote adapter only in the assertion-only tail");
        let adapter_commit = publication
            .find("adapter.commit_after_durable_broadcast_and_sign()")
            .expect("advance the Proposal adapter only in the assertion-only tail");
        assert!(
            persist < registry_commit
                && registry_commit < coordinator_commit
                && coordinator_commit < mode
                && mode < vote_ready
                && coordinator_commit < park
                && park < next_ready
                && next_ready < vote_adapter_commit
                && next_ready < adapter_commit
        );
        let tail = &publication[registry_commit..];
        assert!(!tail.contains("return "));
        assert!(!tail.contains(".is_err()"));

        let adapter_commit = adapter_source
            .split_once("fn commit_after_durable_broadcast_and_sign(self)")
            .expect("locate combined adapter publication")
            .1
            .split_once("/// Borrow-bound adapter successor for one registry-owned recovered Apply")
            .expect("locate end of combined adapter publication")
            .0;
        for required in [
            "RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign",
            "combined_authority_minted: true",
            "proposal_output_authority_minted: true",
            "next_sign: Some(_)",
            "outbound_payload: Some(_)",
            "adapter.reducer = next_reducer",
            "adapter.registry = next_registry",
        ] {
            assert!(
                adapter_commit.contains(required),
                "combined adapter publication omitted {required}"
            );
        }
        let vote_adapter_commit = adapter_source
            .split_once("fn commit_after_durable_vote_broadcast_and_sign(self)")
            .expect("locate combined Vote adapter publication")
            .1
            .split_once("/// Borrow-bound adapter successor for one registry-owned recovered Apply")
            .expect("locate end of combined Vote adapter publication")
            .0;
        for required in [
            "self.is_vote_broadcast_and_sign()",
            "combined_authority_minted: true",
            "proposal_output_authority_minted: false",
            "next_sign: Some(_)",
            "outbound_payload: None",
            "adapter.reducer = next_reducer",
            "adapter.registry = next_registry",
        ] {
            assert!(
                vote_adapter_commit.contains(required),
                "combined Vote adapter publication omitted {required}"
            );
        }

        let registry_prepare = registry_source
            .split_once(
                "pub(super) fn prepare_recovered_lifecycle_sign_broadcast_and_sign_successor",
            )
            .expect("locate combined registry preparation")
            .1
            .split_once(
                "impl<'registry, 'adapter> PreparedRecoveredLifecycleSignBroadcastSuccessor",
            )
            .expect("locate end of combined registry preparation")
            .0;
        let parent = registry_prepare
            .find("let parent_is_exact = match &sign.kind")
            .expect("authenticate the installed parent first");
        let body = registry_prepare
            .find(".project_broadcast_and_sign_authority(body)")
            .expect("consume only the opaque body authority");
        let wal = registry_prepare
            .find(".project_authenticated_signed_broadcast_and_sign(")
            .expect("rejoin the exact parent WAL carrier");
        let retain = registry_prepare
            .find("PreparedRecoveredLifecycleSignBroadcastAndSignSuccessor {")
            .expect("retain the unsplit executable pair");
        assert!(parent < body && body < wal && wal < retain);
        for forbidden in [
            "ValidatedBodyReceipt",
            "fn receipt(",
            "fn candidate(",
            "into_parts",
            ".entries.insert(",
            ".entries.remove(",
        ] {
            assert!(
                !registry_prepare.contains(forbidden),
                "combined registry preparation exposed {forbidden}"
            );
        }

        let commit = registry_source
            .split_once(
                "impl<'registry, 'adapter>\n    BoundRecoveredLifecycleSignBroadcastAndSignSuccessor",
            )
            .expect("locate combined successor publication tail")
            .1
            .split_once("include!(\"v2_lifecycle_work_registry_validate_recovery.rs\")")
            .expect("locate end of combined successor publication tail")
            .0;
        let remove_parent = commit
            .find(".remove(&sign_address)")
            .expect("remove the exact claimed Sign parent");
        let split = commit
            .find("successor.into_registry_children(")
            .expect("split the opaque pair only after publication");
        let broadcast_insert = commit
            .find(".insert(broadcast_address, broadcast_work)")
            .expect("install the inherited Broadcast carrier");
        let next_sign_insert = commit
            .find(".insert(next_sign_address, next_sign_work)")
            .expect("install the fresh next-WAL Sign carrier");
        assert!(
            remove_parent < split
                && split < broadcast_insert
                && broadcast_insert < next_sign_insert
        );
        for required in [
            "RecoveredLifecycleBroadcastAndSignRegistryCommitPermitV1::new()",
            "DurableRecoveredLifecycleSignedBroadcastWork",
            "DurableRecoveredLifecycleNextWalVoteSignWork",
            "dispatch_key: None",
        ] {
            assert!(
                commit.contains(required),
                "combined commit omitted {required}"
            );
        }
        assert!(!commit.contains("pub(super) fn into_"));

        let sign_dispatch = registry_recovery_source
            .split_once("pub(super) fn attest_ready_recovered_lifecycle_sign(")
            .expect("locate recovered Sign attestation")
            .1
            .split_once("/// Attest one exact Ready recovered Decision Fetch")
            .expect("bound recovered Sign attestation and dispatch")
            .0;
        assert_eq!(
            sign_dispatch
                .matches("DurableRecoveredLifecycleNextWalVoteSign")
                .count(),
            2
        );
        assert!(sign_dispatch.contains("PreparedRecoveredLifecycleSignCarrier::NextWalVote"));
        assert!(sign_dispatch.contains(".project_task(identity)"));
    }

    #[test]
    fn transition_surface_is_ordered_borrow_bound_and_inert() {
        let source = include_str!("v2_lifecycle_body_pipeline_transition.rs");
        let production = source
            .split_once("\n#[cfg(test)]\nmod static_tests {")
            .map(|(production, _)| production)
            .expect("transition source has one production prefix");
        let authorized_core = production
            .split("fn stage_body_stage_transition")
            .nth(1)
            .and_then(|suffix| suffix.split("/// Fully reduced coordinator copy").next())
            .expect("body-stage reducer has one bounded production body");
        let staging = authorized_core
            .find("stage_durable_transaction")
            .expect("staged transition clones coordinator state");
        let settlement = authorized_core
            .find("reduce_settle_body_parent_for_continuation")
            .expect("staged transition settles its parent");
        let admission = authorized_core
            .find("reduce_admit")
            .expect("staged transition admits its child");
        assert!(
            settlement < admission,
            "the same-class Effect branch must release capacity before child admission"
        );
        for required in [
            "candidate.replay_authority_is_exact(coordinator.active_context)",
            ".physical_geometry",
            ".normalized()",
            "let Some(&child_digest) = projected_slots.get(&child_slot)",
            "durable_continuation_payload_is_exact",
        ] {
            assert!(
                authorized_core.contains(required),
                "authorized transition core omitted {required}"
            );
        }
        for forbidden in [
            "projection::admission_request",
            "projection::durable_body_frame_reference",
            "candidate.payload =",
            "PendingRuntimeEffectBinding",
            "AdapterEffect",
        ] {
            assert!(
                !authorized_core.contains(forbidden),
                "authorized transition core reopened raw authority through {forbidden}"
            );
        }
        let sealed_fetch = production
            .split("pub(super) fn prepare_sealed_fetch_store_transition")
            .nth(1)
            .and_then(|suffix| {
                suffix
                    .split("/// Stage one sealed Store retirement and exact Validate admission")
                    .next()
            })
            .expect("sealed Fetch-to-Store entrypoint has one bounded body");
        let sealed_store = production
            .split("pub(super) fn prepare_sealed_store_validate_transition")
            .nth(1)
            .and_then(|suffix| {
                suffix
                    .split("/// Consume one exact invalid-body replay seal")
                    .next()
            })
            .expect("sealed Store-to-Validate entrypoint has one bounded body");
        let sealed_no_successor = production
            .split("pub(super) fn prepare_sealed_validate_no_successor_transition")
            .nth(1)
            .and_then(|suffix| {
                suffix
                    .split("/// Stage one sealed certified-Fetch retirement")
                    .next()
            })
            .expect("sealed Validate no-successor entrypoint has one bounded body");
        let sealed_report = production
            .split("pub(super) fn prepare_sealed_validate_report_transition")
            .nth(1)
            .and_then(|suffix| suffix.split("#[cfg(test)]\nfn digest_from_hash").next())
            .expect("sealed Validate report entrypoint has one bounded body");
        let sealed_sign = production
            .split("pub(super) fn prepare_sealed_validate_sign_transition")
            .nth(1)
            .and_then(|suffix| {
                suffix
                    .split("/// Consume one sealed inactive or no-effect Validate preview")
                    .next()
            })
            .expect("sealed Validate-to-Sign entrypoint has one bounded body");
        assert!(sealed_fetch.contains("PreparedCertifiedFetchStoreSuccessor<'registry>"));
        assert!(sealed_store.contains("PreparedDurableStoreValidateSuccessor<'registry>"));
        for sealed in [sealed_fetch, sealed_store] {
            assert!(sealed.contains("project_for_body_transition(lease, verified)"));
            assert!(sealed.contains("PreparedSealedBodyStageTransition"));
            for forbidden in [
                "&AdapterEffect",
                "&PendingRuntimeEffectBinding",
                "&DurableBodyReceipt",
                "CandidateAdmission",
                "candidate.payload =",
                "projection::admission_request",
            ] {
                assert!(
                    !sealed.contains(forbidden),
                    "sealed transition entrypoint accepts or forges {forbidden}"
                );
            }
        }
        assert!(
            sealed_no_successor.contains(
                "preview: PreparedReadyDurableValidateAdapterPreview<'registry, 'adapter>"
            )
        );
        assert!(sealed_no_successor.contains("project_no_successor_for_body_transition("));
        assert!(sealed_no_successor.contains("SealedValidateNoSuccessorProjectionPermit::new()"));
        assert!(sealed_no_successor.contains("_preview: preview"));
        assert!(
            sealed_report.contains(
                "report: PreparedInvalidBodyReportReplayPreAdmission<'registry, 'adapter>"
            )
        );
        assert!(sealed_report.contains("SealedInvalidBodyReportProjectionPermit::new()"));
        assert!(sealed_report.contains("_report: report"));
        for required in [
            "self.ledger_store.is_none()",
            "publication.project_for_body_transition(",
            "SealedValidateSignProjectionPermit::new()",
            "DurableContinuationEdge::ValidateToSignPrepare",
            "DurableContinuationEdge::ValidateToSignCommit",
            "stage_body_stage_transition(",
            "PreparedSealedValidateSignTransition",
        ] {
            assert!(
                sealed_sign.contains(required),
                "sealed Validate-to-Sign entrypoint omitted {required}"
            );
        }
        for forbidden in [
            "&AdapterEffect",
            "&PendingRuntimeEffectBinding",
            "&DurableBodyReceipt",
            "projection::admission_request",
            "persist_durable_projection",
        ] {
            assert!(
                !sealed_sign.contains(forbidden),
                "sealed Validate-to-Sign entrypoint exposes {forbidden}"
            );
        }
        for sealed in [sealed_no_successor, sealed_report] {
            for forbidden in [
                "&DurableBodyReceipt",
                "&AdapterEffect",
                "&PendingRuntimeEffectBinding",
                "projection::admission_request",
                "candidate.payload =",
                "fn commit(",
                "fn staged(",
            ] {
                assert!(
                    !sealed.contains(forbidden),
                    "sealed terminal Validate entrypoint exposes {forbidden}"
                );
            }
        }
        assert!(!production.contains("prepare_fetch_store_transition"));
        assert!(!production.contains("prepare_store_validate_transition"));
        assert!(!production.contains("prepare_validate_report_transition"));
        assert!(!production.contains("prepare_validate_no_successor_transition"));
        assert!(!production.contains("stage_raw_body_stage_transition"));
        assert!(!production.contains("prepare_ready_validate_apply_transition"));
        assert!(!production.contains("prepare_validate_sign_transition"));
        assert!(!production.contains("fn prepare_body_stage_transition"));
        assert!(!production.contains("projection::admission_request"));
        let bls_tests = source
            .split_once("\n#[cfg(all(test, feature = \"bls\"))]\nmod tests {")
            .map(|(_, tests)| tests)
            .expect("BLS transition tests have one bounded suffix");
        for forbidden in [
            "projection::admission_request",
            "prepare_fetch_store_transition",
            "prepare_store_validate_transition",
            "prepare_validate_apply_transition(",
            ".prepare_body_stage_transition(",
        ] {
            assert!(
                !bls_tests.contains(forbidden),
                "BLS transition fixture reopened raw authority through {forbidden}"
            );
        }
        assert!(bls_tests.contains("prepare_authorized_body_transition("));
        assert!(bls_tests.contains("exact_live_wal_body_successor_candidate_for_test("));
        assert!(production.contains("PreparedSealedBodyStageTransition<'coordinator, 'registry>"));
        assert!(production.contains("_successor: SealedBodyStageSuccessor<'registry>"));
        assert!(production.contains("&'a mut LifecycleCoordinator"));
        assert!(production.contains("DurableContinuationEdge::FetchToStore"));
        assert!(production.contains("DurableContinuationEdge::StoreToValidate"));
        assert!(production.contains("DurableContinuationEdge::ValidateToApply"));
        assert!(production.contains("DurableContinuationEdge::ValidateToSignPrepare"));
        assert!(production.contains("DurableContinuationEdge::ValidateToSignCommit"));
        assert!(production.contains("DurableContinuationEdge::ValidateToInvalidBodyReport"));
        assert!(production.contains("DurableContinuation::AdvancedNoSuccessor"));
        assert!(production.contains("PreparedReadyDurableValidateAdapterPreview"));
        assert!(production.contains("PreparedInvalidBodyReportReplayPreAdmission"));
        assert!(production.contains("PreparedSealedValidateNoSuccessorTransition"));
        assert!(production.contains("PreparedSealedValidateReportTransition"));
        for (permit, linearity, mint) in [
            (
                "pub(super) struct SealedValidateNoSuccessorProjectionPermit",
                "impl Drop for SealedValidateNoSuccessorProjectionLinearity",
                "SealedValidateNoSuccessorProjectionPermit::new()",
            ),
            (
                "pub(in crate::sumeragi) struct SealedInvalidBodyReportProjectionPermit",
                "impl Drop for SealedInvalidBodyReportProjectionLinearity",
                "SealedInvalidBodyReportProjectionPermit::new()",
            ),
            (
                "pub(in crate::sumeragi) struct SealedValidateSignProjectionPermit",
                "impl Drop for SealedValidateSignProjectionLinearity",
                "SealedValidateSignProjectionPermit::new()",
            ),
        ] {
            assert!(production.contains(permit));
            assert!(production.contains(linearity));
            assert_eq!(production.matches(mint).count(), 1);
            assert!(!production.contains(&format!("#[derive(Clone)]\n{permit}")));
            assert!(!production.contains(&format!("#[derive(Copy)]\n{permit}")));
            assert!(!production.contains(&format!("#[derive(Clone, Copy)]\n{permit}")));
        }
        assert!(!production.contains("enum BodyStageTransitionEdge"));
        assert!(!production.contains("pub(super) fn stage_body_stage_transition"));
        for forbidden in [
            "persist_durable_projection",
            "fn commit(",
            "fn staged(",
            "ConcreteLifecycleWorkRegistry",
            "RuntimeEffectOwnership",
            "legacy_ordinal",
        ] {
            assert!(
                !production.contains(forbidden),
                "inert transition acquired forbidden authority: {forbidden}"
            );
        }
        for caller_source in [
            include_str!("v2.rs"),
            include_str!("v2_effects.rs"),
            include_str!("v2_runner.rs"),
            include_str!("v2_worker.rs"),
            include_str!("v2_runtime.rs"),
            include_str!("v2_lifecycle_coordinator.rs"),
        ] {
            for unwired in [
                "prepare_sealed_fetch_store_transition",
                "prepare_sealed_store_validate_transition",
                "prepare_sealed_validate_no_successor_transition",
                "prepare_sealed_validate_report_transition",
            ] {
                assert!(
                    !caller_source.contains(unwired),
                    "body-frame transition became production-wired through {unwired}"
                );
            }
        }

        let publication = production
            .split("pub(super) fn persist_and_publish(")
            .nth(1)
            .and_then(|suffix| suffix.split("\n}\n\n#[cfg(test)]").next())
            .expect("live Validate-to-Sign publication has one bounded body");
        let registry_preflight = publication
            .find("prepare_registry_publication(")
            .expect("registry reservation precedes fsync");
        let ledger_fsync = publication
            .find("persist_exact_staged_successor(&staged)")
            .expect("exact LedgerV1 fsync is mandatory");
        let coordinator_swap = publication
            .find("*coordinator = staged")
            .expect("coordinator swap follows fsync");
        let adapter_swap = publication
            .find("registry.publish_after_ledger_fsync()")
            .expect("registry and adapter publication follows coordinator swap");
        assert!(registry_preflight < ledger_fsync);
        assert!(ledger_fsync < coordinator_swap && coordinator_swap < adapter_swap);
        let post_fsync = &publication[coordinator_swap..];
        for forbidden in [
            "?",
            "return Err",
            "publish_status",
            "persist_durable_projection",
            "persist_exact_staged_successor",
        ] {
            assert!(
                !post_fsync.contains(forbidden),
                "post-fsync publication acquired fallible work through {forbidden}"
            );
        }

        let exact_fsync_callers = production
            .matches(".persist_exact_staged_successor(")
            .count()
            + [
                include_str!("v2.rs"),
                include_str!("v2_effects.rs"),
                include_str!("v2_runner.rs"),
                include_str!("v2_worker.rs"),
                include_str!("v2_runtime.rs"),
                include_str!("v2_lifecycle_concrete_admission.rs"),
                reviewed_lifecycle_work_registry_source_for_test(),
            ]
            .iter()
            .map(|source| {
                source
                    .split("\n#[cfg(test)]\nmod tests {")
                    .next()
                    .expect("caller source has a production prefix")
                    .matches(".persist_exact_staged_successor(")
                    .count()
            })
            .sum::<usize>();
        assert_eq!(
            exact_fsync_callers, 1,
            "the sealed live Validate-to-Sign transaction must be the sole exact-fsync caller"
        );
        let ledger_production = reviewed_lifecycle_ledger_source_for_test()
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("ledger source has one production prefix");
        assert_eq!(
            ledger_production
                .matches(".persist_exact_successor(")
                .count(),
            1,
            "the same staged transaction helper must be the sole exact-store successor caller"
        );
    }
}
