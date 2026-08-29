//! Attempt-based SORA Parliament draft and read commands.

use std::collections::BTreeMap;

use super::shared::print_with_summary;
use crate::{Run, RunContext};
use eyre::{Result, WrapErr, bail, eyre};
use iroha::{
    client::{
        Client, PARLIAMENT_API_VERSION_V1, ParliamentAttemptDraftRequestV1,
        ParliamentTlePartialReleaseShareV1, ParliamentTleReleaseContextResponseV1,
        ParliamentTransitionDraftRequestV1,
    },
    data_model::{
        governance::types::{BallotAttemptId, GovernanceAttemptId, ProposalKind},
        isi::InstructionBox,
    },
};
use iroha_core::{
    governance::timed_ovn::TimedOvnReleaseIdentityPublicV1,
    tle_release::{
        AuthorizedTleReleaseProjectionV1, TLE_AUTHORIZED_RELEASE_IDENTITY_PAYLOAD_BYTES_V1,
        TLE_AUTHORIZED_RELEASE_PROJECTION_VERSION_V1, TleAdaptiveDealerCommitmentV1,
        TleAdaptivePublicShareV1, TleKeySessionPublicStateV1, TlePartialReleaseShareV1,
    },
};
use iroha_data_model::isi::governance::{
    ParliamentFinalizeOpenedBallotV1, ParliamentLifecycleTransitionV1,
    ParliamentTleFinalReleaseSignatureV1, SubmitParliamentLifecycleTransitionV1,
};
use url::{Host, Url};

const MAX_RELEASE_PEERS_V1: usize = 31;

fn parse_governance_attempt_id(input: &str) -> Result<GovernanceAttemptId, String> {
    let id = input.parse::<GovernanceAttemptId>().map_err(|_| {
        "must be exactly 64 lowercase hexadecimal characters without a prefix".to_owned()
    })?;
    if id.as_bytes().iter().all(|byte| *byte == 0) {
        return Err("must be a non-zero governance attempt id".to_owned());
    }
    Ok(id)
}

fn parse_ballot_attempt_id(input: &str) -> Result<BallotAttemptId, String> {
    let id = input.parse::<BallotAttemptId>().map_err(|_| {
        "must be exactly 64 lowercase hexadecimal characters without a prefix".to_owned()
    })?;
    if id.as_bytes().iter().all(|byte| *byte == 0) {
        return Err("must be a non-zero ballot attempt id".to_owned());
    }
    Ok(id)
}

fn parse_release_peer_url(input: &str) -> Result<Url, String> {
    let url = Url::parse(input).map_err(|_| "must be an absolute Torii URL".to_owned())?;
    if url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && matches!(url.path(), "" | "/")
        && (url.scheme() == "https"
            || (url.scheme() == "http"
                && url.host().is_some_and(|host| match host {
                    Host::Domain(domain) => domain.eq_ignore_ascii_case("localhost"),
                    Host::Ipv4(address) => address.is_loopback(),
                    Host::Ipv6(address) => address.is_loopback(),
                })))
    {
        return Ok(url);
    }
    Err(
        "must be a root HTTPS Torii URL (HTTP is allowed only for a loopback host) without credentials, query, or fragment"
            .to_owned(),
    )
}

fn release_peer_client(primary: &Client, peer_url: Url) -> Result<Client> {
    let peer_url = parse_release_peer_url(peer_url.as_str()).map_err(|reason| eyre!(reason))?;
    let mut peer = primary.clone();
    peer.torii_url = peer_url;
    // Peer reads and partial requests require fresh account-bound signatures. The
    // account key is therefore retained, but origin-agnostic HTTP credentials,
    // caller-supplied headers, and operator credentials must never cross from the
    // configured primary to an independently supplied signer-peer origin.
    peer.headers.clear();
    peer.operator_key_pair = None;
    Ok(peer)
}

fn release_statement_matches(
    canonical: &ParliamentTleReleaseContextResponseV1,
    candidate: &ParliamentTleReleaseContextResponseV1,
) -> bool {
    canonical.version == candidate.version
        && canonical.ballot_attempt_id == candidate.ballot_attempt_id
        && canonical.governance_attempt_id == candidate.governance_attempt_id
        && canonical.body_instance_id == candidate.body_instance_id
        && canonical.status == candidate.status
        && canonical.release_height == candidate.release_height
        && canonical.opening_deadline_height == candidate.opening_deadline_height
        && canonical.tle_key_session == candidate.tle_key_session
        && canonical.release_identity == candidate.release_identity
        && canonical.identity_digest == candidate.identity_digest
        && canonical.identity_payload_hex == candidate.identity_payload_hex
}

fn release_projection(
    context: &ParliamentTleReleaseContextResponseV1,
) -> Result<AuthorizedTleReleaseProjectionV1> {
    let identity_payload: [u8; TLE_AUTHORIZED_RELEASE_IDENTITY_PAYLOAD_BYTES_V1] =
        hex::decode(&context.identity_payload_hex)
            .wrap_err("failed to decode Parliament TLE release identity payload")?
            .try_into()
            .map_err(|_| eyre!("Parliament TLE release identity payload has the wrong width"))?;
    let key_session = &context.tle_key_session;
    Ok(AuthorizedTleReleaseProjectionV1 {
        version: TLE_AUTHORIZED_RELEASE_PROJECTION_VERSION_V1,
        ballot_attempt_id: context.ballot_attempt_id,
        opening_deadline_height: context.opening_deadline_height,
        finalized_height: context.current_height,
        key_session: TleKeySessionPublicStateV1 {
            version: key_session.version,
            key_session_id: key_session.key_session_id,
            network_id: key_session.network_id,
            roster_hash: key_session.roster_hash,
            committee_size: key_session.committee_size,
            threshold: key_session.threshold,
            generator_h: key_session.generator_h,
            generator_v: key_session.generator_v,
            qualified_dealers: key_session.qualified_dealers.clone(),
            qualified_dealer_commitments: key_session
                .qualified_dealer_commitments
                .iter()
                .map(|dealer| TleAdaptiveDealerCommitmentV1 {
                    dealer_index: dealer.dealer_index,
                    coefficient_commitments: dealer.coefficient_commitments.clone(),
                    constant_pok_commitment: dealer.constant_pok_commitment,
                    constant_pok_response: dealer.constant_pok_response,
                })
                .collect(),
            dkg_event_hash: key_session.dkg_event_hash,
            group_public_key: key_session.group_public_key,
            public_shares: key_session
                .public_shares
                .iter()
                .map(|share| TleAdaptivePublicShareV1 {
                    index: share.index,
                    participant_hash: share.participant_hash,
                    public_key_share: share.public_key_share,
                })
                .collect(),
            transcript_hash: key_session.transcript_hash,
        },
        public_release_identity: TimedOvnReleaseIdentityPublicV1 {
            tle_key_session_id: context.release_identity.tle_key_session_id,
            governance_attempt_id: *context.release_identity.governance_attempt_id.as_bytes(),
            body_instance_id: *context.release_identity.body_instance_id.as_bytes(),
            ballot_attempt_id: *context.release_identity.ballot_attempt_id.as_bytes(),
            survivor_corpus_root: context.release_identity.survivor_corpus_root,
            no_recovery_root: context.release_identity.no_recovery_root,
            target_finalized_height: context.release_identity.target_finalized_height,
            parameter_hash: context.release_identity.parameter_hash,
        },
        identity_payload,
        identity_digest: context.identity_digest,
    })
}

fn release_partial(partial: ParliamentTlePartialReleaseShareV1) -> TlePartialReleaseShareV1 {
    TlePartialReleaseShareV1 {
        key_session_id: partial.key_session_id,
        identity_digest: partial.identity_digest,
        participant_index: partial.participant_index,
        sigma: partial.sigma,
        proof_x: partial.proof_x,
        proof_y: partial.proof_y,
        z_s: partial.z_s,
        z_r: partial.z_r,
        z_u: partial.z_u,
    }
}

fn insert_verified_partial(
    partials: &mut BTreeMap<u16, TlePartialReleaseShareV1>,
    partial: TlePartialReleaseShareV1,
) -> Result<()> {
    if let Some(existing) = partials.get(&partial.participant_index) {
        if existing.sigma != partial.sigma {
            bail!(
                "conflicting valid Parliament TLE partials for participant {}",
                partial.participant_index
            );
        }
        return Ok(());
    }
    partials.insert(partial.participant_index, partial);
    Ok(())
}

/// Draft one canonical Parliament attempt creation for local signing.
#[derive(clap::Args, Debug)]
pub struct DraftAttemptArgs {
    /// JSON file containing one exact tagged `ProposalKind` value.
    #[arg(long, value_name = "PATH")]
    pub proposal_json: std::path::PathBuf,
    /// Zero-based retry sequence for the exact proposal content.
    #[arg(long, default_value_t = 0)]
    pub attempt_sequence: u32,
}

impl Run for DraftAttemptArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let bytes = std::fs::read(&self.proposal_json).wrap_err_with(|| {
            format!(
                "failed to read Parliament proposal JSON `{}`",
                self.proposal_json.display()
            )
        })?;
        let proposal: ProposalKind = norito::json::from_slice(&bytes)
            .wrap_err("failed to decode strict Parliament ProposalKind JSON")?;
        let request = ParliamentAttemptDraftRequestV1 {
            version: PARLIAMENT_API_VERSION_V1,
            proposal,
            attempt_sequence: self.attempt_sequence,
        };
        let client: Client = context.client_from_config();
        let response = client.post_parliament_attempt_draft(&request)?;
        let value = norito::json::to_value(&response)
            .wrap_err("failed to render Parliament attempt draft")?;
        print_with_summary(
            context,
            Some(format!(
                "governance_attempt_id={} proposal_content_id={} instructions=1",
                response.governance_attempt_id.to_hex(),
                response.proposal_content_id.to_hex()
            )),
            &value,
        )
    }
}

/// Draft one exact Parliament lifecycle transition for local signing.
#[derive(clap::Args, Debug)]
pub struct DraftTransitionArgs {
    /// Canonical lowercase identifier of the existing Parliament attempt.
    #[arg(long, value_parser = parse_governance_attempt_id)]
    pub governance_attempt_id: GovernanceAttemptId,
    /// JSON file containing one exact tagged lifecycle transition.
    #[arg(long, value_name = "PATH")]
    pub transition_json: std::path::PathBuf,
}

impl Run for DraftTransitionArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let bytes = std::fs::read(&self.transition_json).wrap_err_with(|| {
            format!(
                "failed to read Parliament transition JSON `{}`",
                self.transition_json.display()
            )
        })?;
        let transition: ParliamentLifecycleTransitionV1 = norito::json::from_slice(&bytes)
            .wrap_err("failed to decode strict Parliament lifecycle transition JSON")?;
        let request = ParliamentTransitionDraftRequestV1 {
            version: PARLIAMENT_API_VERSION_V1,
            governance_attempt_id: self.governance_attempt_id,
            transition,
        };
        request
            .validate_static()
            .map_err(|reason| eyre::eyre!(reason))?;
        let client: Client = context.client_from_config();
        let response = client.post_parliament_transition_draft(&request)?;
        let value = norito::json::to_value(&response)
            .wrap_err("failed to render Parliament transition draft")?;
        print_with_summary(
            context,
            Some(format!(
                "governance_attempt_id={} transition={:?} transition_digest={} instructions=1",
                response.governance_attempt_id.to_hex(),
                response.transition_kind,
                hex::encode(response.transition_digest)
            )),
            &value,
        )
    }
}

/// Read one exact canonical Parliament attempt.
#[derive(clap::Args, Debug)]
pub struct GetAttemptArgs {
    /// Canonical lowercase attempt identifier.
    #[arg(long, value_parser = parse_governance_attempt_id)]
    pub governance_attempt_id: GovernanceAttemptId,
}

impl Run for GetAttemptArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let response = client.get_parliament_attempt(self.governance_attempt_id)?;
        let state_bytes = response.state_payload_hex.len() / 2;
        let value = norito::json::to_value(&response)
            .wrap_err("failed to render Parliament attempt read response")?;
        print_with_summary(
            context,
            Some(format!(
                "governance_attempt_id={} stage={:?} status={:?} height={} state_bytes={state_bytes}",
                response.attempt.id.to_hex(),
                response.attempt.stage,
                response.attempt.status,
                response.current_height
            )),
            &value,
        )
    }
}

/// Fetch, verify, combine, and submit one threshold-opened Parliament ballot.
///
/// The primary release statement is fetched again after peer collection, and
/// the aggregate is verified at that refreshed finalized height immediately
/// before the normally signed transition is submitted.
#[derive(clap::Args, Debug)]
pub struct FinalizeOpenedBallotArgs {
    /// Canonical lowercase identifier of the ballot attempt in `Opening`.
    #[arg(long, value_parser = parse_ballot_attempt_id)]
    pub ballot_attempt_id: BallotAttemptId,
    /// Root URL of one signer peer exposing release-context and partial-release routes.
    ///
    /// Supply every configured signer peer. The coordinator sorts and bounds the
    /// URLs, verifies every public proof locally, de-duplicates equal seats, and
    /// combines the lowest canonical threshold of valid participant indices.
    #[arg(
        long = "peer",
        value_name = "TORII_URL",
        value_parser = parse_release_peer_url,
        required = true
    )]
    pub peer_urls: Vec<Url>,
}

impl Run for FinalizeOpenedBallotArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        if self.peer_urls.len() > MAX_RELEASE_PEERS_V1 {
            bail!(
                "Parliament TLE release coordinator accepts at most {MAX_RELEASE_PEERS_V1} peers"
            );
        }
        let mut unique_peers = BTreeMap::new();
        for peer in self.peer_urls {
            unique_peers.insert(peer.as_str().to_owned(), peer);
        }
        if unique_peers.is_empty() {
            bail!("Parliament TLE release coordinator requires at least one signer peer");
        }

        let primary: Client = context.client_from_config();
        let release_context = primary
            .get_parliament_tle_release_context(self.ballot_attempt_id)
            .wrap_err("failed to fetch the canonical Parliament TLE release context")?;
        let validated = release_projection(&release_context)?
            .validate()
            .wrap_err("failed to replay the Parliament TLE public transcript and identity")?;
        let threshold = usize::from(release_context.tle_key_session.threshold);
        if unique_peers.len() < threshold {
            bail!(
                "Parliament TLE release requires {threshold} distinct signer peers, but only {} were supplied",
                unique_peers.len()
            );
        }
        if unique_peers.len() > usize::from(release_context.tle_key_session.committee_size) {
            bail!("Parliament TLE release peer count exceeds the committed committee size");
        }

        let mut verified_partials = BTreeMap::new();
        let mut failed_peers = Vec::new();
        for (peer_label, peer_url) in unique_peers {
            let peer_client = release_peer_client(&primary, peer_url)?;
            let result = (|| -> Result<TlePartialReleaseShareV1> {
                let peer_context = peer_client
                    .get_parliament_tle_release_context(self.ballot_attempt_id)
                    .wrap_err("peer release context unavailable")?;
                if !release_statement_matches(&release_context, &peer_context) {
                    bail!("peer release context differs from the canonical release statement");
                }
                let partial = release_partial(
                    peer_client
                        .post_parliament_tle_partial_release(&release_context)
                        .wrap_err("peer partial release unavailable")?,
                );
                validated
                    .session()
                    .verify_partial_release(
                        validated.identity(),
                        validated.finalized_height(),
                        &partial,
                    )
                    .wrap_err("peer partial release proof is invalid")?;
                Ok(partial)
            })();
            match result {
                Ok(partial) => insert_verified_partial(&mut verified_partials, partial)?,
                Err(_) => failed_peers.push(peer_label),
            }
        }

        if verified_partials.len() < threshold {
            bail!(
                "Parliament TLE release obtained {} distinct valid shares, below threshold {threshold}; failed peers: {}",
                verified_partials.len(),
                failed_peers.join(", ")
            );
        }
        let canonical_threshold = verified_partials
            .into_values()
            .take(threshold)
            .collect::<Vec<_>>();
        let final_release = validated
            .session()
            .combine_partial_releases(
                validated.identity(),
                validated.finalized_height(),
                &canonical_threshold,
            )
            .wrap_err("failed to combine the canonical Parliament TLE threshold")?;

        // Peer collection can span several finalized heights. Reauthorize on the
        // configured primary immediately before submission so a transitioned or
        // expired ballot cannot be finalized from the initial snapshot.
        let refreshed_release_context = primary
            .get_parliament_tle_release_context(self.ballot_attempt_id)
            .wrap_err("failed to refresh the canonical Parliament TLE release context")?;
        if !release_statement_matches(&release_context, &refreshed_release_context) {
            bail!("Parliament TLE release statement changed while collecting partials");
        }
        if refreshed_release_context.current_height < release_context.current_height {
            bail!("refreshed Parliament TLE release context regressed in finalized height");
        }
        let refreshed_validated = release_projection(&refreshed_release_context)?
            .validate()
            .wrap_err("refreshed Parliament TLE release context is no longer authorized")?;
        refreshed_validated
            .session()
            .verify_final_release(
                refreshed_validated.identity(),
                refreshed_validated.finalized_height(),
                &final_release,
            )
            .wrap_err("combined Parliament TLE release failed refreshed final verification")?;

        let instruction = SubmitParliamentLifecycleTransitionV1 {
            governance_attempt_id: refreshed_release_context.governance_attempt_id,
            transition: ParliamentLifecycleTransitionV1::FinalizeOpenedBallot(
                ParliamentFinalizeOpenedBallotV1 {
                    ballot_attempt_id: refreshed_release_context.ballot_attempt_id,
                    final_release: ParliamentTleFinalReleaseSignatureV1 {
                        key_session_id: final_release.key_session_id,
                        identity_digest: final_release.identity_digest,
                        signature: final_release.signature,
                    },
                },
            ),
        };
        context.finish(vec![InstructionBox::from(instruction)])
    }
}

/// Attempt-based Parliament commands.
#[derive(clap::Subcommand, Debug)]
pub enum ParliamentCommand {
    /// Draft one canonical attempt creation instruction.
    DraftAttempt(DraftAttemptArgs),
    /// Draft one exact lifecycle transition instruction.
    DraftTransition(DraftTransitionArgs),
    /// Read one exact committed attempt projection.
    GetAttempt(GetAttemptArgs),
    /// Verify signer-peer shares and submit `FinalizeOpenedBallot` normally.
    FinalizeOpenedBallot(FinalizeOpenedBallotArgs),
}

impl Run for ParliamentCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Self::DraftAttempt(args) => args.run(context),
            Self::DraftTransition(args) => args.run(context),
            Self::GetAttempt(args) => args.run(context),
            Self::FinalizeOpenedBallot(args) => args.run(context),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::governance::types::{
        BallotAttemptStatusV1, BodyInstanceId, TleKeySessionId,
    };
    use iroha_torii_shared::parliament_api::{
        PARLIAMENT_TLE_RELEASE_IDENTITY_PAYLOAD_BYTES_V1,
        ParliamentTimedOvnReleaseIdentityProjectionV1, ParliamentTleAdaptiveDealerCommitmentV1,
        ParliamentTleAdaptivePublicShareV1, ParliamentTleKeySessionBindingV1,
    };
    use sha2::{Digest as _, Sha256};
    use std::{
        collections::HashMap,
        io::{Read as _, Write as _},
        net::TcpListener,
        sync::mpsc,
        thread,
        time::Duration,
    };

    fn release_context_fixture() -> ParliamentTleReleaseContextResponseV1 {
        let ballot_attempt_id = BallotAttemptId::new([0x33; 32]);
        let governance_attempt_id = GovernanceAttemptId::new([0x11; 32]);
        let body_instance_id = BodyInstanceId::new([0x22; 32]);
        let key_session_id = TleKeySessionId::new([0x44; 32]);
        let tle_key_session = ParliamentTleKeySessionBindingV1 {
            version: PARLIAMENT_API_VERSION_V1,
            key_session_id,
            network_id: [0x45; 32],
            roster_hash: [0x46; 32],
            committee_size: 4,
            threshold: 2,
            generator_h: [0x47; 96],
            generator_v: [0x48; 96],
            qualified_dealers: vec![1, 2],
            qualified_dealer_commitments: vec![
                ParliamentTleAdaptiveDealerCommitmentV1 {
                    dealer_index: 1,
                    coefficient_commitments: vec![[0x49; 96], [0x4A; 96]],
                    constant_pok_commitment: [0x4B; 96],
                    constant_pok_response: [0x4C; 32],
                },
                ParliamentTleAdaptiveDealerCommitmentV1 {
                    dealer_index: 2,
                    coefficient_commitments: vec![[0x4D; 96], [0x4E; 96]],
                    constant_pok_commitment: [0x4F; 96],
                    constant_pok_response: [0x50; 32],
                },
            ],
            dkg_event_hash: [0x51; 32],
            group_public_key: [0x52; 96],
            public_shares: (1_u16..=4)
                .map(|index| ParliamentTleAdaptivePublicShareV1 {
                    index,
                    participant_hash: [u8::try_from(index).expect("small index") + 0x52; 32],
                    public_key_share: [u8::try_from(index).expect("small index") + 0x62; 96],
                })
                .collect(),
            transcript_hash: [0x53; 32],
        };
        let release_identity = ParliamentTimedOvnReleaseIdentityProjectionV1 {
            tle_key_session_id: key_session_id,
            governance_attempt_id,
            body_instance_id,
            ballot_attempt_id,
            survivor_corpus_root: [0x54; 32],
            no_recovery_root: [0x55; 32],
            target_finalized_height: 90,
            parameter_hash: [0x56; 32],
        };
        let mut identity_payload = Vec::new();
        identity_payload.extend_from_slice(b"iroha.parliament.tle.identity-payload.v1\0");
        identity_payload.extend_from_slice(&1_u16.to_be_bytes());
        identity_payload.extend_from_slice(governance_attempt_id.as_bytes());
        identity_payload.extend_from_slice(body_instance_id.as_bytes());
        identity_payload.extend_from_slice(ballot_attempt_id.as_bytes());
        identity_payload.extend_from_slice(&release_identity.survivor_corpus_root);
        identity_payload.extend_from_slice(&release_identity.no_recovery_root);
        identity_payload.extend_from_slice(&release_identity.target_finalized_height.to_be_bytes());
        identity_payload.extend_from_slice(&release_identity.parameter_hash);
        assert_eq!(
            identity_payload.len(),
            PARLIAMENT_TLE_RELEASE_IDENTITY_PAYLOAD_BYTES_V1
        );
        let mut release_message = Vec::new();
        release_message.extend_from_slice(b"iroha.threshold-bls.message.v1\0");
        release_message.extend_from_slice(b"iroha.threshold-bls.session.v1\0");
        release_message.extend_from_slice(&1_u16.to_be_bytes());
        release_message.push(2);
        release_message.extend_from_slice(&tle_key_session.network_id);
        release_message.extend_from_slice(tle_key_session.key_session_id.as_bytes());
        release_message.extend_from_slice(&tle_key_session.roster_hash);
        release_message.extend_from_slice(&tle_key_session.committee_size.to_be_bytes());
        release_message.extend_from_slice(&tle_key_session.threshold.to_be_bytes());
        release_message.extend_from_slice(
            &u32::try_from(identity_payload.len())
                .expect("identity width fits u32")
                .to_be_bytes(),
        );
        release_message.extend_from_slice(&identity_payload);
        ParliamentTleReleaseContextResponseV1 {
            version: PARLIAMENT_API_VERSION_V1,
            current_height: 100,
            ballot_attempt_id,
            governance_attempt_id,
            body_instance_id,
            status: BallotAttemptStatusV1::Opening,
            release_height: 90,
            opening_deadline_height: 110,
            tle_key_session,
            release_identity,
            identity_digest: Sha256::digest(release_message).into(),
            identity_payload_hex: hex::encode(identity_payload),
        }
    }

    fn spawn_release_context_peer(
        response: &ParliamentTleReleaseContextResponseV1,
        requests: usize,
    ) -> (Url, mpsc::Receiver<String>, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind release-context peer");
        let address = listener.local_addr().expect("release-context peer address");
        let body = norito::json::to_vec(response).expect("encode release-context response");
        let (sender, receiver) = mpsc::channel();
        let handle = thread::spawn(move || {
            for _ in 0..requests {
                let (mut stream, _) = listener.accept().expect("accept peer request");
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .expect("set peer request timeout");
                let mut raw = Vec::new();
                let mut chunk = [0_u8; 4096];
                loop {
                    let read = stream.read(&mut chunk).expect("read peer request");
                    if read == 0 {
                        break;
                    }
                    raw.extend_from_slice(&chunk[..read]);
                    if raw.windows(4).any(|window| window == b"\r\n\r\n") {
                        break;
                    }
                }
                sender
                    .send(String::from_utf8(raw).expect("peer request is HTTP text"))
                    .expect("record peer request");
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .expect("write peer response headers");
                stream.write_all(&body).expect("write peer response body");
            }
        });
        (
            Url::parse(&format!("http://{address}")).expect("parse peer URL"),
            receiver,
            handle,
        )
    }

    #[test]
    fn attempt_id_parser_is_exact_lowercase_nonzero_hex() {
        assert!(parse_governance_attempt_id(&"ab".repeat(32)).is_ok());
        for invalid in [
            "00".repeat(32),
            "AB".repeat(32),
            format!("0x{}", "ab".repeat(32)),
            "ab".repeat(31),
        ] {
            assert!(
                parse_governance_attempt_id(&invalid).is_err(),
                "invalid attempt id must fail: {invalid}"
            );
        }
    }

    #[test]
    fn ballot_id_parser_is_exact_lowercase_nonzero_hex() {
        assert!(parse_ballot_attempt_id(&"cd".repeat(32)).is_ok());
        for invalid in [
            "00".repeat(32),
            "CD".repeat(32),
            format!("0x{}", "cd".repeat(32)),
            "cd".repeat(31),
        ] {
            assert!(
                parse_ballot_attempt_id(&invalid).is_err(),
                "invalid ballot id must fail: {invalid}"
            );
        }
    }

    #[test]
    fn release_peer_urls_are_https_or_loopback_http_roots() {
        for valid in [
            "https://validator.example:8080",
            "http://127.0.0.1:8080",
            "http://[::1]:8080/",
            "http://localhost:8080",
        ] {
            assert!(parse_release_peer_url(valid).is_ok(), "valid URL: {valid}");
        }
        for invalid in [
            "http://validator.example:8080",
            "https://user@example.com",
            "https://validator.example/v1",
            "https://validator.example?token=secret",
            "https://validator.example#fragment",
        ] {
            assert!(
                parse_release_peer_url(invalid).is_err(),
                "invalid URL: {invalid}"
            );
        }
    }

    #[test]
    fn release_peer_clients_strip_primary_credentials_but_keep_fresh_account_auth() {
        let expected = release_context_fixture();
        let (peer_url, requests, server) = spawn_release_context_peer(&expected, 2);
        for authorization in ["Basic cHJpbWFyeTpzZWNyZXQ=", "Bearer primary-secret"] {
            let mut primary = Client::with_headers(
                crate::fallback_config(),
                HashMap::from([
                    ("Authorization".to_owned(), authorization.to_owned()),
                    ("X-Primary-Secret".to_owned(), "custom-secret".to_owned()),
                ]),
            );
            primary.set_operator_key_pair(primary.key_pair.clone());
            let peer = release_peer_client(&primary, peer_url.clone()).expect("strict peer client");
            assert!(peer.headers.is_empty());
            assert!(peer.operator_key_pair.is_none());
            assert_eq!(
                peer.get_parliament_tle_release_context(expected.ballot_attempt_id)
                    .expect("legitimate signed peer fetch"),
                expected
            );
        }
        server.join().expect("join release-context peer");
        let requests = requests.into_iter().collect::<Vec<_>>();
        assert_eq!(requests.len(), 2);
        for request in requests {
            let request = request.to_ascii_lowercase();
            assert!(!request.contains("\r\nauthorization:"));
            assert!(!request.contains("\r\nx-primary-secret:"));
            for required in [
                "\r\nx-iroha-account:",
                "\r\nx-iroha-signature:",
                "\r\nx-iroha-timestamp-ms:",
                "\r\nx-iroha-nonce:",
            ] {
                assert!(
                    request.contains(required),
                    "missing signed header {required}"
                );
            }
        }
    }

    #[test]
    fn verified_partial_deduplication_rejects_conflicting_signatures() {
        let partial = TlePartialReleaseShareV1 {
            key_session_id: iroha_core::tle_release::TleKeySessionId::new([1; 32]),
            identity_digest: [2; 32],
            participant_index: 1,
            sigma: [3; 48],
            proof_x: [4; 96],
            proof_y: [5; 48],
            z_s: [6; 32],
            z_r: [7; 32],
            z_u: [8; 32],
        };
        let mut partials = BTreeMap::new();
        insert_verified_partial(&mut partials, partial.clone()).expect("first partial");
        insert_verified_partial(&mut partials, partial.clone()).expect("equal duplicate");
        assert_eq!(partials.len(), 1);

        let conflicting = TlePartialReleaseShareV1 {
            sigma: [9; 48],
            ..partial
        };
        assert!(insert_verified_partial(&mut partials, conflicting).is_err());
    }

    #[test]
    fn proposal_input_rejects_a_request_wrapper_alias() {
        let invalid = norito::json!({
            "version": 1,
            "proposal": {},
            "attemptSequence": 0
        });
        assert!(norito::json::from_value::<ProposalKind>(invalid).is_err());
    }
}
