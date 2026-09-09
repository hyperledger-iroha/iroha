// Shared support for real signed archive and physical-worker boundary tests.
use axum::{
    body::to_bytes,
    extract::{ConnectInfo, OriginalUri, Path, RawQuery, State},
    http::{HeaderMap, HeaderValue, StatusCode, Uri, header},
    response::Response,
};
use std::sync::{Condvar, atomic::AtomicU8};
use tokio::sync::{Notify, Semaphore};

const WAIT: Duration = Duration::from_secs(10);
const PASS: u8 = 0;
const BLOCK: u8 = 1;
const PANIC: u8 = 2;
const UNAVAILABLE: u8 = 3;

struct WorkerGate {
    mode: AtomicU8,
    entered: AtomicUsize,
    finished: AtomicUsize,
    suppressed: AtomicBool,
    executor_thread: std::thread::ThreadId,
    physical_thread: AtomicBool,
    released: Mutex<bool>,
    condition: Condvar,
    progress: Notify,
}

impl WorkerGate {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            mode: AtomicU8::new(PASS),
            entered: AtomicUsize::new(0),
            finished: AtomicUsize::new(0),
            suppressed: AtomicBool::new(false),
            executor_thread: std::thread::current().id(),
            physical_thread: AtomicBool::new(false),
            released: Mutex::new(false),
            condition: Condvar::new(),
            progress: Notify::new(),
        })
    }

    fn release(&self) {
        *self
            .released
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
        self.condition.notify_all();
    }

    async fn wait_count(&self, counter: &AtomicUsize, minimum: usize) {
        tokio::time::timeout(WAIT, async {
            loop {
                let notified = self.progress.notified();
                if counter.load(Ordering::Acquire) >= minimum {
                    return;
                }
                notified.await;
            }
        })
        .await
        .expect("bounded physical worker progress");
    }
}

// Releases even when a caller assertion unwinds; the worker independently has
// a finite wait, so a forgotten/aborted caller cannot strand the native suite.
struct ReleaseOnDrop(Arc<WorkerGate>);
impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        self.0.release();
    }
}

struct PhysicalExit<'a>(&'a WorkerGate);
impl Drop for PhysicalExit<'_> {
    fn drop(&mut self) {
        self.0.finished.fetch_add(1, Ordering::AcqRel);
        self.0.progress.notify_one();
    }
}

impl SccpReplayLocalAuthorityV1 for WorkerGate {
    fn verify_candidate(
        &self,
        finality: SccpReplayArchiveFinalityV1,
        expected: &BTreeMap<SccpReplayAccumulatorIdV1, (SccpReplayDomainV1, SccpReplayForestV1)>,
    ) -> Result<(), SccpReplayLocalAuthorityErrorV1> {
        EmptyForestLocalAuthority.verify_candidate(finality, expected)
    }

    fn rebuild_and_verify(
        &self,
        finality: SccpReplayArchiveFinalityV1,
        expected: &BTreeMap<SccpReplayAccumulatorIdV1, (SccpReplayDomainV1, SccpReplayForestV1)>,
    ) -> Result<SccpReplayArchiveV1, SccpReplayLocalAuthorityErrorV1> {
        EmptyForestLocalAuthority.rebuild_and_verify(finality, expected)
    }

    fn verify_current(
        &self,
        finality: SccpReplayArchiveFinalityV1,
        expected: &BTreeMap<SccpReplayAccumulatorIdV1, (SccpReplayDomainV1, SccpReplayForestV1)>,
    ) -> Result<(), SccpReplayLocalAuthorityErrorV1> {
        let _exit = PhysicalExit(self);
        self.suppressed
            .store(iroha_core::panic_hook::is_suppressed(), Ordering::Release);
        self.physical_thread.store(
            std::thread::current().id() != self.executor_thread,
            Ordering::Release,
        );
        self.entered.fetch_add(1, Ordering::AcqRel);
        self.progress.notify_one();
        match self.mode.load(Ordering::Acquire) {
            BLOCK => {
                let (guard, wait) = self
                    .condition
                    .wait_timeout_while(
                        self.released.lock().expect("gate is healthy"),
                        WAIT,
                        |released| !*released,
                    )
                    .expect("gate wait is healthy");
                drop(guard);
                assert!(!wait.timed_out(), "bounded SCCP physical gate timed out");
            }
            PANIC => panic!("injected SCCP physical authority panic"),
            UNAVAILABLE => return Err(SccpReplayLocalAuthorityErrorV1::Finality),
            PASS => {}
            _ => unreachable!("test mode is closed"),
        }
        EmptyForestLocalAuthority.verify_current(finality, expected)
    }
}

fn worker_fixture() -> (
    crate::test_utils::TestDataDirGuard,
    Fixture,
    Arc<ToriiSccpReplayArchiveServiceV1>,
    Arc<WorkerGate>,
    crate::SharedAppState,
) {
    let data_dir = crate::test_utils::TestDataDirGuard::new();
    let fixture = Fixture::new();
    let gate = WorkerGate::new();
    let service = ToriiSccpReplayArchiveServiceV1::bootstrap_with_components(
        fixture.config.clone(),
        fixture.source.clone(),
        gate.clone(),
    )
    .expect("actual independent three-replica signed archive bootstraps");
    let mut app = crate::tests_runtime_handlers::mk_app_state_for_tests();
    let state = Arc::get_mut(&mut app).expect("new app is uniquely owned");
    state.sccp_replay_archive = Some(service.clone());
    state.query_inflight = Arc::new(Semaphore::new(1));
    state.query_heavy_inflight = Arc::new(Semaphore::new(1));
    state.query_queue_timeout = Duration::ZERO;
    (data_dir, fixture, service, gate, app)
}

async fn request(
    app: crate::SharedAppState,
    witness: bool,
    wrong_path: bool,
) -> Result<Response, crate::Error> {
    request_with_token(app, witness, wrong_path, None).await
}

async fn request_with_token(
    app: crate::SharedAppState,
    witness: bool,
    wrong_path: bool,
    token: Option<&'static str>,
) -> Result<Response, crate::Error> {
    let id = accumulator_id();
    let [boundary, source, route, asset, revision] =
        encode_sccp_replay_accumulator_path_v1(&SccpReplayAccumulatorPathV1 {
            route_key: id.route_key,
            boundary: id.boundary,
        })
        .expect("canonical signed fixture path");
    let mut path = format!("/v1/sccp/replay/{boundary}/{source}/{route}/{asset}/{revision}");
    if witness {
        path.push_str(&format!("/witness/{}", "00".repeat(32)));
    } else {
        path.push_str("/root");
    }
    if wrong_path {
        path.push('/');
    }
    let uri: Uri = path.parse().expect("fixture URI");
    let mut headers = HeaderMap::new();
    headers.insert(
        header::ACCEPT,
        HeaderValue::from_static(crate::utils::NORITO_MIME_TYPE),
    );
    if let Some(token) = token {
        headers.insert("x-api-token", HeaderValue::from_static(token));
    }
    let remote = "127.0.0.1:41001".parse().expect("fixture socket");
    if witness {
        crate::handler_sccp_replay_witness(
            State(app),
            Path((boundary, source, route, asset, revision, "00".repeat(32))),
            OriginalUri(uri),
            RawQuery(None),
            headers,
            ConnectInfo(remote),
        )
        .await
    } else {
        crate::handler_sccp_replay_root(
            State(app),
            Path((boundary, source, route, asset, revision)),
            OriginalUri(uri),
            RawQuery(None),
            headers,
            ConnectInfo(remote),
        )
        .await
    }
}

fn assert_capacity(app: &crate::SharedAppState, permits: usize) {
    assert_eq!(app.query_inflight.available_permits(), permits);
    assert_eq!(app.query_heavy_inflight.available_permits(), permits);
}

async fn bytes(response: Response) -> Vec<u8> {
    to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("bounded response bytes")
        .to_vec()
}

fn refresh_torii(app: &crate::SharedAppState) -> crate::Torii {
    let config = crate::test_utils::mk_minimal_root_cfg();
    let mut torii = crate::Torii::new_with_handle(
        app.chain_id.as_ref().clone(),
        app.state.network_id,
        app.kiso.clone(),
        config.torii,
        app.queue.clone(),
        app.events.clone(),
        app.query_service.clone(),
        app.kura.clone(),
        app.state.clone(),
        app.da_receipt_signer.clone(),
        app.online_peers.clone(),
        None,
        crate::routing::MaybeTelemetry::disabled(),
    )
    .expect("ordinary Torii constructor accepts the disabled-service test configuration");
    torii.sccp_replay_archive = app.sccp_replay_archive.clone();
    torii
}

fn trigger_refresh(app: &crate::SharedAppState) {
    use iroha_data_model::events::pipeline::BlockEvent;
    let event = BlockEvent {
        header: iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::new(1).expect("nonzero height"),
            None,
            None,
            None,
            1,
            0,
        ),
        status: crate::BlockStatus::Committed,
    };
    app.events
        .send(crate::EventBox::Pipeline(crate::PipelineEventBox::Block(
            event,
        )))
        .expect("actual refresh worker subscribed");
}

async fn wait_service_owners(service: &Arc<ToriiSccpReplayArchiveServiceV1>, expected: usize) {
    tokio::time::timeout(WAIT, async {
        while Arc::strong_count(service) != expected {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("all physical refresh owners exit before private fixture teardown");
}
