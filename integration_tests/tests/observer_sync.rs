//! Observer/sync-only node catches up behind a small validator swarm.
use eyre::{Result, WrapErr, eyre};
use integration_tests::sandbox;
use iroha::data_model::{
    isi::register::RegisterPeerWithPop,
    prelude::*,
    query::{parameters::FetchSize, peer::prelude::FindPeers},
};
use iroha::query::AsyncQueryBuilderExt as _;
use iroha_config::base::toml::WriteExt as _;
use iroha_model_base::{name::Name, peer::PeerId};
use iroha_primitives::unique_vec::UniqueVec;
use iroha_test_network::{
    NetworkBuilder, NetworkPeer, NetworkPeerBuilder, init_instruction_registry,
};
use iroha_test_samples::ALICE_ID;
use norito::json::Value as JsonValue;
use std::{
    collections::{HashMap, HashSet},
    num::NonZeroU64,
    time::Duration,
};
use toml::{Table, Value as TomlValue};

// Runtime registration grants the observer delegated-read ingress authority;
// it does not add a validator to the authenticated genesis committee.
fn first_observer_transaction(observer_id: PeerId, pop: Vec<u8>) -> [InstructionBox; 2] {
    [
        RegisterPeerWithPop::new(observer_id, pop).into(),
        SetKeyValue::account(
            ALICE_ID.clone(),
            "note".parse::<Name>().expect("valid metadata key"),
            norito::json!("v1"),
        )
        .into(),
    ]
}

fn checked_history_body(status: reqwest::StatusCode, body: String) -> Result<String> {
    if status.is_success() {
        return Ok(body);
    }
    // Keep complete successful responses; bound only failed-response diagnostics.
    let prefix = String::from_utf8_lossy(&body.as_bytes()[..body.len().min(4096)]);
    Err(eyre!(
        "history request returned {status}; body bytes={}; prefix={prefix:?}",
        body.len()
    ))
}

#[test]
fn first_observer_transaction_preserves_real_pop_and_initial_metadata() {
    init_instruction_registry();
    let key = iroha_crypto::KeyPair::from_seed(
        b"observer registration fixture".to_vec(),
        iroha_crypto::Algorithm::BlsNormal,
    );
    let peer = PeerId::new(key.public_key().clone());
    let pop = iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("real BLS PoP");
    let instructions = first_observer_transaction(peer.clone(), pop.clone());
    assert_eq!(instructions.len(), 2);
    let Some(iroha_data_model::isi::RegisterBox::Peer(registration)) =
        instructions[0]
            .as_any()
            .downcast_ref::<iroha_data_model::isi::RegisterBox>()
    else {
        panic!("ordinary runtime peer registration");
    };
    assert_eq!(registration.peer, peer);
    assert_eq!(registration.pop, pop);
    assert_eq!(registration.activation_at, None);
    assert_eq!(registration.expiry_at, None);
    iroha_crypto::bls_normal_pop_verify(registration.peer.public_key(), &registration.pop)
        .expect("registration retains the observer's valid proof");
    assert_eq!(
        instructions[1],
        InstructionBox::from(SetKeyValue::account(
            ALICE_ID.clone(),
            "note".parse::<Name>().unwrap(),
            norito::json!("v1"),
        ))
    );
}

#[test]
fn history_response_diagnostics_preserve_refusal_and_bound_display() {
    let complete = "x".repeat(8192);
    assert_eq!(
        checked_history_body(reqwest::StatusCode::OK, complete.clone()).unwrap(),
        complete
    );
    let refusal = checked_history_body(
        reqwest::StatusCode::FORBIDDEN,
        r#"{"code":"untrusted_proxy_ingress"}"#.to_owned(),
    )
    .unwrap_err()
    .to_string();
    assert!(refusal.contains("403 Forbidden"));
    assert!(refusal.contains("untrusted_proxy_ingress"));
    let body = format!("{}undisplayed-tail", "é".repeat(4096));
    let expected_count = format!("body bytes={}", body.len());
    let diagnostic = checked_history_body(reqwest::StatusCode::BAD_REQUEST, body)
        .unwrap_err()
        .to_string();
    assert!(diagnostic.contains("400 Bad Request"));
    assert!(diagnostic.contains(&expected_count));
    assert!(!diagnostic.contains("undisplayed-tail"));
    assert!(diagnostic.len() < 4200);
}

fn applied_heights_match(heights: &[u64], expected_peers: usize, target: u64) -> Result<bool> {
    eyre::ensure!(
        expected_peers > 0 && heights.len() == expected_peers,
        "expected {expected_peers} applied heights, got {heights:?}"
    );
    eyre::ensure!(
        heights.iter().all(|&height| height <= target),
        "unexpected applied height above {target}: {heights:?}"
    );
    Ok(heights.iter().all(|&height| height == target))
}

fn status_busy_retry_delay(error: &eyre::Report) -> Option<Duration> {
    match error.downcast_ref::<iroha::Error>() {
        Some(iroha::Error::StatusUnavailable {
            reason: Some(iroha::StatusFailureReason::StateBusy),
            retry_after,
        }) => Some(Duration::from_millis(200).max(retry_after.unwrap_or_default())),
        _ => None,
    }
}

// Both encodings can refuse a read while State is being published. Keep the
// original phase deadline across all readers and honor only that typed refusal.
async fn read_status_until<T, F>(
    deadline: tokio::time::Instant,
    mut read: impl FnMut() -> F,
) -> Result<T>
where
    F: std::future::Future<Output = Result<T>>,
{
    let mut last_busy: Option<eyre::Report> = None;
    loop {
        if tokio::time::Instant::now() >= deadline {
            break;
        }
        match tokio::time::timeout_at(deadline, read()).await {
            Ok(Ok(value)) if tokio::time::Instant::now() < deadline => return Ok(value),
            Err(_) | Ok(Ok(_)) => break,
            Ok(Err(err)) => {
                let Some(delay) = status_busy_retry_delay(&err) else {
                    return Err(err);
                };
                last_busy = Some(err);
                let now = tokio::time::Instant::now();
                if delay >= deadline.saturating_duration_since(now) {
                    break;
                }
                tokio::time::sleep_until(now + delay).await;
            }
        }
    }
    Err(last_busy.unwrap_or_else(|| eyre!("status read deadline elapsed")))
}

async fn wait_for_applied_heights(
    peers: &[&NetworkPeer],
    target: u64,
    sync_timeout: Duration,
) -> Result<Vec<u64>> {
    let deadline = tokio::time::Instant::now() + sync_timeout;
    let mut heights = Vec::with_capacity(peers.len());
    loop {
        heights.clear();
        for peer in peers {
            let status = read_status_until(deadline, || peer.status())
                .await
                .wrap_err_with(|| format!("peer {} did not report its applied height before {target} check; prior peers: {heights:?}", peer.id()))?;
            heights.push(status.blocks);
        }
        if applied_heights_match(&heights, peers.len(), target)? {
            return Ok(heights);
        }
        let now = tokio::time::Instant::now();
        eyre::ensure!(
            now < deadline,
            "applied heights did not reach {target} within {sync_timeout:?}: {heights:?}"
        );
        tokio::time::sleep_until((now + Duration::from_millis(200)).min(deadline)).await;
    }
}

#[tokio::test(start_paused = true)]
async fn status_reads_retry_only_explicit_contention_within_one_deadline() {
    use std::future::ready;
    use tokio::time::Instant;
    let busy = |after| {
        eyre::Report::from(iroha::Error::StatusUnavailable {
            reason: Some(iroha::StatusFailureReason::StateBusy),
            retry_after: Some(after),
        })
    };
    let start = Instant::now();
    let deadline = start + Duration::from_secs(3);
    let mut attempts = 0;
    let observed = read_status_until(deadline, || {
        attempts += 1;
        ready(if attempts == 1 {
            Err(busy(Duration::from_secs(1)))
        } else {
            Ok(6)
        })
    })
    .await
    .unwrap();
    assert_eq!(
        (observed, attempts, Instant::now() - start),
        (6, 2, Duration::from_secs(1))
    );

    // The second reader cannot start a fresh budget or wait past the first one's deadline.
    let mut calls = 0;
    let err = read_status_until::<(), _>(deadline, || {
        calls += 1;
        ready(Err(busy(Duration::from_secs(2))))
    })
    .await
    .unwrap_err();
    assert_eq!(calls, 1);
    assert_eq!(status_busy_retry_delay(&err), Some(Duration::from_secs(2)));
    let mut calls = 0;
    let err = read_status_until::<(), _>(deadline, || {
        calls += 1;
        ready(Err(eyre!("unclassified 503")))
    })
    .await
    .unwrap_err();
    assert_eq!((calls, err.to_string()), (1, "unclassified 503".to_owned()));
    assert!(
        read_status_until(deadline, || async {
            tokio::time::sleep_until(deadline).await;
            Ok(6)
        })
        .await
        .is_err()
    );
    assert_eq!(Instant::now(), deadline);
}

#[test]
fn applied_status_retry_requires_the_exact_typed_state_busy_reason() {
    let busy = iroha::Error::StatusUnavailable {
        reason: Some(iroha::StatusFailureReason::StateBusy),
        retry_after: Some(Duration::from_secs(1)),
    };
    assert_eq!(
        status_busy_retry_delay(&eyre::Report::from(busy)),
        Some(Duration::from_secs(1))
    );
    for error in [
        iroha::Error::StatusUnavailable {
            reason: None,
            retry_after: None,
        },
        iroha::Error::Http {
            operation: "diagnostic.status",
            status: 503,
            retry_after: None,
            body: b"status_state_busy".to_vec(),
        },
    ] {
        assert!(status_busy_retry_delay(&eyre::Report::from(error)).is_none());
    }
    assert!(status_busy_retry_delay(&eyre!("status_state_busy")).is_none());
}

#[test]
fn applied_height_readiness_requires_every_peer_at_the_exact_target() {
    assert!(!applied_heights_match(&[6, 6, 6, 6, 5], 5, 6).unwrap());
    assert!(!applied_heights_match(&[5; 5], 5, 6).unwrap());
    assert!(applied_heights_match(&[6; 5], 5, 6).unwrap());
    assert!(applied_heights_match(&[6; 4], 5, 6).is_err());
    assert!(applied_heights_match(&[7; 5], 5, 6).is_err());
    assert!(applied_heights_match(&[], 0, 6).is_err());
}

#[test]
#[allow(clippy::too_many_lines)]
fn observer_node_catches_up() -> Result<()> {
    init_instruction_registry();
    // Prepare a validator network that satisfies DA quorum requirements.
    let Some((network, rt)) = sandbox::build_network_blocking_or_skip(
        NetworkBuilder::new().with_min_peers(4),
        stringify!(observer_node_catches_up),
    ) else {
        return Ok(());
    };
    // Build observer peer (sync-only) ahead of validator bootstrap so validators trust it.
    let observer = NetworkPeerBuilder::new()
        .with_seed(Some(b"observer"))
        .build(network.env());
    network.attach_peer_client_context(&observer)?;
    // Prepare a trusted_peers override including existing peers and the observer itself.
    // The observer is trusted for networking/block sync, but it is intentionally omitted
    // from `trusted_peers_pop` so the validator roster remains the four validator peers.
    let mut tp: UniqueVec<Peer> = network
        .peers()
        .iter()
        .map(|p| Peer::new(p.p2p_address(), p.id()))
        .collect();
    // Add observer itself to trusted peers; ignore duplicates
    let _ = tp.push(Peer::new(observer.p2p_address(), observer.id()));
    let mut pops_by_peer_id = HashMap::new();
    for peer in network.peers() {
        let pop = peer.bls_pop().expect("network peers should have BLS PoPs");
        pops_by_peer_id.insert(peer.id(), pop.to_vec());
    }
    let trusted_peers: Vec<String> = tp.iter().map(std::string::ToString::to_string).collect();
    let mut trusted_peers_pop = Vec::new();
    let mut seen = HashSet::new();
    for peer in network.peers() {
        let peer_id = peer.id();
        if !seen.insert(peer_id.clone()) {
            continue;
        }
        let pop = pops_by_peer_id
            .get(&peer_id)
            .unwrap_or_else(|| panic!("missing PoP for validator peer {}", peer_id.public_key()));
        let mut pop_entry = Table::new();
        pop_entry.insert(
            "public_key".into(),
            TomlValue::String(peer_id.public_key().to_string()),
        );
        pop_entry.insert(
            "pop_hex".into(),
            TomlValue::String(format!("0x{}", hex::encode(pop))),
        );
        trusted_peers_pop.push(TomlValue::Table(pop_entry));
    }
    let trusted_observer_layer = Table::new()
        .write(["trusted_peers"], trusted_peers.clone())
        .write(
            ["trusted_peers_pop"],
            TomlValue::Array(trusted_peers_pop.clone()),
        );
    // Start validators with the observer in trusted peers so permissioned networking accepts it.
    let validator_layers: Vec<_> = network
        .config_layers()
        .chain(std::iter::once(std::borrow::Cow::Owned(
            trusted_observer_layer.clone(),
        )))
        .collect();
    let genesis = network.genesis();
    let (epoch, _) =
        iroha_data_model::sumeragi_finality::authenticated_genesis(&genesis.0)?.into_parts();
    let voters: HashSet<_> = epoch
        .committee
        .iter()
        .map(|member| member.validator.clone())
        .collect();
    assert_eq!(voters.len(), 4);
    assert_eq!(
        voters,
        network.peers().iter().map(NetworkPeer::id).collect()
    );
    assert!(!voters.contains(&observer.id()));
    for peer in network.peers() {
        sandbox::handle_result(
            rt.block_on(async {
                peer.start_checked(validator_layers.iter(), Some(&genesis))
                    .await
            }),
            "observer_node_catches_up_start_validator",
        )?;
    }
    let override_layer = trusted_observer_layer
        .write(["sumeragi", "role"], "observer")
        .write(["logger", "level"], "INFO");
    // Start the observer with role override and extended trusted peers
    if sandbox::handle_result(
        rt.block_on(async {
            observer
                .start_checked(
                    network
                        .config_layers()
                        .chain(std::iter::once(std::borrow::Cow::Owned(
                            override_layer.clone(),
                        ))),
                    Some(&network.genesis()),
                )
                .await
        }),
        "observer_node_catches_up_start_observer",
    )?
    .is_none()
    {
        return Ok(());
    }
    let sync_timeout = network.sync_timeout();
    let wait_for_observer = |height| -> Result<()> {
        rt.block_on(async {
            tokio::time::timeout(sync_timeout, observer.once_block(height))
                .await
                .map_err(|_| {
                    eyre!("observer did not reach height {height} within {sync_timeout:?}")
                })
        })?;
        Ok(())
    };
    // Observer should have at least the genesis block
    wait_for_observer(1)?;
    // Produce non-genesis blocks and verify height parity across all peers
    // Baseline timestamp (ms) to filter POST /v1/accounts/:id/transactions/query later.
    let now_ms = || -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        u64::try_from(ms).unwrap_or(u64::MAX)
    };
    let _t0 = now_ms();
    let alice = ALICE_ID.clone();
    let key: Name = "note".parse().unwrap();
    let client =
        integration_tests::sync::rebind_blocking_client(&network.peer().client(), |client| {
            client.transaction_status_timeout = std::time::Duration::from_millis(180_000);
        });
    // 1st block: set note = "v1"
    println!("observer_sync: submitting v1");
    let t1_lo = now_ms();
    // The default signed genesis grants Alice CanManagePeers. Register the
    // observer with its genuine PoP in this existing transaction so delegated
    // history reads are authorized without an extra height or history row.
    client.submit_all(
        first_observer_transaction(
            observer.id(),
            observer.bls_pop().expect("observer has a BLS PoP").to_vec(),
        ),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    println!("observer_sync: v1 committed");
    // Wait until validators reach total >= 2 and observer catches up
    rt.block_on(async { network.ensure_blocks_with(|h| h.total >= 2).await })?;
    wait_for_observer(2)?;
    // 2nd block: set note = "v2"
    let _t2_lo = now_ms();
    println!("observer_sync: submitting v2");
    client.submit(
        SetKeyValue::account(alice, key, norito::json!("v2")),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    println!("observer_sync: v2 committed");
    // Wait until validators reach total >= 3 and observer catches up
    rt.block_on(async { network.ensure_blocks_with(|h| h.total >= 3).await })?;
    wait_for_observer(3)?;
    // 3rd block: change some other metadata to ensure another non-genesis block
    let alice2 = ALICE_ID.clone();
    let key2: Name = "znote".parse().unwrap();
    let _t3_lo = now_ms();
    println!("observer_sync: submitting v3");
    client.submit(
        SetKeyValue::account(alice2, key2, norito::json!("v3")),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    println!("observer_sync: v3 committed");
    // Wait until validators reach total >= 4 and observer catches up
    rt.block_on(async { network.ensure_blocks_with(|h| h.total >= 4).await })?;
    wait_for_observer(4)?;
    // 4th block: set note = "v4"
    let t4_lo = now_ms();
    println!("observer_sync: submitting v4");
    client.submit(
        SetKeyValue::account(
            ALICE_ID.clone(),
            "note".parse::<Name>().unwrap(),
            norito::json!("v4"),
        ),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    println!("observer_sync: v4 committed");
    rt.block_on(async { network.ensure_blocks_with(|h| h.total >= 5).await })?;
    wait_for_observer(5)?;
    // 5th block: set znote = "v5"
    let t5_lo = now_ms();
    println!("observer_sync: submitting v5");
    client.submit(
        SetKeyValue::account(
            ALICE_ID.clone(),
            "znote".parse::<Name>().unwrap(),
            norito::json!("v5"),
        ),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    println!("observer_sync: v5 committed");
    rt.block_on(async { network.ensure_blocks_with(|h| h.total >= 6).await })?;
    wait_for_observer(6)?;
    let t_hi = now_ms();
    // Kura/once_block can reach six before the observer publishes applied state.
    // Require actual /status height six on all four validators and the observer
    // under the original synchronization timeout before asserting exact parity.
    let peers: Vec<_> = network
        .peers()
        .iter()
        .chain(std::iter::once(&observer))
        .collect();
    assert_eq!(peers.len(), 5);
    let all_totals = rt.block_on(wait_for_applied_heights(&peers, 6, sync_timeout))?;
    assert!(
        all_totals.iter().all(|&t| t == all_totals[0]),
        "heights differ: {all_totals:?}"
    );
    // Registry membership authorizes read delegation, independently of voting.
    // Require the complete five-peer registry and unchanged four signing roles.
    rt.block_on(async {
        let deadline = tokio::time::Instant::now() + sync_timeout;
        tokio::time::timeout_at(deadline, async {
            let expected_registry: HashSet<_> = peers.iter().map(|peer| peer.id()).collect();
            assert_eq!(expected_registry.len(), 5);
            for peer in &peers {
                eyre::ensure!(
                    tokio::time::Instant::now() < deadline,
                    "observer registry/role verification deadline"
                );
                let client = peer.client();
                let account = client.account_client();
                let mut rows = account
                    .query(FindPeers)
                    .with_fetch_size(FetchSize {
                        fetch_size: NonZeroU64::new(5),
                    })
                    .execute()
                    .await?;
                eyre::ensure!(
                    !rows.has_more(),
                    "observer registry must be one complete page"
                );
                let mut registry = Vec::with_capacity(5);
                while let Some(row) = rows.next().await {
                    registry.push(row?);
                    eyre::ensure!(registry.len() <= 5, "observer registry exceeded five peers");
                }
                eyre::ensure!(
                    tokio::time::Instant::now() < deadline,
                    "observer registry/role verification deadline"
                );
                assert_eq!(registry.len(), 5, "peer {} registry count", peer.id());
                assert_eq!(
                    registry.into_iter().collect::<HashSet<_>>(),
                    expected_registry
                );
                let operator = client.client().with_request_deadline(deadline.into_std());
                let status = integration_tests::sync::run_on_dedicated_thread(move || {
                    operator.get_sumeragi_status()
                })
                .await??;
                eyre::ensure!(
                    tokio::time::Instant::now() < deadline,
                    "observer registry/role verification deadline"
                );
                assert!(status.halted.is_none(), "peer {} halted", peer.id());
                let expected_signer = voters
                    .contains(&peer.id())
                    .then(|| peer.id().public_key().clone());
                assert_eq!(status.signer, expected_signer, "peer {} role", peer.id());
            }
            eyre::ensure!(
                tokio::time::Instant::now() < deadline,
                "observer registry/role verification deadline"
            );
            Ok::<(), eyre::Report>(())
        })
        .await
        .wrap_err("observer registry/role verification exceeded the synchronization timeout")?
    })?;
    // Verify state reflects latest metadata values on all peers (validators + observer).
    // Kura heights can advance before state/indices are fully applied, so poll for metadata.
    let note_key: Name = "note".parse().unwrap();
    let znote_key: Name = "znote".parse().unwrap();
    let wait_for_metadata = |peer: &NetworkPeer| -> Result<()> {
        let deadline = std::time::Instant::now() + sync_timeout;
        let peer_id = peer.id().to_string();
        let mut last_note: Option<String> = None;
        let mut last_zone_note: Option<String> = None;
        loop {
            let client = peer.client();
            let alice = client
                .client()
                .query_single(FindAccountById::new(ALICE_ID.clone()))?;
            assert_eq!(alice.id(), &*ALICE_ID);
            if let Some(note) = alice.metadata().get(&note_key) {
                last_note = Some(note.as_ref().to_string());
            }
            if let Some(znote) = alice.metadata().get(&znote_key) {
                last_zone_note = Some(znote.as_ref().to_string());
            }
            if last_note.as_deref() == Some("v4") && last_zone_note.as_deref() == Some("v5") {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                return Err(eyre!(
                    "peer {peer_id} did not observe note=v4/znote=v5 within {sync_timeout:?} (last note={last_note:?}, znote={last_zone_note:?})"
                ));
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    };
    for p in network.peers() {
        wait_for_metadata(p)?;
    }
    wait_for_metadata(&observer)?;
    // Redundant HTTP verification via Torii: fetch last transactions for Alice from each peer
    let alice_id_str = format!("{}", *ALICE_ID);
    let fetch_tx_http = |peer: &NetworkPeer, limit: usize| -> Result<(u64, Vec<JsonValue>)> {
        let url = format!(
            "{}/v1/accounts/{}/transactions?limit={}",
            peer.torii_url(),
            *ALICE_ID,
            limit
        );
        let body = rt.block_on(async {
            let response = integration_tests::http::client()
                .get(url)
                .header("Accept", "application/json")
                .send()
                .await?;
            let status = response.status();
            checked_history_body(status, response.text().await?)
        })?;
        let parsed: JsonValue = norito::json::from_str(&body)?;
        let (items, total) = match parsed {
            JsonValue::Object(m) => {
                let total = m
                    .get("items")
                    .and_then(norito::json::Value::as_array)
                    .map_or(0, |items| items.len() as u64);
                let items = m
                    .get("items")
                    .and_then(|v| match v {
                        JsonValue::Array(a) => Some(a.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                (items, total)
            }
            _ => (Vec::new(), 0),
        };
        Ok((total, items))
    };
    for p in network.peers() {
        let (total, items) = fetch_tx_http(p, 5)?;
        assert!(
            total >= 5,
            "expected at least 5 Alice txs via HTTP; got {total}"
        );
        assert!(
            !items.is_empty(),
            "expected items via HTTP for {}",
            p.torii_url()
        );
    }
    {
        let (total, items) = fetch_tx_http(&observer, 5)?;
        assert!(
            total >= 5,
            "observer HTTP must report at least 5 txs; got {total}"
        );
        assert!(!items.is_empty(), "expected items via HTTP for observer");
    }
    // POST /v1/accounts/{alice}/transactions/query with timestamp filter since t0
    let post_tx_query =
        |peer: &NetworkPeer, min_ts: u64, max_ts: Option<u64>| -> Result<(u64, Vec<JsonValue>)> {
            let url = format!(
                "{}/v1/accounts/{}/transactions/query",
                peer.torii_url(),
                *ALICE_ID,
            );
            let mut args = vec![
                {
                    let args = norito::json::array([
                        norito::json::to_value("authority").expect("serialize key"),
                        norito::json::to_value(alice_id_str.as_str())
                            .expect("serialize authority value"),
                    ])
                    .expect("serialize args array");
                    norito::json::object([
                        ("op", norito::json::to_value("eq").expect("serialize op")),
                        ("args", args),
                    ])
                    .expect("serialize equality filter")
                },
                norito::json!({"op":"gte","args":["timestamp_ms", min_ts]}),
            ];
            if let Some(max_ts) = max_ts {
                args.push(norito::json!({"op":"lte","args":["timestamp_ms", max_ts]}));
            }
            let env = norito::json!({
                "filter": {"op": "and", "args": args},
                "limit": 100
            });
            let body = norito::json::to_json(&env)?;
            let client = integration_tests::http::client();
            let resp_txt = rt.block_on(async {
                let response = client
                    .post(url)
                    .header("Content-Type", "application/json")
                    .header("Accept", "application/json")
                    .body(body)
                    .send()
                    .await?;
                let status = response.status();
                checked_history_body(status, response.text().await?)
            })?;
            let parsed: JsonValue = norito::json::from_str(&resp_txt)?;
            let (items, total) = match parsed {
                JsonValue::Object(m) => {
                    let total = m
                        .get("items")
                        .and_then(norito::json::Value::as_array)
                        .map_or(0, |items| items.len() as u64);
                    let items = m
                        .get("items")
                        .and_then(|v| match v {
                            JsonValue::Array(a) => Some(a.clone()),
                            _ => None,
                        })
                        .unwrap_or_default();
                    (items, total)
                }
                _ => (Vec::new(), 0),
            };
            Ok((total, items))
        };
    // For a narrower window, use [t1_lo, t_hi], expecting exactly 5 Alice txs
    // Removed unused underscore-prefixed binding _extract_ts
    // Helpers to extract timestamp and hash
    let extract_ts = |it: &JsonValue| -> u64 {
        it.as_object()
            .and_then(|m| m.get("timestamp_ms").and_then(norito::json::Value::as_u64))
            .unwrap_or(0)
    };
    let extract_hash = |it: &JsonValue| -> String {
        it.as_object()
            .and_then(|m| m.get("entrypoint_hash").and_then(|v| v.as_str()))
            .unwrap_or("")
            .to_string()
    };
    for p in network.peers() {
        let (total, mut items) = post_tx_query(p, t1_lo, Some(t_hi))?;
        assert_eq!(
            total,
            5,
            "expected exactly 5 filtered txs on {}",
            p.torii_url()
        );
        assert_eq!(items.len(), 5, "expected 5 items on {}", p.torii_url());
        // All items must be authored by Alice
        for it in &items {
            let auth = match it {
                JsonValue::Object(m) => m
                    .get("authority")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                _ => String::new(),
            };
            assert_eq!(auth, alice_id_str);
        }
        // Sort by timestamp and check the most recent two correspond to last two updates
        items.sort_by_key(&extract_ts);
        let last = items.last().unwrap();
        let prev = &items[items.len() - 2];
        let last_ts = extract_ts(last);
        let prev_ts = extract_ts(prev);
        assert!(
            last_ts >= t5_lo,
            "last tx timestamp {last_ts} < t5_lo {t5_lo}"
        );
        assert!(
            prev_ts >= t4_lo,
            "prev tx timestamp {prev_ts} < t4_lo {t4_lo}"
        );
        // Cross-check entrypoint_hash uniqueness for final two operations
        let last_hash = extract_hash(last);
        let prev_hash = extract_hash(prev);
        assert_ne!(
            last_hash,
            prev_hash,
            "last two entrypoint_hash identical on {}",
            p.torii_url()
        );
    }
    {
        let (total, mut items) = post_tx_query(&observer, t1_lo, Some(t_hi))?;
        assert_eq!(total, 5, "observer expected exactly 5 filtered txs");
        assert_eq!(items.len(), 5, "observer expected 5 items");
        items.sort_by_key(&extract_ts);
        let last = items.last().unwrap();
        let prev = &items[items.len() - 2];
        let last_ts = extract_ts(last);
        let prev_ts = extract_ts(prev);
        assert!(last_ts >= t5_lo);
        assert!(prev_ts >= t4_lo);
        let last_hash = extract_hash(last);
        let prev_hash = extract_hash(prev);
        assert_ne!(
            last_hash, prev_hash,
            "observer last two entrypoint_hash identical"
        );
    }
    // Read the same peer through its normal SDK context and the public JSON
    // representation. JsonOnly enforces the response media type and schema;
    // missing counters cannot silently compare equal to zero.
    rt.block_on(async {
        let deadline = tokio::time::Instant::now() + sync_timeout;
        for peer in peers {
            let status = read_status_until(deadline, || peer.status()).await?;
            let mut builder = peer.client().client().to_builder().api_token(None);
            builder.headers.clear();
            builder.operator_key_pair = None;
            builder.wire_format_preference = iroha::client::WireFormatPreference::JsonOnly;
            let json_client = builder.build()?;
            let json_status =
                read_status_until(deadline, || async { Ok(json_client.status().get().await?) })
                    .await?;
            assert_eq!(status.blocks, 6, "peer {} final applied height", peer.id());
            assert_eq!(
                (json_status.blocks, json_status.blocks_non_empty),
                (status.blocks, status.blocks_non_empty),
                "public JSON status counters disagree for peer {}",
                peer.id()
            );
        }
        Ok(())
    })
}
