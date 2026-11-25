//! Actual generated-profile publication over bounded loopback HTTP; acknowledgements are not readiness.

use super::*;
use crate::localnet::{LocalnetServiceProfile, prepare_localnet_at};
use crate::managed::native_operation::test_support::wallet_http::{
    WalletHttpRequest, wallet_request,
};
use std::{
    io::{self, Write as _},
    net::{Ipv4Addr, TcpListener},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

struct ReceivedPost {
    peer: usize,
    request: WalletHttpRequest,
    received: Instant,
    responding: Instant,
}

struct Peers {
    stopped: Arc<AtomicBool>,
    requests: Arc<Mutex<Vec<ReceivedPost>>>,
    workers: Vec<JoinHandle<io::Result<()>>>,
}
impl Peers {
    fn start(prepared: &PreparedLocalnet, statuses: [u16; 4]) -> Self {
        Self::with_delays(prepared, statuses, [Duration::ZERO; 4])
    }

    fn with_delays(prepared: &PreparedLocalnet, statuses: [u16; 4], delays: [Duration; 4]) -> Self {
        let stopped = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let mut workers = Vec::new();
        for (index, peer) in prepared.peers.iter().enumerate() {
            let url: url::Url = peer.torii_url.parse().unwrap();
            assert_eq!(url.host_str(), Some("127.0.0.1"));
            let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, url.port().unwrap())).unwrap();
            listener.set_nonblocking(true).unwrap();
            let stop = stopped.clone();
            let seen = requests.clone();
            let status = statuses[index];
            let response = format!(
                "HTTP/1.1 {status} AdvertFixture\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            let delay = delays[index];
            workers.push(thread::spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((mut socket, _)) => {
                            let request = wallet_request(&mut socket)?;
                            let received = Instant::now();
                            thread::sleep(delay);
                            let responding = Instant::now();
                            {
                                let mut requests = seen.lock().unwrap();
                                if requests.len() >= 32 {
                                    return Err(io::Error::other("advert fixture request bound"));
                                }
                                requests.push(ReceivedPost {
                                    peer: index,
                                    request,
                                    received,
                                    responding,
                                });
                            }
                            if let Err(error) = socket.write_all(response.as_bytes()) {
                                // A bounded request can close its socket during the fixture delay.
                                if delay.is_zero()
                                    || !matches!(
                                        error.kind(),
                                        io::ErrorKind::BrokenPipe
                                            | io::ErrorKind::ConnectionReset
                                            | io::ErrorKind::ConnectionAborted
                                    )
                                {
                                    return Err(error);
                                }
                            }
                        }
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(5))
                        }
                        Err(error) => return Err(error),
                    }
                }
                Ok(())
            }));
        }
        Self {
            stopped,
            requests,
            workers,
        }
    }
    fn finish(&mut self) {
        self.stopped.store(true, Ordering::SeqCst);
        for worker in self.workers.drain(..) {
            worker.join().unwrap().unwrap();
        }
    }
}
impl Drop for Peers {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::SeqCst);
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}
fn fixture() -> (tempfile::TempDir, PreparedLocalnet) {
    let temporary = tempfile::tempdir().unwrap();
    let ports = crate::managed::LocalnetPorts::reserve().unwrap();
    let prepared = prepare_localnet_at(
        "advert",
        &temporary.path().join("generation"),
        &ports,
        LocalnetServiceProfile::StreamTokenAuthorities,
        None,
    )
    .unwrap();
    (temporary, prepared)
}

#[test]
fn each_original_provider_publishes_its_own_exact_advert_under_independent_custody() {
    let _resources = crate::managed::native_test_guard();
    let (_temporary, prepared) = fixture();
    let mut peers = Peers::start(&prepared, [200; 4]);
    let providers = prepared
        .stream_token_authorities()
        .unwrap()
        .unwrap()
        .providers;
    let owners: Vec<_> = providers
        .iter()
        .map(|provider| {
            ManagedProviderAdvertisement::open(&prepared, provider.provider_id).unwrap()
        })
        .collect();
    let mut originals = Vec::new();
    for (provider, owner) in providers.iter().zip(&owners) {
        let advert = prepared
            .provider_advert(provider.provider_id, now_ms().unwrap() / 1_000)
            .unwrap();
        originals.push(norito::encode_canonical(&advert).unwrap());
        let report = owner
            .publish_original(&advert, Instant::now() + Duration::from_secs(30))
            .unwrap();
        assert_eq!(report.acknowledged_peers, 4);
        assert_eq!(report.issued_at, advert.issued_at);
        assert_eq!(report.expires_at, advert.expires_at);
        assert!(ManagedProviderAdvertisement::open(&prepared, provider.provider_id).is_err());
    }
    peers.finish();
    assert!(
        originals
            .iter()
            .enumerate()
            .all(|(index, bytes)| originals.iter().skip(index + 1).all(|other| bytes != other))
    );
    let requests = peers.requests.lock().unwrap();
    assert_eq!(requests.len(), 12);
    for bytes in originals {
        for peer in 0..4 {
            assert_eq!(
                requests
                    .iter()
                    .filter(|post| post.peer == peer && post.request.body == bytes)
                    .count(),
                1
            );
        }
    }
}

#[test]
fn publisher_partial_peers_reopen_repeats_exact_original_without_readiness_claim() {
    let _resources = crate::managed::native_test_guard();
    let (_temporary, prepared) = fixture();
    let mut peers = Peers::start(&prepared, [503, 200, 503, 200]);
    let original = prepared.stream_token_authorities().unwrap().unwrap();
    let advert = prepared
        .provider_advert(
            crate::managed::native_operation::test_support::provider_id(&prepared, 0),
            now_ms().unwrap() / 1000,
        )
        .unwrap();
    let exact = norito::encode_canonical(&advert).unwrap();
    let publisher = ManagedProviderAdvertisement::open(
        &prepared,
        crate::managed::native_operation::test_support::provider_id(&prepared, 0),
    )
    .unwrap();
    assert!(
        ManagedProviderAdvertisement::open(
            &prepared,
            crate::managed::native_operation::test_support::provider_id(&prepared, 0)
        )
        .is_err()
    );
    assert!(publisher.publish(Instant::now()).is_err());
    assert!(peers.requests.lock().unwrap().is_empty());
    let report = publisher
        .publish_original(&advert, Instant::now() + Duration::from_secs(30))
        .unwrap();
    assert_eq!(report.acknowledged_peers, 2);
    assert_eq!(report.issued_at, advert.issued_at);
    assert_eq!(report.expires_at, advert.expires_at);
    drop(publisher);
    let reopened = ManagedProviderAdvertisement::open(
        &prepared,
        crate::managed::native_operation::test_support::provider_id(&prepared, 0),
    )
    .unwrap();
    // Same original slot is deterministic; the profile test separately covers later slots.
    assert_eq!(
        norito::encode_canonical(
            &prepared
                .provider_advert(
                    crate::managed::native_operation::test_support::provider_id(&prepared, 0),
                    advert.issued_at
                )
                .unwrap()
        )
        .unwrap(),
        exact
    );
    assert_eq!(
        reopened
            .publish_original(&advert, Instant::now() + Duration::from_secs(30))
            .unwrap(),
        report
    );
    peers.finish();
    let requests = peers.requests.lock().unwrap();
    assert_eq!(requests.len(), 8);
    for index in 0..4 {
        assert_eq!(requests.iter().filter(|post| post.peer == index).count(), 2);
    }
    for post in requests.iter() {
        let request = &post.request;
        assert_eq!(request.method, "POST");
        assert_eq!(
            request.target.path(),
            iroha_torii_shared::route_catalog::sorafs::PROVIDER_ADVERT.path()
        );
        assert_eq!(request.body, exact);
    }
    assert_eq!(
        prepared.stream_token_authorities().unwrap().unwrap(),
        original
    );
    assert_eq!(
        reopened.authority.directory.entries(8).unwrap(),
        ["operation.lock"]
    );
}

#[test]
fn publisher_total_failure_attempts_each_peer_once_and_replaced_profile_is_no_http() {
    use iroha_fs::{PrivateDirectory, PublishMode};
    let _resources = crate::managed::native_test_guard();
    let (_temporary, prepared) = fixture();
    let mut peers = Peers::start(&prepared, [503; 4]);
    let publisher = ManagedProviderAdvertisement::open(
        &prepared,
        crate::managed::native_operation::test_support::provider_id(&prepared, 0),
    )
    .unwrap();
    assert!(
        publisher
            .publish(Instant::now() + Duration::from_secs(30))
            .is_err()
    );
    assert_eq!(peers.requests.lock().unwrap().len(), 4);
    let generation =
        PrivateDirectory::open_exact(prepared.context.client_config.parent().unwrap()).unwrap();
    let original = generation.read("peer0.toml", 1024 * 1024).unwrap();
    generation
        .write_atomic("peer0.toml", b"invalid peer", PublishMode::Replace)
        .unwrap();
    assert!(
        publisher
            .publish(Instant::now() + Duration::from_secs(30))
            .is_err()
    );
    assert_eq!(peers.requests.lock().unwrap().len(), 4);
    generation
        .write_atomic("peer0.toml", &original, PublishMode::Replace)
        .unwrap();
    publisher.authority.validate_profile().unwrap();
    peers.finish();
}

#[test]
fn publisher_deadline_divides_remaining_peers_without_extending_original_budget() {
    let now = Instant::now();
    let deadline = now + Duration::from_secs(4);
    assert_eq!(
        peer_deadline(now, deadline, 4),
        Some(now + Duration::from_secs(1))
    );
    assert_eq!(
        peer_deadline(now + Duration::from_secs(1), deadline, 3),
        Some(now + Duration::from_secs(2))
    );
    assert_eq!(
        peer_deadline(now, now + Duration::from_secs(120), 4),
        Some(now + Duration::from_secs(5))
    );
    assert_eq!(peer_deadline(now, now, 4), None);
    assert_eq!(peer_deadline(now, deadline, 0), None);
    assert_eq!(peer_deadline(now, now + Duration::from_nanos(1), 4), None);
}

struct TimedPublication {
    report: Result<ManagedProviderAdvertisementReport>,
    started: Instant,
    finished: Instant,
    posts: [ReceivedPost; 4],
}

fn observe_publication(
    statuses: [u16; 4],
    delays: [Duration; 4],
    budget: Duration,
) -> TimedPublication {
    let (_temporary, prepared) = fixture();
    let provider = crate::managed::native_operation::test_support::provider_id(&prepared, 0);
    let publisher = ManagedProviderAdvertisement::open(&prepared, provider).unwrap();
    let advert = prepared
        .provider_advert(provider, now_ms().unwrap() / 1_000)
        .unwrap();
    let exact = norito::encode_canonical(&advert).unwrap();
    let mut peers = Peers::with_delays(&prepared, statuses, delays);

    let started = Instant::now();
    let report = publisher.publish_original(&advert, started + budget);
    let finished = Instant::now();
    peers.finish();
    let mut posts = std::mem::take(&mut *peers.requests.lock().unwrap());
    posts.sort_by_key(|post| post.peer);
    assert_eq!(posts.len(), 4);
    for (index, post) in posts.iter().enumerate() {
        assert_eq!(post.peer, index);
        assert_eq!(post.request.method, "POST");
        assert_eq!(
            post.request.target.path(),
            iroha_torii_shared::route_catalog::sorafs::PROVIDER_ADVERT.path()
        );
        assert!(
            post.request.body == exact,
            "peer {index} received a different advert"
        );
        assert!(
            post.received <= finished,
            "peer {index} received its POST after the caller returned"
        );
    }
    if let Ok(report) = &report {
        assert_eq!(report.issued_at, advert.issued_at);
        assert_eq!(report.expires_at, advert.expires_at);
    }
    TimedPublication {
        report,
        started,
        finished,
        posts: posts.try_into().ok().unwrap(),
    }
}

#[test]
fn publisher_dispatches_to_healthy_peers_while_the_first_response_is_delayed() {
    let _resources = crate::managed::native_test_guard();
    let case = observe_publication(
        [503, 200, 200, 200],
        [1_200, 150, 150, 150].map(Duration::from_millis),
        Duration::from_secs(30),
    );
    let first_post = case.posts.iter().map(|post| post.received).min().unwrap();
    let last_healthy_post = case.posts[1..]
        .iter()
        .map(|post| post.received)
        .max()
        .unwrap();
    let slow_response = case.posts[0].responding;
    let elapsed = case.finished.duration_since(case.started);
    eprintln!(
        "advert_dispatch elapsed_us={} first_post_to_last_healthy_us={} start_to_last_healthy_us={} start_to_slow_reply_us={}",
        elapsed.as_micros(),
        last_healthy_post.duration_since(first_post).as_micros(),
        last_healthy_post.duration_since(case.started).as_micros(),
        slow_response.duration_since(case.started).as_micros(),
    );
    assert_eq!(case.report.unwrap().acknowledged_peers, 3);
    assert!(
        case.posts
            .iter()
            .all(|post| post.responding <= case.finished),
        "publisher returned with responses still outstanding"
    );
    assert!(
        last_healthy_post < slow_response,
        "healthy peers waited for the first peer's delayed 503"
    );
}

#[test]
fn publisher_bounds_a_stalled_response_by_the_original_deadline() {
    let _resources = crate::managed::native_test_guard();
    let budget = Duration::from_secs(4);
    let case = observe_publication(
        [200; 4],
        [7_000, 0, 0, 0].map(Duration::from_millis),
        budget,
    );
    assert!(
        case.posts[1..]
            .iter()
            .all(|post| post.responding < case.finished)
    );
    assert!(
        case.finished < case.posts[0].responding,
        "the late HTTP 200 held the caller beyond its deadline"
    );
    match case.report {
        Ok(report) => {
            assert!(
                case.finished <= case.started + budget,
                "publisher returned success after its deadline"
            );
            assert_eq!(report.acknowledged_peers, 3);
        }
        Err(error) => assert!(
            matches!(error, crate::managed::Error::NativeDeadline),
            "{error}"
        ),
    }
}

#[test]
fn renderer_advert_rechecks_original_material_and_keeps_the_exclusive_publisher() {
    use crate::managed::service_bootstrap::ManagedServiceBootstrap;
    use iroha_fs::{PrivateDirectory, PublishMode};

    let _resources = crate::managed::native_test_guard();
    let (_temporary, prepared) = fixture();
    let provider = crate::managed::native_operation::test_support::provider_id(&prepared, 0);
    let renderer = ManagedServiceBootstrap::open(&prepared)
        .unwrap()
        .into_generated_runtime()
        .unwrap();
    let publisher = renderer.open_provider_advert(&prepared, provider).unwrap();
    assert!(ManagedProviderAdvertisement::open(&prepared, provider).is_err());
    let expected = prepared
        .provider_advert(provider, now_ms().unwrap() / 1_000)
        .unwrap();
    let exact = norito::encode_canonical(&expected).unwrap();
    let mut peers = Peers::start(&prepared, [200; 4]);
    let report = publisher
        .publish(Instant::now() + Duration::from_secs(30))
        .unwrap();
    assert_eq!(
        (
            report.issued_at,
            report.expires_at,
            report.acknowledged_peers
        ),
        (expected.issued_at, expected.expires_at, 4)
    );
    {
        let posts = peers.requests.lock().unwrap();
        assert_eq!(posts.len(), 4);
        assert!(posts.iter().all(|post| post.request.body == exact));
    }
    let generation =
        PrivateDirectory::open_exact(prepared.context.client_config.parent().unwrap()).unwrap();
    let original = generation.read("peer0.toml", 1024 * 1024).unwrap();
    generation
        .write_atomic("peer0.toml", b"changed configuration", PublishMode::Replace)
        .unwrap();
    assert!(
        publisher
            .publish(Instant::now() + Duration::from_secs(30))
            .is_err()
    );
    assert_eq!(peers.requests.lock().unwrap().len(), 4);
    generation
        .write_atomic("peer0.toml", &original, PublishMode::Replace)
        .unwrap();
    publisher.authority.validate_profile().unwrap();
    drop(publisher);
    drop(ManagedProviderAdvertisement::open(&prepared, provider).unwrap());
    peers.finish();
}
