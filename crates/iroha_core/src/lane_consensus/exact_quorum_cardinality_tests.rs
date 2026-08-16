// Exact first-release autonomous-lane certificate cardinality tests.

#[test]
fn lane_drain_certificate_aggregates_exact_quorum_and_verifies_after_restart() {
    let keys = [
        checked_bls_keypair(101),
        checked_bls_keypair(102),
        checked_bls_keypair(103),
        checked_bls_keypair(104),
    ];
    let (body, validator_set) = lane_drain_fixture(&keys);
    let votes = keys[..3]
        .iter()
        .map(|keypair| {
            LaneDrainVoteV1::new_signed(body.clone(), peer(keypair), keypair.private_key())
                .expect("valid drain vote")
        })
        .collect::<Vec<_>>();
    let certificate = aggregate_lane_drain_votes(body.clone(), validator_set.clone(), &votes)
        .expect("valid drain certificate");
    validate_lane_drain_certificate(&certificate)
        .expect("self-contained certificate verifies after restart");
    assert_eq!(certificate.body, body);
    assert_eq!(certificate.validator_set, validator_set);
    assert_eq!(certificate.signer_proofs.len(), 3);
    assert_eq!(
        certificate
            .signers_bitmap
            .iter()
            .map(|byte| byte.count_ones())
            .sum::<u32>(),
        3
    );
    let encoded = certificate.encode();
    let decoded = LaneDrainCertificateV1::decode(&mut encoded.as_slice())
        .expect("drain certificate round-trips");
    validate_lane_drain_certificate(&decoded)
        .expect("round-tripped drain certificate verifies");
}

#[test]
fn aggregate_lane_block_votes_builds_sorted_bitmap_and_signature() {
    let keys = [
        checked_bls_keypair(1),
        checked_bls_keypair(2),
        checked_bls_keypair(3),
        checked_bls_keypair(4),
    ];
    let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
    validator_set.sort();
    let body = vote_body(&validator_set);
    let vote_a = signed_vote(&body, &keys[0]);
    let vote_c = signed_vote(&body, &keys[2]);
    let vote_d = signed_vote(&body, &keys[3]);
    let qc = aggregate_lane_block_votes_to_qc(
        body.clone(),
        validator_set.clone(),
        &[vote_c.clone(), vote_a.clone(), vote_d.clone()],
    )
    .expect("lane block QC");
    let expected_signer_indices = [vote_a.signer, vote_c.signer, vote_d.signer]
        .into_iter()
        .map(|signer| {
            validator_set
                .iter()
                .position(|validator| validator == &signer)
                .expect("signer in validator set")
        })
        .collect::<Vec<_>>();
    let mut expected_bitmap = vec![0_u8; validator_set.len().div_ceil(8)];
    for index in expected_signer_indices {
        expected_bitmap[index / 8] |= 1_u8 << (index % 8);
    }
    assert_eq!(qc.signers_bitmap, expected_bitmap);
    assert_eq!(qc.body, body);
    assert_eq!(qc.validator_set_hash, HashOf::new(&validator_set));
    assert!(!qc.bls_aggregate_signature.is_empty());
}

fn selected_signer_pops(
    validator_set: &[PeerId],
    signers_bitmap: &[u8],
    keypairs: &[KeyPair],
) -> BTreeMap<PublicKey, Vec<u8>> {
    validator_set
        .iter()
        .enumerate()
        .filter(|(index, _)| {
            signers_bitmap
                .get(index / 8)
                .is_some_and(|byte| byte & (1_u8 << (index % 8)) != 0)
        })
        .map(|(_, validator)| {
            let keypair = keypairs
                .iter()
                .find(|keypair| keypair.public_key() == validator.public_key())
                .expect("selected fixture validator keypair");
            (
                validator.public_key().clone(),
                bls_normal_pop_prove(keypair.private_key()).expect("selected validator PoP"),
            )
        })
        .collect()
}

#[test]
fn drain_certificate_projects_four_votes_and_rejects_four_signers_on_wire() {
    let keys = [
        checked_bls_keypair(101),
        checked_bls_keypair(102),
        checked_bls_keypair(103),
        checked_bls_keypair(104),
    ];
    let (body, validator_set) = lane_drain_fixture(&keys);
    let votes = keys
        .iter()
        .rev()
        .map(|keypair| {
            LaneDrainVoteV1::new_signed(body.clone(), peer(keypair), keypair.private_key())
                .expect("valid drain vote")
        })
        .collect::<Vec<_>>();
    let certificate = aggregate_lane_drain_votes(body, validator_set, &votes)
        .expect("four drain votes project to the exact threshold");
    assert_eq!(certificate.signers_bitmap, vec![0b0000_0111]);
    assert_eq!(
        certificate
            .signer_proofs
            .iter()
            .map(|proof| proof.signer)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    validate_lane_drain_certificate(&certificate).expect("projected drain certificate validates");
    let mut superset = certificate;
    superset.signers_bitmap = vec![0b0000_1111];
    assert_eq!(
        validate_lane_drain_certificate(&superset),
        Err(LaneDrainCertificateError::SignerCountMismatch {
            expected: 3,
            actual: 4,
        })
    );
}

#[test]
fn new_view_certificate_projects_four_votes_and_rejects_four_signers_on_wire() {
    let keypairs = [
        checked_bls_keypair(11),
        checked_bls_keypair(12),
        checked_bls_keypair(13),
        checked_bls_keypair(14),
    ];
    let (network_id, epoch, payload) = autonomous_payload_fixture(&keypairs);
    let source = payload.origin_proposal.clone();
    let body = LaneBlockNewViewBodyV1::for_transition(&source, &payload, 1, network_id, epoch)
        .expect("NewView body");
    let votes = keypairs
        .iter()
        .rev()
        .map(|keypair| {
            LaneBlockNewViewVoteV1::new_signed(
                body.clone(),
                peer(keypair),
                keypair.private_key(),
            )
            .expect("NewView vote")
        })
        .collect::<Vec<_>>();
    let certificate = aggregate_lane_block_new_view_votes(
        body,
        source.descriptor.validator_set.clone(),
        &votes,
    )
    .expect("four NewView votes project to the exact threshold");
    assert_eq!(certificate.signers_bitmap, vec![0b0000_0111]);
    let selected_pops = selected_signer_pops(
        &certificate.validator_set,
        &certificate.signers_bitmap,
        &keypairs,
    );
    validate_lane_block_new_view_certificate(&certificate, &selected_pops)
        .expect("projected NewView certificate validates");
    let mut superset = certificate;
    superset.signers_bitmap = vec![0b0000_1111];
    assert_eq!(
        validate_lane_block_new_view_certificate(&superset, &signer_pops(&keypairs)),
        Err(LaneAutonomousArtifactError::NewViewSignerCountMismatch {
            expected: 3,
            actual: 4,
        })
    );
}

#[test]
fn availability_qc_projects_four_votes_and_rejects_four_signers_on_wire() {
    let keypairs = [
        checked_bls_keypair(34),
        checked_bls_keypair(35),
        checked_bls_keypair(36),
        checked_bls_keypair(37),
    ];
    let (network_id, epoch, payload) = autonomous_payload_fixture(&keypairs);
    let proposal = &payload.origin_proposal;
    let ready_votes = keypairs
        .iter()
        .rev()
        .map(|keypair| {
            signed_autonomous_prepare_vote(
                &payload,
                proposal,
                network_id,
                epoch,
                keypair,
                &keypairs,
            )
            .payload_availability_vote
            .expect("fixture READY vote")
        })
        .collect::<Vec<_>>();
    let qc = aggregate_lane_payload_availability_votes(
        ready_votes[0].body.clone(),
        proposal.descriptor.validator_set.clone(),
        &ready_votes,
    )
    .expect("four READY votes project to the exact threshold");
    assert_eq!(qc.signers_bitmap, vec![0b0000_0111]);
    validate_lane_payload_availability_qc(&qc).expect("projected availability QC validates");
    let mut superset = qc;
    superset.signers_bitmap = vec![0b0000_1111];
    assert_eq!(
        validate_lane_payload_availability_qc(&superset),
        Err(
            LaneAutonomousArtifactError::AvailabilitySignerCountMismatch {
                expected: 3,
                actual: 4,
            }
        )
    );
}

#[test]
fn lane_block_qc_projects_four_votes_and_rejects_four_signers_on_wire() {
    let mut keys = (1_u8..=4).map(checked_bls_keypair).collect::<Vec<_>>();
    keys.sort_by_key(peer);
    let validator_set = keys.iter().map(peer).collect::<Vec<_>>();
    let body = vote_body(&validator_set);
    let votes = keys
        .iter()
        .rev()
        .map(|keypair| signed_vote(&body, keypair))
        .collect::<Vec<_>>();
    let qc = aggregate_lane_block_votes_to_qc(body, validator_set, &votes)
        .expect("four lane-block votes project to the exact threshold");
    assert_eq!(qc.signers_bitmap, vec![0b0000_0111]);
    validate_lane_block_qc_aggregate(&qc, &signer_pops(&keys))
        .expect("projected lane-block QC validates");
    let mut superset = qc;
    superset.signers_bitmap = vec![0b0000_1111];
    assert_eq!(
        validate_lane_block_qc(&superset),
        Err(LaneBlockQcIngressError::SignerCountMismatch {
            expected: 3,
            actual: 4,
        })
    );
}
