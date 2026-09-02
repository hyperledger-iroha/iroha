// JSON wire-contract tests included by `consensus_v2_tests.rs`.
#[cfg(feature = "json")]
#[test]
fn status_and_consensus_envelope_json_reject_unknown_nested_fields() {
    let context = context(&[1, 1, 1, 1]);
    let mut snapshot = status(&context);
    snapshot.highest_prepare_qc =
        Some(qc(&context, 2, GlobalPhase::Prepare, vec![0, 1, 2]).as_ref());
    let mut top = norito::json::to_value(&snapshot).expect("serialize status");
    top.as_object_mut()
        .expect("status object")
        .insert("unknown".to_owned(), norito::json::Value::Bool(true));
    assert!(norito::json::from_value::<SumeragiV2Status>(top).is_err());
    let mut nested = norito::json::to_value(&snapshot).expect("serialize status");
    nested
        .as_object_mut()
        .expect("status object")
        .get_mut("highest_prepare_qc")
        .and_then(norito::json::Value::as_object_mut)
        .expect("QC reference object")
        .insert("unknown".to_owned(), norito::json::Value::Bool(true));
    assert!(norito::json::from_value::<SumeragiV2Status>(nested).is_err());
    let envelope = ConsensusMessageV2::new(ConsensusMessageV2Payload::PayloadChunk(PayloadChunk {
        manifest_hash: HashOf::from_untyped_unchecked(Hash::new(b"manifest")),
        index: 0,
        bytes: vec![1],
        sender: 0,
        signature: vec![2],
    }));
    let mut nested_envelope = norito::json::to_value(&envelope).expect("serialize nested envelope");
    nested_envelope
        .as_object_mut()
        .expect("envelope object")
        .get_mut("payload")
        .and_then(norito::json::Value::as_object_mut)
        .expect("payload variant object")
        .get_mut("message")
        .and_then(norito::json::Value::as_object_mut)
        .expect("payload message object")
        .insert("unknown".to_owned(), norito::json::Value::Bool(true));
    assert!(
        norito::json::from_value::<ConsensusMessageV2>(nested_envelope).is_err(),
        "nested consensus payload must reject unknown fields"
    );
    let mut envelope_json = norito::json::to_value(&envelope).expect("serialize envelope");
    envelope_json
        .as_object_mut()
        .expect("envelope object")
        .insert("unknown".to_owned(), norito::json::Value::Bool(true));
    assert!(norito::json::from_value::<ConsensusMessageV2>(envelope_json).is_err());
}
#[cfg(feature = "json")]
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the JSON schema audit checks every mandatory nullable consensus slot under one canonical contract"
)]
fn current_consensus_json_requires_explicit_nullable_slots() {
    macro_rules! assert_required_nullable_field {
        ($ty:ty, $value:expr, $field:expr) => {{
            let canonical: $ty = $value;
            let value = norito::json::to_value(&canonical).expect("serialize current layout");
            assert!(
                value.get($field).is_some_and(norito::json::Value::is_null),
                "nullable field `{}` must serialize as an explicit null",
                $field
            );
            assert_eq!(
                norito::json::from_value::<$ty>(value.clone())
                    .expect("decode explicit nullable slot"),
                canonical
            );

            let mut missing = value.clone();
            missing
                .as_object_mut()
                .expect("current consensus layout is an object")
                .remove($field);
            let error = norito::json::from_value::<$ty>(missing)
                .expect_err("omitted nullable consensus slot must reject");
            assert!(
                error
                    .to_string()
                    .contains(&format!("missing field `{}`", $field)),
                "unexpected missing-field diagnostic for `{}`: {error}",
                $field
            );

            let mut unknown = value;
            unknown
                .as_object_mut()
                .expect("current consensus layout is an object")
                .insert("unknown".to_owned(), norito::json::Value::Bool(true));
            assert!(
                norito::json::from_value::<$ty>(unknown).is_err(),
                "{} must reject unknown JSON fields",
                stringify!($ty)
            );
        }};
    }

    let context = context(&[1, 1, 1, 1]);
    assert_required_nullable_field!(HeightContext, context.clone(), "next_epoch_snapshot");
    assert_required_nullable_field!(HeightContext, context.clone(), "parent_commit_qc");
    assert_required_nullable_field!(HeightContext, context.clone(), "snapshot_bootstrap");

    let subject = BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"json genesis subject")),
        payload_hash: Hash::new(b"json genesis payload"),
    };
    assert_required_nullable_field!(BlockSubject, subject, "parent_block_hash");

    let native_leaf = NativeAmxApplicationManifestLeafV1 {
        version: NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
        lane_id: LaneId::new(7),
        dataspace_id: DataSpaceId::new(8),
        lane_incarnation: Hash::new(b"json native leaf incarnation"),
        participant_height: 1,
        participant_view: 0,
        predecessor_height: 0,
        predecessor_descriptor_hash: None,
        descriptor_hash: Hash::new(b"json native leaf descriptor"),
        proposal_hash: Hash::new(b"json native leaf proposal"),
        settlement_hash: HashOf::from_untyped_unchecked(Hash::new(b"json native leaf settlement")),
        members: Vec::new(),
        application_block_height: 1,
        application_block_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"json native leaf application",
        )),
        executed_block_wire_hash: Hash::new(b"json native leaf wire"),
    };
    assert_required_nullable_field!(
        NativeAmxApplicationManifestLeafV1,
        native_leaf,
        "predecessor_descriptor_hash"
    );

    let commitment = execution_commitment(0x71);
    assert_required_nullable_field!(ExecutionCommitment, commitment, "offline_cash_top_up_root");

    let round = round(&context, 0);
    let timeout_vote = TimeoutVote {
        round,
        highest_prepare_qc: None,
        signer: 0,
        signature: vec![0x72; 48],
    };
    assert_required_nullable_field!(TimeoutVote, timeout_vote.clone(), "highest_prepare_qc");
    assert_required_nullable_field!(
        TimeoutVoteSignaturePayload,
        TimeoutVoteSignaturePayload {
            protocol_version: PROTOCOL_VERSION,
            round,
            highest_prepare_qc: None,
        },
        "highest_prepare_qc"
    );
    let timeout_group = TimeoutVoteGroup {
        highest_prepare_qc: None,
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0x73; 48],
    };
    assert_required_nullable_field!(
        TimeoutVoteGroup,
        timeout_group.clone(),
        "highest_prepare_qc"
    );
    let timeout_certificate = TimeoutCertificate {
        round,
        groups: vec![timeout_group],
    };
    assert_required_nullable_field!(
        TimeoutCertificateRef,
        timeout_certificate.as_ref(),
        "highest_prepare_qc"
    );
    assert_required_nullable_field!(
        ParentCommitJustification,
        ParentCommitJustification { certificate: None },
        "certificate"
    );
    assert_required_nullable_field!(
        TimeoutJustification,
        TimeoutJustification {
            timeout_certificate,
            highest_prepare_qc: None,
        },
        "highest_prepare_qc"
    );

    let status = status(&context);
    for field in [
        "locked_prepare_qc",
        "highest_prepare_qc",
        "last_timeout_certificate",
        "pending_persistence_id",
        "last_committed_subject",
        "last_commit_qc",
    ] {
        assert_required_nullable_field!(SumeragiV2Status, status.clone(), field);
    }
    for field in ["last_progress", "blocker"] {
        assert_required_nullable_field!(
            SumeragiV2LivenessStatus,
            SumeragiV2LivenessStatus::default(),
            field
        );
    }
    let timeout_intent = SumeragiV2OutboundIntentStatus {
        kind: SumeragiV2OutboundIntentKind::TimeoutVote,
        round,
        proposal_round: None,
        subject: None,
        execution_commitment: None,
        stage: SumeragiV2OutboundIntentStage::Sent,
    };
    for field in ["proposal_round", "subject", "execution_commitment"] {
        assert_required_nullable_field!(SumeragiV2OutboundIntentStatus, timeout_intent, field);
    }
    assert_required_nullable_field!(
        SumeragiV2QueueStatus,
        SumeragiV2QueueStatus {
            queue: SumeragiV2QueueKind::RuntimeProgress,
            depth: 0,
            capacity: 1,
            oldest_age_ms: None,
            service_debt: 0,
        },
        "oldest_age_ms"
    );
    let qc_response = SumeragiV2QcResponse::default();
    assert_required_nullable_field!(SumeragiV2QcResponse, qc_response, "highest_prepare_qc");
    assert_required_nullable_field!(SumeragiV2QcResponse, qc_response, "locked_prepare_qc");
}

#[cfg(feature = "json")]
#[test]
fn sumeragi_v2_status_json_rejects_every_omitted_current_field() {
    macro_rules! assert_fields_required {
        ($ty:ty, $value:expr, [$($field:literal),+ $(,)?]) => {{
            let canonical: $ty = $value;
            let value = norito::json::to_value(&canonical).expect("serialize current status layout");
            $(
                let mut missing = value.clone();
                missing
                    .as_object_mut()
                    .expect("current status layout is an object")
                    .remove($field);
                let error = norito::json::from_value::<$ty>(missing)
                    .expect_err("omitted current status field must reject");
                assert!(
                    error.to_string().contains(concat!("missing field `", $field, "`")),
                    "unexpected missing-field diagnostic for `{}`: {error}",
                    $field
                );
            )+
        }};
    }

    let context = context(&[1, 1, 1, 1]);
    assert_fields_required!(
        SumeragiV2Status,
        status(&context),
        [
            "protocol_version",
            "node_fingerprint",
            "build_fingerprint",
            "config_fingerprint",
            "restart_required",
            "height_context_id",
            "height",
            "view",
            "phase",
            "leader",
            "locked_prepare_qc",
            "highest_prepare_qc",
            "last_timeout_certificate",
            "body_state",
            "pending_persistence_id",
            "last_committed_height",
            "last_committed_subject",
            "height_context",
            "last_commit_qc",
            "liveness",
        ]
    );
    assert_fields_required!(
        SumeragiV2LivenessStatus,
        SumeragiV2LivenessStatus::default(),
        [
            "generation",
            "prepare_quorums",
            "commit_quorums",
            "timeout_quorums",
            "outbound_intents",
            "work",
            "queues",
            "last_progress",
            "no_progress_age_ms",
            "blocker",
            "ignore_counts",
        ]
    );
    assert_fields_required!(
        SumeragiV2OutboundIntentStatus,
        SumeragiV2OutboundIntentStatus {
            kind: SumeragiV2OutboundIntentKind::TimeoutVote,
            round: round(&context, 0),
            proposal_round: None,
            subject: None,
            execution_commitment: None,
            stage: SumeragiV2OutboundIntentStage::Sent,
        },
        [
            "kind",
            "round",
            "proposal_round",
            "subject",
            "execution_commitment",
            "stage",
        ]
    );
    assert_fields_required!(
        SumeragiV2QueueStatus,
        SumeragiV2QueueStatus {
            queue: SumeragiV2QueueKind::RuntimeProgress,
            depth: 0,
            capacity: 1,
            oldest_age_ms: None,
            service_debt: 0,
        },
        [
            "queue",
            "depth",
            "capacity",
            "oldest_age_ms",
            "service_debt",
        ]
    );
    assert_fields_required!(
        SumeragiV2QcResponse,
        SumeragiV2QcResponse::default(),
        ["highest_prepare_qc", "locked_prepare_qc"]
    );
}
#[cfg(feature = "json")]
#[test]
fn authenticated_consensus_json_rejects_unknown_fields_at_every_signed_layer() {
    macro_rules! assert_unknown_rejected {
        ($ty:ty, $value:expr) => {{
            let mut value = norito::json::to_value(&$value).expect("serialize signed layer");
            value
                .as_object_mut()
                .expect("signed layer is a JSON object")
                .insert("unknown".to_owned(), norito::json::Value::Bool(true));
            assert!(
                norito::json::from_value::<$ty>(value).is_err(),
                "{} must reject unknown JSON fields",
                stringify!($ty)
            );
        }};
    }

    let context = context(&[1, 1, 1, 1]);
    let round = round(&context, 0);
    let subject = subject(0x81);
    let payload = b"body";
    let encoded_chunks = rs16_fixture_chunks(&context, payload);
    let manifest = PayloadManifest::derive(
        &context,
        round,
        subject,
        u64::try_from(payload.len()).expect("fixture payload length fits u64"),
        &encoded_chunks,
    )
    .expect("derive signed-layer manifest");
    let chunk_payload = PayloadChunkSignaturePayload {
        protocol_version: PROTOCOL_VERSION,
        context_id: round.context_id,
        epoch: context.epoch,
        height: round.height,
        view: round.view,
        subject,
        manifest_hash: HashOf::new(&manifest),
        encoding: manifest.layout.encoding,
        index: 0,
        total_chunks: u32::try_from(manifest.chunk_hashes.len())
            .expect("fixture chunk count fits u32"),
        chunk_hash: manifest.chunk_hashes[0],
        sender: 0,
    };
    assert_unknown_rejected!(PayloadChunkSignaturePayload, chunk_payload);

    let proposal = Proposal {
        round,
        proposer: context.leader(round.view),
        subject,
        manifest: manifest.clone(),
        justification: ProposalJustification::ParentCommit(ParentCommitJustification {
            certificate: None,
        }),
        signature: vec![0x82; 48],
    };
    assert_unknown_rejected!(Proposal, proposal.clone());
    assert_unknown_rejected!(ProposalJustification, proposal.justification.clone());

    let timeout_vote = TimeoutVote {
        round,
        highest_prepare_qc: None,
        signer: 0,
        signature: vec![0x83; 48],
    };
    assert_unknown_rejected!(
        SumeragiV2Equivocation,
        SumeragiV2Equivocation::TimeoutVote {
            first: timeout_vote.clone(),
            second: timeout_vote,
        }
    );
    assert_unknown_rejected!(
        CertifiedBodyResponseSignaturePayload,
        CertifiedBodyResponseSignaturePayload {
            protocol_version: PROTOCOL_VERSION,
            request_hash: HashOf::from_untyped_unchecked(Hash::new(b"signed-layer body request",)),
            manifest,
            body_hash: Hash::new(payload),
            responder: context.roster[0].validator.clone(),
        }
    );
    assert_unknown_rejected!(
        CommitCertificateResponseSignaturePayload,
        CommitCertificateResponseSignaturePayload {
            protocol_version: PROTOCOL_VERSION,
            request_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"signed-layer certificate request",
            )),
            certificate: qc(&context, 0, GlobalPhase::Commit, vec![0, 1, 2]),
            responder: context.roster[0].validator.clone(),
        }
    );
}
#[cfg(feature = "json")]
#[test]
fn execution_commitment_json_requires_explicit_finality_and_merge_manifests() {
    use iroha_schema::{IntoSchema as _, Metadata};
    let schema = ExecutionCommitment::schema();
    let Metadata::Struct(metadata) = schema
        .get::<ExecutionCommitment>()
        .expect("execution commitment schema")
    else {
        panic!("execution commitment schema must be a struct");
    };
    let merge_carrier = metadata
        .declarations
        .iter()
        .find(|field| field.name == "merge_carrier")
        .expect("merge carrier schema declaration");
    assert_eq!(
        merge_carrier.ty,
        core::any::TypeId::of::<Option<MergeCarrierCommitmentV1>>()
    );
    let lane_finality_manifest = metadata
        .declarations
        .iter()
        .find(|field| field.name == "lane_finality_manifest")
        .expect("lane finality manifest schema declaration");
    assert_eq!(
        lane_finality_manifest.ty,
        core::any::TypeId::of::<Option<MerkleTreeCommitment<LaneFinalityStatement>>>()
    );
    let carrier_free = ExecutionCommitment::without_offline_cash_top_ups_or_merge_carrier(
        Hash::new(b"json parent"),
        Hash::new(b"json post"),
        Hash::new(b"json ordinary"),
        1,
        Hash::new(b"json executed wire"),
    );
    let with_carrier = ExecutionCommitment {
        merge_carrier: Some(MergeCarrierCommitmentV1::new(
            HashOf::from_untyped_unchecked(Hash::new(b"json merge entry")),
        )),
        ..carrier_free
    };
    for commitment in [carrier_free, with_carrier] {
        let value = norito::json::to_value(&commitment).expect("serialize commitment");
        assert!(value.get("merge_carrier").is_some());
        assert!(value.get("lane_finality_manifest").is_some());
        let decoded = norito::json::from_value::<ExecutionCommitment>(value)
            .expect("explicit merge carrier projection decodes");
        assert_eq!(decoded, commitment);
        assert_eq!(decoded.encode(), commitment.encode());
    }
    let mut missing = norito::json::to_value(&carrier_free).expect("serialize commitment");
    missing
        .as_object_mut()
        .expect("commitment is an object")
        .remove("merge_carrier");
    let error = norito::json::from_value::<ExecutionCommitment>(missing)
        .expect_err("omitted merge carrier must reject");
    assert!(error.to_string().contains("missing field `merge_carrier`"));
    let mut missing = norito::json::to_value(&carrier_free).expect("serialize commitment");
    missing
        .as_object_mut()
        .expect("commitment is an object")
        .remove("lane_finality_manifest");
    let error = norito::json::from_value::<ExecutionCommitment>(missing)
        .expect_err("omitted lane finality manifest must reject");
    assert!(
        error
            .to_string()
            .contains("missing field `lane_finality_manifest`")
    );
}
