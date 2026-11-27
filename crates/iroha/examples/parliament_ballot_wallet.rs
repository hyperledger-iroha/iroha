//! Cast one candidate Parliament ballot with a local credential and the Rust SDK.
//!
//! Run `cargo run --release -p iroha --example parliament_ballot_wallet -- CLIENT.toml WALLET_DIR`.
//! Each credential has one owner-only directory holding its 48-byte exported `credential`
//! and `sealed-board.nrt`, the canonical board record from the wallet's authenticated seal.
//! Keep this directory, including its durable choice, across retries. Use a dedicated ballot
//! connection. This example uses direct transport, which exposes network metadata.

use std::{
    io::{self, BufRead, Write},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use eyre::{Result, WrapErr, bail, ensure, eyre};
use iroha::{
    client::Client,
    config::Config,
    data_model::governance::anonymous_ballot::{
        BALLOT_BOARD_RECORD_MAX_BYTES_V1, BallotChoiceV1, ParliamentBallotBoardRecordV1,
    },
};
use iroha_fs::{PrivateDirectory, PublishMode};
use iroha_parliament_ballot::{
    ballot::CastingWindowStatusV1,
    board::BallotBoardV1,
    context::ScopedBallotContextV1,
    wallet::{
        BallotDisclosureAcknowledgementV1, BallotDisclosureV1, BallotPreparationOptionsV1,
        CREDENTIAL_SECRET_BYTES, CredentialHandleV1, DisclosureStatementV1, JobOutcomeV1,
        spawn_ballot_preparation_v1,
    },
};
use zeroize::Zeroizing;

const INTENT: &str = "ballot-intent.nrt";
// A V1 lock contains fixed-size fields and a Norito frame.
const MAX_INTENT_BYTES: usize = 4096;

fn checked_open_board(
    seal: &ParliamentBallotBoardRecordV1,
    current: ParliamentBallotBoardRecordV1,
    now_ms: u64,
) -> Result<BallotBoardV1> {
    ensure!(
        current.context == seal.context
            && current.body_instance_id == seal.body_instance_id
            && current.purpose == seal.purpose
            && current.proposal_content_id == seal.proposal_content_id
            && current.roster_root == seal.roster_root
            && current.registration_mapping_digest == seal.registration_mapping_digest
            && current.sealed_timestamp_ms == seal.sealed_timestamp_ms,
        "the peer's board differs from the authenticated seal"
    );
    let board = BallotBoardV1::restore(current)?;
    ensure!(
        board.status_at(now_ms) == CastingWindowStatusV1::Open,
        "casting is closed or has yet to open"
    );
    Ok(board)
}

fn read_choice(
    context: &ScopedBallotContextV1,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<(BallotChoiceV1, BallotDisclosureAcknowledgementV1)> {
    let disclosure = BallotDisclosureV1::for_context_v1(context);
    writeln!(
        output,
        "Proposal digest {}",
        hex::encode(context.context().proposal_digest.as_bytes())
    )?;
    writeln!(
        output,
        "Body {:?}, scope {}",
        context.body_context().body_role,
        hex::encode(context.scope().as_bytes())
    )?;
    for statement in disclosure.render() {
        writeln!(output, "{statement}")?;
    }
    writeln!(
        output,
        "To acknowledge these statements and lock your choice, type aye, nay, or abstain."
    )?;
    output.flush()?;
    let mut answer = String::new();
    input.read_line(&mut answer)?;
    let choice = match answer.trim_end() {
        "aye" => BallotChoiceV1::Aye,
        "nay" => BallotChoiceV1::Nay,
        "abstain" => BallotChoiceV1::Abstain,
        _ => bail!("ballot cancelled before locking a choice"),
    };
    Ok((choice, disclosure.acknowledge(&DisclosureStatementV1::ALL)?))
}

fn persist_choice(
    directory: &PrivateDirectory,
    context: &ScopedBallotContextV1,
    choice: BallotChoiceV1,
    acknowledgement: &BallotDisclosureAcknowledgementV1,
) -> Result<CredentialHandleV1> {
    let secret = directory.read("credential", CREDENTIAL_SECRET_BYTES)?;
    let secret = secret
        .as_slice()
        .try_into()
        .wrap_err("wrong credential length")?;
    let previous = directory.read_optional(INTENT, MAX_INTENT_BYTES)?;
    let mut handle = CredentialHandleV1::import_v1(
        *context.body_context(),
        secret,
        previous.as_ref().map(|bytes| bytes.as_slice()),
    )?;
    let intent = handle.lock_choice_v1(context, choice, acknowledgement)?;
    if previous.is_none() {
        let record = Zeroizing::new(intent.record_bytes()?);
        match directory.write_atomic(INTENT, &record, PublishMode::CreateNew) {
            Ok(()) => {}
            // Another invocation might have locked a different statement. Compare the
            // stored bytes with this invocation's intent before releasing either proof.
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
    }
    directory.sync()?;
    let record = directory.read(INTENT, MAX_INTENT_BYTES)?;
    handle.confirm_choice_persisted_v1(&record)?;
    Ok(handle)
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let [config_path, directory_path] = args.as_slice() else {
        bail!("usage: parliament_ballot_wallet CLIENT.toml WALLET_DIR");
    };
    let config = Config::load_file(config_path)?;
    // The SDK's public board read inherits generic headers. Begin with a fresh transport
    // and discard every configured credential on both this read and the ballot POST.
    let mut builder = Client::builder(config).api_token(None);
    builder.headers.clear();
    let client = builder.build()?;
    let directory = PrivateDirectory::open(directory_path)?;
    cast(
        &client,
        &directory,
        &mut io::stdin().lock(),
        &mut io::stderr(),
        BallotPreparationOptionsV1::default(),
    )?;
    println!("Peer returned 202. Confirm inclusion from the whole public board.");
    Ok(())
}

fn cast(
    client: &Client,
    directory: &PrivateDirectory,
    input: &mut impl BufRead,
    output: &mut impl Write,
    options: BallotPreparationOptionsV1,
) -> Result<()> {
    let bytes = directory.read("sealed-board.nrt", BALLOT_BOARD_RECORD_MAX_BYTES_V1)?;
    let seal: ParliamentBallotBoardRecordV1 = norito::decode_canonical(&bytes)?;
    let sealed = BallotBoardV1::restore(seal.clone())?;
    ensure!(
        sealed.context().network_id() == client.network_id(),
        "the authenticated seal belongs to another network"
    );
    let current = client.anonymous_ballot_board(sealed.scope())?;
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .try_into()?;
    let board = checked_open_board(&seal, norito::decode_canonical(&current)?, now_ms)?;
    let (choice, acknowledgement) = read_choice(board.context(), input, output)?;
    let handle = persist_choice(directory, board.context(), choice, &acknowledgement)?;
    let job = spawn_ballot_preparation_v1(
        &handle,
        Arc::clone(board.context()),
        &acknowledgement,
        options,
    )?;
    writeln!(
        output,
        "Preparing the proof and stamp. Type submit to send when ready, or cancel to discard."
    )?;
    output.flush()?;
    let mut answer = String::new();
    input.read_line(&mut answer)?;
    if answer.trim_end() != "submit" {
        // Dropping the job requests cancellation, joins and discards any completed proof.
        drop(job);
        bail!("preparation cancelled; the durable choice is retained");
    }
    let submission = match job.finish() {
        JobOutcomeV1::Published(submission) => submission,
        JobOutcomeV1::Failed(error) => return Err(error.into()),
        JobOutcomeV1::Discarded(_) => bail!("ballot preparation was cancelled"),
        JobOutcomeV1::WorkerPanicked => return Err(eyre!("ballot preparation worker failed")),
    };
    client.submit_anonymous_ballot(submission.envelope())
}

#[cfg(test)]
mod tests {
    use iroha::data_model::governance::anonymous_ballot::{BallotScopeV1, NullifierV1};
    use iroha::http::{HttpTransport, Method, Response, TransportFuture, TransportRequest};
    use iroha_parliament_ballot::ballot::{
        AdmissionScopeV1, AdmissionStateV1, precheck_admission_v1,
    };
    use std::{
        num::NonZeroUsize,
        path::Path,
        sync::{
            OnceLock,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use super::*;
    use iroha::data_model::{
        NetworkId,
        block::BlockHeader,
        governance::{
            GovernanceAnchorV1,
            anonymous_ballot::{
                BallotBodyContextV1, Digest512, FrozenParliamentAttemptPolicyV1,
                ParliamentBallotParametersV1, ParliamentBallotPurposeV1,
                ParliamentCandidateTestingPolicyV1, ParliamentLifecycleParametersV1,
                ProofProfileIdV1, SealedCredentialListV1,
            },
            types::{
                BodyElectionAttemptId, BodyInstanceId, GovernanceAttemptId, ParliamentBody,
                ProposalContentId, RiskTierV1,
            },
        },
        prelude::{Hash, HashOf},
    };
    use iroha_parliament_ballot::{context::build_context_v1, inputs::body_digest_v1};

    fn seal() -> ParliamentBallotBoardRecordV1 {
        static RECORD: OnceLock<ParliamentBallotBoardRecordV1> = OnceLock::new();
        RECORD
            .get_or_init(|| {
                let body = BallotBodyContextV1 {
                    network_id: NetworkId::from_genesis_hash(
                        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x77; 32])),
                    ),
                    governance_attempt_id: GovernanceAttemptId::new([1; 32]),
                    election_attempt_id: BodyElectionAttemptId::new([2; 32]),
                    body_role: ParliamentBody::PolicyJury,
                };
                let commitments = (1..=3)
                    .map(|n| {
                        *CredentialHandleV1::import_v1(body, &[n; CREDENTIAL_SECRET_BYTES], None)
                            .expect("credential")
                            .commitment()
                    })
                    .collect();
                let list = SealedCredentialListV1::from_unsorted(
                    ProofProfileIdV1::V1,
                    body_digest_v1(&body).expect("digest"),
                    commitments,
                )
                .expect("list");
                let policy = FrozenParliamentAttemptPolicyV1::freeze_v1(
                    &[ParliamentBody::PolicyJury, ParliamentBody::ConfirmationJury],
                    &ParliamentLifecycleParametersV1::default(),
                    Some(&ParliamentBallotParametersV1::default()),
                    1000,
                    3600,
                )
                .expect("attempt")
                .ballot_policy(RiskTierV1::Standard, body.body_role)
                .expect("policy");
                ParliamentBallotBoardRecordV1 {
                    purpose: ParliamentBallotPurposeV1::CandidateTesting(
                        ParliamentCandidateTestingPolicyV1::CANDIDATE_V1,
                    ),
                    context: build_context_v1(body, Digest512::new([3; 64]), &list, policy)
                        .expect("context"),
                    body_instance_id: BodyInstanceId::new([4; 32]),
                    proposal_content_id: ProposalContentId::new([5; 32]),
                    roster_root: Digest512::new([6; 64]),
                    registration_mapping_digest: Digest512::new([7; 64]),
                    sealed_timestamp_ms: 1000,
                    opening_at_ms: None,
                    opening: None,
                    casting_clock: None,
                    entries: vec![],
                    closure: None,
                }
            })
            .clone()
    }

    #[test]
    fn board_requires_the_authenticated_seal_and_an_open_window() {
        let sealed = seal();
        assert!(checked_open_board(&sealed, sealed.clone(), 0).is_err());
        let mut opened = sealed.clone();
        let at = sealed.sealed_timestamp_ms + sealed.context.policy.binding.vote_notice_ms;
        opened.opening_at_ms = Some(at);
        opened.opening = Some(GovernanceAnchorV1 {
            height: 10,
            position: 0,
            timestamp_ms: at,
        });
        let bytes = norito::core::to_bytes_bounded(&opened, BALLOT_BOARD_RECORD_MAX_BYTES_V1)
            .expect("Torii board response");
        let decoded = norito::decode_canonical(&bytes).expect("decode Torii frame");
        assert!(checked_open_board(&sealed, decoded, at).is_ok());
        opened.context.list_digest = Digest512::new([0; 64]);
        assert!(checked_open_board(&sealed, opened.clone(), at).is_err());
        // Even matching expected bytes must pass the protocol's recomputation.
        assert!(checked_open_board(&opened, opened.clone(), at).is_err());
    }

    #[test]
    fn local_acknowledgement_and_durable_intent_survive_restart() {
        let board = BallotBoardV1::restore(seal()).expect("seal");
        let context = board.context();
        assert!(read_choice(context, &mut &b""[..], &mut Vec::new()).is_err());
        let mut output = Vec::new();
        let (choice, acknowledgement) =
            read_choice(context, &mut &b"aye\n"[..], &mut output).expect("acknowledged choice");
        let text = String::from_utf8(output).expect("disclosure");
        for statement in BallotDisclosureV1::for_context_v1(context).render() {
            assert!(text.contains(&statement));
        }
        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join("credential-owner");
        let owner = PrivateDirectory::open_or_create(&path).expect("private directory");
        owner
            .write_atomic(
                "credential",
                &[1; CREDENTIAL_SECRET_BYTES],
                PublishMode::CreateNew,
            )
            .expect("credential");
        let handle = persist_choice(&owner, context, choice, &acknowledgement).expect("new lock");
        assert!(handle.is_choice_persisted());
        drop(owner);
        let owner = PrivateDirectory::open(&path).expect("re-open");
        assert_eq!(
            persist_choice(&owner, context, choice, &acknowledgement)
                .expect("retry")
                .choice_lock(),
            handle.choice_lock()
        );
        assert!(persist_choice(&owner, context, BallotChoiceV1::Nay, &acknowledgement).is_err());
        owner
            .write_atomic(INTENT, b"truncated", PublishMode::Replace)
            .expect("corrupt test record");
        assert!(persist_choice(&owner, context, choice, &acknowledgement).is_err());
    }

    #[derive(Debug)]
    struct Peer {
        opened: BallotBoardV1,
        handled: AtomicUsize,
    }

    impl AdmissionStateV1 for Peer {
        fn network_id(&self) -> NetworkId {
            *self.opened.context().network_id()
        }

        fn scope(&self, scope: &BallotScopeV1) -> Option<AdmissionScopeV1> {
            (scope == self.opened.scope()).then(|| AdmissionScopeV1 {
                context: Arc::clone(self.opened.context()),
                window: self
                    .opened
                    .status_at(self.opened.opening().unwrap().timestamp_ms),
            })
        }

        fn nullifier_committed(&self, _: &BallotScopeV1, _: &NullifierV1) -> bool {
            false
        }

        fn verified_pending_holder(&self, _: &BallotScopeV1, _: &NullifierV1) -> bool {
            self.handled.load(Ordering::SeqCst) > 0
        }
    }

    impl HttpTransport for Peer {
        fn send_blocking(&self, request: TransportRequest) -> Result<Response<Vec<u8>>> {
            let board = &self.opened;
            ensure!(
                request
                    .headers
                    .iter()
                    .all(|(k, _)| matches!(k.as_str(), "accept" | "content-type")),
                "unexpected request headers"
            );
            match request.method {
                Method::GET => {
                    ensure!(
                        request.url.path()
                            == format!("/v1/gov/anonymous-ballots/boards/{}", board.scope()),
                        "wrong board read"
                    );
                    Ok(Response::builder()
                        .status(200)
                        .body(norito::core::to_bytes_bounded(
                            &self.opened.record(),
                            BALLOT_BOARD_RECORD_MAX_BYTES_V1,
                        )?)?)
                }
                Method::POST => {
                    ensure!(
                        request.url.path() == "/v1/gov/anonymous-ballots",
                        "wrong submit route"
                    );
                    let ballot = precheck_admission_v1(&request.body, self)?.verify_proof()?;
                    ballot.recheck(self)?;
                    ensure!(
                        ballot.envelope().ballot.vote == BallotChoiceV1::Aye,
                        "wrong choice"
                    );
                    self.handled.fetch_add(1, Ordering::SeqCst);
                    Ok(Response::builder().status(202).body(vec![])?)
                }
                _ => bail!("unexpected method"),
            }
        }
        fn send(&self, request: TransportRequest) -> TransportFuture<'_> {
            Box::pin(async move { self.send_blocking(request) })
        }
    }

    fn wallet_with_peer(temp: &tempfile::TempDir) -> Result<(Client, PrivateDirectory, Arc<Peer>)> {
        let sealed = seal();
        let owner = PrivateDirectory::open_or_create(temp.path().join("wallet"))?;
        owner.write_atomic(
            "sealed-board.nrt",
            &norito::encode_canonical(&sealed)?,
            PublishMode::CreateNew,
        )?;
        owner.write_atomic(
            "credential",
            &[1; CREDENTIAL_SECRET_BYTES],
            PublishMode::CreateNew,
        )?;
        let config_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/client.toml");
        let mut config = Config::load_file(config_path)?;
        config.network_id = sealed.context.body_context.network_id;
        let mut opened = sealed;
        let at = opened.sealed_timestamp_ms + opened.context.policy.binding.vote_notice_ms;
        opened.opening_at_ms = Some(at);
        opened.opening = Some(GovernanceAnchorV1 {
            height: 8,
            position: 0,
            timestamp_ms: at,
        });
        let peer = Arc::new(Peer {
            opened: BallotBoardV1::restore(opened)?,
            handled: AtomicUsize::new(0),
        });
        let mut builder = Client::builder(config).api_token(None);
        builder.headers.clear();
        let client = builder.http_transport(peer.clone()).build()?;
        Ok((client, owner, peer))
    }

    #[test]
    fn cancellation_joins_the_job_without_posting_or_losing_the_choice() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let (client, owner, peer) = wallet_with_peer(&temp)?;
        assert!(
            cast(
                &client,
                &owner,
                &mut &b"aye\ncancel\n"[..],
                &mut Vec::new(),
                BallotPreparationOptionsV1 {
                    stamp_threads: NonZeroUsize::MIN
                }
            )
            .is_err()
        );
        assert_eq!(peer.handled.load(Ordering::SeqCst), 0);
        assert!(owner.read_optional(INTENT, MAX_INTENT_BYTES)?.is_some());
        Ok(())
    }

    #[test]
    #[ignore = "mines a fresh 28-bit admission stamp with production entropy"]
    fn whole_board_to_durable_choice_to_verified_submission() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let (client, owner, peer) = wallet_with_peer(&temp)?;
        cast(
            &client,
            &owner,
            &mut &b"aye\nsubmit\n"[..],
            &mut Vec::new(),
            BallotPreparationOptionsV1 {
                stamp_threads: NonZeroUsize::new(4).unwrap(),
            },
        )?;
        assert_eq!(peer.handled.load(Ordering::SeqCst), 1);
        Ok(())
    }
}
