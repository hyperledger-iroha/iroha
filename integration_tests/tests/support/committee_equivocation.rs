//! One validator operator's genuine double-sign, delivered over the production peer transport.
//!
//! Consensus evidence has no HTTP, CLI or instruction entry: an honest core must itself
//! receive two conflicting messages signed by one committee key on an authenticated direct
//! peer session (`specs/sumeragi_evidence_api.md`). This fixture is that operator. It stops
//! one separately running, non-voting candidate process and speaks the unchanged P2P stack
//! under that process's own transport identity and effective configuration. Through it the
//! fixture delivers two Prepare votes for the next uncommitted height, both signed with the
//! offending validator's real consensus key. The offender's own daemon keeps running: the
//! second signature comes from a duplicate signer holding the same key, which is how a
//! validator double-signs in practice.
//!
//! No node carries a fault control. Detection, local retention, block admission and the
//! delayed slash are performed by the honest daemons' own production code.

use super::*;
use iroha::client::{SUMERAGI_EVIDENCE_LIST_MAX_LIMIT, SumeragiEvidenceListFilter};
use iroha::data_model::block::consensus::{
    ConsensusMode, EvidencePenaltyStatus, EvidenceRecord, EvidenceScope, RecordedEvidenceV1,
};
use iroha::data_model::parameter::system::SumeragiConsensusMode;
use iroha_config::{
    base::read::ConfigReader,
    parameters::{actual, user},
};
use iroha_core::{
    IrohaNetwork,
    governance::manifest::LaneManifestRegistry,
    sumeragi::{
        crypto::{KeyPairSigner, core_key},
        driver::traits::{Frame, Net as _, SendOutcome},
        net::P2pNet,
        node::{PROTOCOL_VERSION, consensus_configuration_fingerprint, root_instance},
    },
};
use iroha_futures::supervisor::Supervisor;
use iroha_p2p::{
    P2pIdentityKeys,
    network::{
        HandshakeCapabilities,
        message::{UpdatePeers, UpdateTopology},
    },
};
use iroha_sumeragi::{
    crypto::Signer as _,
    message::{Evidence as NativeEvidence, TrafficClass, Vote, VoteKind, WireMessage},
    types::{Committee, Hash32, SIGNATURE_LEN, Signature},
};
use std::{collections::HashSet, sync::Arc};

/// The two conflicting signed votes and the single consensus slot they share.
pub struct DoubleSign {
    /// Validator whose real consensus key signed both votes.
    pub offender: PeerId,
    /// Uncommitted height both votes name; the offence height of the evidence.
    pub height: u64,
    first: Vote,
    second: Vote,
}

/// Committed evidence of one double-sign after its mandatory penalty was applied.
pub struct AppliedPenalty {
    /// Height of the block that admitted the proof.
    pub admitted_height: u64,
    /// Height of the block that applied the slash.
    pub applied_height: u64,
}

/// Parse the effective configuration the stopped process itself booted from.
fn stopped_process_config(peer: &NetworkPeer) -> Result<actual::Root> {
    let log = peer
        .latest_stdout_log_path()
        .ok_or_else(|| eyre!("stopped transport process retained no boot log"))?;
    let name = log
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix("-stdout.log"))
        .ok_or_else(|| eyre!("transport process boot log has no run identity"))?;
    let path = log.with_file_name(format!("{name}-config.toml"));
    ensure!(
        path.is_file(),
        "transport process retained no effective configuration at {}",
        path.display()
    );
    ConfigReader::new()
        .without_env()
        .read_toml_with_extends(&path)
        .map_err(|error| eyre!("read transport process configuration: {error:?}"))?
        .read_and_complete::<user::Root>()
        .map_err(|error| eyre!("complete transport process configuration: {error:?}"))?
        .parse()
        .map_err(|error| eyre!("parse transport process configuration: {error:?}"))
}

/// The handshake capabilities an honest daemon of this network advertises and requires.
///
/// Every value is derived from the signed genesis and the stopped process's own parsed
/// configuration with the daemon's public primitives. A wrong value is refused by every
/// honest peer's handshake, so drift fails this fixture instead of weakening a node.
fn handshake_capabilities(
    config: &actual::Root,
    genesis: &iroha::data_model::block::SignedBlock,
) -> Result<HandshakeCapabilities> {
    let metadata = iroha_genesis::signed_genesis_consensus_metadata(genesis)?;
    let mode = match metadata.mode {
        SumeragiConsensusMode::Permissioned => ConsensusMode::Permissioned,
        SumeragiConsensusMode::Npos => ConsensusMode::Npos,
    };
    let instance = root_instance(genesis, config.common.chain.as_ref())
        .map_err(|error| eyre!("derive the signed root instance: {error:?}"))?;
    let original_fees = iroha_genesis::signed_genesis_root_fee_policy(genesis)?;
    let manifests = LaneManifestRegistry::from_config(
        &config.nexus.lane_catalog,
        &config.nexus.governance,
        &config.nexus.registry,
    );
    ensure!(
        !config.nexus.compliance.enabled,
        "this fixture reproduces the handshake of a network without a lane compliance engine"
    );
    let nexus_policy_digest = actual::nexus_consensus_policy_digest_with_runtime_policies(
        &config.nexus,
        Some(&original_fees),
        None,
        Some(manifests.baseline_consensus_policy_digest()),
    )
    .map_err(|error| eyre!("derive the Nexus consensus-policy digest: {error}"))?;
    let host_profiles = iroha_p2p::HostProfileSet::new(
        iroha_core::runtime::attestation::release_host_profiles(
            iroha_core::runtime::release_catalog()
                .map_err(|error| eyre!("release catalog: {error}"))?,
        )
        .into_iter()
        .map(|id| id.0),
    )
    .map_err(|error| eyre!("{error}"))?;
    let consensus = iroha_p2p::ConsensusHandshakeCaps {
        mode,
        proto_version: u32::from(PROTOCOL_VERSION),
        consensus_fingerprint: instance.0,
        config: iroha_p2p::ConsensusConfigCaps {
            // The daemon advertises its bootstrap value here; the signed execution policy
            // is bound by the instance and the native configuration fingerprint below.
            execution_policy_hash: [0; 32],
            nexus_policy_digest,
            native_config_fingerprint: consensus_configuration_fingerprint(genesis)
                .map_err(|error| eyre!("derive the native configuration fingerprint: {error:?}"))?
                .into(),
            host_profiles,
        },
        // This endpoint reads no committed State and gates no peer on host profiles; every
        // honest peer still checks the advertised release set against its own selection.
        required_host_profiles: None,
    };
    let confidential = iroha_p2p::ConfidentialHandshakeCaps {
        enabled: config.confidential.enabled,
        assume_valid: config.confidential.assume_valid,
        verifier_backend: config.confidential.verifier_backend.clone(),
        features: Some(
            iroha::data_model::confidential::ConfidentialFeatureDigest::new(
                None,
                None,
                None,
                Some(iroha_config::parameters::defaults::confidential::RULES_VERSION),
                Some(iroha_core::state::default_genesis_confidential_policy_hash()),
            ),
        ),
    };
    let crypto = iroha_p2p::CryptoHandshakeCaps {
        sm_enabled: iroha::crypto::Algorithm::Sm2
            .require_implementation(iroha::crypto::CryptoOperation::VerifySignature)
            .is_ok(),
        sm_openssl_preview: config.crypto.enable_sm_openssl_preview,
        require_sm_handshake_match: config.network.require_sm_handshake_match,
        require_sm_openssl_preview_match: config.network.require_sm_openssl_preview_match,
    };
    Ok(HandshakeCapabilities {
        consensus: Some(consensus),
        confidential: Some(confidential),
        crypto: Some(crypto),
    })
}

/// Stop exactly one original process and retain the same exit evidence as an all-seat stop.
async fn stop_process_cleanly(peer: &NetworkPeer) -> Result<()> {
    let id = peer.id();
    let mut events = peer.events();
    let pid = peer
        .process_id()
        .await
        .filter(|_| peer.is_running())
        .ok_or_else(|| eyre!("transport process {id} exited before its shutdown subscription"))?;
    let broker = peer.disposable_broker_shutdown_evidence().await;
    let log = peer.latest_stdout_log_path();
    peer.shutdown().await;
    verify_clean_peer_exit(&mut events).wrap_err_with(|| format!("process {id} shutdown"))?;
    let broker =
        broker.map_err(|error| eyre!("process {id} original broker capture failed: {error:#}"))?;
    peer.verify_disposable_broker_shutdown(&broker, pid)
        .wrap_err_with(|| format!("process {id} original broker shutdown"))?;
    ensure!(
        !peer.is_running()
            && peer.process_id().await.is_none()
            && log.is_some()
            && peer.latest_stdout_log_path() == log,
        "process {id} retained a process or replaced its original boot log during shutdown"
    );
    Ok(())
}

async fn consensus_status(
    peer: &NetworkPeer,
) -> Result<iroha::data_model::sumeragi::SumeragiStatus> {
    let client = peer.client().client().clone();
    read_on_dedicated_thread(move || client.get_sumeragi_status())
        .await
        .wrap_err("consensus status worker failed")
}

/// Double-sign the next uncommitted height as `offender` through `carrier`'s transport.
///
/// `tip` is the caller's independently authenticated committed tip. Every other voter of its
/// committee must have applied it and no later block; each receives both votes directly and
/// must retain the conflict before the carrier process is restarted. The votes name the view
/// the voters report once the transport is authenticated. Exactly one pair is signed: a
/// failed delivery fails the fixture and never signs a second offence.
#[cfg(unix)]
pub async fn double_sign(
    network: &Network,
    carrier: &PeerId,
    offender: &PeerId,
    tip: &CertifiedBlock,
) -> Result<DoubleSign> {
    let deadline = Instant::now() + WAIT;
    let schedule = &tip.commitment().schedule;
    let roster = schedule
        .current
        .committee
        .iter()
        .map(|seat| seat.validator.clone())
        .collect::<Vec<_>>();
    let certificate = tip
        .commit_qc()
        .ok_or_else(|| eyre!("authenticated tip retained no commit certificate"))?;
    ensure!(
        roster.contains(offender)
            && !roster.contains(carrier)
            && certificate.height == tip.height()
            && schedule.boundary.is_none(),
        "double-sign requires a current voter, a non-voting transport and a successor in the tip's own epoch"
    );
    let height = tip
        .height()
        .checked_add(1)
        .ok_or_else(|| eyre!("offence height overflow"))?;
    let carrier_process = exact_process_roster(network, std::slice::from_ref(carrier))?[0];
    let offender_process = exact_process_roster(network, std::slice::from_ref(offender))?[0];
    let witnesses = roster
        .iter()
        .filter(|peer| *peer != offender)
        .cloned()
        .collect::<Vec<_>>();
    let witness_processes = exact_process_roster(network, &witnesses)?;

    // Every voter must have applied the authenticated tip, and none may have passed it.
    loop {
        let mut applying = false;
        for process in &witness_processes {
            let status = consensus_status(process).await?;
            ensure!(
                status.instance == certificate.instance.0
                    && status.applied_height <= tip.height()
                    && status.height <= height
                    && status.halted.is_none(),
                "voter {} left the authenticated tip before the offence",
                process.id()
            );
            applying |= status.applied_height < tip.height() || status.height < height;
        }
        if !applying {
            break;
        }
        ensure!(
            Instant::now() < deadline,
            "voters did not all reach the authenticated tip"
        );
        sleep(POLL).await;
    }

    let committee = Committee::new(
        roster
            .iter()
            .map(|peer| core_key(peer.public_key()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| eyre!("authenticated committee key: {error}"))?,
    )
    .map_err(|error| eyre!("authenticated committee: {error:?}"))?;
    let signer = KeyPairSigner::new(
        offender_process
            .bls_key_pair()
            .ok_or_else(|| eyre!("offender has no real consensus key"))?,
    )
    .map_err(|error| eyre!("offender consensus key: {error}"))?;
    let index = committee
        .index_of(signer.public_key())
        .ok_or_else(|| eyre!("offender key is not a member of the authenticated committee"))?;
    let signed = |view: u64, label: &[u8]| -> Result<Vote> {
        let mut vote = Vote {
            kind: VoteKind::Prepare,
            instance: certificate.instance,
            epoch: certificate.epoch,
            height,
            view,
            block_hash: Hash32(Hash::new_from_chunks(&[b"double-signed block", label]).into()),
            result: Hash32(Hash::new_from_chunks(&[b"double-signed result", label]).into()),
            signer: index,
            sig: Signature([0; SIGNATURE_LEN]),
        };
        vote.sig = signer
            .sign(&vote.preimage())
            .map_err(|error| eyre!("offender signature: {error:?}"))?;
        Ok(vote)
    };

    stop_process_cleanly(carrier_process).await?;
    let config = stopped_process_config(carrier_process)?;
    ensure!(
        carrier_process.p2p_address() == *config.network.address.value(),
        "transport process configuration lost its original peer address"
    );
    let genesis = network.genesis();
    let capabilities = handshake_capabilities(&config, &genesis.0)?;
    let targets = witness_processes
        .iter()
        .map(|process| process.network_peer_id())
        .collect::<HashSet<_>>();
    let mut supervisor = Supervisor::new();
    let (endpoint, child) = IrohaNetwork::start_with_crypto_and_initial_authorities(
        P2pIdentityKeys::new(
            carrier_process
                .bls_key_pair()
                .ok_or_else(|| eyre!("transport process has no peer identity key"))?
                .clone(),
            carrier_process.soranet_transport_key_pair().clone(),
        )
        .map_err(|error| eyre!("transport identity: {error}"))?,
        config.network.clone(),
        network.network_id(),
        capabilities,
        targets.clone(),
        targets.clone(),
        None,
        supervisor.shutdown_signal(),
    )
    .await
    .map_err(|error| eyre!("start the transport endpoint: {error}"))?;
    supervisor.monitor(child);

    let delivered = async {
        endpoint.update_topology(UpdateTopology(targets.clone()));
        endpoint.update_peers_addresses(UpdatePeers(
            witness_processes
                .iter()
                .map(|process| (process.network_peer_id(), process.p2p_address()))
                .collect(),
        ));
        let mut online = endpoint.online_peers_receiver();
        tokio::time::timeout_at(deadline.into(), async {
            loop {
                if targets
                    .iter()
                    .all(|id| online.borrow().iter().any(|peer| peer.id() == id))
                {
                    return Ok::<_, eyre::Report>(());
                }
                online
                    .changed()
                    .await
                    .map_err(|_| eyre!("transport endpoint closed before every voter connected"))?;
            }
        })
        .await
        .wrap_err("voters did not all authenticate the transport endpoint")??;

        // Sign only now, with every session already authenticated. An uncommitted height is
        // not a fixed slot: voters keep rotating views without work, and a core accepts a
        // vote only within one view of its own. The highest view any voter reports is
        // inside every voter's window while their views differ by at most one.
        let (view, reported) = loop {
            let mut views = Vec::with_capacity(witness_processes.len());
            let mut reported = Vec::with_capacity(witness_processes.len());
            for process in &witness_processes {
                let status = consensus_status(process).await?;
                ensure!(
                    status.height == height
                        && status.applied_height == tip.height()
                        && status.halted.is_none(),
                    "voter {} left the authenticated tip before the offence",
                    process.id()
                );
                views.push(status.view);
                reported.push(status.footprint.evidence_keys);
            }
            let highest = *views
                .iter()
                .max()
                .ok_or_else(|| eyre!("committee has no witness besides the offender"))?;
            if views.iter().all(|view| view + 1 >= highest) {
                break (highest, reported);
            }
            ensure!(
                Instant::now() < deadline,
                "voters did not converge within one view of each other"
            );
            sleep(POLL).await;
        };
        let first = signed(view, b"first")?;
        let second = signed(view, b"second")?;
        ensure!(
            first.value() != second.value(),
            "the two votes must sign different values in one slot"
        );

        let transport = P2pNet::new(endpoint.clone());
        for vote in [first, second] {
            let frame = Frame {
                instance: vote.instance,
                class: TrafficClass::Control,
                bytes: Arc::from(
                    WireMessage::Vote(vote)
                        .encode()
                        .map_err(|error| eyre!("encode the signed vote: {error}"))?,
                ),
            };
            for process in &witness_processes {
                let recipient = core_key(process.network_peer_id().public_key())
                    .map_err(|error| eyre!("voter transport key: {error}"))?;
                ensure!(
                    matches!(transport.send(&recipient, &frame), SendOutcome::Admitted),
                    "the transport refused a signed vote for voter {}",
                    process.id()
                );
            }
        }
        // The core's deduplication key is the only pre-commit observation of a retained
        // conflict. Every witness must report exactly this one new offence; the key itself
        // leaves the core's view window later, so one observation per witness is retained.
        let mut retained = vec![false; witness_processes.len()];
        loop {
            for ((process, before), retained) in
                witness_processes.iter().zip(&reported).zip(&mut retained)
            {
                let status = consensus_status(process).await?;
                ensure!(
                    status.height == height && status.applied_height == tip.height(),
                    "the double-signed height advanced before every voter retained the conflict"
                );
                let observed = status.footprint.evidence_keys;
                ensure!(
                    observed <= before + 1,
                    "voter {} reported an unrelated consensus offence",
                    process.id()
                );
                *retained |= observed == before + 1;
            }
            if retained.iter().all(|retained| *retained) {
                return Ok((first, second));
            }
            ensure!(
                Instant::now() < deadline,
                "voters did not all retain the conflicting signed votes"
            );
            sleep(POLL).await;
        }
    }
    .await;
    supervisor.shutdown_signal().send();
    let closed = supervisor
        .start()
        .await
        .map_err(|error| eyre!("transport endpoint shutdown: {error:?}"));
    let (first, second) = delivered?;
    closed?;

    carrier_process
        .start_checked(
            network.config_layers_for_peer(carrier_process),
            Some(&genesis),
        )
        .await
        .wrap_err("restart the original transport process")?;
    Ok(DoubleSign {
        offender: offender.clone(),
        height,
        first,
        second,
    })
}

/// Read the committed record of exactly this double-sign, if a block has admitted it.
///
/// The evidence list is a node-local operator read that only the node's own key may sign, so it
/// goes through `reader`'s own client, never an account client re-pointed at another node.
async fn committed_record(
    reader: &NetworkPeer,
    sign: &DoubleSign,
) -> Result<Option<EvidenceRecord>> {
    let client = reader.client().client().clone();
    // One explicit maximal page: a qualification network retains far fewer records, and a
    // truncated page must never be read as absence.
    let page = read_on_dedicated_thread(move || {
        client.get_sumeragi_evidence_list_wire(SumeragiEvidenceListFilter {
            limit: Some(SUMERAGI_EVIDENCE_LIST_MAX_LIMIT),
            ..SumeragiEvidenceListFilter::default()
        })
    })
    .await
    .wrap_err("committed evidence worker failed")?;
    ensure!(
        page.total == u64::try_from(page.items.len())?,
        "committed evidence exceeds the one page this fixture reads"
    );
    let mut matching = page.items.into_iter().filter(|record| {
        record.attribution.height == sign.height
            && record
                .attribution
                .offenders
                .iter()
                .any(|offender| offender.peer_id == sign.offender)
    });
    let record = matching.next();
    ensure!(
        matching.next().is_none(),
        "one double-sign produced more than one committed evidence record"
    );
    let Some(record) = record else {
        return Ok(None);
    };
    let RecordedEvidenceV1::Native(proof) = &record.evidence else {
        return Err(eyre!("double-sign was recorded as non-native evidence"));
    };
    let NativeEvidence::VoteEquivocation(left, right) = proof
        .decode_native()
        .map_err(|error| eyre!("decode the committed proof: {error}"))?
    else {
        return Err(eyre!("double-sign was recorded as a different offence"));
    };
    ensure!(
        ((left, right) == (sign.first, sign.second) || (left, right) == (sign.second, sign.first))
            && record.attribution.scope == EvidenceScope::Root
            && record.attribution.instance == sign.first.instance.0
            && record.attribution.epoch == sign.first.epoch.epoch
            && record.attribution.context_id == sign.first.epoch.context.0
            && record.attribution.offenders.len() == 1
            && record.attribution.offenders[0].signer == sign.first.signer
            && record.attribution.offenders[0].lane_stake.is_none()
            && !record.attribution.safety_violation,
        "committed evidence is not the exact original double-sign attributed to its one signer"
    );
    Ok(Some(record))
}

/// Drive separately paid heights until the proof is admitted and its delayed slash applied.
///
/// A block led by the offender carries no admission, so the proof may enter one or more
/// heights after the offence. The penalty must then follow exactly `slashing_delay_blocks`
/// later. Every height is ordinary paid work signed by the current exact quorum. Committed
/// evidence is read through the honest `reader` node's own operator client.
pub async fn await_applied_penalty(
    network: &Network,
    voters: &[PeerId],
    reader: &NetworkPeer,
    sign: &DoubleSign,
    slashing_delay_blocks: u64,
) -> Result<AppliedPenalty> {
    let mut height = sign.height - 1;
    // Each voter leads at most once in a rotation; the delay then elapses.
    let bound = sign.height + u64::try_from(voters.len())? + slashing_delay_blocks;
    loop {
        if let Some(record) = committed_record(reader, sign).await? {
            ensure!(
                record.recorded_at_height >= sign.height && record.recorded_at_height <= height,
                "evidence was recorded outside the paid heights after its offence"
            );
            match record.penalty_status {
                EvidencePenaltyStatus::Applied { height: applied } => {
                    ensure!(
                        applied == record.recorded_at_height + slashing_delay_blocks
                            && applied <= height,
                        "the mandatory penalty did not follow the signed slashing delay"
                    );
                    return Ok(AppliedPenalty {
                        admitted_height: record.recorded_at_height,
                        applied_height: applied,
                    });
                }
                EvidencePenaltyStatus::Pending => ensure!(
                    height < record.recorded_at_height + slashing_delay_blocks,
                    "the mandatory penalty is overdue"
                ),
            }
        }
        ensure!(
            height < bound,
            "no honest leader admitted the retained double-sign"
        );
        height += 1;
        advance_to_height(network, voters, height).await?;
    }
}
