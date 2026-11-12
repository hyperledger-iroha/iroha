//! Tests of credential registration and the credential-aware seal (design §3.2, §3.3) with
//! genuine registration proofs. Binding juries here seat three and keep one alternate, so
//! the four invited seats need four proofs, made once per test binary.

use std::{collections::BTreeSet, sync::OnceLock};

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    governance::{
        anonymous_ballot::{
            BallotBodyContextV1, BallotScopeV1, CredentialRegistrationV1,
            ParliamentCredentialSourceV1, RegisterParliamentCredentialV1,
            SealedCredentialListPublishedV1,
        },
        types::{AssignmentId, BodyElectionAttemptId},
    },
    transaction::TransactionEntrypoint,
};
use iroha_parliament_ballot::{
    board::{
        BallotBoardV1, RegisteredCredentialV1, RegistrationLedgerErrorV1, RegistrationLedgerV1,
        SealErrorV1,
    },
    context::ContextErrorV1,
    inputs::ProofInputErrorV1,
    wallet::{CredentialHandleV1, prepare_registration_v1},
};

use super::{
    test_driver::{
        Answer, CONTENT, CREATED_HEIGHT, Driver, anchor, attempt, candidate, citizens,
        lifecycle_parameters, network, pipeline, policy_with,
    },
    *,
};

use AnonymousParliamentErrorV1 as Error;

pub(super) const JURY: ParliamentBody = ParliamentBody::PolicyJury;

/// A driver whose binding juries seat three and keep one alternate, drawn at its slot.
pub(super) fn drawn_small_jury() -> Driver {
    let mut parameters = lifecycle_parameters();
    for sizing in &mut parameters.body_sizes {
        if BodyDecisionModeV1::of(sizing.body) == BodyDecisionModeV1::AnonymousBallot {
            sizing.target_seats = 3;
            sizing.alternate_cap = 1;
        }
    }
    let mut driver = Driver::with(
        candidate(),
        attempt(0, RiskTierV1::Standard),
        policy_with(&parameters),
        0,
        40,
    );
    driver.qualify();
    driver.register_initial();
    driver.advance_to_slot();
    driver
}

/// The jury's election and its invited seats in rank order: primaries, then alternates.
pub(super) fn jury_invitations(
    driver: &Driver,
) -> (BodyElectionAttemptId, Vec<(AssignmentId, AccountId)>) {
    let (generation, election) = driver
        .state
        .active_election(JURY)
        .expect("the jury's election");
    let election = &driver.state.generations()[generation].elections()[election];
    let invited = election
        .primaries()
        .iter()
        .chain(election.alternates())
        .map(|seat| (seat.assignment_id, seat.member.clone()))
        .collect();
    (election.id(), invited)
}

/// The genuine registration of invited seat `index`, proved once per test binary.
pub(super) fn registration(index: usize) -> CredentialRegistrationV1 {
    static REGISTRATIONS: OnceLock<Vec<CredentialRegistrationV1>> = OnceLock::new();
    REGISTRATIONS.get_or_init(|| {
        let driver = drawn_small_jury();
        let (election, invited) = jury_invitations(&driver);
        let context = driver
            .state
            .binding_body_context(&network(), election)
            .expect("the jury's context");
        invited
            .iter()
            .zip(0x40_u8..)
            .map(|((assignment, member), secret)| {
                let handle = CredentialHandleV1::import_v1(context, &[secret; 48], None)
                    .expect("fixture credential");
                prepare_registration_v1(&handle, member, assignment).expect("genuine registration")
            })
            .collect()
    })[index]
        .clone()
}

/// The signed input that carried registration `index`.
fn source(index: u8) -> ParliamentCredentialSourceV1 {
    ParliamentCredentialSourceV1 {
        height: CREATED_HEIGHT + 50 + u64::from(index),
        input_index: u64::from(index),
        instruction_index: 0,
        entrypoint_hash: HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::prehashed(
            [0x60 + index; Hash::LENGTH],
        )),
    }
}

/// The registration input of invited seat `index`.
pub(super) fn input(driver: &Driver, index: usize) -> RegisterParliamentCredentialV1 {
    RegisterParliamentCredentialV1 {
        governance_attempt_id: driver.id(),
        election_attempt_id: jury_invitations(driver).0,
        registration: registration(index),
    }
}

/// Register the invited seats `indices` in the current block, keeping their records.
pub(super) fn register(
    driver: &mut Driver,
    records: &mut AnonymousElectionCredentialsV1,
    indices: impl IntoIterator<Item = usize>,
) {
    let (_, invited) = jury_invitations(driver);
    for index in indices {
        let (assignment, member) = &invited[index];
        // Registered in the current block.
        let source = ParliamentCredentialSourceV1 {
            height: driver.height,
            ..source(u8::try_from(index).expect("small index"))
        };
        let (accepted, record) = driver
            .state
            .register_credential(
                &network(),
                member,
                &input(driver, index),
                records,
                source,
                driver.now_ms,
            )
            .expect("register the seat's credential");
        assert_eq!(accepted, *assignment);
        assert!(
            driver
                .state
                .accepted_assignments(jury_invitations(driver).0)
                .is_ok_and(|accepted_set| accepted_set.contains(assignment))
        );
        assert_eq!(record.commitment, registration(index).commitment);
        records.push((accepted, record));
    }
}

/// Every public body of the initial generation accepts its primaries.
pub(super) fn answer_public_bodies(driver: &mut Driver) {
    for body in pipeline() {
        if BodyDecisionModeV1::of(body) == BodyDecisionModeV1::PublicFinding {
            driver.answer(body, Answer::Primaries);
        }
    }
}

/// The time of the invitation close: the earliest live item once every election is drawn.
fn close_ms(driver: &Driver) -> u64 {
    driver.state.lifecycle_items().expect("items")[0].due_ms
}

/// Plan the block of the invitation close, apply the same items with the same credentials,
/// and check the plan predicted every outcome; returns the outcomes. Only the jury's close
/// may ask for credentials.
pub(super) fn close_invitations(
    driver: &mut Driver,
    records: &AnonymousElectionCredentialsV1,
) -> Vec<AnonymousItemOutcomeV1> {
    let (jury_election, _) = jury_invitations(driver);
    let credentials = |election, _: &BTreeSet<AssignmentId>| {
        assert_eq!(
            election, jury_election,
            "only a binding jury's seal asks for credentials"
        );
        records.clone()
    };
    let close_ms = close_ms(driver);
    driver.height += 1;
    driver.now_ms = close_ms;
    let plan = driver
        .state
        .plan_due_items(&network(), driver.height, driver.now_ms, 0, credentials)
        .expect("plan");
    let mut position = 0;
    let mut outcomes = Vec::new();
    for planned in &plan {
        let at = anchor(driver.height, position, driver.now_ms);
        outcomes.push(
            match driver.state.apply_item_with_credentials(
                &network(),
                planned.item,
                at,
                credentials,
            ) {
                Ok(effect) => {
                    position += 1;
                    AnonymousItemOutcomeV1::Applied(effect)
                }
                Err(error @ (Error::StaleItem | Error::NotActive { .. })) => {
                    AnonymousItemOutcomeV1::Lapsed(Box::new(error))
                }
                Err(error) => AnonymousItemOutcomeV1::Refused(Box::new(error)),
            },
        );
    }
    assert_eq!(
        plan.into_iter()
            .map(|planned| planned.outcome)
            .collect::<Vec<_>>(),
        outcomes,
        "the plan predicts what execution applies"
    );
    outcomes
}

/// The jury's effect among `outcomes`.
pub(super) fn jury_effect(outcomes: &[AnonymousItemOutcomeV1]) -> &AnonymousItemEffectV1 {
    let boards: Vec<_> = outcomes
        .iter()
        .filter_map(|outcome| match outcome {
            AnonymousItemOutcomeV1::Applied(effect) if effect.board.is_some() => Some(effect),
            _ => None,
        })
        .collect();
    assert_eq!(boards.len(), 1, "only the binding jury has a board");
    boards[0]
}

/// The seal output the credentials of `assignments` give the sealed jury.
fn expected_published(
    driver: &Driver,
    election: BodyElectionAttemptId,
    records: &AnonymousElectionCredentialsV1,
    assignments: &[AssignmentId],
) -> SealedCredentialListPublishedV1 {
    let context = BallotBodyContextV1 {
        network_id: network(),
        governance_attempt_id: driver.id(),
        election_attempt_id: election,
        body_role: JURY,
    };
    let ledger = RegistrationLedgerV1::restore(
        context,
        records
            .iter()
            .filter(|(assignment, _)| assignments.contains(assignment))
            .map(|(assignment, record)| {
                (
                    *assignment,
                    RegisteredCredentialV1 {
                        commitment: record.commitment,
                        authorization: record.authorization,
                    },
                )
            }),
    )
    .expect("ledger");
    let sealed = driver.state.sealed_body(JURY).expect("the jury sealed");
    ledger
        .seal(assignments)
        .expect("seal")
        .published(sealed.body_instance_id, sealed.roster_root)
        .expect("published")
}

#[test]
fn a_binding_seat_is_accepted_only_with_its_verified_credential() {
    let mut driver = drawn_small_jury();
    let (election, invited) = jury_invitations(&driver);
    let id = driver.id();
    let now = driver.now_ms;
    let mut records = Vec::new();
    // A plain acceptance of a binding seat is refused.
    assert_eq!(
        driver
            .state
            .record_invitation_response(id, election, JURY, &invited[0].1, true, now),
        Err(Error::CredentialRequired)
    );
    // Another seat's registration does not verify for this authority and seat, and its
    // refusal changes nothing.
    let before = driver.state.clone();
    let refused = driver.state.register_credential(
        &network(),
        &invited[1].1,
        &input(&driver, 0),
        &records,
        source(0),
        now,
    );
    assert!(
        matches!(&refused, Err(error @ Error::Credential(_)) if !error.is_local()),
        "{refused:?}"
    );
    assert_eq!(driver.state, before);
    // A source outside finalized history is refused.
    let mut zero = source(0);
    zero.height = 0;
    assert_eq!(
        driver.state.register_credential(
            &network(),
            &invited[0].1,
            &input(&driver, 0),
            &records,
            zero,
            now,
        ),
        Err(Error::CredentialSource)
    );
    // The invited seat registers its own credential, once.
    register(&mut driver, &mut records, [0]);
    driver.assert_restores();
    assert_eq!(
        driver.state.register_credential(
            &network(),
            &invited[0].1,
            &input(&driver, 0),
            &records,
            source(0),
            now,
        ),
        Err(Error::ResponseReplay)
    );
    // A commitment registered for one seat is refused for another before its proof is
    // verified.
    assert!(matches!(
        driver.state.register_credential(
            &network(),
            &invited[1].1,
            &input(&driver, 0),
            &records,
            source(1),
            now,
        ),
        Err(Error::Credential(error)) if *error == RegistrationLedgerErrorV1::DuplicateCommitment
    ));
    // Stored credentials that are not the election's accepted seats are World's
    // corruption, never a registrant's fault.
    assert_eq!(
        driver.state.register_credential(
            &network(),
            &invited[1].1,
            &input(&driver, 1),
            &[],
            source(1),
            now,
        ),
        Err(Error::CredentialStore)
    );
    // An account without an invitation holds no seat.
    let stranger = citizens(41)
        .into_iter()
        .find(|account| !driver.citizens.contains(account))
        .expect("a citizen outside the snapshot");
    assert_eq!(
        driver.state.register_credential(
            &network(),
            &stranger,
            &input(&driver, 1),
            &records,
            source(1),
            now,
        ),
        Err(Error::UnknownInvitation)
    );
    // An election that is not a body's active one is unknown.
    let mut foreign = input(&driver, 1);
    foreign.election_attempt_id = BodyElectionAttemptId::new([0x5f; 32]);
    assert_eq!(
        driver.state.register_credential(
            &network(),
            &invited[1].1,
            &foreign,
            &records,
            source(1),
            now,
        ),
        Err(Error::UnknownElection)
    );
    // A public body's election takes no credential.
    let (generation, public) = driver
        .state
        .active_election(ParliamentBody::RulesCommittee)
        .expect("a public election");
    let public = driver.state.generations()[generation].elections()[public].id();
    assert_eq!(
        driver.state.binding_body_context(&network(), public),
        Err(Error::DecisionMode {
            body: ParliamentBody::RulesCommittee
        })
    );
    // Once the window closed, nothing is registered.
    let close = close_ms(&driver);
    assert_eq!(
        driver.state.register_credential(
            &network(),
            &invited[1].1,
            &input(&driver, 1),
            &records,
            source(1),
            close,
        ),
        Err(Error::InvitationWindowClosed)
    );
    // An attempt that ended takes no credential.
    let mut ended = driver.state.clone();
    ended.attempt.status = GovernanceAttemptStatusV1::Rejected;
    assert!(matches!(
        ended.register_credential(
            &network(),
            &invited[1].1,
            &input(&driver, 1),
            &records,
            source(1),
            now,
        ),
        Err(Error::NotActive { .. })
    ));
}

#[test]
fn a_binding_jury_seals_exactly_its_seated_credentials_and_the_plan_predicts_it() {
    let mut driver = drawn_small_jury();
    let (election, invited) = jury_invitations(&driver);
    let mut records = Vec::new();
    // Every invited seat accepts: three primaries and the alternate, who is not seated.
    register(&mut driver, &mut records, 0..4);
    answer_public_bodies(&mut driver);
    let outcomes = close_invitations(&mut driver, &records);
    let effect = jury_effect(&outcomes);
    assert_eq!(effect.close, Some(AnonymousInvitationCloseV1::Sealed));
    let board = effect.board.as_ref().expect("the jury's board");
    let state = driver.state.body(JURY).expect("the jury sealed");
    let sealed = state.sealed_body();
    let binding = state
        .roster_binding()
        .expect("a binding jury's roster binding");
    // The seal lists exactly the three primaries' credentials, never the alternate's.
    let primaries: Vec<AssignmentId> = invited[..3]
        .iter()
        .map(|(assignment, _)| *assignment)
        .collect();
    assert_eq!(
        board.published,
        expected_published(&driver, election, &records, &primaries)
    );
    assert_eq!(sealed.seats, 3);
    assert_eq!(binding.seats, 3);
    assert_eq!(board.published.list_digest, binding.list_digest);
    assert_eq!(board.board.body_instance_id, sealed.body_instance_id);
    assert_eq!(board.board.roster_root, sealed.roster_root);
    // The board starts from the facts the attempt fixed.
    assert_eq!(board.board.purpose, candidate());
    assert_eq!(board.board.proposal_content_id, CONTENT);
    assert_eq!(
        board.board.sealed_timestamp_ms,
        state.sealed_at().timestamp_ms
    );
    assert!(board.board.entries.is_empty() && board.board.closure.is_none());
    BallotBoardV1::restore(board.board.clone()).expect("a stored board restores");
    driver.assert_restores();
    // Restore binds the stored credentials and the board, under its scope, to the State.
    let scope = *BallotBoardV1::restore(board.board.clone())
        .expect("a stored board restores")
        .scope();
    let credentials = |id, _: &BTreeSet<AssignmentId>| {
        if id == election {
            records.clone()
        } else {
            Vec::new()
        }
    };
    assert_eq!(
        driver.state.validate_credentials(
            &network(),
            driver.height,
            credentials,
            [(&scope, &board.board)]
        ),
        Ok(())
    );
    assert_eq!(
        driver.state.validate_credentials(
            &network(),
            driver.height,
            credentials,
            std::iter::empty()
        ),
        Err(Error::Restore { field: "boards" })
    );
    let mut moved = board.board.clone();
    moved.sealed_timestamp_ms += 1;
    assert_eq!(
        driver.state.validate_credentials(
            &network(),
            driver.height,
            credentials,
            [(&scope, &moved)]
        ),
        Err(Error::Restore { field: "boards" })
    );
    let other_scope = BallotScopeV1::new([0x7a; 64]);
    assert_eq!(
        driver.state.validate_credentials(
            &network(),
            driver.height,
            credentials,
            [(&other_scope, &board.board)]
        ),
        Err(Error::Restore { field: "boards" })
    );
    let lost_one = |id, accepted: &BTreeSet<AssignmentId>| {
        let mut records = credentials(id, accepted);
        if !records.is_empty() {
            records.remove(0);
        }
        records
    };
    assert_eq!(
        driver.state.validate_credentials(
            &network(),
            driver.height,
            lost_one,
            [(&scope, &board.board)]
        ),
        Err(Error::CredentialStore)
    );
}

#[test]
fn a_decline_seats_the_alternate_whose_credential_the_seal_then_lists() {
    let mut driver = drawn_small_jury();
    let (election, invited) = jury_invitations(&driver);
    let id = driver.id();
    let mut records = Vec::new();
    register(&mut driver, &mut records, [0, 1, 3]);
    driver
        .state
        .record_invitation_response(id, election, JURY, &invited[2].1, false, driver.now_ms)
        .expect("decline");
    answer_public_bodies(&mut driver);
    let outcomes = close_invitations(&mut driver, &records);
    let board = jury_effect(&outcomes)
        .board
        .as_ref()
        .expect("the jury's board");
    let seated: Vec<AssignmentId> = [0, 1, 3].iter().map(|index| invited[*index].0).collect();
    assert_eq!(
        board.published,
        expected_published(&driver, election, &records, &seated)
    );
    driver.assert_restores();
}

#[test]
fn a_binding_jury_below_three_credentials_ends_without_a_roster() {
    let mut driver = drawn_small_jury();
    let (election, invited) = jury_invitations(&driver);
    let id = driver.id();
    let mut records = Vec::new();
    register(&mut driver, &mut records, [0, 1]);
    for (_, member) in &invited[2..] {
        driver
            .state
            .record_invitation_response(id, election, JURY, member, false, driver.now_ms)
            .expect("decline");
    }
    answer_public_bodies(&mut driver);
    let outcomes = close_invitations(&mut driver, &records);
    assert!(
        outcomes.iter().all(|outcome| matches!(
            outcome,
            AnonymousItemOutcomeV1::Applied(AnonymousItemEffectV1 { board: None, .. })
        )),
        "{outcomes:?}"
    );
    assert!(outcomes.iter().any(|outcome| matches!(
        outcome,
        AnonymousItemOutcomeV1::Applied(AnonymousItemEffectV1 {
            close: Some(AnonymousInvitationCloseV1::NoRoster(
                AnonymousNoRosterKindV1::InsufficientRoster
            )),
            ..
        })
    )));
    assert_eq!(driver.state.sealed_body(JURY), None);
    assert_eq!(driver.state.no_roster_bodies().get(&JURY), Some(&0));
    driver.assert_restores();
}

#[test]
fn incomplete_credentials_refuse_the_seal_and_the_plan_reports_the_refusal() {
    let mut driver = drawn_small_jury();
    let mut records = Vec::new();
    register(&mut driver, &mut records, 0..3);
    answer_public_bodies(&mut driver);
    let close_ms = close_ms(&driver);
    let jury_close = driver
        .state
        .lifecycle_items()
        .expect("items")
        .into_iter()
        .map(|due| due.item)
        .find(|item| {
            matches!(
                item,
                AnonymousLifecycleItemV1::CloseInvitations { body, .. } if *body == JURY
            )
        })
        .expect("the jury's close");
    let before = driver.state.clone();
    // A store that lost one seated credential cannot seal the jury.
    let missing = records[1..].to_vec();
    assert_eq!(
        driver.state.apply_item_with_credentials(
            &network(),
            jury_close,
            anchor(driver.height + 1, 0, close_ms),
            |_, _| missing.clone(),
        ),
        Err(Error::CredentialStore)
    );
    assert_eq!(driver.state, before, "a refused seal changes nothing");
    let plan = driver
        .state
        .plan_due_items(&network(), driver.height + 1, close_ms, 0, |_, _| {
            missing.clone()
        })
        .expect("a deterministic refusal is planned");
    assert!(plan.iter().any(|planned| planned.item == jury_close
        && planned.outcome == AnonymousItemOutcomeV1::Refused(Box::new(Error::CredentialStore))));
}

#[test]
fn only_a_node_local_proof_failure_is_local() {
    let local = ContextErrorV1::Inputs(ProofInputErrorV1::ProfileIdentity);
    assert!(Error::Seal(Box::new(SealErrorV1::Context(local.clone()))).is_local());
    assert!(Error::Credential(Box::new(RegistrationLedgerErrorV1::Context(local))).is_local());
    assert!(!Error::Seal(Box::new(SealErrorV1::MissingCredential)).is_local());
    assert!(
        !Error::Credential(Box::new(RegistrationLedgerErrorV1::DuplicateCommitment)).is_local()
    );
    assert!(!Error::CredentialStore.is_local());
    assert!(!Error::RosterBinding.is_local());
}

#[test]
fn a_closure_names_its_network_its_seal_and_the_proposal_digest() {
    let fields = ["network_id", "proposal_digest", "list_digest", "profile_id"];
    for field in fields {
        let mut driver = Driver::at_policy_reflection(RiskTierV1::Standard);
        let mut record = driver.close_record(JURY, (20, 5, 0));
        match field {
            "network_id" => {
                record.network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                    Hash::prehashed([0x11; Hash::LENGTH]),
                ));
            }
            "proposal_digest" => record.proposal_digest = Digest512::new([0x77; 64]),
            "list_digest" => record.list_digest = Digest512::new([0x78; 64]),
            _ => {
                record.profile_id =
                    iroha_data_model::governance::anonymous_ballot::ProofProfileIdV1::new(
                        [0x79; 64],
                    );
            }
        }
        let id = driver.id();
        assert_eq!(
            driver.state.close_ballot(id, record, &network(), Vec::new),
            Err(Error::Certificate(
                AnonymousGovernanceCertificateErrorV1::BallotContext { field }
            )),
            "{field}"
        );
    }
}
