#[test]
fn applied_proposal_retirement_preserves_unselected_evidence_and_capacity() {
    let (keys, validators) = lane_block_validator_fixture(4);
    let applied = lane_block_proposal_at_height(&validators, 13);
    let mut other_incarnation = retag_lane_block_proposal_payload(applied.clone(), 0xB0);
    other_incarnation.descriptor.lane_incarnation = Hash::new(b"unselected lane incarnation");
    other_incarnation.descriptor.descriptor_hash =
        other_incarnation.descriptor.computed_descriptor_hash();
    other_incarnation.proposal_hash = other_incarnation.computed_proposal_hash();
    let unresolved = lane_block_proposal_at_height(&validators, 14);
    let ordinary = lane_block_proposal_at_height(&validators, 15);
    let ordinary_higher = retarget_lane_block_proposal_exact_view(&ordinary, 3)
        .expect("another view preserves the same executable payload");
    let mut cache = LaneBlockSessionCache::new(3);
    for proposal in [&applied, &other_incarnation] {
        cache
            .insert_proposal(proposal.clone())
            .expect("cache proposal");
        for phase in [CertPhase::Prepare, CertPhase::Commit] {
            let body = proposal.vote_body(phase);
            let votes = keys[..3]
                .iter()
                .map(|key| signed_vote(&body, key))
                .collect::<Vec<_>>();
            let qc = aggregate_lane_block_votes_to_qc(body, validators.clone(), &votes)
                .expect("exact three-of-four certificate");
            cache
                .insert_qc_with_pops(qc, &signer_pops(&keys))
                .expect("protect authenticated Commit evidence");
        }
    }
    for proposal in [&unresolved, &ordinary_higher, &ordinary] {
        cache
            .insert_proposal(proposal.clone())
            .expect("fill ordinary capacity");
    }
    let original = cache.clone();
    let applied_key = LaneBlockSessionKey::from_proposal(&applied);
    assert_eq!(cache.retained_vote_bodies().len(), 5);
    assert_eq!(
        cache.retire_applied_proposals(&[applied.clone(), applied.clone()]),
        Ok(4),
        "one selected session and its three signer locks retire once"
    );
    assert_eq!(cache.capacity, 3);
    assert_eq!(cache.len(), 4);
    assert!(!cache.contains_proposal(&applied));
    for proposal in [&other_incarnation, &unresolved, &ordinary_higher, &ordinary] {
        let key = LaneBlockSessionKey::from_proposal(proposal);
        assert_eq!(cache.get(&key), original.get(&key));
    }
    assert_eq!(
        cache.order,
        original
            .order
            .iter()
            .copied()
            .filter(|key| key != &applied_key)
            .collect::<VecDeque<_>>()
    );
    assert_eq!(cache.commit_vote_lock_len(), 3);
    assert!(cache.commit_vote_locks.keys().all(|(slot, _)| {
        slot.lane_incarnation == other_incarnation.descriptor.lane_incarnation
    }));
    assert!(
        applied
            .descriptor
            .accepted_transaction_hashes
            .iter()
            .all(|hash| { !cache.entrypoint_claims.contains_key(hash) })
    );
    for hash in &ordinary.descriptor.accepted_transaction_hashes {
        assert_eq!(
            original.entrypoint_claims.get(hash),
            Some(&LaneBlockSessionKey::from_proposal(&ordinary_higher)),
            "the first inserted higher view owns this unrelated shared claim"
        );
        assert_eq!(
            cache.entrypoint_claims.get(hash),
            original.entrypoint_claims.get(hash)
        );
    }
    let retired = cache.clone();
    assert_eq!(cache.retire_applied_proposals(&[applied]), Ok(0));
    assert_eq!(cache, retired);
    let fresh = lane_block_proposal_at_height(&validators, 16);
    cache
        .insert_proposal(fresh.clone())
        .expect("reuse released ordinary capacity");
    assert!(!cache.contains_proposal(&unresolved));
    assert!(cache.contains_proposal(&ordinary));
    assert!(cache.contains_proposal(&ordinary_higher));
    assert!(cache.contains_proposal(&other_incarnation));
    assert!(cache.contains_proposal(&fresh));
    assert_eq!(
        cache.len(),
        4,
        "three ordinary slots plus the unselected protected owner"
    );
}

#[test]
fn applied_proposal_retirement_preflights_all_selected_quorums() {
    let (keys, validators) = lane_block_validator_fixture(4);
    let first = lane_block_proposal_at_height(&validators, 13);
    let canonical = lane_block_proposal_at_height(&validators, 14);
    let conflicting = retag_lane_block_proposal_payload(canonical.clone(), 0xB1);
    for with_proposal in [false, true] {
        for with_commit in [false, true] {
            let mut cache = LaneBlockSessionCache::new(2);
            cache
                .insert_proposal(first.clone())
                .expect("retain earlier selected input");
            if with_proposal {
                cache
                    .insert_proposal(conflicting.clone())
                    .expect("optional proposal shell");
            }
            for phase in [CertPhase::Prepare, CertPhase::Commit] {
                if phase == CertPhase::Commit && !with_commit {
                    break;
                }
                let body = conflicting.vote_body(phase);
                let votes = keys[..3]
                    .iter()
                    .map(|key| signed_vote(&body, key))
                    .collect::<Vec<_>>();
                let qc = aggregate_lane_block_votes_to_qc(body, validators.clone(), &votes)
                    .expect("exact three-of-four conflicting proof");
                cache
                    .insert_qc_with_pops(qc, &signer_pops(&keys))
                    .expect("retain verified proof");
            }
            let original = cache.clone();
            assert_eq!(
                cache.retire_applied_proposals(&[first.clone(), canonical.clone()]),
                Err(LaneBlockSessionError::ConflictingProposal)
            );
            assert_eq!(
                cache, original,
                "later conflict must prevent every earlier retirement"
            );
            assert_eq!(
                cache.retain_canonical_rollover_evidence(
                    2,
                    |_, height| [first.clone(), canonical.clone()]
                        .into_iter()
                        .find(|proposal| proposal.descriptor.lane_block_height == height),
                    |_, _, _, _| true,
                    |_, _, _, _| false,
                ),
                Err(LaneBlockSessionError::ConflictingProposal),
                "rollover must preserve the shared conflict policy even for finalized slots"
            );
            assert_eq!(cache, original);
        }
    }
}

#[test]
fn applied_proposal_retirement_revalidates_orphan_commit_lock_quorums() {
    let (keys, validators) = lane_block_validator_fixture(4);
    let canonical = lane_block_proposal_at_height(&validators, 13);
    let conflicting = retag_lane_block_proposal_payload(canonical.clone(), 0xB2);
    for commit_signers in [2, 3] {
        let mut cache = LaneBlockSessionCache::new(1);
        cache
            .insert_proposal(conflicting.clone())
            .expect("retain signed proposal");
        for key in &keys[..3] {
            cache
                .insert_vote(
                    signed_vote(&conflicting.vote_body(CertPhase::Prepare), key),
                    None,
                )
                .expect("form exact three-of-four PrepareQC");
        }
        for key in &keys[..commit_signers] {
            cache
                .insert_vote(
                    signed_vote(&conflicting.vote_body(CertPhase::Commit), key),
                    None,
                )
                .expect("retain authenticated independent Commit lock");
        }
        cache.retain_sessions_for_admissible_lanes(|_, _, _, _, _| false);
        assert!(cache.is_empty());
        assert!(cache.retained_vote_bodies().is_empty());
        assert_eq!(
            cache.rollover_slots(),
            BTreeSet::from([(canonical.descriptor.lane_id, 13)])
        );
        let original = cache.clone();
        if commit_signers == 3 {
            assert_eq!(
                cache.retire_applied_proposals(&[canonical.clone()]),
                Err(LaneBlockSessionError::ConflictingProposal)
            );
            assert_eq!(cache, original);
            assert_eq!(
                cache.retain_canonical_rollover_evidence(
                    1,
                    |_, _| Some(canonical.clone()),
                    |_, _, _, _| true,
                    |_, _, _, _| false
                ),
                Err(LaneBlockSessionError::ConflictingProposal)
            );
            assert_eq!(cache, original);
        } else {
            assert_eq!(cache.retire_applied_proposals(&[canonical.clone()]), Ok(2));
            assert!(cache.rollover_slots().is_empty());
            assert_eq!(cache.capacity, 1);
        }
    }
}

#[test]
fn applied_proposal_retirement_rejects_invalid_or_conflicting_target_sets() {
    let (_keys, validators) = lane_block_validator_fixture(4);
    let first = lane_block_proposal_at_height(&validators, 13);
    let mut invalid = lane_block_proposal_at_height(&validators, 14);
    invalid.descriptor.descriptor_hash = Hash::new(b"invalid applied target");
    let mut cache = LaneBlockSessionCache::new(1);
    cache
        .insert_proposal(first.clone())
        .expect("retain earlier valid target");
    let original = cache.clone();
    assert!(matches!(
        cache.retire_applied_proposals(&[first.clone(), invalid]),
        Err(LaneBlockSessionError::InvalidProposal(_))
    ));
    assert_eq!(cache, original);
    let conflicting = retag_lane_block_proposal_payload(first.clone(), 0xB3);
    assert_eq!(
        cache.retire_applied_proposals(&[first, conflicting]),
        Err(LaneBlockSessionError::ConflictingProposal)
    );
    assert_eq!(cache, original);
    assert_eq!(cache.retire_applied_proposals(&[]), Ok(0));
    assert_eq!(cache, original);
}

#[test]
fn retained_vote_body_inventory_covers_partial_owners_without_mutation() {
    let (keys, validators) = lane_block_validator_fixture(4);
    let proposals = (13..17)
        .map(|height| lane_block_proposal_at_height(&validators, height))
        .collect::<Vec<_>>();
    let mut cache = LaneBlockSessionCache::new(4);
    cache
        .insert_proposal(proposals[0].clone())
        .expect("proposal-only owner");
    let qc_body = proposals[1].vote_body(CertPhase::Prepare);
    let qc_votes = keys[..3]
        .iter()
        .map(|key| signed_vote(&qc_body, key))
        .collect::<Vec<_>>();
    cache
        .insert_qc_with_pops(
            aggregate_lane_block_votes_to_qc(qc_body.clone(), validators, &qc_votes)
                .expect("three-of-four QC"),
            &signer_pops(&keys),
        )
        .expect("proposal-less QC owner");
    let prepare = signed_vote(&proposals[2].vote_body(CertPhase::Prepare), &keys[0]);
    cache
        .insert_vote(prepare.clone(), None)
        .expect("proposal-less Prepare vote owner");
    // The projection must also preserve the actual height of a retained raw
    // Commit body; it does not manufacture a proposal or authenticate a session.
    let commit = signed_vote(&proposals[3].vote_body(CertPhase::Commit), &keys[0]);
    let key = LaneBlockSessionKey::from_vote_body(&commit.body);
    cache.sessions.insert(
        key,
        LaneBlockSession {
            commit_votes: BTreeMap::from([(commit.signer.clone(), commit.clone())]),
            ..LaneBlockSession::default()
        },
    );
    let original = cache.clone();
    assert_eq!(
        cache.retained_vote_bodies(),
        vec![
            proposals[0].vote_body(CertPhase::Prepare),
            qc_body,
            prepare.body,
            commit.body
        ]
    );
    assert_eq!(cache, original);
    assert_eq!(cache.rollover_slots().len(), 4);
}

#[test]
fn recovered_proposal_batch_is_idempotent_and_replaces_uncertified_slot_at_capacity() {
    let (_keys, validators) = lane_block_validator_fixture(4);
    let canonical = lane_block_proposal_at_height(&validators, 13);
    let conflicting = retag_lane_block_proposal_payload(canonical.clone(), 0xA0);
    let mut cache = LaneBlockSessionCache::new(1);
    cache
        .insert_proposal(conflicting.clone())
        .expect("fill the one recovery slot");
    cache
        .insert_recovered_proposals(&[canonical.clone(), canonical.clone()])
        .expect("the unique canonical replacement fits at capacity");
    assert_eq!(cache.len(), 1);
    assert!(cache.contains_proposal(&canonical));
    assert!(!cache.contains_proposal(&conflicting));
    let exact = cache.clone();
    cache
        .insert_recovered_proposals(&[canonical.clone(), canonical])
        .expect("duplicate recovery consumes no additional slot");
    assert_eq!(cache, exact);
}

#[test]
fn recovered_proposal_batch_preserves_required_history_and_commit_evidence() {
    let (keys, validators) = lane_block_validator_fixture(4);
    let protected = lane_block_proposal_at_height(&validators, 15);
    let historical = lane_block_proposal_at_height(&validators, 13);
    let unrelated = lane_block_proposal_at_height(&validators, 14);
    let recovered = lane_block_proposal_at_height(&validators, 16);
    let mut cache = LaneBlockSessionCache::new(2);
    cache
        .insert_proposal(protected.clone())
        .expect("install protected proposal");
    for phase in [CertPhase::Prepare, CertPhase::Commit] {
        let body = protected.vote_body(phase);
        let votes = keys[..3]
            .iter()
            .map(|key| signed_vote(&body, key))
            .collect::<Vec<_>>();
        let qc = aggregate_lane_block_votes_to_qc(body, validators.clone(), &votes)
            .expect("exact three-of-four quorum");
        cache
            .insert_qc_with_pops(qc, &signer_pops(&keys))
            .expect("retain verified quorum");
    }
    cache
        .insert_proposal(historical.clone())
        .expect("retain oldest required history");
    cache
        .insert_proposal(unrelated.clone())
        .expect("fill ordinary capacity with speculative work");
    let protected_session = cache
        .get(&LaneBlockSessionKey::from_proposal(&protected))
        .cloned();
    let signer_locks = cache.commit_vote_locks.clone();
    cache
        .insert_recovered_proposals(&[recovered.clone(), historical.clone()])
        .expect("required history and new canonical input fit independently of Commit evidence");
    assert_eq!(
        cache.len(),
        3,
        "two ordinary required sources plus protected Commit evidence"
    );
    assert!(cache.contains_proposal(&historical));
    assert!(cache.contains_proposal(&recovered));
    assert!(!cache.contains_proposal(&unrelated));
    assert_eq!(
        cache
            .get(&LaneBlockSessionKey::from_proposal(&protected))
            .cloned(),
        protected_session
    );
    assert_eq!(cache.commit_vote_locks, signer_locks);
    let exact = cache.clone();
    cache
        .insert_recovered_proposals(&[recovered, historical])
        .expect("the same required batch retains its exact recency order");
    assert_eq!(cache, exact);
}

#[test]
fn recovered_proposal_batch_preflights_later_quorum_before_any_eviction() {
    let (keys, validators) = lane_block_validator_fixture(4);
    let earlier = lane_block_proposal_at_height(&validators, 13);
    let certified = lane_block_proposal_at_height(&validators, 15);
    let conflicting = retag_lane_block_proposal_payload(certified.clone(), 0xA1);
    let unrelated = lane_block_proposal_at_height(&validators, 16);
    for with_proposal in [false, true] {
        for with_commit in [false, true] {
            let mut cache = LaneBlockSessionCache::new(2);
            if with_proposal {
                cache
                    .insert_proposal(certified.clone())
                    .expect("install optional quorum proposal shell");
            }
            for phase in [CertPhase::Prepare, CertPhase::Commit] {
                if phase == CertPhase::Commit && !with_commit {
                    break;
                }
                let body = certified.vote_body(phase);
                let votes = keys[..3]
                    .iter()
                    .map(|key| signed_vote(&body, key))
                    .collect::<Vec<_>>();
                let qc = aggregate_lane_block_votes_to_qc(body, validators.clone(), &votes)
                    .expect("exact three-of-four quorum");
                cache
                    .insert_qc_with_pops(qc, &signer_pops(&keys))
                    .expect("retain verified original quorum");
            }
            cache
                .insert_proposal(unrelated.clone())
                .expect("make the quorum the oldest retained session");
            let original = cache.clone();
            assert_eq!(
                cache.insert_recovered_proposals(&[earlier.clone(), conflicting.clone()]),
                Err(LaneBlockSessionError::ConflictingProposal),
                "preflight must see the later slot's original quorum before an earlier insertion can evict its Prepare-only evidence"
            );
            assert_eq!(
                cache, original,
                "rejected recovery must publish no partial cache or signer-lock changes"
            );
        }
    }
}

#[test]
fn recovered_proposal_batch_rejects_required_union_over_capacity() {
    let (_keys, validators) = lane_block_validator_fixture(4);
    let first = lane_block_proposal_at_height(&validators, 13);
    let second = lane_block_proposal_at_height(&validators, 14);
    let mut cache = LaneBlockSessionCache::new(1);
    cache
        .insert_proposal(first.clone())
        .expect("retain existing canonical source");
    let original = cache.clone();
    assert_eq!(
        cache.insert_recovered_proposals(&[first.clone(), second.clone()]),
        Err(LaneBlockSessionError::RecoveryCapacityExceeded)
    );
    assert_eq!(cache, original);
    let mut invalid = second;
    invalid.descriptor.descriptor_hash = Hash::new(b"invalid recovery descriptor");
    assert!(matches!(
        cache.insert_recovered_proposals(&[first.clone(), invalid]),
        Err(LaneBlockSessionError::InvalidProposal(_))
    ));
    assert_eq!(cache, original);
    let conflict = retag_lane_block_proposal_payload(first.clone(), 0xA2);
    assert_eq!(
        cache.insert_recovered_proposals(&[first, conflict]),
        Err(LaneBlockSessionError::ConflictingProposal)
    );
    assert_eq!(cache, original);
}

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
