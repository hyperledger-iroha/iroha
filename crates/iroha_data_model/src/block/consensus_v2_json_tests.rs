// JSON wire-contract tests included by `consensus_v2_tests.rs`.


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
        previous_native_settlement_hash: None,
        members: Vec::new(),
        application_block_height: 1,
        application_block_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"json native leaf application",
        )),
        executed_block_wire_hash: Hash::new(b"json native leaf wire"),
    };
    assert_required_nullable_field!(
        NativeAmxApplicationManifestLeafV1,
        native_leaf.clone(),
        "predecessor_descriptor_hash"
    );
    assert_required_nullable_field!(
        NativeAmxApplicationManifestLeafV1,
        native_leaf,
        "previous_native_settlement_hash"
    );

    let commitment = execution_commitment(0x71);
    assert_required_nullable_field!(ExecutionCommitment, commitment, "kagemusha_top_up_root");

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
        "beacon_horizon",
    ] {
        assert_required_nullable_field!(SumeragiV2Status, status.clone(), field);
    }
    let horizon = BeaconHorizonStatusV1 {
        epoch_length_blocks: 0,
        next_required_pulse_height: None,
        active_session_id: None,
        session_covers_next_pulse: false,
        local_provider_ready: false,
    };
    for field in ["next_required_pulse_height", "active_session_id"] {
        assert_required_nullable_field!(BeaconHorizonStatusV1, horizon, field);
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
        stage: SumeragiV2OutboundIntentStage::Retained,
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






#[test]
fn execution_commitment_requires_both_selective_root_count_fields() {
    use iroha_schema::{IntoSchema as _, Metadata};
    let schema = ExecutionCommitment::schema();
    let Metadata::Struct(metadata) = schema.get::<ExecutionCommitment>().unwrap() else {
        panic!("execution commitment must be a struct");
    };
    let commitment = ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
        Hash::new(b"selective parent"),
        Hash::new(b"selective post"),
        Hash::new(b"selective writes"),
        1,
        Hash::new(b"selective wire"),
    );
    let value = norito::json::to_value(&commitment).unwrap();
    for field in [
        "transaction_input_commitment",
        "transaction_output_commitment",
    ] {
        assert!(
            metadata
                .declarations
                .iter()
                .any(|declaration| declaration.name == field)
        );
        assert!(
            value.get(field).is_some(),
            "empty tree is explicit null, never omitted"
        );
        let mut missing = value.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(norito::json::from_value::<ExecutionCommitment>(missing).is_err());
    }
    #[derive(norito::codec::Encode)]
    struct WithoutSelectiveCommitments {
        parent_state_root: Hash,
        post_state_root: Hash,
        ordinary_writes_root: Hash,
        kagemusha_top_up_root: Option<Hash>,
        kagemusha_top_up_count: u32,
        native_amx_application_manifest_version: u16,
        native_amx_application_manifest_root: Hash,
        native_amx_application_manifest_count: u32,
        lane_finality_manifest: Option<MerkleTreeCommitment<LaneFinalityStatement>>,
        merge_carrier: Option<MergeCarrierCommitmentV1>,
        executed_block_wire_len: u64,
        executed_block_wire_hash: Hash,
    }
    let wire = WithoutSelectiveCommitments {
        parent_state_root: commitment.parent_state_root,
        post_state_root: commitment.post_state_root,
        ordinary_writes_root: commitment.ordinary_writes_root,
        kagemusha_top_up_root: commitment.kagemusha_top_up_root,
        kagemusha_top_up_count: commitment.kagemusha_top_up_count,
        native_amx_application_manifest_version: commitment.native_amx_application_manifest_version,
        native_amx_application_manifest_root: commitment.native_amx_application_manifest_root,
        native_amx_application_manifest_count: commitment.native_amx_application_manifest_count,
        lane_finality_manifest: commitment.lane_finality_manifest,
        merge_carrier: commitment.merge_carrier,
        executed_block_wire_len: commitment.executed_block_wire_len,
        executed_block_wire_hash: commitment.executed_block_wire_hash,
    }
    .encode();
    let mut retired = wire.as_slice();
    assert!(<ExecutionCommitment as norito::codec::DecodeAll>::decode_all(&mut retired).is_err());
}
