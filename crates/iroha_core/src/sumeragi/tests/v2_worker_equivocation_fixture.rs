/// Build exact signed phase-vote evidence for the production admission bridge.
fn exact_vote_equivocation(
    service: &ProductionV2Services,
    keys: &[KeyPair],
) -> wire::SumeragiV2Equivocation {
    let round = wire::ConsensusRound {
        context_id: service.context.id(),
        height: service.context.height,
        view: 0,
    };
    let signer = 1;
    let execution_commitment = wire::ExecutionCommitment::without_offline_cash_top_ups_or_merge_carrier(
        Hash::new(b"equivocation parent state"),
        Hash::new(b"equivocation post state"),
        Hash::new(b"equivocation ordinary writes"),
        1,
        Hash::new(b"equivocation executed block"),
    );
    let signed_vote = |seed: u8| {
        let mut vote = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: wire::BlockSubject {
                parent_block_hash: None,
                block_hash: HashOf::from_untyped_unchecked(Hash::prehashed([seed; 32])),
                payload_hash: Hash::prehashed([seed.wrapping_add(1); 32]),
            },
            execution_commitment,
            signer,
            signature: Vec::new(),
        };
        vote.signature = Signature::new(
            keys[usize::try_from(signer).expect("small signer index")].private_key(),
            &vote.signature_preimage(),
        )
        .payload()
        .to_vec();
        vote
    };
    wire::SumeragiV2Equivocation::PhaseVote {
        first: signed_vote(0xA1),
        second: signed_vote(0xA2),
    }
}

#[test]
fn production_equivocation_bridge_retains_locally_and_deduplicates_service_replay() {
    let (mut service, keys) = fixture();
    let evidence = exact_vote_equivocation(&service, &keys);
    service
        .report_equivocation(evidence.clone())
        .expect("retain valid exact equivocation evidence");
    let shared_state = Arc::clone(&service.state);
    assert_eq!(
        shared_state.world.consensus_evidence.view().iter().count(),
        0,
        "private observation must not mutate consensus state"
    );
    assert_eq!(shared_state.sumeragi_v2_pending_evidence.lock().len(), 1);

    let wire::SumeragiV2Equivocation::PhaseVote { first, second } = evidence.clone() else {
        unreachable!("phase-vote fixture")
    };
    service
        .report_equivocation(wire::SumeragiV2Equivocation::PhaseVote {
            first: second,
            second: first,
        })
        .expect("swapped replay is an idempotent duplicate");

    let (mut restarted_service, _) = fixture();
    restarted_service.context = service.context.clone();
    restarted_service.validator_set_pops = service.validator_set_pops.clone();
    restarted_service.state = Arc::clone(&shared_state);
    restarted_service
        .report_equivocation(evidence)
        .expect("service replay observes the process-local canonical key");
    assert_eq!(
        shared_state.world.consensus_evidence.view().iter().count(),
        0
    );
    assert_eq!(shared_state.sumeragi_v2_pending_evidence.lock().len(), 1);
}

#[test]
fn production_equivocation_bridge_rejects_invalid_or_unanchored_evidence() {
    let (mut invalid_service, invalid_keys) = fixture();
    let mut forged = exact_vote_equivocation(&invalid_service, &invalid_keys);
    let wire::SumeragiV2Equivocation::PhaseVote { second, .. } = &mut forged else {
        unreachable!("phase-vote fixture")
    };
    second.signature[0] ^= 0x80;
    assert!(
        invalid_service.report_equivocation(forged).is_err(),
        "invalid evidence must fail before persistence or reporting"
    );
    assert_eq!(
        invalid_service
            .state
            .world
            .consensus_evidence
            .view()
            .iter()
            .count(),
        0
    );

    let (mut foreign_context_service, foreign_keys) = fixture();
    foreign_context_service.context.network_id =
        crate::sumeragi::synthetic_network_id("foreign-evidence-chain");
    let foreign_evidence = exact_vote_equivocation(&foreign_context_service, &foreign_keys);
    assert!(
        foreign_context_service
            .report_equivocation(foreign_evidence)
            .is_err(),
        "a valid pair from an unanchored context must fail closed"
    );
    assert_eq!(
        foreign_context_service
            .state
            .world
            .consensus_evidence
            .view()
            .iter()
            .count(),
        0
    );
}
