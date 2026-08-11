#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, num::NonZeroU32};

    use iroha_config::parameters::actual::{
        LaneConfig as ActualLaneConfig, LaneRoutingMatcher, LaneRoutingPolicy, LaneRoutingRule,
    };
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        block::consensus::SumeragiLanePayloadOwnership,
        nexus::{
            AUTOSCALE_META_CREATED_HEIGHT, AUTOSCALE_META_MANAGED, DataSpaceCatalog, DataSpaceId,
            LaneCatalog, LaneConfig, LaneId,
        },
    };

    use super::*;

    fn routing_for_lanes(lanes: &[u32]) -> Vec<RoutingDecision> {
        lanes
            .iter()
            .enumerate()
            .map(|(idx, lane)| {
                RoutingDecision::new(
                    LaneId::new(*lane),
                    DataSpaceId::new(u64::try_from(idx + 1).expect("dataspace id fits")),
                )
            })
            .collect()
    }

    fn routing_for_lane_dataspaces(routes: &[(u32, u64)]) -> Vec<RoutingDecision> {
        routes
            .iter()
            .map(|(lane, dataspace)| {
                RoutingDecision::new(LaneId::new(*lane), DataSpaceId::new(*dataspace))
            })
            .collect()
    }

    fn lane_catalog_from_configs(lanes: Vec<LaneConfig>) -> LaneCatalog {
        let max_lane = lanes.iter().map(|lane| lane.id.as_u32()).max().unwrap_or(0);
        let lane_count = NonZeroU32::new(max_lane.saturating_add(1))
            .expect("lane catalog requires nonzero lane count");
        LaneCatalog::new(lane_count, lanes).expect("valid lane catalog")
    }

    fn nexus_with_routing(routing_policy: LaneRoutingPolicy, lane_catalog: LaneCatalog) -> Nexus {
        let lane_config = ActualLaneConfig::from_catalog(&lane_catalog);
        Nexus {
            enabled: true,
            routing_policy,
            lane_catalog,
            lane_config,
            dataspace_catalog: DataSpaceCatalog::default(),
            ..Nexus::default()
        }
    }

    fn default_routing_policy() -> LaneRoutingPolicy {
        LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: Vec::new(),
        }
    }

    fn default_lane_config() -> LaneConfig {
        LaneConfig::default()
    }

    fn sidecar_lane_config(lane_id: LaneId) -> LaneConfig {
        LaneConfig {
            id: lane_id,
            dataspace_id: DataSpaceId::UNIVERSAL,
            alias: format!("sidecar-{}", lane_id.as_u32()),
            ..LaneConfig::default()
        }
    }

    fn autoscale_elastic_lane_config(lane_id: LaneId, created_height: u64) -> LaneConfig {
        let mut metadata = BTreeMap::new();
        metadata.insert(AUTOSCALE_META_MANAGED.to_string(), "true".to_string());
        metadata.insert(
            AUTOSCALE_META_CREATED_HEIGHT.to_string(),
            created_height.to_string(),
        );
        let mut lane = LaneConfig {
            id: lane_id,
            dataspace_id: DataSpaceId::UNIVERSAL,
            alias: format!("elastic-lane-{}", lane_id.as_u32()),
            metadata,
            ..LaneConfig::default()
        };
        crate::state::attach_synthetic_autoscale_committee_for_test(&mut lane);
        lane
    }

    fn proposal_candidate(gas_cost: u64, is_ivm_heavy: bool) -> ProposalAdmissionCandidate {
        ProposalAdmissionCandidate {
            gas_cost,
            is_ivm_heavy,
        }
    }

    fn proposal_context() -> ProposalAdmissionContext {
        ProposalAdmissionContext {
            accepted_before_batch: 0,
            accepted_in_batch: 0,
            max_in_block: 4,
            gas_limit_per_block: Some(10),
            gas_used_in_block: 0,
            max_ivm_transactions: Some(1),
            ivm_transactions_included: 0,
        }
    }

    fn accepted_schedule(indices: &[usize]) -> ProposalBatchSchedule {
        ProposalBatchSchedule {
            actions: indices
                .iter()
                .copied()
                .map(|index| ProposalBatchAction::Accept {
                    index,
                    exceeds_gas_limit: false,
                })
                .collect(),
            ..ProposalBatchSchedule::default()
        }
    }

    fn mixed_schedule(actions: Vec<ProposalBatchAction>) -> ProposalBatchSchedule {
        ProposalBatchSchedule {
            actions,
            ..ProposalBatchSchedule::default()
        }
    }

    fn test_peer(seed: u8) -> PeerId {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("deterministic peer key");
        PeerId::new(key_pair.public_key().clone())
    }

    fn autonomous_reservation_height_context(validators: &[PeerId]) -> wire::HeightContext {
        let mut roster = validators
            .iter()
            .cloned()
            .map(|validator| wire::ValidatorPower {
                validator,
                power: 1,
            })
            .collect::<Vec<_>>();
        roster.sort_by(|left, right| left.validator.cmp(&right.validator));
        wire::HeightContext {
            network_id: crate::sumeragi::synthetic_network_id("autonomous-reservation-slot-test"),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 7,
            epoch_end_height: u64::MAX,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("valid frozen quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"autonomous reservation nexus context"),
            execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1024,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 512 * 1024,
                max_chunk_count: 1024,
            },
            leader_seed: [0xA7; 32],
        }
    }

    fn tx_hash(seed: u8) -> Hash {
        Hash::prehashed([seed; Hash::LENGTH])
    }

    fn tx_hashes(count: usize) -> Vec<Hash> {
        (0..count)
            .map(|index| tx_hash(u8::try_from(index + 1).expect("test hash seed fits u8")))
            .collect()
    }

    fn lane_tip(lane: u32, dataspace: u64, latest_lane_block_height: u64) -> LaneBlockTip {
        LaneBlockTip {
            lane_id: LaneId::new(lane),
            dataspace_id: DataSpaceId::new(dataspace),
            lane_incarnation: Hash::new(
                format!("lane-tip-incarnation:{lane}:{dataspace}").as_bytes(),
            ),
            latest_lane_block_height,
            latest_lane_block_descriptor_hash: None,
        }
    }

    fn lane_subject_test_incarnation(lane: u32, dataspace: u64) -> Hash {
        Hash::new(
            [
                b"lane-subject-test-incarnation:".as_slice(),
                &lane.to_be_bytes(),
                &dataspace.to_be_bytes(),
            ]
            .concat(),
        )
    }

    fn lane_tip_with_descriptor(
        lane: u32,
        dataspace: u64,
        latest_lane_block_height: u64,
        descriptor_seed: u8,
    ) -> LaneBlockTip {
        LaneBlockTip {
            latest_lane_block_descriptor_hash: Some(Hash::prehashed(
                [descriptor_seed; Hash::LENGTH],
            )),
            ..lane_tip(lane, dataspace, latest_lane_block_height)
        }
    }

    fn committee(
        lane: u32,
        dataspace: u64,
        validators: Vec<PeerId>,
        min_quorum: Option<u32>,
    ) -> LaneConsensusCommittee {
        LaneConsensusCommittee {
            lane_id: LaneId::new(lane),
            dataspace_id: DataSpaceId::new(dataspace),
            validators,
            min_quorum,
        }
    }

    fn lane_block_proposal_with_committee(
        validators: Vec<PeerId>,
        min_quorum: Option<u32>,
    ) -> LaneBlockProposal {
        let routing = routing_for_lane_dataspaces(&[(1, 11)]);
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0]),
            &[committee(1, 11, validators, min_quorum)],
            "permissioned",
        )
        .expect("lane consensus domain");
        let plan = plan_lane_payload(
            &domains,
            &[lane_tip_with_descriptor(1, 11, 3, 0xA7)],
            &[tx_hash(0xC7)],
            4,
            2,
        )
        .expect("lane payload plan");
        plan.entries[0].lane_block_proposal.clone()
    }

    fn refresh_lane_block_proposal_hashes(proposal: &mut LaneBlockProposal) {
        proposal.block_descriptor.descriptor_hash =
            lane_block_descriptor_artifact(&proposal.block_descriptor).computed_descriptor_hash();
        proposal.artifact.descriptor = lane_block_descriptor_artifact(&proposal.block_descriptor);
        let proposal_hash = proposal.artifact.computed_proposal_hash();
        proposal.artifact.proposal_hash = proposal_hash;
        proposal.proposal_hash = proposal_hash;
    }

    fn lane_redrive_artifact() -> LaneBlockProposalV1 {
        let mut artifact = lane_block_proposal_with_committee(
            vec![test_peer(4), test_peer(1), test_peer(3), test_peer(2)],
            None,
        )
        .artifact;
        artifact.descriptor.lane_block_view = 0;
        artifact.descriptor.descriptor_hash = artifact.descriptor.computed_descriptor_hash();
        artifact.proposal_hash = artifact.computed_proposal_hash();
        artifact
    }

    fn retarget_lane_redrive_artifact(
        mut proposal: LaneBlockProposalV1,
        lane: u32,
        dataspace: u64,
        lane_block_height: u64,
        lane_block_view: u64,
    ) -> LaneBlockProposalV1 {
        proposal.descriptor.lane_id = LaneId::new(lane);
        proposal.descriptor.dataspace_id = DataSpaceId::new(dataspace);
        proposal.descriptor.previous_lane_block_height = lane_block_height.saturating_sub(1);
        proposal.descriptor.previous_lane_block_descriptor_hash =
            (lane_block_height > 1).then_some(Hash::prehashed([0xA7; Hash::LENGTH]));
        proposal.descriptor.lane_block_height = lane_block_height;
        proposal.descriptor.lane_block_view = lane_block_view;
        proposal.descriptor.descriptor_hash = proposal.descriptor.computed_descriptor_hash();
        proposal.proposal_hash = proposal.computed_proposal_hash();
        proposal
    }

    #[test]
    fn autonomous_reservation_slot_is_canonical_and_transaction_independent() {
        let lane_id = LaneId::new(3);
        let dataspace_id = DataSpaceId::new(33);
        let lane_incarnation = Hash::new(b"autonomous reservation active incarnation");
        let predecessor_hash = Hash::new(b"autonomous reservation predecessor");
        let validators = vec![test_peer(4), test_peer(1), test_peer(3), test_peer(2)];
        let context = autonomous_reservation_height_context(&validators);
        context.validate().expect("valid frozen height context");

        let plan = assemble_autonomous_lane_reservation_slot(
            &context,
            lane_id,
            dataspace_id,
            lane_incarnation,
            3,
            Some(predecessor_hash),
            validators.clone(),
        )
        .expect("canonical autonomous reservation slot");
        let repeated = assemble_autonomous_lane_reservation_slot(
            &context,
            lane_id,
            dataspace_id,
            lane_incarnation,
            3,
            Some(predecessor_hash),
            validators,
        )
        .expect("same transaction-independent slot");

        let mut expected_validators = vec![test_peer(4), test_peer(1), test_peer(3), test_peer(2)];
        expected_validators.sort();
        assert_eq!(plan, repeated);
        let selection_authorization = plan
            .selection_authorization()
            .expect("canonical slot yields queue selection authority");
        assert_eq!(selection_authorization.scope(), plan.reservation_scope());
        assert_eq!(selection_authorization.validator_count(), 4);
        assert_eq!(selection_authorization.producer(), 1_u128 << 3);
        assert_eq!(plan.previous_lane_block_height, 3);
        assert_eq!(
            plan.previous_lane_block_descriptor_hash,
            Some(predecessor_hash)
        );
        assert_eq!(plan.lane_block_height, 4);
        assert_eq!(plan.lane_block_view, 0);
        assert_eq!(plan.validator_set, expected_validators);
        assert_eq!(plan.validator_set_hash, HashOf::new(&plan.validator_set));
        assert_eq!(plan.author, plan.validator_set[3]);
        assert_eq!(
            plan.qc_mode_tag,
            LaneRelayEnvelope::lane_qc_mode_tag_for(
                lane_id,
                dataspace_id,
                &v2_lane_context_mode_tag(&context),
            )
        );
        assert_eq!(
            plan.reservation_scope(),
            LaneQueueReservationScopeV1 {
                lane_id,
                dataspace_id,
                lane_incarnation,
                proposal_height: context.height,
                lane_block_height: 4,
                lane_block_view: 0,
                reservation_owner_hash: plan.reservation_owner_hash,
                proposal_identity_hash: plan.proposal_identity_hash,
            }
        );

        let mut other_context = context.clone();
        other_context.leader_seed[0] ^= 1;
        let other_context_plan = assemble_autonomous_lane_reservation_slot(
            &other_context,
            lane_id,
            dataspace_id,
            lane_incarnation,
            3,
            Some(predecessor_hash),
            plan.validator_set.clone(),
        )
        .expect("context-separated autonomous reservation slot");
        assert_ne!(
            plan.proposal_identity_hash,
            other_context_plan.proposal_identity_hash
        );
        assert_ne!(
            plan.reservation_owner_hash,
            other_context_plan.reservation_owner_hash
        );
    }

    #[test]
    fn autonomous_reservation_eligibility_rejects_stale_blocked_and_inactive() {
        let lane_id = LaneId::new(3);
        let dataspace_id = DataSpaceId::new(33);
        assert_eq!(
            validate_autonomous_lane_reservation_eligibility(
                8,
                7,
                true,
                false,
                lane_id,
                dataspace_id,
            ),
            Ok(())
        );
        assert_eq!(
            validate_autonomous_lane_reservation_eligibility(
                8,
                8,
                true,
                false,
                lane_id,
                dataspace_id,
            ),
            Err(AutonomousLaneReservationSlotPlanError::StaleHeightContext {
                context_height: 8,
                committed_height: 8,
            })
        );
        assert_eq!(
            validate_autonomous_lane_reservation_eligibility(
                8,
                7,
                false,
                false,
                lane_id,
                dataspace_id,
            ),
            Err(AutonomousLaneReservationSlotPlanError::InactiveRoute {
                lane_id,
                dataspace_id,
            })
        );
        assert_eq!(
            validate_autonomous_lane_reservation_eligibility(
                8,
                7,
                true,
                true,
                lane_id,
                dataspace_id,
            ),
            Err(AutonomousLaneReservationSlotPlanError::BlockedPredecessor {
                lane_id,
                dataspace_id,
            })
        );
    }

    #[test]
    fn autonomous_reservation_requires_exact_non_genesis_predecessor_hash() {
        let validators = vec![test_peer(1), test_peer(2), test_peer(3), test_peer(4)];
        let context = autonomous_reservation_height_context(&validators);
        assert_eq!(
            assemble_autonomous_lane_reservation_slot(
                &context,
                LaneId::new(3),
                DataSpaceId::new(33),
                Hash::new(b"autonomous reservation active incarnation"),
                1,
                None,
                validators,
            ),
            Err(
                AutonomousLaneReservationSlotPlanError::MissingPredecessorHash {
                    previous_lane_block_height: 1,
                }
            )
        );
    }

    #[test]
    fn lane_redrive_leader_is_deterministic_and_rotates_by_view_and_timeout() {
        let proposal = lane_redrive_artifact();
        let validators = &proposal.descriptor.validator_set;
        let leader_view_0 = lane_block_redrive_leader(&proposal, 0)
            .expect("canonical proposal has a transport leader");
        assert!(validators.contains(leader_view_0));
        assert_eq!(
            lane_block_redrive_leader(&proposal, 0),
            Some(leader_view_0),
            "leader selection must be deterministic"
        );

        let leader_after_timeout =
            lane_block_redrive_leader(&proposal, 1).expect("backup transport leader exists");
        assert_ne!(leader_view_0, leader_after_timeout);

        let next_view = retarget_lane_redrive_artifact(proposal.clone(), 1, 11, 4, 1);
        assert_eq!(
            lane_block_redrive_leader(&next_view, 0),
            Some(leader_after_timeout),
            "a lane view change and one timeout use the same canonical one-step rotation"
        );

        let mut forged = proposal;
        forged.proposal_hash = Hash::prehashed([0xFF; Hash::LENGTH]);
        assert_eq!(
            lane_block_redrive_leader(&forged, 0),
            None,
            "a forged proposal cannot acquire a scheduler leader"
        );
    }

    #[test]
    fn lane_redrive_tracker_rejects_conflicts_and_stale_views() {
        let now = Instant::now();
        let view_0 = lane_redrive_artifact();
        let mut tracker = LaneBlockRedriveTracker::new(8);
        assert_eq!(
            tracker.observe(&view_0, now),
            LaneBlockRedriveObservation::Inserted
        );
        assert_eq!(
            tracker.observe(&view_0, now + Duration::from_secs(1)),
            LaneBlockRedriveObservation::Duplicate,
            "duplicate relay must not reset its timeout clock"
        );

        let mut conflicting = view_0.clone();
        conflicting.descriptor.subject_hash = Hash::prehashed([0xD1; Hash::LENGTH]);
        conflicting.descriptor.descriptor_hash = conflicting.descriptor.computed_descriptor_hash();
        conflicting.proposal_hash = conflicting.computed_proposal_hash();
        assert_eq!(
            tracker.observe(&conflicting, now),
            LaneBlockRedriveObservation::Conflicting
        );

        let view_2 = retarget_lane_redrive_artifact(view_0.clone(), 1, 11, 4, 2);
        assert_eq!(
            tracker.observe(&view_2, now + Duration::from_secs(2)),
            LaneBlockRedriveObservation::Superseded { previous_view: 0 }
        );
        let view_1 = retarget_lane_redrive_artifact(view_0, 1, 11, 4, 1);
        assert_eq!(
            tracker.observe(&view_1, now + Duration::from_secs(3)),
            LaneBlockRedriveObservation::Stale { current_view: 2 }
        );
        assert_eq!(
            tracker.redrive_round(
                &view_1,
                now + Duration::from_secs(4),
                Duration::from_secs(1)
            ),
            None,
            "stale lane views must never be redriven"
        );
    }

    #[test]
    fn lane_redrive_timeout_is_independent_per_lane_and_height() {
        let now = Instant::now();
        let lane_1_height_4 = lane_redrive_artifact();
        let lane_1_height_5 = retarget_lane_redrive_artifact(lane_1_height_4.clone(), 1, 11, 5, 0);
        let lane_2 = retarget_lane_redrive_artifact(lane_1_height_4.clone(), 2, 22, 9, 0);
        let idle_lane = retarget_lane_redrive_artifact(lane_1_height_4.clone(), 3, 33, 1, 0);
        let timeout = Duration::from_secs(1);
        let mut tracker = LaneBlockRedriveTracker::new(8);

        assert_eq!(
            tracker.observe(&lane_1_height_4, now),
            LaneBlockRedriveObservation::Inserted
        );
        assert_eq!(
            tracker.observe(&lane_1_height_5, now + Duration::from_millis(250)),
            LaneBlockRedriveObservation::Inserted,
            "out-of-order successor heights retain an independent clock"
        );
        assert_eq!(
            tracker.observe(&lane_2, now + Duration::from_millis(900)),
            LaneBlockRedriveObservation::Inserted
        );

        let sample_at = now + Duration::from_millis(1_100);
        assert_eq!(
            tracker.redrive_round(&lane_1_height_4, sample_at, timeout),
            Some(1)
        );
        assert_eq!(
            tracker.redrive_round(&lane_1_height_5, sample_at, timeout),
            Some(0)
        );
        assert_eq!(tracker.redrive_round(&lane_2, sample_at, timeout), Some(0));
        assert_eq!(
            tracker.redrive_round(&idle_lane, sample_at, timeout),
            None,
            "an idle lane creates no clock and cannot stall active lane redrive"
        );

        let fallback_at = now + Duration::from_secs(4);
        for validator in &lane_1_height_4.descriptor.validator_set {
            assert!(
                tracker.peer_may_redrive(&lane_1_height_4, validator, fallback_at, timeout),
                "after one full coordinator cycle every committee member must be able to recover despite observation skew"
            );
        }
    }

    #[test]
    fn lane_redrive_tracker_is_bounded_and_evicts_oldest_identity() {
        let now = Instant::now();
        let first = lane_redrive_artifact();
        let second = retarget_lane_redrive_artifact(first.clone(), 2, 22, 1, 0);
        let mut tracker = LaneBlockRedriveTracker::new(1);
        assert_eq!(
            tracker.observe(&first, now),
            LaneBlockRedriveObservation::Inserted
        );
        assert_eq!(
            tracker.observe(&second, now + Duration::from_millis(1)),
            LaneBlockRedriveObservation::Inserted
        );
        assert_eq!(
            tracker.redrive_round(&first, now + Duration::from_secs(1), Duration::from_secs(1)),
            None
        );
        assert_eq!(
            tracker.redrive_round(
                &second,
                now + Duration::from_secs(1),
                Duration::from_secs(1)
            ),
            Some(0)
        );
    }

    #[test]
    fn lane_redrive_tracker_compacts_superseded_views_with_interleaved_lanes() {
        let now = Instant::now();
        let lane_a = lane_redrive_artifact();
        let lane_b = retarget_lane_redrive_artifact(lane_a.clone(), 2, 22, 7, 0);
        let mut tracker = LaneBlockRedriveTracker::new(2);
        assert_eq!(
            tracker.observe(&lane_a, now),
            LaneBlockRedriveObservation::Inserted
        );
        assert_eq!(
            tracker.observe(&lane_b, now),
            LaneBlockRedriveObservation::Inserted
        );

        for view in 1..=64 {
            let lane_a_view = retarget_lane_redrive_artifact(lane_a.clone(), 1, 11, 4, view);
            assert!(matches!(
                tracker.observe(&lane_a_view, now + Duration::from_millis(view)),
                LaneBlockRedriveObservation::Superseded { .. }
            ));
        }

        assert_eq!(tracker.observed_at.len(), 2);
        assert_eq!(
            tracker.order.len(),
            2,
            "superseded identities must not accumulate behind an interleaved lane"
        );
        assert!(
            tracker
                .redrive_round(&lane_b, now, Duration::from_secs(1))
                .is_some()
        );
    }

    #[test]
    fn lane_proposal_batch_tracks_only_lanes_with_work() {
        let batch = LaneProposalBatch::from_routing_decisions(&routing_for_lanes(&[3, 1, 3]));

        assert_eq!(
            batch.active_lane_ids(),
            vec![LaneId::new(1), LaneId::new(3)]
        );
        assert_eq!(batch.active_lane_count(), 2);
        assert!(batch.has_parallel_work());
    }

    #[test]
    fn lane_proposal_batch_rotates_start_lane_by_height_and_view() {
        let batch = LaneProposalBatch::from_routing_decisions(&routing_for_lanes(&[1, 2, 1, 2]));

        assert_eq!(batch.interleaved_indices_for_slot(0, 0), vec![0, 1, 2, 3]);
        assert_eq!(batch.interleaved_indices_for_slot(1, 0), vec![1, 0, 3, 2]);
        assert_eq!(batch.interleaved_indices_for_slot(1, 1), vec![0, 1, 2, 3]);
    }

    #[test]
    fn lane_proposal_batch_does_not_wait_for_idle_lane_ids() {
        let batch = LaneProposalBatch::from_routing_decisions(&routing_for_lanes(&[1, 3, 1]));

        assert_eq!(
            batch.active_lane_ids(),
            vec![LaneId::new(1), LaneId::new(3)]
        );
        assert_eq!(batch.interleaved_indices_from_offset(0), vec![0, 1, 2]);
        assert_eq!(batch.interleaved_indices_from_offset(1), vec![1, 0, 2]);
    }

    #[test]
    fn lane_proposal_batch_preserves_serial_order_for_empty_or_single_lane_work() {
        let empty = LaneProposalBatch::from_routing_decisions(&[]);
        assert_eq!(empty.active_lane_count(), 0);
        assert_eq!(
            empty.interleaved_indices_for_slot(7, 9),
            Vec::<usize>::new()
        );

        let single_lane = LaneProposalBatch::from_routing_decisions(&routing_for_lanes(&[7, 7]));
        assert_eq!(single_lane.active_lane_count(), 1);
        assert!(!single_lane.has_parallel_work());
        assert_eq!(single_lane.interleaved_indices_for_slot(7, 9), vec![0, 1]);
    }

    #[test]
    fn proposal_lookahead_ignores_unrouted_sidecar_lane() {
        let lane_catalog = lane_catalog_from_configs(vec![
            default_lane_config(),
            sidecar_lane_config(LaneId::new(1)),
        ]);
        let nexus = nexus_with_routing(default_routing_policy(), lane_catalog);

        assert!(!proposal_lookahead_enabled(&nexus, 1));
    }

    #[test]
    fn proposal_lookahead_enables_for_explicit_rule_lane() {
        let routing_policy = LaneRoutingPolicy {
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: Some(DataSpaceId::UNIVERSAL),
                matcher: LaneRoutingMatcher {
                    account: Some("alice".to_string()),
                    instruction: None,
                    description: None,
                },
            }],
            ..default_routing_policy()
        };
        let lane_catalog = lane_catalog_from_configs(vec![
            default_lane_config(),
            sidecar_lane_config(LaneId::new(1)),
        ]);
        let nexus = nexus_with_routing(routing_policy, lane_catalog);

        assert!(proposal_lookahead_enabled(&nexus, 1));
    }

    #[test]
    fn proposal_lookahead_respects_autoscale_creation_height() {
        let lane_catalog = lane_catalog_from_configs(vec![
            default_lane_config(),
            autoscale_elastic_lane_config(LaneId::new(1), 7),
        ]);
        let mut nexus = nexus_with_routing(default_routing_policy(), lane_catalog);
        nexus.autoscale.enabled = true;
        nexus.autoscale.min_lanes = NonZeroU32::new(1).expect("nonzero min");
        nexus.autoscale.max_lanes = NonZeroU32::new(4).expect("nonzero max");

        assert!(!proposal_lookahead_enabled(&nexus, 6));
        assert!(proposal_lookahead_enabled(&nexus, 7));
    }

    #[test]
    fn proposal_lookahead_fails_closed_when_nexus_is_disabled() {
        let routing_policy = LaneRoutingPolicy {
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: Some(DataSpaceId::UNIVERSAL),
                matcher: LaneRoutingMatcher {
                    account: Some("alice".to_string()),
                    instruction: None,
                    description: None,
                },
            }],
            ..default_routing_policy()
        };
        let lane_catalog = lane_catalog_from_configs(vec![
            default_lane_config(),
            sidecar_lane_config(LaneId::new(1)),
        ]);
        let mut nexus = nexus_with_routing(routing_policy, lane_catalog);
        nexus.enabled = false;

        assert!(!proposal_lookahead_enabled(&nexus, 1));
    }

    #[test]
    fn proposal_fetch_cap_widens_only_for_schedulable_multilane_routes() {
        let single_lane = nexus_with_routing(
            default_routing_policy(),
            lane_catalog_from_configs(vec![
                default_lane_config(),
                sidecar_lane_config(LaneId::new(1)),
            ]),
        );
        assert_eq!(
            proposal_fetch_cap(&single_lane, 1, 8, 2),
            2,
            "unrouted sidecars must not widen queue scans"
        );

        let explicit_lane_policy = LaneRoutingPolicy {
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: Some(DataSpaceId::UNIVERSAL),
                matcher: LaneRoutingMatcher {
                    account: Some("alice".to_string()),
                    instruction: None,
                    description: None,
                },
            }],
            ..default_routing_policy()
        };
        let explicit_lane = nexus_with_routing(
            explicit_lane_policy,
            lane_catalog_from_configs(vec![
                default_lane_config(),
                sidecar_lane_config(LaneId::new(1)),
            ]),
        );
        assert_eq!(
            proposal_fetch_cap(&explicit_lane, 1, 8, 2),
            8,
            "reachable sidecar lanes may widen scans to find lane-local work"
        );
    }

    #[test]
    fn proposal_fetch_cap_respects_budget_slot_and_autoscale_activation_bounds() {
        let lane_catalog = lane_catalog_from_configs(vec![
            default_lane_config(),
            autoscale_elastic_lane_config(LaneId::new(1), 7),
        ]);
        let mut nexus = nexus_with_routing(default_routing_policy(), lane_catalog);
        nexus.autoscale.enabled = true;
        nexus.autoscale.min_lanes = NonZeroU32::new(1).expect("nonzero min");
        nexus.autoscale.max_lanes = NonZeroU32::new(4).expect("nonzero max");

        assert_eq!(
            proposal_fetch_cap(&nexus, 6, 8, 2),
            2,
            "future-created autoscale lanes must not widen scans"
        );
        assert_eq!(
            proposal_fetch_cap(&nexus, 7, 8, 2),
            8,
            "active autoscale lanes may widen scans"
        );
        assert_eq!(proposal_fetch_cap(&nexus, 7, 0, 2), 0);
        assert_eq!(proposal_fetch_cap(&nexus, 7, 8, 0), 0);
    }

    #[test]
    fn proposal_admission_defers_when_block_slots_are_full() {
        let mut context = proposal_context();
        context.accepted_before_batch = 3;
        context.accepted_in_batch = 1;
        context.max_in_block = 4;

        assert_eq!(
            decide_proposal_candidate_admission(
                proposal_candidate(1, false),
                std::iter::empty::<ProposalAdmissionCandidate>(),
                context
            ),
            ProposalAdmissionDecision::Defer {
                reason: ProposalDeferralReason::BlockFull
            }
        );
    }

    #[test]
    fn proposal_admission_enforces_ivm_cap_without_blocking_non_ivm_work() {
        let mut context = proposal_context();
        context.ivm_transactions_included = 1;

        assert_eq!(
            decide_proposal_candidate_admission(
                proposal_candidate(1, true),
                std::iter::empty::<ProposalAdmissionCandidate>(),
                context
            ),
            ProposalAdmissionDecision::Defer {
                reason: ProposalDeferralReason::IvmLimit
            }
        );
        assert_eq!(
            decide_proposal_candidate_admission(
                proposal_candidate(1, false),
                std::iter::empty::<ProposalAdmissionCandidate>(),
                context
            ),
            ProposalAdmissionDecision::Accept {
                exceeds_gas_limit: false
            }
        );
    }

    #[test]
    fn proposal_admission_defers_gas_overflow_after_first_acceptance() {
        let mut context = proposal_context();
        context.gas_used_in_block = 7;
        context.accepted_before_batch = 1;

        assert_eq!(
            decide_proposal_candidate_admission(
                proposal_candidate(4, false),
                std::iter::empty::<ProposalAdmissionCandidate>(),
                context
            ),
            ProposalAdmissionDecision::Defer {
                reason: ProposalDeferralReason::GasLimit
            }
        );
    }

    #[test]
    fn proposal_admission_defers_oversized_first_when_later_candidate_fits() {
        let context = proposal_context();

        assert_eq!(
            decide_proposal_candidate_admission(
                proposal_candidate(11, false),
                [proposal_candidate(3, false)],
                context
            ),
            ProposalAdmissionDecision::Defer {
                reason: ProposalDeferralReason::GasLimit
            }
        );
    }

    #[test]
    fn proposal_admission_accepts_oversized_first_when_no_later_candidate_fits() {
        let context = proposal_context();

        assert_eq!(
            decide_proposal_candidate_admission(
                proposal_candidate(11, false),
                [proposal_candidate(12, false), proposal_candidate(11, true)],
                context
            ),
            ProposalAdmissionDecision::Accept {
                exceeds_gas_limit: true
            }
        );
    }

    #[test]
    fn proposal_admission_ignores_later_candidate_that_would_exceed_ivm_cap() {
        let mut context = proposal_context();
        context.ivm_transactions_included = 1;

        assert_eq!(
            decide_proposal_candidate_admission(
                proposal_candidate(11, false),
                [proposal_candidate(3, true)],
                context
            ),
            ProposalAdmissionDecision::Accept {
                exceeds_gas_limit: true
            }
        );
    }

    #[test]
    fn schedule_proposal_batch_interleaves_and_accumulates_resources() {
        let routing = routing_for_lanes(&[1, 2, 1]);
        let candidates = vec![
            proposal_candidate(2, false),
            proposal_candidate(3, true),
            proposal_candidate(5, false),
        ];
        let mut context = proposal_context();
        context.max_ivm_transactions = Some(2);

        let schedule = schedule_proposal_batch(&routing, &candidates, context, 0, 0)
            .expect("schedule proposal batch");

        assert_eq!(
            schedule.actions,
            vec![
                ProposalBatchAction::Accept {
                    index: 0,
                    exceeds_gas_limit: false
                },
                ProposalBatchAction::Accept {
                    index: 1,
                    exceeds_gas_limit: false
                },
                ProposalBatchAction::Accept {
                    index: 2,
                    exceeds_gas_limit: false
                },
            ]
        );
        assert_eq!(schedule.gas_used_delta, 10);
        assert_eq!(schedule.ivm_transactions_included_delta, 1);
        assert_eq!(schedule.ivm_transactions_deferred, 0);
    }

    #[test]
    fn schedule_proposal_batch_rotates_and_defers_after_block_full() {
        let routing = routing_for_lanes(&[1, 2, 1, 2]);
        let candidates = vec![
            proposal_candidate(1, false),
            proposal_candidate(1, false),
            proposal_candidate(1, false),
            proposal_candidate(1, false),
        ];
        let mut context = proposal_context();
        context.max_in_block = 2;

        let schedule = schedule_proposal_batch(&routing, &candidates, context, 1, 0)
            .expect("schedule proposal batch");

        assert_eq!(
            schedule.actions,
            vec![
                ProposalBatchAction::Accept {
                    index: 1,
                    exceeds_gas_limit: false
                },
                ProposalBatchAction::Accept {
                    index: 0,
                    exceeds_gas_limit: false
                },
                ProposalBatchAction::Defer {
                    index: 3,
                    reason: ProposalDeferralReason::BlockFull
                },
                ProposalBatchAction::Defer {
                    index: 2,
                    reason: ProposalDeferralReason::BlockFull
                },
            ]
        );
        assert_eq!(schedule.gas_used_delta, 2);
    }

    #[test]
    fn schedule_proposal_batch_prefers_later_fitting_candidate_over_oversized_first() {
        let routing = routing_for_lanes(&[1, 2]);
        let candidates = vec![proposal_candidate(11, false), proposal_candidate(3, false)];
        let context = proposal_context();

        let schedule = schedule_proposal_batch(&routing, &candidates, context, 0, 0)
            .expect("schedule proposal batch");

        assert_eq!(
            schedule.actions,
            vec![
                ProposalBatchAction::Defer {
                    index: 0,
                    reason: ProposalDeferralReason::GasLimit
                },
                ProposalBatchAction::Accept {
                    index: 1,
                    exceeds_gas_limit: false
                },
            ]
        );
        assert_eq!(schedule.gas_used_delta, 3);
    }

    #[test]
    fn schedule_proposal_batch_counts_ivm_deferrals() {
        let routing = routing_for_lanes(&[1, 2]);
        let candidates = vec![proposal_candidate(1, true), proposal_candidate(2, false)];
        let mut context = proposal_context();
        context.ivm_transactions_included = 1;

        let schedule = schedule_proposal_batch(&routing, &candidates, context, 0, 0)
            .expect("schedule proposal batch");

        assert_eq!(
            schedule.actions,
            vec![
                ProposalBatchAction::Defer {
                    index: 0,
                    reason: ProposalDeferralReason::IvmLimit
                },
                ProposalBatchAction::Accept {
                    index: 1,
                    exceeds_gas_limit: false
                },
            ]
        );
        assert_eq!(schedule.gas_used_delta, 2);
        assert_eq!(schedule.ivm_transactions_included_delta, 0);
        assert_eq!(schedule.ivm_transactions_deferred, 1);
    }

    #[test]
    fn defer_accepted_proposal_actions_preserves_existing_deferrals_without_resource_deltas() {
        let schedule = ProposalBatchSchedule {
            actions: vec![
                ProposalBatchAction::Accept {
                    index: 0,
                    exceeds_gas_limit: false,
                },
                ProposalBatchAction::Defer {
                    index: 1,
                    reason: ProposalDeferralReason::GasLimit,
                },
                ProposalBatchAction::Accept {
                    index: 2,
                    exceeds_gas_limit: true,
                },
            ],
            gas_used_delta: 13,
            ivm_transactions_included_delta: 1,
            ivm_transactions_deferred: 1,
        };

        let deferred =
            defer_accepted_proposal_actions(&schedule, ProposalDeferralReason::LaneConsensus);

        assert_eq!(
            deferred.actions,
            vec![
                ProposalBatchAction::Defer {
                    index: 0,
                    reason: ProposalDeferralReason::LaneConsensus,
                },
                ProposalBatchAction::Defer {
                    index: 1,
                    reason: ProposalDeferralReason::GasLimit,
                },
                ProposalBatchAction::Defer {
                    index: 2,
                    reason: ProposalDeferralReason::LaneConsensus,
                },
            ]
        );
        assert_eq!(deferred.gas_used_delta, 0);
        assert_eq!(deferred.ivm_transactions_included_delta, 0);
        assert_eq!(deferred.ivm_transactions_deferred, 1);
    }

    #[test]
    fn defer_accepted_proposal_actions_for_lanes_recomputes_remaining_resource_deltas() {
        let routing = routing_for_lanes(&[1, 2, 3]);
        let candidates = vec![
            proposal_candidate(3, true),
            proposal_candidate(5, false),
            proposal_candidate(7, true),
        ];
        let schedule = ProposalBatchSchedule {
            actions: vec![
                ProposalBatchAction::Accept {
                    index: 0,
                    exceeds_gas_limit: false,
                },
                ProposalBatchAction::Accept {
                    index: 1,
                    exceeds_gas_limit: false,
                },
                ProposalBatchAction::Defer {
                    index: 2,
                    reason: ProposalDeferralReason::GasLimit,
                },
            ],
            gas_used_delta: 8,
            ivm_transactions_included_delta: 1,
            ivm_transactions_deferred: 0,
        };
        let blocked_lanes = BTreeSet::from([LaneId::new(1)]);

        let deferred = defer_accepted_proposal_actions_for_lanes(
            &schedule,
            &routing,
            &candidates,
            &blocked_lanes,
            ProposalDeferralReason::LaneConsensus,
        );

        assert_eq!(
            deferred.actions,
            vec![
                ProposalBatchAction::Defer {
                    index: 0,
                    reason: ProposalDeferralReason::LaneConsensus,
                },
                ProposalBatchAction::Accept {
                    index: 1,
                    exceeds_gas_limit: false,
                },
                ProposalBatchAction::Defer {
                    index: 2,
                    reason: ProposalDeferralReason::GasLimit,
                },
            ]
        );
        assert_eq!(deferred.gas_used_delta, 5);
        assert_eq!(deferred.ivm_transactions_included_delta, 0);
        assert_eq!(deferred.ivm_transactions_deferred, 0);
    }

    #[test]
    fn schedule_proposal_batch_rejects_mismatched_candidates_and_routes() {
        let routing = routing_for_lanes(&[1, 2]);
        let candidates = vec![proposal_candidate(1, false)];

        assert_eq!(
            schedule_proposal_batch(&routing, &candidates, proposal_context(), 0, 0),
            Err(ProposalBatchScheduleError::CandidateRoutingLengthMismatch {
                candidates: 1,
                routing_decisions: 2
            })
        );
    }

    #[test]
    fn lane_consensus_domains_include_only_accepted_work_and_canonicalize_validators() {
        let routing = routing_for_lane_dataspaces(&[(1, 11), (2, 22), (1, 11)]);
        let validators = vec![test_peer(3), test_peer(1), test_peer(4), test_peer(2)];
        let mut expected_validators = validators.clone();
        expected_validators.sort();
        let schedule = mixed_schedule(vec![
            ProposalBatchAction::Accept {
                index: 0,
                exceeds_gas_limit: false,
            },
            ProposalBatchAction::Defer {
                index: 1,
                reason: ProposalDeferralReason::GasLimit,
            },
            ProposalBatchAction::Accept {
                index: 2,
                exceeds_gas_limit: false,
            },
        ]);

        let domains = plan_lane_consensus_domains(
            &routing,
            &schedule,
            &[committee(1, 11, validators, None)],
            "permissioned",
        )
        .expect("lane consensus domains");

        assert_eq!(domains.len(), 1);
        let domain = &domains[0];
        assert_eq!(domain.lane_id, LaneId::new(1));
        assert_eq!(domain.dataspace_id, DataSpaceId::new(11));
        assert_eq!(domain.accepted_candidates, 2);
        assert_eq!(domain.accepted_candidate_indices, vec![0, 2]);
        assert_eq!(domain.validator_set, expected_validators);
        assert_eq!(domain.quorum.validator_count, 4);
        assert_eq!(domain.quorum.min_quorum, 3);
        assert_eq!(
            domain.qc_mode_tag,
            LaneRelayEnvelope::lane_qc_mode_tag_for(
                LaneId::new(1),
                DataSpaceId::new(11),
                "permissioned"
            )
        );
    }

    #[test]
    fn lane_consensus_domains_preserve_scheduler_candidate_order_per_lane() {
        let routing = routing_for_lane_dataspaces(&[(1, 11), (2, 22), (1, 11), (2, 22)]);
        let schedule = accepted_schedule(&[2, 1, 0, 3]);
        let validators = vec![test_peer(1), test_peer(2), test_peer(3)];

        let domains = plan_lane_consensus_domains(
            &routing,
            &schedule,
            &[
                committee(1, 11, validators.clone(), None),
                committee(2, 22, validators, None),
            ],
            "permissioned",
        )
        .expect("lane consensus domains");

        assert_eq!(domains.len(), 2);
        assert_eq!(domains[0].lane_id, LaneId::new(1));
        assert_eq!(domains[0].accepted_candidate_indices, vec![2, 0]);
        assert_eq!(domains[1].lane_id, LaneId::new(2));
        assert_eq!(domains[1].accepted_candidate_indices, vec![1, 3]);
    }

    #[test]
    fn lane_consensus_committees_use_authority_for_accepted_lanes_only() {
        let routing = routing_for_lane_dataspaces(&[(1, 11), (2, 22), (1, 11), (3, 33)]);
        let schedule = mixed_schedule(vec![
            ProposalBatchAction::Accept {
                index: 2,
                exceeds_gas_limit: false,
            },
            ProposalBatchAction::Defer {
                index: 1,
                reason: ProposalDeferralReason::GasLimit,
            },
            ProposalBatchAction::Accept {
                index: 0,
                exceeds_gas_limit: false,
            },
        ]);
        let lane1_authority = vec![test_peer(3), test_peer(1), test_peer(2)];
        let shared_validators = vec![test_peer(9), test_peer(10), test_peer(11)];
        let mut requested = Vec::new();

        let committees = plan_lane_consensus_committees_with_authority(
            &routing,
            &schedule,
            Some(&shared_validators),
            |lane_id, dataspace_id| {
                requested.push((lane_id, dataspace_id));
                if lane_id == LaneId::new(1) {
                    lane1_authority.clone()
                } else {
                    Vec::new()
                }
            },
        )
        .expect("lane consensus committees");

        assert_eq!(requested, vec![(LaneId::new(1), DataSpaceId::new(11))]);
        assert_eq!(committees.len(), 1);
        assert_eq!(committees[0].lane_id, LaneId::new(1));
        assert_eq!(committees[0].dataspace_id, DataSpaceId::new(11));
        assert_eq!(committees[0].validators, lane1_authority);
    }

    #[test]
    fn lane_consensus_committees_require_authority_without_shared_domain() {
        let routing = routing_for_lane_dataspaces(&[(1, 11), (2, 22)]);
        let schedule = accepted_schedule(&[0, 1]);

        assert_eq!(
            plan_lane_consensus_committees_with_authority(
                &routing,
                &schedule,
                None,
                |lane_id, _| {
                    if lane_id == LaneId::new(1) {
                        vec![test_peer(1), test_peer(2), test_peer(3)]
                    } else {
                        Vec::new()
                    }
                }
            ),
            Err(LaneConsensusDomainError::MissingLaneCommittee {
                lane_id: LaneId::new(2),
            })
        );
    }

    #[test]
    fn lane_consensus_committees_use_explicit_shared_domain_roster() {
        let routing = routing_for_lane_dataspaces(&[(1, 11), (2, 22)]);
        let schedule = accepted_schedule(&[0, 1]);
        let shared_validators = vec![test_peer(4), test_peer(5), test_peer(6)];

        let committees = plan_lane_consensus_committees_with_authority(
            &routing,
            &schedule,
            Some(&shared_validators),
            |_, _| Vec::new(),
        )
        .expect("shared-domain committees");

        assert_eq!(
            committees
                .iter()
                .map(|committee| (
                    committee.lane_id,
                    committee.dataspace_id,
                    committee.validators.clone()
                ))
                .collect::<Vec<_>>(),
            vec![
                (
                    LaneId::new(1),
                    DataSpaceId::new(11),
                    shared_validators.clone()
                ),
                (LaneId::new(2), DataSpaceId::new(22), shared_validators),
            ]
        );
    }

    #[test]
    fn lane_block_subjects_bind_coordinates_mode_tag_and_candidate_order() {
        let routing = routing_for_lane_dataspaces(&[(1, 11), (2, 22), (1, 11), (2, 22)]);
        let validators = vec![test_peer(1), test_peer(2), test_peer(3)];
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[2, 1, 0, 3]),
            &[
                committee(1, 11, validators.clone(), None),
                committee(2, 22, validators, None),
            ],
            "permissioned",
        )
        .expect("lane consensus domains");

        let subjects =
            plan_lane_block_subjects(&domains, &tx_hashes(4), 42, 7).expect("lane block subjects");

        assert_eq!(subjects.len(), 2);
        assert_eq!(subjects[0].lane_id, LaneId::new(1));
        assert_eq!(subjects[0].dataspace_id, DataSpaceId::new(11));
        assert_eq!(subjects[0].lane_block_height, 42);
        assert_eq!(subjects[0].lane_block_view, 7);
        assert_eq!(subjects[0].accepted_candidate_indices, vec![2, 0]);
        assert_eq!(
            subjects[0].qc_mode_tag,
            LaneRelayEnvelope::lane_qc_mode_tag_for(
                LaneId::new(1),
                DataSpaceId::new(11),
                "permissioned"
            )
        );

        let view_drift = plan_lane_block_subjects(&domains, &tx_hashes(4), 42, 8)
            .expect("lane block subjects with view drift");
        assert_ne!(subjects[0].subject_hash, view_drift[0].subject_hash);

        let mut reordered_work = domains.clone();
        reordered_work[0].accepted_candidate_indices.reverse();
        let reordered_subjects = plan_lane_block_subjects(&reordered_work, &tx_hashes(4), 42, 7)
            .expect("reordered subjects");
        assert_ne!(subjects[0].subject_hash, reordered_subjects[0].subject_hash);

        let mut mode_drift = domains.clone();
        mode_drift[0].qc_mode_tag.push_str("::tampered");
        let mode_drift_subjects = plan_lane_block_subjects(&mode_drift, &tx_hashes(4), 42, 7)
            .expect("mode drift subjects");
        assert_ne!(
            subjects[0].subject_hash,
            mode_drift_subjects[0].subject_hash
        );
    }

    #[test]
    fn lane_block_subjects_are_sorted_independent_of_domain_input_order() {
        let routing = routing_for_lane_dataspaces(&[(1, 11), (2, 22)]);
        let validators = vec![test_peer(1), test_peer(2), test_peer(3)];
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0, 1]),
            &[
                committee(1, 11, validators.clone(), None),
                committee(2, 22, validators, None),
            ],
            "permissioned",
        )
        .expect("lane consensus domains");
        let mut reversed_domains = domains.clone();
        reversed_domains.reverse();

        let subjects =
            plan_lane_block_subjects(&domains, &tx_hashes(2), 3, 4).expect("lane block subjects");
        let reversed_subjects = plan_lane_block_subjects(&reversed_domains, &tx_hashes(2), 3, 4)
            .expect("reversed subjects");

        assert_eq!(
            subjects
                .iter()
                .map(|subject| subject.lane_id)
                .collect::<Vec<_>>(),
            vec![LaneId::new(1), LaneId::new(2)]
        );
        assert_eq!(
            subjects
                .iter()
                .map(|subject| subject.subject_hash)
                .collect::<Vec<_>>(),
            reversed_subjects
                .iter()
                .map(|subject| subject.subject_hash)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn lane_block_subjects_for_slots_bind_independent_lane_heights_and_views() {
        let routing = routing_for_lane_dataspaces(&[(1, 11), (2, 22), (1, 11), (2, 22)]);
        let validators = vec![test_peer(1), test_peer(2), test_peer(3)];
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[2, 1, 0, 3]),
            &[
                committee(1, 11, validators.clone(), None),
                committee(2, 22, validators, None),
            ],
            "permissioned",
        )
        .expect("lane consensus domains");
        let slots = vec![
            LaneBlockSlot {
                lane_id: LaneId::new(2),
                dataspace_id: DataSpaceId::new(22),
                lane_incarnation: lane_subject_test_incarnation(2, 22),
                lane_block_height: 4,
                lane_block_view: 8,
            },
            LaneBlockSlot {
                lane_id: LaneId::new(1),
                dataspace_id: DataSpaceId::new(11),
                lane_incarnation: lane_subject_test_incarnation(1, 11),
                lane_block_height: 10,
                lane_block_view: 1,
            },
        ];

        let subjects = plan_lane_block_subjects_for_slots(&domains, &tx_hashes(4), &slots)
            .expect("slotted subjects");

        assert_eq!(subjects.len(), 2);
        assert_eq!(subjects[0].lane_id, LaneId::new(1));
        assert_eq!(subjects[0].lane_block_height, 10);
        assert_eq!(subjects[0].lane_block_view, 1);
        assert_eq!(subjects[1].lane_id, LaneId::new(2));
        assert_eq!(subjects[1].lane_block_height, 4);
        assert_eq!(subjects[1].lane_block_view, 8);

        let uniform_subjects = plan_lane_block_subjects(&domains, &tx_hashes(4), 10, 1)
            .expect("uniform-slot subjects");
        assert_eq!(subjects[0].subject_hash, uniform_subjects[0].subject_hash);
        assert_ne!(subjects[1].subject_hash, uniform_subjects[1].subject_hash);
    }

    #[test]
    fn lane_block_slots_from_tips_advance_active_lanes_independently() {
        let routing = routing_for_lane_dataspaces(&[(1, 11), (2, 22), (1, 11), (2, 22)]);
        let validators = vec![test_peer(1), test_peer(2), test_peer(3)];
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[2, 1, 0, 3]),
            &[
                committee(1, 11, validators.clone(), None),
                committee(2, 22, validators, None),
            ],
            "permissioned",
        )
        .expect("lane consensus domains");
        let lane_tips = vec![lane_tip(2, 22, 3), lane_tip(7, 77, 31), lane_tip(1, 11, 9)];

        let slots =
            plan_next_lane_block_slots(&domains, &lane_tips, 5).expect("next lane block slots");

        assert_eq!(
            slots,
            vec![
                LaneBlockSlot {
                    lane_id: LaneId::new(1),
                    dataspace_id: DataSpaceId::new(11),
                    lane_incarnation: lane_tip(1, 11, 0).lane_incarnation,
                    lane_block_height: 10,
                    lane_block_view: 5,
                },
                LaneBlockSlot {
                    lane_id: LaneId::new(2),
                    dataspace_id: DataSpaceId::new(22),
                    lane_incarnation: lane_tip(2, 22, 0).lane_incarnation,
                    lane_block_height: 4,
                    lane_block_view: 5,
                },
            ],
            "slot planning must ignore idle lane tips and sort active slots deterministically"
        );

        let subjects = plan_lane_block_subjects_for_slots(&domains, &tx_hashes(4), &slots)
            .expect("slotted subjects");
        assert_eq!(subjects[0].lane_block_height, 10);
        assert_eq!(subjects[1].lane_block_height, 4);
        assert_ne!(
            subjects[0].subject_hash, subjects[1].subject_hash,
            "independent lane heights should produce distinct subject identities"
        );

        let new_lane_domain = LaneConsensusDomain {
            lane_id: LaneId::new(8),
            dataspace_id: DataSpaceId::new(88),
            accepted_candidates: 1,
            accepted_candidate_indices: vec![0],
            validator_set: vec![test_peer(4), test_peer(5), test_peer(6)],
            quorum: LaneRelayQuorumContext::new(3, 2).expect("valid quorum"),
            qc_mode_tag: LaneRelayEnvelope::lane_qc_mode_tag_for(
                LaneId::new(8),
                DataSpaceId::new(88),
                "permissioned",
            ),
        };
        let new_lane_slots =
            plan_next_lane_block_slots(&[new_lane_domain], &[lane_tip(8, 88, 0)], 0)
                .expect("new lane first slot");
        assert_eq!(new_lane_slots[0].lane_block_height, 1);
    }

    #[test]
    fn lane_payload_plan_derives_tips_slots_subjects_and_ownerships() {
        let routing = routing_for_lane_dataspaces(&[(1, 11), (2, 22), (1, 11)]);
        let validators = vec![test_peer(1), test_peer(2), test_peer(3)];
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[2, 1, 0]),
            &[
                committee(1, 11, validators.clone(), None),
                committee(2, 22, validators, None),
            ],
            "permissioned",
        )
        .expect("lane consensus domains");
        let known_tips = vec![lane_tip(2, 22, 0), lane_tip_with_descriptor(1, 11, 7, 0x71)];
        let candidate_hashes = vec![tx_hash(0xA0), tx_hash(0xA1), tx_hash(0xA2)];
        let proposal_height = 10;

        let plan = plan_lane_payload(&domains, &known_tips, &candidate_hashes, proposal_height, 5)
            .expect("lane plan");

        assert_eq!(
            plan.lane_tips,
            vec![
                LaneBlockTip {
                    lane_id: LaneId::new(1),
                    dataspace_id: DataSpaceId::new(11),
                    lane_incarnation: Hash::new(b"lane-tip-incarnation:1:11"),
                    latest_lane_block_height: 7,
                    latest_lane_block_descriptor_hash: Some(Hash::prehashed([0x71; Hash::LENGTH])),
                },
                LaneBlockTip {
                    lane_id: LaneId::new(2),
                    dataspace_id: DataSpaceId::new(22),
                    lane_incarnation: Hash::new(b"lane-tip-incarnation:2:22"),
                    latest_lane_block_height: 0,
                    latest_lane_block_descriptor_hash: None,
                },
            ]
        );
        assert_eq!(
            plan.slots,
            vec![
                LaneBlockSlot {
                    lane_id: LaneId::new(1),
                    dataspace_id: DataSpaceId::new(11),
                    lane_incarnation: Hash::new(b"lane-tip-incarnation:1:11"),
                    lane_block_height: 8,
                    lane_block_view: 5,
                },
                LaneBlockSlot {
                    lane_id: LaneId::new(2),
                    dataspace_id: DataSpaceId::new(22),
                    lane_incarnation: Hash::new(b"lane-tip-incarnation:2:22"),
                    lane_block_height: 1,
                    lane_block_view: 5,
                },
            ]
        );
        assert_eq!(
            plan.subjects
                .iter()
                .map(|subject| (
                    subject.lane_id,
                    subject.lane_block_height,
                    subject.accepted_candidate_indices.clone()
                ))
                .collect::<Vec<_>>(),
            vec![
                (LaneId::new(1), 8, vec![2, 0]),
                (LaneId::new(2), 1, vec![1]),
            ]
        );
        assert_eq!(
            plan.ownerships
                .iter()
                .map(|ownership| (
                    ownership.lane_id,
                    ownership.lane_block_height,
                    ownership.accepted_candidate_indices.clone()
                ))
                .collect::<Vec<_>>(),
            vec![
                (LaneId::new(1), 8, vec![2, 0]),
                (LaneId::new(2), 1, vec![1]),
            ]
        );
        assert_eq!(
            plan.entries
                .iter()
                .map(|entry| (
                    entry.domain.lane_id,
                    entry.tip.latest_lane_block_height,
                    entry.slot.lane_block_height,
                    entry.subject.subject_hash,
                    entry.ownership.subject_hash,
                    entry.accepted_transaction_hashes.clone(),
                ))
                .collect::<Vec<_>>(),
            vec![
                (
                    LaneId::new(1),
                    7,
                    8,
                    plan.subjects[0].subject_hash,
                    plan.ownerships[0].subject_hash,
                    vec![candidate_hashes[2], candidate_hashes[0]],
                ),
                (
                    LaneId::new(2),
                    0,
                    1,
                    plan.subjects[1].subject_hash,
                    plan.ownerships[1].subject_hash,
                    vec![candidate_hashes[1]],
                ),
            ],
            "standalone lane descriptors must group matching tip, slot, subject, ownership, and transaction hashes"
        );
        assert_eq!(
            plan.lane_block_proposals,
            plan.entries
                .iter()
                .map(|entry| entry.lane_block_proposal.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            plan.lane_block_proposal_artifacts,
            plan.entries
                .iter()
                .map(|entry| entry.lane_block_proposal.artifact.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(plan.lane_block_prepare_vote_plans.len(), plan.entries.len());
        assert_eq!(plan.lane_block_commit_vote_plans.len(), plan.entries.len());
        assert_eq!(
            plan.entries[0].domain.validator_set,
            domains[0].validator_set
        );
        assert_eq!(plan.entries[0].subject, plan.subjects[0]);
        assert_eq!(plan.entries[0].ownership, plan.ownerships[0]);
        let first_descriptor = &plan.entries[0].block_descriptor;
        assert_eq!(first_descriptor.lane_id, LaneId::new(1));
        assert_eq!(first_descriptor.dataspace_id, DataSpaceId::new(11));
        assert_eq!(first_descriptor.proposal_height, proposal_height);
        assert_eq!(first_descriptor.previous_lane_block_height, 7);
        assert_eq!(
            first_descriptor.previous_lane_block_descriptor_hash,
            Some(Hash::prehashed([0x71; Hash::LENGTH]))
        );
        assert_eq!(first_descriptor.lane_block_height, 8);
        assert_eq!(first_descriptor.lane_block_view, 5);
        assert_eq!(first_descriptor.subject_hash, plan.subjects[0].subject_hash);
        assert_eq!(
            first_descriptor.payload_ownership_hash,
            plan.ownerships[0].payload_ownership_hash
        );
        assert_eq!(
            first_descriptor.rbc_instance_hash,
            plan.ownerships[0].rbc_instance_hash
        );
        assert_eq!(first_descriptor.accepted_candidate_indices, vec![2, 0]);
        assert_eq!(
            first_descriptor.accepted_transaction_hashes,
            vec![candidate_hashes[2], candidate_hashes[0]]
        );
        assert_eq!(first_descriptor.validator_set, domains[0].validator_set);
        assert_eq!(first_descriptor.quorum, domains[0].quorum);
        assert_eq!(first_descriptor.qc_mode_tag, domains[0].qc_mode_tag);
        let first_proposal = &plan.entries[0].lane_block_proposal;
        assert_eq!(&first_proposal.block_descriptor, first_descriptor);
        assert_eq!(first_proposal.subject, plan.subjects[0]);
        assert_eq!(first_proposal.ownership, plan.ownerships[0]);
        assert_eq!(
            first_proposal.artifact.descriptor.descriptor_hash,
            first_descriptor.descriptor_hash
        );
        assert_eq!(
            first_proposal.artifact.computed_proposal_hash(),
            first_proposal.proposal_hash
        );
        assert_ne!(
            first_proposal.proposal_hash,
            first_descriptor.descriptor_hash
        );
        assert_ne!(first_proposal.proposal_hash, first_descriptor.subject_hash);
        assert_ne!(
            first_descriptor.descriptor_hash,
            first_descriptor.subject_hash
        );
        let first_prepare_votes = &plan.lane_block_prepare_vote_plans[0];
        let first_commit_votes = &plan.lane_block_commit_vote_plans[0];
        assert_eq!(first_prepare_votes.phase, CertPhase::Prepare);
        assert_eq!(first_commit_votes.phase, CertPhase::Commit);
        assert_eq!(
            first_prepare_votes.proposal_hash,
            first_proposal.proposal_hash
        );
        assert_eq!(
            first_commit_votes.proposal_hash,
            first_proposal.proposal_hash
        );
        assert_eq!(
            first_prepare_votes.descriptor_hash,
            first_descriptor.descriptor_hash
        );
        assert_eq!(
            first_prepare_votes.votes.len(),
            first_descriptor.validator_set.len()
        );
        assert_eq!(
            first_prepare_votes.votes[0].body,
            first_proposal.artifact.vote_body(CertPhase::Prepare)
        );
        assert_eq!(
            first_commit_votes.votes[0].body,
            first_proposal.artifact.vote_body(CertPhase::Commit)
        );
        assert_eq!(
            first_prepare_votes
                .votes
                .iter()
                .map(|vote| vote.signer_index)
                .collect::<Vec<_>>(),
            vec![0, 1, 2],
            "full-committee vote templates should follow canonical signer order"
        );
        assert!(
            first_prepare_votes
                .votes
                .windows(2)
                .all(|votes| votes[0].signing_hash == votes[1].signing_hash),
            "prepare vote signing hash must be common across signers"
        );
        assert_ne!(
            first_prepare_votes.votes[0].signing_hash, first_commit_votes.votes[0].signing_hash,
            "prepare and commit vote templates must not share a signable digest"
        );
        assert_eq!(
            plan.subjects[0].subject_hash,
            plan.ownerships[0].subject_hash
        );
        assert_ne!(
            plan.ownerships[0].payload_ownership_hash,
            plan.ownerships[0].rbc_instance_hash
        );

        for entry in &plan.entries {
            let ownership = &entry.ownership;
            let wire_ownership = SumeragiLanePayloadOwnership {
                proposal_height,
                proposal_view: entry.slot.lane_block_view,
                lane_id: ownership.lane_id,
                dataspace_id: ownership.dataspace_id,
                lane_incarnation: ownership.lane_incarnation,
                lane_block_height: ownership.lane_block_height,
                lane_block_view: ownership.lane_block_view,
                subject_hash: ownership.subject_hash,
                qc_mode_tag: ownership.qc_mode_tag.clone(),
                accepted_candidate_indices: ownership
                    .accepted_candidate_indices
                    .iter()
                    .map(|index| u64::try_from(*index).expect("candidate index fits u64"))
                    .collect(),
                accepted_transaction_hashes: ownership.accepted_transaction_hashes.clone(),
                previous_lane_block_height: entry.block_descriptor.previous_lane_block_height,
                previous_lane_block_descriptor_hash: entry
                    .block_descriptor
                    .previous_lane_block_descriptor_hash,
                lane_block_descriptor_hash: Some(entry.block_descriptor.descriptor_hash),
                lane_block_descriptor_validator_set: entry.block_descriptor.validator_set.clone(),
                lane_block_descriptor_validator_count: entry
                    .block_descriptor
                    .quorum
                    .validator_count,
                lane_block_descriptor_min_quorum: entry.block_descriptor.quorum.min_quorum,
                payload_ownership_hash: ownership.payload_ownership_hash,
                rbc_instance_hash: ownership.rbc_instance_hash,
            };

            wire_ownership
                .validate_replay_material()
                .expect("scheduler wire ownership should validate replay material");
        }
    }

    #[test]
    fn lane_block_descriptor_binds_committee_without_changing_payload_identity() {
        let routing = routing_for_lane_dataspaces(&[(1, 11)]);
        let candidate_hashes = vec![tx_hash(0xA9)];
        let known_tips = vec![lane_tip(1, 11, 3)];
        let domains_a = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0]),
            &[committee(
                1,
                11,
                vec![test_peer(1), test_peer(2), test_peer(3)],
                None,
            )],
            "permissioned",
        )
        .expect("lane consensus domain");
        let domains_b = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0]),
            &[committee(
                1,
                11,
                vec![test_peer(4), test_peer(5), test_peer(6)],
                None,
            )],
            "permissioned",
        )
        .expect("lane consensus domain");

        let plan_a = plan_lane_payload(&domains_a, &known_tips, &candidate_hashes, 100, 2)
            .expect("lane plan with first committee");
        let plan_b = plan_lane_payload(&domains_b, &known_tips, &candidate_hashes, 100, 2)
            .expect("lane plan with second committee");

        assert_eq!(
            plan_a.entries[0].subject.subject_hash, plan_b.entries[0].subject.subject_hash,
            "lane-local payload identity does not include committee membership"
        );
        assert_eq!(
            plan_a.entries[0].ownership.payload_ownership_hash,
            plan_b.entries[0].ownership.payload_ownership_hash
        );
        assert_ne!(
            plan_a.entries[0].block_descriptor.validator_set,
            plan_b.entries[0].block_descriptor.validator_set
        );
        assert_ne!(
            plan_a.entries[0].block_descriptor.descriptor_hash,
            plan_b.entries[0].block_descriptor.descriptor_hash,
            "standalone descriptor must bind the lane-local voting committee"
        );
        assert_ne!(
            plan_a.entries[0].lane_block_proposal.proposal_hash,
            plan_b.entries[0].lane_block_proposal.proposal_hash,
            "standalone proposal identity must bind the voting committee through the descriptor"
        );
    }

    #[test]
    fn lane_block_descriptor_binds_predecessor_descriptor_without_changing_payload_identity() {
        let routing = routing_for_lane_dataspaces(&[(1, 11)]);
        let candidate_hashes = vec![tx_hash(0xAA)];
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0]),
            &[committee(
                1,
                11,
                vec![test_peer(1), test_peer(2), test_peer(3)],
                None,
            )],
            "permissioned",
        )
        .expect("lane consensus domain");

        let plan_a = plan_lane_payload(
            &domains,
            &[lane_tip_with_descriptor(1, 11, 3, 0xA1)],
            &candidate_hashes,
            100,
            2,
        )
        .expect("lane plan with first predecessor descriptor");
        let plan_b = plan_lane_payload(
            &domains,
            &[lane_tip_with_descriptor(1, 11, 3, 0xA2)],
            &candidate_hashes,
            100,
            2,
        )
        .expect("lane plan with second predecessor descriptor");

        assert_eq!(
            plan_a.entries[0].subject.subject_hash, plan_b.entries[0].subject.subject_hash,
            "lane-local payload identity is independent of predecessor descriptor material"
        );
        assert_eq!(
            plan_a.entries[0].ownership.payload_ownership_hash,
            plan_b.entries[0].ownership.payload_ownership_hash
        );
        assert_ne!(
            plan_a.entries[0]
                .block_descriptor
                .previous_lane_block_descriptor_hash,
            plan_b.entries[0]
                .block_descriptor
                .previous_lane_block_descriptor_hash
        );
        assert_ne!(
            plan_a.entries[0].block_descriptor.descriptor_hash,
            plan_b.entries[0].block_descriptor.descriptor_hash,
            "standalone descriptor must bind the predecessor lane descriptor"
        );
        assert_ne!(
            plan_a.entries[0].lane_block_proposal.proposal_hash,
            plan_b.entries[0].lane_block_proposal.proposal_hash,
            "standalone proposal identity must bind predecessor descriptor lineage"
        );
    }

    #[test]
    fn lane_payload_plan_entries_reject_internal_stage_drift() {
        let routing = routing_for_lane_dataspaces(&[(1, 11)]);
        let validators = vec![test_peer(1), test_peer(2), test_peer(3)];
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0]),
            &[committee(1, 11, validators, None)],
            "permissioned",
        )
        .expect("lane consensus domain");
        let candidate_hashes = vec![tx_hash(0xB0)];
        let plan = plan_lane_payload(&domains, &[], &candidate_hashes, 5, 2).expect("lane plan");
        let mut tampered_ownerships = plan.ownerships.clone();
        tampered_ownerships[0].accepted_candidate_indices.push(99);

        let err = build_lane_payload_plan_entries(
            &domains,
            &plan.lane_tips,
            &plan.slots,
            &plan.subjects,
            &tampered_ownerships,
            &candidate_hashes,
            plan.entries[0].block_descriptor.proposal_height,
        )
        .expect_err("entry builder must reject mismatched ownership descriptors");

        assert_eq!(
            err,
            LanePayloadPlanError::InconsistentEntry {
                lane_id: LaneId::new(1)
            }
        );
    }

    #[test]
    fn lane_block_proposal_rejects_descriptor_subject_drift() {
        let routing = routing_for_lane_dataspaces(&[(1, 11)]);
        let validators = vec![test_peer(1), test_peer(2), test_peer(3)];
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0]),
            &[committee(1, 11, validators, None)],
            "permissioned",
        )
        .expect("lane consensus domain");
        let candidate_hashes = vec![tx_hash(0xB1)];
        let plan = plan_lane_payload(&domains, &[], &candidate_hashes, 5, 2).expect("lane plan");
        let mut descriptor = plan.entries[0].block_descriptor.clone();
        descriptor.subject_hash = Hash::prehashed([0xE1; Hash::LENGTH]);

        let err = build_lane_block_proposal(
            descriptor.lane_id,
            &descriptor,
            &plan.entries[0].subject,
            &plan.entries[0].ownership,
        )
        .expect_err("proposal builder must reject descriptor/subject drift");

        assert_eq!(
            err,
            LanePayloadPlanError::InconsistentEntry {
                lane_id: LaneId::new(1)
            }
        );
    }

    #[test]
    fn lane_block_vote_plan_sorts_signers_and_uses_common_signing_hash() {
        let proposal = lane_block_proposal_with_committee(
            vec![test_peer(3), test_peer(1), test_peer(4), test_peer(2)],
            Some(3),
        );
        let validators = proposal.block_descriptor.validator_set.clone();
        let vote_plan = plan_lane_block_vote_quorum(
            &proposal,
            CertPhase::Prepare,
            &[
                validators[2].clone(),
                validators[0].clone(),
                validators[1].clone(),
            ],
        )
        .expect("prepare vote quorum");

        assert_eq!(vote_plan.phase, CertPhase::Prepare);
        assert_eq!(vote_plan.proposal_hash, proposal.proposal_hash);
        assert_eq!(
            vote_plan.descriptor_hash,
            proposal.block_descriptor.descriptor_hash
        );
        assert_eq!(vote_plan.min_quorum, 3);
        assert_eq!(
            vote_plan
                .votes
                .iter()
                .map(|vote| vote.signer_index)
                .collect::<Vec<_>>(),
            vec![0, 1, 2],
            "votes must be sorted by descriptor signer index, not input order"
        );
        assert_eq!(
            vote_plan.votes[0].signing_hash,
            vote_plan.votes[1].signing_hash
        );
        assert_eq!(
            vote_plan.votes[0].validator_set_hash,
            vote_plan.validator_set_hash
        );
        assert_eq!(vote_plan.votes[0].lane_id, LaneId::new(1));
        assert_eq!(vote_plan.votes[0].dataspace_id, DataSpaceId::new(11));
        assert_eq!(vote_plan.votes[0].lane_block_height, 4);
        assert_eq!(vote_plan.votes[0].lane_block_view, 2);

        let single_vote = plan_lane_block_vote(&proposal, CertPhase::Prepare, &validators[1])
            .expect("single lane vote");
        assert_eq!(
            single_vote.signing_hash, vote_plan.votes[0].signing_hash,
            "signer-local transport fields must stay outside the signable digest"
        );

        let commit_votes = plan_lane_block_votes(
            &proposal,
            CertPhase::Commit,
            &[validators[0].clone(), validators[1].clone()],
        )
        .expect("commit votes");
        assert_ne!(
            commit_votes[0].signing_hash, vote_plan.votes[0].signing_hash,
            "prepare and commit votes must be domain-separated"
        );
    }

    #[test]
    fn lane_block_vote_plan_rejects_invalid_phase_and_under_quorum() {
        let proposal = lane_block_proposal_with_committee(
            vec![test_peer(1), test_peer(2), test_peer(3)],
            Some(3),
        );
        let validators = proposal.block_descriptor.validator_set.clone();

        assert_eq!(
            plan_lane_block_vote(&proposal, CertPhase::NewView, &validators[0]),
            Err(LaneBlockVotePlanError::InvalidPhase {
                phase: CertPhase::NewView,
            })
        );
        assert_eq!(
            plan_lane_block_vote_quorum(
                &proposal,
                CertPhase::Prepare,
                std::slice::from_ref(&validators[0]),
            ),
            Err(LaneBlockVotePlanError::InsufficientVoteQuorum {
                lane_id: LaneId::new(1),
                observed: 1,
                min_quorum: 3,
            })
        );
    }

    #[test]
    fn lane_block_vote_plan_rejects_noncanonical_descriptor_quorum() {
        for min_quorum in [2, 4] {
            let mut proposal = lane_block_proposal_with_committee(
                vec![test_peer(1), test_peer(2), test_peer(3), test_peer(4)],
                Some(3),
            );
            proposal.block_descriptor.quorum.min_quorum = min_quorum;
            refresh_lane_block_proposal_hashes(&mut proposal);
            let signer = proposal.block_descriptor.validator_set[0].clone();

            assert_eq!(
                plan_lane_block_vote(&proposal, CertPhase::Prepare, &signer),
                Err(LaneBlockVotePlanError::InvalidQuorum {
                    lane_id: LaneId::new(1),
                    validator_count: 4,
                    min_quorum,
                }),
                "lane validators must not sign descriptors whose quorum diverges from canonical 3-of-4"
            );
        }
    }

    #[test]
    fn lane_block_vote_plan_rejects_duplicate_and_unknown_signers() {
        let proposal = lane_block_proposal_with_committee(
            vec![test_peer(1), test_peer(2), test_peer(3)],
            Some(3),
        );
        let validators = proposal.block_descriptor.validator_set.clone();

        assert_eq!(
            plan_lane_block_votes(
                &proposal,
                CertPhase::Prepare,
                &[validators[0].clone(), validators[0].clone()],
            ),
            Err(LaneBlockVotePlanError::DuplicateSigner {
                lane_id: LaneId::new(1),
            })
        );
        assert_eq!(
            plan_lane_block_vote(&proposal, CertPhase::Prepare, &test_peer(99)),
            Err(LaneBlockVotePlanError::SignerNotInCommittee {
                lane_id: LaneId::new(1),
            })
        );
    }

    #[test]
    fn lane_block_vote_plan_rejects_tampered_descriptor_and_proposal_hashes() {
        let proposal = lane_block_proposal_with_committee(
            vec![test_peer(1), test_peer(2), test_peer(3)],
            Some(3),
        );
        let signer = proposal.block_descriptor.validator_set[0].clone();

        let mut descriptor_tampered = proposal.clone();
        let actual_descriptor = Hash::prehashed([0xD1; Hash::LENGTH]);
        descriptor_tampered.block_descriptor.descriptor_hash = actual_descriptor;
        let descriptor_err =
            plan_lane_block_vote(&descriptor_tampered, CertPhase::Prepare, &signer)
                .expect_err("descriptor hash drift must be rejected");
        assert!(matches!(
            descriptor_err,
            LaneBlockVotePlanError::DescriptorHashMismatch {
                lane_id,
                actual,
                ..
            } if lane_id == LaneId::new(1) && actual == actual_descriptor
        ));

        let mut proposal_tampered = proposal;
        let actual_proposal = Hash::prehashed([0xD2; Hash::LENGTH]);
        proposal_tampered.proposal_hash = actual_proposal;
        let proposal_err = plan_lane_block_vote(&proposal_tampered, CertPhase::Prepare, &signer)
            .expect_err("proposal hash drift must be rejected");
        assert!(matches!(
            proposal_err,
            LaneBlockVotePlanError::ProposalHashMismatch {
                lane_id,
                actual,
                ..
            } if lane_id == LaneId::new(1) && actual == actual_proposal
        ));
    }

    #[test]
    fn lane_block_vote_plan_rejects_tampered_public_artifact() {
        let proposal = lane_block_proposal_with_committee(
            vec![test_peer(1), test_peer(2), test_peer(3)],
            Some(3),
        );
        let signer = proposal.block_descriptor.validator_set[0].clone();

        let mut descriptor_artifact_tampered = proposal.clone();
        descriptor_artifact_tampered
            .artifact
            .descriptor
            .validator_set_hash =
            HashOf::from_untyped_unchecked(Hash::prehashed([0xE1; Hash::LENGTH]));
        assert_eq!(
            plan_lane_block_vote(&descriptor_artifact_tampered, CertPhase::Prepare, &signer),
            Err(LaneBlockVotePlanError::InconsistentProposal {
                lane_id: LaneId::new(1),
            })
        );

        let mut proposal_artifact_tampered = proposal;
        let actual = Hash::prehashed([0xE2; Hash::LENGTH]);
        proposal_artifact_tampered.artifact.proposal_hash = actual;
        assert_eq!(
            plan_lane_block_vote(&proposal_artifact_tampered, CertPhase::Prepare, &signer),
            Err(LaneBlockVotePlanError::ProposalHashMismatch {
                lane_id: LaneId::new(1),
                expected: proposal_artifact_tampered.proposal_hash,
                actual,
            })
        );
    }

    #[test]
    fn lane_block_vote_plan_rejects_noncanonical_descriptor_validator_set() {
        let mut proposal = lane_block_proposal_with_committee(
            vec![test_peer(1), test_peer(2), test_peer(3)],
            Some(3),
        );
        let signer = proposal.block_descriptor.validator_set[0].clone();
        proposal.block_descriptor.validator_set.swap(0, 1);

        assert_eq!(
            plan_lane_block_vote(&proposal, CertPhase::Prepare, &signer),
            Err(LaneBlockVotePlanError::ValidatorSetNotCanonical {
                lane_id: LaneId::new(1),
            })
        );
    }

    #[test]
    fn lane_payload_plan_rejects_missing_candidate_hash_for_accepted_index() {
        let routing = routing_for_lane_dataspaces(&[(1, 11), (1, 11)]);
        let validators = vec![test_peer(1), test_peer(2), test_peer(3)];
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[1]),
            &[committee(1, 11, validators, None)],
            "permissioned",
        )
        .expect("lane consensus domain");

        let err = plan_lane_payload(&domains, &[], &[tx_hash(0xC0)], 5, 2)
            .expect_err("accepted candidate without transaction hash must fail closed");

        assert_eq!(
            err,
            LanePayloadPlanError::Subjects(LaneBlockSubjectError::CandidateHashIndexOutOfBounds {
                lane_id: LaneId::new(1),
                index: 1,
                candidate_hashes: 1,
            })
        );
    }

    #[test]
    fn lane_payload_plan_wraps_tip_dataspace_mismatch() {
        let routing = routing_for_lane_dataspaces(&[(1, 11)]);
        let validators = vec![test_peer(1), test_peer(2), test_peer(3)];
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0]),
            &[committee(1, 11, validators, None)],
            "permissioned",
        )
        .expect("lane consensus domain");

        let err = plan_lane_payload(&domains, &[lane_tip(1, 99, 4)], &[tx_hash(0xD0)], 4, 0)
            .expect_err("foreign-dataspace tip must fail closed");

        assert_eq!(
            err,
            LanePayloadPlanError::Tips(LaneBlockTipPlanError::LaneTipDataspaceMismatch {
                lane_id: LaneId::new(1),
                expected: DataSpaceId::new(11),
                actual: DataSpaceId::new(99),
            })
        );
    }

    #[test]
    fn latest_lane_block_tips_use_latest_exact_incarnation_tip() {
        let routing = routing_for_lane_dataspaces(&[(1, 11), (2, 22), (3, 33)]);
        let validators = vec![test_peer(1), test_peer(2), test_peer(3)];
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0, 1, 2]),
            &[
                committee(1, 11, validators.clone(), None),
                committee(2, 22, validators.clone(), None),
                committee(3, 33, validators, None),
            ],
            "permissioned",
        )
        .expect("lane consensus domains");
        let known_tips = vec![
            lane_tip(1, 11, 4),
            lane_tip(7, 77, 99),
            lane_tip_with_descriptor(1, 11, 8, 0x81),
            lane_tip(3, 33, 0),
        ];

        let tips = plan_latest_lane_block_tips_for_tests(&domains, &known_tips)
            .expect("latest lane block tips");

        assert_eq!(
            tips,
            vec![
                lane_tip_with_descriptor(1, 11, 8, 0x81),
                lane_tip(2, 22, 0),
                lane_tip(3, 33, 0),
            ],
            "tip reducer should keep the latest active-lane tip, ignore idle-lane tips, and start never-seen lanes at zero"
        );
    }

    #[test]
    fn latest_lane_block_tips_reject_conflicting_descriptor_hashes_at_same_height() {
        let routing = routing_for_lane_dataspaces(&[(1, 11)]);
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0]),
            &[committee(
                1,
                11,
                vec![test_peer(1), test_peer(2), test_peer(3)],
                None,
            )],
            "permissioned",
        )
        .expect("lane consensus domains");

        assert_eq!(
            plan_latest_lane_block_tips_for_tests(
                &domains,
                &[
                    lane_tip_with_descriptor(1, 11, 8, 0xB1),
                    lane_tip_with_descriptor(1, 11, 8, 0xB2),
                ],
            ),
            Err(LaneBlockTipPlanError::ConflictingLaneTipDescriptorHash {
                lane_id: LaneId::new(1),
                latest_lane_block_height: 8,
            }),
            "same-height lane tips with different predecessor descriptors must fail closed"
        );
    }

    #[test]
    fn latest_lane_block_tips_ignore_retired_incarnation_height() {
        let routing = routing_for_lane_dataspaces(&[(1, 11)]);
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0]),
            &[committee(
                1,
                11,
                vec![test_peer(1), test_peer(2), test_peer(3)],
                None,
            )],
            "permissioned",
        )
        .expect("lane consensus domains");
        let active_incarnation = Hash::new(b"active-lane-incarnation");
        let retired_tip = lane_tip_with_descriptor(1, 11, 99, 0xB1);
        assert_ne!(retired_tip.lane_incarnation, active_incarnation);

        let tips = plan_latest_lane_block_tips_with_incarnations(
            &domains,
            &[retired_tip],
            &BTreeMap::from([(LaneId::new(1), active_incarnation)]),
        )
        .expect("retired incarnation is inert");

        assert_eq!(
            tips,
            vec![LaneBlockTip {
                lane_id: LaneId::new(1),
                dataspace_id: DataSpaceId::new(11),
                lane_incarnation: active_incarnation,
                latest_lane_block_height: 0,
                latest_lane_block_descriptor_hash: None,
            }],
            "a high retired-incarnation tip must not advance the active lane namespace"
        );
    }

    #[test]
    fn latest_lane_block_tips_reject_duplicate_domains_and_dataspace_drift() {
        let routing = routing_for_lane_dataspaces(&[(1, 11)]);
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0]),
            &[committee(
                1,
                11,
                vec![test_peer(1), test_peer(2), test_peer(3)],
                None,
            )],
            "permissioned",
        )
        .expect("lane consensus domains");

        assert_eq!(
            plan_latest_lane_block_tips_for_tests(&[domains[0].clone(), domains[0].clone()], &[],),
            Err(LaneBlockTipPlanError::DuplicateLaneDomain {
                lane_id: LaneId::new(1),
                dataspace_id: DataSpaceId::new(11),
            })
        );

        assert_eq!(
            plan_latest_lane_block_tips_for_tests(&domains, &[lane_tip(1, 99, 7)]),
            Err(LaneBlockTipPlanError::LaneTipDataspaceMismatch {
                lane_id: LaneId::new(1),
                expected: DataSpaceId::new(11),
                actual: DataSpaceId::new(99),
            })
        );
    }

    #[test]
    fn lane_block_slots_from_tips_reject_malformed_inputs() {
        let routing = routing_for_lane_dataspaces(&[(1, 11)]);
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0]),
            &[committee(
                1,
                11,
                vec![test_peer(1), test_peer(2), test_peer(3)],
                None,
            )],
            "permissioned",
        )
        .expect("lane consensus domains");
        let tip = lane_tip(1, 11, 7);

        assert_eq!(
            plan_next_lane_block_slots(&domains, &[], 0),
            Err(LaneBlockSlotPlanError::MissingLaneTip {
                lane_id: LaneId::new(1),
            })
        );

        assert_eq!(
            plan_next_lane_block_slots(&domains, &[tip, tip], 0),
            Err(LaneBlockSlotPlanError::DuplicateLaneTip {
                lane_id: LaneId::new(1),
            })
        );

        let mismatched_dataspace = LaneBlockTip {
            dataspace_id: DataSpaceId::new(99),
            ..tip
        };
        assert_eq!(
            plan_next_lane_block_slots(&domains, &[mismatched_dataspace], 0),
            Err(LaneBlockSlotPlanError::LaneTipDataspaceMismatch {
                lane_id: LaneId::new(1),
                expected: DataSpaceId::new(11),
                actual: DataSpaceId::new(99),
            })
        );

        let overflow = LaneBlockTip {
            latest_lane_block_height: u64::MAX,
            ..tip
        };
        assert_eq!(
            plan_next_lane_block_slots(&domains, &[overflow], 0),
            Err(LaneBlockSlotPlanError::LaneBlockHeightOverflow {
                lane_id: LaneId::new(1),
                latest_lane_block_height: u64::MAX,
            })
        );

        assert_eq!(
            plan_next_lane_block_slots(&[domains[0].clone(), domains[0].clone()], &[tip], 0),
            Err(LaneBlockSlotPlanError::DuplicateLaneDomain {
                lane_id: LaneId::new(1),
                dataspace_id: DataSpaceId::new(11),
            })
        );
    }

    #[test]
    fn lane_block_subjects_for_slots_reject_malformed_slots() {
        let routing = routing_for_lane_dataspaces(&[(1, 11)]);
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0]),
            &[committee(
                1,
                11,
                vec![test_peer(1), test_peer(2), test_peer(3)],
                None,
            )],
            "permissioned",
        )
        .expect("lane consensus domains");
        let slot = LaneBlockSlot {
            lane_id: LaneId::new(1),
            dataspace_id: DataSpaceId::new(11),
            lane_incarnation: lane_subject_test_incarnation(1, 11),
            lane_block_height: 7,
            lane_block_view: 3,
        };

        assert_eq!(
            plan_lane_block_subjects_for_slots(&domains, &tx_hashes(1), &[]),
            Err(LaneBlockSubjectError::MissingLaneSlot {
                lane_id: LaneId::new(1),
            })
        );

        assert_eq!(
            plan_lane_block_subjects_for_slots(&domains, &tx_hashes(1), &[slot, slot]),
            Err(LaneBlockSubjectError::DuplicateLaneSlot {
                lane_id: LaneId::new(1),
            })
        );

        let mismatched_dataspace = LaneBlockSlot {
            dataspace_id: DataSpaceId::new(99),
            ..slot
        };
        assert_eq!(
            plan_lane_block_subjects_for_slots(&domains, &tx_hashes(1), &[mismatched_dataspace]),
            Err(LaneBlockSubjectError::LaneSlotDataspaceMismatch {
                lane_id: LaneId::new(1),
                expected: DataSpaceId::new(11),
                actual: DataSpaceId::new(99),
            })
        );

        let unexpected = LaneBlockSlot {
            lane_id: LaneId::new(2),
            dataspace_id: DataSpaceId::new(22),
            lane_incarnation: lane_subject_test_incarnation(2, 22),
            lane_block_height: 1,
            lane_block_view: 0,
        };
        assert_eq!(
            plan_lane_block_subjects_for_slots(&domains, &tx_hashes(1), &[slot, unexpected]),
            Err(LaneBlockSubjectError::UnexpectedLaneSlot {
                lane_id: LaneId::new(2),
            })
        );
    }

    #[test]
    fn lane_payload_ownership_binds_subject_hash_coordinates_and_candidate_order() {
        let routing = routing_for_lane_dataspaces(&[(1, 11), (2, 22), (1, 11), (2, 22)]);
        let validators = vec![test_peer(1), test_peer(2), test_peer(3)];
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[2, 1, 0, 3]),
            &[
                committee(1, 11, validators.clone(), None),
                committee(2, 22, validators, None),
            ],
            "permissioned",
        )
        .expect("lane consensus domains");
        let candidate_hashes = tx_hashes(4);
        let subjects = plan_lane_block_subjects(&domains, &candidate_hashes, 42, 7)
            .expect("lane block subjects");

        let ownerships = plan_lane_payload_ownership(&subjects).expect("lane payload ownership");

        assert_eq!(ownerships.len(), 2);
        assert_eq!(ownerships[0].lane_id, LaneId::new(1));
        assert_eq!(ownerships[0].dataspace_id, DataSpaceId::new(11));
        assert_eq!(ownerships[0].lane_block_height, 42);
        assert_eq!(ownerships[0].lane_block_view, 7);
        assert_eq!(ownerships[0].subject_hash, subjects[0].subject_hash);
        assert_eq!(ownerships[0].accepted_candidate_indices, vec![2, 0]);
        assert_eq!(
            ownerships[0].accepted_transaction_hashes,
            vec![candidate_hashes[2], candidate_hashes[0]]
        );
        assert_ne!(
            ownerships[0].payload_ownership_hash,
            ownerships[0].rbc_instance_hash
        );

        let view_drift_subjects = plan_lane_block_subjects(&domains, &candidate_hashes, 42, 8)
            .expect("lane block subjects with view drift");
        let view_drift_ownerships =
            plan_lane_payload_ownership(&view_drift_subjects).expect("view drift ownership");
        assert_ne!(
            ownerships[0].payload_ownership_hash,
            view_drift_ownerships[0].payload_ownership_hash
        );
        assert_ne!(
            ownerships[0].rbc_instance_hash,
            view_drift_ownerships[0].rbc_instance_hash
        );

        let hash_drift_candidate_hashes =
            vec![tx_hash(0xE0), tx_hash(0xE1), tx_hash(0xE2), tx_hash(0xE3)];
        let hash_drift_subjects =
            plan_lane_block_subjects(&domains, &hash_drift_candidate_hashes, 42, 7)
                .expect("hash drift subjects");
        let hash_drift_ownerships =
            plan_lane_payload_ownership(&hash_drift_subjects).expect("hash drift ownership");
        assert_ne!(
            subjects[0].subject_hash,
            hash_drift_subjects[0].subject_hash
        );
        assert_ne!(
            ownerships[0].payload_ownership_hash,
            hash_drift_ownerships[0].payload_ownership_hash
        );
        assert_ne!(
            ownerships[0].rbc_instance_hash,
            hash_drift_ownerships[0].rbc_instance_hash
        );

        let mut reordered_work = domains.clone();
        reordered_work[0].accepted_candidate_indices.reverse();
        let reordered_subjects =
            plan_lane_block_subjects(&reordered_work, &candidate_hashes, 42, 7)
                .expect("reordered subjects");
        let reordered_ownerships =
            plan_lane_payload_ownership(&reordered_subjects).expect("reordered ownership");
        assert_ne!(
            ownerships[0].payload_ownership_hash,
            reordered_ownerships[0].payload_ownership_hash
        );
        assert_ne!(
            ownerships[0].rbc_instance_hash,
            reordered_ownerships[0].rbc_instance_hash
        );
    }

    #[test]
    fn lane_payload_ownership_is_sorted_independent_of_subject_input_order() {
        let routing = routing_for_lane_dataspaces(&[(1, 11), (2, 22)]);
        let validators = vec![test_peer(1), test_peer(2), test_peer(3)];
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0, 1]),
            &[
                committee(1, 11, validators.clone(), None),
                committee(2, 22, validators, None),
            ],
            "permissioned",
        )
        .expect("lane consensus domains");
        let subjects =
            plan_lane_block_subjects(&domains, &tx_hashes(2), 3, 4).expect("lane block subjects");
        let mut reversed_subjects = subjects.clone();
        reversed_subjects.reverse();

        let ownerships = plan_lane_payload_ownership(&subjects).expect("lane payload ownership");
        let reversed_ownerships =
            plan_lane_payload_ownership(&reversed_subjects).expect("reversed ownership");

        assert_eq!(
            ownerships
                .iter()
                .map(|ownership| ownership.lane_id)
                .collect::<Vec<_>>(),
            vec![LaneId::new(1), LaneId::new(2)]
        );
        assert_eq!(
            ownerships
                .iter()
                .map(|ownership| {
                    (
                        ownership.payload_ownership_hash,
                        ownership.rbc_instance_hash,
                    )
                })
                .collect::<Vec<_>>(),
            reversed_ownerships
                .iter()
                .map(|ownership| {
                    (
                        ownership.payload_ownership_hash,
                        ownership.rbc_instance_hash,
                    )
                })
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn lane_payload_ownership_rejects_malformed_subjects() {
        let routing = routing_for_lane_dataspaces(&[(1, 11)]);
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0]),
            &[committee(
                1,
                11,
                vec![test_peer(1), test_peer(2), test_peer(3)],
                None,
            )],
            "permissioned",
        )
        .expect("lane consensus domains");
        let subjects =
            plan_lane_block_subjects(&domains, &tx_hashes(1), 9, 2).expect("lane block subjects");
        let mut malformed = subjects[0].clone();

        malformed.qc_mode_tag = " ".to_string();
        assert_eq!(
            plan_lane_payload_ownership(&[malformed.clone()]),
            Err(LanePayloadOwnershipError::BlankQcModeTag {
                lane_id: LaneId::new(1),
            })
        );

        malformed = subjects[0].clone();
        malformed.accepted_candidate_indices.clear();
        assert_eq!(
            plan_lane_payload_ownership(&[malformed.clone()]),
            Err(LanePayloadOwnershipError::EmptyCandidateSet {
                lane_id: LaneId::new(1),
            })
        );

        malformed = subjects[0].clone();
        malformed.accepted_transaction_hashes.clear();
        assert_eq!(
            plan_lane_payload_ownership(&[malformed.clone()]),
            Err(LanePayloadOwnershipError::CandidateHashCountMismatch {
                lane_id: LaneId::new(1),
                candidate_indices: 1,
                candidate_hashes: 0,
            })
        );

        malformed = subjects[0].clone();
        malformed.accepted_candidate_indices.push(0);
        malformed.accepted_transaction_hashes.push(tx_hash(0xF0));
        assert_eq!(
            plan_lane_payload_ownership(&[malformed.clone()]),
            Err(LanePayloadOwnershipError::DuplicateCandidateIndex {
                lane_id: LaneId::new(1),
                index: 0,
            })
        );

        malformed = subjects[0].clone();
        malformed.subject_hash = Hash::new(b"tampered lane block subject");
        assert_eq!(
            plan_lane_payload_ownership(&[malformed.clone()]),
            Err(LanePayloadOwnershipError::SubjectHashMismatch {
                lane_id: LaneId::new(1),
                expected: subjects[0].subject_hash,
                actual: malformed.subject_hash,
            })
        );

        assert_eq!(
            plan_lane_payload_ownership(&[subjects[0].clone(), subjects[0].clone()]),
            Err(LanePayloadOwnershipError::DuplicateLaneSlot {
                lane_id: LaneId::new(1),
                dataspace_id: DataSpaceId::new(11),
                lane_block_height: 9,
                lane_block_view: 2,
            })
        );
    }

    #[test]
    fn lane_block_subjects_reject_malformed_domains() {
        let routing = routing_for_lane_dataspaces(&[(1, 11)]);
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0]),
            &[committee(
                1,
                11,
                vec![test_peer(1), test_peer(2), test_peer(3)],
                None,
            )],
            "permissioned",
        )
        .expect("lane consensus domains");
        let mut malformed = domains[0].clone();

        malformed.qc_mode_tag = " ".to_string();
        assert_eq!(
            plan_lane_block_subjects(&[malformed.clone()], &tx_hashes(1), 1, 0),
            Err(LaneBlockSubjectError::BlankQcModeTag {
                lane_id: LaneId::new(1),
            })
        );

        malformed = domains[0].clone();
        malformed.accepted_candidate_indices.clear();
        malformed.accepted_candidates = 0;
        assert_eq!(
            plan_lane_block_subjects(&[malformed.clone()], &tx_hashes(1), 1, 0),
            Err(LaneBlockSubjectError::EmptyCandidateSet {
                lane_id: LaneId::new(1),
            })
        );

        malformed = domains[0].clone();
        malformed.accepted_candidates = 2;
        assert_eq!(
            plan_lane_block_subjects(&[malformed.clone()], &tx_hashes(1), 1, 0),
            Err(LaneBlockSubjectError::CandidateCountMismatch {
                lane_id: LaneId::new(1),
                accepted_candidates: 2,
                candidate_indices: 1,
            })
        );

        malformed = domains[0].clone();
        malformed.accepted_candidate_indices.push(0);
        malformed.accepted_candidates = malformed.accepted_candidate_indices.len();
        assert_eq!(
            plan_lane_block_subjects(&[malformed.clone()], &tx_hashes(1), 1, 0),
            Err(LaneBlockSubjectError::DuplicateCandidateIndex {
                lane_id: LaneId::new(1),
                index: 0,
            })
        );

        assert_eq!(
            plan_lane_block_subjects(&[domains[0].clone()], &[], 1, 0),
            Err(LaneBlockSubjectError::CandidateHashIndexOutOfBounds {
                lane_id: LaneId::new(1),
                index: 0,
                candidate_hashes: 0,
            })
        );

        assert_eq!(
            plan_lane_block_subjects(
                &[domains[0].clone(), domains[0].clone()],
                &tx_hashes(1),
                1,
                0
            ),
            Err(LaneBlockSubjectError::DuplicateLaneDomain {
                lane_id: LaneId::new(1),
            })
        );
    }

    #[test]
    fn lane_consensus_domains_use_explicit_quorum() {
        let routing = routing_for_lane_dataspaces(&[(7, 70)]);
        let validators = vec![test_peer(1), test_peer(2), test_peer(3), test_peer(4)];

        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0]),
            &[committee(7, 70, validators, Some(3))],
            "npos",
        )
        .expect("lane consensus domains");

        assert_eq!(domains[0].quorum.validator_count, 4);
        assert_eq!(domains[0].quorum.min_quorum, 3);
        assert_eq!(
            domains[0].qc_mode_tag,
            "npos::lane-relay:v1:70:7".to_string()
        );
    }

    #[test]
    fn lane_consensus_domains_reject_noncanonical_explicit_quorum() {
        let routing = routing_for_lane_dataspaces(&[(7, 70)]);
        let validators = vec![test_peer(1), test_peer(2), test_peer(3), test_peer(4)];

        for min_quorum in [1, 2, 4] {
            assert_eq!(
                plan_lane_consensus_domains(
                    &routing,
                    &accepted_schedule(&[0]),
                    &[committee(7, 70, validators.clone(), Some(min_quorum))],
                    "npos",
                ),
                Err(LaneConsensusDomainError::InvalidQuorum {
                    lane_id: LaneId::new(7),
                    validator_count: 4,
                    min_quorum,
                }),
                "four-validator lane committees must use canonical 3-of-4 commit quorum"
            );
        }
    }

    #[test]
    fn lane_consensus_domains_ignore_missing_committee_for_deferred_lane() {
        let routing = routing_for_lane_dataspaces(&[(1, 11), (2, 22)]);
        let schedule = mixed_schedule(vec![
            ProposalBatchAction::Accept {
                index: 0,
                exceeds_gas_limit: false,
            },
            ProposalBatchAction::Defer {
                index: 1,
                reason: ProposalDeferralReason::GasLimit,
            },
        ]);

        let domains = plan_lane_consensus_domains(
            &routing,
            &schedule,
            &[committee(1, 11, vec![test_peer(1)], None)],
            "permissioned",
        )
        .expect("lane consensus domains");

        assert_eq!(domains.len(), 1);
        assert_eq!(domains[0].lane_id, LaneId::new(1));
    }

    include!("lane_planner/consensus_domain_rejection_tests.rs");
}
