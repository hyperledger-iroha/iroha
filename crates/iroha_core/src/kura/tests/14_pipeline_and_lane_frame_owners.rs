// Exact declared frame owners for the included Kura pipeline/lane artifact definitions.
// Included by kura::tests to reuse the established valid artifact fixtures.
fn assert_pipeline_artifact_frame_bytes<T>(value: &T) -> T
where
    T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>,
{
    let frame = norito::encode_canonical(value).expect("encode pipeline artifact owner");
    assert_eq!(frame[6..22], norito::schema::identity::frame_hash::<T>());
    let decoded = norito::decode_canonical::<T>(&frame).expect("decode pipeline artifact owner");
    assert_eq!(
        norito::encode_canonical(&decoded).expect("re-encode pipeline artifact"),
        frame
    );
    let mut wrong_owner = frame.clone();
    wrong_owner[6] ^= 1;
    assert!(matches!(
        norito::decode_canonical::<T>(&wrong_owner),
        Err(norito::Error::SchemaMismatch)
    ));
    assert!(norito::decode_canonical::<T>(&frame[..frame.len() - 1]).is_err());
    let mut trailing = frame;
    trailing.push(0);
    assert!(norito::decode_canonical::<T>(&trailing).is_err());
    decoded
}

#[test]
fn pipeline_and_fastpq_owner_frames_preserve_recovery_metadata() {
    use crate::private_settlement::global_state::tests::assert_private_settlement_frame_v1 as check;
    let block = DummyBlocks::new().next();
    let tx_hash = HashOf::from_untyped_unchecked(Hash::new(b"pipeline-frame-entrypoint"));
    let tx = PipelineTxSnapshot::compact(tx_hash, 3, 7);
    let decoded = assert_pipeline_artifact_frame_bytes(&tx);
    assert_eq!(
        (
            decoded.hash,
            decoded.reads,
            decoded.writes,
            decoded.read_count,
            decoded.write_count
        ),
        (
            tx.hash,
            tx.reads.clone(),
            tx.writes.clone(),
            tx.read_count,
            tx.write_count
        )
    );
    let proof = sample_fastpq_snapshot(1, block.hash(), 8);
    check(&proof, "iroha_core::kura::FastpqProofSnapshot");
    let mut sidecar = PipelineRecoverySidecar::new(
        1,
        block.hash(),
        PipelineDagSnapshot {
            fingerprint: [0x42; 32],
            key_count: 10,
        },
        vec![tx],
    );
    sidecar.fastpq_proofs.push(proof);
    let decoded = assert_pipeline_artifact_frame_bytes(&sidecar);
    assert_eq!(decoded.to_json_value(), sidecar.to_json_value());
    let frame = norito::encode_canonical(&sidecar).expect("encode pipeline sidecar");
    assert!(matches!(
        norito::decode_canonical::<PipelineTxSnapshot>(&frame),
        Err(norito::Error::SchemaMismatch)
    ));
}

#[test]
fn autonomous_payload_frame_owners_preserve_exact_attempt_and_process_identity() {
    use crate::private_settlement::global_state::tests::assert_private_settlement_frame_v1 as check;
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload) =
        autonomous_lane_payload_for_kura(LaneId::SINGLE, DataSpaceId::UNIVERSAL, 1, &signer);
    payload
        .validate(network_id, epoch)
        .expect("existing payload fixture validates");
    let artifact = AutonomousLaneBlockArtifact::new(payload.clone());
    let latest = AutonomousLaneBlockLatestAttemptV1::from_payload(&payload);
    let view = AutonomousLaneBlockViewState::from_artifact(&artifact);
    let entrypoint_hash = payload
        .origin_proposal
        .descriptor
        .accepted_transaction_hashes[0];
    let claim = AutonomousLaneEntrypointClaimV1::new(&payload, entrypoint_hash);
    let retirement = AutonomousLaneSlotRetirementV1::from_payload(&payload);
    check(&artifact, "iroha_core::kura::AutonomousLaneBlockArtifact");
    check(
        &latest,
        "iroha_core::kura::AutonomousLaneBlockLatestAttemptV1",
    );
    check(&view, "iroha_core::kura::AutonomousLaneBlockViewState");
    check(&claim, "iroha_core::kura::AutonomousLaneEntrypointClaimV1");
    check(
        &retirement,
        "iroha_core::kura::AutonomousLaneSlotRetirementV1",
    );
    assert!(retirement.matches_payload(&payload));
    let record = AutonomousLifecycleProcessGenerationRecordV1::new(
        network_id,
        PeerId::new(signer.public_key().clone()),
        1,
    )
    .expect("validated process generation fixture");
    record
        .validate_structure()
        .expect("process generation body hash matches");
    check(
        &record.body,
        "iroha_core::kura::AutonomousLifecycleProcessGenerationBodyV1",
    );
    check(
        &record,
        "iroha_core::kura::AutonomousLifecycleProcessGenerationRecordV1",
    );
    let mut tampered = record.clone();
    tampered.body.generation += 1;
    assert!(
        tampered.validate_structure().is_err(),
        "framing cannot authorize a changed process generation"
    );
    let record_frame = norito::encode_canonical(&record).expect("encode process record");
    assert!(matches!(
        norito::decode_canonical::<AutonomousLifecycleProcessGenerationBodyV1>(&record_frame),
        Err(norito::Error::SchemaMismatch)
    ));

    let mut evidence = AutonomousLifecycleLosingRetirementCustodyEvidenceV1 {
        version: 1,
        height_context_id: HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"custody-frame-context",
        ))),
        retirement_hash: HashOf::new(&retirement),
        origin_proposal_hash: payload.origin_proposal.proposal_hash,
        executable_payload_hash: payload.payload_hash,
    };
    let frame = norito::encode_canonical(&evidence).expect("encode custody evidence owner");
    assert_eq!(
        frame[6..22],
        norito::schema::identity::frame_hash::<AutonomousLifecycleLosingRetirementCustodyEvidenceV1>(
        )
    );
    evidence.origin_proposal_hash = Hash::new(b"substituted-custody-origin");
    assert_ne!(
        norito::encode_canonical(&evidence).expect("encode changed custody evidence"),
        frame
    );
}

#[test]
fn pipeline_and_lane_artifact_frame_contracts_use_declared_owners() {
    fn check<T: norito::NoritoSerialize>(name: &str) {
        assert_eq!(T::nominal_name(), name);
        assert_eq!(T::frame_name(), name);
    }
    fn decoded<T>(name: &str)
    where
        T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>,
    {
        check::<T>(name);
    }
    decoded::<AutonomousLaneBlockArtifact>("iroha_core::kura::AutonomousLaneBlockArtifact");
    decoded::<AutonomousLaneBlockLatestAttemptV1>(
        "iroha_core::kura::AutonomousLaneBlockLatestAttemptV1",
    );
    decoded::<AutonomousLaneBlockViewState>("iroha_core::kura::AutonomousLaneBlockViewState");
    decoded::<AutonomousLaneEntrypointClaimV1>("iroha_core::kura::AutonomousLaneEntrypointClaimV1");
    decoded::<AutonomousLaneSlotRetirementV1>("iroha_core::kura::AutonomousLaneSlotRetirementV1");
    decoded::<AutonomousLifecycleAttemptBindingV1>(
        "iroha_core::kura::AutonomousLifecycleAttemptBindingV1",
    );
    decoded::<AutonomousLifecycleBootstrapBodyV1>(
        "iroha_core::kura::AutonomousLifecycleBootstrapBodyV1",
    );
    decoded::<AutonomousLifecycleBootstrapV1>("iroha_core::kura::AutonomousLifecycleBootstrapV1");
    check::<AutonomousLifecycleCanonicalHistoricalRecoveryCustodyEvidenceV1>(
        "iroha_core::kura::AutonomousLifecycleCanonicalHistoricalRecoveryCustodyEvidenceV1",
    );
    check::<AutonomousLifecycleCanonicalCarrierRepairCustodyEvidenceV1>(
        "iroha_core::kura::AutonomousLifecycleCanonicalCarrierRepairCustodyEvidenceV1",
    );
    decoded::<AutonomousLifecycleCursorUnsignedV1>(
        "iroha_core::kura::AutonomousLifecycleCursorUnsignedV1",
    );
    decoded::<AutonomousLifecycleCursorV1>("iroha_core::kura::AutonomousLifecycleCursorV1");
    check::<AutonomousLifecycleHistoricalQcResponseCustodyEvidenceV1>(
        "iroha_core::kura::AutonomousLifecycleHistoricalQcResponseCustodyEvidenceV1",
    );
    check::<AutonomousLifecycleLosingRetirementCustodyEvidenceV1>(
        "iroha_core::kura::AutonomousLifecycleLosingRetirementCustodyEvidenceV1",
    );
    decoded::<AutonomousLifecycleProcessGenerationBodyV1>(
        "iroha_core::kura::AutonomousLifecycleProcessGenerationBodyV1",
    );
    decoded::<AutonomousLifecycleProcessGenerationRecordV1>(
        "iroha_core::kura::AutonomousLifecycleProcessGenerationRecordV1",
    );
    check::<AutonomousLifecycleProtectedCarrierReceiveCustodyEvidenceV1>(
        "iroha_core::kura::AutonomousLifecycleProtectedCarrierReceiveCustodyEvidenceV1",
    );
    decoded::<AutonomousLifecycleTerminalOutcomeBodyV1>(
        "iroha_core::kura::AutonomousLifecycleTerminalOutcomeBodyV1",
    );
    decoded::<AutonomousLifecycleTerminalOutcomeV1>(
        "iroha_core::kura::AutonomousLifecycleTerminalOutcomeV1",
    );
    decoded::<CertifiedLaneBlockArtifact>("iroha_core::kura::CertifiedLaneBlockArtifact");
    decoded::<FastpqProofSnapshot>("iroha_core::kura::FastpqProofSnapshot");
    decoded::<LaneBlockApplicationReceiptArtifact>(
        "iroha_core::kura::LaneBlockApplicationReceiptArtifact",
    );
    decoded::<LaneBlockArtifact>("iroha_core::kura::LaneBlockArtifact");
    decoded::<LaneBlockExecutionInputArtifact>("iroha_core::kura::LaneBlockExecutionInputArtifact");
    decoded::<LaneBlockExecutionPreflightArtifact>(
        "iroha_core::kura::LaneBlockExecutionPreflightArtifact",
    );
    decoded::<LaneMergeApplicationFrontierV1>("iroha_core::kura::LaneMergeApplicationFrontierV1");
    decoded::<LatestCertifiedLaneBlockFrontierV1>(
        "iroha_core::kura::LatestCertifiedLaneBlockFrontierV1",
    );
    decoded::<PipelineRecoverySidecar>("iroha_core::kura::PipelineRecoverySidecar");
    decoded::<PipelineTxSnapshot>("iroha_core::kura::PipelineTxSnapshot");
    decoded::<AutonomousLaneMergeBundleV1>("iroha_core::kura::AutonomousLaneMergeBundleV1");
    decoded::<BoundProgressAppendIntentV1>("iroha_core::kura::BoundProgressAppendIntentV1");
    decoded::<CanonicalAutonomousLaneReplicaV1>(
        "iroha_core::kura::CanonicalAutonomousLaneReplicaV1",
    );
    decoded::<NativeAmxParticipantApplicationManifestArtifactV1>(
        "iroha_core::kura::NativeAmxParticipantApplicationManifestArtifactV1",
    );
    decoded::<NativeAmxEvidencePruneIntentV2>("iroha_core::kura::NativeAmxEvidencePruneIntentV2");
    decoded::<NativeAmxParticipantApplicationReceiptArtifact>(
        "iroha_core::kura::NativeAmxParticipantApplicationReceiptArtifact",
    );
    decoded::<NativeAmxParticipantReceiptLatestIndexV2>(
        "iroha_core::kura::NativeAmxParticipantReceiptLatestIndexV2",
    );
    decoded::<KuraPruneIntentV3>("iroha_core::kura::KuraPruneIntentV3");
    decoded::<MergeLedgerCarrierRecord>("iroha_core::kura::MergeLedgerCarrierRecord");
}

#[test]
fn canonical_replica_frame_owners_preserve_validated_bundle_binding() {
    use crate::private_settlement::global_state::tests::assert_private_settlement_frame_v1 as check;
    let fixture = canonical_autonomous_replica_fixture();
    let descriptor = &fixture.certified.proposal.descriptor;
    let entry = fixture
        .lane_config
        .entry(descriptor.lane_id)
        .expect("lane entry");
    let (data_path, _) =
        Kura::canonical_autonomous_lane_replica_paths_for_entry(entry, &fixture.kura.store_root);
    let bytes = fs::read(data_path).expect("read canonical replica fixture");
    let record = norito::decode_canonical::<CanonicalAutonomousLaneReplicaV1>(&bytes)
        .expect("decode current canonical replica owner");
    Kura::validate_canonical_autonomous_lane_replica_structure(&record)
        .expect("existing canonical replica fixture validates");
    check(
        &record,
        "iroha_core::kura::CanonicalAutonomousLaneReplicaV1",
    );
    check(
        &record.bundle,
        "iroha_core::kura::AutonomousLaneMergeBundleV1",
    );
    assert_eq!(record.bundle, fixture.source.bundle);
    let mut changed = record.clone();
    changed.carrier_height += 1;
    assert!(Kura::validate_canonical_autonomous_lane_replica_structure(&changed).is_err());
    assert!(matches!(
        norito::decode_canonical::<AutonomousLaneMergeBundleV1>(&bytes),
        Err(norito::Error::SchemaMismatch)
    ));
}

#[test]
fn native_amx_frame_owners_preserve_manifest_receipt_and_prune_bindings() {
    use crate::private_settlement::global_state::tests::assert_private_settlement_frame_v1 as check;
    let (_temp, _config, _lanes, kura) = temporary_kura_fixture();
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("lane storage entry");
    let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2]);
    let receipt = &receipts[1];
    let manifest_path =
        Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 2);
    let bytes = fs::read(manifest_path).expect("read current manifest artifact");
    let manifest =
        norito::decode_canonical::<NativeAmxParticipantApplicationManifestArtifactV1>(&bytes)
            .expect("decode manifest artifact owner");
    Kura::validate_native_amx_participant_application_manifest_artifact(&manifest)
        .expect("existing manifest fixture validates");
    assert_eq!(receipt.manifest_artifact_hash, HashOf::new(&manifest));
    let latest = NativeAmxParticipantReceiptLatestIndexV2::from_receipt(receipt);
    assert!(latest.matches_receipt(receipt));
    let intent = native_amx_prune_intent_for_test(&kura, &entry, receipt, &[1]);
    check(
        &manifest,
        "iroha_core::kura::NativeAmxParticipantApplicationManifestArtifactV1",
    );
    check(
        receipt,
        "iroha_core::kura::NativeAmxParticipantApplicationReceiptArtifact",
    );
    check(
        &latest,
        "iroha_core::kura::NativeAmxParticipantReceiptLatestIndexV2",
    );
    check(&intent, "iroha_core::kura::NativeAmxEvidencePruneIntentV2");
    assert_eq!(intent.protected_latest.identity, latest);
    assert_eq!(
        intent.protected_latest.receipt_artifact_hash,
        HashOf::new(receipt)
    );
    let mut changed = receipt.clone();
    changed.executed_block_wire_hash = Hash::new(b"changed frame receipt wire");
    assert!(!latest.matches_receipt(&changed));
    assert!(matches!(
        norito::decode_canonical::<NativeAmxParticipantApplicationReceiptArtifact>(&bytes),
        Err(norito::Error::SchemaMismatch)
    ));
}

#[test]
fn prune_and_merge_carrier_frame_owners_preserve_retained_coordinates() {
    use crate::private_settlement::global_state::tests::assert_private_settlement_frame_v1 as check;
    let intent = canonical_prune_intent_artifact_fixture();
    check(&intent, "iroha_core::kura::KuraPruneIntentV3");
    let mut blocks = DummyBlocks::new();
    let _genesis = blocks.next();
    let block = blocks.next();
    let entry = sample_merge_entry_for_block(1, &block);
    let carrier = MergeLedgerCarrierRecord::new(&entry, &block);
    check(&carrier, "iroha_core::kura::MergeLedgerCarrierRecord");
    assert_eq!(carrier.entry_hash, entry.canonical_hash());
    assert_eq!(carrier.block_hash, block.hash());
    assert_eq!(carrier.block_height, block.header().height().get());
    let frame = norito::encode_canonical(&carrier).expect("encode carrier owner");
    assert!(matches!(
        norito::decode_canonical::<KuraPruneIntentV3>(&frame),
        Err(norito::Error::SchemaMismatch)
    ));
}

fn frame_kura_test_payload<Owner, Payload>(current: &Owner, unsupported: &Payload) -> Vec<u8>
where
    Owner: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>,
    Payload: norito::SerializePayload,
{
    // Adversarial layouts remain payload-only; the real current owner supplies
    // their envelope. A positive control checks this exact framing procedure.
    let (current_payload, current_flags, payload, flags) = {
        let _canonical =
            norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        let (current_payload, current_flags) = norito::codec::encode_with_header_flags(current);
        let (payload, flags) = norito::codec::encode_with_header_flags(unsupported);
        (current_payload, current_flags, payload, flags)
    };
    let control =
        norito::core::frame_bare_with_header_flags::<Owner>(&current_payload, current_flags)
            .expect("frame current Kura owner control");
    assert_eq!(
        control,
        norito::encode_canonical(current).expect("encode current owner")
    );
    let decoded = norito::decode_canonical::<Owner>(&control)
        .expect("current payload roundtrips through the same framing procedure");
    assert_eq!(
        norito::encode_canonical(&decoded).expect("re-encode control"),
        control
    );
    let frame = norito::core::frame_bare_with_header_flags::<Owner>(&payload, flags)
        .expect("frame adversarial Kura payload under current owner");
    let view = norito::core::from_bytes_view(&frame)
        .expect("adversarial frame has a valid length, header and checksum");
    assert_eq!(
        view.schema(),
        norito::schema::identity::frame_hash::<Owner>()
    );
    assert_eq!(view.as_bytes(), payload.as_slice());
    frame
}

fn assert_kura_test_payload_rejected<Owner>(frame: &[u8])
where
    Owner: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>,
{
    let error = match norito::decode_canonical::<Owner>(frame) {
        Ok(_) => panic!("unsupported Kura payload decoded as the current layout"),
        Err(error) => error,
    };
    assert!(
        !matches!(error, norito::Error::SchemaMismatch),
        "the negative control must reach payload decoding under its actual owner"
    );
}

#[test]
fn progress_sidecar_test_frame_has_its_own_current_identity() {
    crate::private_settlement::global_state::tests::assert_private_settlement_frame_v1(
        &DummySidecar { height: 7 },
        "iroha_core::kura::tests::DummySidecar",
    );
}
