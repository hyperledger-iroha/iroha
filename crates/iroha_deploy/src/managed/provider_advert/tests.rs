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

struct Peers {
    stopped: Arc<AtomicBool>,
    requests: Arc<Mutex<Vec<(usize, WalletHttpRequest)>>>,
    workers: Vec<JoinHandle<io::Result<()>>>,
}
impl Peers {
    fn start(prepared: &PreparedLocalnet, statuses: [u16; 4]) -> Self {
        let stopped = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let workers = prepared.peers.iter().enumerate().map(|(index, peer)| {
            let url: url::Url = peer.torii_url.parse().unwrap();
            assert_eq!(url.host_str(), Some("127.0.0.1"));
            let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, url.port().unwrap())).unwrap();
            listener.set_nonblocking(true).unwrap();
            let stop = stopped.clone(); let seen = requests.clone(); let status = statuses[index];
            thread::spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((mut socket, _)) => {
                            let request = wallet_request(&mut socket)?;
                            let mut requests = seen.lock().unwrap();
                            if requests.len() >= 32 { return Err(io::Error::other("advert fixture request bound")); }
                            requests.push((index, request)); drop(requests);
                            write!(socket, "HTTP/1.1 {status} AdvertFixture\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")?;
                        }
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => thread::sleep(Duration::from_millis(5)),
                        Err(error) => return Err(error),
                    }
                }
                Ok(())
            })
        }).collect();
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
                    .filter(|(selected, request)| *selected == peer && request.body == bytes)
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
        assert_eq!(
            requests.iter().filter(|(peer, _)| *peer == index).count(),
            2
        );
    }
    for (_, request) in requests.iter() {
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
