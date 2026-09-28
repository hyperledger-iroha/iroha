// SCCP v1 route-governance intake, its per-subject Parliament head, and the
// fail-closed generic bridge-proof verifier gate.
// TODO(ws33): drive certificate enactment end to end (including due-certificate
// execution failures) in `crates/iroha_core/tests/sccp_governance.rs`.

fn sccp_route_governance_test_proposal(
    network_id: iroha_data_model::NetworkId,
) -> iroha_data_model::sccp::governance::SccpGovernanceProposalV1 {
    use iroha_data_model::sccp::{
        governance::{
            SccpGovernanceActionV1, SccpGovernanceProposalV1, SccpGovernanceSubjectV1,
            SccpSetParametersActionV1,
        },
        params::SccpParametersV1,
    };
    SccpGovernanceProposalV1 {
        network_id,
        base_revisions: vec![(SccpGovernanceSubjectV1::Parameters, 0).into()],
        actions: vec![SccpGovernanceActionV1::SetParameters(
            SccpSetParametersActionV1 {
                next: SccpParametersV1::taira_default(),
            },
        )],
    }
}

fn bond_sccp_route_governance_test_proposer(state_transaction: &mut StateTransaction<'_, '_>) {
    state_transaction.gov.citizenship_bond_amount = Quantity::from(10_u32);
    state_transaction.world.citizens.insert(
        ALICE_ID.clone(),
        crate::state::CitizenshipRecord::new(ALICE_ID.clone(), Quantity::from(10_u32), 1),
    );
}

fn sccp_route_governance_test_kind(
    proposal: &iroha_data_model::sccp::governance::SccpGovernanceProposalV1,
) -> ProposalKind {
    ProposalKind::SccpRouteGovernance(
        iroha_data_model::governance::types::SccpRouteGovernanceProposal {
            proposal: Box::new(proposal.clone()),
        },
    )
}

world_test!(sccp_route_governance_proposal_rejects_statically_invalid_proposals {
    blank_test_state_transaction!(checked state, block, stx);
    bond_sccp_route_governance_test_proposer(&mut stx);
    let foreign = iroha_data_model::NetworkId::from_genesis_hash(
        iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"foreign SCCP network")),
    );
    assert_ne!(foreign, stx.network_id);
    let mut without_actions = sccp_route_governance_test_proposal(stx.network_id);
    without_actions.actions.clear();
    for (proposal, needle) in [
        (sccp_route_governance_test_proposal(foreign), "network_id"),
        (without_actions, "no actions"),
    ] {
        let error = gov::ProposeSccpRouteGovernance { proposal }
            .execute(&ALICE_ID, &mut stx)
            .expect_err("a statically invalid SCCP proposal must be rejected");
        assert!(
            matches!(&error, InstructionExecutionError::InvariantViolation(message)
                if message.contains(needle)),
            "unexpected static rejection: {error:?}"
        );
    }
    assert!(stx.world.governance_proposals.iter().next().is_none());
});

world_test!(sccp_route_governance_proposal_requires_permission_or_bond {
    blank_test_state_transaction!(checked state, block, stx);
    let proposal = sccp_route_governance_test_proposal(stx.network_id);
    let error = gov::ProposeSccpRouteGovernance {
        proposal: proposal.clone(),
    }
    .execute(&ALICE_ID, &mut stx)
    .expect_err("an unbonded account without the proposer permission must be rejected");
    assert!(
        matches!(&error, InstructionExecutionError::InvariantViolation(message)
            if message.contains("CanProposeSccpRouteGovernance required")),
        "unexpected proposer rejection: {error:?}"
    );
    assert!(stx.world.governance_proposals.iter().next().is_none());
    stx.world.account_permissions.insert(
        ALICE_ID.clone(),
        BTreeSet::from([Permission::from(
            iroha_executor_data_model::permission::sccp::CanProposeSccpRouteGovernance,
        )]),
    );
    gov::ProposeSccpRouteGovernance { proposal }
        .execute(&ALICE_ID, &mut stx)
        .expect("the exact proposer permission admits the proposal");
    assert_eq!(stx.world.governance_proposals.iter().count(), 1);
});

world_test!(sccp_route_governance_proposal_records_once_and_replays_idempotently {
    blank_test_state_transaction!(checked state, block, stx);
    bond_sccp_route_governance_test_proposer(&mut stx);
    let proposal = sccp_route_governance_test_proposal(stx.network_id);
    gov::ProposeSccpRouteGovernance {
        proposal: proposal.clone(),
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("a bonded citizen may propose a valid SCCP governance change");
    let kind = sccp_route_governance_test_kind(&proposal);
    let proposal_id = kind.fingerprint();
    let record = stx
        .world
        .governance_proposals
        .get(&proposal_id)
        .expect("canonical SCCP route-governance proposal");
    assert_eq!(record.kind, kind);
    assert_eq!(record.proposer, ALICE_ID.clone());
    assert_eq!(
        record.status,
        crate::state::GovernanceProposalStatus::Proposed
    );
    let before = norito::codec::Encode::encode(record);
    gov::ProposeSccpRouteGovernance { proposal }
        .execute(&ALICE_ID, &mut stx)
        .expect("an exact re-proposal is idempotent");
    assert_eq!(stx.world.governance_proposals.iter().count(), 1);
    assert_eq!(
        stx.world
            .governance_proposals
            .get(&proposal_id)
            .map(norito::codec::Encode::encode),
        Some(before)
    );
});

world_test!(sccp_route_governance_expected_head_is_scoped_per_subject {
    let state = blank_test_state();
    let mut block = state.block(first_test_block_header_with_checked_height());
    let mut stx = block.transaction();
    let kind = sccp_route_governance_test_kind(&sccp_route_governance_test_proposal(
        stx.network_id,
    ));
    let subject_id = kind.governed_subject_id_v1().expect("subject id");
    let GovernanceExpectedHeadV1::Present(fresh) =
        parliament_expected_head_v1(&kind, &stx).expect("head")
    else {
        panic!("an SCCP head is always present");
    };
    assert_eq!(fresh.subject_id, subject_id);
    assert_eq!(fresh.version, 1);
    let parameters = iroha_data_model::sccp::governance::SccpGovernanceSubjectV1::Parameters;
    assert_eq!(
        fresh.head_root,
        parliament_governance_head_root_v1(&vec![(parameters.clone(), 0_u64)])
    );
    // Another subject's revision leaves the head unchanged; this subject's revision moves it.
    crate::smartcontracts::isi::sccp::store::set_governance_revision(
        &mut stx,
        iroha_data_model::sccp::governance::SccpGovernanceSubjectV1::LightClient(
            iroha_data_model::bridge::SccpNetworkV1::EthereumMainnet,
        ),
        4,
    );
    assert_eq!(
        parliament_expected_head_v1(&kind, &stx).expect("head"),
        GovernanceExpectedHeadV1::Present(fresh.clone())
    );
    crate::smartcontracts::isi::sccp::store::set_governance_revision(&mut stx, parameters, 2);
    let GovernanceExpectedHeadV1::Present(moved) =
        parliament_expected_head_v1(&kind, &stx).expect("head")
    else {
        panic!("an SCCP head is always present");
    };
    assert_eq!(moved.version, 3);
    assert_ne!(moved.head_root, fresh.head_root);
});

world_test!(bridge_payloads_without_an_authoritative_verifier_fail_closed {
    let ics = BridgeProofPayload::Ics(iroha_data_model::bridge::BridgeIcsProof {
        verifier_manifest_hash: [0xAA; 32],
        state_root: [0x11; 32],
        leaf_hash: [0x22; 32],
        proof: iroha_crypto::MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(vec![
            [0x22; 32],
            [0x33; 32],
        ])
        .get_proof(0)
        .expect("fixture Merkle proof"),
        hash_function: iroha_data_model::bridge::BridgeHashFunction::Sha256,
    });
    let transparent = BridgeProofPayload::TransparentZk(BridgeTransparentProof {
        verifier_manifest_hash: [0xBB; 32],
        proof: ProofBox::new("halo2/mock".into(), vec![0xDE, 0xAD]),
        recursion_depth: Some(1),
    });
    for payload in [ics, transparent] {
        let error = reject_unverifiable_bridge_payload(&payload)
            .expect_err("a payload without an on-chain verifier must fail closed");
        assert!(
            format!("{error:?}").contains("authoritative on-chain verifier"),
            "unexpected bridge payload rejection: {error:?}"
        );
    }
});

world_test!(sccp_governance_proposals_list_open_proposals_with_their_admissibility {
    use crate::smartcontracts::isi::sccp::{read, store};
    blank_test_state_transaction!(checked state, block, stx);
    bond_sccp_route_governance_test_proposer(&mut stx);
    assert!(read::governance_proposals(&*stx.world).is_empty());
    let proposal = sccp_route_governance_test_proposal(stx.network_id);
    gov::ProposeSccpRouteGovernance {
        proposal: proposal.clone(),
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("a bonded citizen proposes");
    let listed = read::governance_proposals(&*stx.world);
    let [open] = listed.as_slice() else {
        panic!("one open proposal: {listed:?}");
    };
    assert_eq!(open.proposal, proposal);
    assert_eq!(
        open.content_id,
        iroha_data_model::governance::types::ProposalContentId::derive_v1(
            &sccp_route_governance_test_kind(&proposal)
        )
    );
    assert!(open.admissible);
    assert_eq!(open.latest_attempt, None);
    // Another enactment moved the subject, so no attempt can pass the preflight any more.
    store::set_governance_revision(
        &mut stx,
        iroha_data_model::sccp::governance::SccpGovernanceSubjectV1::Parameters,
        1,
    );
    let listed = read::governance_proposals(&*stx.world);
    assert!(!listed[0].admissible);
});
