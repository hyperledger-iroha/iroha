fn ordinary_lane_signing_fixture() -> (V2LaneWorkAdapter, Vec<KeyPair>, LaneBlockProposalV1) {
    let (adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let incarnation = adapter
        .state
        .lane_incarnation_at_height(LaneId::SINGLE, adapter.context.height)
        .expect("active signing lane incarnation");
    let proposal = proposal_for_route(
        &adapter,
        &keys,
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
        incarnation,
        adapter.context.height,
        1,
    );
    assert!(
        proposal
            .descriptor
            .validator_set
            .contains(&adapter.local_peer)
    );
    assert!(adapter.proposal_predecessor_is_ready_for_progress(&proposal));
    (adapter, keys, proposal)
}

#[test]
fn exact_lane_members_sign_both_phases_with_their_bound_key() {
    for phase in [CertPhase::Prepare, CertPhase::Commit] {
        let (mut adapter, _, proposal) = ordinary_lane_signing_fixture();
        let vote = adapter
            .sign_lane_vote(&proposal, phase)
            .expect("authorized signing succeeds")
            .expect("exact member emits a vote");
        assert_eq!(vote.signer, adapter.local_peer);
        assert_eq!(vote.body, proposal.vote_body(phase));
        Signature::try_from_bytes(&vote.bls_signature)
            .expect("canonical signature")
            .verify(
                adapter.local_peer.public_key(),
                &vote.body.signature_preimage(),
            )
            .expect("signature verifies against the bound local identity");
    }
}

#[test]
fn lane_nonmembers_cannot_sign_or_consume_ready_authority() {
    for phase in [CertPhase::Prepare, CertPhase::Commit] {
        let (mut adapter, _, proposal) = ordinary_lane_signing_fixture();
        let outsider = KeyPair::try_from_seed(vec![0xFA; 32], Algorithm::BlsNormal)
            .expect("deterministic nonmember key");
        adapter.local_peer = PeerId::new(outsider.public_key().clone());
        adapter.key_pair = outsider;
        let before = adapter.lane_ready_authorizations.len();
        assert!(
            adapter
                .sign_lane_vote(&proposal, phase)
                .expect("nonmember is ineligible")
                .is_none()
        );
        assert_eq!(adapter.lane_ready_authorizations.len(), before);
        assert!(!adapter.output_guard.restart_required());
    }
}

#[test]
fn lane_signing_identity_mismatch_fails_before_signature_or_ready_consumption() {
    for phase in [CertPhase::Prepare, CertPhase::Commit] {
        let (mut adapter, _, proposal) = ordinary_lane_signing_fixture();
        adapter.key_pair = KeyPair::try_from_seed(vec![0xFB; 32], Algorithm::BlsNormal)
            .expect("deterministic mismatched signer key");
        let before = adapter.lane_ready_authorizations.len();
        assert!(matches!(
            adapter.sign_lane_vote(&proposal, phase),
            Err(V2LaneWorkError::SigningGuard(_))
        ));
        assert!(adapter.output_guard.restart_required());
        assert_eq!(adapter.lane_ready_authorizations.len(), before);
    }
}

#[test]
fn closed_output_guard_prevents_both_lane_signature_phases() {
    for phase in [CertPhase::Prepare, CertPhase::Commit] {
        let (mut adapter, _, proposal) = ordinary_lane_signing_fixture();
        adapter.output_guard.close_admission_for_restart();
        assert!(
            adapter
                .sign_lane_vote(&proposal, phase)
                .expect("closed output is ineligible")
                .is_none()
        );
    }
}

#[test]
fn lane_signing_rejects_a_self_consistent_substituted_committee() {
    for phase in [CertPhase::Prepare, CertPhase::Commit] {
        let (mut adapter, keys, original) = ordinary_lane_signing_fixture();
        let outsider = KeyPair::try_from_seed(vec![0xFC; 32], Algorithm::BlsNormal)
            .expect("deterministic substituted committee member");
        let mut replaced = false;
        let mut foreign_keys = keys
            .into_iter()
            .map(|key| {
                if !replaced && key.public_key() != adapter.local_peer.public_key() {
                    replaced = true;
                    outsider.clone()
                } else {
                    key
                }
            })
            .collect::<Vec<_>>();
        foreign_keys.sort_by_key(|key| PeerId::new(key.public_key().clone()));
        let proposal = proposal_for_route(
            &adapter,
            &foreign_keys,
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
            original.descriptor.lane_incarnation,
            adapter.context.height,
            1,
        );
        assert!(
            proposal
                .descriptor
                .validator_set
                .contains(&adapter.local_peer)
        );
        validate_lane_block_proposal(&proposal)
            .expect("substituted descriptor is internally well formed");
        assert_ne!(
            proposal.descriptor.validator_set,
            original.descriptor.validator_set
        );
        assert!(
            adapter
                .sign_lane_vote(&proposal, phase)
                .expect("foreign committee is ineligible")
                .is_none()
        );
        assert!(!adapter.output_guard.restart_required());
    }
}
