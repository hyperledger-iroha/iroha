//! Named deterministic tests of the commit-attestation extension (§3.7, SR39–SR42, MA1–MA12),
//! and of the `CoreStatus` roles (Appendix E, E45). One core driven by scripted inputs; the
//! harness plays the other validators with genuine or tampered fake attestations
//! (`testing::fake_attestation`).

use super::*;
use crate::{
    message::{Defect, Status},
    preimage::{KIND_COMMIT, KIND_PREPARE},
    testing::FakeVerifier,
};

fn prop(h: &mut H, view: u64, block: &Block, justify: Option<TimeoutCert>) -> Vec<Action> {
    let p = h.proposal(view, block, justify);
    h.deliver(h.leader(view), WireMessage::Proposal(Box::new(p)))
}

fn qc_msg(h: &mut H, qc: Qc) -> Vec<Action> {
    let from = h.others(1, &[])[0];
    h.deliver(from, WireMessage::Qc(qc))
}

/// The `CommitQC`s of the core's `CommitBlock` actions.
fn committed(actions: &[Action]) -> Vec<Qc> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::CommitBlock { commit_qc, .. } => Some(commit_qc.clone()),
            _ => None,
        })
        .collect()
}

/// Whether `qc` verifies with its attestations under the harness committee of its height.
fn attested_ok(h: &H, qc: &Qc) -> bool {
    crate::crypto::Verifier::new(
        &h.v.crypto,
        &I,
        &crate::testing::TEST_EPOCH.id,
        &h.committee_at(qc.height),
    )
    .verify_qc(&FakeVerifier, qc)
    .is_ok()
}

fn unavailable(actions: &[Action]) -> usize {
    faults(actions)
        .iter()
        .filter(|f| matches!(f, LocalFault::AttestationUnavailable { .. }))
        .count()
}

/// MA7, SR39–SR42: the builder flags a payload → the leader proposes a flagged header; every
/// Commit vote of the block carries its signer's attestation, and the `CommitQC` the proxy tail
/// forms carries exactly its signers' attestations and verifies.
#[test]
fn det_a1_flagged_block_commits_with_attestations() {
    // (a) The leader copies the builder's flag into the header, and votes with it.
    let mut h = H::new(4, pick::leader(0));
    h.run_until(h.now + 1_100);
    let out = h.built_flagged(b"mint");
    let sent = proposals(&out);
    assert_eq!(sent.len(), 1, "the leader proposes");
    assert!(sent[0].header.attest, "the builder's flag is in the header");
    let b = Block {
        header: sent[0].header.clone(),
        payload: b"mint".to_vec(),
    };
    let out = h.exec_all();
    let prepare = votes_of(&out, VoteKind::Prepare);
    assert!(prepare.iter().all(|v| v.attest && v.attestation.is_none()));
    let pqc = h.qc_q(VoteKind::Prepare, 0, &b);
    assert!(pqc.attest);
    let out = qc_msg(&mut h, pqc);
    let commit = votes_of(&out, VoteKind::Commit);
    assert_eq!(commit.len(), 1, "the leader (set A) Commit-votes");
    assert!(commit[0].attest);
    let committee = h.committee();
    assert_eq!(
        crate::crypto::verify_vote_attestation(&FakeVerifier, &committee, &commit[0]),
        Ok(())
    );

    // (b) The proxy tail forms the flagged CommitQC from attested votes.
    let mut h = H::new(4, pick::proxy_tail(0));
    let b = H::flagged(h.block(0, b"mint"));
    prop(&mut h, 0, &b, None);
    h.exec_all();
    let pqc = h.qc_q(VoteKind::Prepare, 0, &b);
    qc_msg(&mut h, pqc);
    let own = h
        .core
        .mine
        .commit
        .clone()
        .expect("own Commit (P keeps it local)");
    assert!(own.attestation.is_some());
    let mut all = Vec::new();
    for signer in h.others(2, &[]) {
        let vote = h.vote(VoteKind::Commit, signer, 0, &b);
        all.extend(h.deliver(signer, WireMessage::Vote(vote)));
    }
    let formed = committed(&all);
    assert_eq!(formed.len(), 1, "committed on the formed CommitQC");
    let qc = &formed[0];
    assert!(qc.attest);
    assert_eq!(qc.attestations.len(), qc.signers.count_ones());
    assert_eq!(qc.signers.count_ones(), h.q());
    assert!(
        attested_ok(&h, qc),
        "exactly its signers' attestations, in order"
    );
    // It is broadcast as formed.
    assert!(qcs(&all).iter().any(|c| c == qc));
}

/// MA1, SR41: a Commit vote of a flagged block counts only with an attestation that verifies;
/// a forged or stripped copy leaves no state, and the genuine votes still form the certificate.
#[test]
fn det_a2_unattested_commit_votes_not_counted() {
    let mut h = H::new(4, pick::proxy_tail(0));
    let b = H::flagged(h.block(0, b"mint"));
    h.bodies.insert(h.bh(&b), b.clone());
    let pqc = h.qc_q(VoteKind::Prepare, 0, &b);
    qc_msg(&mut h, pqc);
    let own = h
        .core
        .mine
        .commit
        .clone()
        .expect("own Commit (P keeps it local)");
    assert!(own.attestation.is_some(), "own attested Commit");
    let others = h.others(3, &[]);
    let (forger, stripped_signer, borrower) = (others[0], others[1], others[2]);
    let stage = h.core.stage;
    // W: forged attestation; Y: attestation stripped by a relay.
    let mut tampered = h.vote(VoteKind::Commit, forger, 0, &b);
    if let Some(a) = tampered.attestation.as_mut() {
        let mut bytes = a.signature.as_slice().to_vec();
        bytes[0] ^= 0xff;
        a.signature = crate::message::AttestationSignature::try_from_slice(&bytes).unwrap();
    }
    h.deliver(forger, WireMessage::Vote(tampered));
    let mut stripped = h.vote(VoteKind::Commit, stripped_signer, 0, &b);
    stripped.attestation = None;
    h.deliver(stripped_signer, WireMessage::Vote(stripped));
    // Z: its own Commit with the attestation of another member.
    let mut foreign = h.vote(VoteKind::Commit, borrower, 0, &b);
    foreign.attestation = h.vote(VoteKind::Commit, forger, 0, &b).attestation;
    h.deliver(borrower, WireMessage::Vote(foreign));
    assert_eq!(h.core.votes.len(), 1, "only the own vote is pooled");
    assert_eq!(h.core.stage, stage, "no contagion from uncounted votes");
    // Y's genuine retransmission counts; still q − 1 = 2.
    let genuine = h.vote(VoteKind::Commit, stripped_signer, 0, &b);
    let out = h.deliver(stripped_signer, WireMessage::Vote(genuine));
    assert!(
        committed(&out).is_empty(),
        "a forged attestation was counted"
    );
    assert_eq!(h.core.tip.height, 0);
    let genuine = h.vote(VoteKind::Commit, forger, 0, &b);
    let out = h.deliver(forger, WireMessage::Vote(genuine));
    let formed = committed(&out);
    assert_eq!(formed.len(), 1);
    assert!(attested_ok(&h, &formed[0]));
    assert!(
        evidence(&h.all).is_empty(),
        "no evidence for unsigned attestation bytes"
    );
}

/// MA3, SR42: an attestation binds the result: attestations of `(h, bh, R′)` count neither in
/// a Commit vote for `(h, bh, R)` nor in a `CommitQC` for it.
#[test]
fn det_a3_attestation_binds_result() {
    let mut h = H::new(4, pick::proxy_tail(0));
    let b = H::flagged(h.block(0, b"mint"));
    h.bodies.insert(h.bh(&b), b.clone());
    let value = (h.bh(&b), result_of(&b));
    let other = (value.0, Hash32([0x5a; 32]));
    let pqc = h.qc_q(VoteKind::Prepare, 0, &b);
    qc_msg(&mut h, pqc);
    let o = h.others(3, &[]);
    // W's vote for R with W's attestation of R′.
    let mut vote = h.vote(VoteKind::Commit, o[0], 0, &b);
    vote.attestation = h
        .vote_value_flagged(VoteKind::Commit, o[0], 0, other, true)
        .attestation;
    h.deliver(o[0], WireMessage::Vote(vote));
    let genuine = h.vote(VoteKind::Commit, o[1], 0, &b);
    let out = h.deliver(o[1], WireMessage::Vote(genuine));
    assert!(
        committed(&out).is_empty(),
        "an attestation of another R counted"
    );

    // A CommitQC for R whose attestations were made for R′ does not commit.
    let mut h = H::new(4, pick::set_b(0));
    let b = H::flagged(h.block(0, b"mint"));
    h.bodies.insert(h.bh(&b), b.clone());
    let value = (h.bh(&b), result_of(&b));
    let signers = h.others(3, &[]);
    let mut qc = h.qc_value_flagged(VoteKind::Commit, 0, value, &signers, true);
    qc.attestations = h
        .qc_value_flagged(
            VoteKind::Commit,
            0,
            (value.0, Hash32([0x5a; 32])),
            &signers,
            true,
        )
        .attestations;
    qc_msg(&mut h, qc);
    assert_eq!(h.core.tip.height, 0, "attestations of another R accepted");
    let genuine = h.qc_value_flagged(VoteKind::Commit, 0, value, &signers, true);
    qc_msg(&mut h, genuine);
    assert_eq!(h.core.tip.height, 1);
}

/// MA2, MA11, SR42 (core): a flagged `CommitQC` whose attestations are missing, incomplete,
/// forged or from another height, or one over-aggregated beyond `q` signers, commits nothing —
/// as a `Qc`, in a `Status` or as a proposal's `parent_qc` — and the genuine one commits.
#[test]
fn det_a4_commitqc_attestations_checked_core() {
    let mut h = H::new(4, pick::set_b(0));
    let b = H::flagged(h.block(0, b"mint"));
    h.bodies.insert(h.bh(&b), b.clone());
    let genuine = h.qc_q(VoteKind::Commit, 0, &b);
    let other_height = {
        let value = (h.bh(&b), result_of(&b));
        let statement =
            preimage::att_preimage(&I, &crate::testing::TEST_EPOCH.id, 2, &value.0, &value.1);
        (h.signer_keys_of(&genuine).iter())
            .map(|k| fake_attestation(k, 2, &statement).signature)
            .collect::<Vec<_>>()
    };
    let mut forged = genuine.attestations.clone();
    let mut bytes = forged[0].as_slice().to_vec();
    bytes[3] ^= 1;
    forged[0] = crate::message::AttestationSignature::try_from_slice(&bytes).unwrap();
    let mut bad: Vec<Qc> = [
        Vec::new(),
        genuine.attestations[1..].to_vec(),
        forged,
        other_height,
    ]
    .into_iter()
    .map(|attestations| Qc {
        attestations,
        ..genuine.clone()
    })
    .collect();
    // MA11: over-aggregated by a Byzantine aggregator — `q + 1` genuine signatures and
    // attestations (a KAGEMUSHA bundle has exactly `q`).
    let everyone: Vec<ValidatorIndex> = (0..4).collect();
    bad.push(h.qc(VoteKind::Commit, 0, &b, &everyone));
    let from = h.others(1, &[])[0];
    for qc in &bad {
        qc_msg(&mut h, qc.clone());
        let status = Status {
            instance: I,
            height: 2,
            view: 0,
            committed_qc: Some(qc.clone()),
            ..Status::default()
        };
        h.deliver(from, WireMessage::Status(Box::new(status)));
        h.now += h.local.rebroadcast_interval;
        assert_eq!(
            h.core.tip.height,
            0,
            "committed on {:?}",
            qc.attestations.len()
        );
    }
    // As the parent_qc of a proposal for height 2.
    let next_leader = {
        let topo = Topology::compute(
            &h.v.crypto,
            &I,
            &crate::testing::TEST_EPOCH,
            &h.committee(),
            2,
            0,
            W,
            &[],
        );
        topo.leader(0)
    };
    let child = Block {
        header: BlockHeader {
            height: 2,
            parent_hash: h.bh(&b),
            parent_result: result_of(&b),
            proposer: next_leader,
            ..h.block(0, b"child").header
        },
        payload: b"child".to_vec(),
    };
    let child = Block {
        header: BlockHeader {
            payload_hash: preimage::payload_hash(&h.v.crypto, &child.payload),
            ..child.header
        },
        ..child
    };
    let bh = h.bh(&child);
    let ad = preimage::att_digest(&h.v.crypto, None, bad.first());
    let msg = preimage::prop_preimage(&I, &crate::testing::TEST_EPOCH.id, 2, 0, &bh, &ad);
    let p = Proposal {
        instance: I,
        height: 2,
        view: 0,
        header: child.header.clone(),
        justify: None,
        parent_qc: bad.first().cloned(),
        payload: Some(child.payload.clone()),
        sig: h.signer_of(&h.key_at(next_leader)).sign(&msg),
    };
    h.deliver(next_leader, WireMessage::Proposal(Box::new(p)));
    assert_eq!(h.core.tip.height, 0, "committed through a parent_qc");
    let out = qc_msg(&mut h, genuine.clone());
    assert_eq!(h.core.tip.height, 1);
    assert_eq!(committed(&out), vec![genuine]);
}

/// MA5, MA10, SR40: a node without attestation authority, or with one whose attestations its
/// own verifier rejects, Prepares a flagged block but sends no Commit vote for it
/// (`AttestationUnavailable` once per view); unflagged blocks are unaffected.
#[test]
fn det_a5_no_authority_abstains_from_commit_only() {
    let mut h = H::new(4, pick::set_a(0));
    h.attestor = crate::testing::FakeAttestor::without_authority([h.key_at(h.me)]);
    h.restart();
    let b = H::flagged(h.block(0, b"mint"));
    prop(&mut h, 0, &b, None);
    let out = h.exec_all();
    assert_eq!(
        votes_of(&out, VoteKind::Prepare).len(),
        1,
        "Prepare is unaffected"
    );
    let pqc = h.qc_q(VoteKind::Prepare, 0, &b);
    let out = qc_msg(&mut h, pqc);
    assert!(
        votes_of(&out, VoteKind::Commit).is_empty(),
        "no unattested Commit"
    );
    assert_eq!(unavailable(&out), 1);
    assert!(!out.iter().any(|a| matches!(a, Action::PersistSafety(_))));
    // Stage raises call try_commit again: no Commit, no second report in this view.
    let later = h.run_until(h.now + 3_000);
    assert!(votes_of(&later, VoteKind::Commit).is_empty());
    assert_eq!(unavailable(&later), 0);
    assert!(h.my_sigs(KIND_COMMIT, 1, 0).is_empty(), "nothing signed");
    // The height commits through the others; an unflagged block is Commit-voted normally.
    let cqc = h.qc_q(VoteKind::Commit, 0, &b);
    qc_msg(&mut h, cqc);
    assert_eq!(h.core.tip.height, 1);
    // MA10: a misconfigured authority (attestations its own verifier rejects) abstains too:
    // the proxy tail never pools an invalid attestation of its own, so every certificate it
    // forms verifies.
    let mut h = H::new(4, pick::proxy_tail(0));
    h.attestor = crate::testing::FakeAttestor::forging();
    h.restart();
    let b = H::flagged(h.block(0, b"mint"));
    h.bodies.insert(h.bh(&b), b.clone());
    let pqc = h.qc_q(VoteKind::Prepare, 0, &b);
    let out = qc_msg(&mut h, pqc);
    assert!(
        h.core.mine.commit.is_none(),
        "no Commit with a rejected attestation"
    );
    assert_eq!(unavailable(&out), 1);
    let mut all = Vec::new();
    for signer in h.others(3, &[]) {
        let vote = h.vote(VoteKind::Commit, signer, 0, &b);
        all.extend(h.deliver(signer, WireMessage::Vote(vote)));
    }
    let formed = committed(&all);
    assert_eq!(formed.len(), 1);
    assert!(attested_ok(&h, &formed[0]), "the formed CommitQC verifies");
    let mut h = H::new(4, pick::set_a(0));
    h.attestor = crate::testing::FakeAttestor::without_authority([h.key_at(h.me)]);
    h.restart();
    let plain = h.block(0, b"plain");
    prop(&mut h, 0, &plain, None);
    h.exec_all();
    let pqc = h.qc_q(VoteKind::Prepare, 0, &plain);
    let out = qc_msg(&mut h, pqc);
    assert_eq!(votes_of(&out, VoteKind::Commit).len(), 1);
    assert_eq!(unavailable(&h.all), 0);
}

/// MA12, SR40: an authority that needs the node's own execution of the block (KAGEMUSHA: `R`'s
/// preimage) answers `Pending` when the `PrepareQC` of a flagged block arrives first: no Commit
/// vote, nothing persisted, no `AttestationUnavailable`. Its execution completing to the
/// certified `R` makes the node Commit-vote at once with its attestation; an execution to
/// another `R` leaves it `Pending` (an `ExecutionMismatch` only).
#[test]
fn det_a9_pending_attestor_commits_after_execution() {
    for divergent in [false, true] {
        let mut h = H::new(4, pick::set_a(0));
        let executed = crate::testing::Executed::new();
        h.attestor = crate::testing::FakeAttestor::new().after_execution(executed.clone());
        h.restart();
        let b = H::flagged(h.block(0, b"mint"));
        prop(&mut h, 0, &b, None);
        assert_eq!(h.pending_exec.len(), 1, "B is executing");
        let pqc = h.qc_q(VoteKind::Prepare, 0, &b);
        let out = qc_msg(&mut h, pqc);
        assert!(
            votes_of(&out, VoteKind::Commit).is_empty(),
            "not attested yet"
        );
        assert!(!out.iter().any(|a| matches!(a, Action::PersistSafety(_))));
        assert_eq!(unavailable(&out), 0, "a pending authority is no fault");
        assert!(h.my_sigs(KIND_COMMIT, 1, 0).is_empty(), "nothing signed");
        let result = if divergent {
            Hash32([0x5a; 32])
        } else {
            result_of(&b)
        };
        executed.record(&I, &b.header.epoch, 1, &h.bh(&b), &result);
        let out = h.exec(h.bh(&b), ExecOutcome::Valid(result));
        let commit = votes_of(&out, VoteKind::Commit);
        if divergent {
            assert!(commit.is_empty(), "no Commit on another R");
            assert!(faults(&out).contains(&LocalFault::ExecutionMismatch { height: 1, view: 0 }));
            continue;
        }
        assert_eq!(commit.len(), 1, "the Commit follows the execution");
        assert!(commit[0].attest);
        assert_eq!(
            crate::crypto::verify_vote_attestation(&FakeVerifier, &h.committee(), &commit[0]),
            Ok(())
        );
        assert_eq!(unavailable(&h.all), 0);
    }
}

/// MA6, SR39: the flag is signed. A Byzantine aggregator that strips a genuine flagged
/// `CommitQC` of its attestations and clears its flag cannot make a node that lacks the
/// header commit — through a `Qc`, a `Status` or a proposal's `parent_qc`.
#[test]
fn det_a6_flag_is_signed() {
    let mut h = H::new(4, pick::set_b(0));
    let b = H::flagged(h.block(0, b"mint"));
    let genuine = h.qc_q(VoteKind::Commit, 0, &b);
    // Well-formed as an unflagged certificate: without the witness too, only the signed flag
    // can reject it (a kept witness is refused as `AttestationShape` before the signature).
    let stripped = Qc {
        attest: false,
        attestations: Vec::new(),
        attestation_witness: None,
        ..genuine.clone()
    };
    assert_eq!(
        crate::crypto::verify_attestations(&FakeVerifier, &h.committee_at(1), &stripped),
        Ok(()),
        "the stripped certificate has the shape of an unflagged one"
    );
    assert!(
        !h.core.blocks.contains_key(&h.bh(&b)),
        "the node lacks the header"
    );
    qc_msg(&mut h, stripped.clone());
    assert_eq!(h.core.tip.height, 0, "committed on a stripped certificate");
    let status = Status {
        instance: I,
        height: 2,
        view: 0,
        committed_qc: Some(stripped),
        ..Status::default()
    };
    let from = h.others(1, &[])[0];
    h.deliver(from, WireMessage::Status(Box::new(status)));
    assert_eq!(h.core.tip.height, 0);
    qc_msg(&mut h, genuine);
    assert_eq!(h.core.tip.height, 1);
}

/// Empty blocks are defects regardless of view, flag or otherwise valid authentication.
#[test]
fn det_a7_empty_proposals_are_rejected_at_every_view() {
    for view in [0, 1, 2, 7] {
        for attest in [false, true] {
            let mut h = H::new(4, pick::set_b(0));
            for next in 1..=view {
                h.enter_view(next);
            }
            let justify = h.core.high_tc.clone();
            let mut block = h.block(view, b"");
            block.header.attest = attest;
            let out = prop(&mut h, view, &block, justify);
            assert!(matches!(
                evidence(&out)[..],
                [Evidence::InvalidProposal {
                    defect: Defect::EmptyPayload,
                    ..
                }]
            ));
            assert_eq!(timeouts(&out).len(), 1, "early timeout");
            assert!(h.core.proposal.is_none());
        }
    }
}

/// MA9, SR39 with SR27/SR25: after a restart the node re-creates exactly its signed Prepare
/// (flag included) and its attested Commit, byte for byte.
#[test]
fn det_a8_restart_resends_identical_attested_votes() {
    let mut h = H::new(4, pick::set_a(0));
    let b = H::flagged(h.block(0, b"mint"));
    prop(&mut h, 0, &b, None);
    let out = h.exec_all();
    let prepare = votes_of(&out, VoteKind::Prepare)[0].clone();
    assert!(prepare.attest);
    let pqc = h.qc_q(VoteKind::Prepare, 0, &b);
    let out = qc_msg(&mut h, pqc);
    let commit = votes_of(&out, VoteKind::Commit)[0].clone();
    assert!(commit.attestation.is_some());
    let out = h.restart();
    assert_eq!(
        h.core.mine.prepare.as_ref(),
        Some(&prepare),
        "identical Prepare"
    );
    assert_eq!(
        votes_of(&out, VoteKind::Commit),
        vec![commit],
        "identical Commit"
    );
    assert_eq!(
        h.my_sigs(KIND_PREPARE, 1, 0).len(),
        1,
        "one Prepare preimage"
    );
    assert_eq!(h.my_sigs(KIND_COMMIT, 1, 0).len(), 1, "one Commit preimage");
}

/// E45: `CoreStatus` reports the leader and proxy tail of the round and the lock's view;
/// neither role while awaiting the next configuration.
#[test]
fn status_reports_roles_and_lock_view() {
    let mut h = H::new(4, pick::set_a(0));
    let status = h.core.status();
    assert_eq!(status.leader, Some(h.key_at(h.leader(0))));
    assert_eq!(status.proxy_tail, Some(h.key_at(h.proxy_tail(0))));
    assert_eq!(status.high_qc_view, None);
    let b = h.block(0, b"B");
    let pqc = h.qc_q(VoteKind::Prepare, 0, &b);
    qc_msg(&mut h, pqc);
    assert_eq!(h.core.status().high_qc_view, Some(0));
    h.enter_view(1);
    let status = h.core.status();
    assert_eq!(status.leader, Some(h.key_at(h.leader(1))));
    assert_eq!(status.proxy_tail, Some(h.key_at(h.proxy_tail(1))));
    // Awaiting (apply held back two heights): no roles.
    h.auto_apply = false;
    h.commit_heights(2);
    let block = h.block(0, b"C");
    h.bodies.insert(h.bh(&block), block.clone());
    let cqc = h.qc_q(VoteKind::Commit, 0, &block);
    qc_msg(&mut h, cqc);
    assert!(h.core.awaiting);
    let status = h.core.status();
    assert_eq!((status.leader, status.proxy_tail), (None, None));
}
