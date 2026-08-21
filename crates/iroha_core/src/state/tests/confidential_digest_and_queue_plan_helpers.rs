state_test! { sync confidential_digest_respects_activation_height
    use iroha_data_model::{
        confidential::ConfidentialStatus,
        proof::{VerifyingKeyId, VerifyingKeyRecord},
        zk::BackendTag,
    };
    let mut world = World::new();
    let id = VerifyingKeyId::new("halo2/ipa", "vk_activation");
    let_row! { mut record = VerifyingKeyRecord::new_with_owner( 1, "circuit_activation", None, "core", BackendTag::Halo2IpaPasta, "pallas", [0x11; 32], [0x22; 32], ) };
    record.status = ConfidentialStatus::Proposed;
    record.activation_height = Some(5);
    record.gas_schedule_id = Some("sched_activation".into());
    record.public_inputs_schema_hash = [0x33; 32];
    world.verifying_keys.insert(id.clone(), record.clone());
    world
        .verifying_keys_by_circuit
        .insert((record.circuit_id.clone(), record.version), id.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new(world, kura, query);
    state.zk.registry_max_delta_per_block = 10;
    let view = state.view();
    let_row! { digest_before = compute_confidential_feature_digest(view.world(), &view.zk, view.sccp_registry.as_ref(), 4) };
    assert_eq!(digest_before.vk_set_hash, None);
    let_row! { digest_at_activation = compute_confidential_feature_digest(view.world(), &view.zk, view.sccp_registry.as_ref(), 5) };
    assert!(digest_at_activation.vk_set_hash.is_some());
}
state_test! { sync confidential_digest_excludes_active_vk_outside_height_window
    use iroha_data_model::{
        confidential::ConfidentialStatus,
        proof::{VerifyingKeyId, VerifyingKeyRecord},
        zk::BackendTag,
    };
    let mut world = World::new();
    let id = VerifyingKeyId::new("halo2/ipa", "vk_windowed_active");
    let_row! { mut record = VerifyingKeyRecord::new_with_owner( 1, "circuit_windowed_active", None, "core", BackendTag::Halo2IpaPasta, "pallas", [0x21; 32], [0x42; 32], ) };
    record.status = ConfidentialStatus::Active;
    record.activation_height = Some(5);
    record.withdraw_height = Some(8);
    record.gas_schedule_id = Some("sched_windowed_active".into());
    record.public_inputs_schema_hash = [0x63; 32];
    world.verifying_keys.insert(id.clone(), record.clone());
    world
        .verifying_keys_by_circuit
        .insert((record.circuit_id.clone(), record.version), id);
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new(world, kura, query);
    let view = state.view();
    assert_eq!(compute_vk_set_hash_at_height(view.world(), 4), None);
    assert!(compute_vk_set_hash_at_height(view.world(), 5).is_some());
    assert_eq!(compute_vk_set_hash_at_height(view.world(), 8), None);
    let_row! { digest_before = compute_confidential_feature_digest(view.world(), &view.zk, view.sccp_registry.as_ref(), 4) };
    assert_eq!(digest_before.vk_set_hash, None);
    let_row! { digest_active = compute_confidential_feature_digest(view.world(), &view.zk, view.sccp_registry.as_ref(), 5) };
    assert!(digest_active.vk_set_hash.is_some());
    let_row! { digest_withdrawn = compute_confidential_feature_digest(view.world(), &view.zk, view.sccp_registry.as_ref(), 8) };
    assert_eq!(digest_withdrawn.vk_set_hash, None);
}
state_test! { sync confidential_registry_delta_cap_limits_transitions
    use iroha_data_model::{
        confidential::ConfidentialStatus,
        proof::{VerifyingKeyId, VerifyingKeyRecord},
        zk::BackendTag,
    };
    let mut world = World::new();
    let_row! { ids = [ VerifyingKeyId::new("halo2/ipa", "vk_alpha"), VerifyingKeyId::new("halo2/ipa", "vk_beta"), ] };
    for (idx, id) in ids.iter().enumerate() {
        let_row! { mut record = VerifyingKeyRecord::new_with_owner( 1, format!("circuit_{idx}"), None, "core", BackendTag::Halo2IpaPasta, "pallas", [0x40 + u8::try_from(idx).expect("vk index fits in u8"); 32], [0x50 + u8::try_from(idx).expect("vk index fits in u8"); 32], ) };
        record.status = ConfidentialStatus::Proposed;
        record.activation_height = Some(2);
        record.gas_schedule_id = Some(format!("sched_{idx}"));
        record.public_inputs_schema_hash =
            [0x60 + u8::try_from(idx).expect("vk index fits in u8"); 32];
        world.verifying_keys.insert(id.clone(), record.clone());
        world
            .verifying_keys_by_circuit
            .insert((record.circuit_id.clone(), record.version), id.clone());
    }
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new(world, kura, query);
    state.zk.registry_max_delta_per_block = 1;
    let header = BlockHeader::new(NonZeroU64::new(2).unwrap(), None, None, None, 0, 0);
    let block = state.block(header);
    let_row! { alpha_status = block .world .verifying_keys .get(&ids[0]) .map(|rec| rec.status) .expect("alpha vk present") };
    let_row! { beta_status = block .world .verifying_keys .get(&ids[1]) .map(|rec| rec.status) .expect("beta vk present") };
    assert_eq!(alpha_status, ConfidentialStatus::Active);
    assert_eq!(beta_status, ConfidentialStatus::Proposed);
}
fn assemble_ivm_header(code: &[u8]) -> Vec<u8> {
    let_row! { mut blob = ivm::ProgramMetadata { version_major: 1, version_minor: 0, mode: 0, vector_length: 0, max_cycles: 1_000_000, abi_version: 1, } .encode() };
    blob.extend_from_slice(code);
    blob
}
/// Used to inject faulty payload for testing
fn new_dummy_block_with_payload(f: impl FnOnce(&mut BlockHeader)) -> CommittedBlock {
    let_row! { (leader_public_key, leader_private_key) = crate::state::checked_keypair_with_algorithm(iroha_crypto::Algorithm::BlsNormal) .into_parts() };
    let peer_id = PeerId::new(leader_public_key);
    let topology = Topology::new(vec![peer_id]);
    ValidBlock::new_dummy_and_modify_header(&leader_private_key, f)
        .commit(&topology)
        .unpack(|_| {})
        .unwrap()
}
fn new_dummy_block() -> CommittedBlock {
    new_dummy_block_with_payload(|_| {})
}
fn dummy_merge_qc() -> MergeQuorumCertificate {
    let validator_set = Vec::<PeerId>::new();
    MergeQuorumCertificate::new(
        0,
        0,
        1,
        HashOf::from_untyped_unchecked(Hash::new(b"dummy-merge-parent")),
        *DEFAULT_TEST_NETWORK_ID,
        VALIDATOR_SET_HASH_VERSION_V1,
        HashOf::new(&validator_set),
        validator_set,
        vec![0x01],
        Vec::new(),
        vec![0xAA],
        iroha_crypto::Hash::new(b"qc"),
    )
}
state_test! { sync malformed_merge_execution_batch_rejects_empty_lane_set
    let state = blank_test_state();
    let application_block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 1, 0);
    let_row! { batch = MergeExecutionBatch { version: 1, base_state_height: 0, base_state_hash: HashOf::from_untyped_unchecked(Hash::new(b"execution-base")), application_block_header, execution_root: Hash::new(b"execution-root"), lanes: Vec::new(), entrypoint_count: 0, entrypoint_merkle_root: HashOf::from_untyped_unchecked(Hash::new(b"execution-entrypoints")), result_merkle_root: HashOf::from_untyped_unchecked(Hash::new(b"execution-results")), application_write_set_root: Hash::new(b"application-write-set"), write_set_root: Hash::new(b"write-set"), expected_post_state_hash: HashOf::from_untyped_unchecked(Hash::new(b"post-state")), batch_hash: Hash::new(b"batch"), } };
    assert!(matches!(
        state.validate_merge_execution_batch(&[], &batch, &BTreeMap::new(), true),
        Err(MergeLedgerCommitError::ExecutionBatchInvalid(reason))
            if reason == "lane count is empty or exceeds the hard limit"
    ));
}
state_test! { sync merge_execution_canonical_order_is_route_first
    let_row! { (earlier_route_later_proposal, _) = sample_committed_lane_block_session_for_state_test( LaneId::new(1), DataSpaceId::new(9), Hash::new(b"route-first earlier incarnation"), 99, 1, ) };
    let_row! { (later_route_earlier_proposal, _) = sample_committed_lane_block_session_for_state_test( LaneId::new(2), DataSpaceId::new(1), Hash::new(b"route-first later incarnation"), 1, 1, ) };
    let_row! { mut proposals = vec![ later_route_earlier_proposal.proposal, earlier_route_later_proposal.proposal, ] };
    proposals.sort_by_key(merge_execution_canonical_order_key);
    assert_eq!(
        proposals
            .iter()
            .map(|proposal| proposal.descriptor.lane_id)
            .collect::<Vec<_>>(),
        vec![LaneId::new(1), LaneId::new(2)],
        "proposal timing must not reorder the canonical lane/route prefix"
    );
}
fn empty_merge_settlement(
    lane_id: LaneId,
    lane_incarnation: Hash,
    dataspace_id: DataSpaceId,
    block_height: u64,
) -> LaneBlockCommitment {
    LaneBlockCommitment {
        block_height,
        lane_id,
        lane_incarnation,
        dataspace_id,
        tx_count: 0,
        total_local_amount: "0".parse().expect("valid settlement quantity"),
        total_xor_due: "0".parse().expect("valid settlement quantity"),
        total_xor_after_haircut: "0".parse().expect("valid settlement quantity"),
        total_xor_variance: "0".parse().expect("valid settlement quantity"),
        swap_metadata: None,
        receipts: Vec::new(),
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: Vec::new(),
    }
}
fn merge_candidate_with_lanes(epoch: u64, count: usize) -> crate::merge::MergeLedgerCandidate {
    let mut lane_snapshots = Vec::with_capacity(count);
    for idx in 0..count {
        let tip = u8::try_from(idx).expect("lane index fits in u8");
        let hint = tip.strict_add(1);
        let lane_id = LaneId::new(u32::try_from(idx).expect("lane index fits in u32"));
        let lane_incarnation = iroha_crypto::Hash::new([tip, 0x49]);
        let dataspace_id = DataSpaceId::new(u64::try_from(idx + 1).expect("dsid fits in u64"));
        let lane_block_height = epoch.saturating_add(u64::try_from(idx).expect("height fits"));
        let_row! { settlement_commitment = empty_merge_settlement(lane_id, lane_incarnation, dataspace_id, lane_block_height) };
        lane_snapshots.push(MergeLaneSnapshot {
            lane_id,
            lane_incarnation,
            incarnation_activation_height: 1,
            proposal_height: epoch.max(1),
            dataspace_id,
            lane_block_height,
            tip_hash: HashOf::from_untyped_unchecked(iroha_crypto::Hash::new([tip])),
            merge_hint_root: iroha_crypto::Hash::new([hint]),
            settlement_hash: canonical_merge_settlement_hash(&settlement_commitment)
                .expect("test settlement commitment should hash canonically"),
            settlement_commitment,
            relay_envelope: None,
        });
    }
    let_row! { active_lanes = lane_snapshots .iter() .map(|snapshot| MergeLaneBinding { lane_id: snapshot.lane_id, dataspace_id: snapshot.dataspace_id, lane_config_hash: Hash::new( format!("test-merge-lane-{}", snapshot.lane_id.as_u32()).as_bytes(), ), incarnation: snapshot.lane_incarnation, activation_height: snapshot.incarnation_activation_height, }) .collect::<Vec<_>>() };
    let_row! { lifecycle_incarnations = active_lanes .iter() .map( |binding| iroha_data_model::nexus::LaneLifecycleIncarnationEntry { lane_id: binding.lane_id, incarnation: binding.incarnation, }, ) .collect::<Vec<_>>() };
    let_row! { merge_hint_roots: Vec<Hash> = lane_snapshots .iter() .map(|snapshot| snapshot.merge_hint_root) .collect() };
    let global_state_root = crate::merge::reduce_merge_hint_roots(&merge_hint_roots);
    crate::merge::MergeLedgerCandidate {
        version: crate::merge::MergeLedgerCandidate::VERSION,
        epoch_id: epoch,
        view: 0,
        carrier_height: 2,
        carrier_parent_hash: HashOf::from_untyped_unchecked(Hash::new(b"test-merge-parent")),
        lane_catalog_hash: Hash::new(format!("test-merge-catalog-{count}").as_bytes()),
        incarnation_root: iroha_data_model::nexus::LaneLifecycleParameterV1::incarnation_root(
            &lifecycle_incarnations,
        ),
        activation_root: crate::merge::merge_activation_root(&active_lanes),
        active_lanes,
        lane_snapshots,
        execution_batch: None,
        lane_drain_certificates: Vec::new(),
        global_state_root,
    }
}
fn merge_candidate_from_relay(
    state: &State,
    epoch: u64,
    envelope: &LaneRelayEnvelope,
) -> crate::merge::MergeLedgerCandidate {
    if state.latest_block_hash_fast().is_none() {
        let_row! { parent = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"test-relay-carrier-parent")) };
        let mut block_hashes = state.block_hashes.block();
        block_hashes.push_for_tests(parent);
        block_hashes.commit_for_tests();
    }
    let merge_hint_root = envelope.merge_hint_root().expect("merge hint root");
    let_row! { lane_snapshots = vec![MergeLaneSnapshot { lane_id: envelope.lane_id, lane_incarnation: envelope.lane_incarnation, incarnation_activation_height: state .lane_incarnation_activation_heights_snapshot() .get(&envelope.lane_id) .copied() .unwrap_or_default() .saturating_add(1), proposal_height: envelope.block_header.height().get(), dataspace_id: envelope.dataspace_id, lane_block_height: envelope.block_height, tip_hash: envelope.block_header.hash(), merge_hint_root, settlement_commitment: envelope.settlement_commitment.clone(), settlement_hash: envelope.settlement_hash, relay_envelope: Some(envelope.clone()), }] };
    let nexus = state.nexus_snapshot();
    let incarnations = state.lane_incarnations_snapshot();
    let activations = state.lane_incarnation_activation_heights_snapshot();
    let_row! { active_lanes = nexus .lane_catalog .lanes() .iter() .map(|lane| MergeLaneBinding { lane_id: lane.id, dataspace_id: lane.dataspace_id, lane_config_hash: merge_lane_config_hash(lane), incarnation: incarnations[&lane.id], activation_height: activations[&lane.id].saturating_add(1), }) .collect::<Vec<_>>() };
    let_row! { lifecycle_incarnations = active_lanes .iter() .map( |binding| iroha_data_model::nexus::LaneLifecycleIncarnationEntry { lane_id: binding.lane_id, incarnation: binding.incarnation, }, ) .collect::<Vec<_>>() };
    let merge_hint_roots = vec![merge_hint_root];
    let global_state_root = crate::merge::reduce_merge_hint_roots(&merge_hint_roots);
    crate::merge::MergeLedgerCandidate {
        version: crate::merge::MergeLedgerCandidate::VERSION,
        epoch_id: epoch,
        view: envelope.block_header.view_change_index(),
        carrier_height: u64::try_from(state.committed_height())
            .unwrap_or(u64::MAX)
            .saturating_add(1),
        carrier_parent_hash: state
            .latest_block_hash_fast()
            .expect("test merge carrier parent was seeded"),
        lane_catalog_hash: merge_lane_catalog_hash(&nexus.lane_catalog),
        incarnation_root: iroha_data_model::nexus::LaneLifecycleParameterV1::incarnation_root(
            &lifecycle_incarnations,
        ),
        activation_root: crate::merge::merge_activation_root(&active_lanes),
        active_lanes,
        lane_snapshots,
        execution_batch: None,
        lane_drain_certificates: Vec::new(),
        global_state_root,
    }
}
fn merge_entry_from_candidate(
    candidate: crate::merge::MergeLedgerCandidate,
    merge_qc: MergeQuorumCertificate,
) -> MergeLedgerEntry {
    candidate.into_entry(merge_qc)
}
fn ensure_merge_carrier_parent_for_test(state: &State) {
    let committed_height = state.committed_height();
    let durable_count = state.exact_durable_block_count().unwrap();
    if committed_height > 0 {
        assert_eq!(
            committed_height, durable_count,
            "partially hydrated test state must not synthesize an occupied carrier height"
        );
        let_row! { durable_tip = state .kura .get_durable_block_hash( NonZeroUsize::new(durable_count).expect("durable height is non-zero"), ) .expect("durable test state exposes its canonical tip") };
        assert_eq!(
            state.latest_block_hash_fast(),
            Some(durable_tip),
            "committed test state tip must match Kura before carrier synthesis"
        );
        return;
    }
    if durable_count > 0 {
        let mut block_hashes = state.block_hashes.block();
        for height in 1..=durable_count {
            let height = NonZeroUsize::new(height).expect("durable height is non-zero");
            let_row! { hash = state .kura .get_durable_block_hash(height) .expect("durable merge-carrier parent hash") };
            block_hashes.push_for_tests(hash);
        }
        block_hashes.commit_for_tests();
        let tip_height = NonZeroUsize::new(durable_count).expect("durable tip is non-zero");
        let_row! { tip = state .kura .get_block(tip_height) .expect("durable merge-carrier parent block") };
        state.update_latest_block_header_cache_for_tests(tip.header().clone());
        seed_empty_transaction_height_for_state_test(
            state,
            u64::try_from(durable_count).expect("durable test height fits u64"),
        );
        assert_eq!(state.committed_height(), durable_count);
        return;
    }
    let_row! { parent = new_dummy_block_with_payload(|header| { header.set_height(nonzero!(1_u64)); header.set_prev_block_hash(None); header.set_view_change_index(0); }) };
    let parent_hash = parent.as_ref().hash();
    state
        .kura
        .store_block(parent.clone())
        .expect("store canonical merge-carrier parent");
    let mut block_hashes = state.block_hashes.block();
    block_hashes.push_for_tests(parent_hash);
    block_hashes.commit_for_tests();
    state.update_latest_block_header_cache_for_tests(parent.as_ref().header().clone());
    seed_empty_transaction_height_for_state_test(state, 1);
    assert_eq!(state.latest_block_hash_fast(), Some(parent_hash));
    assert_eq!(state.exact_durable_block_count().unwrap(), 1);
}
fn store_merge_carrier_without_state_publication_for_test(
    state: &State,
    entry: &MergeLedgerEntry,
) -> CommittedBlock {
    let_row! { expected_height = u64::try_from(state.committed_height()) .expect("committed test height fits u64") .saturating_add(1) };
    assert_eq!(
        entry.merge_qc.carrier_height, expected_height,
        "test merge entry must target the next canonical carrier"
    );
    assert_eq!(
        state.latest_block_hash_fast(),
        Some(entry.merge_qc.carrier_parent_hash),
        "test merge entry must bind the current canonical parent"
    );
    let_row! { mut carrier = new_dummy_block_with_payload(|header| { header.set_height( NonZeroU64::new(entry.merge_qc.carrier_height) .expect("test merge carrier height is non-zero"), ); header.set_prev_block_hash(Some(entry.merge_qc.carrier_parent_hash)); header.set_view_change_index(entry.merge_qc.view); }) };
    let_row! { execution_context = carrier .as_ref() .execution_context() .cloned() .unwrap_or_else(|| iroha_data_model::block::BlockExecutionContextBundle::new(Vec::new())) .with_merge_entry(iroha_data_model::block::CertifiedMergeLedgerReference::new( entry, )) };
    carrier
        .as_mut()
        .set_execution_context(Some(execution_context));
    state
        .kura
        .store_block_with_merge_entry(Arc::new(carrier.as_ref().clone()), entry)
        .expect("store canonical merge carrier");
    persist_merge_carrier_finality_for_state_test(&state.kura, carrier.as_ref());
    carrier
}
fn publish_committed_merge_carrier_for_test(state: &State, entry: &MergeLedgerEntry) {
    let carrier = store_merge_carrier_without_state_publication_for_test(state, entry);
    let carrier_hash = carrier.as_ref().hash();
    let mut block_hashes = state.block_hashes.block();
    block_hashes.push_for_tests(carrier_hash);
    block_hashes.commit_for_tests();
    state.update_latest_block_header_cache_for_tests(carrier.as_ref().header().clone());
    seed_empty_transaction_height_for_state_test(state, entry.merge_qc.carrier_height);
    assert_eq!(state.latest_block_hash_fast(), Some(carrier_hash));
    assert_eq!(
        state.exact_durable_block_count().unwrap(),
        usize::try_from(entry.merge_qc.carrier_height)
            .expect("test merge carrier height fits usize")
    );
}
fn set_commit_topology_from_keypairs(state: &State, keypairs: &[KeyPair]) {
    let mut topo = state.commit_topology.block();
    topo.clear();
    for keypair in keypairs {
        topo.push(PeerId::new(keypair.public_key().clone()));
    }
    topo.commit();
}
fn configure_commit_topology(state: &State, count: usize) -> Vec<KeyPair> {
    let mut peers = Vec::with_capacity(count);
    let mut keypairs = Vec::with_capacity(count);
    for _ in 0..count {
        let_row! { keypair = crate::state::checked_keypair_with_algorithm(iroha_crypto::Algorithm::BlsNormal) };
        peers.push(PeerId::new(keypair.public_key().clone()));
        keypairs.push(keypair);
    }
    let mut topo = state.commit_topology.block();
    topo.clear();
    for peer in peers {
        topo.push(peer);
    }
    topo.commit();
    let_row! { committed_peers: Vec<_> = keypairs .iter() .map(|keypair| PeerId::new(keypair.public_key().clone())) .collect() };
    let mut world_block = state.world.block();
    {
        let mut peers = world_block.peers_mut_for_testing().transaction();
        for peer in committed_peers {
            if !peers.iter().any(|existing| existing == &peer) {
                peers.push(peer);
            }
        }
        peers.apply();
    }
    world_block.commit();
    seed_consensus_keys_with_pops(state, &keypairs);
    keypairs
}
fn configure_commit_topology_preserving_world_peers(state: &State, count: usize) -> Vec<KeyPair> {
    let_row! { keypairs: Vec<_> = (0..count) .map(|_| crate::state::checked_keypair_with_algorithm(iroha_crypto::Algorithm::BlsNormal)) .collect() };
    set_commit_topology_from_keypairs(state, &keypairs);
    seed_consensus_keys_with_pops(state, &keypairs);
    keypairs
}
fn record_commit_ready_merge_candidate_with_lanes(
    state: &mut State,
    count: usize,
    first_height: u64,
) -> (
    crate::merge::MergeLedgerCandidate,
    Vec<KeyPair>,
    Vec<KeyPair>,
) {
    assert!(count > 0, "test helper requires at least one lane");
    let lane_count = u32::try_from(count).expect("lane count fits in u32");
    let (validator_ids, validator_keypairs) = bls_accounts_in("validators", 4);
    seed_consensus_keys_with_pops(state, &validator_keypairs);
    let_row! { lanes: Vec<LaneConfig> = (0..lane_count) .map(|idx| { if idx == 0 { LaneConfig::default() } else { LaneConfig { id: LaneId::new(idx), alias: format!("lane-{idx}"), dataspace_id: DataSpaceId::UNIVERSAL, ..LaneConfig::default() } } }) .collect() };
    let_row! { lane_catalog = LaneCatalog::new( core::num::NonZeroU32::new(lane_count).expect("non-zero"), lanes, ) .expect("lane catalog") };
    let_row! { nexus = iroha_config::parameters::actual::Nexus { lane_catalog, ..iroha_config::parameters::actual::Nexus::default() } };
    state.set_nexus(nexus).expect("apply Nexus lane catalog");
    let_row! { registry_entries: Vec<_> = (0..lane_count) .map(|idx| { ( LaneId::new(idx), DataSpaceId::UNIVERSAL, validator_ids.clone(), ) }) .collect() };
    install_lane_manifest_registry(state, &registry_entries);
    let commit_keypairs = configure_commit_topology_preserving_world_peers(state, 1);
    for idx in 0..lane_count {
        let_row! { envelope = seed_effect_authenticated_relay_for_merge_test( state, sample_lane_relay_envelope_for_state( state, first_height, LaneId::new(idx), &validator_keypairs, ), ) };
        state
            .record_lane_relay(&envelope)
            .expect("commit-ready relay accepted");
    }
    ensure_merge_carrier_parent_for_test(state);
    let_row! { candidate = state .merge_entry_candidates_from_lane_relays() .into_iter() .next() .expect("merge candidate from recorded relays") };
    (candidate, commit_keypairs, validator_keypairs)
}
fn empty_global_block_after(previous: Option<&SignedBlock>) -> SignedBlock {
    let_row! { creation_time_ms = previous.map_or(1_700_000_000_000, |block| { block.header().creation_time_ms.saturating_add(1) }) };
    autoscale_signed_block_with_committed_fragments(previous, creation_time_ms, 0)
}
fn merge_carrier_finality_fixture_keypair() -> KeyPair {
    KeyPair::try_from_seed(vec![0xD3; 32], Algorithm::BlsNormal)
        .expect("derive deterministic merge-carrier finality fixture key")
}
fn merge_carrier_finality_artifact(
    block: &SignedBlock,
    parent: Option<&V2FinalityArtifact>,
) -> V2FinalityArtifact {
    merge_carrier_finality_artifact_with_network(
        block,
        parent,
        iroha_data_model::NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"state-merge-carrier-finality-test",
            )),
        ),
    )
}
fn merge_carrier_finality_artifact_with_network(
    block: &SignedBlock,
    parent: Option<&V2FinalityArtifact>,
    network_id: iroha_data_model::NetworkId,
) -> V2FinalityArtifact {
    let keypair = merge_carrier_finality_fixture_keypair();
    let_row! { roster = vec![ValidatorPower { validator: PeerId::new(keypair.public_key().clone()), power: 1, }] };
    let height = block.header().height().get();
    assert_eq!(
        parent.map_or(1, |artifact| artifact.height.saturating_add(1)),
        height,
        "merge-carrier finality fixtures must form a contiguous chain"
    );
    let_row! { context = HeightContext { network_id, protocol_version: PROTOCOL_VERSION, height, epoch: 0, epoch_end_height: u64::MAX, next_epoch_snapshot: None, mode: ConsensusMode::Permissioned, parent_commit_qc: parent.map(|artifact| artifact.commit_qc.clone()), snapshot_bootstrap: None, quorum: DualQuorum::from_roster(&roster).expect("valid one-validator fixture quorum"), roster, nexus_amx_context_hash: Hash::new(b"state merge finality nexus AMX context"), execution_policy_hash: Hash::new(b"state merge finality execution policy"), da_layout: DataAvailabilityLayout { encoding: PayloadEncoding::ReedSolomon16, chunk_size_bytes: 1024, data_shards: 1, parity_shards: 1, max_payload_size_bytes: 4096, max_chunk_count: 8, }, leader_seed: [0xD3; 32], } };
    let executed_block_wire = block.encode_wire().expect("canonical executed block wire");
    let_row! { mut execution_commitment = ExecutionCommitment::new_without_merge_carrier( Hash::new(b"state merge finality parent state"), Hash::new(b"state merge finality post state"), Hash::new(b"state merge finality ordinary writes"), None, 0, 1, Hash::new(&executed_block_wire), ) .expect("canonical merge-carrier finality execution commitment") };
    execution_commitment.executed_block_wire_len =
        u64::try_from(executed_block_wire.len()).expect("fixture wire length fits u64");
    execution_commitment.merge_carrier = block
        .execution_context()
        .and_then(|bundle| bundle.merge_entry.as_ref())
        .map(|reference| {
            iroha_data_model::block::consensus_v2::MergeCarrierCommitmentV1::new(
                reference.entry_hash,
            )
        });
    let_row! { subject = BlockSubject { parent_block_hash: block.header().prev_block_hash(), block_hash: block.hash(), payload_hash: block .canonical_proposal_wire_hash() .expect("canonical proposal block wire"), } };
    let_row! { round = ConsensusRound { context_id: context.id(), height, view: block.header().view_change_index(), } };
    let_row! { mut commit_qc = QuorumCertificate { round, proposal_round: round, phase: GlobalPhase::Commit, subject, execution_commitment, signers: vec![0], aggregate_signature: vec![1], } };
    let_row! { preimage = commit_qc .signer_preimage(&context, 0) .expect("valid merge-carrier finality fixture signer") };
    let_row! { signature = Signature::try_new(keypair.private_key(), &preimage) .expect("sign merge-carrier finality fixture vote") .payload() .to_vec() };
    commit_qc.aggregate_signature =
        iroha_crypto::bls_normal_aggregate_signatures(&[signature.as_slice()])
            .expect("aggregate merge-carrier finality fixture vote");
    let_row! { validator_set_pops = vec![ bls_normal_pop_prove(keypair.private_key()) .expect("derive merge-carrier finality fixture PoP"), ] };
    let artifact = V2FinalityArtifact::new(context, subject, commit_qc, validator_set_pops);
    artifact
        .verify()
        .expect("merge-carrier finality fixture is cryptographically valid");
    artifact
}
fn persist_merge_carrier_finality_for_state_test(kura: &Kura, block: &SignedBlock) {
    let height = block.header().height().get();
    let_row! { parent = height .checked_sub(1) .filter(|parent_height| *parent_height > 0) .map(|parent_height| { kura.v2_finality_artifact(parent_height) .expect("read merge-carrier parent finality") .unwrap_or_else(|| { let parent = kura .get_block( NonZeroUsize::new( usize::try_from(parent_height) .expect("fixture parent height fits usize"), ) .expect("fixture parent height is non-zero"), ) .expect("merge-carrier parent block is durable"); persist_merge_carrier_finality_for_state_test(kura, parent.as_ref()); kura.v2_finality_artifact(parent_height) .expect("reread merge-carrier parent finality") .expect("merge-carrier parent finality is now durable") }) }) };
    let artifact = merge_carrier_finality_artifact(block, parent.as_ref());
    let_row! { _ = kura .store_v2_finality_artifact(&artifact) .expect("persist exact merge-carrier finality fixture") };
}
fn certified_merge_carrier_after(previous: &SignedBlock, entry: &MergeLedgerEntry) -> SignedBlock {
    let mut carrier = empty_global_block_after(Some(previous));
    assert_eq!(
        carrier.header().height().get(),
        entry.merge_qc.carrier_height,
        "merge fixture carrier height must match its QC"
    );
    assert_eq!(
        carrier.header().prev_block_hash(),
        Some(entry.merge_qc.carrier_parent_hash),
        "merge fixture carrier parent must match its QC"
    );
    assert_eq!(
        carrier.header().view_change_index(),
        entry.merge_qc.view,
        "merge fixture carrier view must match its QC"
    );
    let_row! { context = carrier .execution_context() .cloned() .unwrap_or_else(|| iroha_data_model::block::BlockExecutionContextBundle::new(Vec::new())) .with_merge_entry(iroha_data_model::block::CertifiedMergeLedgerReference::new( entry, )) };
    carrier.set_execution_context(Some(context));
    carrier
}
fn commit_block_metadata_to_state(state: &State, block: &SignedBlock) {
    let mut state_block = state.block(block.header().clone());
    state_block.block_hashes.push(block.hash());
    insert_empty_transaction_block_for_state_commit(&mut state_block, block);
    state_block
        .commit()
        .expect("test global block metadata must commit to State");
}
fn commit_exact_merge_carrier_to_state(
    state: &State,
    carrier: &SignedBlock,
    entry: &MergeLedgerEntry,
) {
    persist_merge_carrier_finality_for_state_test(&state.kura, carrier);
    let_row! { mut state_block = state .block_with_certified_merge_entry(carrier.header().clone(), entry) .expect("certified merge entry must stage on its exact carrier") };
    state_block.block_hashes.push(carrier.hash());
    insert_empty_transaction_block_for_state_commit(&mut state_block, carrier);
    state_block
        .commit()
        .expect("exact certified merge carrier must commit to State");
    state
        .record_globally_committed_merge_entry(entry, MergeLedgerPublicationMode::LiveCommit)
        .expect("exact certified merge carrier must publish its merge cache");
}
fn configured_single_lane_merge_state() -> (State, Vec<KeyPair>, Vec<KeyPair>, SignedBlock) {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::default(), Arc::clone(&kura), query);
    state
        .set_nexus(iroha_config::parameters::actual::Nexus {
            ..iroha_config::parameters::actual::Nexus::default()
        })
        .expect("enable single-lane Nexus merge fixture");
    let (validator_ids, validator_keypairs) = bls_accounts_in("validators", 4);
    seed_consensus_keys_with_pops(&state, &validator_keypairs);
    install_lane_manifest_registry(
        &state,
        &[(LaneId::SINGLE, DataSpaceId::UNIVERSAL, validator_ids)],
    );
    let commit_keypairs = configure_commit_topology_preserving_world_peers(&state, 1);
    let parent = empty_global_block_after(None);
    kura.store_block(Arc::new(parent.clone()))
        .expect("store merge fixture carrier parent");
    commit_block_metadata_with_genesis_checkpoint_to_state(&state, &parent);
    (state, validator_keypairs, commit_keypairs, parent)
}
fn queue_plan_entrypoint_for_state_test(state: &State, tag: u8) -> TransactionEntrypoint {
    let_row! { transaction_keypair = KeyPair::try_from_seed(vec![tag.wrapping_add(0x31); 32], Algorithm::Ed25519) .expect("deterministic QueuePlan transaction key") };
    let authority = AccountId::new(transaction_keypair.public_key().clone());
    let_row! { mut transaction = TransactionBuilder::new( *state.network_id_ref(), authority, iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None), ) };
    transaction.set_creation_time(Duration::from_millis(u64::from(tag).saturating_add(1)));
    TransactionEntrypoint::External(
        transaction
            .with_instructions([Log::new(Level::INFO, format!("queue-plan-admission-{tag}"))])
            .sign(transaction_keypair.private_key()),
    )
}
fn queue_plan_admission_certificate_for_state_test(
    state: &State,
    routing_plan: crate::queue::RoutingPlan,
    validator_keypairs: &[KeyPair],
    authority_height: u64,
    tag: u8,
) -> (crate::torii_proxy::QueuePlanAdmissionBindingV2, Vec<u8>) {
    let_row! { proposal_height = authority_height .checked_add(1) .expect("fixture proposal height") };
    let_row! { predecessor_block_hash = if authority_height == 0 { None } else { usize::try_from(authority_height) .ok() .and_then(|height| height.checked_sub(1)) .and_then(|index| state.block_hashes.view().get(index).copied()) } };
    let_row! { route_incarnations = routing_plan .legs() .into_iter() .map(|leg| { let validator_set = crate::queue::queue_plan_authoritative_peers_in_view_at_height( &state.view(), leg.route, proposal_height, ) .expect("fixture route authority"); assert!( !validator_set.is_empty(), "fixture route must have authoritative validators" ); crate::queue::QueuePlanRouteIncarnationV2 { leg, lane_incarnation: state .lane_incarnation(leg.route.lane_id) .expect("fixture route has an active incarnation"), validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1, validator_set_hash: HashOf::new(&validator_set), validator_count: u16::try_from(validator_set.len()) .expect("fixture validator count"), durability_threshold: u16::try_from(validator_set.len().div_ceil(3)) .expect("fixture durability threshold"), validator_set, } }) .collect::<Vec<_>>() };
    let_row! { admission_context = crate::queue::QueuePlanAdmissionContextV2 { version: crate::queue::QUEUE_PLAN_ADMISSION_CONTEXT_VERSION_V2, authority_height, proposal_height, predecessor_block_hash, routing_plan_digest: routing_plan.digest(), route_incarnations, } };
    let entrypoint = queue_plan_entrypoint_for_state_test(state, tag);
    let_row! { binding = crate::torii_proxy::QueuePlanAdmissionBindingV2::new( &state.network_id, &entrypoint, &routing_plan, admission_context, u64::from(tag).saturating_add(100), ) .expect("canonical QueuePlan admission binding") };
    let_row! { certificate = queue_plan_admission_certificate_bytes_for_state_test(&binding, validator_keypairs) };
    (binding, certificate)
}
fn queue_plan_admission_certificate_bytes_for_state_test(
    binding: &crate::torii_proxy::QueuePlanAdmissionBindingV2,
    validator_keypairs: &[KeyPair],
) -> Vec<u8> {
    let binding_hash = binding.canonical_hash();
    let coordinator = &binding.admission_context.route_incarnations[0];
    let_row! { attestations = coordinator .validator_set .iter() .take(usize::from(coordinator.durability_threshold)) .enumerate() .map(|(index, validator)| { let keypair = validator_keypairs .iter() .find(|keypair| keypair.public_key() == validator.public_key()) .expect("fixture retains every authoritative validator key"); let validator_index = u16::try_from(index).expect("fixture validator index fits u16"); let signing_bytes = crate::torii_proxy::queue_plan_admission_attestation_signing_bytes_v2( binding_hash, validator_index, ) .expect("QueuePlan attestation preimage"); crate::torii_proxy::QueuePlanAdmissionAttestationV2 { version: crate::torii_proxy::QUEUE_PLAN_ADMISSION_ATTESTATION_VERSION_V2, validator_index, signature: Signature::try_new(keypair.private_key(), &signing_bytes) .expect("QueuePlan attestation signature"), } }) .collect() };
    let_row! { certificate = crate::torii_proxy::QueuePlanAdmissionCertificateV2 { version: crate::torii_proxy::QUEUE_PLAN_ADMISSION_CERTIFICATE_VERSION_V2, binding: binding.clone(), attestations, } };
    norito::to_bytes(&certificate).expect("canonical QueuePlan admission certificate")
}
