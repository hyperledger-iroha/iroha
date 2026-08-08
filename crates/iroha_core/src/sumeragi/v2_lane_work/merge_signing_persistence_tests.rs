#[test]
fn canonical_kura_anchor_cannot_bypass_route_reset_or_incarnation_guards() {
    let lane_id = LaneId::SINGLE;
    let dataspace_id = DataSpaceId::UNIVERSAL;

    {
        let (adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 3);
        let incarnation = adapter
            .state
            .lane_incarnation_at_height(lane_id, adapter.context.height)
            .expect("canonical lane incarnation is active");
        let proposal = proposal_for_route(
            &adapter,
            &keys,
            lane_id,
            dataspace_id,
            incarnation,
            adapter.context.height,
            1,
        );
        let canonical = store_canonical_anchor(&adapter, &proposal, &keys[0]);
        assert!(
            adapter.kura.read_lane_block_artifact(lane_id, 1).is_some(),
            "fixture must retain a raw canonical Kura anchor"
        );
        assert!(adapter.canonical_anchor_for_proposal(&canonical).is_some());
        assert!(
            adapter
                .canonical_proposal_for_vote_body(&canonical.vote_body(CertPhase::Prepare))
                .is_some()
        );

        mark_lane_reset(&adapter, lane_id, adapter.context.height);
        assert!(
            adapter.kura.read_lane_block_artifact(lane_id, 1).is_some(),
            "reset validation must be tested with the canonical file still present"
        );
        assert!(
            adapter.canonical_anchor_for_proposal(&canonical).is_none(),
            "a canonical file at the reset watermark is not an admissible anchor"
        );
        assert!(
            adapter
                .canonical_proposal_for_vote_body(&canonical.vote_body(CertPhase::Prepare))
                .is_none(),
            "historical vote recovery must apply the reset guard too"
        );
        assert!(
            !adapter.lane_proposal_authorized(&canonical, None, true, 0),
            "canonical-anchor fast path must not bypass the reset guard"
        );
    }

    {
        let (adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 2);
        let incarnation = adapter
            .state
            .lane_incarnation_at_height(lane_id, adapter.context.height)
            .expect("canonical lane incarnation is active");
        let wrong_dataspace = DataSpaceId::new(91);
        let proposal = proposal_for_route(
            &adapter,
            &keys,
            lane_id,
            wrong_dataspace,
            incarnation,
            adapter.context.height,
            1,
        );
        if let Some(canonical) = try_store_canonical_anchor(&adapter, &proposal, &keys[0]) {
            assert!(
                adapter.canonical_anchor_for_proposal(&canonical).is_none(),
                "canonical storage must not make an inactive dataspace route authoritative"
            );
            assert!(
                adapter
                    .canonical_proposal_for_vote_body(&canonical.vote_body(CertPhase::Prepare))
                    .is_none()
            );
            assert!(!adapter.lane_proposal_authorized(&canonical, None, true, 0));
        } else {
            assert!(
                adapter.kura.read_lane_block_artifact(lane_id, 1).is_none(),
                "Kura must not expose an artifact rejected for inactive route geometry"
            );
        }
    }

    {
        let (adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 2);
        let active_incarnation = adapter
            .state
            .lane_incarnation_at_height(lane_id, adapter.context.height)
            .expect("canonical lane incarnation is active");
        let stale_incarnation = Hash::new(b"canonical-but-retired-lane-incarnation");
        assert_ne!(stale_incarnation, active_incarnation);
        let proposal = proposal_for_route(
            &adapter,
            &keys,
            lane_id,
            dataspace_id,
            stale_incarnation,
            adapter.context.height,
            1,
        );
        if let Some(canonical) = try_store_canonical_anchor(&adapter, &proposal, &keys[0]) {
            assert!(
                adapter.canonical_anchor_for_proposal(&canonical).is_none(),
                "canonical storage must not authorize a retired incarnation"
            );
            assert!(
                adapter
                    .canonical_proposal_for_vote_body(&canonical.vote_body(CertPhase::Prepare))
                    .is_none()
            );
            assert!(!adapter.lane_proposal_authorized(&canonical, None, true, 0));
        } else {
            assert!(
                adapter.kura.read_lane_block_artifact(lane_id, 1).is_none(),
                "Kura must not expose an artifact rejected for a retired incarnation"
            );
        }
    }
}

#[test]
fn merge_signers_must_meet_both_count_and_power_quorums() {
    let (adapter, keys) = fixture(wire::ConsensusMode::Npos);
    assert!(!adapter.frozen_dual_quorum_met(&[0, 1]));
    assert!(!adapter.frozen_dual_quorum_met(&[1, 2, 3]));
    assert!(adapter.frozen_dual_quorum_met(&[0, 1, 3]));
    assert!(adapter.frozen_dual_quorum_met(&[0, 1, 2, 3]));

    let low_power_count_quorum =
        missing_sidecar_reference_with_signers(&adapter, &keys, 0, &[1, 2, 3]);
    assert!(matches!(
        authenticate_bounded_merge_sidecar_holders(
            &adapter.context,
            &low_power_count_quorum
        ),
        Err(reason) if reason.contains("dual quorum")
    ));

    let dual_quorum = missing_sidecar_reference_with_signers(&adapter, &keys, 0, &[0, 1, 3]);
    authenticate_bounded_merge_sidecar_holders(&adapter.context, &dual_quorum)
        .expect("the same verifier accepts count plus strict power quorum");
}

fn merge_candidate_for_persistence_retry(
    adapter: &V2LaneWorkAdapter,
    view: wire::View,
) -> crate::merge::MergeLedgerCandidate {
    let nexus = adapter.state.nexus_snapshot();
    let active_lanes = nexus
        .lane_catalog
        .lanes()
        .iter()
        .map(|lane| iroha_data_model::merge::MergeLaneBinding {
            lane_id: lane.id,
            dataspace_id: lane.dataspace_id,
            lane_config_hash: crate::merge::merge_lane_config_hash(lane),
            incarnation: adapter
                .state
                .lane_incarnation_at_height(lane.id, adapter.context.height)
                .expect("fixture lane incarnation is active"),
            activation_height: 1,
        })
        .collect::<Vec<_>>();
    let incarnation_entries = active_lanes
        .iter()
        .map(
            |binding| iroha_data_model::nexus::LaneLifecycleIncarnationEntry {
                lane_id: binding.lane_id,
                incarnation: binding.incarnation,
            },
        )
        .collect::<Vec<_>>();
    crate::merge::MergeLedgerCandidate {
        version: crate::merge::MergeLedgerCandidate::VERSION,
        epoch_id: 1,
        view,
        carrier_height: adapter.context.height,
        carrier_parent_hash: adapter
            .context
            .parent_commit_qc
            .as_ref()
            .expect("non-genesis fixture parent")
            .subject
            .block_hash,
        lane_catalog_hash: iroha_data_model::nexus::LaneLifecycleParameterV1::catalog_hash(
            &nexus.lane_catalog,
        ),
        active_lanes: active_lanes.clone(),
        incarnation_root: iroha_data_model::nexus::LaneLifecycleParameterV1::incarnation_root(
            &incarnation_entries,
        ),
        activation_root: crate::merge::merge_activation_root(&active_lanes),
        lane_snapshots: Vec::new(),
        lane_drain_certificates: Vec::new(),
        queue_plan_admissions: Vec::new(),
        execution_batch: None,
        global_state_root: crate::merge::reduce_merge_hint_roots(&[]),
    }
}

#[test]
fn merge_candidate_selection_preserves_authorized_digest_and_relay_priority() {
    let relay_digest = Hash::new(b"relay candidate");
    let installed_digest = Hash::new(b"installed candidate");
    let digest = |candidate: &(u8, Hash)| candidate.1;

    assert_eq!(
        preferred_merge_candidates(
            None,
            vec![(2, relay_digest)],
            vec![(3, installed_digest)],
            digest,
        ),
        vec![(2, relay_digest)],
        "the deterministic leader candidate takes priority over opportunistic installation"
    );
    assert_eq!(
        preferred_merge_candidates(
            Some(relay_digest),
            vec![(2, relay_digest)],
            vec![(3, installed_digest)],
            digest,
        ),
        vec![(2, relay_digest)],
        "a durable signing decision must survive later candidate installation"
    );
    assert!(
        preferred_merge_candidates(
            Some(Hash::new(b"unavailable authorized candidate")),
            vec![(2, relay_digest)],
            vec![(3, installed_digest)],
            digest,
        )
        .is_empty(),
        "an unavailable durable decision must fail closed instead of selecting another digest"
    );
}

#[test]
fn installed_execution_candidate_with_wrong_carrier_context_never_reaches_local_signing() {
    let (mut adapter, _) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    adapter
        .retain_merge_sidecars_for_global_view(0, None, None)
        .expect("install exact unlocked reducer directive");
    adapter.drain_effects(usize::MAX);

    let mut candidate = merge_candidate_for_persistence_retry(&adapter, 0);
    candidate.execution_batch = Some(iroha_data_model::merge::MergeExecutionBatch {
        version: 1,
        base_state_height: adapter.context.height.saturating_sub(1),
        base_state_hash: HashOf::from_untyped_unchecked(Hash::new(b"retired execution base state")),
        application_block_header: BlockHeader::new(
            NonZeroU64::new(adapter.context.height).expect("non-zero carrier height"),
            Some(candidate.carrier_parent_hash),
            None,
            None,
            1,
            candidate.view,
        ),
        lanes: Vec::new(),
        entrypoint_count: 1,
        entrypoint_merkle_root: HashOf::from_untyped_unchecked(Hash::new(
            b"retired execution entrypoints",
        )),
        result_merkle_root: HashOf::from_untyped_unchecked(Hash::new(b"retired execution results")),
        execution_root: Hash::new(b"retired execution root"),
        application_write_set_root: Hash::new(b"retired execution application writes"),
        write_set_root: Hash::new(b"retired execution writes"),
        expected_post_state_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"retired execution post state",
        )),
        batch_hash: Hash::new(b"retired execution batch"),
    });
    let digest = crate::merge::merge_qc_message_digest(
        &adapter.context.chain_id,
        &candidate,
        VALIDATOR_SET_HASH_VERSION_V1,
        adapter.frozen_validator_set_hash(),
    );
    let key = MergeKey {
        epoch_id: candidate.epoch_id,
        view: candidate.view,
        digest,
    };
    adapter.merge_entries.insert(
        key,
        PendingMerge {
            stage: PendingMergeStage::Collecting(candidate),
            signatures: BTreeMap::new(),
        },
    );

    adapter
        .refresh_merge_candidates(0)
        .expect("carrier-mismatched execution candidate fails closed without signing");
    assert!(adapter.merge_entries[&key].signatures.is_empty());
    assert!(
        adapter
            .drain_effects(usize::MAX)
            .iter()
            .all(|effect| !matches!(effect, V2LaneWorkEffect::BroadcastMerge(_))),
        "a carrier-mismatched execution-batch candidate must not reach the private key"
    );
}

fn merge_signing_context_for_test(
    adapter: &V2LaneWorkAdapter,
    candidate: &crate::merge::MergeLedgerCandidate,
) -> MergeSigningContextV1 {
    MergeSigningContextV1 {
        epoch_id: candidate.epoch_id,
        view: candidate.view,
        carrier_height: candidate.carrier_height,
        parent_hash: candidate.carrier_parent_hash,
        validator_set_hash: adapter.frozen_validator_set_hash(),
    }
}

fn remote_merge_leader_view(adapter: &V2LaneWorkAdapter) -> wire::View {
    let local = adapter
        .local_validator_index()
        .expect("fixture local validator is in the frozen roster");
    let search_bound = u64::try_from(adapter.context.roster.len())
        .expect("fixture roster length fits u64")
        .saturating_mul(2);
    (0..search_bound)
        .find(|view| adapter.context.leader(*view) != local)
        .expect("rotating leader schedule reaches a remote validator")
}

fn signed_merge_share_for_test(
    adapter: &V2LaneWorkAdapter,
    keys: &[KeyPair],
    candidate: &crate::merge::MergeLedgerCandidate,
    signer: wire::ValidatorIndex,
) -> MergeCommitteeSignature {
    let digest = crate::merge::merge_qc_message_digest(
        &adapter.context.chain_id,
        candidate,
        VALIDATOR_SET_HASH_VERSION_V1,
        adapter.frozen_validator_set_hash(),
    );
    let signature = Signature::try_new(
        keys[usize::try_from(signer).expect("fixture signer index fits usize")].private_key(),
        digest.as_ref(),
    )
    .expect("sign merge-share test fixture")
    .payload()
    .to_vec();
    MergeCommitteeSignature {
        version: MERGE_COMMITTEE_SIGNATURE_VERSION_V2,
        epoch_id: candidate.epoch_id,
        view: candidate.view,
        signer,
        message_digest: digest,
        bls_sig: signature,
        leader_candidate_body: (signer == adapter.context.leader(candidate.view))
            .then(|| candidate.canonical_bytes()),
    }
}

fn synthetic_merge_execution_batch_for_test(
    adapter: &V2LaneWorkAdapter,
    application_block_header: BlockHeader,
) -> iroha_data_model::merge::MergeExecutionBatch {
    iroha_data_model::merge::MergeExecutionBatch {
        version: 1,
        base_state_height: adapter.context.height.saturating_sub(1),
        base_state_hash: adapter.state.lane_execution_state_hash(),
        application_block_header,
        lanes: Vec::new(),
        entrypoint_count: 0,
        entrypoint_merkle_root: HashOf::from_untyped_unchecked(Hash::new(
            b"synthetic execution entrypoints",
        )),
        result_merkle_root: HashOf::from_untyped_unchecked(Hash::new(
            b"synthetic execution results",
        )),
        execution_root: Hash::new(b"synthetic execution root"),
        application_write_set_root: Hash::new(b"synthetic execution application writes"),
        write_set_root: Hash::new(b"synthetic execution writes"),
        expected_post_state_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"synthetic execution post state",
        )),
        batch_hash: Hash::new(b"synthetic execution batch"),
    }
}

#[test]
fn authenticated_leader_candidate_recovers_exact_follower_share_after_restart() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    let view = remote_merge_leader_view(&adapter);
    let candidate = record_production_merge_candidate_for_persistence_retry(&adapter, &keys, view);
    let candidate_bytes = candidate.canonical_bytes();
    let leader = adapter.context.leader(view);
    let local = adapter
        .local_validator_index()
        .expect("fixture local validator is in the frozen roster");
    assert_ne!(leader, local);
    adapter
        .retain_merge_sidecars_for_global_view(view, None, None)
        .expect("install exact unlocked follower directive");
    assert!(
        adapter
            .drain_effects(usize::MAX)
            .iter()
            .all(|effect| !matches!(effect, V2LaneWorkEffect::BroadcastMerge(_))),
        "a follower must not select or sign a proposer-local candidate"
    );

    let leader_share = signed_merge_share_for_test(&adapter, &keys, &candidate, leader);
    let leader_digest = leader_share.message_digest;
    assert_eq!(
        leader_share.leader_candidate_body.as_deref(),
        Some(candidate_bytes.as_slice())
    );
    assert_eq!(
        adapter
            .accept_merge_signature(leader_share.clone(), view)
            .expect("admit authenticated leader candidate"),
        V2LaneIngressOutcome::Inserted
    );
    assert_eq!(
        adapter
            .accept_merge_signature(leader_share, view)
            .expect("re-admit exact leader candidate"),
        V2LaneIngressOutcome::Duplicate
    );
    let follower_share = adapter
        .drain_effects(usize::MAX)
        .into_iter()
        .find_map(|effect| match effect {
            V2LaneWorkEffect::BroadcastMerge(share) if share.signer == local => Some(share),
            _ => None,
        })
        .expect("leader admission releases one local follower share");
    assert_eq!(follower_share.version, MERGE_COMMITTEE_SIGNATURE_VERSION_V2);
    assert_eq!(follower_share.message_digest, leader_digest);
    assert!(
        follower_share.leader_candidate_body.is_none(),
        "a follower transmission must remain bodyless"
    );
    assert_eq!(
        adapter
            .merge_signing_guard
            .authorized_candidate(&merge_signing_context_for_test(&adapter, &candidate))
            .expect("read exact durable candidate"),
        Some((
            follower_share.message_digest,
            candidate.clone(),
            candidate_bytes.clone(),
        ))
    );

    let context = adapter.context.clone();
    let local_peer = adapter.local_peer.clone();
    let key_pair = adapter.key_pair.clone();
    let state = Arc::clone(&adapter.state);
    let kura = Arc::clone(&adapter.kura);
    let limits = adapter.limits;
    drop(adapter);
    let mut reopened = V2LaneWorkAdapter::new(
        context, local_peer, key_pair, true, state, kura, limits, None,
    )
    .expect("reopen adapter with exact pre-QC journal");
    reopened
        .retain_merge_sidecars_for_global_view(view, None, None)
        .expect("reconstruct exact candidate under the follower directive");
    let recovered = reopened
        .drain_effects(usize::MAX)
        .into_iter()
        .find_map(|effect| match effect {
            V2LaneWorkEffect::BroadcastMerge(share) if share.signer == local => Some(share),
            _ => None,
        })
        .expect("restart reconstructs the exact follower share");
    assert_eq!(recovered.message_digest, follower_share.message_digest);
    assert!(recovered.leader_candidate_body.is_none());

    reopened
        .schedule_merge_share_retransmissions(view)
        .expect("schedule exact follower retransmission");
    let retransmitted = reopened
        .drain_effects(usize::MAX)
        .into_iter()
        .find_map(|effect| match effect {
            V2LaneWorkEffect::BroadcastMerge(share) if share.signer == local => Some(share),
            _ => None,
        })
        .expect("retransmit recovered follower share");
    assert_eq!(retransmitted, recovered);
    assert_eq!(
        reopened
            .merge_signing_guard
            .authorized_candidate(&merge_signing_context_for_test(&reopened, &candidate))
            .expect("read restarted exact durable candidate"),
        Some((recovered.message_digest, candidate, candidate_bytes,))
    );
}

#[test]
fn merge_share_transport_rejects_omission_nonleader_body_and_legacy_version() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    let view = remote_merge_leader_view(&adapter);
    let candidate = record_production_merge_candidate_for_persistence_retry(&adapter, &keys, view);
    let leader = adapter.context.leader(view);
    let follower = adapter
        .local_validator_index()
        .expect("fixture local validator is in the frozen roster");
    adapter
        .retain_merge_sidecars_for_global_view(view, None, None)
        .expect("install exact unlocked follower directive");
    adapter.drain_effects(usize::MAX);

    let mut omitted = signed_merge_share_for_test(&adapter, &keys, &candidate, leader);
    omitted.leader_candidate_body = None;
    assert_eq!(
        adapter
            .accept_merge_signature(omitted, view)
            .expect("reject omitted leader body"),
        V2LaneIngressOutcome::Rejected
    );

    let mut legacy = signed_merge_share_for_test(&adapter, &keys, &candidate, leader);
    legacy.version = MERGE_COMMITTEE_SIGNATURE_VERSION_V2.saturating_sub(1);
    assert_eq!(
        adapter
            .accept_merge_signature(legacy, view)
            .expect("reject legacy merge-share version"),
        V2LaneIngressOutcome::Rejected
    );

    let mut follower_with_body = signed_merge_share_for_test(&adapter, &keys, &candidate, follower);
    assert!(follower_with_body.leader_candidate_body.is_none());
    follower_with_body.leader_candidate_body = Some(candidate.canonical_bytes());
    assert_eq!(
        adapter
            .accept_merge_signature(follower_with_body, view)
            .expect("reject nonleader candidate body"),
        V2LaneIngressOutcome::Rejected
    );
    assert_eq!(
        adapter
            .merge_signing_guard
            .authorized_candidate(&merge_signing_context_for_test(&adapter, &candidate))
            .expect("read untouched signing guard"),
        None
    );
}

#[test]
fn merge_leader_candidate_body_is_canonical_under_ambient_layout() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    let view = remote_merge_leader_view(&adapter);
    let candidate = record_production_merge_candidate_for_persistence_retry(&adapter, &keys, view);
    adapter
        .retain_merge_sidecars_for_global_view(view, None, None)
        .expect("install exact unlocked follower directive");
    adapter.drain_effects(usize::MAX);
    let leader = adapter.context.leader(view);
    let share = signed_merge_share_for_test(&adapter, &keys, &candidate, leader);
    let parent = adapter
        .state
        .latest_block_header_fast()
        .expect("fixture has exact committed parent");
    let alternate_flags =
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;

    let decoded = {
        let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        adapter.decode_and_validate_leader_candidate(&share, view, &parent)
    }
    .expect("canonical leader body remains valid under alternate ambient flags");
    assert_eq!(decoded, candidate);

    let canonical_body =
        norito::encode_canonical(&candidate).expect("encode canonical merge candidate");
    assert_eq!(
        share.leader_candidate_body.as_deref(),
        Some(canonical_body.as_slice())
    );
    let alternate_body = {
        let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        norito::to_bytes(&candidate).expect("encode alternate-layout merge candidate")
    };
    assert_ne!(alternate_body, canonical_body);
    let mut noncanonical = share;
    noncanonical.leader_candidate_body = Some(alternate_body);
    let reason = {
        let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        adapter
            .decode_and_validate_leader_candidate(&noncanonical, view, &parent)
            .expect_err("alternate-layout leader body must fail closed")
    };
    assert!(
        reason.contains("not canonical"),
        "unexpected alternate-layout rejection: {reason}"
    );
}

#[test]
fn merge_leader_candidate_rejects_substitution_outer_epoch_and_oversize_before_journal() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    let view = remote_merge_leader_view(&adapter);
    let candidate = record_production_merge_candidate_for_persistence_retry(&adapter, &keys, view);
    let leader = adapter.context.leader(view);
    adapter
        .retain_merge_sidecars_for_global_view(view, None, None)
        .expect("install exact unlocked follower directive");
    adapter.drain_effects(usize::MAX);

    let mut substituted = candidate.clone();
    substituted.global_state_root = Hash::new(b"authenticated substituted merge body");
    let mut substituted_share = signed_merge_share_for_test(&adapter, &keys, &candidate, leader);
    substituted_share.leader_candidate_body = Some(substituted.canonical_bytes());
    assert_eq!(
        adapter
            .accept_merge_signature(substituted_share, view)
            .expect("reject body substitution"),
        V2LaneIngressOutcome::Rejected
    );

    let mut wrong_outer_epoch = signed_merge_share_for_test(&adapter, &keys, &candidate, leader);
    wrong_outer_epoch.epoch_id = wrong_outer_epoch.epoch_id.saturating_add(1);
    assert_eq!(
        adapter
            .accept_merge_signature(wrong_outer_epoch, view)
            .expect("reject outer epoch drift"),
        V2LaneIngressOutcome::Rejected
    );

    adapter.limits.merge_share_frame_capacity =
        iroha_config::parameters::defaults::sumeragi::V2_MERGE_LEADER_BODY_FRAME_HEADROOM_BYTES;
    let oversize = signed_merge_share_for_test(&adapter, &keys, &candidate, leader);
    assert_eq!(
        adapter
            .accept_merge_signature(oversize, view)
            .expect("reject candidate outside configured full-frame partition"),
        V2LaneIngressOutcome::Rejected
    );
    assert_eq!(
        adapter
            .merge_signing_guard
            .authorized_candidate(&merge_signing_context_for_test(&adapter, &candidate))
            .expect("read untouched signing guard"),
        None
    );
}

#[test]
fn authenticated_execution_candidate_rejects_noncanonical_carrier_context_header() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    let view = remote_merge_leader_view(&adapter);
    let mut candidate = merge_candidate_for_persistence_retry(&adapter, view);
    let expected_header = adapter
        .merge_carrier_context_header(view)
        .expect("derive exact deterministic carrier context");
    let wrong_creation_time = u64::try_from(expected_header.creation_time().as_millis())
        .expect("fixture carrier time fits u64")
        .checked_add(1)
        .expect("fixture carrier time can advance");
    let wrong_header = BlockHeader::new(
        expected_header.height(),
        expected_header.prev_block_hash(),
        None,
        None,
        wrong_creation_time,
        expected_header.view_change_index(),
    );
    assert_ne!(wrong_header, expected_header);
    candidate.execution_batch = Some(synthetic_merge_execution_batch_for_test(
        &adapter,
        wrong_header,
    ));
    assert!(
        candidate.lane_snapshots.is_empty(),
        "the carrier-context test must not be preempted by mixed candidate forms"
    );

    let leader = adapter.context.leader(view);
    let share = signed_merge_share_for_test(&adapter, &keys, &candidate, leader);
    adapter
        .retain_merge_sidecars_for_global_view(view, None, None)
        .expect("install exact unlocked follower directive");
    adapter.drain_effects(usize::MAX);
    let parent = adapter
        .state
        .latest_block_header_fast()
        .expect("fixture has exact committed parent");
    let reason = adapter
        .decode_and_validate_leader_candidate(&share, view, &parent)
        .expect_err("wrong-time execution candidate must not obtain a follower share");
    assert!(
        reason.contains("exact deterministic carrier context header"),
        "unexpected carrier-context rejection: {reason}"
    );
    assert_eq!(
        adapter
            .accept_merge_signature(share, view)
            .expect("reject execution candidate for an uncarryable header"),
        V2LaneIngressOutcome::Rejected
    );
    assert_eq!(
        adapter
            .merge_signing_guard
            .authorized_candidate(&merge_signing_context_for_test(&adapter, &candidate))
            .expect("read untouched signing guard"),
        None
    );
}

#[test]
fn authenticated_relay_candidate_cannot_be_relabelled_as_execution() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    let view = remote_merge_leader_view(&adapter);
    let mut candidate =
        record_production_merge_candidate_for_persistence_retry(&adapter, &keys, view);
    let exact_header = adapter
        .merge_carrier_context_header(view)
        .expect("derive exact deterministic carrier context");
    candidate.execution_batch = Some(synthetic_merge_execution_batch_for_test(
        &adapter,
        exact_header,
    ));
    let leader = adapter.context.leader(view);
    let share = signed_merge_share_for_test(&adapter, &keys, &candidate, leader);
    adapter
        .retain_merge_sidecars_for_global_view(view, None, None)
        .expect("install exact unlocked follower directive");
    adapter.drain_effects(usize::MAX);
    let parent = adapter
        .state
        .latest_block_header_fast()
        .expect("fixture has exact committed parent");
    let reason = adapter
        .decode_and_validate_leader_candidate(&share, view, &parent)
        .expect_err("relay snapshots cannot be relabeled as autonomous execution");
    assert!(
        reason.contains("execution candidates must not mix relay snapshots"),
        "unexpected authenticated execution rejection: {reason}"
    );
    assert_eq!(
        adapter
            .accept_merge_signature(share, view)
            .expect("reject unmarked autonomous candidate"),
        V2LaneIngressOutcome::Rejected
    );
    assert_eq!(
        adapter
            .merge_signing_guard
            .authorized_candidate(&merge_signing_context_for_test(&adapter, &candidate))
            .expect("read untouched signing guard"),
        None
    );
}

#[test]
fn durable_local_merge_claim_rejects_same_context_candidate_drift() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    let candidate = record_production_merge_candidate_for_persistence_retry(&adapter, &keys, 0);
    let signer = adapter
        .local_validator_index()
        .expect("fixture local validator is in the frozen roster");
    let first_digest = crate::merge::merge_qc_message_digest(
        &adapter.context.chain_id,
        &candidate,
        VALIDATOR_SET_HASH_VERSION_V1,
        adapter.frozen_validator_set_hash(),
    );
    adapter
        .retain_merge_sidecars_for_global_view(0, None, None)
        .expect("install exact unlocked reducer directive");
    assert_eq!(
        adapter
            .merge_claims
            .get(&(candidate.epoch_id, candidate.view, signer)),
        Some(&first_digest)
    );

    let mut drifted = candidate.clone();
    drifted.global_state_root = Hash::new(b"same-context conflicting merge payload");
    let drifted_digest = crate::merge::merge_qc_message_digest(
        &adapter.context.chain_id,
        &drifted,
        VALIDATOR_SET_HASH_VERSION_V1,
        adapter.frozen_validator_set_hash(),
    );
    assert_ne!(first_digest, drifted_digest);
    assert_eq!(
        adapter.authorize_local_merge_claim(&drifted, 0, signer, drifted_digest),
        Err(MergeSidecarError::LocalSigningEquivocation)
    );
    assert_eq!(
        adapter
            .merge_claims
            .get(&(candidate.epoch_id, candidate.view, signer)),
        Some(&first_digest),
        "a conflicting candidate must never overwrite the in-memory decision"
    );
    assert_eq!(
        adapter
            .merge_signing_guard
            .authorized_digest(&merge_signing_context_for_test(&adapter, &candidate))
            .expect("read durable exact-context decision"),
        Some(first_digest),
        "a conflicting candidate must never overwrite the durable decision"
    );
}

#[test]
fn durable_local_merge_claim_rejects_conflict_after_adapter_reopen() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    let candidate = record_production_merge_candidate_for_persistence_retry(&adapter, &keys, 0);
    let signer = adapter
        .local_validator_index()
        .expect("fixture local validator is in the frozen roster");
    let first_digest = crate::merge::merge_qc_message_digest(
        &adapter.context.chain_id,
        &candidate,
        VALIDATOR_SET_HASH_VERSION_V1,
        adapter.frozen_validator_set_hash(),
    );
    adapter
        .retain_merge_sidecars_for_global_view(0, None, None)
        .expect("authorize pre-restart merge decision from the unlocked directive");

    let context = adapter.context.clone();
    let local_peer = adapter.local_peer.clone();
    let key_pair = adapter.key_pair.clone();
    let state = Arc::clone(&adapter.state);
    let kura = Arc::clone(&adapter.kura);
    let limits = adapter.limits;
    drop(adapter);

    let mut reopened = V2LaneWorkAdapter::new(
        context, local_peer, key_pair, true, state, kura, limits, None,
    )
    .expect("reopen adapter against the same committed frontier");
    assert!(
        reopened
            .drain_effects(usize::MAX)
            .iter()
            .all(|effect| !matches!(effect, V2LaneWorkEffect::BroadcastMerge(_))),
        "constructor must not emit a merge share before reducer recovery"
    );
    reopened
        .retain_merge_sidecars_for_global_view(0, None, None)
        .expect("install reopened exact unlocked directive");
    assert!(
        reopened
            .drain_effects(usize::MAX)
            .iter()
            .any(|effect| matches!(effect, V2LaneWorkEffect::BroadcastMerge(signature) if signature.message_digest == first_digest)),
        "the exact unlocked directive may release the recovered candidate share"
    );
    reopened.merge_claims.clear();
    reopened.merge_entries.clear();
    reopened.purge_queued_merge_broadcasts();
    let mut drifted = candidate.clone();
    drifted.global_state_root = Hash::new(b"post-restart conflicting merge payload");
    let drifted_digest = crate::merge::merge_qc_message_digest(
        &reopened.context.chain_id,
        &drifted,
        VALIDATOR_SET_HASH_VERSION_V1,
        reopened.frozen_validator_set_hash(),
    );
    assert_eq!(
        reopened.authorize_local_merge_claim(&drifted, 0, signer, drifted_digest),
        Err(MergeSidecarError::LocalSigningEquivocation)
    );
    assert!(
        reopened
            .merge_claims
            .get(&(candidate.epoch_id, candidate.view, signer))
            .is_none(),
        "restart rejection must not manufacture a conflicting in-memory claim"
    );
    assert_eq!(
        reopened
            .merge_signing_guard
            .authorized_digest(&merge_signing_context_for_test(&reopened, &candidate))
            .expect("read restarted durable decision"),
        Some(first_digest)
    );
}

#[test]
fn locked_later_view_directive_purges_queued_merge_shares_and_disables_retry() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    let candidate = record_production_merge_candidate_for_persistence_retry(&adapter, &keys, 0);
    adapter
        .retain_merge_sidecars_for_global_view(0, None, None)
        .expect("install initial unlocked directive");
    assert!(adapter.effects.iter().any(
        |effect| matches!(effect, V2LaneWorkEffect::BroadcastMerge(signature) if signature.view == 0)
    ));

    let locked = wire::BlockSubject {
        parent_block_hash: Some(candidate.carrier_parent_hash),
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"locked later-view carrier")),
        payload_hash: Hash::new(b"locked later-view payload"),
    };
    adapter
        .retain_merge_sidecars_for_global_view(1, Some(locked), None)
        .expect("install locked later-view directive");
    assert!(adapter.merge_entries.is_empty());
    assert!(adapter.merge_claims.is_empty());
    assert!(
        adapter
            .effects
            .iter()
            .all(|effect| !matches!(effect, V2LaneWorkEffect::BroadcastMerge(_)))
    );
    adapter
        .schedule_retransmission()
        .expect("schedule locked-view retransmission");
    assert!(
        adapter
            .drain_effects(usize::MAX)
            .iter()
            .all(|effect| !matches!(effect, V2LaneWorkEffect::BroadcastMerge(_)))
    );
}

fn record_production_merge_candidate_for_persistence_retry(
    adapter: &V2LaneWorkAdapter,
    keys: &[KeyPair],
    view: wire::View,
) -> crate::merge::MergeLedgerCandidate {
    let lane_id = LaneId::SINGLE;
    let dataspace_id = DataSpaceId::UNIVERSAL;
    let lane_height = 1;
    let header = BlockHeader::new(
        NonZeroU64::new(lane_height).expect("non-zero lane height"),
        None,
        None,
        None,
        1_700_000_000_000,
        0,
    );

    // Relay admission requires committee members to be present in both
    // the exact frozen commit topology and World. The v2 adapter fixture
    // seeds the key registry directly and commits synthetic parent blocks,
    // so complete that production authority tuple before constructing
    // authenticated relay evidence.
    {
        let mut topology = adapter.state.commit_topology.block();
        topology.clear();
        for entry in &adapter.context.roster {
            topology.push(entry.validator.clone());
        }
        topology.commit();
    }
    let mut world_block = adapter.state.world.block();
    {
        let mut peers = world_block.peers_mut_for_testing().transaction();
        for key in keys {
            let peer = PeerId::new(key.public_key().clone());
            if !peers.iter().any(|existing| existing == &peer) {
                peers.push(peer);
            }
        }
        peers.apply();
    }
    world_block.commit();

    let validators = keys
        .iter()
        .map(|key| AccountId::new(key.public_key().clone()))
        .collect::<Vec<_>>();
    let validator_bindings = validators
        .iter()
        .zip(keys)
        .map(|(validator, key)| ManifestValidatorBinding {
            validator: validator.clone(),
            peer_id: PeerId::new(key.public_key().clone()),
            torii_url: None,
        })
        .collect::<Vec<_>>();
    let status = LaneManifestStatus {
        lane: lane_id,
        alias: "default".to_owned(),
        dataspace: dataspace_id,
        visibility: LaneVisibility::Public,
        storage: LaneStorageProfile::FullReplica,
        governance: Some("parliament".to_owned()),
        manifest_path: Some(std::path::PathBuf::from(
            "/tmp/v2-merge-persistence-retry-manifest.json",
        )),
        governance_rules: Some(GovernanceRules {
            validators,
            validator_bindings,
            ..GovernanceRules::default()
        }),
        privacy_commitments: Vec::new(),
    };
    adapter
        .state
        .install_lane_manifests(&Arc::new(LaneManifestRegistry::from_statuses(
            BTreeMap::from([(lane_id, status)]),
        )));

    // Mirror the production relay-committee ranking. The fixture has the
    // exact 3f+1 topology, so every live validator is selected while this
    // ordering remains consensus-significant in the embedded QC.
    let epoch_seed = crate::sumeragi::npos_seed_for_height_from_world(
        &adapter.state.world.view(),
        &adapter.context.chain_id,
        lane_height,
    );
    let mut seed_preimage = Vec::new();
    seed_preimage.extend_from_slice(b"iroha:lane-relay:committee-seed:v1");
    seed_preimage.extend_from_slice(&epoch_seed);
    seed_preimage.extend_from_slice(&dataspace_id.as_u64().to_le_bytes());
    seed_preimage.extend_from_slice(&lane_id.as_u32().to_le_bytes());
    let committee_seed: [u8; 32] = Hash::new(seed_preimage).into();
    let mut ranked = adapter
        .state
        .commit_topology_snapshot()
        .into_iter()
        .map(|peer| {
            let mut member_preimage = Vec::new();
            member_preimage.extend_from_slice(b"iroha:lane-relay:committee-member:v1");
            member_preimage.extend_from_slice(&committee_seed);
            member_preimage.extend(
                norito::encode_canonical(&peer)
                    .expect("canonically encode relay committee member for ranking"),
            );
            (Hash::new(member_preimage), peer)
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    let committee = ranked.into_iter().map(|(_, peer)| peer).collect::<Vec<_>>();
    assert_eq!(
        committee.len(),
        keys.len(),
        "fixture must provide exact 3f+1 relay committee"
    );

    let mode_tag = LaneRelayEnvelope::lane_qc_mode_tag_for(
        lane_id,
        dataspace_id,
        crate::sumeragi::consensus::PERMISSIONED_TAG,
    );
    let parent_state_root = Hash::new(b"v2 merge retry parent state");
    let post_state_root = Hash::new(b"v2 merge retry post state");
    let mut qc = crate::sumeragi::consensus::Qc {
        phase: crate::sumeragi::consensus::Phase::Commit,
        subject_block_hash: header.hash(),
        parent_state_root,
        post_state_root,
        height: lane_height,
        view: 0,
        epoch: 0,
        chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
        rechain_seq: 0,
        mode_tag: mode_tag.clone(),
        highest_qc: None,
        validator_set_hash: HashOf::new(&committee),
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set: committee,
        aggregate: crate::sumeragi::consensus::QcAggregate {
            signers_bitmap: Vec::new(),
            bls_aggregate_signature: Vec::new(),
        },
    };
    let vote = crate::sumeragi::consensus::Vote {
        phase: qc.phase,
        block_hash: qc.subject_block_hash,
        parent_state_root: qc.parent_state_root,
        post_state_root: qc.post_state_root,
        height: qc.height,
        view: qc.view,
        epoch: qc.epoch,
        chain_order_hash: qc.chain_order_hash,
        rechain_seq: qc.rechain_seq,
        highest_qc: None,
        signer: 0,
        bls_sig: Vec::new(),
    };
    let preimage =
        crate::sumeragi::consensus::vote_preimage(&adapter.context.chain_id, &mode_tag, &vote);
    let signatures = keys
        .iter()
        .map(|key| {
            Signature::try_new(key.private_key(), &preimage)
                .expect("sign production-valid relay QC")
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let signature_refs = signatures.iter().map(Vec::as_slice).collect::<Vec<_>>();
    qc.aggregate = crate::sumeragi::consensus::QcAggregate {
        signers_bitmap: vec![(1_u8 << keys.len()) - 1],
        bls_aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
            .expect("aggregate production-valid relay QC"),
    };

    let settlement = LaneBlockCommitment {
        block_height: lane_height,
        lane_id,
        lane_incarnation: adapter
            .state
            .lane_incarnation_at_height(lane_id, lane_height)
            .expect("fixture lane incarnation is active"),
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
    };
    let manifest_root = [0x44; 32];
    let envelope = LaneRelayEnvelope::new(header, Some(qc), None, settlement, 0)
        .expect("construct production-valid relay envelope")
        .with_lane_block_descriptor_hash(Some(Hash::new(
            b"v2 merge persistence retry lane descriptor",
        )))
        .with_manifest_root(Some(manifest_root))
        .with_fastpq_proof_material(Some(LaneFastpqProofMaterial {
            proof_digest: Hash::new(b"v2 merge persistence retry FastPQ proof"),
            verified_at_height: lane_height,
        }));
    let material = envelope
        .fastpq_proof
        .expect("merge-ready relay carries FastPQ material");
    let claim_digest =
        lane_relay_fastpq_claim_digest(&envelope).expect("derive exact relay claim digest");
    let binding = AxtFastpqBinding {
        parameter: fastpq_prover::AXT_DEFAULT_PARAMETER.to_owned(),
        source_dsid: dataspace_id.as_u64(),
        source_dataspace: "v2-merge-persistence-retry".to_owned(),
        source_receipt_id: "v2-merge-persistence-retry-relay".to_owned(),
        source_tx_commitment: hex::encode(Hash::new(b"v2 merge persistence retry source").as_ref()),
        claim_type: "authorization".to_owned(),
        claim_digest: hex::encode(claim_digest.as_ref()),
        witness_commitment: hex::encode(Hash::new(b"v2 merge persistence retry witness").as_ref()),
        policy_commitment: hex::encode(Hash::new(b"v2 merge persistence retry policy").as_ref()),
        verified_effect_type: LANE_RELAY_FASTPQ_EFFECT_TYPE.to_owned(),
        corridor: "v2-merge-persistence-retry".to_owned(),
        verifier_id: "fastpq".to_owned(),
        verifier_version: "v1".to_owned(),
        target_dsids: vec![dataspace_id.as_u64()],
        effect_binding: None,
    };
    let lane_finality_statement_hash = envelope
        .lane_finality_statement_hash()
        .expect("merge-ready relay carries a finality statement");
    let (fastpq_old_root, fastpq_new_root) = envelope
        .qc
        .as_ref()
        .map_or(([0; 32], [0; 32]), |qc| {
            (qc.parent_state_root.into(), qc.post_state_root.into())
        });
    let record = VerifiedLaneRelayRecord::new(
        envelope.clone(),
        material.proof_digest,
        Hash::new(b"v2 merge persistence retry statement").into(),
        lane_finality_statement_hash,
        fastpq_old_root,
        fastpq_new_root,
        lane_finality_statement_hash.into(),
        Hash::new(b"v2 merge persistence retry inner proof"),
        material.verified_at_height,
        manifest_root,
        binding,
    );
    let relay_state_key = envelope
        .relay_ref()
        .relay_state_key()
        .parse()
        .expect("derive canonical verified relay state key");
    let record_json = iroha_primitives::json::Json::try_new(record)
        .expect("encode verified relay record as JSON");
    let record_bytes =
        norito::to_bytes(&record_json).expect("encode verified relay contract state");
    let mut world_block = adapter.state.world.block();
    world_block
        .smart_contract_state
        .insert(relay_state_key, record_bytes);
    world_block.commit();
    adapter
        .state
        .record_lane_relay(&envelope)
        .expect("production relay admission accepts retry fixture");
    let candidates = adapter
        .state
        .merge_entry_candidates_from_lane_relays_for_view(view);
    assert_eq!(
        candidates.len(),
        1,
        "one admitted relay yields one candidate"
    );
    candidates
        .into_iter()
        .next()
        .expect("relay merge candidate")
}

#[test]
fn merge_signing_rejects_wrong_round_context_and_post_apply_state() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    let candidate = record_production_merge_candidate_for_persistence_retry(&adapter, &keys, 0);
    let signer = adapter
        .local_validator_index()
        .expect("fixture local validator is in the frozen roster");
    adapter
        .retain_merge_sidecars_for_global_view(0, None, None)
        .expect("install exact unlocked directive");
    adapter.drain_effects(usize::MAX);

    let mut wrong_view = candidate.clone();
    wrong_view.view = wrong_view.view.saturating_add(1);
    let mut wrong_height = candidate.clone();
    wrong_height.carrier_height = wrong_height.carrier_height.saturating_add(1);
    let mut wrong_parent = candidate.clone();
    wrong_parent.carrier_parent_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"wrong merge signing parent"));
    for (label, drifted) in [
        ("view", wrong_view),
        ("height", wrong_height),
        ("parent", wrong_parent),
    ] {
        let digest = crate::merge::merge_qc_message_digest(
            &adapter.context.chain_id,
            &drifted,
            VALIDATOR_SET_HASH_VERSION_V1,
            adapter.frozen_validator_set_hash(),
        );
        assert_eq!(
            adapter.authorize_local_merge_claim(&drifted, 0, signer, digest),
            Err(MergeSidecarError::LocalSigningEquivocation),
            "wrong {label} must fail before private-key use"
        );
    }
    assert!(
        adapter
            .effects
            .iter()
            .all(|effect| !matches!(effect, V2LaneWorkEffect::BroadcastMerge(_)))
    );

    let applied = test_block(
        adapter.context.height,
        Some(candidate.carrier_parent_hash),
        None,
        &keys[0],
    );
    adapter
        .kura
        .store_block(applied.clone())
        .expect("persist exact post-apply carrier");
    let committed = ValidBlock::committed_from_replay_signed_block(applied);
    commit_test_block_to_state(adapter.state.as_ref(), &committed, &adapter.context);
    let digest = crate::merge::merge_qc_message_digest(
        &adapter.context.chain_id,
        &candidate,
        VALIDATOR_SET_HASH_VERSION_V1,
        adapter.frozen_validator_set_hash(),
    );
    assert_eq!(
        adapter.authorize_local_merge_claim(&candidate, 0, signer, digest),
        Err(MergeSidecarError::LocalSigningEquivocation),
        "post-apply recovery must never authorize another share"
    );
    adapter
        .refresh_merge_candidates(0)
        .expect("post-apply refresh remains signing-silent");
    adapter
        .schedule_retransmission()
        .expect("schedule post-apply retransmission");
    assert!(
        adapter
            .drain_effects(usize::MAX)
            .iter()
            .all(|effect| !matches!(effect, V2LaneWorkEffect::BroadcastMerge(_)))
    );
}

#[test]
fn merge_signing_rejects_block_first_kura_ahead_crash_image() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    let candidate = record_production_merge_candidate_for_persistence_retry(&adapter, &keys, 0);
    let signer = adapter
        .local_validator_index()
        .expect("fixture local validator is in the frozen roster");
    let digest = crate::merge::merge_qc_message_digest(
        &adapter.context.chain_id,
        &candidate,
        VALIDATOR_SET_HASH_VERSION_V1,
        adapter.frozen_validator_set_hash(),
    );
    let durable_carrier = test_block(
        adapter.context.height,
        Some(candidate.carrier_parent_hash),
        None,
        &keys[0],
    );
    adapter
        .kura
        .store_block(durable_carrier)
        .expect("persist block-first carrier without advancing State");

    adapter
        .retain_merge_sidecars_for_global_view(candidate.view, None, None)
        .expect("install exact unlocked reducer directive");
    assert!(
        adapter
            .drain_effects(usize::MAX)
            .iter()
            .all(|effect| !matches!(effect, V2LaneWorkEffect::BroadcastMerge(_))),
        "a Kura-ahead crash image must not release a private-key operation"
    );
    assert!(matches!(
        adapter.authorize_local_merge_claim(&candidate, candidate.view, signer, digest),
        Err(MergeSidecarError::SigningGuard(message))
            if message.contains("identical committed State and durable Kura frontiers")
    ));
    assert_eq!(
        adapter
            .merge_signing_guard
            .authorized_digest(&merge_signing_context_for_test(&adapter, &candidate))
            .expect("read durable signing guard"),
        None
    );
}

#[test]
fn same_round_merge_claims_survive_successful_kura_staging() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    let local_index = adapter
        .local_validator_index()
        .expect("fixture local validator is in the frozen roster");
    let local_leader_view = (0..u64::try_from(adapter.context.roster.len())
        .expect("fixture roster length fits u64"))
        .find(|view| adapter.context.leader(*view) == local_index)
        .expect("rotating leader schedule reaches the local validator");
    let candidate =
        record_production_merge_candidate_for_persistence_retry(&adapter, &keys, local_leader_view);
    let digest = crate::merge::merge_qc_message_digest(
        &adapter.context.chain_id,
        &candidate,
        VALIDATOR_SET_HASH_VERSION_V1,
        adapter.frozen_validator_set_hash(),
    );
    let key = MergeKey {
        epoch_id: candidate.epoch_id,
        view: candidate.view,
        digest,
    };
    adapter
        .retain_merge_sidecars_for_global_view(candidate.view, None, None)
        .expect("install exact unlocked reducer directive");
    assert_eq!(
        adapter
            .merge_claims
            .get(&(candidate.epoch_id, candidate.view, local_index)),
        Some(&digest),
        "local claim must be recorded before its signature is produced"
    );

    let mut accepted_remote_signers = Vec::new();
    for (index, key_pair) in keys.iter().enumerate() {
        let signer = u32::try_from(index).expect("fixture signer index fits u32");
        if signer == local_index {
            continue;
        }
        let signature = Signature::try_new(key_pair.private_key(), digest.as_ref())
            .expect("sign remote merge share")
            .payload()
            .to_vec();
        assert_eq!(
            adapter
                .accept_merge_signature(
                    MergeCommitteeSignature {
                        version: MERGE_COMMITTEE_SIGNATURE_VERSION_V2,
                        epoch_id: candidate.epoch_id,
                        view: candidate.view,
                        signer,
                        message_digest: digest,
                        bls_sig: signature,
                        leader_candidate_body: (signer == adapter.context.leader(candidate.view))
                            .then(|| candidate.canonical_bytes()),
                    },
                    candidate.view,
                )
                .expect("persist remote merge signature"),
            V2LaneIngressOutcome::Inserted
        );
        accepted_remote_signers.push(signer);
        if !adapter.merge_entries.contains_key(&key) {
            break;
        }
    }
    assert!(
        !adapter.merge_entries.contains_key(&key),
        "fixture shares must form quorum and publish the certified entry"
    );
    for signer in std::iter::once(local_index).chain(accepted_remote_signers) {
        assert_eq!(
            adapter
                .merge_claims
                .get(&(candidate.epoch_id, candidate.view, signer)),
            Some(&digest),
            "Kura staging must not reopen any same-round signer decision"
        );
    }
    let (_, staged) = adapter
        .kura
        .select_pending_certified_merge_entry()
        .expect("read pending certified merge entry")
        .expect("quorum must stage one exact merge entry");
    assert_eq!(staged.merge_qc.message_digest, digest);
}

#[test]
fn quorate_merge_persistence_failure_latches_restart_required() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    let candidate = record_production_merge_candidate_for_persistence_retry(&adapter, &keys, 0);
    adapter
        .retain_merge_sidecars_for_global_view(candidate.view, None, None)
        .expect("install exact unlocked reducer directive");
    let digest = crate::merge::merge_qc_message_digest(
        &adapter.context.chain_id,
        &candidate,
        VALIDATOR_SET_HASH_VERSION_V1,
        adapter.frozen_validator_set_hash(),
    );
    let key = MergeKey {
        epoch_id: candidate.epoch_id,
        view: candidate.view,
        digest,
    };
    let signatures = keys
        .iter()
        .enumerate()
        .map(|(index, key_pair)| {
            (
                u32::try_from(index).expect("fixture signer index fits u32"),
                Signature::try_new(key_pair.private_key(), digest.as_ref())
                    .expect("sign retry candidate")
                    .payload()
                    .to_vec(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    adapter.merge_entries.insert(
        key,
        PendingMerge {
            stage: PendingMergeStage::Collecting(candidate.clone()),
            signatures,
        },
    );

    let pending_dir = adapter.kura.store_root().join("pending_merge_entries");
    if pending_dir.is_dir() {
        std::fs::remove_dir(&pending_dir)
            .expect("remove empty pending sidecar directory before obstruction");
    }
    std::fs::write(&pending_dir, b"temporarily block pending sidecar directory")
        .expect("install transient Kura obstruction");
    let output_guard = Arc::clone(&adapter.output_guard);
    let publication = {
        let operation = output_guard
            .begin_fail_stop_operation()
            .expect("fixture output admission remains open");
        match adapter.try_commit_merge(key) {
            Ok(()) => {
                operation.complete();
                Ok(())
            }
            Err(error) => Err(error),
        }
    };
    assert!(matches!(publication, Err(V2LaneWorkError::Persistence(_))));
    assert!(
        adapter.merge_entries.contains_key(&key),
        "failed Kura publication must retain the complete quorum"
    );
    let certified_entry = match &adapter.merge_entries[&key].stage {
        PendingMergeStage::Certified(entry) => entry.clone(),
        PendingMergeStage::Collecting(_) => {
            panic!("production quorum must advance to Certified before Kura publication")
        }
    };
    assert_eq!(certified_entry.merge_qc.message_digest, key.digest);
    assert_eq!(certified_entry.epoch_id, candidate.epoch_id);
    assert_eq!(certified_entry.lane_snapshots, candidate.lane_snapshots);
    assert_eq!(certified_entry.active_lanes, candidate.active_lanes);
    let certified_hash = crate::merge::merge_ledger_entry_hash(&certified_entry);
    std::fs::remove_file(&pending_dir).expect("remove transient Kura obstruction");

    assert!(
        adapter.output_guard.restart_required(),
        "failed durable publication must poison this process before it can sign again"
    );
    assert!(matches!(
        adapter.schedule_retransmission(),
        Err(V2LaneWorkError::RestartRequired)
    ));
    assert_eq!(
        adapter
            .kura
            .merge_entry_by_hash(certified_hash)
            .expect("read exact unpublished merge entry"),
        None,
        "a poisoned process must not retry durable publication"
    );
}

#[test]
fn merge_signature_state_is_bound_to_the_active_global_view() {
    let (mut adapter, _) = fixture(wire::ConsensusMode::Permissioned);
    let stale_digest = Hash::new(b"stale merge claim");
    adapter.merge_claims.insert((7, 0, 0), stale_digest);
    adapter
        .retain_merge_sidecars_for_global_view(1, None, None)
        .expect("install next unlocked reducer view");
    assert!(
        adapter.merge_claims.is_empty(),
        "advancing the reducer view must retire old-view signing claims"
    );

    let stale = MergeCommitteeSignature {
        version: MERGE_COMMITTEE_SIGNATURE_VERSION_V2,
        epoch_id: 7,
        view: 0,
        signer: 0,
        message_digest: stale_digest,
        bls_sig: vec![0xA5; 96],
        leader_candidate_body: None,
    };
    assert_eq!(
        adapter
            .accept_merge_signature(stale, 1)
            .expect("reject stale remote signature without local durability work"),
        V2LaneIngressOutcome::Rejected
    );
    assert!(adapter.merge_claims.is_empty());
    assert!(adapter.merge_entries.is_empty());
}
