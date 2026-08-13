    fn digest(byte: u8) -> LifecycleDigest {
        LifecycleDigest::new([byte; 32])
    }

    fn context() -> LifecycleContext {
        LifecycleContext::new(digest(1), 7)
    }

    fn key(seed: u8, phase: LifecyclePhase) -> LifecycleKey {
        let stage = match phase {
            LifecyclePhase::Serve => LifecycleStageKind::CertifiedServe,
            LifecyclePhase::ProducerTurn => LifecycleStageKind::ProducerTurn,
            _ => panic!("ledger key fixture only covers Serve/ProducerTurn"),
        };
        super::super::replay_authority::exact_record_fixture(context(), stage, seed).key
    }

    fn stage(kind: LifecycleStageKind) -> LifecycleStage {
        LifecycleStage::new(
            kind,
            if kind == LifecycleStageKind::ProducerTurn {
                PredecessorScope::ProducerHandoffBarrier
            } else {
                PredecessorScope::ReadyOrdinalPrefix
            },
        )
    }

    fn owner(first: u128) -> OwnerId {
        OwnerId::new(CausalRoot::new(digest(9)), first)
    }

    fn body_key(
        phase: LifecyclePhase,
        _execution_commitment: Option<LifecycleDigest>,
    ) -> LifecycleKey {
        let stage = match phase {
            LifecyclePhase::Fetch => LifecycleStageKind::FetchBody,
            LifecyclePhase::Store => LifecycleStageKind::StoreBody,
            LifecyclePhase::Validate => LifecycleStageKind::ValidateBody,
            LifecyclePhase::Apply => LifecycleStageKind::ApplyDecision,
            LifecyclePhase::Prepare => LifecycleStageKind::SignPrepareVote,
            LifecyclePhase::Commit => LifecycleStageKind::SignCommitVote,
            LifecyclePhase::DiagnosticInvalidBody => LifecycleStageKind::ReportInvalidBody,
            _ => panic!("ledger body fixture received a non-body phase"),
        };
        super::super::replay_authority::exact_record_fixture(context(), stage, 3).key
    }

    fn body_stage(kind: LifecycleStageKind) -> LifecycleStage {
        LifecycleStage::new(kind, PredecessorScope::Independent)
    }

    pub(super) fn replay_authority_for(
        key: LifecycleKey,
        stage: LifecycleStage,
        payload: DurablePayloadReference,
    ) -> LifecycleReplayAuthorityV1 {
        let seed = u8::try_from(key.round().view()).expect("fixture view fits u8");
        let record_context = LifecycleContext::new(key.context(), key.round().height());
        let case = super::super::replay_authority::exact_record_fixture(
            record_context,
            stage.kind(),
            seed,
        );
        if stage.kind() == LifecycleStageKind::CertifiedServe {
            return case
                .authority
                .terminalized_certified_serve(record_context, key, stage, payload)
                .unwrap_or(case.authority);
        }
        super::super::replay_authority::exact_replay_authority_for_payload_fixture(
            record_context,
            stage.kind(),
            seed,
            payload,
        )
    }

    fn exact_body_payload(stage: LifecycleStageKind) -> DurablePayloadReference {
        super::super::replay_authority::exact_record_fixture(context(), stage, 3).payload
    }

    #[test]
    fn body_frame_reference_roundtrips_and_is_bound_to_the_body_key() {
        let key = body_key(LifecyclePhase::Store, None);
        let payload = exact_body_payload(LifecycleStageKind::StoreBody);
        let encoded = LifecyclePayloadReferenceV1::from_schema(key, payload)
            .expect("exact body reference projects into LedgerV1");
        assert!(encoded.validate());
        assert_eq!(encoded.to_schema(key), Some(payload));

        let foreign_key = LifecycleKey::new(
            key.context(),
            key.round(),
            key.proposal_round(),
            Some(digest(43)),
            LifecyclePhase::Store,
            key.execution_commitment(),
        );
        assert!(LifecyclePayloadReferenceV1::from_schema(foreign_key, payload).is_none());
        assert_eq!(encoded.to_schema(foreign_key), None);

        let record = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            key,
            owner(1),
            1,
            LifecycleWorkClass::Store,
            body_stage(LifecycleStageKind::StoreBody),
            None,
            digest(9),
            payload,
            DurableContinuation::None,
        )
        .expect("body-bound Store record");
        LifecycleLedgerV1::new(context(), 1, vec![record], BTreeMap::new())
            .expect("body-bound Store ledger");

        let validate_key = body_key(LifecyclePhase::Validate, None);
        let validate_reference = DurableBodyFrameReference::new(
            context().id(),
            validate_key
                .proposal_round()
                .expect("Validate proposal round"),
            validate_key.subject().expect("Validate subject"),
            digest(41),
            digest(42),
        );
        let invalid_validate = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            validate_key,
            owner(1),
            1,
            LifecycleWorkClass::Validate,
            body_stage(LifecycleStageKind::ValidateBody),
            Some(TerminalOutcome::Rejected(7)),
            digest(9),
            DurablePayloadReference::BodyFrame(validate_reference),
            DurableContinuation::None,
        )
        .expect("construct invalid terminal Validate fixture");
        assert_invalid_records(1, vec![invalid_validate]);

        let mut corrupted = encoded;
        *corrupted
            .canonical_reference
            .last_mut()
            .expect("body reference has canonical bytes") ^= 1;
        assert!(!corrupted.validate());
        assert_eq!(corrupted.to_schema(key), None);
    }

    #[test]
    fn recovered_validate_parent_matches_only_its_exact_durable_body_frame() {
        let context_hash = Hash::new(b"recovered Validate body-frame context");
        let active_context = LifecycleContext::new(
            LifecycleDigest::new(*context_hash.as_ref()),
            context().height(),
        );
        let (replay, durable) = super::super::replay_authority::exact_body_record_fixture(
            active_context,
            LifecycleStageKind::ValidateBody,
            3,
        );
        let round = durable.round();
        let subject = durable.subject();
        let commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"parent state"),
            Hash::new(b"post state"),
            Hash::new(b"ordinary writes"),
            1,
            Hash::new(b"executed block"),
        );
        let parent = AuthenticatedRecoveredWalValidateLedgerParent {
            key: replay.key,
            owner: owner(1),
            ordinal: 1,
            payload: replay.payload,
            replay_authority: replay.authority,
            inherited_prepare_authority: false,
            wal_identity: RecoveredWalFrameIdentity::for_test(0, 1, [0xA5; 32]),
            tag: EventTag::new(
                round.height,
                round.view,
                crate::sumeragi::v2_core::Generation::new(0),
            ),
            vote: wire::Vote {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Prepare,
                subject,
                execution_commitment: commitment,
                signer: 0,
                signature: Vec::new(),
            },
        };
        assert!(parent.matches_durable_receipt(active_context, &durable));

        let substituted = DurableBodyReceipt::for_test(
            durable.context_id(),
            round,
            subject,
            iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(
                b"substituted recovered Validate manifest",
            )),
        );
        assert!(!parent.matches_durable_receipt(active_context, &substituted));
    }

    fn validate_successor_pair(
        edge: DurableContinuationEdge,
    ) -> (LifecycleLedgerRecordV1, LifecycleLedgerRecordV1) {
        let (child_phase, child_class, child_stage) = match edge {
            DurableContinuationEdge::ValidateToApply => (
                LifecyclePhase::Apply,
                LifecycleWorkClass::Apply,
                LifecycleStageKind::ApplyDecision,
            ),
            DurableContinuationEdge::ValidateToInvalidBodyReport => (
                LifecyclePhase::DiagnosticInvalidBody,
                LifecycleWorkClass::InvalidBodyReport,
                LifecycleStageKind::ReportInvalidBody,
            ),
            DurableContinuationEdge::ValidateToSignPrepare => (
                LifecyclePhase::Prepare,
                LifecycleWorkClass::SignVote,
                LifecycleStageKind::SignPrepareVote,
            ),
            DurableContinuationEdge::ValidateToSignCommit => (
                LifecyclePhase::Commit,
                LifecycleWorkClass::SignVote,
                LifecycleStageKind::SignCommitVote,
            ),
            DurableContinuationEdge::FetchToStore | DurableContinuationEdge::StoreToValidate => {
                panic!("Validate fixture requires a Validate continuation edge")
            }
        };
        let parent_key = body_key(LifecyclePhase::Validate, None);
        let body_frame = exact_body_payload(LifecycleStageKind::ValidateBody);
        let parent = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            parent_key,
            owner(1),
            1,
            LifecycleWorkClass::Validate,
            body_stage(LifecycleStageKind::ValidateBody),
            Some(TerminalOutcome::Advanced),
            digest(9),
            body_frame,
            DurableContinuation::successor(edge, 2),
        )
        .expect("valid advanced Validate ledger row");
        let child_payload = match edge {
            DurableContinuationEdge::ValidateToApply => {
                exact_body_payload(LifecycleStageKind::ApplyDecision)
            }
            DurableContinuationEdge::ValidateToInvalidBodyReport
            | DurableContinuationEdge::ValidateToSignPrepare
            | DurableContinuationEdge::ValidateToSignCommit => DurablePayloadReference::None,
            DurableContinuationEdge::FetchToStore | DurableContinuationEdge::StoreToValidate => {
                unreachable!("Validate fixture excludes pre-Validate edges")
            }
        };
        let child = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            body_key(child_phase, Some(digest(41))),
            owner(1),
            2,
            child_class,
            body_stage(child_stage),
            None,
            digest(9),
            child_payload,
            DurableContinuation::None,
        )
        .expect("valid live Apply ledger row");
        (parent, child)
    }

    fn validate_apply_pair() -> (LifecycleLedgerRecordV1, LifecycleLedgerRecordV1) {
        validate_successor_pair(DurableContinuationEdge::ValidateToApply)
    }

    fn complete_body_pipeline_chain() -> Vec<LifecycleLedgerRecordV1> {
        let commitment = Some(digest(41));
        let body_frame = exact_body_payload(LifecycleStageKind::StoreBody);
        [
            (
                LifecyclePhase::Fetch,
                LifecycleWorkClass::Fetch,
                LifecycleStageKind::FetchBody,
                1,
                Some(TerminalOutcome::Advanced),
                DurableContinuation::successor(DurableContinuationEdge::FetchToStore, 2),
            ),
            (
                LifecyclePhase::Store,
                LifecycleWorkClass::Store,
                LifecycleStageKind::StoreBody,
                2,
                Some(TerminalOutcome::Advanced),
                DurableContinuation::successor(DurableContinuationEdge::StoreToValidate, 3),
            ),
            (
                LifecyclePhase::Validate,
                LifecycleWorkClass::Validate,
                LifecycleStageKind::ValidateBody,
                3,
                Some(TerminalOutcome::Advanced),
                DurableContinuation::successor(DurableContinuationEdge::ValidateToApply, 4),
            ),
            (
                LifecyclePhase::Apply,
                LifecycleWorkClass::Apply,
                LifecycleStageKind::ApplyDecision,
                4,
                None,
                DurableContinuation::None,
            ),
        ]
        .into_iter()
        .map(
            |(phase, work_class, stage_kind, ordinal, terminal, continuation)| {
                let key = body_key(phase, commitment);
                LifecycleLedgerRecordV1::new_exact_replay_fixture(
                    key,
                    owner(1),
                    ordinal,
                    work_class,
                    body_stage(stage_kind),
                    terminal,
                    digest(9),
                    body_frame,
                    continuation,
                )
                .expect("valid complete body-pipeline ledger row")
            },
        )
        .collect()
    }

    fn assert_invalid_records(high_water: u128, records: Vec<LifecycleLedgerRecordV1>) {
        assert!(matches!(
            LifecycleLedgerV1::new(context(), high_water, records, BTreeMap::new()),
            Err(LifecycleLedgerError::InvalidLedger(_))
        ));
    }

    #[test]
    fn durable_body_successor_edges_reject_mixed_or_substituted_frames() {
        let frame_for = |key: LifecycleKey, byte: u8| {
            DurablePayloadReference::BodyFrame(DurableBodyFrameReference::new(
                key.context(),
                key.proposal_round().expect("body proposal round"),
                key.subject().expect("body subject"),
                digest(41),
                digest(byte),
            ))
        };

        let commitment = Some(digest(41));
        let fetch_key = body_key(LifecyclePhase::Fetch, commitment);
        let store_key = body_key(LifecyclePhase::Store, commitment);
        let validate_key = body_key(LifecyclePhase::Validate, commitment);
        let store_frame = frame_for(store_key, 42);
        let foreign_frame = frame_for(validate_key, 43);
        let exact_fetch_frame = exact_body_payload(LifecycleStageKind::StoreBody);
        let fetch = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            fetch_key,
            owner(1),
            1,
            LifecycleWorkClass::Fetch,
            body_stage(LifecycleStageKind::FetchBody),
            Some(TerminalOutcome::Advanced),
            digest(9),
            exact_fetch_frame,
            DurableContinuation::successor(DurableContinuationEdge::FetchToStore, 2),
        )
        .expect("construct BodyFrame-backed Fetch parent");
        let store_child = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            store_key,
            owner(1),
            2,
            LifecycleWorkClass::Store,
            body_stage(LifecycleStageKind::StoreBody),
            None,
            digest(9),
            exact_fetch_frame,
            DurableContinuation::None,
        )
        .expect("construct exact Store child");
        LifecycleLedgerV1::new(
            context(),
            2,
            vec![fetch.clone(), store_child.clone()],
            BTreeMap::new(),
        )
        .expect("Fetch-to-Store preserves the exact body frame");
        let mut payload_free_fetch = fetch.clone();
        payload_free_fetch.payload_reference =
            LifecyclePayloadReferenceV1::from_schema(fetch_key, DurablePayloadReference::None)
                .expect("encode payload-free Fetch negative");
        assert_invalid_records(2, vec![payload_free_fetch, store_child.clone()]);
        let mut foreign_store = store_child;
        foreign_store.payload_reference =
            LifecyclePayloadReferenceV1::from_schema(store_key, frame_for(store_key, 43))
                .expect("encode substituted Store body frame");
        assert_invalid_records(2, vec![fetch, foreign_store]);
        let store = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            store_key,
            owner(1),
            1,
            LifecycleWorkClass::Store,
            body_stage(LifecycleStageKind::StoreBody),
            Some(TerminalOutcome::Advanced),
            digest(9),
            store_frame,
            DurableContinuation::successor(DurableContinuationEdge::StoreToValidate, 2),
        )
        .expect("construct body-bound Store parent");
        let foreign_validate = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            validate_key,
            owner(1),
            2,
            LifecycleWorkClass::Validate,
            body_stage(LifecycleStageKind::ValidateBody),
            None,
            digest(9),
            foreign_frame,
            DurableContinuation::None,
        )
        .expect("construct substituted Validate child");
        assert_invalid_records(2, vec![store.clone(), foreign_validate]);
        let missing_validate = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            validate_key,
            owner(1),
            2,
            LifecycleWorkClass::Validate,
            body_stage(LifecycleStageKind::ValidateBody),
            None,
            digest(9),
            DurablePayloadReference::None,
            DurableContinuation::None,
        )
        .expect("construct mixed Validate child");
        assert_invalid_records(2, vec![store, missing_validate]);

        let (mut validate, mut apply) = validate_apply_pair();
        let validate_key = validate.key().expect("decode Validate key");
        let apply_key = apply.key().expect("decode Apply key");
        validate.payload_reference =
            LifecyclePayloadReferenceV1::from_schema(validate_key, frame_for(validate_key, 44))
                .expect("encode Validate body frame");
        apply.payload_reference =
            LifecyclePayloadReferenceV1::from_schema(apply_key, frame_for(apply_key, 45))
                .expect("encode substituted Apply body frame");
        assert_invalid_records(2, vec![validate, apply]);
    }

    #[test]
    fn payload_free_store_and_apply_rows_are_not_ledger_v1() {
        for (phase, work_class, stage_kind) in [
            (
                LifecyclePhase::Store,
                LifecycleWorkClass::Store,
                LifecycleStageKind::StoreBody,
            ),
            (
                LifecyclePhase::Apply,
                LifecycleWorkClass::Apply,
                LifecycleStageKind::ApplyDecision,
            ),
        ] {
            let record = LifecycleLedgerRecordV1::new_exact_replay_fixture(
                body_key(phase, Some(digest(41))),
                owner(1),
                1,
                work_class,
                body_stage(stage_kind),
                None,
                digest(9),
                DurablePayloadReference::None,
                DurableContinuation::None,
            )
            .expect("construct a locally decodable payload-free body-stage row");
            assert_invalid_records(1, vec![record]);
        }
    }

    fn serve_pair() -> (LifecycleLedgerRecordV1, LifecycleLedgerRecordV1) {
        let pending = super::super::replay_authority::exact_record_fixture(
            context(),
            LifecycleStageKind::CertifiedServe,
            2,
        )
        .payload;
        let DurablePayloadReference::CertifiedServePending {
            request,
            certificate,
        } = pending
        else {
            unreachable!("canonical Serve fixture has pending durable material")
        };
        let serve = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            key(2, LifecyclePhase::Serve),
            owner(1),
            1,
            LifecycleWorkClass::CertifiedServe,
            stage(LifecycleStageKind::CertifiedServe),
            Some(TerminalOutcome::Completed(Some(digest(23)))),
            digest(20),
            DurablePayloadReference::CertifiedServeCompleted {
                request,
                certificate,
                response: digest(23),
            },
            DurableContinuation::None,
        )
        .expect("valid Serve ledger record");
        let producer = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            key(2, LifecyclePhase::ProducerTurn),
            owner(1),
            2,
            LifecycleWorkClass::ProducerTurn,
            stage(LifecycleStageKind::ProducerTurn),
            None,
            digest(20),
            DurablePayloadReference::None,
            DurableContinuation::None,
        )
        .expect("valid producer ledger record");
        (serve, producer)
    }

    #[test]
    fn serve_debt_rejects_individually_valid_foreign_producer_family() {
        let (serve, mut producer) = serve_pair();
        producer.replay_authority =
            super::super::replay_authority::foreign_certified_serve_family_authority_fixture(
                context(),
                LifecycleStageKind::ProducerTurn,
                2,
            );
        assert!(matches!(
            LifecycleLedgerV1::new(
                context(),
                2,
                vec![serve, producer],
                BTreeMap::from([(1, 2)]),
            ),
            Err(LifecycleLedgerError::InvalidLedger(_))
        ));
    }

    #[test]
    fn frame_roundtrip_is_canonical_and_preserves_high_water() {
        let (serve, producer) = serve_pair();
        let ledger = LifecycleLedgerV1::new(
            context(),
            9,
            vec![producer, serve],
            BTreeMap::from([(1, 2)]),
        )
        .expect("valid ledger");
        let frame = encode_frame(&ledger, 1024 * 1024).expect("encode frame");
        let decoded = decode_frame(&frame, 1024 * 1024).expect("decode frame");
        assert_eq!(decoded, ledger);
        assert_eq!(decoded.high_water(), 9);
        assert_eq!(decoded.records()[0].ordinal(), 1);
        assert_eq!(decoded.records()[1].ordinal(), 2);
    }

    #[test]
    fn advanced_validate_roundtrip_authenticates_its_exact_apply_successor() {
        let root = tempfile::tempdir().expect("temporary directory");
        let (parent, child) = validate_apply_pair();
        let ledger = LifecycleLedgerV1::new(context(), 2, vec![parent, child], BTreeMap::new())
            .expect("exact Validate-to-Apply successor is durable");
        let (store, empty) =
            LifecycleLedgerStoreV1::open(root.path(), context()).expect("open ledger store");
        assert!(empty.records().is_empty());
        store.persist(&ledger).expect("persist successor edge");
        let (_, reopened) =
            LifecycleLedgerStoreV1::open(root.path(), context()).expect("reopen successor edge");
        assert_eq!(reopened, ledger);
        assert_eq!(
            reopened.records()[0].continuation(),
            Some(DurableContinuation::successor(
                DurableContinuationEdge::ValidateToApply,
                2,
            ))
        );

        let snapshot = reopened
            .recovery_snapshot(BTreeMap::from([(1, BTreeSet::new()), (2, BTreeSet::new())]))
            .expect("recovery retains the typed successor edge");
        assert_eq!(
            snapshot.records[0].continuation,
            DurableContinuation::successor(DurableContinuationEdge::ValidateToApply, 2)
        );
        assert_eq!(snapshot.records[1].continuation, DurableContinuation::None);
    }

    #[test]
    fn complete_body_pipeline_chain_roundtrips_all_successor_edges() {
        let ledger = LifecycleLedgerV1::new(
            context(),
            4,
            complete_body_pipeline_chain(),
            BTreeMap::new(),
        )
        .expect("all three exact body-pipeline edges form one durable chain");
        assert_eq!(
            ledger
                .records()
                .iter()
                .map(LifecycleLedgerRecordV1::continuation)
                .collect::<Vec<_>>(),
            vec![
                Some(DurableContinuation::successor(
                    DurableContinuationEdge::FetchToStore,
                    2,
                )),
                Some(DurableContinuation::successor(
                    DurableContinuationEdge::StoreToValidate,
                    3,
                )),
                Some(DurableContinuation::successor(
                    DurableContinuationEdge::ValidateToApply,
                    4,
                )),
                Some(DurableContinuation::None),
            ]
        );
        let snapshot = ledger
            .recovery_snapshot((1..=4).map(|ordinal| (ordinal, BTreeSet::new())).collect())
            .expect("complete body-pipeline chain survives recovery projection");
        assert_eq!(
            snapshot
                .records
                .iter()
                .map(|record| record.continuation)
                .collect::<Vec<_>>(),
            vec![
                DurableContinuation::successor(DurableContinuationEdge::FetchToStore, 2),
                DurableContinuation::successor(DurableContinuationEdge::StoreToValidate, 3),
                DurableContinuation::successor(DurableContinuationEdge::ValidateToApply, 4),
                DurableContinuation::None,
            ]
        );
    }

    #[test]
    fn all_validate_continuations_roundtrip_with_canonical_wire_shapes() {
        for edge in [
            DurableContinuationEdge::ValidateToApply,
            DurableContinuationEdge::ValidateToInvalidBodyReport,
            DurableContinuationEdge::ValidateToSignPrepare,
            DurableContinuationEdge::ValidateToSignCommit,
        ] {
            let (parent, child) = validate_successor_pair(edge);
            let ledger = LifecycleLedgerV1::new(context(), 2, vec![parent, child], BTreeMap::new())
                .expect("typed Validate successor edge is valid");
            let frame = encode_frame(&ledger, 1024 * 1024).expect("encode typed continuation");
            let decoded = decode_frame(&frame, 1024 * 1024).expect("decode typed continuation");
            assert_eq!(
                decoded.records()[0].continuation(),
                Some(DurableContinuation::successor(edge, 2))
            );
        }

        let no_successor = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            body_key(LifecyclePhase::Validate, None),
            owner(1),
            1,
            LifecycleWorkClass::Validate,
            body_stage(LifecycleStageKind::ValidateBody),
            Some(TerminalOutcome::Advanced),
            digest(9),
            exact_body_payload(LifecycleStageKind::ValidateBody),
            DurableContinuation::AdvancedNoSuccessor,
        )
        .expect("construct no-successor Validate tombstone");
        let ledger = LifecycleLedgerV1::new(context(), 1, vec![no_successor], BTreeMap::new())
            .expect("Validate may finish without a child");
        assert_eq!(
            ledger.records()[0].continuation(),
            Some(DurableContinuation::AdvancedNoSuccessor)
        );

        let payload_free_no_successor = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            body_key(LifecyclePhase::Validate, None),
            owner(1),
            1,
            LifecycleWorkClass::Validate,
            body_stage(LifecycleStageKind::ValidateBody),
            Some(TerminalOutcome::Advanced),
            digest(9),
            DurablePayloadReference::None,
            DurableContinuation::AdvancedNoSuccessor,
        )
        .expect("the local row shape is checked again by the complete ledger relation");
        assert_invalid_records(1, vec![payload_free_no_successor]);

        let payload_free_live = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            body_key(LifecyclePhase::Validate, None),
            owner(1),
            1,
            LifecycleWorkClass::Validate,
            body_stage(LifecycleStageKind::ValidateBody),
            None,
            digest(9),
            DurablePayloadReference::None,
            DurableContinuation::None,
        )
        .expect("the complete ledger rejects a payload-free live Validate row");
        assert_invalid_records(1, vec![payload_free_live]);
    }

    #[test]
    fn persisted_continuation_rejects_unknown_and_noncanonical_option_shapes() {
        let (mut parent, child) = validate_apply_pair();
        parent.continuation = PersistedDurableContinuationV1 {
            code: PersistedDurableContinuationV1::VALIDATE_TO_APPLY,
            successor_ordinal: None,
        };
        assert_invalid_records(2, vec![parent, child.clone()]);

        let (mut parent, child) = validate_apply_pair();
        parent.continuation = PersistedDurableContinuationV1 {
            code: PersistedDurableContinuationV1::NONE,
            successor_ordinal: Some(2),
        };
        assert_invalid_records(2, vec![parent, child.clone()]);

        let (mut parent, child) = validate_apply_pair();
        parent.continuation = PersistedDurableContinuationV1 {
            code: u8::MAX,
            successor_ordinal: Some(2),
        };
        assert_invalid_records(2, vec![parent, child]);
    }

    #[test]
    fn advanced_validate_rejects_missing_or_foreign_successor_edges() {
        let (mut parent, child) = validate_apply_pair();
        parent.continuation =
            PersistedDurableContinuationV1::from_schema(DurableContinuation::None);
        assert!(matches!(
            LifecycleLedgerV1::new(context(), 2, vec![parent, child.clone()], BTreeMap::new(),),
            Err(LifecycleLedgerError::InvalidLedger(_))
        ));

        let (mut parent, child) = validate_apply_pair();
        parent.continuation = PersistedDurableContinuationV1::from_schema(
            DurableContinuation::successor(DurableContinuationEdge::ValidateToApply, 3),
        );
        assert!(matches!(
            LifecycleLedgerV1::new(context(), 3, vec![parent, child], BTreeMap::new()),
            Err(LifecycleLedgerError::InvalidLedger(_))
        ));

        let (parent, mut foreign_owner) = validate_apply_pair();
        foreign_owner.causal_root = *digest(55).as_bytes();
        foreign_owner.owner_first_ordinal = 2;
        assert!(matches!(
            LifecycleLedgerV1::new(context(), 2, vec![parent, foreign_owner], BTreeMap::new(),),
            Err(LifecycleLedgerError::InvalidLedger(_))
        ));

        let (parent, mut foreign_lineage) = validate_apply_pair();
        foreign_lineage.key.subject = Some(*digest(56).as_bytes());
        assert!(matches!(
            LifecycleLedgerV1::new(context(), 2, vec![parent, foreign_lineage], BTreeMap::new(),),
            Err(LifecycleLedgerError::InvalidLedger(_))
        ));
    }

    #[test]
    fn typed_continuation_rejects_live_backward_and_unauthenticated_edges() {
        let (mut live_parent, child) = validate_apply_pair();
        live_parent.terminal = None;
        assert_invalid_records(2, vec![live_parent, child]);

        let (mut cancelled_parent, child) = validate_apply_pair();
        cancelled_parent.terminal =
            Some(PersistedTerminalV1::from_schema(TerminalOutcome::Cancelled));
        assert_invalid_records(2, vec![cancelled_parent, child]);

        let (mut backward_parent, child) = validate_apply_pair();
        backward_parent.continuation = PersistedDurableContinuationV1::from_schema(
            DurableContinuation::successor(DurableContinuationEdge::ValidateToApply, 1),
        );
        assert_invalid_records(2, vec![backward_parent, child]);

        let (parent, mut foreign_child_source) = validate_apply_pair();
        foreign_child_source.reconstruction_source = *digest(57).as_bytes();
        assert_invalid_records(2, vec![parent, foreign_child_source]);

        let (mut foreign_parent_source, mut foreign_child_source) = validate_apply_pair();
        foreign_parent_source.reconstruction_source = *digest(58).as_bytes();
        foreign_child_source.reconstruction_source = *digest(58).as_bytes();
        assert_invalid_records(2, vec![foreign_parent_source, foreign_child_source]);

        let (mut absent_proposal_parent, mut absent_proposal_child) = validate_apply_pair();
        absent_proposal_parent.key.proposal_height = None;
        absent_proposal_parent.key.proposal_view = None;
        absent_proposal_child.key.proposal_height = None;
        absent_proposal_child.key.proposal_view = None;
        assert_invalid_records(2, vec![absent_proposal_parent, absent_proposal_child]);

        let (parent, mut foreign_scope) = validate_apply_pair();
        foreign_scope.predecessor_code = predecessor_code(PredecessorScope::ReadyOrdinalPrefix);
        assert_invalid_records(2, vec![parent, foreign_scope]);

        let (mut inherited_commitment, mut substituted_commitment) = validate_apply_pair();
        inherited_commitment.key.execution_commitment = Some(*digest(41).as_bytes());
        substituted_commitment.key.execution_commitment = Some(*digest(42).as_bytes());
        assert_invalid_records(2, vec![inherited_commitment, substituted_commitment]);

        let (parent, mut linked_live_apply) = validate_apply_pair();
        linked_live_apply.continuation = PersistedDurableContinuationV1::from_schema(
            DurableContinuation::successor(DurableContinuationEdge::ValidateToApply, 2),
        );
        assert_invalid_records(2, vec![parent, linked_live_apply]);

        let (mut wrong_edge, child) = validate_apply_pair();
        wrong_edge.continuation = PersistedDurableContinuationV1::from_schema(
            DurableContinuation::successor(DurableContinuationEdge::ValidateToSignPrepare, 2),
        );
        assert_invalid_records(2, vec![wrong_edge, child]);

        let mut no_child = validate_apply_pair().0;
        no_child.continuation =
            PersistedDurableContinuationV1::from_schema(DurableContinuation::AdvancedNoSuccessor);
        no_child.reconstruction_source = *digest(58).as_bytes();
        assert_invalid_records(1, vec![no_child]);

        let mut chain = complete_body_pipeline_chain();
        let mut fetch_without_child = chain.remove(0);
        fetch_without_child.continuation =
            PersistedDurableContinuationV1::from_schema(DurableContinuation::AdvancedNoSuccessor);
        assert_invalid_records(1, vec![fetch_without_child]);
    }

    #[test]
    fn first_ledger_directory_creation_fails_closed_until_parent_sync() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let parent = temporary.path().join("fresh-parent");
        let ledger_root = parent.join("ledger");
        let mut injected = false;
        let result = ensure_durable_ledger_directory_with(&ledger_root, &mut |path| {
            if !injected && ledger_root.exists() && path == parent {
                injected = true;
                return Err(LifecycleLedgerError::Io(
                    "injected parent synchronisation failure".to_owned(),
                ));
            }
            sync_ledger_directory(path)
        });
        assert!(matches!(result, Err(LifecycleLedgerError::Io(_))));
        assert!(injected);

        let (_store, ledger) = LifecycleLedgerStoreV1::open(&ledger_root, context())
            .expect("retry synchronises the existing root before exposure");
        assert_eq!(ledger.context(), context());
        assert_eq!(ledger.high_water(), 0);
        assert!(ledger.records().is_empty());
    }

    #[test]
    fn store_roundtrip_rejects_corrupt_and_foreign_frames() {
        let root = tempfile::tempdir().expect("temporary directory");
        let (store, empty) =
            LifecycleLedgerStoreV1::open(root.path(), context()).expect("open empty store");
        assert_eq!(empty, LifecycleLedgerV1::empty(context()));
        let (serve, producer) = serve_pair();
        let ledger = LifecycleLedgerV1::new(
            context(),
            2,
            vec![serve, producer],
            BTreeMap::from([(1, 2)]),
        )
        .expect("valid ledger");
        store.persist(&ledger).expect("persist ledger");
        let (_, loaded) =
            LifecycleLedgerStoreV1::open(root.path(), context()).expect("reload ledger");
        assert_eq!(loaded, ledger);

        let mut frame = fs::read(root.path().join(LEDGER_FILE)).expect("ledger frame");
        *frame.last_mut().expect("nonempty frame") ^= 0x80;
        fs::write(root.path().join(LEDGER_FILE), frame).expect("corrupt fixture");
        assert!(matches!(
            LifecycleLedgerStoreV1::open(root.path(), context()),
            Err(LifecycleLedgerError::InvalidFrame(_))
        ));
    }

    #[test]
    fn durable_repair_receipt_reloads_the_current_store_frame() {
        let root = tempfile::tempdir().expect("temporary directory");
        let (store, empty) =
            LifecycleLedgerStoreV1::open(root.path(), context()).expect("open empty store");
        assert!(empty.records().is_empty());
        let (serve, producer) = serve_pair();
        let ledger = LifecycleLedgerV1::new(
            context(),
            2,
            vec![serve.clone(), producer.clone()],
            BTreeMap::from([(1, 2)]),
        )
        .expect("valid receipt frame");
        store.persist(&ledger).expect("persist receipt frame");
        let frame = encode_frame(&ledger, store.max_frame_bytes).expect("encode receipt frame");
        let receipt = DurableWalVoteLedgerRepairReceipt {
            store_path: store.path.clone(),
            context: context(),
            parent_key: serve.key().expect("Serve key"),
            child_key: producer.key().expect("producer key"),
            edge: DurableContinuationEdge::ValidateToSignPrepare,
            child_ordinal: 2,
            ledger_frame_hash: LifecycleDigest::new(Hash::new(frame).into()),
        };
        assert!(receipt.belongs_to(&store));

        let replaced = LifecycleLedgerV1::new(
            context(),
            3,
            vec![serve, producer],
            BTreeMap::from([(1, 2)]),
        )
        .expect("valid later frame");
        store.persist(&replaced).expect("replace receipt frame");
        assert!(
            !receipt.belongs_to(&store),
            "a receipt for earlier bytes cannot authorize a later same-path frame"
        );
    }

    #[test]
    fn store_discards_regular_temp_residue_and_rejects_nonregular_temp_paths() {
        let root = tempfile::tempdir().expect("temporary directory");
        let (store, empty) =
            LifecycleLedgerStoreV1::open(root.path(), context()).expect("open empty store");
        let temporary = root.path().join(LEDGER_FILE).with_extension("norito.tmp");
        fs::write(&temporary, b"interrupted ledger write").expect("temporary crash residue");
        store
            .persist(&empty)
            .expect("regular crash residue is safely replaced");
        assert!(!temporary.exists());

        fs::create_dir(&temporary).expect("nonregular temporary path");
        assert!(matches!(
            store.persist(&empty),
            Err(LifecycleLedgerError::InvalidFrame(_))
        ));
    }

    #[test]
    fn malformed_owner_and_producer_debt_are_rejected() {
        let (serve, mut producer) = serve_pair();
        producer.owner_first_ordinal = 2;
        assert!(matches!(
            LifecycleLedgerV1::new(
                context(),
                2,
                vec![serve.clone(), producer],
                BTreeMap::from([(1, 2)]),
            ),
            Err(LifecycleLedgerError::InvalidLedger(_))
        ));
        let (_, producer) = serve_pair();
        assert!(matches!(
            LifecycleLedgerV1::new(
                context(),
                3,
                vec![serve, producer],
                BTreeMap::from([(1, 3)]),
            ),
            Err(LifecycleLedgerError::InvalidLedger(_))
        ));

        let (serve, mut producer) = serve_pair();
        producer.predecessor_code = predecessor_code(PredecessorScope::ReadyOrdinalPrefix);
        assert!(matches!(
            LifecycleLedgerV1::new(
                context(),
                2,
                vec![serve, producer],
                BTreeMap::from([(1, 2)]),
            ),
            Err(LifecycleLedgerError::InvalidLedger(_))
        ));

        let (serve, mut producer) = serve_pair();
        producer.reconstruction_source = *digest(99).as_bytes();
        assert!(matches!(
            LifecycleLedgerV1::new(
                context(),
                2,
                vec![serve, producer],
                BTreeMap::from([(1, 2)]),
            ),
            Err(LifecycleLedgerError::InvalidLedger(_))
        ));

        let (serve, mut producer) = serve_pair();
        producer.key = PersistedLifecycleKeyV1::from_schema(key(3, LifecyclePhase::ProducerTurn));
        assert!(matches!(
            LifecycleLedgerV1::new(
                context(),
                2,
                vec![serve, producer],
                BTreeMap::from([(1, 2)]),
            ),
            Err(LifecycleLedgerError::InvalidLedger(_))
        ));
    }

    #[test]
    fn completed_and_cancelled_atomic_pairs_are_valid_without_debt() {
        let (mut serve, producer) = serve_pair();
        serve.terminal = None;
        serve.payload_reference =
            LifecyclePayloadReferenceV1::certified_serve_pending(digest(2), digest(21), digest(22));
        LifecycleLedgerV1::new(
            context(),
            2,
            vec![serve, producer],
            BTreeMap::from([(1, 2)]),
        )
        .expect("live atomic pair");

        let (serve, mut producer) = serve_pair();
        producer.terminal = Some(PersistedTerminalV1::from_schema(TerminalOutcome::Advanced));
        LifecycleLedgerV1::new(context(), 2, vec![serve, producer], BTreeMap::new())
            .expect("completed atomic pair");

        let (mut serve, mut producer) = serve_pair();
        serve.terminal = Some(PersistedTerminalV1::from_schema(TerminalOutcome::Cancelled));
        serve.payload_reference = LifecyclePayloadReferenceV1::certified_serve_negative(
            digest(2),
            digest(21),
            digest(22),
            DurableServeNegativeOutcome::Cancelled,
        );
        producer.terminal = Some(PersistedTerminalV1::from_schema(TerminalOutcome::Cancelled));
        LifecycleLedgerV1::new(context(), 2, vec![serve, producer], BTreeMap::new())
            .expect("cancelled atomic pair");
    }

    #[test]
    fn negative_terminal_kinds_are_not_interchangeable() {
        let (mut serve, producer) = serve_pair();
        serve.terminal = Some(PersistedTerminalV1::from_schema(TerminalOutcome::Rejected(
            7,
        )));
        serve.payload_reference = LifecyclePayloadReferenceV1::certified_serve_negative(
            digest(2),
            digest(21),
            digest(22),
            DurableServeNegativeOutcome::Failed(7),
        );
        assert!(matches!(
            LifecycleLedgerV1::new(
                context(),
                2,
                vec![serve, producer],
                BTreeMap::from([(1, 2)]),
            ),
            Err(LifecycleLedgerError::InvalidLedger(_))
        ));
    }

    #[test]
    fn one_signed_serve_request_cannot_own_two_lifecycle_pairs() {
        let (serve, producer) = serve_pair();
        let (mut duplicate_serve, mut duplicate_producer) = serve_pair();
        duplicate_serve.key = PersistedLifecycleKeyV1::from_schema(key(4, LifecyclePhase::Serve));
        duplicate_serve.causal_root = *digest(10).as_bytes();
        duplicate_serve.owner_first_ordinal = 3;
        duplicate_serve.ordinal = 3;
        duplicate_serve.payload_reference = LifecyclePayloadReferenceV1::certified_serve_completed(
            digest(4),
            digest(21),
            digest(22),
            digest(23),
        );
        duplicate_producer.key =
            PersistedLifecycleKeyV1::from_schema(key(4, LifecyclePhase::ProducerTurn));
        duplicate_producer.causal_root = *digest(10).as_bytes();
        duplicate_producer.owner_first_ordinal = 3;
        duplicate_producer.ordinal = 4;

        assert!(matches!(
            LifecycleLedgerV1::new(
                context(),
                4,
                vec![serve, producer, duplicate_serve, duplicate_producer],
                BTreeMap::from([(1, 2), (3, 4)]),
            ),
            Err(LifecycleLedgerError::InvalidLedger(_))
        ));
    }

    #[test]
    fn orphan_serve_or_producer_records_are_rejected() {
        let (serve, producer) = serve_pair();
        assert!(matches!(
            LifecycleLedgerV1::new(context(), 2, vec![serve], BTreeMap::new()),
            Err(LifecycleLedgerError::InvalidLedger(_))
        ));
        assert!(matches!(
            LifecycleLedgerV1::new(context(), 2, vec![producer], BTreeMap::new()),
            Err(LifecycleLedgerError::InvalidLedger(_))
        ));
    }

    #[test]
    fn opaque_or_noncanonical_certified_serve_references_are_rejected() {
        let (mut serve, producer) = serve_pair();
        serve.payload_reference.canonical_reference = vec![1, 2, 3];
        let digest = Hash::new(&serve.payload_reference.canonical_reference);
        serve
            .payload_reference
            .digest
            .copy_from_slice(digest.as_ref());

        assert!(matches!(
            LifecycleLedgerV1::new(
                context(),
                2,
                vec![serve, producer],
                BTreeMap::from([(1, 2)]),
            ),
            Err(LifecycleLedgerError::InvalidLedger(_))
        ));
    }
