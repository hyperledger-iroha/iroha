    #[test]
    fn historical_alternative_qc_uses_full_frozen_finality_roster_pops() {
        let (adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let (block, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
        adapter
            .kura
            .store_block(block.clone())
            .expect("persist historical lane carrier");
        let finality = verified_finality_artifact_for_block(&adapter, &keys, &block);
        adapter
            .kura
            .store_v2_finality_artifact(&finality)
            .expect("persist historical lane frozen roster");

        let retained = CommittedLaneBlockSession {
            proposal: proposal.clone(),
            prepare_qc: lane_qc_for_phase(&proposal, &keys[..3], CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&proposal, &keys[..3], CertPhase::Commit),
        };
        adapter
            .kura
            .persist_committed_lane_block_session(&retained, &lane_signer_pops(&keys[..3]))
            .expect("persist one exact 3-of-4 lane certificate");

        let alternative = lane_qc_for_phase(&proposal, &keys[1..], CertPhase::Commit);
        assert_ne!(
            retained.commit_qc.signers_bitmap, alternative.signers_bitmap,
            "fixture must select the validator omitted by the retained QC"
        );
        let durable = adapter
            .kura
            .read_certified_lane_block_artifact(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
            )
            .expect("read retained historical lane certificate");
        assert!(
            !durable.signer_pops.contains_key(keys[3].public_key()),
            "the retained certificate must contain only its exact signer union"
        );

        assert!(
            durable_historical_lane_output_source_hash(
                adapter.kura.as_ref(),
                &BlockMessage::LaneBlockQc(alternative.clone()),
            )
            .expect("verify alternate historical QC against frozen finality PoPs")
            .is_some(),
            "a valid alternate 3-of-4 QC for the same body must remain reconstructible"
        );

        let mut forged = alternative;
        forged.bls_aggregate_signature[0] ^= 0x80;
        assert!(
            durable_historical_lane_output_source_hash(
                adapter.kura.as_ref(),
                &BlockMessage::LaneBlockQc(forged),
            )
            .is_err(),
            "the complete frozen PoP source must not weaken aggregate validation"
        );
    }

    #[test]
    fn current_height_qc_uses_frozen_context_pops_after_state_index_pruning() {
        let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let (block, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
        adapter
            .kura
            .store_block(block.clone())
            .expect("persist current-height lane carrier");
        let committed = ValidBlock::committed_from_replay_signed_block(block);
        commit_test_block_to_state(adapter.state.as_ref(), &committed, &adapter.context);
        let session = CommittedLaneBlockSession {
            proposal: proposal.clone(),
            prepare_qc: lane_qc_for_phase(&proposal, &keys[..3], CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&proposal, &keys[..3], CertPhase::Commit),
        };
        adapter.pending_committed_lanes.push_back(session);

        {
            let mut world = adapter.state.world.block();
            for key in &keys {
                world
                    .consensus_keys_by_pk
                    .insert(key.public_key().to_string(), Vec::new());
            }
            world.commit();
        }
        {
            let world = adapter.state.world_view();
            assert!(keys.iter().all(|key| {
                crate::state::live_consensus_key_pop_for_peer(
                    &world,
                    &PeerId::new(key.public_key().clone()),
                    adapter.context.height,
                )
                .is_none()
            }));
        }

        assert_eq!(
            adapter
                .persist_anchored_sessions()
                .expect("persist with the authenticated frozen context PoPs"),
            1
        );
        let durable = adapter
            .kura
            .read_certified_lane_block_artifact(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
            )
            .expect("read certificate persisted after mutable index pruning");
        assert_eq!(durable.signer_pops, lane_signer_pops(&keys[..3]));
    }
