//! Peer registry and observer connectivity churn with a fixed authenticated voter set.
use eyre::{Result, WrapErr, eyre};
use integration_tests::{sandbox, sync::run_on_dedicated_thread};
use iroha::data_model::{
    Level, ValidationFail,
    isi::{
        InstructionBox, Log, Unregister,
        error::{InstructionExecutionError, InvalidParameterError},
        register::{RegisterCommitteePeerWithPop, RegisterPeerWithPop},
    },
    query::peer::prelude::FindPeers,
    transaction::error::TransactionRejectionReason,
};
use iroha::query::AsyncQueryBuilderExt as _;
use iroha_model_base::peer::PeerId;
use iroha_test_network::*;
use rand::{SeedableRng, prelude::IteratorRandom};
use rand_chacha::ChaCha8Rng;
use std::{collections::BTreeSet, time::Duration};
use tokio::time::{Instant, sleep};
#[tokio::test]
async fn connected_peers_with_seven_voters_and_observer() -> Result<()> {
    Box::pin(connected_peers_with_f(
        stringify!(connected_peers_with_seven_voters_and_observer),
        2,
    ))
    .await
}
#[tokio::test]
async fn connected_peers_with_four_voters_and_observer() -> Result<()> {
    Box::pin(connected_peers_with_f(
        stringify!(connected_peers_with_four_voters_and_observer),
        1,
    ))
    .await
}
#[tokio::test]
async fn register_new_peer() -> Result<()> {
    let Some(network) =
        sandbox::build_network_or_skip(connected_peer_builder(4)?, stringify!(register_new_peer))
    else {
        return Ok(());
    };
    let voters = authenticated_genesis_voters(&network, 4)?;
    let peer = &network.observers()[0];
    let submit_timeout = network
        .sync_timeout()
        .saturating_add(network.da_commit_quorum_timeout());
    let peer_sync_timeout = network
        .sync_timeout()
        .saturating_mul(2)
        .saturating_add(network.da_commit_quorum_timeout());
    let genesis = network.genesis();
    // Permissioned P2P trust is provisioned independently of registry membership.
    // The late process is trusted from bootstrap but stays offline until its
    // registry entry commits. It remains outside the authenticated voter roster.
    futures_util::future::try_join_all(network.validators().iter().map(|validator| {
        validator.start_checked(network.config_layers_for_peer(validator), Some(&genesis))
    }))
    .await?;
    assert!(!peer.is_running());
    assert_peers_status(network.validators(), 1, 3, network.sync_timeout()).await?;
    wait_for_peer_registry(network.validators(), &voters, network.sync_timeout()).await?;
    assert_consensus_roles(&network, false, network.sync_timeout()).await?;

    let register = RegisterPeerWithPop::new(
        peer.id(),
        peer.bls_pop()
            .expect("network peer should have BLS PoP")
            .to_vec(),
    );
    let client = integration_tests::sync::rebind_blocking_client(&network.client(), |client| {
        client.transaction_status_timeout = submit_timeout;
        client.transaction_ttl = Some(submit_timeout.saturating_add(Duration::from_secs(120)));
    });
    submit_instruction_or_warn(client.clone(), register, "register_new_peer").await?;
    let mut registered = voters;
    registered.insert(peer.id());
    wait_for_peer_registry(network.validators(), &registered, network.sync_timeout()).await?;
    assert_peers_status(network.validators(), 2, 3, network.sync_timeout()).await?;
    // RegisterPeerWithPop creates a candidate; it does not install a fifth voter.
    // This process can synchronize as an explicitly non-voting observer.
    peer.start(network.config_layers_for_peer(peer), Some(&genesis))
        .await?;
    assert_peers_status(network.all_peers(), 2, 4, peer_sync_timeout).await?;
    wait_for_peer_registry(network.all_peers(), &registered, network.sync_timeout()).await?;
    assert_consensus_roles(&network, true, network.sync_timeout()).await?;
    submit_instruction_or_warn(
        client,
        Log::new(Level::INFO, "register_new_peer_sync".to_owned()),
        "register_new_peer",
    )
    .await?;
    assert_peers_status(network.all_peers(), 3, 4, peer_sync_timeout).await?;
    wait_for_peer_registry(network.all_peers(), &registered, network.sync_timeout()).await?;
    assert_consensus_roles(&network, true, network.sync_timeout()).await?;
    Ok(())
}
/// Exercise registry and transport churn without changing authenticated voters.
#[allow(clippy::too_many_lines)]
async fn connected_peers_with_f(context: &'static str, faults: usize) -> Result<()> {
    let n_voters = 3 * faults + 1;
    let builder = connected_peer_builder(n_voters)?;
    let Some(network) = sandbox::start_network_async_or_skip(builder, context).await? else {
        return Ok(());
    };
    let voters = authenticated_genesis_voters(&network, n_voters)?;
    let observer = &network.observers()[0];
    let mut registered = voters.clone();
    registered.insert(observer.id());
    // Preserve the existing churn and submission bounds; no quorum changes occur.
    let sync_timeout = network.sync_timeout().saturating_mul(2);
    let submit_timeout = sync_timeout.saturating_add(network.da_commit_quorum_timeout());
    let client = integration_tests::sync::rebind_blocking_client(&network.client(), |client| {
        client.transaction_status_timeout = submit_timeout;
        client.transaction_ttl = Some(submit_timeout.saturating_add(Duration::from_secs(120)));
    });
    let connected_with_observer = u64::try_from(n_voters)?;
    let connected_without_observer = u64::try_from(n_voters - 1)?;
    assert_peers_status(
        network.all_peers(),
        1,
        connected_with_observer,
        sync_timeout,
    )
    .await?;
    wait_for_peer_registry(network.all_peers(), &voters, sync_timeout).await?;

    // Ordinary unregistration cannot rewrite the authenticated consensus roster.
    let error = submit_instruction_or_warn(
        client.clone(),
        Unregister::peer(network.validators()[0].id()),
        context,
    )
    .await
    .expect_err("a current authenticated voter must not unregister");
    eyre::ensure!(
        is_active_voter_refusal(&error),
        "expected the exact active-voter refusal, got {error:?}"
    );
    assert_peers_status(
        network.all_peers(),
        2,
        connected_with_observer,
        sync_timeout,
    )
    .await?;
    wait_for_peer_registry(network.all_peers(), &voters, sync_timeout).await?;
    assert_consensus_roles(&network, true, sync_timeout).await?;

    // This registry identity has a genuine PoP but no global voting authority.
    let registration = RegisterCommitteePeerWithPop::new(
        observer.id(),
        observer.bls_pop().expect("observer has a BLS PoP").to_vec(),
    );
    submit_instruction_or_warn(client.clone(), registration.clone(), context).await?;
    assert_peers_status(
        network.all_peers(),
        3,
        connected_with_observer,
        sync_timeout,
    )
    .await?;
    wait_for_peer_registry(network.all_peers(), &registered, sync_timeout).await?;
    assert_consensus_roles(&network, true, sync_timeout).await?;

    submit_instruction_or_warn(client.clone(), Unregister::peer(observer.id()), context).await?;
    wait_for_peer_registry(network.all_peers(), &voters, sync_timeout).await?;
    // The configured observer remains a trusted sync peer after registry removal.
    // Require its applied state to include the removal before stopping its process.
    assert_peers_status(
        network.all_peers(),
        4,
        connected_with_observer,
        sync_timeout,
    )
    .await?;
    observer.shutdown().await;
    assert_peers_status(
        network.validators(),
        4,
        connected_without_observer,
        sync_timeout,
    )
    .await?;
    submit_instruction_or_warn(
        client.clone(),
        Log::new(Level::INFO, "connected_peers_without_observer".to_owned()),
        context,
    )
    .await?;
    assert_peers_status(
        network.validators(),
        5,
        connected_without_observer,
        sync_timeout,
    )
    .await?;
    wait_for_peer_registry(network.validators(), &voters, sync_timeout).await?;
    assert_consensus_roles(&network, false, sync_timeout).await?;

    submit_instruction_or_warn(client.clone(), registration, context).await?;
    assert_peers_status(
        network.validators(),
        6,
        connected_without_observer,
        sync_timeout,
    )
    .await?;
    wait_for_peer_registry(network.validators(), &registered, sync_timeout).await?;
    observer
        .start(
            network.config_layers_for_peer(observer),
            Some(&network.genesis()),
        )
        .await?;
    assert_peers_status(
        network.all_peers(),
        6,
        connected_with_observer,
        sync_timeout,
    )
    .await?;
    submit_instruction_or_warn(
        client,
        Log::new(Level::INFO, "connected_peers_restarted_observer".to_owned()),
        context,
    )
    .await?;
    assert_peers_status(
        network.all_peers(),
        7,
        connected_with_observer,
        sync_timeout,
    )
    .await?;
    wait_for_peer_registry(network.all_peers(), &registered, sync_timeout).await?;
    assert_consensus_roles(&network, true, sync_timeout).await?;
    Ok(())
}

fn connected_peer_builder(n_voters: usize) -> Result<NetworkBuilder> {
    Ok(NetworkBuilder::new()
        .with_peers(n_voters)
        .with_block_cadence(Duration::from_secs(2))
        .with_observer_p2p_bootstrap(ObserverP2pBootstrap::new(1)?)?)
}

pub(super) fn authenticated_genesis_voters(
    network: &Network,
    expected: usize,
) -> Result<BTreeSet<PeerId>> {
    let genesis = network.genesis();
    let (epoch, _) =
        iroha_data_model::sumeragi_finality::authenticated_genesis(&genesis.0)?.into_parts();
    let voters: BTreeSet<_> = epoch
        .committee
        .iter()
        .map(|member| member.validator.clone())
        .collect();
    assert_eq!(voters.len(), expected);
    assert_eq!(
        voters,
        network.validators().iter().map(NetworkPeer::id).collect()
    );
    assert_eq!(network.observers().len(), 1);
    let observer = &network.observers()[0];
    for layer in network.config_layers().take(1) {
        let trusted = layer
            .get("trusted_peers")
            .and_then(toml::Value::as_array)
            .expect("explicit observer P2P trust");
        assert!(
            trusted
                .iter()
                .filter_map(toml::Value::as_str)
                .any(|entry| entry.starts_with(&format!("{}@", observer.id())))
        );
        assert_eq!(
            layer
                .get("trusted_peers_pop")
                .and_then(toml::Value::as_array)
                .expect("exact voting PoPs")
                .len(),
            expected
        );
    }
    assert!(!voters.contains(&observer.id()));
    assert!(
        !network
            .topology_entries()
            .iter()
            .any(|entry| entry.peer == observer.id())
    );
    for layer in network.config_layers() {
        if let Some(entries) = layer
            .get("trusted_peers_pop")
            .and_then(toml::Value::as_array)
        {
            assert!(entries.iter().all(
                |entry| entry.get("public_key").and_then(toml::Value::as_str)
                    != Some(observer.id().public_key().to_string().as_str())
            ));
        }
    }
    assert_eq!(
        network
            .config_layers_for_peer(observer)
            .last()
            .and_then(|layer| layer
                .get("sumeragi")?
                .get("role")?
                .as_str()
                .map(str::to_owned)),
        Some("observer".to_owned())
    );
    Ok(voters)
}

fn is_active_voter_refusal(error: &eyre::Report) -> bool {
    matches!(error.downcast_ref::<TransactionRejectionReason>(),
        Some(TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(message))
        ))) if message == "cannot unregister a peer from the current authenticated consensus roster")
}

// Observe every original voter signing and the configured observer abstaining.
// These diagnostics complement the signed-genesis and runtime refusal checks;
// they are not an independently verified current-committee certificate.
pub(super) async fn assert_consensus_roles(
    network: &Network,
    include_observer: bool,
    bound: Duration,
) -> Result<()> {
    let deadline = Instant::now() + bound;
    for peer in network
        .validators()
        .iter()
        .chain(include_observer.then(|| &network.observers()[0]))
    {
        eyre::ensure!(
            Instant::now() < deadline,
            "consensus role observation timed out"
        );
        let client = peer.client();
        let status = tokio::time::timeout_at(
            deadline,
            run_on_dedicated_thread(move || client.client().get_sumeragi_status()),
        )
        .await
        .wrap_err("consensus role observation timed out")???;
        eyre::ensure!(
            Instant::now() < deadline,
            "consensus role observation timed out"
        );
        assert!(
            status.halted.is_none(),
            "peer {} halted: {:?}",
            peer.id(),
            status.halted
        );
        let expected = network
            .validators()
            .iter()
            .any(|voter| voter.id() == peer.id())
            .then(|| peer.id().public_key().clone());
        assert_eq!(
            status.signer,
            expected,
            "peer {} changed consensus role",
            peer.id()
        );
    }
    Ok(())
}

fn registry_matches(actual: &[PeerId], expected: &BTreeSet<PeerId>) -> bool {
    actual.len() == expected.len() && actual.iter().cloned().collect::<BTreeSet<_>>() == *expected
}

pub(super) async fn wait_for_peer_registry(
    peers: impl IntoIterator<Item = &'_ NetworkPeer>,
    expected: &BTreeSet<PeerId>,
    bound: Duration,
) -> Result<()> {
    let peers: Vec<_> = peers.into_iter().collect();
    let deadline = Instant::now() + bound;
    loop {
        let mut mismatches = Vec::new();
        for peer in &peers {
            eyre::ensure!(
                Instant::now() < deadline,
                "peer registry deadline; expected {expected:?}, observed {mismatches:?}"
            );
            let roster = tokio::time::timeout_at(
                deadline,
                peer.client()
                    .account_client()
                    .query(FindPeers)
                    .execute_all(),
            )
            .await
            .wrap_err_with(|| {
                format!("peer registry deadline; expected {expected:?}, observed {mismatches:?}")
            })??;
            if !registry_matches(&roster, expected) {
                mismatches.push((peer.id(), roster));
            }
        }
        eyre::ensure!(
            Instant::now() < deadline,
            "peer registry deadline; expected {expected:?}, observed {mismatches:?}"
        );
        if mismatches.is_empty() {
            return Ok(());
        }
        sleep(
            deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(200)),
        )
        .await;
    }
}

// This route reads the online P2P snapshot directly; it does not turn registry
// membership into a transport count. Keep its scalar response bounded in memory.
pub(super) async fn online_peer_count(http: &reqwest::Client, peer: &NetworkPeer) -> Result<u64> {
    let url = peer.client().client().endpoint().join("/status/peers")?;
    let mut response = http
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await?;
    eyre::ensure!(
        response.status() == reqwest::StatusCode::OK,
        "online peer count returned {} for {}",
        response.status(),
        peer.id()
    );
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        eyre::ensure!(
            chunk.len() <= 64 - body.len(),
            "online peer count response exceeds 64 bytes"
        );
        body.extend_from_slice(&chunk);
    }
    decode_online_peer_count(&body)
}

fn decode_online_peer_count(body: &[u8]) -> Result<u64> {
    eyre::ensure!(
        body.len() <= 64,
        "online peer count response exceeds 64 bytes"
    );
    norito::json::from_slice(body).wrap_err("invalid online peer count")
}

#[test]
fn online_peer_count_requires_a_complete_bounded_unsigned_scalar() {
    for (body, count) in [
        ("0", 0),
        ("4", 4),
        (" 7\n", 7),
        ("18446744073709551615", u64::MAX),
    ] {
        assert_eq!(decode_online_peer_count(body.as_bytes()).unwrap(), count);
    }
    for body in [
        "",
        "-1",
        "4.5",
        "null",
        "true",
        "\"4\"",
        "{\"peers\":4}",
        "[4]",
        "4 5",
        "18446744073709551616",
    ] {
        assert!(
            decode_online_peer_count(body.as_bytes()).is_err(),
            "accepted {body:?}"
        );
    }
    assert!(decode_online_peer_count(&[b' '; 65]).is_err());
}

fn status_readiness_retry_delay(error: &eyre::Report, owner_running: bool) -> Option<Duration> {
    match error.downcast_ref::<iroha::Error>() {
        Some(iroha::Error::StatusUnavailable {
            reason: Some(iroha::StatusFailureReason::StateBusy),
            retry_after,
        }) => Some(Duration::from_millis(200).max(retry_after.unwrap_or_default())),
        Some(iroha::Error::Transport {
            operation: "diagnostic.status",
            kind: iroha::TransportErrorKind::Io(std::io::ErrorKind::ConnectionRefused),
            ..
        }) if owner_running => Some(Duration::from_millis(200)),
        _ => None,
    }
}

#[test]
fn status_readiness_retry_requires_exact_busy_or_live_owner_connection_refusal() {
    let refused = |operation, kind| {
        eyre::Report::from(iroha::Error::Transport {
            operation,
            kind,
            details: "controlled fixture refusal".to_owned(),
        })
    };
    let connection_refused = iroha::TransportErrorKind::Io(std::io::ErrorKind::ConnectionRefused);
    let status_refused = refused("diagnostic.status", connection_refused);
    assert_eq!(
        status_readiness_retry_delay(&status_refused, true),
        Some(Duration::from_millis(200))
    );
    assert_eq!(status_readiness_retry_delay(&status_refused, false), None);
    assert_eq!(
        status_readiness_retry_delay(&refused("diagnostic.peers", connection_refused), true),
        None
    );
    for kind in [
        iroha::TransportErrorKind::Io(std::io::ErrorKind::ConnectionReset),
        iroha::TransportErrorKind::Io(std::io::ErrorKind::TimedOut),
        iroha::TransportErrorKind::Io(std::io::ErrorKind::PermissionDenied),
        iroha::TransportErrorKind::Other,
    ] {
        assert_eq!(
            status_readiness_retry_delay(&refused("diagnostic.status", kind), true),
            None
        );
    }
    let busy = eyre::Report::from(iroha::Error::StatusUnavailable {
        reason: Some(iroha::StatusFailureReason::StateBusy),
        retry_after: Some(Duration::from_secs(1)),
    });
    for running in [false, true] {
        assert_eq!(
            status_readiness_retry_delay(&busy, running),
            Some(Duration::from_secs(1))
        );
    }
    for error in [
        eyre::Report::from(iroha::Error::StatusUnavailable {
            reason: None,
            retry_after: None,
        }),
        eyre::Report::from(iroha::Error::Http {
            operation: "diagnostic.status",
            status: 503,
            retry_after: None,
            body: b"status_state_busy".to_vec(),
        }),
        eyre!("diagnostic.status connection refused status_state_busy"),
    ] {
        assert_eq!(status_readiness_retry_delay(&error, true), None);
    }
}

pub(super) async fn assert_peers_status(
    peers: impl IntoIterator<Item = &'_ NetworkPeer>,
    expected_blocks: u64,
    expected_peers: u64,
    bound: Duration,
) -> Result<()> {
    let peers: Vec<_> = peers.into_iter().collect();
    let deadline = Instant::now() + bound;
    let http = reqwest::Client::builder()
        .timeout(integration_tests::http::request_timeout())
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let mut previous_mismatches = Vec::new();
    loop {
        let mut mismatches = Vec::new();
        let mut retry_delay = Duration::from_millis(200);
        for peer in &peers {
            eyre::ensure!(
                Instant::now() < deadline,
                "peer status deadline: current={mismatches:?}, previous={previous_mismatches:?}"
            );
            let result = tokio::time::timeout_at(deadline, peer.status())
                .await
                .wrap_err_with(|| format!("peer status deadline: current={mismatches:?}, previous={previous_mismatches:?}"))?;
            match result {
                Ok(status) => {
                    // The classified status deliberately reports zero registered
                    // peers on an unregistered observer. Transport connectivity
                    // is a separate observation from the canonical live endpoint.
                    let online = tokio::time::timeout_at(
                        deadline,
                        online_peer_count(&http, peer),
                    )
                    .await
                    .wrap_err_with(|| format!(
                        "peer transport deadline: current={mismatches:?}, previous={previous_mismatches:?}"
                    ))??;
                    if online != expected_peers || status.blocks < expected_blocks {
                        mismatches.push(format!(
                            "{}: online={} expected={}, classified_peers={}, applied={} minimum={}",
                            peer.id(),
                            online,
                            expected_peers,
                            status.peers,
                            status.blocks,
                            expected_blocks
                        ));
                    }
                }
                Err(error) => {
                    let Some(delay) = status_readiness_retry_delay(&error, peer.is_running())
                    else {
                        return Err(error);
                    };
                    retry_delay = retry_delay.max(delay);
                    mismatches.push(format!(
                        "{}: {error}; typed={:?}",
                        peer.id(),
                        error.downcast_ref::<iroha::Error>()
                    ));
                }
            }
        }
        eyre::ensure!(
            Instant::now() < deadline,
            "peer status deadline: current={mismatches:?}, previous={previous_mismatches:?}"
        );
        if mismatches.is_empty() {
            return Ok(());
        }
        previous_mismatches = mismatches;
        sleep(
            deadline
                .saturating_duration_since(Instant::now())
                .min(retry_delay),
        )
        .await;
    }
}

#[test]
fn connected_peer_fixture_keeps_authenticated_voters_separate_from_observer() -> Result<()> {
    for count in [4, 7] {
        let network = connected_peer_builder(count)?.build();
        let voters = authenticated_genesis_voters(&network, count)?;
        assert!(!network.observers()[0].is_running());
        let exact = voters.iter().cloned().collect::<Vec<_>>();
        assert!(registry_matches(&exact, &voters));
        assert!(!registry_matches(&exact[..exact.len() - 1], &voters));
        let mut substituted = exact.clone();
        substituted[0] = network.observers()[0].id();
        assert!(!registry_matches(&substituted, &voters));
        let mut duplicate = exact.clone();
        duplicate[0] = duplicate[1].clone();
        assert!(!registry_matches(&duplicate, &voters));
    }
    Ok(())
}

#[test]
fn active_voter_refusal_requires_the_exact_typed_rejection() {
    let rejection = |message: &str| {
        TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message.to_owned(),
            )),
        ))
    };
    assert!(is_active_voter_refusal(&eyre::Report::from(rejection(
        "cannot unregister a peer from the current authenticated consensus roster"
    ))));
    assert!(!is_active_voter_refusal(&eyre::Report::from(rejection(
        "unrelated invalid parameter"
    ))));
    assert!(!is_active_voter_refusal(&eyre!(
        "cannot unregister a peer from the current authenticated consensus roster"
    )));
    assert!(!is_active_voter_refusal(&eyre::Report::from(
        TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
            "cannot unregister a peer from the current authenticated consensus roster".to_owned()
        ))
    )));
}

async fn submit_instruction_or_warn(
    client: iroha::blocking::Client,
    instruction: impl Into<InstructionBox>,
    context: &str,
) -> Result<()> {
    let instruction = instruction.into();
    let context = context.to_string();
    run_on_dedicated_thread(move || {
        client
            .submit(
                instruction,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .map(|_| ())
            .wrap_err(context)
    })
    .await??;
    Ok(())
}

fn leader_peer<'a>(peers: impl IntoIterator<Item = &'a NetworkPeer>) -> &'a NetworkPeer {
    peers
        .into_iter()
        .min_by(|left, right| left.id().cmp(&right.id()))
        .expect("network should have at least one peer")
}

#[allow(clippy::too_many_lines)]
async fn connected_validators_restart_with_f(context: &'static str, faults: usize) -> Result<()> {
    let n_peers = 3 * faults + 1;
    let builder = NetworkBuilder::new()
        .with_peers(n_peers)
        .with_block_cadence(Duration::from_secs(2));
    let Some(network) = sandbox::start_network_async_or_skip(builder, context).await? else {
        return Ok(());
    };
    let sync_timeout = network.sync_timeout().saturating_mul(2);
    let submit_timeout = sync_timeout.saturating_add(network.da_commit_quorum_timeout());
    assert_peers_status(
        network.peers(),
        1,
        expected_connected_peers(n_peers),
        sync_timeout,
    )
    .await?;
    let mut randomized_peers = {
        let mut rng = ChaCha8Rng::seed_from_u64(0x434f_4e4e);
        network.peers().iter().sample(&mut rng, n_peers)
    };
    let restarted_peer = randomized_peers.remove(0);
    let client = leader_peer(randomized_peers.iter().copied()).client();
    let client = integration_tests::sync::rebind_blocking_client(&client, |client| {
        client.transaction_status_timeout = submit_timeout;
        client.transaction_ttl = Some(submit_timeout.saturating_add(Duration::from_secs(120)));
    });
    let expected_roster = network
        .peers()
        .iter()
        .map(NetworkPeer::id)
        .collect::<BTreeSet<_>>();
    let rejection = submit_instruction_or_warn(
        client.clone(),
        Unregister::peer(restarted_peer.id()),
        context,
    )
    .await
    .expect_err("an active authenticated validator cannot be unregistered");
    assert!(
        is_active_voter_refusal(&rejection),
        "unexpected unregister rejection: {rejection:?}"
    );
    let retained_roster = client
        .account_client()
        .query(FindPeers)
        .execute_all()
        .await?
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        retained_roster, expected_roster,
        "rejected removal must preserve every roster identity"
    );
    // Rejection is itself a committed result; every validator must retain it before shutdown.
    assert_peers_status(
        network.peers(),
        2,
        expected_connected_peers(n_peers),
        sync_timeout,
    )
    .await?;
    restarted_peer.shutdown().await;
    assert_peers_status(
        randomized_peers.iter().copied(),
        2,
        expected_connected_peers(n_peers - 1),
        sync_timeout,
    )
    .await?;
    // The original committee still has its exact quorum (3/4 or at least 5/7).
    // A real transaction must apply while the one owned validator is offline.
    submit_instruction_or_warn(
        client,
        Log::new(
            Level::INFO,
            "connected_peers_with_one_validator_offline".to_owned(),
        ),
        context,
    )
    .await?;
    assert_peers_status(
        randomized_peers.iter().copied(),
        3,
        expected_connected_peers(n_peers - 1),
        sync_timeout,
    )
    .await?;
    let genesis = network.genesis();
    restarted_peer
        .start(
            network.config_layers_for_peer(restarted_peer),
            Some(&genesis),
        )
        .await?;
    assert_peers_status(
        std::iter::once(restarted_peer),
        3,
        expected_connected_peers(n_peers),
        sync_timeout,
    )
    .await?;
    assert_peers_status(
        network.peers(),
        3,
        expected_connected_peers(n_peers),
        sync_timeout,
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn connected_validators_restart_with_f_1() -> Result<()> {
    Box::pin(connected_validators_restart_with_f(
        stringify!(connected_validators_restart_with_f_1),
        1,
    ))
    .await
}

#[tokio::test]
async fn connected_validators_restart_with_f_2() -> Result<()> {
    Box::pin(connected_validators_restart_with_f(
        stringify!(connected_validators_restart_with_f_2),
        2,
    ))
    .await
}

fn expected_connected_peers(running_peers: usize) -> u64 {
    u64::try_from(running_peers.saturating_sub(1)).expect("fixture peer count fits u64")
}

#[test]
fn connected_peer_counts_track_running_processes_without_resizing_quorums() {
    assert_eq!(expected_connected_peers(4), 3);
    assert_eq!(expected_connected_peers(3), 2);
    assert_eq!(expected_connected_peers(7), 6);
    assert_eq!(expected_connected_peers(6), 5);
    assert_eq!(iroha_sumeragi::types::quorum(4), 3);
    assert_eq!(iroha_sumeragi::types::quorum(7), 5);
}
