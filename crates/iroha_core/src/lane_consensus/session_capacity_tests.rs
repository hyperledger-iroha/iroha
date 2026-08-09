    #[test]
    fn lane_block_session_capacity_and_pruning_preserve_commit_locks() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let base = lane_block_proposal_at_height(&validator_set, 13);
        let protected = lane_block_proposal_at_view(&base, 0, 0x40);
        let protected_key = LaneBlockSessionKey::from_proposal(&protected);
        let commit_vote = signed_vote(&protected.vote_body(CertPhase::Commit), &keys[0]);
        let mut cache = LaneBlockSessionCache::new(1);
        assert_eq!(
            cache.insert_proposal(protected.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        for signer in &keys {
            let prepare_vote = signed_vote(&protected.vote_body(CertPhase::Prepare), signer);
            assert_eq!(
                cache.insert_vote(prepare_vote.clone(), Some(&prepare_vote.signer)),
                Ok(LaneBlockSessionInsertOutcome::Inserted)
            );
        }
        assert_eq!(
            cache.insert_vote(commit_vote.clone(), Some(&commit_vote.signer)),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        for view in 1_u64..32 {
            assert_eq!(
                cache.insert_proposal(lane_block_proposal_at_view(
                    &base,
                    view,
                    u8::try_from(view).expect("fixture view fits u8"),
                )),
                Ok(LaneBlockSessionInsertOutcome::Inserted)
            );
        }
        assert!(
            cache.get(&protected_key).is_some(),
            "ordinary capacity eviction must never discard commit evidence"
        );
        assert!(
            cache.len() <= 2,
            "only one ordinary replay session may remain"
        );

        assert!(
            cache.retain_sessions_for_admissible_lanes(|_, _, _, _, _| false) > 0,
            "inactive-route pruning should remove replay state"
        );
        assert!(cache.is_empty());
        let conflicting = lane_block_proposal_at_view(&base, 40, 0xE0);
        assert_eq!(
            cache.insert_proposal(conflicting.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        for signer in &keys {
            let prepare_vote = signed_vote(&conflicting.vote_body(CertPhase::Prepare), signer);
            assert_eq!(
                cache.insert_vote(prepare_vote.clone(), Some(&prepare_vote.signer)),
                Ok(LaneBlockSessionInsertOutcome::Inserted)
            );
        }
        let conflicting_vote = signed_vote(&conflicting.vote_body(CertPhase::Commit), &keys[0]);
        assert_eq!(
            cache.insert_vote(conflicting_vote, None),
            Err(LaneBlockSessionError::ConflictingVote),
            "the signer commit lock must outlive pruned session state"
        );
    }

    #[test]
    fn drained_committed_sessions_retire_under_capacity_but_keep_signer_lock() {
        let keys = [checked_bls_keypair(1), checked_bls_keypair(2)];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let pops = signer_pops(&keys);
        let mut cache = LaneBlockSessionCache::new(1);
        let first = lane_block_proposal_at_height(&validator_set, 13);
        let first_key = LaneBlockSessionKey::from_proposal(&first);

        for lane_height in 13_u64..45 {
            let proposal = lane_block_proposal_at_height(&validator_set, lane_height);
            assert_eq!(
                cache.insert_proposal(proposal.clone()),
                Ok(LaneBlockSessionInsertOutcome::Inserted)
            );
            for phase in [CertPhase::Prepare, CertPhase::Commit] {
                let body = proposal.vote_body(phase);
                let votes = [signed_vote(&body, &keys[0]), signed_vote(&body, &keys[1])];
                let qc = aggregate_lane_block_votes_to_qc(body, validator_set.clone(), &votes)
                    .expect("lane block QC");
                assert_eq!(
                    cache.insert_qc_with_pops(qc, &pops),
                    Ok(LaneBlockSessionInsertOutcome::Inserted)
                );
            }
            assert_eq!(
                cache.drain_committed_sessions_up_to(1).len(),
                1,
                "each certified lane session should hand off once"
            );
            assert!(
                cache.len() <= 1,
                "drained commit evidence must return under the ordinary cache bound"
            );
        }
        assert!(
            cache.get(&first_key).is_none(),
            "the oldest drained session should be retired under sustained progress"
        );

        let conflicting = lane_block_proposal_at_view(&first, 99, 0xF0);
        assert_eq!(
            cache.insert_proposal(conflicting.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        for signer in &keys[..2] {
            let prepare_vote = signed_vote(&conflicting.vote_body(CertPhase::Prepare), signer);
            assert_eq!(
                cache.insert_vote(prepare_vote.clone(), Some(&prepare_vote.signer)),
                Ok(LaneBlockSessionInsertOutcome::Inserted)
            );
        }
        let conflicting_vote = signed_vote(&conflicting.vote_body(CertPhase::Commit), &keys[0]);
        assert_eq!(
            cache.insert_vote(conflicting_vote, None),
            Err(LaneBlockSessionError::ConflictingVote),
            "retiring drained replay state must not retire the signer commit lock"
        );
        assert!(cache.commit_vote_lock_len() > 0);
        assert!(
            cache.prune_sessions_and_commit_vote_locks_for_finalized_slots(
                |lane_id, dataspace_id, _lane_incarnation, lane_block_height| {
                    lane_id == first.descriptor.lane_id
                        && dataspace_id == first.descriptor.dataspace_id
                        && lane_block_height <= 44
                },
            ) > 0
        );
        assert!(cache.is_empty());
        assert_eq!(
            cache.commit_vote_lock_len(),
            0,
            "only an explicit durable boundary should retire historical signer locks"
        );
    }

    #[test]
    fn durable_slot_retirement_covers_only_finalized_lane_heights() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let mut cache = LaneBlockSessionCache::new(8);
        for lane_height in 13_u64..=15 {
            let proposal = lane_block_proposal_at_height(&validator_set, lane_height);
            assert_eq!(
                cache.insert_proposal(proposal.clone()),
                Ok(LaneBlockSessionInsertOutcome::Inserted)
            );
            for signer in &keys {
                let prepare_vote = signed_vote(&proposal.vote_body(CertPhase::Prepare), signer);
                assert_eq!(
                    cache.insert_vote(prepare_vote.clone(), Some(&prepare_vote.signer)),
                    Ok(LaneBlockSessionInsertOutcome::Inserted)
                );
            }
            let vote = signed_vote(&proposal.vote_body(CertPhase::Commit), &keys[0]);
            assert_eq!(
                cache.insert_vote(vote.clone(), Some(&vote.signer)),
                Ok(LaneBlockSessionInsertOutcome::Inserted)
            );
        }
        assert_eq!(cache.commit_vote_lock_slots().len(), 3);

        assert_eq!(
            cache.prune_sessions_and_commit_vote_locks_for_finalized_slots(
                |lane_id, dataspace_id, _lane_incarnation, lane_block_height| {
                    lane_id == LaneId::new(7)
                        && dataspace_id == DataSpaceId::new(11)
                        && lane_block_height <= 14
                },
            ),
            4,
            "two sessions and their two signer locks should retire atomically"
        );
        assert_eq!(cache.len(), 1);
        assert_eq!(
            cache.commit_vote_lock_slots(),
            BTreeSet::from([(
                LaneId::new(7),
                DataSpaceId::new(11),
                lane_block_proposal_at_height(&validator_set, 15)
                    .descriptor
                    .lane_incarnation,
                15,
            )]),
            "the higher unfinalized slot must remain protected"
        );
    }
