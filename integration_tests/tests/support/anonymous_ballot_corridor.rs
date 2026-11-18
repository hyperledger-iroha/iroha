//! A post-activation candidate vote on the same four disposable validators and their retained stores.

use std::{
    collections::BTreeMap,
    io::Write as _,
    num::NonZeroU64,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use eyre::{Context as _, Result, ensure, eyre};
use iroha::{
    blocking::Client,
    crypto::{Algorithm, ExposedPrivateKey, KeyPair},
    data_model::prelude::*,
};
use iroha_core::governance::parliament_anonymous::{
    AnonymousAttemptRejectionV1, AnonymousParliamentAttemptStateV1,
};
use iroha_data_model::{
    block::{consensus::SumeragiRootScope, execution_output::ExecutionOutputV1},
    governance::{
        GovernanceAnchorV1,
        anonymous_ballot::*,
        types::{DeliberationPhaseV1, GovernanceAttemptId, ParliamentBody, ProposalKind},
    },
    isi::governance::{
        ParliamentEndorsePublicFindingV1, ParliamentInvitationDecisionV1,
        ParliamentRecordInvitationResponseV1, RegisterCitizen,
    },
    parameter::{
        Parameter,
        system::{SumeragiNposParameters, SumeragiParameter},
    },
    sumeragi_finality::SumeragiFinalityVerifier,
};
use iroha_fs::{PrivateDirectory, PublishMode};
use iroha_test_network::{Network, NetworkBuilder, NetworkPeer};
use iroha_test_samples::ALICE_ID;

use super::harness;

const CITIZENS: usize = 24;
const POLICY: ParliamentBody = ParliamentBody::PolicyJury;
const CASTING_WINDOW_MS: u64 = 3_600_000;
const INVITATION_MS: u64 = 600_000;

fn citizen(index: usize) -> Result<KeyPair> {
    Ok(KeyPair::try_from_seed(
        format!("anonymous-four-peer-voter-{index}").into_bytes(),
        Algorithm::Ed25519,
    )?)
}

/// Commit candidate testing and finite windows in the positive disposable network's original genesis.
pub(super) fn genesis(mut builder: NetworkBuilder) -> Result<NetworkBuilder> {
    let mut lifecycle = ParliamentLifecycleParametersV1::default();
    for sizing in &mut lifecycle.body_sizes {
        sizing.target_seats = 3;
        sizing.alternate_cap = 0;
    }
    lifecycle.lifecycle = FrozenLifecyclePolicyV1 {
        qualification_window_ms: 1,
        invitation_window_ms: INVITATION_MS,
        orientation_window_ms: 1,
        evidence_window_ms: 1,
        questions_window_ms: 1,
        responses_window_ms: 1,
        deliberation_window_ms: 1,
        public_finding_window_ms: INVITATION_MS,
        enactment_delay_ms: 1,
    };
    let ballot = ParliamentBallotParametersV1 {
        vote_notice_ms: 0,
        casting_window_ms: CASTING_WINDOW_MS,
        ..Default::default()
    };
    // Freeze the native beacon cadence in both of its original genesis commitments.
    let mut npos = SumeragiNposParameters::default();
    npos.epoch_length_blocks = NonZeroU64::new(8).expect("positive epoch");
    npos.evidence_horizon_blocks = 16;
    npos.slashing_delay_blocks = 8;
    npos.validate()
        .map_err(|e| eyre!("invalid four-peer native epoch: {e}"))?;
    builder = builder
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::EpochLengthBlocks(npos.epoch_length_blocks),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Custom(
            npos.into_custom_parameter(),
        )));
    for value in [
        ParliamentCandidateTestingPolicyV1::CANDIDATE_V1.to_custom_parameter()?,
        lifecycle.to_custom_parameter()?,
        ballot.to_custom_parameter()?,
    ] {
        builder = builder.with_genesis_instruction(SetParameter::new(Parameter::Custom(value)));
    }
    let escrow = AccountId::new(citizen(CITIZENS)?.public_key().clone());
    let xor = SumeragiNposParameters::default().xor_asset_definition_id;
    builder = builder
        .with_genesis_instruction(Register::account(Account::new(escrow.clone())))
        .with_config_layer(move |cfg| {
            cfg.write(["gov", "citizenship_asset_id"], xor.to_string())
                .write(["gov", "citizenship_escrow_account"], escrow.to_string());
        });
    for index in 0..CITIZENS {
        builder = builder.with_genesis_instruction(Register::account(Account::new(
            AccountId::new(citizen(index)?.public_key().clone()),
        )));
    }
    Ok(builder)
}

fn fund_citizens(network: &Network) -> Result<BTreeMap<AccountId, Client>> {
    let admin = network.client();
    let asset = AssetId::new(
        SumeragiNposParameters::default().xor_asset_definition_id,
        ALICE_ID.clone(),
    );
    let bond = iroha_config::parameters::defaults::governance::citizenship_bond_amount();
    let allocation = bond.try_add(&Quantity::from(10_000u32))?;
    let mut citizens = BTreeMap::new();
    for index in 0..CITIZENS {
        let key = citizen(index)?;
        let owner = AccountId::new(key.public_key().clone());
        harness::submit_applied(
            &admin,
            Transfer::asset_quantity(asset.clone(), allocation.clone(), owner.clone()).into(),
        )?;
        let client = network.peers()[0].client_for(&owner, key.private_key().clone());
        harness::submit_applied(
            &client,
            RegisterCitizen {
                owner: owner.clone(),
                amount: bond.clone(),
            }
            .into(),
        )?;
        citizens.insert(owner, client);
    }
    Ok(citizens)
}

fn carry(network: &Network) -> Result<()> {
    let client = network.client();
    harness::carry_to_height(
        network,
        &client,
        harness::tip(&client)? + 1,
        "anonymous Parliament lifecycle",
    )
}

fn read(peer: &NetworkPeer, id: GovernanceAttemptId) -> Result<AnonymousParliamentAttemptStateV1> {
    let client = peer.client();
    let bytes = client.client().anonymous_parliament_attempt(&id)?;
    let attempt: AnonymousParliamentAttemptStateV1 = norito::decode_canonical(&bytes)?;
    attempt.validate(client.client().network_id())?;
    ensure!(attempt.attempt().id == id, "peer returned another attempt");
    Ok(attempt)
}

fn transition(
    c: &Client,
    id: GovernanceAttemptId,
    transition: AnonymousParliamentTransitionV1,
) -> Result<()> {
    harness::submit_applied(
        c,
        SubmitAnonymousParliamentTransitionV1 {
            governance_attempt_id: id,
            transition,
        }
        .into(),
    )?;
    Ok(())
}

fn public_example(
    dir: &Path,
    name: &str,
    args: &[&std::ffi::OsStr],
    input: Option<&[u8]>,
) -> Result<Vec<u8>> {
    let mut child = Command::new(dir.join(name))
        .args(args)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .wrap_err_with(|| format!("start the candidate's {name} example from {dir:?}"))?;
    if let Some(bytes) = input {
        child
            .stdin
            .take()
            .ok_or_else(|| eyre!("missing wallet input"))?
            .write_all(bytes)?;
    }
    let output = child.wait_with_output()?;
    ensure!(output.status.success(), "{name} exited {}", output.status);
    Ok(output.stdout)
}

fn store(dir: &PrivateDirectory, name: &str, bytes: &[u8]) -> Result<()> {
    dir.write_atomic(name, bytes, PublishMode::CreateNew)?;
    dir.sync()?;
    Ok(())
}

fn vote_config(
    dir: &PrivateDirectory,
    network: &Network,
    peer: &NetworkPeer,
    name: &str,
) -> Result<()> {
    // The stock client requires a key in its local configuration. This throwaway key has
    // no registered account, money or permission and neither public ballot route sends it.
    let ephemeral = KeyPair::random();
    let config = format!(
        "chain = {:?}\nnetwork_id = {:?}\ntorii_url = {:?}\n[account]\nchain_discriminant = 753\npublic_key = {:?}\nprivate_key = {:?}\n",
        network.chain_id().to_string(),
        network.network_id().to_string(),
        peer.torii_url(),
        ephemeral.public_key().to_string(),
        ExposedPrivateKey(ephemeral.private_key().clone()).to_string()
    );
    store(dir, name, config.as_bytes())
}

fn invitations(
    network: &Network,
    id: GovernanceAttemptId,
    citizens: &BTreeMap<AccountId, Client>,
    root: &Path,
    examples: &Path,
) -> Result<(PathBuf, BallotBodyContextV1)> {
    let peer = &network.peers()[0];
    let state = read(peer, id)?;
    let generation = state
        .generations()
        .first()
        .ok_or_else(|| eyre!("initial sortition is missing"))?;
    let mut voter = None;
    for election in generation.elections() {
        ensure!(
            election.primaries().len() == 3,
            "the real native pulse did not draw three primaries for {:?}",
            election.body()
        );
        for seat in election.primaries() {
            let signer = citizens
                .get(&seat.member)
                .ok_or_else(|| eyre!("missing the invited citizen signer"))?;
            if BodyDecisionModeV1::of(election.body()) == BodyDecisionModeV1::AnonymousBallot {
                let body = state.binding_body_context(&network.network_id(), election.id())?;
                let assignment = hex::encode(seat.assignment_id.as_bytes());
                let wallet = root.join(&assignment);
                let directory = PrivateDirectory::open_or_create(&wallet)?;
                store(&directory, "body.nrt", &norito::encode_canonical(&body)?)?;
                let public = public_example(
                    examples,
                    "parliament_credential_draft",
                    &[
                        network.network_id().to_string().as_ref(),
                        seat.member.to_string().as_ref(),
                        wallet.join("body.nrt").as_os_str(),
                        assignment.as_ref(),
                        wallet.as_os_str(),
                    ],
                    None,
                )?;
                let registration: RegisterParliamentCredentialV1 =
                    norito::json::from_slice(&public)?;
                ensure!(
                    registration.governance_attempt_id == id
                        && registration.election_attempt_id == election.id(),
                    "registration draft differs from the invitation"
                );
                harness::submit_applied(signer, registration.into())?;
                if election.body() == POLICY && voter.is_none() {
                    voter = Some((wallet, body));
                }
            } else {
                transition(
                    signer,
                    id,
                    AnonymousParliamentTransitionV1::RecordInvitationResponse(
                        ParliamentRecordInvitationResponseV1 {
                            election_attempt_id: election.id(),
                            body: election.body(),
                            decision: ParliamentInvitationDecisionV1::Accept,
                        },
                    ),
                )?;
            }
        }
    }
    voter.ok_or_else(|| eyre!("no Policy Jury voter registered a credential"))
}

fn unix_ms() -> Result<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .try_into()?)
}

fn wait_until(ms: u64) -> Result<()> {
    std::thread::sleep(Duration::from_millis(ms.saturating_sub(unix_ms()?)));
    Ok(())
}

fn govern_to_opening(
    network: &Network,
    id: GovernanceAttemptId,
    citizens: &BTreeMap<AccountId, Client>,
) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(1_800);
    loop {
        ensure!(
            Instant::now() < deadline,
            "the public bodies did not reach the jury opening"
        );
        let state = read(&network.peers()[0], id)?;
        if state.opening_at_ms(POLICY).is_some() {
            carry(network)?;
            carry(network)?;
            return Ok(());
        }
        let mut submitted = false;
        for required in &state.policy().required_bodies {
            let Some(body) = state.body(required.body) else {
                continue;
            };
            if required.decision_mode == BodyDecisionModeV1::AnonymousBallot
                || !body
                    .current_phase()
                    .is_some_and(|p| p.phase == DeliberationPhaseV1::Reflection)
                || !body.finding().is_some_and(|f| f.decision().is_none())
            {
                continue;
            }
            let instance = body.sealed_body().body_instance_id;
            let result_root: [u8; 32] = iroha_crypto::Hash::new(norito::encode_canonical(&(
                id,
                instance,
                "four-peer public finding",
            ))?)
            .into();
            let quorum = iroha_data_model::governance::types::parliament_quorum_seats_v1(
                u32::from(body.sealed_body().seats),
            );
            for seat in body.seats().iter().take(usize::try_from(quorum)?) {
                if body
                    .finding()
                    .is_some_and(|f| f.endorsements().contains_key(&seat.assignment_id))
                {
                    continue;
                }
                transition(
                    &citizens[&seat.member],
                    id,
                    AnonymousParliamentTransitionV1::EndorsePublicFinding(
                        ParliamentEndorsePublicFindingV1 {
                            body_instance_id: instance,
                            result_root,
                        },
                    ),
                )?;
                submitted = true;
            }
        }
        if !submitted && let Some(due) = state.lifecycle_items()?.iter().map(|i| i.due_ms).min() {
            wait_until(due.min(unix_ms()?.saturating_add(30_000)))?;
        }
        carry(network)?;
    }
}

struct Opened {
    anchor: GovernanceAnchorV1,
    scope: BallotScopeV1,
    board: ParliamentBallotBoardRecordV1,
}

fn authenticated_opening(
    network: &Network,
    voter: BallotBodyContextV1,
    proposal: &ProposalKind,
) -> Result<Opened> {
    let client = network.peers()[0].client();
    let proof = |h| {
        client
            .client()
            .get_sumeragi_finality_proof(NonZeroU64::new(h).unwrap(), SumeragiRootScope::Global)
    };
    let origin = network.genesis().0;
    let first = proof(1)?;
    let mut verifier = SumeragiFinalityVerifier::new(
        &origin,
        &network.chain_id().to_string(),
        first.committee.clone(),
    )?;
    verifier.verify(&first)?;
    let mut sealed = None;
    let mut opened = None;
    for h in 2..=harness::tip(&client)? {
        let block = verifier.verify(&proof(h)?)?;
        for output in block.block().execution_outputs() {
            let ExecutionOutputV1::GovernanceBlockStart(row) = output else {
                break;
            };
            let GovernanceOutputAnchorV1::Pass(anchor) = row.anchor else {
                continue;
            };
            match &row.event {
                GovernanceBlockStartEventV1::SealedCredentialListPublished(event)
                    if event.body_context == voter =>
                {
                    ensure!(
                        sealed.replace((anchor, event.clone())).is_none(),
                        "repeated credential seal"
                    );
                }
                GovernanceBlockStartEventV1::CastingOpened(event)
                    if event.body_context == voter =>
                {
                    ensure!(
                        opened.replace((anchor, event.clone())).is_none(),
                        "repeated casting opening"
                    );
                }
                _ => {}
            }
        }
    }
    let (anchor, seal) = sealed.ok_or_else(|| eyre!("missing certified credential publication"))?;
    let (opening, event) = opened.ok_or_else(|| eyre!("missing certified casting opening"))?;
    let bytes = client.client().anonymous_ballot_board(&event.scope)?;
    let board: ParliamentBallotBoardRecordV1 = norito::decode_canonical(&bytes)?;
    check_open_board(&board, voter, proposal, anchor, &seal, opening, &event)?;
    Ok(Opened {
        anchor,
        scope: event.scope,
        board,
    })
}

fn check_open_board(
    board: &ParliamentBallotBoardRecordV1,
    voter: BallotBodyContextV1,
    proposal: &ProposalKind,
    anchor: GovernanceAnchorV1,
    seal: &SealedCredentialListPublishedV1,
    opening: GovernanceAnchorV1,
    event: &CastingOpenedV1,
) -> Result<()> {
    board.validate()?;
    ensure!(
        board.context.body_context == voter
            && board.context.proposal_digest == digests::proposal_digest_v1(proposal)?
            && board.purpose
                == ParliamentBallotPurposeV1::CandidateTesting(
                    ParliamentCandidateTestingPolicyV1::CANDIDATE_V1
                )
            && board.opening_at_ms == Some(event.opening_at_ms)
            && board.proposal_content_id
                == iroha_data_model::governance::types::ProposalContentId::derive_v1(proposal)
            && board.body_instance_id == seal.body_instance_id
            && board.body_instance_id == event.body_instance_id
            && board.context.policy_hash == event.policy_hash
            && board.opening == Some(opening)
            && board.casting_clock.is_some()
            && board.sealed_timestamp_ms == anchor.timestamp_ms
            && board.roster_root == seal.roster_root
            && board.context.list_digest == seal.list_digest
            && board.registration_mapping_digest == seal.registration_mapping_digest
            && SealedCredentialListBytesV1::from_list(&board.context.sealed_list()?)?
                == seal.sealed_list_bytes
            && board.entries.is_empty()
            && board.closure.is_none(),
        "board differs from the certified seal or opening"
    );
    Ok(())
}

fn closed_reads(
    network: &Network,
    opened: &Opened,
    id: GovernanceAttemptId,
) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut agreed = None;
    for peer in network.validators() {
        let client = peer.client();
        let board = client.client().anonymous_ballot_board(&opened.scope)?;
        let parsed: ParliamentBallotBoardRecordV1 = norito::decode_canonical(&board)?;
        let closed = parsed
            .closure
            .as_ref()
            .ok_or_else(|| eyre!("the jury did not close"))?;
        ensure!(
            (
                closed.accepted_ballots,
                closed.aye,
                closed.nay,
                closed.abstain
            ) == (1, 1, 0, 0)
                && closed.scope == opened.scope
                && closed.outcome == BallotOutcomeV1::NoQuorum
                && parsed.entries.len() == 1,
            "the closed corpus or one-voter outcome differs"
        );
        let state = read(peer, id)?;
        ensure!(
            matches!(state.rejection(), Some(AnonymousAttemptRejectionV1::Ballot(record)) if record.as_ref() == closed)
                && state.result(POLICY).is_none()
                && state.certificate().is_none(),
            "the jury result is missing or incorrectly certified"
        );
        let pair = (board, norito::encode_canonical(&state)?);
        ensure!(
            agreed.as_ref().is_none_or(|first| first == &pair),
            "validators disagree on the closed state"
        );
        agreed = Some(pair);
    }
    agreed.ok_or_else(|| eyre!("missing validators"))
}

/// Exercise the candidate after a genuine baseline Parliament decision activated the family.
pub(super) fn exercise(network: &Network, runtime: &tokio::runtime::Runtime) -> Result<()> {
    ensure!(
        network.validators().len() == 4
            && network.peers().iter().all(|p| p
                .api_address()
                .to_string()
                .parse::<std::net::SocketAddr>()
                .is_ok_and(|a| a.ip().is_loopback())),
        "this exercise requires four disposable localhost validators"
    );
    let mut examples = std::env::current_exe()?
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| eyre!("missing Cargo profile path"))?
        .join("examples");
    if let Some(path) = std::env::var_os("IROHA_PARLIAMENT_EXAMPLE_DIR") {
        examples = PathBuf::from(path);
    }
    let private = tempfile::tempdir()?;
    let root = private.path();
    let clients = fund_citizens(network)?;
    let admin = network.client();
    let proposed = harness::propose(
        network,
        &admin,
        harness::draft_on_every_validator(
            network,
            harness::NATIVE_GAME_CAPTURE,
            harness::tip(&admin)? + 100_000,
            [0x54; 32],
        )?,
    )?;
    let create = CreateAnonymousParliamentAttemptV1 {
        proposal: proposed.proposal.clone(),
        attempt_sequence: 0,
    };
    let id = create.governance_attempt_id();
    harness::submit_applied(&admin, create.into())?;
    for _ in 0..3 {
        carry(network)?;
    }
    ensure!(
        read(&network.peers()[0], id)?
            .qualification()
            .ended()
            .is_some(),
        "qualification has not ended"
    );
    transition(
        &admin,
        id,
        AnonymousParliamentTransitionV1::RegisterInitialSortition,
    )?;
    let state = read(&network.peers()[0], id)?;
    let slot = *state
        .awaiting_slots()
        .first()
        .ok_or_else(|| eyre!("no future native sortition pulse"))?;
    harness::carry_to_height(network, &admin, slot, "native Parliament draw")?;
    let (wallet, voter) = invitations(network, id, &clients, root, &examples)?;
    govern_to_opening(network, id, &clients)?;
    let opened = authenticated_opening(network, voter, &proposed.proposal)?;
    let pre = PrivateDirectory::open(&wallet)?;
    store(
        &pre,
        "sealed-board.nrt",
        &norito::encode_canonical(&opened.board)?,
    )?;
    vote_config(&pre, network, &network.peers()[0], "client.toml")?;
    let config = wallet.join("client.toml");
    public_example(
        &examples,
        "parliament_ballot_wallet",
        &[config.as_os_str(), wallet.as_os_str()],
        Some(b"aye\nsubmit\n"),
    )?;

    // A 202 is admission only. Require the signed carrier to appear on the complete public board.
    let deadline = Instant::now() + Duration::from_secs(240);
    loop {
        carry(network)?;
        let bytes = admin.client().anonymous_ballot_board(&opened.scope)?;
        let board: ParliamentBallotBoardRecordV1 = norito::decode_canonical(&bytes)?;
        if board.entries.len() == 1 {
            break;
        }
        ensure!(
            Instant::now() < deadline,
            "admitted ballot did not reach the public corpus"
        );
    }
    let closing = opened
        .board
        .casting_clock
        .as_ref()
        .unwrap()
        .timestamp_ms
        .checked_add(CASTING_WINDOW_MS)
        .ok_or_else(|| eyre!("closing time overflow"))?;
    wait_until(closing)?;
    carry(network)?;
    let before = closed_reads(network, &opened, id)?;
    store(&pre, "closed.nrt", &before.0)?;
    let verify = |config: &Path| -> Result<()> {
        public_example(
            &examples,
            "parliament_ballot_verify",
            &[
                config.as_os_str(),
                wallet.join("closed.nrt").as_os_str(),
                opened.anchor.height.to_string().as_ref(),
                opened.anchor.position.to_string().as_ref(),
            ],
            None,
        )?;
        Ok(())
    };
    verify(&config)?;
    let height = harness::tip(&admin)?;
    for peer in network.validators() {
        runtime.block_on(peer.shutdown());
    }
    for peer in network.validators() {
        let layers: Vec<_> = network.config_layers_for_peer(peer).collect();
        runtime.block_on(peer.start_checked(layers.iter(), None))?;
    }
    harness::await_height_on_every_peer(network, height)?;
    ensure!(
        closed_reads(network, &opened, id)? == before,
        "closed board or attempt changed on retained-store restart"
    );
    vote_config(&pre, network, &network.peers()[3], "restored.toml")?;
    verify(&wallet.join("restored.toml"))?;
    eprintln!(
        "ANONYMOUS_PARLIAMENT_BALLOT validators=4 accepted=1 aye=1 quorum=false closed=true restarted=4 genesis={} scope={}",
        network.network_id(),
        hex::encode(opened.scope.as_bytes())
    );
    Ok(())
}
