// Lane-incarnation namespace coverage for durable commit-vote locks.
//
// Included by `lane_consensus::tests` to preserve the exact libtest name.

#[test]
fn commit_vote_locks_are_namespaced_by_lane_incarnation() {
    let keys = [
        checked_bls_keypair(1),
        checked_bls_keypair(2),
        checked_bls_keypair(3),
    ];
    let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
    validator_set.sort();
    let mut cache = LaneBlockSessionCache::new(8);

    let original = lane_block_proposal_at_height(&validator_set, 13);
    assert_eq!(
        cache.insert_proposal(original.clone()),
        Ok(LaneBlockSessionInsertOutcome::Inserted)
    );
    for signer in &keys {
        let prepare = signed_vote(&original.vote_body(CertPhase::Prepare), signer);
        assert_eq!(
            cache.insert_vote(prepare.clone(), Some(&prepare.signer)),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
    }
    let original_commit = signed_vote(&original.vote_body(CertPhase::Commit), &keys[0]);
    assert_eq!(
        cache.insert_vote(original_commit.clone(), Some(&original_commit.signer)),
        Ok(LaneBlockSessionInsertOutcome::Inserted)
    );

    let recreated_incarnation = Hash::new(b"recreated-commit-lock-incarnation");
    let mut recreated = retag_lane_block_proposal_payload(original, 0xD7);
    recreated.descriptor.lane_incarnation = recreated_incarnation;
    recreated.descriptor.descriptor_hash = recreated.descriptor.computed_descriptor_hash();
    recreated.proposal_hash = recreated.computed_proposal_hash();
    assert_eq!(
        cache.insert_proposal(recreated.clone()),
        Ok(LaneBlockSessionInsertOutcome::Inserted),
        "a recreated lane must own a distinct local-height namespace"
    );
    for signer in &keys {
        let prepare = signed_vote(&recreated.vote_body(CertPhase::Prepare), signer);
        assert_eq!(
            cache.insert_vote(prepare.clone(), Some(&prepare.signer)),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
    }
    let recreated_commit = signed_vote(&recreated.vote_body(CertPhase::Commit), &keys[0]);
    assert_eq!(
        cache.insert_vote(recreated_commit.clone(), Some(&recreated_commit.signer)),
        Ok(LaneBlockSessionInsertOutcome::Inserted),
        "an old-incarnation signer lock must not block the recreated lane"
    );
    assert_eq!(cache.commit_vote_lock_len(), 2);

    assert_eq!(
        cache.prune_commit_vote_locks_for_inactive_incarnations(
            |_lane_id, _dataspace_id, incarnation| incarnation == recreated_incarnation,
        ),
        1,
        "retiring an incarnation should remove only its obsolete signer lock"
    );
    assert_eq!(
        cache.commit_vote_lock_slots(),
        BTreeSet::from([(
            recreated.descriptor.lane_id,
            recreated.descriptor.dataspace_id,
            recreated_incarnation,
            recreated.descriptor.lane_block_height,
        )])
    );
}
