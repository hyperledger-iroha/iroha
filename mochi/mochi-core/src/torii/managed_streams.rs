const INITIAL_BACKOFF: Duration = Duration::from_millis(500);
const MAX_BACKOFF: Duration = Duration::from_secs(8);
/// Reconnecting wrapper around [`BlockStream`] that automatically retries with backoff.
#[derive(Debug)]
pub struct ManagedBlockStream {
    sender: broadcast::Sender<BlockStreamEvent>,
    shutdown: watch::Sender<bool>,
    worker: JoinHandle<()>,
    alias: Arc<str>,
}
impl ManagedBlockStream {
    /// Spawn a reconnection loop for `/v1/blocks/stream` using the provided runtime handle.
    ///
    /// The `alias` is used for diagnostics so UI layers can attribute log messages
    /// and reconnection notices to the originating peer.
    pub fn spawn(handle: &Handle, alias: String, client: ToriiClient) -> Self {
        Self::spawn_with_factory(handle, alias, move || {
            let client = client.clone();
            async move { client.subscribe_block_stream().await }
        })
    }
    fn spawn_with_factory<F, Fut>(handle: &Handle, alias: impl Into<String>, factory: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ToriiResult<WsSubscription>> + Send + 'static,
    {
        let (shutdown, mut shutdown_rx) = watch::channel(false);
        let (sender, _) = broadcast::channel(128);
        let alias: Arc<str> = Arc::from(alias.into().into_boxed_str());
        let factory = Arc::new(factory);
        let run_factory = factory.clone();
        let run_sender = sender.clone();
        let run_alias = alias.clone();
        let worker = handle.spawn(async move {
            run_managed_block_stream(run_alias, run_factory, run_sender, &mut shutdown_rx).await;
        });
        Self {
            sender,
            shutdown,
            worker,
            alias,
        }
    }
    /// Acquire a receiver that yields decoded block events with reconnection semantics.
    pub fn subscribe(&self) -> broadcast::Receiver<BlockStreamEvent> {
        self.sender.subscribe()
    }
    /// Abort the reconnection loop and underlying subscription, if running.
    pub fn abort(&self) {
        let _ = self.shutdown.send(true);
        if !self.worker.is_finished() {
            self.worker.abort();
        }
    }
    /// Returns `true` when the reconnection loop has finished executing.
    pub fn is_finished(&self) -> bool {
        self.worker.is_finished()
    }
    /// Returns the alias associated with this managed stream.
    pub fn alias(&self) -> &str {
        self.alias.as_ref()
    }
}
impl Drop for ManagedBlockStream {
    fn drop(&mut self) {
        self.abort();
    }
}
async fn run_managed_block_stream<F, Fut>(
    alias: Arc<str>,
    factory: Arc<F>,
    sender: broadcast::Sender<BlockStreamEvent>,
    shutdown: &mut watch::Receiver<bool>,
) where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ToriiResult<WsSubscription>> + Send + 'static,
{
    let mut backoff = INITIAL_BACKOFF;
    let mut has_connected = false;
    loop {
        if shutdown_requested(shutdown) {
            break;
        }
        let subscription = match (factory.as_ref())().await {
            Ok(subscription) => subscription,
            Err(err) => {
                let _ = sender.send(BlockStreamEvent::DecodeError {
                    error: BlockStreamDecodeError::new(
                        BlockDecodeStage::Stream,
                        0,
                        err.to_string(),
                    ),
                });
                let _ = sender.send(BlockStreamEvent::Text {
                    text: format!(
                        "Block stream `{}` reconnecting after error: {err}",
                        alias.as_ref()
                    ),
                });
                if wait_for_shutdown_or_delay(shutdown, backoff).await {
                    break;
                }
                backoff = (backoff.saturating_mul(2)).min(MAX_BACKOFF);
                continue;
            }
        };
        if has_connected {
            let _ = sender.send(BlockStreamEvent::Text {
                text: format!("Block stream `{}` reconnected.", alias.as_ref()),
            });
        } else {
            has_connected = true;
        }
        backoff = INITIAL_BACKOFF;
        let block_stream = BlockStream::new(subscription);
        let mut receiver = block_stream.subscribe();
        let mut should_stop = false;
        loop {
            tokio::select! {
                changed = shutdown.changed() => {
                    match changed {
                        Ok(_) => {
                            if shutdown_requested(shutdown) {
                                should_stop = true;
                                break;
                            }
                        }
                        Err(_) => {
                            should_stop = true;
                            break;
                        }
                    }
                }
                item = receiver.recv() => {
                    match item {
                        Ok(event) => {
                            let _ = sender.send(event.clone());
                            if matches!(event, BlockStreamEvent::Closed) {
                                break;
                            }
                        }
                        Err(RecvError::Lagged(skipped)) => {
                            let _ = sender.send(BlockStreamEvent::Lagged {
                                skipped: lag_to_usize(skipped),
                            });
                        }
                        Err(RecvError::Closed) => {
                            break;
                        }
                    }
                }
            }
        }
        block_stream.abort();
        if should_stop || shutdown_requested(shutdown) {
            break;
        }
        if wait_for_shutdown_or_delay(shutdown, backoff).await {
            break;
        }
    }
}
/// Reconnecting wrapper around [`EventStream`] with exponential backoff.
#[derive(Debug)]
pub struct ManagedEventStream {
    sender: broadcast::Sender<EventStreamEvent>,
    shutdown: watch::Sender<bool>,
    worker: JoinHandle<()>,
    alias: Arc<str>,
}
impl ManagedEventStream {
    /// Spawn a reconnection loop for `/v1/events/ws` using the provided runtime handle.
    pub fn spawn(handle: &Handle, alias: String, client: ToriiClient) -> Self {
        Self::spawn_with_factory(handle, alias, move || {
            let client = client.clone();
            async move { client.subscribe_events_stream().await }
        })
    }
    fn spawn_with_factory<F, Fut>(handle: &Handle, alias: impl Into<String>, factory: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ToriiResult<WsSubscription>> + Send + 'static,
    {
        let (shutdown, mut shutdown_rx) = watch::channel(false);
        let (sender, _) = broadcast::channel(128);
        let alias: Arc<str> = Arc::from(alias.into().into_boxed_str());
        let factory = Arc::new(factory);
        let run_factory = factory.clone();
        let run_sender = sender.clone();
        let run_alias = alias.clone();
        let worker = handle.spawn(async move {
            run_managed_event_stream(run_alias, run_factory, run_sender, &mut shutdown_rx).await;
        });
        Self {
            sender,
            shutdown,
            worker,
            alias,
        }
    }
    /// Acquire a receiver that yields decoded events with reconnection semantics.
    pub fn subscribe(&self) -> broadcast::Receiver<EventStreamEvent> {
        self.sender.subscribe()
    }
    /// Abort the reconnection loop and underlying subscription, if running.
    pub fn abort(&self) {
        let _ = self.shutdown.send(true);
        if !self.worker.is_finished() {
            self.worker.abort();
        }
    }
    /// Returns `true` when the reconnection loop has finished executing.
    pub fn is_finished(&self) -> bool {
        self.worker.is_finished()
    }
    /// Returns the alias associated with this managed stream.
    pub fn alias(&self) -> &str {
        self.alias.as_ref()
    }
}
impl Drop for ManagedEventStream {
    fn drop(&mut self) {
        self.abort();
    }
}
async fn run_managed_event_stream<F, Fut>(
    alias: Arc<str>,
    factory: Arc<F>,
    sender: broadcast::Sender<EventStreamEvent>,
    shutdown: &mut watch::Receiver<bool>,
) where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ToriiResult<WsSubscription>> + Send + 'static,
{
    let mut backoff = INITIAL_BACKOFF;
    let mut has_connected = false;
    loop {
        if shutdown_requested(shutdown) {
            break;
        }
        let subscription = match (factory.as_ref())().await {
            Ok(subscription) => subscription,
            Err(err) => {
                let _ = sender.send(EventStreamEvent::DecodeError {
                    error: EventStreamDecodeError::new(
                        EventDecodeStage::Stream,
                        0,
                        err.to_string(),
                    ),
                });
                let _ = sender.send(EventStreamEvent::Text {
                    text: format!(
                        "Event stream `{}` reconnecting after error: {err}",
                        alias.as_ref()
                    ),
                });
                if wait_for_shutdown_or_delay(shutdown, backoff).await {
                    break;
                }
                backoff = (backoff.saturating_mul(2)).min(MAX_BACKOFF);
                continue;
            }
        };
        if has_connected {
            let _ = sender.send(EventStreamEvent::Text {
                text: format!("Event stream `{}` reconnected.", alias.as_ref()),
            });
        } else {
            has_connected = true;
        }
        backoff = INITIAL_BACKOFF;
        let event_stream = EventStream::new(subscription);
        let mut receiver = event_stream.subscribe();
        let mut should_stop = false;
        loop {
            tokio::select! {
                changed = shutdown.changed() => {
                    match changed {
                        Ok(_) => {
                            if shutdown_requested(shutdown) {
                                should_stop = true;
                                break;
                            }
                        }
                        Err(_) => {
                            should_stop = true;
                            break;
                        }
                    }
                }
                item = receiver.recv() => {
                    match item {
                        Ok(event) => {
                            let _ = sender.send(event.clone());
                            if matches!(event, EventStreamEvent::Closed) {
                                break;
                            }
                        }
                        Err(RecvError::Lagged(skipped)) => {
                            let _ = sender.send(EventStreamEvent::Lagged {
                                skipped: lag_to_usize(skipped),
                            });
                        }
                        Err(RecvError::Closed) => {
                            break;
                        }
                    }
                }
            }
        }
        event_stream.abort();
        if should_stop || shutdown_requested(shutdown) {
            break;
        }
        if wait_for_shutdown_or_delay(shutdown, backoff).await {
            break;
        }
    }
}
/// Events emitted by the status polling helper.
#[derive(Debug, Clone)]
pub enum StatusStreamEvent {
    /// Fresh status snapshot returned by Torii.
    Snapshot {
        /// Shared telemetry snapshot.
        snapshot: Arc<ToriiStatusSnapshot>,
        /// Optional Sumeragi status payload.
        sumeragi: Option<Arc<SumeragiV2Status>>,
        /// Optional non-authoritative Sumeragi diagnostics payload.
        sumeragi_diagnostics: Option<Arc<SumeragiDiagnosticsStatus>>,
        /// Optional metrics payload parsed from `/metrics`. When metrics polling is throttled,
        /// this value reuses the last successfully fetched snapshot until the refresh interval
        /// elapses.
        metrics: Option<Arc<ToriiMetricsSnapshot>>,
        /// Error information describing why metrics could not be fetched.
        metrics_error: Option<ToriiErrorInfo>,
    },
    /// Failed to fetch a snapshot; includes summary and failure count.
    Error {
        /// Classified error information suitable for UI display.
        error: ToriiErrorInfo,
        /// Number of consecutive failures observed so far.
        consecutive_failures: u32,
    },
    /// Status polling loop exited.
    Closed,
}
/// Configuration values used when spawning [`ManagedStatusStream`].
#[derive(Debug, Clone)]
pub struct StatusStreamOptions {
    /// Delay between successive `/status` polls.
    pub poll_interval: Duration,
    /// Optional refresh cadence for `/metrics`. When unset, metrics are fetched on every poll.
    /// Supply `Some(Duration::ZERO)` to disable metrics entirely, or a positive duration to
    /// throttle sampling to at most once per interval.
    pub metrics_poll_interval: Option<Duration>,
}
impl StatusStreamOptions {
    /// Create options with the supplied poll interval and default metrics behaviour.
    #[must_use]
    pub const fn new(poll_interval: Duration) -> Self {
        Self {
            poll_interval,
            metrics_poll_interval: None,
        }
    }
    /// Override the metrics refresh cadence.
    #[must_use]
    pub const fn with_metrics_poll_interval(mut self, interval: Option<Duration>) -> Self {
        self.metrics_poll_interval = interval;
        self
    }
}
/// Periodic status poller with exponential backoff on failures.
#[derive(Debug)]
pub struct ManagedStatusStream {
    sender: broadcast::Sender<StatusStreamEvent>,
    shutdown: watch::Sender<bool>,
    worker: JoinHandle<()>,
    alias: Arc<str>,
}
impl ManagedStatusStream {
    /// Spawn a polling loop that fetches `/status` on the requested interval.
    ///
    /// `poll_interval` controls the delay between successful samples. Failures
    /// automatically retry using the standard exponential backoff window shared
    /// with the streamed endpoints.
    pub fn spawn(
        handle: &Handle,
        alias: impl Into<String>,
        client: ToriiClient,
        poll_interval: Duration,
    ) -> Self {
        Self::spawn_with_options(
            handle,
            alias,
            client,
            StatusStreamOptions::new(poll_interval),
        )
    }
    /// Spawn a polling loop using the supplied options.
    pub fn spawn_with_options(
        handle: &Handle,
        alias: impl Into<String>,
        client: ToriiClient,
        options: StatusStreamOptions,
    ) -> Self {
        let (shutdown, mut shutdown_rx) = watch::channel(false);
        let (sender, _) = broadcast::channel(128);
        let alias: Arc<str> = Arc::from(alias.into().into_boxed_str());
        let worker_alias = alias.clone();
        let worker_sender = sender.clone();
        let worker_client = client.clone();
        let worker_options = options.clone();
        let worker = handle.spawn(async move {
            run_managed_status_stream(
                worker_alias,
                worker_client,
                worker_options,
                worker_sender,
                &mut shutdown_rx,
            )
            .await;
        });
        Self {
            sender,
            shutdown,
            worker,
            alias,
        }
    }
    /// Acquire a receiver yielding poll results and failure notices.
    pub fn subscribe(&self) -> broadcast::Receiver<StatusStreamEvent> {
        self.sender.subscribe()
    }
    /// Abort the polling loop immediately.
    pub fn abort(&self) {
        let _ = self.shutdown.send(true);
        if !self.worker.is_finished() {
            self.worker.abort();
        }
    }
    /// Returns `true` once the polling loop has terminated.
    pub fn is_finished(&self) -> bool {
        self.worker.is_finished()
    }
    /// Returns the alias associated with this status stream.
    pub fn alias(&self) -> &str {
        self.alias.as_ref()
    }
}
impl Drop for ManagedStatusStream {
    fn drop(&mut self) {
        self.abort();
    }
}
#[derive(Default)]
struct MetricsCache {
    last_snapshot: Option<Arc<ToriiMetricsSnapshot>>,
    last_error: Option<ToriiErrorInfo>,
    last_poll: Option<Instant>,
}
async fn run_managed_status_stream(
    _alias: Arc<str>,
    client: ToriiClient,
    options: StatusStreamOptions,
    sender: broadcast::Sender<StatusStreamEvent>,
    shutdown: &mut watch::Receiver<bool>,
) {
    let mut backoff = INITIAL_BACKOFF;
    let mut consecutive_failures = 0u32;
    let mut metrics_cache = MetricsCache::default();
    let poll_interval = options.poll_interval;
    let metrics_interval = options.metrics_poll_interval;
    loop {
        if shutdown_requested(shutdown) {
            break;
        }
        match client.fetch_status_snapshot().await {
            Ok(snapshot) => {
                let snapshot_arc = Arc::new(snapshot);
                let sumeragi = match client.fetch_sumeragi_status().await {
                    Ok(status) => Some(Arc::new(status)),
                    Err(err) => {
                        let _ = sender.send(StatusStreamEvent::Error {
                            error: err.summarize(),
                            consecutive_failures,
                        });
                        None
                    }
                };
                let sumeragi_diagnostics = match client.fetch_sumeragi_diagnostics().await {
                    Ok(diagnostics) => Some(Arc::new(diagnostics)),
                    Err(_) => None,
                };
                let (metrics, metrics_error) =
                    fetch_metrics_snapshot_if_needed(&client, metrics_interval, &mut metrics_cache)
                        .await;
                consecutive_failures = 0;
                backoff = INITIAL_BACKOFF;
                let _ = sender.send(StatusStreamEvent::Snapshot {
                    snapshot: snapshot_arc,
                    sumeragi,
                    sumeragi_diagnostics,
                    metrics,
                    metrics_error,
                });
            }
            Err(err) => {
                consecutive_failures = consecutive_failures.saturating_add(1);
                let _ = sender.send(StatusStreamEvent::Error {
                    error: err.summarize(),
                    consecutive_failures,
                });
                if wait_for_shutdown_or_delay(shutdown, backoff).await {
                    let _ = sender.send(StatusStreamEvent::Closed);
                    return;
                }
                backoff = (backoff.saturating_mul(2)).min(MAX_BACKOFF);
                continue;
            }
        }
        if wait_for_shutdown_or_delay(shutdown, poll_interval).await {
            break;
        }
    }
    let _ = sender.send(StatusStreamEvent::Closed);
}
async fn fetch_metrics_snapshot_if_needed(
    client: &ToriiClient,
    interval: Option<Duration>,
    cache: &mut MetricsCache,
) -> (Option<Arc<ToriiMetricsSnapshot>>, Option<ToriiErrorInfo>) {
    match interval {
        Some(delay) if delay.is_zero() => (None, None),
        None => fetch_metrics_snapshot_now(client, cache).await,
        Some(delay) => {
            let now = Instant::now();
            let should_fetch = cache
                .last_poll
                .map(|last| now.saturating_duration_since(last) >= delay)
                .unwrap_or(true);
            if should_fetch {
                cache.last_poll = Some(now);
                match client.fetch_metrics_snapshot().await {
                    Ok(snapshot) => {
                        let arc = Arc::new(snapshot);
                        cache.last_snapshot = Some(arc.clone());
                        cache.last_error = None;
                        (Some(arc), None)
                    }
                    Err(err) => {
                        let summary = err.summarize();
                        cache.last_error = Some(summary.clone());
                        (cache.last_snapshot.clone(), Some(summary))
                    }
                }
            } else {
                (cache.last_snapshot.clone(), cache.last_error.clone())
            }
        }
    }
}
async fn fetch_metrics_snapshot_now(
    client: &ToriiClient,
    cache: &mut MetricsCache,
) -> (Option<Arc<ToriiMetricsSnapshot>>, Option<ToriiErrorInfo>) {
    cache.last_poll = Some(Instant::now());
    match client.fetch_metrics_snapshot().await {
        Ok(snapshot) => {
            let arc = Arc::new(snapshot);
            cache.last_snapshot = Some(arc.clone());
            cache.last_error = None;
            (Some(arc), None)
        }
        Err(err) => {
            let summary = err.summarize();
            cache.last_error = Some(summary.clone());
            (None, Some(summary))
        }
    }
}
fn shutdown_requested(shutdown: &watch::Receiver<bool>) -> bool {
    *shutdown.borrow()
}
async fn wait_for_shutdown_or_delay(shutdown: &mut watch::Receiver<bool>, delay: Duration) -> bool {
    if shutdown_requested(shutdown) {
        return true;
    }
    tokio::select! {
        changed = shutdown.changed() => {
            match changed {
                Ok(_) => shutdown_requested(shutdown),
                Err(_) => true,
            }
        }
        _ = sleep(delay) => false,
    }
}
