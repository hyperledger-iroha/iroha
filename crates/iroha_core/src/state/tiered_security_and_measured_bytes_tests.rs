    fn axt_security_map_fixture() -> (
        DataSpaceId,
        AxtHandleCounterRecord,
        AssetDefinitionId,
        AxtAssetIncarnationV1,
        AxtHandleBudgetKey,
        AxtHandleBudgetRecord,
    ) {
        let definition_id = AssetDefinitionId::from_uuid_bytes([
            0x41, 0x63, 0x85, 0xa7, 0xc9, 0xeb, 0x4d, 0xef, 0x80, 0x31, 0x42, 0x53, 0x64, 0x75,
            0x86, 0x97,
        ])
        .expect("tiered AXT security-map fixture UUID is valid");
        let default_context = AxtHandleIssuerContextV1::default();
        let registration_header = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"tiered AXT security-map registration",
        ));
        let incarnation = AxtAssetIncarnationV1::derive(
            &default_context.network_id,
            &definition_id,
            &registration_header,
            &Hash::new(b"tiered AXT security-map execution"),
            0,
        );
        let context = AxtHandleIssuerContextV1 {
            asset_definition_incarnation: incarnation,
            ..default_context
        };
        let payload = AssetHandleIssuerPayloadV1 {
            context,
            asset_definition_id: definition_id.clone(),
            scope: vec!["transfer".to_owned()],
            subject: HandleSubject {
                account: "tiered-axt-security-subject".to_owned(),
                origin_dsid: Some(context.asset_dsid),
            },
            budget: HandleBudget {
                remaining: Quantity::from(11_u64),
                per_use: None,
            },
            active_handle_era: 9,
            next_handle_counter: 4,
            group_binding: GroupBinding {
                composability_group_id: b"tiered-axt-security".to_vec(),
                epoch_id: 5,
            },
            target_lane: LaneId::SINGLE,
            axt_binding: AxtBinding::new([0xC7; 32]),
            manifest_view_root: [0xD7; 32],
            expiry_slot: 101,
            max_clock_skew_ms: None,
        };
        let budget_key = AxtHandleBudgetKey::from_issuer_payload_v1(&payload);
        let mut budget_record = AxtHandleBudgetRecord::empty();
        budget_record
            .try_consume(&budget_key, &Quantity::from(4_u64), 101)
            .expect("tiered AXT family consumption fits its signed budget");
        (
            context.asset_dsid,
            AxtHandleCounterRecord::try_from_parts(
                payload.next_handle_counter,
                payload.active_handle_era,
            )
            .expect("tiered AXT counter fixture is canonical"),
            definition_id,
            incarnation,
            budget_key,
            budget_record,
        )
    }
    #[test]
    fn streamed_hash_matches_canonical_json() {
        let value: norito::json::Value = norito::json::from_str(
            r#"{
                "domain": "wonderland",
                "accounts": [
                    {"id": "i105-subject-alice", "metadata": {"email": "alice@example.com"}},
                    {"id": "i105-subject-bob", "metadata": {"roles": ["admin", "auditor"]}}
                ],
                "supply": 42,
                "flags": {
                    "enabled": true,
                    "threshold": 0.5
                }
            }"#,
        )
        .expect("valid JSON fixture");
        let (stream_hash, stream_len) =
            compute_json_hash(&value).expect("streamed hash computation");
        let encoded = norito::json::to_vec(&value).expect("direct encode");
        assert_eq!(stream_len, encoded.len());
        assert_eq!(stream_hash, sha256(&encoded));
    }
    #[test]
    fn hot_bytes_account_for_vec_capacity() {
        let mut value = Vec::with_capacity(12);
        value.extend_from_slice(&[1_u8, 2, 3, 4, 5, 6, 7, 8]);
        let expected =
            std::mem::size_of::<Vec<u8>>() + value.capacity() * std::mem::size_of::<u8>();
        let measured = compute_hot_bytes(&value).expect("hot byte measurement");
        assert_eq!(measured, expected);
    }
    #[test]
    fn measured_bytes_account_for_proof_attachment_list_capacity() {
        let attachment = ProofAttachment::new_ref(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![0xCA, 0xFE]),
            VerifyingKeyId::new("halo2/ipa", "tiered-capacity-fixture"),
        );
        let mut compact = Vec::with_capacity(1);
        compact.push(attachment.clone());
        let mut reserved = Vec::with_capacity(8);
        reserved.push(attachment);
        let compact = ProofAttachmentList::try_from(compact).expect("valid compact list");
        let reserved = ProofAttachmentList::try_from(reserved).expect("valid reserved list");
        assert_eq!(compact.as_slice(), reserved.as_slice());
        let capacity_delta = reserved
            .capacity()
            .checked_sub(compact.capacity())
            .expect("reserved fixture has greater capacity");
        assert!(capacity_delta > 0);
        let measured_delta = MeasuredBytes::measured_bytes(&reserved)
            .checked_sub(MeasuredBytes::measured_bytes(&compact))
            .expect("reserved fixture has greater measured size");
        assert_eq!(
            measured_delta,
            capacity_delta.saturating_mul(std::mem::size_of::<ProofAttachment>())
        );
    }
    #[test]
    fn measured_bytes_track_governance_approval_sizes() {
        use std::collections::BTreeSet;
        let mut approval = crate::state::GovernanceStageApproval {
            epoch: 1,
            approvers: BTreeSet::new(),
            rejections: BTreeSet::new(),
            abstentions: BTreeSet::new(),
            required: 2,
            quorum_bps: 5000,
        };
        let empty_bytes = MeasuredBytes::measured_bytes(&approval);
        let keypair = iroha_crypto::KeyPair::try_from_seed(
            b"tiered-approval".to_vec(),
            iroha_crypto::Algorithm::Ed25519,
        )
        .expect("fixture seed must derive a valid keypair");
        assert!(
            iroha_crypto::KeyPair::try_from_seed(vec![0; 32], iroha_crypto::Algorithm::Ed25519)
                .is_err(),
            "checked Ed25519 seed derivation must reject weak all-zero fixture seeds"
        );
        approval
            .approvers
            .insert(iroha_data_model::account::AccountId::new(
                keypair.public_key().clone(),
            ));
        let filled_bytes = MeasuredBytes::measured_bytes(&approval);
        assert!(filled_bytes >= empty_bytes);
        let mut approvals = crate::state::GovernanceStageApprovals::default();
        let base_bytes = MeasuredBytes::measured_bytes(&approvals);
        approvals.stages.insert(
            iroha_data_model::governance::types::ParliamentBody::AgendaCouncil,
            approval,
        );
        let updated_bytes = MeasuredBytes::measured_bytes(&approvals);
        assert!(updated_bytes >= base_bytes);
    }
    #[test]
    fn measured_bytes_cover_trigger_filters() {
        use iroha_data_model::{
            events::{EventFilterBox, data::DataEventFilter},
            trigger::{TriggerId, action::Repeats},
        };
        let trigger_id: TriggerId = "audit_trigger".parse().expect("trigger id");
        let repeats = Repeats::Exactly(1);
        let filter = EventFilterBox::Data(DataEventFilter::Any);
        assert!(MeasuredBytes::measured_bytes(&trigger_id) >= std::mem::size_of::<TriggerId>());
        assert_eq!(
            MeasuredBytes::measured_bytes(&repeats),
            std::mem::size_of::<Repeats>()
        );
        assert!(MeasuredBytes::measured_bytes(&filter) >= std::mem::size_of::<EventFilterBox>());
    }
    #[test]
    fn measured_bytes_cover_opaque_account_id() {
        let opaque = OpaqueAccountId::from_hash(Hash::new([7_u8; 32]));
        assert_eq!(
            MeasuredBytes::measured_bytes(&opaque),
            std::mem::size_of::<OpaqueAccountId>()
        );
    }
    #[test]
    fn measured_bytes_cover_opaque_asset_definition_id() {
        use iroha_data_model::asset::AssetDefinitionId;
        let opaque = AssetDefinitionId::from_uuid_bytes([
            0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0x4d, 0xef, 0x80, 0x11, 0x22, 0x33, 0x44, 0x55,
            0x66, 0x77,
        ])
        .expect("measured-bytes fixture UUID is valid");
        assert_eq!(
            MeasuredBytes::measured_bytes(&opaque),
            std::mem::size_of::<AssetDefinitionId>()
        );
    }
    #[test]
    fn axt_security_maps_roundtrip_through_tiered_payloads_with_exact_sizes() {
        let (dataspace, counter, definition_id, incarnation, budget_key, budget_record) =
            axt_security_map_fixture();
        assert_eq!(
            MeasuredBytes::measured_bytes(&counter),
            std::mem::size_of::<AxtHandleCounterRecord>()
        );
        assert_eq!(
            MeasuredBytes::measured_bytes(&incarnation),
            std::mem::size_of::<AxtAssetIncarnationV1>()
        );
        assert_eq!(
            MeasuredBytes::measured_bytes(&budget_record),
            std::mem::size_of::<AxtHandleBudgetRecord>()
                .saturating_add(budget_record.consumed().measured_bytes_extra())
        );
        let mut world = World::default();
        world.axt_handle_counters.insert(dataspace, counter);
        world
            .axt_asset_incarnations
            .insert(definition_id.clone(), incarnation);
        world
            .axt_handle_budget_ledger
            .insert(budget_key.clone(), budget_record.clone());

        let temp = tempdir().expect("temporary tiered AXT payload directory");
        let mut backend =
            TieredStateBackend::new(true, 0, 1, 0, Some(temp.path().to_path_buf()), None, 0, 0);
        backend
            .record_world_snapshot(&world)
            .expect("persist tiered AXT security maps");
        let manifest = backend.last_manifest().expect("tiered manifest recorded");
        let snapshot_index = manifest.snapshot_index;

        let counter_entry = manifest
            .cold_entries
            .iter()
            .find(|entry| entry.segment == TieredSegment::AxtHandleCounters)
            .expect("AXT counter is accounted in the tiered manifest");
        let incarnation_entry = manifest
            .cold_entries
            .iter()
            .find(|entry| entry.segment == TieredSegment::AxtAssetIncarnations)
            .expect("AXT asset incarnation is accounted in the tiered manifest");
        let budget_entry = manifest
            .cold_entries
            .iter()
            .find(|entry| entry.segment == TieredSegment::AxtHandleBudgetLedger)
            .expect("AXT family budget is accounted in the tiered manifest");

        assert_eq!(
            counter_entry.key_payload,
            norito::codec::Encode::encode(&dataspace)
        );
        assert_eq!(
            incarnation_entry.key_payload,
            norito::codec::Encode::encode(&definition_id)
        );
        assert_eq!(
            budget_entry.key_payload,
            norito::codec::Encode::encode(&budget_key)
        );
        assert_eq!(
            counter_entry.value_size_bytes(),
            MeasuredBytes::measured_bytes(&counter)
        );
        assert_eq!(
            incarnation_entry.value_size_bytes(),
            MeasuredBytes::measured_bytes(&incarnation)
        );
        assert_eq!(
            budget_entry.value_size_bytes(),
            MeasuredBytes::measured_bytes(&budget_record)
        );

        let counter_bytes = backend
            .read_cold_payload(snapshot_index, counter_entry)
            .expect("read AXT counter cold payload")
            .expect("AXT counter cold payload exists");
        let incarnation_bytes = backend
            .read_cold_payload(snapshot_index, incarnation_entry)
            .expect("read AXT incarnation cold payload")
            .expect("AXT incarnation cold payload exists");
        let budget_bytes = backend
            .read_cold_payload(snapshot_index, budget_entry)
            .expect("read AXT budget cold payload")
            .expect("AXT budget cold payload exists");
        assert_eq!(
            json::from_slice::<AxtHandleCounterRecord>(&counter_bytes)
                .expect("decode AXT counter payload"),
            counter
        );
        assert_eq!(
            json::from_slice::<AxtAssetIncarnationV1>(&incarnation_bytes)
                .expect("decode AXT incarnation payload"),
            incarnation
        );
        assert_eq!(
            json::from_slice::<AxtHandleBudgetRecord>(&budget_bytes)
                .expect("decode AXT budget payload"),
            budget_record
        );
    }
    #[test]
    fn axt_security_maps_roundtrip_through_incremental_payloads() {
        let (dataspace, counter, definition_id, incarnation, budget_key, budget_record) =
            axt_security_map_fixture();
        let mut world = World::default();
        world.axt_handle_counters.insert(dataspace, counter);
        world
            .axt_asset_incarnations
            .insert(definition_id.clone(), incarnation);
        world
            .axt_handle_budget_ledger
            .insert(budget_key.clone(), budget_record.clone());

        let temp = tempdir().expect("temporary incremental AXT payload directory");
        let mut backend =
            TieredStateBackend::new(true, 0, 1, 0, Some(temp.path().to_path_buf()), None, 0, 0);
        backend
            .record_world_snapshot(&world)
            .expect("seed tiered AXT security maps");

        let updated_counter = AxtHandleCounterRecord::try_from_parts(5, 9)
            .expect("updated tiered counter fixture is canonical");
        let default_context = AxtHandleIssuerContextV1::default();
        let updated_incarnation = AxtAssetIncarnationV1::derive(
            &default_context.network_id,
            &definition_id,
            &HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"updated tiered AXT registration",
            )),
            &Hash::new(b"updated tiered AXT execution"),
            1,
        );
        let mut updated_budget = budget_record;
        updated_budget
            .try_consume(&budget_key, &Quantity::from(1_u64), 102)
            .expect("updated tiered consumption fits its signed budget");
        let mut block = world.block();
        block.axt_handle_counters.insert(dataspace, updated_counter);
        block
            .axt_asset_incarnations
            .insert(definition_id, updated_incarnation);
        block
            .axt_handle_budget_ledger
            .insert(budget_key, updated_budget.clone());
        let payload = block.tiered_snapshot_payload();
        backend
            .record_world_snapshot_with_payload(&payload)
            .expect("persist incremental AXT security-map payloads");

        let manifest = backend.last_manifest().expect("tiered manifest recorded");
        let snapshot_index = manifest.snapshot_index;
        let payload_for = |segment| {
            let entry = manifest
                .cold_entries
                .iter()
                .find(|entry| entry.segment == segment)
                .expect("updated AXT entry is present in the cold manifest");
            backend
                .read_cold_payload(snapshot_index, entry)
                .expect("read updated AXT cold payload")
                .expect("updated AXT cold payload exists")
        };
        assert_eq!(
            json::from_slice::<AxtHandleCounterRecord>(&payload_for(
                TieredSegment::AxtHandleCounters
            ))
            .expect("decode updated AXT counter"),
            updated_counter
        );
        assert_eq!(
            json::from_slice::<AxtAssetIncarnationV1>(&payload_for(
                TieredSegment::AxtAssetIncarnations
            ))
            .expect("decode updated AXT incarnation"),
            updated_incarnation
        );
        assert_eq!(
            json::from_slice::<AxtHandleBudgetRecord>(&payload_for(
                TieredSegment::AxtHandleBudgetLedger
            ))
            .expect("decode updated AXT budget"),
            updated_budget
        );
    }
    fn dummy_state_entry(seed: u8) -> (StatePath, Vec<u8>) {
        (
            format!("tiered_state_{seed}")
                .parse()
                .expect("valid tiered-state fixture path"),
            vec![seed; usize::from(seed).saturating_add(2)],
        )
    }
    #[test]
    fn snapshot_failure_leaves_existing_snapshot_intact() {
        let temp = tempdir().expect("tmpdir");
        let root = temp.path().to_path_buf();
        let mut backend = TieredStateBackend::new(true, 0, 0, 0, Some(root.clone()), None, 0, 0);
        let existing_dir = root.join(format!("{:020}", 1_u64));
        fs::create_dir_all(&existing_dir).expect("create existing snapshot");
        let marker = existing_dir.join("marker.txt");
        fs::write(&marker, b"keep me").expect("write marker");
        let plan = TieredSnapshotPlan {
            root: root.clone(),
            snapshot_dir: existing_dir.clone(),
            manifest: TieredSnapshotManifest {
                snapshot_index: 1,
                total_entries: 0,
                hot_entries: Vec::new(),
                cold_entries: Vec::new(),
                cold_bytes_total: 0,
                cold_reused_entries: 0,
                cold_reused_bytes: 0,
                hot_promotions: 0,
                hot_demotions: 0,
                hot_grace_overflow_keys: 0,
                hot_grace_overflow_bytes: 0,
            },
            cold_entries: Vec::new(),
        };
        let staging_path = plan.snapshot_dir.with_extension("staging");
        fs::write(&staging_path, b"block staging dir creation").expect("write staging file");
        let result = backend.execute_snapshot_plan(plan, &World::default());
        assert!(
            result.is_err(),
            "expected staging collision to fail snapshot"
        );
        assert!(
            marker.exists(),
            "existing snapshot directory should remain when staging fails"
        );
    }
