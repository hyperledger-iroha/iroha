// Included inside output_recovery_tests; cancellation never needs body or service authority.

fn obsolete_proposal_timeout(
    verified: &VerifiedHeightContext,
    keys: &[KeyPair],
    view: u64,
) -> wire::TimeoutCertificate {
    let round = wire::ConsensusRound {
        context_id: verified.context().id(),
        height: verified.context().height,
        view,
    };
    let signers = vec![0, 1, 2];
    let shares = signers
        .iter()
        .map(|signer| {
            let vote = wire::TimeoutVote {
                round,
                highest_prepare_qc: None,
                signer: *signer,
                signature: Vec::new(),
            };
            Signature::new(
                keys[usize::try_from(*signer).expect("fixture timeout signer")].private_key(),
                &vote.signature_preimage(),
            )
            .payload()
            .to_vec()
        })
        .collect::<Vec<_>>();
    let certificate = wire::TimeoutCertificate {
        round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: None,
            signers,
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(
                &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            )
            .expect("aggregate obsolete-Proposal timeout shares"),
        }],
    };
    assert!(
        verified
            .verify_consensus_message(&wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate.clone()),
            ))
            .is_ok()
    );
    certificate
}

fn obsolete_proposal_signed(
    verified: &VerifiedHeightContext,
    keys: &[KeyPair],
    view: u64,
    marker: u8,
) -> wire::Proposal {
    let round = wire::ConsensusRound {
        context_id: verified.context().id(),
        height: verified.context().height,
        view,
    };
    let body = [marker; 8];
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new([marker, 1])),
        payload_hash: Hash::new(body),
    };
    let payload =
        crate::sumeragi::v2_chunks::encode_payload(verified.context(), round, subject, &body)
            .expect("encode canonical obsolete-Proposal manifest");
    let justification = if view == 0 {
        wire::ProposalJustification::ParentCommit(wire::ParentCommitJustification {
            certificate: None,
        })
    } else {
        wire::ProposalJustification::Timeout(wire::TimeoutJustification {
            timeout_certificate: obsolete_proposal_timeout(verified, keys, view - 1),
            highest_prepare_qc: None,
        })
    };
    let proposer = verified.context().leader(view);
    let mut proposal = wire::Proposal {
        round,
        proposer,
        subject,
        manifest: payload.manifest().clone(),
        justification,
        signature: Vec::new(),
    };
    proposal.signature = Signature::new(
        keys[usize::try_from(proposer).expect("fixture Proposal signer")].private_key(),
        &proposal.signature_preimage(),
    )
    .payload()
    .to_vec();
    assert!(
        verified
            .verify_consensus_message(&wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Proposal(proposal.clone()),
            ))
            .is_ok()
    );
    proposal
}

fn obsolete_proposal_pair(
    verified: &VerifiedHeightContext,
    signed: wire::Proposal,
    parent_ordinal: u128,
    child_ordinal: u128,
) -> [LifecycleLedgerRecordV1; 2] {
    let mut unsigned = signed.clone();
    unsigned.signature.clear();
    let context = super::super::projection::lifecycle_context(verified.context());
    let [parent_case, child_case] =
        super::super::replay_authority::exact_proposal_sign_broadcast_fixture(
            context,
            unsigned,
            signed.clone(),
        );
    let tag = EventTag::new(signed.round.height, signed.round.view, Generation::new(0));
    let effect = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::Proposal(signed.clone()),
    ));
    let ownership = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&effect),
        vec![
            RuntimeEffectOwnership::fresh_for_test_with_semantic_identity(
                tag,
                parent_ordinal,
                &signed.signature,
            ),
        ],
    )
    .expect("bind independently named Proposal owner")
    .pop()
    .expect("one Proposal owner");
    let pending = ownership
        .exact_pending_adapter_effect_binding(&effect)
        .expect("bind exact Proposal output");
    let prepared = super::super::work_registry::PreparedLifecycleAdmissionV1::direct_signed(
        context, verified, effect, pending,
    )
    .unwrap_or_else(|_| panic!("prepare exact Proposal owner"));
    let owner = OwnerId::new(prepared.candidate().causal_root, parent_ordinal);
    let parent = LifecycleLedgerRecordV1::new(
        parent_case.key,
        owner,
        parent_ordinal,
        parent_case.work_class,
        parent_case.stage,
        Some(TerminalOutcome::Advanced),
        owner.causal_root().digest(),
        parent_case.payload,
        parent_case.authority,
        DurableContinuation::successor(
            DurableContinuationEdge::SignProposalToBroadcast,
            child_ordinal,
        ),
    )
    .expect("construct exact Advanced Proposal Sign");
    let child = LifecycleLedgerRecordV1::new(
        child_case.key,
        owner,
        child_ordinal,
        child_case.work_class,
        child_case.stage,
        None,
        owner.causal_root().digest(),
        child_case.payload,
        child_case.authority,
        DurableContinuation::None,
    )
    .expect("construct exact signed Proposal child");
    [parent, child]
}

fn obsolete_proposal_ledger(
    verified: &VerifiedHeightContext,
    records: Vec<LifecycleLedgerRecordV1>,
) -> LifecycleLedgerV1 {
    LifecycleLedgerV1::new(
        super::super::projection::lifecycle_context(verified.context()),
        records
            .iter()
            .map(LifecycleLedgerRecordV1::ordinal)
            .max()
            .unwrap_or(0),
        records,
        BTreeMap::new(),
    )
    .expect("construct canonical obsolete-Proposal ledger")
}

fn obsolete_proposal_frontier(
    verified: &VerifiedHeightContext,
    keys: &[KeyPair],
    timed_out_view: u64,
) -> crate::sumeragi::v2::LeaderWireRecoveryAuthority {
    crate::sumeragi::v2::LeaderWireRecoveryAuthority::from_verified_installed_timeout_for_test(
        verified,
        &obsolete_proposal_timeout(verified, keys, timed_out_view),
    )
    .expect("seal cryptographically verified installed-timeout frontier")
}

fn obsolete_proposal_decision(
    verified: &VerifiedHeightContext,
    keys: &[KeyPair],
    proposal: &wire::Proposal,
) -> wire::QuorumCertificate {
    let mut decision = prepare_certificate(verified, keys, proposal.round, proposal.subject, false);
    decision.phase = wire::GlobalPhase::Commit;
    let preimage = wire::Vote {
        round: decision.round,
        proposal_round: decision.proposal_round,
        phase: decision.phase,
        subject: decision.subject,
        execution_commitment: decision.execution_commitment,
        signer: 0,
        signature: Vec::new(),
    }
    .signature_preimage();
    let shares = decision
        .signers
        .iter()
        .map(|signer| {
            Signature::new(
                keys[usize::try_from(*signer).expect("fixture Decision signer")].private_key(),
                &preimage,
            )
            .payload()
            .to_vec()
        })
        .collect::<Vec<_>>();
    decision.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
        &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
    )
    .expect("aggregate Decision signatures");
    decision
}

fn pending_kura_incident_output_record(
    verified: &VerifiedHeightContext,
    effect: AdapterEffect,
    owner_ordinal: u128,
    ordinal: u128,
) -> LifecycleLedgerRecordV1 {
    let context = super::super::projection::lifecycle_context(verified.context());
    let tag = EventTag::new(verified.context().height, 0, Generation::INITIAL);
    // A lifecycle ordinal does not enter fresh_for_test's causal root. Name
    // each incident owner explicitly; outputs of one owner share that name.
    let mut semantic_identity = b"pending Kura incident output owner".to_vec();
    semantic_identity.extend_from_slice(&owner_ordinal.to_le_bytes());
    let ownership = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&effect),
        vec![
            RuntimeEffectOwnership::fresh_for_test_with_semantic_identity(
                tag,
                owner_ordinal,
                &semantic_identity,
            ),
        ],
    )
    .expect("bind the incident's independently named output owner")
    .pop()
    .expect("one incident output owner");
    let pending = ownership
        .exact_pending_adapter_effect_binding(&effect)
        .expect("bind the exact incident output");
    let prepared = super::super::work_registry::PreparedLifecycleAdmissionV1::direct_signed(
        context, verified, effect, pending,
    )
    .unwrap_or_else(|_| panic!("prepare the authenticated incident output"));
    let candidate = prepared.candidate().clone();
    LifecycleLedgerRecordV1::new(
        candidate.key,
        OwnerId::new(candidate.causal_root, owner_ordinal),
        ordinal,
        candidate.work_class,
        candidate.stage,
        None,
        candidate.reconstruction_source,
        candidate.payload,
        candidate.replay_authority,
        DurableContinuation::None,
    )
    .expect("construct the exact incident output row")
}

impl super::super::ProductionLifecycleOwnerV1 {
    /// Persist the complete eight-row cut retained by the production incident.
    pub(in crate::sumeragi) fn persist_pending_kura_incident_outputs_for_test(
        verified: &VerifiedHeightContext,
        proposal: wire::Proposal,
        durable: &DurableBodyReceipt,
        prepare_vote: wire::Vote,
        prepare_qc: wire::QuorumCertificate,
        commit_vote: wire::Vote,
        decision: wire::QuorumCertificate,
        root: &Path,
    ) -> LifecycleLedgerV1 {
        verified
            .verify_consensus_message(&wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Proposal(proposal.clone()),
            ))
            .expect("authenticate pending Kura Proposal fixture");
        let context = super::super::projection::lifecycle_context(verified.context());
        let tag = EventTag::new(
            proposal.round.height,
            proposal.round.view,
            Generation::INITIAL,
        );
        let validate = super::super::replay_authority::exact_local_body_record_fixture(
            context,
            tag,
            proposal.manifest.clone(),
            durable,
            LifecycleStageKind::ValidateBody,
        )
        .expect("derive the exact retained local Validate row");
        let validate_root = CausalRoot::new(LifecycleDigest::new(
            *Hash::new(b"pending Kura incident local Validate owner").as_ref(),
        ));
        let mut records = vec![
            LifecycleLedgerRecordV1::new(
                validate.key,
                OwnerId::new(validate_root, 542069),
                542069,
                validate.work_class,
                validate.stage,
                Some(TerminalOutcome::Advanced),
                validate_root.digest(),
                validate.payload,
                validate.authority,
                DurableContinuation::AdvancedNoSuccessor,
            )
            .expect("retain the successful Validate tombstone"),
        ];
        records.extend(obsolete_proposal_pair(verified, proposal, 542077, 542080));

        let mut unsigned = prepare_vote.clone();
        unsigned.signature.clear();
        let [parent, child] = super::super::replay_authority::exact_prepare_sign_broadcast_fixture(
            context,
            unsigned,
            prepare_vote.clone(),
        );
        let prepare_output = pending_kura_incident_output_record(
            verified,
            AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Vote(prepare_vote),
            )),
            542081,
            542083,
        );
        let prepare_owner = prepare_output.owner();
        records.push(
            LifecycleLedgerRecordV1::new(
                parent.key,
                prepare_owner,
                542081,
                parent.work_class,
                parent.stage,
                Some(TerminalOutcome::Advanced),
                prepare_owner.causal_root().digest(),
                parent.payload,
                parent.authority,
                DurableContinuation::successor(
                    DurableContinuationEdge::SignPrepareToBroadcast,
                    542083,
                ),
            )
            .expect("retain the exact Prepare Sign predecessor"),
        );
        records.push(
            LifecycleLedgerRecordV1::new(
                child.key,
                prepare_owner,
                542083,
                child.work_class,
                child.stage,
                None,
                prepare_owner.causal_root().digest(),
                child.payload,
                child.authority,
                DurableContinuation::None,
            )
            .expect("retain the live signed Prepare output"),
        );

        for (ordinal, owner_ordinal, payload) in [
            (
                542123,
                542123,
                wire::ConsensusMessageV2Payload::QuorumCertificate(prepare_qc),
            ),
            (
                542126,
                542123,
                wire::ConsensusMessageV2Payload::Vote(commit_vote),
            ),
            (
                542138,
                542138,
                wire::ConsensusMessageV2Payload::QuorumCertificate(decision),
            ),
        ] {
            let output = pending_kura_incident_output_record(
                verified,
                AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(payload)),
                owner_ordinal,
                ordinal,
            );
            let authenticated = output
                .authenticate_recovered_lifecycle_output(context, verified, None, None)
                .expect("authenticate the exact terminal output fixture");
            assert_eq!(authenticated.owner(), output.owner());
            records.push(output.with_terminal_for_test(Some(TerminalOutcome::Advanced)));
        }
        assert_eq!(
            records
                .iter()
                .map(|row| (row.ordinal(), row.owner().first_admission_ordinal()))
                .collect::<Vec<_>>(),
            vec![
                (542069, 542069),
                (542077, 542077),
                (542080, 542077),
                (542081, 542081),
                (542083, 542081),
                (542123, 542123),
                (542126, 542123),
                (542138, 542138),
            ],
        );
        assert_eq!(
            records
                .iter()
                .map(|row| row.owner().causal_root())
                .collect::<BTreeSet<_>>()
                .len(),
            5,
            "the five distinct incident owners must have distinct causal roots",
        );
        let ledger = obsolete_proposal_ledger(verified, records);
        assert_eq!(ledger.records().len(), 8);
        let (store, _) = LifecycleLedgerStoreV1::open(root, ledger.context())
            .expect("open pending Kura Proposal ledger");
        store
            .persist(&ledger)
            .expect("persist the complete incident cut");
        ledger
    }

    /// Verify that cancellation changed only the retained Proposal's terminal state.
    pub(in crate::sumeragi) fn assert_pending_kura_proposal_cancelled_for_test(
        &self,
        before: &LifecycleLedgerV1,
    ) {
        let after = self
            .coordinator
            .ledger_store
            .as_ref()
            .expect("pending Kura owner retains its ledger store")
            .load()
            .expect("reload durable Proposal cancellation");
        assert_eq!(after.context(), before.context());
        assert_eq!(after.high_water(), before.high_water());
        assert_eq!(after.records().len(), before.records().len());
        for (original, retained) in before.records().iter().zip(after.records()) {
            if original.ordinal() != 542080 {
                assert_eq!(
                    retained, original,
                    "cancellation preserves every other incident row"
                );
            }
        }
        let original = before
            .records()
            .iter()
            .find(|row| row.ordinal() == 542080)
            .unwrap();
        let cancelled = after
            .records()
            .iter()
            .find(|row| row.ordinal() == 542080)
            .unwrap();
        let expected = |terminal| {
            LifecycleLedgerRecordV1::new(
                original.key().unwrap(),
                original.owner(),
                original.ordinal(),
                original.work_class().unwrap(),
                original.stage().unwrap(),
                terminal,
                original.reconstruction_source(),
                original.durable_payload().unwrap(),
                self.coordinator.durable_records[&original.ordinal()]
                    .replay_authority
                    .clone(),
                original.continuation().unwrap(),
            )
            .expect("reconstruct exact retained Proposal row")
        };
        assert_eq!(*original, expected(None));
        assert_eq!(*cancelled, expected(Some(TerminalOutcome::Cancelled)));
        assert_eq!(self.recovered_lifecycle_output_count(), 1);
    }

    /// Compare the retained incident history after actual service acceptance.
    pub(in crate::sumeragi) fn assert_pending_kura_incident_settled_for_test(
        before: &LifecycleLedgerV1,
        root: &Path,
    ) {
        let (store, _) = LifecycleLedgerStoreV1::open(root, before.context())
            .expect("reopen the settled incident ledger");
        let after = store.load().expect("read the settled incident ledger");
        for original in before.records() {
            let retained = after
                .records()
                .iter()
                .find(|row| row.ordinal() == original.ordinal())
                .expect("every incident row survives pending Kura replay");
            let terminal = match original.ordinal() {
                542080 => Some(TerminalOutcome::Cancelled),
                542083 => Some(TerminalOutcome::Advanced),
                _ => {
                    assert_eq!(
                        retained, original,
                        "terminal history must remain byte-exact"
                    );
                    continue;
                }
            };
            let expected = original.clone().with_terminal_for_test(terminal);
            assert_eq!(*retained, expected);
        }
    }
}

#[test]
fn cold_output_cancels_same_view_proposal_after_authenticated_decision_without_timeout() {
    let (verified, keys) = verified_fixture();
    let proposal = obsolete_proposal_signed(&verified, &keys, 0, 0xD1);
    let decision = obsolete_proposal_decision(&verified, &keys, &proposal);
    let frontier =
        crate::sumeragi::v2::LeaderWireRecoveryAuthority::from_verified_decision_for_test(
            &verified, &decision,
        )
        .expect("authenticate durable Decision frontier");
    let ledger = obsolete_proposal_ledger(
        &verified,
        obsolete_proposal_pair(&verified, proposal, 3, 7).into(),
    );
    let before = ledger.clone();
    let outputs = PreparedLifecycleOutputRecoveryV1::assemble_with_frontier(
        &ledger,
        &verified,
        RecoveredWalStartupProjectionV1::None,
        Some(frontier),
    )
    .expect("Decision retires its same-view Proposal before PendingKura recovery");
    assert_eq!(ledger, before);
    let output = outputs.entries.get(&7).expect("retain exact Proposal row");
    assert_eq!(output.terminal_outcome(), TerminalOutcome::Cancelled);
    assert!(!output.requires_output_service());
    assert!(output.authenticates_settlement(&verified));
}

#[test]
fn cold_decision_proposal_cancellation_preserves_authentication_boundaries() {
    let (verified, keys) = verified_fixture();
    let proposal = obsolete_proposal_signed(&verified, &keys, 0, 0xD2);
    let decision = obsolete_proposal_decision(&verified, &keys, &proposal);
    let frontier =
        crate::sumeragi::v2::LeaderWireRecoveryAuthority::from_verified_decision_for_test(
            &verified, &decision,
        )
        .expect("authenticate Decision frontier");
    let ledger = obsolete_proposal_ledger(
        &verified,
        obsolete_proposal_pair(&verified, proposal.clone(), 3, 7).into(),
    );
    let mut forged_decision = decision;
    forged_decision.aggregate_signature[0] ^= 1;
    assert!(
        crate::sumeragi::v2::LeaderWireRecoveryAuthority::from_verified_decision_for_test(
            &verified,
            &forged_decision,
        )
        .is_none()
    );
    let mut foreign_height = proposal.round;
    foreign_height.height += 1;
    assert!(!frontier.proves_obsolete_proposal(foreign_height));
    let mut foreign_context = proposal.round;
    foreign_context.context_id = wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
        b"foreign Decision cancellation context",
    )));
    assert!(!frontier.proves_obsolete_proposal(foreign_context));
    let unqualified = crate::sumeragi::v2::LeaderWireRecoveryAuthority::from_replayed_adapter(
        verified.context().id(),
        verified.context().height,
        [0; 32],
        0,
        true,
    );
    assert_obsolete_proposal_rejected(&ledger, &verified, Some(unqualified), 7);
    let mut forged_proposal = proposal.clone();
    forged_proposal.signature[0] ^= 1;
    let forged = obsolete_proposal_ledger(
        &verified,
        obsolete_proposal_pair(&verified, forged_proposal, 3, 7).into(),
    );
    assert_obsolete_proposal_rejected(&forged, &verified, Some(frontier), 7);
    let unlinked = obsolete_proposal_ledger(
        &verified,
        vec![direct_output_record(
            &verified,
            AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Proposal(proposal),
            )),
            7,
        )],
    );
    assert_obsolete_proposal_rejected(&unlinked, &verified, Some(frontier), 7);
}

fn assert_obsolete_proposal_rejected(
    ledger: &LifecycleLedgerV1,
    verified: &VerifiedHeightContext,
    frontier: Option<crate::sumeragi::v2::LeaderWireRecoveryAuthority>,
    ordinal: u128,
) {
    let error = PreparedLifecycleOutputRecoveryV1::assemble_with_frontier(
        ledger,
        verified,
        RecoveredWalStartupProjectionV1::None,
        frontier,
    )
    .expect_err("unproved Proposal cancellation must fail closed");
    assert!(matches!(
        error,
        LifecycleRecoveryAssemblyErrorKind::InvalidLifecycleOutputRecovery {
            ordinal: observed,
            work_class: LifecycleWorkClass::Broadcast,
            stage,
        } if observed == ordinal && stage.kind() == LifecycleStageKind::BroadcastProposal
    ));
}

#[test]
fn cold_output_cancels_only_exact_proposal_child_below_installed_view() {
    let (verified, keys) = verified_fixture();
    let proposal = obsolete_proposal_signed(&verified, &keys, 1, 0xC1);
    let ledger = obsolete_proposal_ledger(
        &verified,
        obsolete_proposal_pair(&verified, proposal, 3, 7).into(),
    );
    let unchanged = ledger.clone();
    let recovered = PreparedLifecycleOutputRecoveryV1::assemble_with_frontier(
        &ledger,
        &verified,
        RecoveredWalStartupProjectionV1::None,
        Some(obsolete_proposal_frontier(&verified, &keys, 1)),
    )
    .expect("installed next view authorizes exact old Proposal cancellation");
    assert_eq!(ledger, unchanged, "assembly cannot rewrite durable rows");
    assert_eq!(recovered.entries.len(), 1);
    let output = recovered
        .entries
        .get(&7)
        .expect("retain exact child ordinal");
    assert_eq!(output.owner(), ledger.records()[1].owner());
    assert_eq!(output.ordinal(), 7);
    assert_eq!(output.terminal_outcome(), TerminalOutcome::Cancelled);
    assert!(!output.requires_output_service());
    assert!(output.authenticates_settlement(&verified));
    assert_eq!(output.candidate().key, ledger.records()[1].key().unwrap());
}

#[test]
fn cold_output_rejects_current_and_future_proposal_cancellation() {
    let (verified, keys) = verified_fixture();
    let frontier = obsolete_proposal_frontier(&verified, &keys, 0);
    for view in [1, 2] {
        let proposal = obsolete_proposal_signed(&verified, &keys, view, 0xC2);
        let ledger = obsolete_proposal_ledger(
            &verified,
            obsolete_proposal_pair(&verified, proposal, 3, 7).into(),
        );
        assert_obsolete_proposal_rejected(&ledger, &verified, Some(frontier), 7);
    }
}

#[test]
fn cold_output_rejects_proposal_without_authenticated_installed_timeout() {
    let (verified, keys) = verified_fixture();
    let proposal = obsolete_proposal_signed(&verified, &keys, 0, 0xC3);
    let ledger = obsolete_proposal_ledger(
        &verified,
        obsolete_proposal_pair(&verified, proposal, 3, 7).into(),
    );
    assert_obsolete_proposal_rejected(&ledger, &verified, None, 7);
    let unqualified = crate::sumeragi::v2::LeaderWireRecoveryAuthority::from_replayed_adapter(
        verified.context().id(),
        verified.context().height,
        [0xC3; 32],
        3,
        false,
    );
    assert_obsolete_proposal_rejected(&ledger, &verified, Some(unqualified), 7);
}

#[test]
fn cold_output_rejects_foreign_installed_timeout_frontier() {
    let (verified, keys) = verified_fixture();
    let mut foreign_context = verified.context().clone();
    foreign_context.leader_seed[0] ^= 1;
    let proofs = keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("foreign fixture PoP")
        })
        .collect();
    let foreign = VerifiedHeightContext::genesis(foreign_context, proofs)
        .expect("verify distinct frozen context");
    let proposal = obsolete_proposal_signed(&verified, &keys, 0, 0xC4);
    let ledger = obsolete_proposal_ledger(
        &verified,
        obsolete_proposal_pair(&verified, proposal, 3, 7).into(),
    );
    assert_obsolete_proposal_rejected(
        &ledger,
        &verified,
        Some(obsolete_proposal_frontier(&foreign, &keys, 1)),
        7,
    );
}

#[test]
fn cold_output_rejects_unlinked_proposal_despite_another_exact_sign_parent() {
    let (verified, keys) = verified_fixture();
    let target = obsolete_proposal_signed(&verified, &keys, 0, 0xC5);
    let target = direct_output_record(
        &verified,
        AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Proposal(target),
        )),
        7,
    );
    let missing = obsolete_proposal_ledger(&verified, vec![target.clone()]);
    let frontier = obsolete_proposal_frontier(&verified, &keys, 1);
    assert_obsolete_proposal_rejected(&missing, &verified, Some(frontier), 7);
    let other = obsolete_proposal_signed(&verified, &keys, 0, 0xC6);
    let [parent, child] = obsolete_proposal_pair(&verified, other, 9, 12);
    let wrong_parent = obsolete_proposal_ledger(&verified, vec![target, parent, child]);
    assert_obsolete_proposal_rejected(&wrong_parent, &verified, Some(frontier), 7);
}

#[test]
fn cold_output_rejects_tampered_proposal_even_with_exact_parent_and_later_timeout() {
    let (verified, keys) = verified_fixture();
    let mut proposal = obsolete_proposal_signed(&verified, &keys, 0, 0xC7);
    proposal.signature[0] ^= 1;
    let ledger = obsolete_proposal_ledger(
        &verified,
        obsolete_proposal_pair(&verified, proposal, 3, 7).into(),
    );
    assert_obsolete_proposal_rejected(
        &ledger,
        &verified,
        Some(obsolete_proposal_frontier(&verified, &keys, 1)),
        7,
    );
}

#[test]
fn cold_output_rejects_forged_timeout_cancellation_frontier() {
    let (verified, keys) = verified_fixture();
    let mut timeout = obsolete_proposal_timeout(&verified, &keys, 1);
    timeout.groups[0].aggregate_signature[0] ^= 1;
    let frontier =
        crate::sumeragi::v2::LeaderWireRecoveryAuthority::from_verified_installed_timeout_for_test(
            &verified, &timeout,
        );
    assert!(
        frontier.is_none(),
        "forged TC cannot mint cancellation authority"
    );
    let proposal = obsolete_proposal_signed(&verified, &keys, 0, 0xC8);
    let ledger = obsolete_proposal_ledger(
        &verified,
        obsolete_proposal_pair(&verified, proposal, 3, 7).into(),
    );
    assert_obsolete_proposal_rejected(&ledger, &verified, frontier, 7);
}

fn obsolete_proposal_owner(
    verified: &VerifiedHeightContext,
    keys: &[KeyPair],
    ledger: LifecycleLedgerV1,
) -> (super::super::ProductionLifecycleOwnerV1, tempfile::TempDir) {
    let root = tempfile::TempDir::new().expect("temporary cancellation storage root");
    let (ledger_store, _) =
        LifecycleLedgerStoreV1::open(&root.path().join("ledger"), ledger.context())
            .expect("open cancellation ledger store");
    ledger_store
        .persist(&ledger)
        .expect("fsync original cancellation ledger");
    drop(ledger_store);
    let owner = obsolete_proposal_reopen_owner(verified, keys, root.path());
    (owner, root)
}

fn obsolete_proposal_reopen_owner(
    verified: &VerifiedHeightContext,
    keys: &[KeyPair],
    root: &Path,
) -> super::super::ProductionLifecycleOwnerV1 {
    let body_store = V2BodyStore::open(root.join("body"), verified.context().clone())
        .expect("open empty cancellation body store");
    let (mut payload_store, payloads) =
        CertifiedServePayloadStoreV1::open(&root.join("payload"), verified.context())
            .expect("open empty cancellation payload store");
    let serve_payloads = payloads
        .authenticate(verified, &keys[0], &body_store)
        .expect("authenticate empty cancellation payload inventory");
    let (ledger_store, ledger) = LifecycleLedgerStoreV1::open(
        &root.join("ledger"),
        super::super::projection::lifecycle_context(verified.context()),
    )
    .expect("open cancellation ledger store");
    let outputs = PreparedLifecycleOutputRecoveryV1::assemble_with_frontier(
        &ledger,
        verified,
        RecoveredWalStartupProjectionV1::None,
        Some(obsolete_proposal_frontier(verified, keys, 1)),
    )
    .expect("assemble cancellation carriers");
    let mut candidates = BTreeMap::new();
    assert!(outputs.splice_candidates(&mut candidates));
    let mut recovery = AuthenticatedLifecycleRecoveryCut {
        context: ledger.context(),
        authenticated_ledger: ledger,
        candidates,
        validate_no_successor: BTreeMap::new(),
        released_validate: None,
        lifecycle_outputs: Some(outputs),
        serve_payloads,
    };
    let authority = super::super::authority::lifecycle_output_owner_test_authority(
        verified,
        recovery.candidates.len(),
    )
    .expect("authenticate capacity for every live cold output");
    let mut registry = super::super::LifecycleWorkRegistryHolder::empty();
    let prepared = LifecycleCoordinator::prepare_with_authenticated_store_borrowed(
        authority,
        ledger_store,
        &payload_store,
        &recovery,
    )
    .expect("prepare exact cancellation coordinator");
    let coordinator = prepared
        .commit_with_registry(registry.registry_mut(), &mut payload_store, &mut recovery)
        .unwrap_or_else(|_| panic!("publish exact cancellation owner"));
    super::super::ProductionLifecycleOwnerV1 {
        verified: verified.clone(),
        coordinator,
        registry,
        recovered_lifecycle_outputs: recovery.take_lifecycle_output_recovery(),
        payload_store,
        serve_payloads: recovery.into_serve_payloads(),
        body_store: Some(body_store),
        body_store_identity: None,
        kura_binding: None,
        apply_service: None,
        adapter_startup: Some(
            crate::sumeragi::v2::ProductionLifecycleAdapterStartupV1::fixture_for_test(),
        ),
        owner_open_successor: None,
    }
}

#[test]
fn cold_proposal_cancellation_fsync_preserves_row_and_skips_output_service() {
    let (verified, keys) = verified_fixture();
    let proposal = obsolete_proposal_signed(&verified, &keys, 1, 0xC9);
    let ledger = obsolete_proposal_ledger(
        &verified,
        obsolete_proposal_pair(&verified, proposal, 3, 7).into(),
    );
    let original = ledger.clone();
    let (mut owner, _root) = obsolete_proposal_owner(&verified, &keys, ledger);
    let before = owner.coordinator.records[&7].clone();
    let calls = std::cell::Cell::new(0);
    assert_eq!(
        owner
            .settle_next_recovered_lifecycle_output(|_| {
                calls.set(calls.get() + 1);
                Err::<super::super::LifecycleOutputServiceDispositionV1, _>(
                    "unexpected Proposal output",
                )
            })
            .expect("cancel exact obsolete Proposal without output I/O"),
        RecoveredLifecycleOutputSettlementV1::Completed,
    );
    assert_eq!(calls.get(), 0);
    assert!(!owner.has_recovered_lifecycle_outputs());
    let mut expected = before;
    expected.state = LifecycleState::Terminal(TerminalOutcome::Cancelled);
    assert_eq!(owner.coordinator.records[&7], expected);
    let durable = owner
        .coordinator
        .ledger_store
        .as_ref()
        .unwrap()
        .load()
        .expect("reload fsynced Proposal cancellation");
    assert_eq!(durable.high_water(), original.high_water());
    assert_eq!(durable.records().len(), original.records().len());
    assert_eq!(durable.records()[0], original.records()[0]);
    let child = &durable.records()[1];
    assert_eq!(child.owner(), original.records()[1].owner());
    assert_eq!(child.ordinal(), 7);
    assert_eq!(child.key(), original.records()[1].key());
    assert_eq!(child.stage(), original.records()[1].stage());
    assert_eq!(child.continuation(), original.records()[1].continuation());
    assert_eq!(child.terminal(), Some(Some(TerminalOutcome::Cancelled)));
    assert!(
        PreparedLifecycleOutputRecoveryV1::assemble(
            &durable,
            &verified,
            RecoveredWalStartupProjectionV1::None
        )
        .expect("terminal cancellation needs no new frontier on restart")
        .is_empty()
    );
}

#[test]
fn cold_proposal_cancellation_waits_for_older_ready_output() {
    let (verified, keys) = verified_fixture();
    let proposal = obsolete_proposal_signed(&verified, &keys, 1, 0xCA);
    let [parent, child] = obsolete_proposal_pair(&verified, proposal, 3, 7);
    let vote = direct_output_record(
        &verified,
        AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(signed_vote(&verified, &keys, 0xCB)),
        )),
        1,
    );
    let ledger = obsolete_proposal_ledger(&verified, vec![vote, parent, child]);
    let (mut owner, _root) = obsolete_proposal_owner(&verified, &keys, ledger);
    let calls = std::cell::Cell::new(0);
    assert_eq!(owner.settle_next_recovered_lifecycle_output(|effect| {
        assert!(matches!(effect, AdapterEffect::Broadcast(message) if matches!(&message.payload, wire::ConsensusMessageV2Payload::Vote(_))));
        calls.set(calls.get() + 1);
        Ok::<_, &'static str>(super::super::LifecycleOutputServiceDispositionV1::SourceRetained)
    }).expect("older output retains its Ready row"), RecoveredLifecycleOutputSettlementV1::SourceRetained);
    assert_eq!(owner.coordinator.records[&7].state, LifecycleState::Ready);
    assert_eq!(
        owner
            .settle_next_recovered_lifecycle_output(|_| {
                calls.set(calls.get() + 1);
                Ok::<_, &'static str>(super::super::LifecycleOutputServiceDispositionV1::Accepted)
            })
            .expect("settle older output first"),
        RecoveredLifecycleOutputSettlementV1::Completed
    );
    assert_eq!(calls.get(), 2);
    assert_eq!(owner.coordinator.records[&7].state, LifecycleState::Ready);
    assert_eq!(
        owner
            .settle_next_recovered_lifecycle_output(|_| {
                calls.set(calls.get() + 1);
                Err::<super::super::LifecycleOutputServiceDispositionV1, _>(
                    "unexpected Proposal output",
                )
            })
            .expect("cancel newly oldest obsolete Proposal"),
        RecoveredLifecycleOutputSettlementV1::Completed
    );
    assert_eq!(calls.get(), 2, "cancellation cannot enter output service");
    assert_eq!(
        owner.coordinator.records[&7].state,
        LifecycleState::Terminal(TerminalOutcome::Cancelled)
    );
}

#[test]
fn cold_proposal_cancellation_fsync_failure_retains_ready_owner_without_output() {
    let (verified, keys) = verified_fixture();
    let proposal = obsolete_proposal_signed(&verified, &keys, 1, 0xCC);
    let ledger = obsolete_proposal_ledger(
        &verified,
        obsolete_proposal_pair(&verified, proposal, 3, 7).into(),
    );
    let original = ledger.clone();
    let (mut owner, root) = obsolete_proposal_owner(&verified, &keys, ledger);
    let records = owner.coordinator.records.clone();
    owner
        .coordinator
        .redirect_test_ledger_to_missing_parent(root.path());
    let calls = std::cell::Cell::new(0);
    assert!(matches!(
        owner.settle_next_recovered_lifecycle_output(|_| {
            calls.set(calls.get() + 1);
            Ok::<_, &'static str>(super::super::LifecycleOutputServiceDispositionV1::Accepted)
        }),
        Err(RecoveredLifecycleOutputSettlementErrorV1::Durability)
    ));
    assert_eq!(calls.get(), 0);
    assert_eq!(owner.coordinator.records, records);
    assert!(owner.coordinator.ready_index.contains(&7));
    assert!(owner.has_recovered_lifecycle_outputs());
    assert_eq!(
        owner.coordinator.fault(),
        Some(CoordinatorFault::DurabilityFailure)
    );
    let (_, retained) =
        LifecycleLedgerStoreV1::open(&root.path().join("ledger"), original.context())
            .expect("reopen original ledger after rejected fsync");
    assert_eq!(
        retained, original,
        "failed cancellation cannot publish a terminal row"
    );
    drop(owner);
    let mut restarted = obsolete_proposal_reopen_owner(&verified, &keys, root.path());
    assert!(restarted.coordinator.ready_index.contains(&7));
    assert!(restarted.has_recovered_lifecycle_outputs());
    assert_eq!(
        restarted
            .settle_next_recovered_lifecycle_output(|_| {
                calls.set(calls.get() + 1);
                Err::<super::super::LifecycleOutputServiceDispositionV1, _>(
                    "unexpected restarted Proposal output",
                )
            })
            .expect("restart retries the same durable cancellation"),
        RecoveredLifecycleOutputSettlementV1::Completed
    );
    assert_eq!(
        calls.get(),
        0,
        "restart cancellation cannot enter output service"
    );
    assert!(!restarted.has_recovered_lifecycle_outputs());
    let retried = restarted
        .coordinator
        .ledger_store
        .as_ref()
        .unwrap()
        .load()
        .expect("reload successfully retried cancellation");
    assert_eq!(retried.high_water(), original.high_water());
    assert_eq!(retried.records().len(), original.records().len());
    assert_eq!(retried.records()[0], original.records()[0]);
    assert_eq!(retried.records()[1].owner(), original.records()[1].owner());
    assert_eq!(retried.records()[1].ordinal(), 7);
    assert_eq!(
        retried.records()[1].terminal(),
        Some(Some(TerminalOutcome::Cancelled))
    );
}

impl super::super::ProductionLifecycleOwnerV1 {
    /// Retain only historical validation. Ordinary Decision admission must create Apply.
    pub(in crate::sumeragi) fn persist_pending_kura_released_validate_for_test(
        verified: &VerifiedHeightContext,
        manifest: &wire::PayloadManifest,
        durable: &DurableBodyReceipt,
        root: &Path,
    ) {
        let context = super::super::projection::lifecycle_context(verified.context());
        let tag = EventTag::new(
            verified.context().height,
            manifest.round.view,
            Generation::INITIAL,
        );
        let validate = super::super::replay_authority::exact_local_body_record_fixture(
            context,
            tag,
            manifest.clone(),
            durable,
            LifecycleStageKind::ValidateBody,
        )
        .expect("bind the real released Validate body");
        let causal_root = CausalRoot::new(LifecycleDigest::new(
            *Hash::new(b"pending Kura standalone Apply historical Validate").as_ref(),
        ));
        let record = LifecycleLedgerRecordV1::new(
            validate.key,
            OwnerId::new(causal_root, 41),
            41,
            validate.work_class,
            validate.stage,
            Some(TerminalOutcome::Advanced),
            causal_root.digest(),
            validate.payload,
            validate.authority,
            DurableContinuation::AdvancedNoSuccessor,
        )
        .expect("construct historical terminal Validate");
        let ledger = LifecycleLedgerV1::new(context, 41, vec![record], BTreeMap::new())
            .expect("construct released Validate prefix");
        let (store, _) =
            LifecycleLedgerStoreV1::open(root, context).expect("open released Validate prefix");
        store
            .persist(&ledger)
            .expect("persist released Validate prefix");
    }

    /// Capture the Apply actually published by ordinary DecisionReleasedApply.
    pub(in crate::sumeragi) fn snapshot_pending_kura_standalone_apply_for_test(
        &self,
        root: &Path,
    ) -> LifecycleLedgerV1 {
        let (_, ledger) = LifecycleLedgerStoreV1::open(root, self.coordinator.active_context)
            .expect("read ordinary admitted Apply");
        let applies = ledger
            .records()
            .iter()
            .filter(|row| row.work_class() == Some(LifecycleWorkClass::Apply))
            .collect::<Vec<_>>();
        assert_eq!(
            applies.len(),
            1,
            "ordinary admission publishes one actual Apply"
        );
        let apply = applies[0];
        assert_eq!(
            ledger.records().len(),
            2,
            "only original Validate and new standalone Apply"
        );
        assert_eq!(apply.owner().first_admission_ordinal(), apply.ordinal());
        assert_eq!(apply.terminal(), Some(None));
        assert_eq!(apply.continuation(), Some(DurableContinuation::None));
        let actual = &self.coordinator.records[&apply.ordinal()];
        assert_eq!(actual.owner, apply.owner());
        assert_eq!(actual.state, super::super::LifecycleState::Ready);
        let carriers = self
            .registry
            .registry_for_test()
            .finalization_entry_kind_census()
            .1
            .into_iter()
            .filter(|(ordinal, _)| *ordinal == apply.ordinal())
            .collect::<Vec<_>>();
        assert_eq!(
            carriers,
            vec![(apply.ordinal(), "DurableRecoveredDecisionApply")]
        );
        ledger
    }

    /// Require byte-preserving passive admission and no duplicate executable owner.
    pub(in crate::sumeragi) fn assert_pending_kura_passive_apply_for_test(
        &mut self,
        before: &LifecycleLedgerV1,
        root: &Path,
    ) {
        Self::assert_pending_kura_apply_progress_for_test(before, root, false);
        let apply = before
            .records()
            .iter()
            .find(|row| row.work_class() == Some(LifecycleWorkClass::Apply))
            .expect("one original standalone Apply");
        let actual = &self.coordinator.records[&apply.ordinal()];
        assert_eq!(actual.owner, apply.owner());
        assert_eq!(actual.key, apply.key().expect("original Apply key"));
        assert_eq!(actual.state, super::super::LifecycleState::Ready);
        assert!(self.coordinator.ready_index.contains(&apply.ordinal()));
        let carriers = self
            .registry
            .registry_for_test()
            .finalization_entry_kind_census()
            .1;
        assert!(
            carriers
                .iter()
                .all(|(ordinal, _)| *ordinal != apply.ordinal()),
            "native PendingKura is the sole executable Apply owner"
        );
        assert_eq!(
            self.exact_lifecycle_output_ordinals_for_registry_census(),
            Some(BTreeSet::from([apply.ordinal()]))
        );
        assert!(self.has_recovered_lifecycle_outputs());
        assert_eq!(self.recovered_lifecycle_output_count(), 1);
        let calls = std::cell::Cell::new(0);
        assert_eq!(
            self.settle_next_recovered_lifecycle_output(|_| {
                calls.set(calls.get() + 1);
                Ok::<_, &'static str>(
                    super::super::concrete_admission::LifecycleOutputServiceDispositionV1::Accepted,
                )
            })
            .expect("ordinary output drain retains passive Apply"),
            RecoveredLifecycleOutputSettlementV1::Deferred
        );
        assert_eq!(
            calls.get(),
            0,
            "output service cannot execute pending Apply"
        );
        Self::assert_pending_kura_apply_progress_for_test(before, root, false);
    }

    /// Verify no mutation before StateApplied and only exact Apply terminalization after it.
    pub(in crate::sumeragi) fn assert_pending_kura_apply_progress_for_test(
        before: &LifecycleLedgerV1,
        root: &Path,
        completed: bool,
    ) {
        let (_, actual) = LifecycleLedgerStoreV1::open(root, before.context())
            .expect("read retained standalone Apply ledger");
        if !completed {
            assert_eq!(
                actual, *before,
                "unfinished replay cannot change the retained ledger"
            );
            return;
        }
        let apply = before
            .records()
            .iter()
            .find(|row| row.work_class() == Some(LifecycleWorkClass::Apply))
            .expect("the original Apply is retained");
        let expected = LifecycleLedgerV1::new(
            before.context(),
            before.high_water(),
            before
                .records()
                .iter()
                .map(|row| {
                    if row.ordinal() == apply.ordinal() {
                        row.clone()
                            .with_terminal_for_test(Some(TerminalOutcome::Advanced))
                    } else {
                        row.clone()
                    }
                })
                .collect(),
            BTreeMap::new(),
        )
        .expect("exact terminal successor preserves every unrelated row");
        assert_eq!(
            actual, expected,
            "only the native Apply's terminal state may advance"
        );
    }

    /// Replace only owner identity in an otherwise genuine ordinary-admission row.
    pub(in crate::sumeragi) fn replace_pending_kura_apply_owner_for_test(
        before: &LifecycleLedgerV1,
        root: &Path,
    ) {
        let wrong = CausalRoot::new(LifecycleDigest::new(
            *Hash::new(b"foreign owner cannot borrow pending Kura Decision").as_ref(),
        ));
        let rows = before
            .records()
            .iter()
            .map(|row| {
                if row.work_class() != Some(LifecycleWorkClass::Apply) {
                    return row.clone();
                }
                assert_ne!(wrong, row.owner().causal_root());
                row.clone()
                    .with_pending_kura_foreign_owner_for_test(OwnerId::new(wrong, row.ordinal()))
            })
            .collect();
        let changed =
            LifecycleLedgerV1::new(before.context(), before.high_water(), rows, BTreeMap::new())
                .expect("construct exact wrong-owner control ledger");
        let (store, _) = LifecycleLedgerStoreV1::open(root, before.context())
            .expect("open wrong-owner control ledger");
        store
            .persist(&changed)
            .expect("persist wrong-owner control ledger");
    }
}
