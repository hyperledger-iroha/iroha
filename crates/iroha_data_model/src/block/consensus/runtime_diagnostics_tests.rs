// Runtime diagnostics fixtures and regression tests included in the parent test module.
fn sample_native_amx_qc(
    phase: NativeAmxPhase,
    source_id: [u8; 32],
    plan_digest: Hash,
    coordinator: (LaneId, DataSpaceId),
    participant: (LaneId, DataSpaceId),
    mut validator_set: Vec<PeerId>,
) -> NativeAmxAttestationQcV2 {
    validator_set.sort();
    validator_set.dedup();
    let (coordinator_lane_id, coordinator_dataspace_id) = coordinator;
    let (participant_lane_id, participant_dataspace_id) = participant;
    let participant_validator_count =
        u32::try_from(validator_set.len()).expect("fixture validator count fits u32");
    let participant_min_quorum = u32::try_from(
        validator_set
            .len()
            .saturating_sub(validator_set.len().saturating_sub(1) / 3)
            .max(1),
    )
    .expect("fixture validator quorum fits u32");
    let validator_set_hash = HashOf::new(&validator_set);
    let validator_set_pops = vec![vec![0x5A; 96]; validator_set.len()];
    let network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"native-amx-model-genesis",
    )));
    let coordinator_lane_incarnation = Hash::new(b"native-amx-model-coordinator");
    let participant_lane_incarnation = Hash::new(
        [
            b"native-amx-model-participant:".as_slice(),
            &participant_lane_id.as_u32().to_be_bytes(),
        ]
        .concat(),
    );
    let coordinator_proposal_hash = Hash::new(b"native-amx-model-proposal");
    let participant_previous_block_descriptor_hash = Some(Hash::new(
        [
            b"native-amx-model-participant-parent:".as_slice(),
            &participant_lane_id.as_u32().to_be_bytes(),
        ]
        .concat(),
    ));
    let mut body = NativeAmxAttestationBodyV2 {
        round: crate::block::consensus_v2::ConsensusRound {
            context_id: crate::block::consensus_v2::HeightContextId(
                HashOf::from_untyped_unchecked(Hash::new(b"native-amx-receipt-context")),
            ),
            height: 42,
            view: 3,
        },
        epoch: 7,
        network_id,
        source_id,
        tx_entrypoint_hash: HashOf::from_untyped_unchecked(Hash::prehashed(source_id)),
        plan_digest,
        phase,
        coordinator_lane_id,
        coordinator_dataspace_id,
        coordinator_lane_incarnation,
        participant_lane_id,
        participant_dataspace_id,
        participant_lane_incarnation,
        participant_previous_block_height: 41,
        participant_previous_block_descriptor_hash,
        participant_lane_block_height: 42,
        participant_lane_block_view: 0,
        participant_proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
        participant_settlement_commitment: Hash::prehashed([0; Hash::LENGTH]),
        participant_validator_set_hash: validator_set_hash,
        participant_validator_count,
        participant_min_quorum,
        authority_context_height: 42,
        planned_coordinator_block_height: 42,
        coordinator_lane_block_view: 3,
        coordinator_proposal_hash,
    };
    body.participant_proposal_hash =
        sample_native_amx_participant_proposal(&body, validator_set.clone()).proposal_hash;
    body.participant_settlement_commitment = body
        .computed_grouped_participant_settlement_commitment(&[body.source_id])
        .expect("single-source test fixture settlement is valid");
    NativeAmxAttestationQcV2::try_new(
        body,
        VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash,
        validator_set,
        validator_set_pops,
        vec![0b0000_0111],
        vec![0xA5; 96],
    )
    .expect("fixture validator set and proofs must align")
}
#[test]
fn native_amx_grouped_receipt_structure_matches_rust_owned_fixture() {
    let document = grouped_native_amx_fixture_document();
    let receipt_group = document
        .pointer("/golden/receipt_group")
        .cloned()
        .expect("fixture contains receipt group");
    let commitment: LaneBlockCommitment =
        norito::json::from_value(receipt_group.clone()).expect("fixture receipt group decodes");
    commitment
        .validate_native_amx_receipts()
        .expect("Rust-owned grouped Native AMX fixture is structurally valid");
    validate_grouped_native_amx_application_evidence(&document)
        .expect("Rust-owned Native AMX application evidence is valid");
    for receipt in &commitment.native_amx_receipts {
        for leg in &receipt.legs {
            assert!(
                !leg.requires_mixed_role_anchor_validation(),
                "golden grouped legs contain their exact current entrypoint"
            );
        }
    }
    for (path, label) in [
        ("", "settlement commitment"),
        ("/native_amx_receipts/0", "receipt"),
        ("/native_amx_receipts/0/legs/0", "leg"),
        (
            "/native_amx_receipts/0/legs/0/participant_proposal",
            "participant proposal",
        ),
        (
            "/native_amx_receipts/0/legs/0/participant_proposal/descriptor",
            "participant descriptor",
        ),
        (
            "/native_amx_receipts/0/legs/0/participant_settlement",
            "participant settlement",
        ),
        (
            "/native_amx_receipts/0/legs/0/participant_settlement/receipts/0",
            "participant settlement receipt",
        ),
        ("/native_amx_receipts/0/legs/0/prepare_qc", "attestation QC"),
        (
            "/native_amx_receipts/0/legs/0/prepare_qc/body",
            "attestation body",
        ),
        (
            "/native_amx_receipts/0/legs/0/prepare_qc/body/phase",
            "attestation phase",
        ),
    ] {
        let mut mutated = receipt_group.clone();
        let target = if path.is_empty() {
            mutated.as_object_mut()
        } else {
            mutated
                .pointer_mut(path)
                .and_then(norito::json::Value::as_object_mut)
        }
        .unwrap_or_else(|| panic!("fixture contains {label} object"));
        target.insert(
            "retired_native_amx_field".to_owned(),
            norito::json::Value::Null,
        );
        assert!(
            norito::json::from_value::<LaneBlockCommitment>(mutated).is_err(),
            "unknown {label} fields must fail exact Native AMX JSON decoding"
        );
    }
    let hint = LaneBlockProposalPayloadHintV1 {
        proposal_height: 42,
        proposal_view: 3,
        proposal_block_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"native-amx-payload-hint-unknown-field",
        )),
    };
    let mut hint_json = norito::json::to_value(&hint).expect("serialize payload hint");
    hint_json
        .as_object_mut()
        .expect("payload hint is an object")
        .insert(
            "retired_native_amx_field".to_owned(),
            norito::json::Value::Null,
        );
    assert!(
        norito::json::from_value::<LaneBlockProposalPayloadHintV1>(hint_json).is_err(),
        "unknown payload-hint fields must fail exact Native AMX JSON decoding"
    );
}
fn npos_diagnostics() -> SumeragiNposDiagnostics {
    SumeragiNposDiagnostics {
        epoch_length_blocks: NonZeroU64::new(100).unwrap(),
        epoch_seed: [0xA5; 32],
        prf_height: 7,
        prf_view: 2,
    }
}
fn diagnostics(npos: Option<SumeragiNposDiagnostics>) -> SumeragiDiagnosticsStatus {
    SumeragiDiagnosticsStatus {
        pipeline_execution: SumeragiPipelineExecutionStatus::default(),
        tx_queue_depth: 0,
        tx_queue_capacity: 1,
        tx_queue_retained_bytes: 0,
        tx_queue_max_retained_bytes: 1,
        tx_queue_saturated: false,
        tx_queue_saturated_by_count: false,
        tx_queue_saturated_by_bytes: false,
        tx_queue_saturated_by_age: false,
        tx_queue_oldest_queued_age_ms: 0,
        npos,
        lane_commitments: Vec::new(),
        dataspace_commitments: Vec::new(),
        lane_settlement_commitments: Vec::new(),
        lane_relay_envelopes: Vec::new(),
        lane_payload_ownerships: Vec::new(),
        committed_lane_blocks: Vec::new(),
        lane_block_sessions: Vec::new(),
        lane_governance_sealed_total: 0,
        lane_governance_sealed_aliases: Vec::new(),
        lane_governance: Vec::new(),
        native_amx_participant_applications: Vec::new(),
        autonomous_lane_executions: Vec::new(),
    }
}
fn native_amx_participant_application(
    lane: u32,
    dataspace: u64,
) -> SumeragiNativeAmxParticipantApplication {
    SumeragiNativeAmxParticipantApplication {
        lane_id: LaneId::new(lane),
        dataspace_id: DataSpaceId::new(dataspace),
        lane_incarnation: {
            let mut bytes = b"native-amx-diagnostics-incarnation".to_vec();
            bytes.extend_from_slice(&lane.to_le_bytes());
            Hash::new(bytes)
        },
        participant_height: 8,
        participant_view: 1,
        predecessor_height: 7,
        predecessor_descriptor_hash: Some(Hash::new(b"native-amx-diagnostics-predecessor")),
        descriptor_hash: Hash::new(b"native-amx-diagnostics-descriptor"),
        proposal_hash: Hash::new(b"native-amx-diagnostics-proposal"),
        settlement_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"native-amx-diagnostics-settlement",
        )),
        source_count: 2,
        application_block_height: Some(15),
        application_block_hash: Some(HashOf::from_untyped_unchecked(Hash::new(
            b"native-amx-diagnostics-application-block",
        ))),
        state: SumeragiNativeAmxParticipantApplicationState::DurablyApplied,
    }
}
fn autonomous_lane_execution(lane: u32, lane_height: u64) -> SumeragiAutonomousLaneExecution {
    SumeragiAutonomousLaneExecution {
        lane_id: LaneId::new(lane),
        dataspace_id: DataSpaceId::new(u64::from(lane)),
        lane_incarnation: Hash::new(
            format!("autonomous-diagnostics-incarnation-{lane}").as_bytes(),
        ),
        lane_block_height: lane_height,
        lane_block_view: 0,
        proposal_height: lane_height,
        proposal_view: Some(0),
        reservation_owner_hash: Hash::new(
            format!("autonomous-diagnostics-owner-{lane}-{lane_height}").as_bytes(),
        ),
        proposal_identity_hash: Hash::new(
            format!("autonomous-diagnostics-slot-{lane}-{lane_height}").as_bytes(),
        ),
        reservation_group_hash: Hash::new(
            format!("autonomous-diagnostics-group-{lane}-{lane_height}").as_bytes(),
        ),
        proposal_hash: Some(Hash::new(
            format!("autonomous-diagnostics-proposal-{lane}-{lane_height}").as_bytes(),
        )),
        descriptor_hash: Some(Hash::new(
            format!("autonomous-diagnostics-descriptor-{lane}-{lane_height}").as_bytes(),
        )),
        executable_payload_hash: Some(Hash::new(b"autonomous-diagnostics-payload")),
        source_bundle_hash: Some(Hash::new(b"autonomous-diagnostics-bundle")),
        merge_entry_hash: None,
        application_block_height: None,
        application_block_hash: None,
        reservation_count: 2,
        transaction_count: 2,
        highest_durable_stage: SumeragiAutonomousLaneExecutionStage::CertifiedBundleDurable,
        stuck_reason: Some(SumeragiAutonomousLaneExecutionStuckReason::AwaitingMergeSelection),
    }
}
#[test]
fn permissioned_diagnostics_omit_npos_shape() {
    let value = norito::json::to_value(&diagnostics(None)).expect("serialize diagnostics");
    assert!(
        value
            .as_object()
            .expect("diagnostics object")
            .get("npos")
            .is_none()
    );
}
#[test]
fn diagnostics_json_rejects_unknown_outer_and_npos_fields() {
    let mut outer = norito::json::to_value(&diagnostics(Some(npos_diagnostics())))
        .expect("serialize diagnostics");
    outer
        .as_object_mut()
        .expect("diagnostics object")
        .insert("unknown".to_owned(), norito::json::Value::from(1_u64));
    assert!(norito::json::from_value::<SumeragiDiagnosticsStatus>(outer).is_err());
    let mut nested = norito::json::to_value(&diagnostics(Some(npos_diagnostics())))
        .expect("serialize diagnostics");
    nested
        .as_object_mut()
        .and_then(|root| root.get_mut("npos"))
        .and_then(norito::json::Value::as_object_mut)
        .expect("NPoS diagnostics object")
        .insert("unknown".to_owned(), norito::json::Value::from(true));
    assert!(norito::json::from_value::<SumeragiDiagnosticsStatus>(nested).is_err());
    let mut missing_autonomous =
        norito::json::to_value(&diagnostics(None)).expect("serialize diagnostics");
    missing_autonomous
        .as_object_mut()
        .expect("diagnostics object")
        .remove("autonomous_lane_executions");
    assert!(
        norito::json::from_value::<SumeragiDiagnosticsStatus>(missing_autonomous).is_err(),
        "the first-release autonomous diagnostics vector is required"
    );
}
#[test]
fn native_amx_participant_diagnostics_roundtrip_and_validate() {
    let mut value = diagnostics(None);
    value.native_amx_participant_applications = vec![
        native_amx_participant_application(3, 8),
        native_amx_participant_application(4, 2),
    ];
    value
        .validate_native_amx_participant_applications()
        .expect("valid ordered diagnostics");
    let encoded = value.encode();
    let mut encoded_input = encoded.as_slice();
    let decoded = SumeragiDiagnosticsStatus::decode_all(&mut encoded_input)
        .expect("decode diagnostics binary roundtrip");
    assert_eq!(decoded, value);
    let json = norito::json::to_value(&value).expect("serialize diagnostics JSON");
    let row = json
        .get("native_amx_participant_applications")
        .and_then(norito::json::Value::as_array)
        .and_then(|rows| rows.first())
        .and_then(norito::json::Value::as_object)
        .expect("Native AMX participant diagnostics row");
    assert_eq!(
        row.get("state").and_then(norito::json::Value::as_str),
        Some("durably_applied")
    );
    let json_roundtrip: SumeragiDiagnosticsStatus =
        norito::json::from_value(json).expect("decode diagnostics JSON roundtrip");
    assert_eq!(json_roundtrip, value);
}
#[test]
fn native_amx_participant_diagnostics_reject_bounds_order_and_geometry() {
    let mut value = diagnostics(None);
    value.native_amx_participant_applications = vec![
        native_amx_participant_application(3, 8);
        SUMERAGI_NATIVE_AMX_PARTICIPANT_APPLICATIONS_MAX
            + 1
    ];
    assert_eq!(
        value.validate_native_amx_participant_applications(),
        Err("Native AMX participant diagnostics vector exceeds its hard limit")
    );
    value.native_amx_participant_applications = vec![
        native_amx_participant_application(4, 2),
        native_amx_participant_application(3, 8),
    ];
    assert_eq!(
        value.validate_native_amx_participant_applications(),
        Err("Native AMX participant diagnostics must be strictly ordered by route and incarnation")
    );
    let mut malformed = native_amx_participant_application(3, 8);
    malformed.predecessor_height = 6;
    assert_eq!(
        malformed.validate(),
        Err("Native AMX participant diagnostics predecessor must be contiguous")
    );
    malformed = native_amx_participant_application(3, 8);
    malformed.source_count = 4_097;
    assert_eq!(
        malformed.validate(),
        Err("Native AMX participant diagnostics source count is out of bounds")
    );
    malformed = native_amx_participant_application(3, 8);
    malformed.application_block_hash = None;
    assert_eq!(
        malformed.validate(),
        Err("Native AMX participant diagnostics application height and hash must appear together")
    );
    malformed = native_amx_participant_application(3, 8);
    malformed.state = SumeragiNativeAmxParticipantApplicationState::CertifiedPendingCarrier;
    assert_eq!(
        malformed.validate(),
        Err("Native AMX participant diagnostics state disagrees with its application block")
    );
    malformed.state = SumeragiNativeAmxParticipantApplicationState::Conflict;
    assert_eq!(
        malformed.validate(),
        Err("Native AMX participant diagnostics state disagrees with its application block")
    );
    malformed.application_block_height = None;
    malformed.application_block_hash = None;
    malformed.state = SumeragiNativeAmxParticipantApplicationState::CommittedEvidencePending;
    assert_eq!(
        malformed.validate(),
        Err("Native AMX participant diagnostics state disagrees with its application block")
    );
    malformed.state = SumeragiNativeAmxParticipantApplicationState::DurablyApplied;
    assert_eq!(
        malformed.validate(),
        Err("Native AMX participant diagnostics state disagrees with its application block")
    );
}
#[test]
fn autonomous_lane_execution_diagnostics_roundtrip_order_and_bound() {
    let mut value = diagnostics(None);
    value.autonomous_lane_executions = vec![
        autonomous_lane_execution(1, 2),
        autonomous_lane_execution(2, 1),
    ];
    value
        .validate_autonomous_lane_executions()
        .expect("ordered autonomous execution diagnostics");
    let encoded = norito::to_bytes(&value).expect("encode diagnostics");
    let decoded: SumeragiDiagnosticsStatus =
        norito::decode_from_bytes(&encoded).expect("decode diagnostics");
    assert_eq!(decoded, value);
    value.autonomous_lane_executions.reverse();
    assert_eq!(
        value.validate_autonomous_lane_executions(),
        Err("autonomous lane execution diagnostics must be strictly ordered by exact identity")
    );
    value.autonomous_lane_executions = (0..=SUMERAGI_AUTONOMOUS_LANE_EXECUTIONS_MAX)
        .map(|index| {
            autonomous_lane_execution(u32::try_from(index).expect("fixture lane fits u32"), 1)
        })
        .collect();
    assert_eq!(
        value.validate_autonomous_lane_executions(),
        Err("autonomous lane execution diagnostics vector exceeds its hard limit")
    );
}
#[test]
fn autonomous_lane_execution_proposal_view_is_honest_at_queue_boundary() {
    let mut reservations = autonomous_lane_execution(1, 1);
    reservations.proposal_view = None;
    reservations.proposal_hash = None;
    reservations.descriptor_hash = None;
    reservations.executable_payload_hash = None;
    reservations.source_bundle_hash = None;
    reservations.highest_durable_stage = SumeragiAutonomousLaneExecutionStage::ReservationsDurable;
    reservations.stuck_reason =
        Some(SumeragiAutonomousLaneExecutionStuckReason::AwaitingExecutablePayload);
    reservations
        .validate()
        .expect("Queue-only reservation evidence has no global proposal view");
    let encoded = norito::to_bytes(&reservations).expect("encode Queue-only row");
    let decoded: SumeragiAutonomousLaneExecution =
        norito::decode_from_bytes(&encoded).expect("decode Queue-only row");
    assert_eq!(decoded, reservations);
    decoded
        .validate()
        .expect("binary Queue-only row retains valid provisional identity");
    let json = norito::json::to_value(&reservations).expect("serialize Queue-only row");
    assert!(
        json.get("proposal_view").is_none(),
        "an unknown proposal view must be omitted, not synthesized as zero"
    );
    reservations.proposal_view = Some(0);
    assert_eq!(
        reservations.validate(),
        Err("autonomous lane reservation diagnostics cannot claim a global proposal view")
    );
    let mut queue_conflict = reservations;
    queue_conflict.proposal_view = None;
    queue_conflict.highest_durable_stage = SumeragiAutonomousLaneExecutionStage::Conflict;
    queue_conflict.stuck_reason =
        Some(SumeragiAutonomousLaneExecutionStuckReason::EvidenceConflict);
    queue_conflict
        .validate()
        .expect("a Queue-only conflict may precede finalized proposal identity");
    let mut payload = autonomous_lane_execution(1, 1);
    payload.proposal_view = None;
    payload.source_bundle_hash = None;
    payload.highest_durable_stage = SumeragiAutonomousLaneExecutionStage::ExecutablePayloadDurable;
    payload.stuck_reason =
        Some(SumeragiAutonomousLaneExecutionStuckReason::AwaitingPayloadAvailability);
    payload
        .validate()
        .expect("a durable unanchored payload may honestly omit its proposal view");
    payload.proposal_view = Some(0);
    payload
        .validate()
        .expect("authenticated proposal view zero remains a valid exact value");
    let json = norito::json::to_value(&payload).expect("serialize anchored payload row");
    assert_eq!(
        json.get("proposal_view")
            .and_then(norito::json::Value::as_u64),
        Some(0)
    );
}
#[test]
fn autonomous_lane_execution_stage_reasons_are_exhaustive_and_stable() {
    use SumeragiAutonomousLaneExecutionStage as Stage;
    use SumeragiAutonomousLaneExecutionStuckReason as Reason;
    let cases = [
        (
            Stage::ReservationsDurable,
            "reservations_durable",
            Some(Reason::AwaitingExecutablePayload),
            Some("awaiting_executable_payload"),
        ),
        (
            Stage::ExecutablePayloadDurable,
            "executable_payload_durable",
            Some(Reason::AwaitingPayloadAvailability),
            Some("awaiting_payload_availability"),
        ),
        (
            Stage::PayloadAvailabilityCertified,
            "payload_availability_certified",
            Some(Reason::AwaitingLaneCertification),
            Some("awaiting_lane_certification"),
        ),
        (
            Stage::LaneCertified,
            "lane_certified",
            Some(Reason::CertifiedBundleUnavailable),
            Some("certified_bundle_unavailable"),
        ),
        (
            Stage::CertifiedBundleDurable,
            "certified_bundle_durable",
            Some(Reason::AwaitingMergeSelection),
            Some("awaiting_merge_selection"),
        ),
        (
            Stage::MergeCandidateDurable,
            "merge_candidate_durable",
            Some(Reason::AwaitingGlobalCarrier),
            Some("awaiting_global_carrier"),
        ),
        (
            Stage::GlobalCarrierCommitted,
            "global_carrier_committed",
            Some(Reason::AwaitingApplicationReceipt),
            Some("awaiting_application_receipt"),
        ),
        (
            Stage::KuraWsvApplicationReceiptDurable,
            "kura_wsv_application_receipt_durable",
            Some(Reason::QueueFinalizationUnverifiable),
            Some("queue_finalization_unverifiable"),
        ),
        (Stage::QueueFinalized, "queue_finalized", None, None),
        (
            Stage::Conflict,
            "conflict",
            Some(Reason::EvidenceConflict),
            Some("evidence_conflict"),
        ),
    ];
    for (stage, stage_label, reason, reason_label) in cases {
        assert_eq!(stage.as_str(), stage_label);
        assert_eq!(stage.expected_stuck_reason(), reason);
        assert_eq!(reason.map(Reason::as_str), reason_label);
        let stage_json = norito::json::to_value(&stage).expect("serialize stage label");
        assert_eq!(stage_json.as_str(), Some(stage_label));
        let stage_roundtrip: Stage =
            norito::json::from_value(stage_json).expect("decode stage label");
        assert_eq!(stage_roundtrip, stage);
        if let Some(reason) = reason {
            let reason_json =
                norito::json::to_value(&reason).expect("serialize stuck-reason label");
            assert_eq!(reason_json.as_str(), reason_label);
            let reason_roundtrip: Reason =
                norito::json::from_value(reason_json).expect("decode stuck-reason label");
            assert_eq!(reason_roundtrip, reason);
        }
    }
}
#[test]
fn autonomous_lane_execution_conflict_is_explicit_and_fail_closed() {
    let mut reservations = autonomous_lane_execution(1, 1);
    reservations.proposal_view = None;
    reservations.proposal_hash = None;
    reservations.descriptor_hash = None;
    reservations.executable_payload_hash = None;
    reservations.source_bundle_hash = None;
    reservations.highest_durable_stage = SumeragiAutonomousLaneExecutionStage::ReservationsDurable;
    reservations.stuck_reason =
        Some(SumeragiAutonomousLaneExecutionStuckReason::AwaitingExecutablePayload);
    reservations
        .validate()
        .expect("reservation-only diagnostics retain an honest provisional identity");
    reservations.proposal_hash = Some(Hash::new(b"unpaired-finalized-proposal"));
    assert_eq!(
        reservations.validate(),
        Err("autonomous lane execution proposal and descriptor hashes must appear together")
    );
    let mut row = autonomous_lane_execution(1, 1);
    row.transaction_count = 4_097;
    row.reservation_count = 4_097;
    assert_eq!(
        row.validate(),
        Err("autonomous lane execution counters are malformed")
    );
    row.transaction_count = 2;
    row.reservation_count = 2;
    row.highest_durable_stage = SumeragiAutonomousLaneExecutionStage::MergeCandidateDurable;
    row.stuck_reason = Some(SumeragiAutonomousLaneExecutionStuckReason::AwaitingGlobalCarrier);
    assert_eq!(
        row.validate(),
        Err("autonomous lane execution evidence does not match its durable stage")
    );
    row.highest_durable_stage = SumeragiAutonomousLaneExecutionStage::Conflict;
    row.stuck_reason = None;
    assert_eq!(
        row.validate(),
        Err("autonomous lane execution conflict requires an evidence-conflict reason")
    );
    row.stuck_reason = Some(SumeragiAutonomousLaneExecutionStuckReason::EvidenceConflict);
    row.reservation_count = 1;
    row.validate().expect("explicit conflict row");
    row.reservation_count = 2;
    row.highest_durable_stage =
        SumeragiAutonomousLaneExecutionStage::KuraWsvApplicationReceiptDurable;
    row.stuck_reason =
        Some(SumeragiAutonomousLaneExecutionStuckReason::QueueFinalizationUnverifiable);
    assert_eq!(
        row.validate(),
        Err("durable autonomous application stage requires a carrier identity")
    );
    row.merge_entry_hash = Some(HashOf::from_untyped_unchecked(Hash::new(
        b"autonomous-diagnostics-merge-entry",
    )));
    row.application_block_height = Some(5);
    row.application_block_hash = Some(HashOf::from_untyped_unchecked(Hash::new(
        b"autonomous-diagnostics-carrier",
    )));
    row.validate().expect("complete durable application row");
    row.highest_durable_stage = SumeragiAutonomousLaneExecutionStage::QueueFinalized;
    row.stuck_reason = None;
    row.validate()
        .expect("independently proven queue-finalized row");
}
