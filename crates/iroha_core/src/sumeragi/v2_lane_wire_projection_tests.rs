//! Exact shared-effect/native witness round trips, including lossy TC grouping.

use super::super::LaneNativeWitnesses;
use super::*;

struct Witnesses(Option<LaneManifestV1>);
impl LaneNativeWitnesses for Witnesses {
    fn value(&self, subject: reducer::Subject) -> Option<&LaneValueRefV1> {
        self.0
            .as_ref()
            .map(|manifest| &manifest.value)
            .filter(|value| value.subject().unwrap() == subject)
    }
    fn manifest(&self, subject: reducer::Subject) -> Option<&LaneManifestV1> {
        self.0
            .as_ref()
            .filter(|manifest| manifest.value.subject().unwrap() == subject)
    }
}
fn witnesses(fixture: &Fixture) -> Witnesses {
    Witnesses(Some(LaneManifestV1 {
        value: fixture.value(0),
        layout: fixture.frozen.da_layout,
        chunk_root: Hash::new(b"RS16 root"),
        byte_len: 1,
        chunk_count: wire::expected_encoded_chunk_count(1, fixture.frozen.da_layout).unwrap(),
    }))
}
fn proposal(
    fixture: &Fixture,
    witnesses: &Witnesses,
    view: u64,
    justification: LaneJustificationV1,
) -> LaneProposalBodyV1 {
    LaneProposalBodyV1 {
        round: fixture.round(view),
        proposer: fixture
            .context
            .roster()
            .iter()
            .position(|entry| entry.id() == fixture.context.leader(view))
            .unwrap() as u32,
        manifest: witnesses.0.as_ref().unwrap().clone(),
        justification,
    }
}

#[test]
fn native_effect_projection_roundtrips_every_wal_record_and_exact_signing_intent() {
    let fixture = fixture();
    let auth = fixture.authenticator();
    let witnesses = witnesses(&fixture);
    let prepare = fixture.qc(fixture.statement(0));
    let mut commit_statement = prepare.statement.clone();
    commit_statement.phase = LanePhaseV1::Commit;
    let commit = fixture.qc(commit_statement.clone());
    let body = proposal(&fixture, &witnesses, 0, LaneJustificationV1::Opening);
    let records = [
        LaneWalRecordV1::ProposalIntent(body.clone()),
        LaneWalRecordV1::PrepareIntent {
            statement: prepare.statement.clone(),
            signer: 0,
        },
        LaneWalRecordV1::ObservePrepare(prepare.clone()),
        LaneWalRecordV1::LockAndCommit {
            prepare: prepare.clone(),
            statement: commit_statement,
            signer: 0,
        },
        LaneWalRecordV1::TimeoutIntent {
            body: LaneTimeoutBodyV1 {
                round: fixture.round(0),
                highest_prepare: Some(prepare.clone()),
            },
            signer: 0,
        },
        LaneWalRecordV1::InstallTimeout(fixture.tc(0, Some(prepare.clone()))),
        LaneWalRecordV1::Decision(commit),
    ];
    for (index, record) in records.into_iter().enumerate() {
        let native = LaneWalEnvelopeV1 {
            version: FORMAT,
            persistence_id: index as u64 + 1,
            record,
        };
        let entry = auth.wal_entry(&native).unwrap();
        let projected = auth.native_wal(&entry, &witnesses).unwrap();
        assert_eq!(projected, native);
        assert_eq!(auth.wal_entry(&projected).unwrap(), entry);
        let sign = match entry.record() {
            reducer::WalRecord::ProposalIntent(body) => {
                Some(reducer::SignableMessage::Proposal(body.clone()))
            }
            reducer::WalRecord::PrepareIntent(vote)
            | reducer::WalRecord::LockAndCommit { vote, .. } => {
                Some(reducer::SignableMessage::Vote(*vote))
            }
            reducer::WalRecord::TimeoutIntent(vote) => {
                Some(reducer::SignableMessage::TimeoutVote(vote.clone()))
            }
            _ => None,
        };
        if let Some(sign) = sign {
            let bytes = auth.native_signing_preimage(&sign, &native).unwrap();
            let expected = match &native.record {
                LaneWalRecordV1::ProposalIntent(body) => body.signature_preimage().unwrap(),
                LaneWalRecordV1::PrepareIntent { statement, .. }
                | LaneWalRecordV1::LockAndCommit { statement, .. } => {
                    statement.signature_preimage().unwrap()
                }
                LaneWalRecordV1::TimeoutIntent { body, .. } => body.signature_preimage().unwrap(),
                _ => unreachable!(),
            };
            assert_eq!(bytes, expected);
            let mut substituted = native.clone();
            substituted.record = LaneWalRecordV1::ObservePrepare(prepare.clone());
            assert!(auth.native_signing_preimage(&sign, &substituted).is_err());
        }
        assert!(
            auth.native_wal(&entry, &Witnesses(None)).is_err(),
            "every record here needs the retained value"
        );
    }
    let later = proposal(
        &fixture,
        &witnesses,
        1,
        LaneJustificationV1::Timeout(fixture.tc(0, Some(prepare))),
    );
    let entry = auth
        .wal_entry(&LaneWalEnvelopeV1 {
            version: FORMAT,
            persistence_id: 1,
            record: LaneWalRecordV1::ProposalIntent(later.clone()),
        })
        .unwrap();
    assert_eq!(
        auth.native_wal(&entry, &witnesses).unwrap().record,
        LaneWalRecordV1::ProposalIntent(later)
    );
}

#[test]
fn native_effect_projection_prepayload_timeout_needs_no_value_or_body() {
    let fixture = fixture();
    let auth = fixture.authenticator();
    let missing = Witnesses(None);
    let mut reducer = reducer::Reducer::new(
        fixture.context.clone(),
        Some(fixture.context.roster()[0].id()),
        reducer::Generation::INITIAL,
    )
    .unwrap();
    let result = reducer
        .step(reducer::Event::TimeoutElapsed {
            tag: reducer.current_tag(),
        })
        .unwrap();
    let [reducer::Effect::Persist { tag, entry }] = result.effects() else {
        panic!("timeout must persist first")
    };
    let native = auth.native_wal(entry, &missing).unwrap();
    let LaneWalRecordV1::TimeoutIntent { body, signer } = &native.record else {
        unreachable!()
    };
    assert!(body.highest_prepare.is_none());
    // This unit test covers projection. Physical fsync and issued-effect custody
    // are tested by the State-backed lane WAL suite, not synthesized here.
    let sign = reducer::SignableMessage::TimeoutVote(match entry.record() {
        reducer::WalRecord::TimeoutIntent(vote) => vote.clone(),
        _ => unreachable!(),
    });
    let preimage = auth.native_signing_preimage(&sign, &native).unwrap();
    let share = fixture.share(*signer, &preimage);
    let native_vote = LaneTimeoutVoteV1 {
        body: body.clone(),
        share,
    };
    let core_vote = auth.timeout_vote(&native_vote).unwrap();
    assert_eq!(
        auth.native_broadcast(
            &reducer::ConsensusMessageV2::TimeoutVote(core_vote),
            &missing,
            None
        )
        .unwrap(),
        LaneMessageV1::TimeoutVote(native_vote)
    );
    assert_eq!(tag.view(), 0);
    let tc = fixture.tc(0, None);
    assert_eq!(
        auth.native_broadcast(
            &reducer::ConsensusMessageV2::TimeoutCertificate(auth.tc(&tc).unwrap()),
            &missing,
            None
        )
        .unwrap(),
        LaneMessageV1::TimeoutCertificate(tc)
    );
}

#[test]
fn native_effect_projection_broadcast_checks_signatures_and_refuses_foreign_or_missing_witnesses() {
    let fixture = fixture();
    let auth = fixture.authenticator();
    let witnesses = witnesses(&fixture);
    let statement = fixture.statement(0);
    let vote = LaneVoteV1 {
        share: fixture.share(0, &statement.signature_preimage().unwrap()),
        statement,
    };
    let core_vote = auth.vote(&vote).unwrap();
    let broadcast = reducer::ConsensusMessageV2::Vote(core_vote.clone());
    assert_eq!(
        auth.native_broadcast(&broadcast, &witnesses, None).unwrap(),
        LaneMessageV1::Vote(vote)
    );
    assert!(
        auth.native_broadcast(&broadcast, &Witnesses(None), None)
            .is_err()
    );
    let invalid =
        reducer::SignedVote::new(core_vote.vote(), reducer::OpaqueSignature::new(vec![0; 96]));
    assert!(
        auth.native_broadcast(
            &reducer::ConsensusMessageV2::Vote(invalid),
            &witnesses,
            None
        )
        .is_err()
    );
    let qc = fixture.qc(fixture.statement(0));
    assert_eq!(
        auth.native_broadcast(
            &reducer::ConsensusMessageV2::QuorumCertificate(auth.qc(&qc).unwrap()),
            &witnesses,
            None
        )
        .unwrap(),
        LaneMessageV1::QuorumCertificate(qc)
    );
    let foreign = reducer::Vote::new(
        reducer::ContextId::repeat(77),
        core_vote.vote().round(),
        core_vote.vote().phase(),
        core_vote.vote().subject(),
        core_vote.vote().signer(),
    );
    let entry = reducer::WalEntry::new(
        reducer::PersistenceId::new(1),
        reducer::WalRecord::PrepareIntent(foreign),
    );
    assert!(auth.native_wal(&entry, &witnesses).is_err());
    let outsider = reducer::Vote::new(
        fixture.context.id(),
        core_vote.vote().round(),
        core_vote.vote().phase(),
        core_vote.vote().subject(),
        reducer::ValidatorId::repeat(88),
    );
    assert!(
        auth.native_wal(
            &reducer::WalEntry::new(
                reducer::PersistenceId::new(1),
                reducer::WalRecord::PrepareIntent(outsider)
            ),
            &witnesses
        )
        .is_err()
    );
}

#[test]
fn native_effect_projection_never_reconstructs_a_signed_proposal_from_lossy_tc_groups() {
    use iroha_data_model::block::lane_consensus::LaneProposalV1;
    let fixture = fixture();
    let auth = fixture.authenticator();
    let witnesses = witnesses(&fixture);
    let first_qc = fixture.qc(fixture.statement(0));
    let mut other_qc = first_qc.clone();
    let preimage = other_qc.statement.signature_preimage().unwrap();
    other_qc.shares = (1..4)
        .map(|signer| fixture.share(signer, &preimage))
        .collect();
    let tc = LaneTcV1 {
        round: fixture.round(0),
        votes: (0..3)
            .map(|signer| {
                let body = LaneTimeoutBodyV1 {
                    round: fixture.round(0),
                    highest_prepare: Some(if signer == 1 {
                        other_qc.clone()
                    } else {
                        first_qc.clone()
                    }),
                };
                let share = fixture.share(signer, &body.signature_preimage().unwrap());
                LaneTimeoutVoteV1 { body, share }
            })
            .collect(),
    };
    let body = proposal(&fixture, &witnesses, 1, LaneJustificationV1::Timeout(tc));
    let signature = fixture
        .share(body.proposer, &body.signature_preimage().unwrap())
        .signature;
    let original = LaneMessageV1::Proposal(LaneProposalV1 {
        body: body.clone(),
        signature,
    });
    let reducer::Event::ProposalReceived {
        proposal: core_signed,
        ..
    } = auth
        .event(
            &original,
            reducer::EventTag::new(1, 1, reducer::Generation::INITIAL),
        )
        .unwrap()
    else {
        unreachable!()
    };
    let broadcast = reducer::ConsensusMessageV2::Proposal(core_signed.clone());
    assert_eq!(
        auth.native_broadcast(&broadcast, &witnesses, Some(&body))
            .unwrap(),
        original
    );
    assert!(auth.native_broadcast(&broadcast, &witnesses, None).is_err());
    let reconstructed = auth
        .native_local_proposal(core_signed.proposal(), &witnesses)
        .unwrap();
    assert_ne!(
        reconstructed, body,
        "group projection retained only the first valid QC subset"
    );
    assert_eq!(
        auth.proposal_body(&reconstructed).unwrap(),
        *core_signed.proposal()
    );
    assert!(
        auth.native_broadcast(&broadcast, &witnesses, Some(&reconstructed))
            .is_err(),
        "same abstract proposal cannot substitute a different signed preimage"
    );
    let exact_intent = LaneWalEnvelopeV1 {
        version: FORMAT,
        persistence_id: 1,
        record: LaneWalRecordV1::ProposalIntent(body.clone()),
    };
    assert_eq!(
        auth.native_signing_preimage(
            &reducer::SignableMessage::Proposal(core_signed.proposal().clone()),
            &exact_intent
        )
        .unwrap(),
        body.signature_preimage().unwrap()
    );
}
