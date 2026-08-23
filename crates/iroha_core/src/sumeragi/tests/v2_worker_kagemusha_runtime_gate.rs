#[cfg(feature = "bls")]
fn run_kagemusha_runtime_gated_vote(
    local_runtime_effective_config_sha256: Option<[u8; 32]>,
    phase: wire::GlobalPhase,
) -> V2IoCompletion {
    let crate::sumeragi::v2_apply::ProductionKagemushaRuntimeGateFixtureV1 {
        context,
        body_store,
        apply_service,
        commit_qc,
        validator_keys,
        directory: _directory,
    } = crate::sumeragi::v2_apply::production_kagemusha_runtime_gate_fixture_v1(
        local_runtime_effective_config_sha256,
    );
    let output_guard = ConsensusOutputGuard::isolated();
    let io = V2IoHandle::spawn(
        body_store,
        apply_service,
        context.clone(),
        validator_keys[0].clone(),
        Some(0),
        2,
        2,
        1,
        output_guard,
    )
    .expect("spawn production Kagemusha-gated I/O worker");
    let round = commit_qc.round;
    let vote = wire::Vote {
        round,
        proposal_round: commit_qc.proposal_round,
        phase,
        subject: commit_qc.subject,
        execution_commitment: commit_qc.execution_commitment,
        signer: 0,
        signature: Vec::new(),
    };
    io.enqueue(V2IoCommand::Sign {
        task: ConsensusSignTask::for_test(
            0x4B,
            EventTag::new(context.height, round.view, Generation::new(context.height)),
            super::super::v2::SignRequest::Vote(vote),
        ),
        restore_outbound_payload: false,
    })
    .expect("enqueue Kagemusha-gated vote");
    let completion = io
        .recv_completion_timeout(Duration::from_secs(5))
        .expect("receive Kagemusha-gated vote completion");
    io.shutdown()
        .expect("join production Kagemusha-gated I/O worker");
    completion
}

#[cfg(feature = "bls")]
#[test]
fn production_vote_worker_rejects_missing_and_mismatched_kagemusha_projection() {
    for phase in [wire::GlobalPhase::Prepare, wire::GlobalPhase::Commit] {
        for (local, label) in [(None, "missing"), (Some([0x56; 32]), "mismatched")] {
            let completion = run_kagemusha_runtime_gated_vote(local, phase);
            let V2IoCompletion::RecoveryRequired(reason) = completion else {
                panic!("{label} projection must fail closed before {phase:?} signing");
            };
            assert!(
                reason.contains(
                    "active Kagemusha V4 release requires a different complete runtime projection"
                ),
                "unexpected {label} {phase:?} rejection: {reason}"
            );
        }
    }
}

#[cfg(feature = "bls")]
#[test]
fn production_vote_worker_signs_prepare_and_commit_for_exact_kagemusha_projection() {
    for phase in [wire::GlobalPhase::Prepare, wire::GlobalPhase::Commit] {
        let completion = run_kagemusha_runtime_gated_vote(Some([0x55; 32]), phase);
        let V2IoCompletion::Signature { signature, .. } = completion else {
            panic!("exact projection must permit {phase:?} signing");
        };
        assert!(!signature.is_empty());
    }
}
