//! Explicit process admission ownership for in-process broker scenarios.

use super::*;
use std::ops::{Deref, DerefMut};

pub(super) fn new_test_process_pool() -> Arc<DecodeResourcePoolV1> {
    Arc::new(DecodeResourcePoolV1::new(MAX_BROKER_SHARED_DECODE_BYTES_V1))
}

/// One endpoint scenario models a client process and a separate broker process.
/// Cloning the fixture preserves both process identities across reconnections.
#[derive(Clone)]
pub(super) struct BrokerTestEndpoint {
    endpoint: EndpointPolicy,
    client_pool: Arc<DecodeResourcePoolV1>,
    server_pool: Arc<DecodeResourcePoolV1>,
}
impl BrokerTestEndpoint {
    pub(super) fn for_test(path: PathBuf) -> Self {
        Self {
            endpoint: EndpointPolicy::for_test(path),
            client_pool: new_test_process_pool(),
            server_pool: new_test_process_pool(),
        }
    }
    pub(super) fn fake_listener(&self, listener: UnixListener) -> BrokerTestListener {
        BrokerTestListener {
            listener,
            decode_pool: Arc::clone(&self.server_pool),
        }
    }
}
impl Deref for BrokerTestEndpoint {
    type Target = EndpointPolicy;
    fn deref(&self) -> &Self::Target {
        &self.endpoint
    }
}
impl DerefMut for BrokerTestEndpoint {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.endpoint
    }
}

pub(super) struct BrokerTestListener {
    listener: UnixListener,
    decode_pool: Arc<DecodeResourcePoolV1>,
}
impl BrokerTestListener {
    pub(super) fn accept(
        &self,
    ) -> std::io::Result<(BrokerTestStream, std::os::unix::net::SocketAddr)> {
        let (stream, address) = self.listener.accept()?;
        Ok((
            BrokerTestStream {
                stream,
                decode_pool: Arc::clone(&self.decode_pool),
            },
            address,
        ))
    }
}
impl Deref for BrokerTestListener {
    type Target = UnixListener;
    fn deref(&self) -> &Self::Target {
        &self.listener
    }
}

pub(super) struct BrokerTestStream {
    stream: UnixStream,
    pub(super) decode_pool: Arc<DecodeResourcePoolV1>,
}
impl Deref for BrokerTestStream {
    type Target = UnixStream;
    fn deref(&self) -> &Self::Target {
        &self.stream
    }
}
impl DerefMut for BrokerTestStream {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.stream
    }
}
impl std::io::Read for BrokerTestStream {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        std::io::Read::read(&mut self.stream, bytes)
    }
}
impl std::io::Write for BrokerTestStream {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        std::io::Write::write(&mut self.stream, bytes)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        std::io::Write::flush(&mut self.stream)
    }
}

pub(super) fn resolve_test_process(
    bindings: &IrohaRuntimeProviderBindingsV1,
    endpoint: &BrokerTestEndpoint,
) -> Result<IrohaRuntimeDeps, IrohaRuntimeProviderRegistryErrorV1> {
    resolve_with_decode_pool(bindings, endpoint, Arc::clone(&endpoint.client_pool))
}
pub(super) fn connect_test_process(
    endpoint: &BrokerTestEndpoint,
    chain_id: &str,
    network_id: NetworkId,
    catalog: Vec<ProviderBindingWireV1>,
) -> Result<(Arc<BrokerSession>, Vec<ProviderObservationWireV1>), BrokerError> {
    BrokerSession::connect(
        endpoint,
        chain_id,
        network_id,
        catalog,
        Arc::clone(&endpoint.client_pool),
    )
}
pub(super) fn prepare_test_server_state(
    bindings: &IrohaRuntimeProviderBindingsV1,
    backends: RuntimeProviderBrokerBackendsV1,
) -> Result<BrokerServerStateV1, RuntimeProviderBrokerServerErrorV1> {
    // A standalone state fixture represents one broker process. Clones of the
    // resulting state share this owner, as do all workers using that state.
    prepare_server_state(bindings, backends, new_test_process_pool())
}
pub(super) fn serve_test_process(
    bindings: &IrohaRuntimeProviderBindingsV1,
    backends: RuntimeProviderBrokerBackendsV1,
    endpoint: &BrokerTestEndpoint,
    lifecycle: Arc<RuntimeProviderBrokerLifecycleV1>,
) -> Result<(), RuntimeProviderBrokerServerErrorV1> {
    serve_test_process_with_lifecycle(bindings, backends, endpoint, lifecycle, || {})
}
pub(super) fn serve_test_process_with_lifecycle<R>(
    bindings: &IrohaRuntimeProviderBindingsV1,
    backends: RuntimeProviderBrokerBackendsV1,
    endpoint: &BrokerTestEndpoint,
    lifecycle: Arc<RuntimeProviderBrokerLifecycleV1>,
    on_ready: R,
) -> Result<(), RuntimeProviderBrokerServerErrorV1>
where
    R: FnOnce(),
{
    serve_test_process_with_fallible_readiness(bindings, backends, endpoint, lifecycle, || {
        on_ready();
        Ok(())
    })
}
pub(super) fn serve_test_process_with_fallible_readiness<R>(
    bindings: &IrohaRuntimeProviderBindingsV1,
    backends: RuntimeProviderBrokerBackendsV1,
    endpoint: &BrokerTestEndpoint,
    lifecycle: Arc<RuntimeProviderBrokerLifecycleV1>,
    on_ready: R,
) -> Result<(), RuntimeProviderBrokerServerErrorV1>
where
    R: FnOnce() -> Result<(), RuntimeProviderBrokerReadinessErrorV1>,
{
    serve_test_process_with_peer_authorizer(
        bindings,
        backends,
        endpoint,
        lifecycle,
        on_ready,
        verify_peer_uid,
    )
}
pub(super) fn serve_test_process_with_peer_authorizer<R, A>(
    bindings: &IrohaRuntimeProviderBindingsV1,
    backends: RuntimeProviderBrokerBackendsV1,
    endpoint: &BrokerTestEndpoint,
    lifecycle: Arc<RuntimeProviderBrokerLifecycleV1>,
    on_ready: R,
    authorize_peer_uid: A,
) -> Result<(), RuntimeProviderBrokerServerErrorV1>
where
    R: FnOnce() -> Result<(), RuntimeProviderBrokerReadinessErrorV1>,
    A: Fn(u32, u32) -> Result<(), BrokerError>,
{
    serve_with_policy_and_fallible_readiness_and_peer_authorizer(
        bindings,
        backends,
        endpoint,
        lifecycle,
        on_ready,
        authorize_peer_uid,
        Arc::clone(&endpoint.server_pool),
    )
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simulated_processes_have_separate_caps_and_clones_share_ownership() {
        let fixture = BrokerTestEndpoint::for_test(PathBuf::from("unused-process-pool.sock"));
        assert_eq!(
            fixture.client_pool.max_bytes,
            MAX_BROKER_SHARED_DECODE_BYTES_V1
        );
        assert_eq!(
            fixture.server_pool.max_bytes,
            MAX_BROKER_SHARED_DECODE_BYTES_V1
        );
        assert!(!Arc::ptr_eq(&fixture.client_pool, &fixture.server_pool));
        let cloned = fixture.clone();
        assert!(Arc::ptr_eq(&fixture.client_pool, &cloned.client_pool));
        assert!(Arc::ptr_eq(&fixture.server_pool, &cloned.server_pool));
        let operation = OPERATION_SEALED_COMPARE_AND_SWAP_V1;
        let required = operation_decode_policy(operation).max_composed_bytes;
        assert!(required > MAX_BROKER_SHARED_DECODE_BYTES_V1 / 2);
        let held = std::thread::scope(|scope| {
            let jobs = [&fixture.client_pool, &fixture.server_pool].map(|pool| {
                let pool = Arc::clone(pool);
                scope.spawn(move || {
                    DecodeResourceAdmissionV1::acquire_operation_from(pool, operation)
                })
            });
            jobs.map(|job| {
                job.join()
                    .expect("join independent process owner")
                    .expect("one valid request fits its process cap")
            })
        });
        assert_eq!(
            fixture.client_pool.used_bytes.load(Ordering::Acquire),
            required
        );
        assert_eq!(
            fixture.server_pool.used_bytes.load(Ordering::Acquire),
            required
        );
        drop(held);
        assert_eq!(fixture.client_pool.used_bytes.load(Ordering::Acquire), 0);
        assert_eq!(fixture.server_pool.used_bytes.load(Ordering::Acquire), 0);
        let held = DecodeResourceAdmissionV1::acquire_operation_from(
            Arc::clone(&fixture.client_pool),
            operation,
        )
        .expect("first same-process request fits");
        assert!(
            matches!(
                DecodeResourceAdmissionV1::acquire_operation_from(
                    Arc::clone(&cloned.client_pool),
                    operation
                ),
                Err(BrokerError::Unavailable)
            ),
            "a clone cannot escape its process's unchanged cap"
        );
        let response = ScrubbedBytes::with_decode_admission(vec![7], held);
        assert_eq!(
            fixture.client_pool.used_bytes.load(Ordering::Acquire),
            required
        );
        drop(response);
        assert_eq!(fixture.client_pool.used_bytes.load(Ordering::Acquire), 0);
    }

    #[test]
    fn production_default_pool_is_one_process_singleton() {
        let first = shared_decode_resource_pool();
        let second = shared_decode_resource_pool();
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(first.max_bytes, MAX_BROKER_SHARED_DECODE_BYTES_V1);
        let worker = std::thread::spawn(shared_decode_resource_pool)
            .join()
            .expect("join another production thread");
        assert!(Arc::ptr_eq(&first, &worker));
        assert!(!Arc::ptr_eq(&first, &new_test_process_pool()));
    }

    #[test]
    fn reconnect_preserves_the_session_process_pool() {
        let endpoint = BrokerTestEndpoint::for_test(PathBuf::from("unused-pool-reconnect.sock"));
        let (stream, _peer) = UnixStream::pair().expect("create initial connection");
        let session = BrokerSession {
            decode_pool: Arc::clone(&endpoint.client_pool),
            connection: Mutex::new(BrokerConnection {
                stream,
                session_id: [1; 32],
                next_request_id: 1,
                poison_reason: Some(BrokerConnectionFailure::Unavailable),
            }),
            chain_id: "process-pool-test".to_owned(),
            network_id: NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
                iroha_data_model::block::BlockHeader,
            >::from_untyped_unchecked(
                iroha_crypto::Hash::prehashed([1; 32])
            )),
            endpoint: endpoint.endpoint.clone(),
            requested_catalog: Vec::new(),
        };
        let (replacement, _replacement_peer) =
            UnixStream::pair().expect("create replacement connection");
        session
            .reconnect_using(None, || {
                Ok(BrokerConnection {
                    stream: replacement,
                    session_id: [2; 32],
                    next_request_id: 1,
                    poison_reason: None,
                })
            })
            .expect("replace unavailable connection");
        assert!(Arc::ptr_eq(&session.decode_pool, &endpoint.client_pool));
        assert_eq!(session.connection.lock().unwrap().session_id, [2; 32]);
        session
            .reconnect_using(None, || {
                panic!("healthy reconnect must not authenticate again")
            })
            .expect("healthy session retains current connection");
        assert!(Arc::ptr_eq(&session.decode_pool, &endpoint.client_pool));
        assert_eq!(session.decode_pool.used_bytes.load(Ordering::Acquire), 0);
    }

    #[test]
    fn accepted_fake_connections_share_the_fixture_broker_process() {
        let directory = tempfile::Builder::new()
            .prefix(".pool-")
            .tempdir_in(".")
            .expect("create bounded fake broker socket path");
        let path = directory.path().join("broker.sock");
        let fixture = BrokerTestEndpoint::for_test(path.clone());
        let listener = fixture.fake_listener(UnixListener::bind(&path).expect("bind fake broker"));
        let _first_client = UnixStream::connect(&path).expect("connect first client");
        let (first, _) = listener.accept().expect("accept first connection");
        let _second_client = UnixStream::connect(&path).expect("connect second client");
        let (second, _) = listener.accept().expect("accept second connection");
        assert!(Arc::ptr_eq(&first.decode_pool, &second.decode_pool));
        assert!(Arc::ptr_eq(&first.decode_pool, &fixture.server_pool));
        assert!(!Arc::ptr_eq(&first.decode_pool, &fixture.client_pool));
        assert_eq!(
            first.decode_pool.max_bytes,
            MAX_BROKER_SHARED_DECODE_BYTES_V1
        );
    }

    #[test]
    fn handshake_decode_uses_its_process_or_the_enclosing_operation() {
        let pool = new_test_process_pool();
        let frame = encode_frame(
            FRAME_KIND_HANDSHAKE_RESPONSE_V1,
            &7_u8,
            MAX_HANDSHAKE_FRAME_BYTES_V1,
        )
        .expect("encode bounded control frame");
        let decode = |bytes: &[u8]| {
            decode_frame_with_policy_from::<u8>(
                bytes,
                FRAME_KIND_HANDSHAKE_RESPONSE_V1,
                MAX_HANDSHAKE_FRAME_BYTES_V1,
                CONTROL_DECODE_POLICY_V1,
                Arc::clone(&pool),
            )
        };
        assert_eq!(decode(&frame), Ok(7));
        assert_eq!(pool.used_bytes.load(Ordering::Acquire), 0);
        let full = pool
            .try_acquire(pool.max_bytes)
            .expect("exhaust selected process");
        assert_eq!(decode(&frame), Err(BrokerError::Unavailable));

        let enclosing_pool = new_test_process_pool();
        let enclosing = DecodeResourceAdmissionV1::acquire_operation_from(
            Arc::clone(&enclosing_pool),
            OPERATION_QUALIFY_V1,
        )
        .expect("reserve enclosing operation");
        let scope = enclosing.enter();
        assert_eq!(decode(&frame), Ok(7));
        assert_eq!(
            decode(&frame[..frame.len() - 1]),
            Err(BrokerError::Protocol)
        );
        assert!(Arc::ptr_eq(
            &current_decode_resource_admission()
                .expect("enclosing admission survives both decodes"),
            &enclosing,
        ));
        assert_eq!(
            enclosing_pool.used_bytes.load(Ordering::Acquire),
            operation_decode_policy(OPERATION_QUALIFY_V1).max_composed_bytes
        );
        assert_eq!(pool.used_bytes.load(Ordering::Acquire), pool.max_bytes);
        drop(scope);
        drop(enclosing);
        assert_eq!(enclosing_pool.used_bytes.load(Ordering::Acquire), 0);
        drop(full);
        assert_eq!(decode(&frame), Ok(7));
        assert_eq!(pool.used_bytes.load(Ordering::Acquire), 0);
    }
}
