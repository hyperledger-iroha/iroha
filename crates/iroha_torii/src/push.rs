//! Best-effort Torii push notification bridge.
//!
//! Local device, delivery, and provider-credential files are consumed through bounded direct-file
//! reads. Startup enumeration and dispatch are likewise count-bounded so corrupted local
//! persistence cannot determine peak memory. Remote provider bodies are streamed under one
//! source-coupled ceiling, and successful delivery responses are not buffered.
use crate::account_activity::AccountActivityRole;
use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as BASE64_URL_SAFE_NO_PAD};
use dashmap::DashMap;
use iroha_config::parameters::actual;
use iroha_core::{EventsSender, kura::Kura};
use iroha_crypto::HashOf;
use iroha_data_model::{
    account::AccountId,
    block::SignedBlock,
    events::{
        EventBox,
        pipeline::{BlockStatus, PipelineEventBox},
    },
    transaction::signed::{SignedTransaction, TransactionEntrypoint, TransactionResult},
};
use jsonwebtoken::{Algorithm, EncodingKey};
#[cfg(test)]
use nonzero_ext::nonzero;
use parking_lot::Mutex as StorageMutex;
use reqwest::StatusCode as HttpStatusCode;
use sha2::{Digest as _, Sha256};
#[cfg(test)]
use std::sync::Mutex;
use std::{
    fs,
    io::{self, Read as _, Write as _},
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
const PUSH_DIR: &str = "push";
const DEVICES_DIR: &str = "devices";
const QUEUE_DIR: &str = "queue";
const FCM_TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
const FCM_SCOPE: &str = "https://www.googleapis.com/auth/firebase.messaging";
const APNS_SANDBOX_ENDPOINT: &str = "https://api.sandbox.push.apple.com";
const APNS_PRODUCTION_ENDPOINT: &str = "https://api.push.apple.com";
const INITIAL_BACKOFF: Duration = Duration::from_secs(30);
const MAX_BACKOFF: Duration = Duration::from_secs(60 * 60);
const DISPATCH_TICK: Duration = Duration::from_secs(5);
// Push persistence is local best-effort state, but every record remains resident
// after startup. These source-local ceilings bound both directory work and the
// retained maps; field validation keeps a maximum-count set from amplifying its
// bounded on-disk representation into unbounded strings or collections.
const MAX_PUSH_DEVICES: usize = 4 * 1024;
const MAX_PUSH_QUEUE_JOBS: usize = 4 * MAX_PUSH_DEVICES;
const MAX_PUSH_DEVICE_ACCOUNT_ENTRIES: usize = MAX_PUSH_DEVICES;
const MAX_PUSH_DEVICE_RECORD_BYTES: usize = 64 * 1024;
const MAX_PUSH_QUEUE_JOB_BYTES: usize = 16 * 1024;
const MAX_PUSH_TOKEN_BYTES: usize = 8 * 1024;
// `torii.push.max_topics_per_device` may lower, but never raise, this invariant.
const MAX_PUSH_TOPICS: usize = 256;
const MAX_PUSH_TOPIC_BYTES: usize = 256;
const MAX_PUSH_TOPIC_BYTES_PER_DEVICE: usize = 8 * 1024;
const MAX_PUSH_ACTIVITY_KIND_BYTES: usize = 256;
const MAX_PUSH_TX_HASH_BYTES: usize = 256;
const MAX_PUSH_DIRECTION_BYTES: usize = 64;
const MAX_FCM_SERVICE_ACCOUNT_BYTES: usize = 64 * 1024;
const MAX_APNS_PRIVATE_KEY_BYTES: usize = 16 * 1024;
// FCM OAuth/send and APNs responses are small, fixed-schema JSON envelopes.
// Keep one local ceiling for every provider-controlled response body so a
// proxy, chunked sender, or future transparent decompressor cannot grow a
// dispatch task's retained buffer without bound.
const MAX_PUSH_PROVIDER_RESPONSE_BYTES: usize = 64 * 1024;
const PUSH_DISPATCH_BATCH_SIZE: usize = 256;
#[derive(Clone, Copy, Debug)]
struct PushStorageLimits {
    max_devices: usize,
    max_queue_jobs: usize,
    max_device_account_entries: usize,
    max_device_record_bytes: usize,
    max_queue_job_bytes: usize,
    dispatch_batch_size: usize,
}
const PUSH_STORAGE_LIMITS: PushStorageLimits = PushStorageLimits {
    max_devices: MAX_PUSH_DEVICES,
    max_queue_jobs: MAX_PUSH_QUEUE_JOBS,
    max_device_account_entries: MAX_PUSH_DEVICE_ACCOUNT_ENTRIES,
    max_device_record_bytes: MAX_PUSH_DEVICE_RECORD_BYTES,
    max_queue_job_bytes: MAX_PUSH_QUEUE_JOB_BYTES,
    dispatch_batch_size: PUSH_DISPATCH_BATCH_SIZE,
};
/// Logical push notification target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Fcm,
    Apns,
}
impl Platform {
    fn from_label(label: &str) -> Result<Self, PushError> {
        match label.trim().to_ascii_uppercase().as_str() {
            "FCM" => Ok(Self::Fcm),
            "APNS" => Ok(Self::Apns),
            other => Err(PushError::InvalidPlatform(other.to_string())),
        }
    }
    fn from_stored(label: &str) -> Option<Self> {
        match label {
            "FCM" => Some(Self::Fcm),
            "APNS" => Some(Self::Apns),
            _ => None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Fcm => "FCM",
            Self::Apns => "APNS",
        }
    }
}
/// Request payload for `POST /v1/notify/devices`.
#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonDeserialize,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::NoritoSerialize,
)]
pub struct RegisterDeviceRequest {
    pub account_id: String,
    pub platform: String,
    pub token: String,
    pub topics: Option<Vec<String>>,
}
/// Request payload for `DELETE /v1/notify/devices`.
pub type UnregisterDeviceRequest = RegisterDeviceRequest;
#[derive(Clone, Debug)]
pub struct RegisteredDevice {
    pub account_id: String,
    pub platform: Platform,
    pub topics: Vec<String>,
    pub token_fingerprint: String,
}
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    crate::json_macros::JsonDeserialize,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::NoritoSerialize,
)]
pub struct PushActivityPayload {
    pub account_id: String,
    pub activity_kind: String,
    pub tx_hash: String,
    pub block_height: u64,
    pub instruction_index: u64,
    pub direction: String,
}
#[derive(Debug, Clone)]
pub enum PushError {
    Disabled,
    InvalidAccount(String),
    InvalidPlatform(String),
    InvalidEnvironment(String),
    MissingCredentials { platform: Platform },
    TooManyTopics { max: usize },
    EmptyToken,
    Storage(String),
}
/// Dispatch settings derived from configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DispatchSettings {
    /// HTTP connect timeout.
    pub connect_timeout: Duration,
    /// HTTP request timeout.
    pub request_timeout: Duration,
}
/// Provider-specific credentials derived from configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderCredentials {
    Fcm {
        project_id: String,
        service_account_path: PathBuf,
    },
    Apns {
        endpoint: String,
        topic: String,
        team_id: String,
        key_id: String,
        private_key_path: PathBuf,
    },
}
/// Single provider delivery attempt.
#[derive(Clone, Debug)]
pub struct PushDelivery {
    pub platform: Platform,
    pub token: String,
    pub payload: PushActivityPayload,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DispatchOutcome {
    Sent,
    Retry(String),
    InvalidToken(String),
    PermanentFailure(String),
}
/// Dispatcher trait so provider delivery can be mocked in tests.
#[async_trait]
pub trait PushDispatcher: Send + Sync {
    /// Dispatch a payload to the configured push provider.
    async fn send(
        &self,
        delivery: &PushDelivery,
        settings: &DispatchSettings,
        credentials: &ProviderCredentials,
    ) -> DispatchOutcome;
}
#[derive(Clone, Default)]
struct RealPushDispatcher;
#[async_trait]
impl PushDispatcher for RealPushDispatcher {
    async fn send(
        &self,
        delivery: &PushDelivery,
        settings: &DispatchSettings,
        credentials: &ProviderCredentials,
    ) -> DispatchOutcome {
        match credentials {
            ProviderCredentials::Fcm {
                project_id,
                service_account_path,
            } => send_fcm_http_v1(project_id, service_account_path, delivery, settings).await,
            ProviderCredentials::Apns {
                endpoint,
                topic,
                team_id,
                key_id,
                private_key_path,
            } => {
                send_apns_http2(
                    endpoint,
                    topic,
                    team_id,
                    key_id,
                    private_key_path,
                    delivery,
                    settings,
                )
                .await
            }
        }
    }
}
#[derive(Clone)]
pub struct PushBridge {
    config: actual::Push,
    settings: DispatchSettings,
    dispatcher: Arc<dyn PushDispatcher>,
    devices: Arc<DashMap<String, DeviceRecord>>,
    queue: Arc<DashMap<String, DeliveryJob>>,
    data_dir: Arc<PathBuf>,
    storage_limits: PushStorageLimits,
    mutation_guard: Arc<StorageMutex<()>>,
}
impl PushBridge {
    pub fn new(config: actual::Push) -> Self {
        Self::with_dispatcher(config, Arc::new(RealPushDispatcher))
    }
    pub fn with_dispatcher(config: actual::Push, dispatcher: Arc<dyn PushDispatcher>) -> Self {
        Self::with_dispatcher_and_limits(config, dispatcher, PUSH_STORAGE_LIMITS)
    }
    fn with_dispatcher_and_limits(
        config: actual::Push,
        dispatcher: Arc<dyn PushDispatcher>,
        storage_limits: PushStorageLimits,
    ) -> Self {
        let settings = DispatchSettings {
            connect_timeout: config.connect_timeout,
            request_timeout: config.request_timeout,
        };
        let data_dir = Arc::new(crate::data_dir::base_dir().join(PUSH_DIR));
        let bridge = Self {
            config,
            settings,
            dispatcher,
            devices: Arc::new(DashMap::new()),
            queue: Arc::new(DashMap::new()),
            data_dir,
            storage_limits,
            mutation_guard: Arc::new(StorageMutex::new(())),
        };
        bridge.load_from_disk();
        bridge
    }
    pub fn register_device(&self, request: RegisterDeviceRequest) -> Result<(), PushError> {
        if !self.config.enabled {
            return Err(PushError::Disabled);
        }
        let account_id = canonical_account(&request.account_id)?;
        let platform = Platform::from_label(&request.platform)?;
        if !self.has_credentials(platform) {
            return Err(PushError::MissingCredentials { platform });
        }
        let token = request.token.trim();
        if token.is_empty() {
            return Err(PushError::EmptyToken);
        }
        if token.len() > MAX_PUSH_TOKEN_BYTES {
            return Err(storage_limit_error(
                "device token bytes",
                token.len(),
                MAX_PUSH_TOKEN_BYTES,
            ));
        }
        let max_topics = self.config.max_topics_per_device.get().min(MAX_PUSH_TOPICS);
        let topics = normalize_topics(request.topics, max_topics)?;
        let token = token.to_owned();
        let token_fingerprint = fingerprint(token.as_bytes());
        let record = DeviceRecord {
            account_id: account_id.to_string(),
            platform: platform.label().to_string(),
            token,
            token_fingerprint: token_fingerprint.clone(),
            topics,
            updated_at_ms: now_ms(),
        };
        let _mutation = self.mutation_guard.lock();
        let old = self
            .devices
            .get(&token_fingerprint)
            .map(|entry| entry.clone());
        if old.is_none() && self.devices.len() >= self.storage_limits.max_devices {
            return Err(storage_limit_error(
                "registered device count",
                self.devices.len().saturating_add(1),
                self.storage_limits.max_devices,
            ));
        }
        if let Some(old) = old {
            let old_path = self.device_path(&old.account_id, &token_fingerprint);
            remove_device_file(&old_path).map_err(storage_error)?;
        }
        self.persist_device(&record)?;
        self.devices.insert(token_fingerprint, record);
        Ok(())
    }
    pub fn unregister_device(&self, request: UnregisterDeviceRequest) -> Result<(), PushError> {
        if !self.config.enabled {
            return Err(PushError::Disabled);
        }
        let account_id = canonical_account(&request.account_id)?;
        let platform = Platform::from_label(&request.platform)?;
        let token = request.token.trim();
        if token.is_empty() {
            return Err(PushError::EmptyToken);
        }
        if token.len() > MAX_PUSH_TOKEN_BYTES {
            // Registration rejects such tokens, so no persisted record can match.
            return Ok(());
        }
        let token_fingerprint = fingerprint(token.as_bytes());
        let Some(record) = self
            .devices
            .get(&token_fingerprint)
            .map(|entry| entry.clone())
        else {
            return Ok(());
        };
        if record.account_id != account_id.to_string() || record.platform != platform.label() {
            return Ok(());
        }
        drop(record);
        self.remove_device_by_fingerprint(&token_fingerprint)
    }
    pub(crate) fn enqueue_activity(
        &self,
        account: &AccountId,
        tx_hash: &str,
        block_height: u64,
        instruction_index: usize,
        activity_kind: &str,
        role: AccountActivityRole,
    ) -> Result<usize, PushError> {
        if !self.config.enabled {
            return Err(PushError::Disabled);
        }
        let account_id = account.to_string();
        let devices = self.devices_for_account(&account_id);
        let mut queued = 0usize;
        for device in devices {
            let payload = PushActivityPayload {
                account_id: account_id.clone(),
                activity_kind: activity_kind.to_string(),
                tx_hash: tx_hash.to_string(),
                block_height,
                instruction_index: instruction_index as u64,
                direction: role.as_str().to_string(),
            };
            let dedupe_key = delivery_dedupe_key(&payload, &device.token_fingerprint);
            let job = DeliveryJob {
                dedupe_key: dedupe_key.clone(),
                account_id: account_id.clone(),
                token_fingerprint: device.token_fingerprint.clone(),
                payload,
                attempts: 0,
                next_attempt_ms: now_ms(),
                created_at_ms: now_ms(),
                updated_at_ms: now_ms(),
            };
            let _mutation = self.mutation_guard.lock();
            if self.queue.contains_key(&dedupe_key) {
                continue;
            }
            if self.queue.len() >= self.storage_limits.max_queue_jobs {
                return Err(storage_limit_error(
                    "push queue job count",
                    self.queue.len().saturating_add(1),
                    self.storage_limits.max_queue_jobs,
                ));
            }
            self.persist_job(&job)?;
            self.queue.insert(dedupe_key, job);
            queued += 1;
        }
        Ok(queued)
    }
    pub async fn dispatch_due_once(&self) {
        if !self.config.enabled {
            return;
        }
        let now = now_ms();
        let keys: Vec<String> = self
            .queue
            .iter()
            .filter(|entry| entry.next_attempt_ms <= now)
            .map(|entry| entry.key().clone())
            .take(self.storage_limits.dispatch_batch_size)
            .collect();
        for key in keys {
            let Some(job) = self.queue.get(&key).map(|entry| entry.clone()) else {
                continue;
            };
            self.dispatch_job(job).await;
        }
    }
    pub fn start_event_worker(&self, kura: Arc<Kura>, events: EventsSender) {
        if !self.config.enabled {
            return;
        }
        let bridge = self.clone();
        tokio::spawn(async move {
            let mut receiver = events.subscribe();
            let mut tick = tokio::time::interval(DISPATCH_TICK);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    event = receiver.recv() => match event {
                        Ok(event) => {
                            if let Some(height) = committed_block_height(&event) {
                                bridge.enqueue_committed_block(&kura, height);
                                bridge.dispatch_due_once().await;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                            iroha_logger::warn!(skipped, "push bridge skipped lagged events");
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    },
                    _ = tick.tick() => {
                        bridge.dispatch_due_once().await;
                    }
                }
            }
        });
    }
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }
    pub fn queued_count(&self) -> usize {
        self.queue.len()
    }
    #[cfg(test)]
    pub(crate) fn registered_device_by_token(&self, token: &str) -> Option<RegisteredDevice> {
        self.devices
            .get(&fingerprint(token.as_bytes()))
            .map(|entry| registered_device_from_record(&entry))
    }
    #[cfg(test)]
    pub(crate) fn registered_device(&self, token: &str) -> Option<RegisteredDevice> {
        self.registered_device_by_token(token)
    }
    fn enqueue_committed_block(&self, kura: &Kura, height: u64) {
        let Some(height) = usize::try_from(height).ok().and_then(NonZeroUsize::new) else {
            return;
        };
        let Some(block) = kura.get_block(height) else {
            return;
        };
        self.enqueue_block_activities(&block);
    }
    fn enqueue_block_activities(&self, block: &SignedBlock) {
        let block_height = block.header().height().get();
        for (entrypoint_hash, tx, result) in external_signed_transaction_results(block) {
            if result.is_err() {
                continue;
            }
            let tx_hash = entrypoint_hash.to_string();
            for (instruction_index, instruction) in
                tx.instructions().explicit_instructions().enumerate()
            {
                let activity_kind = crate::explorer::instruction_kind(instruction).as_str();
                for activity in crate::account_activity::instruction_account_activities(instruction)
                {
                    if let Err(error) = self.enqueue_activity(
                        &activity.account,
                        &tx_hash,
                        block_height,
                        instruction_index,
                        activity_kind,
                        activity.role,
                    ) {
                        iroha_logger::warn!(?error, "failed to enqueue push activity");
                    }
                }
            }
        }
    }
    async fn dispatch_job(&self, job: DeliveryJob) {
        let Some(device) = self
            .devices
            .get(&job.token_fingerprint)
            .map(|entry| entry.clone())
        else {
            self.remove_job(&job.dedupe_key);
            return;
        };
        let Some(platform) = Platform::from_stored(&device.platform) else {
            self.remove_job(&job.dedupe_key);
            return;
        };
        let credentials = match self.credentials_for(platform) {
            Ok(credentials) => credentials,
            Err(error) => {
                iroha_logger::warn!(?error, "push provider credentials unavailable");
                self.reschedule_job(job);
                return;
            }
        };
        let delivery = PushDelivery {
            platform,
            token: device.token,
            payload: job.payload.clone(),
        };
        match self
            .dispatcher
            .send(&delivery, &self.settings, &credentials)
            .await
        {
            DispatchOutcome::Sent | DispatchOutcome::PermanentFailure(_) => {
                self.remove_job(&job.dedupe_key);
            }
            DispatchOutcome::InvalidToken(reason) => {
                iroha_logger::info!(
                    token_fingerprint = job.token_fingerprint.as_str(),
                    reason = reason.as_str(),
                    "removing invalid push token"
                );
                self.remove_job(&job.dedupe_key);
                if let Err(error) = self.remove_device_by_fingerprint(&job.token_fingerprint) {
                    iroha_logger::warn!(?error, "failed to remove invalid push device");
                }
            }
            DispatchOutcome::Retry(reason) => {
                iroha_logger::debug!(
                    reason = reason.as_str(),
                    "push delivery scheduled for retry"
                );
                self.reschedule_job(job);
            }
        }
    }
    fn reschedule_job(&self, mut job: DeliveryJob) {
        job.attempts = job.attempts.saturating_add(1);
        let backoff = retry_backoff(job.attempts);
        job.next_attempt_ms = now_ms().saturating_add(duration_ms(backoff));
        job.updated_at_ms = now_ms();
        let _mutation = self.mutation_guard.lock();
        if !self.queue.contains_key(&job.dedupe_key)
            && self.queue.len() >= self.storage_limits.max_queue_jobs
        {
            iroha_logger::warn!(
                maximum = self.storage_limits.max_queue_jobs,
                "dropping push retry because the hard queue capacity is full"
            );
            return;
        }
        if let Err(error) = self.persist_job(&job) {
            iroha_logger::warn!(?error, "failed to persist push retry");
        }
        self.queue.insert(job.dedupe_key.clone(), job);
    }
    fn has_credentials(&self, platform: Platform) -> bool {
        self.credentials_for(platform).is_ok()
    }
    fn credentials_for(&self, platform: Platform) -> Result<ProviderCredentials, PushError> {
        match platform {
            Platform::Fcm => match (
                trim_ref(self.config.fcm_project_id.as_deref()),
                self.config.fcm_service_account_path.as_ref(),
            ) {
                (Some(project_id), Some(path)) => Ok(ProviderCredentials::Fcm {
                    project_id: project_id.to_owned(),
                    service_account_path: path.clone(),
                }),
                _ => Err(PushError::MissingCredentials { platform }),
            },
            Platform::Apns => {
                let endpoint = match self
                    .config
                    .apns_endpoint
                    .as_ref()
                    .and_then(|s| trim_ref(Some(s)))
                {
                    Some(endpoint) => endpoint.to_owned(),
                    None => match self.config.apns_environment.trim() {
                        "sandbox" => APNS_SANDBOX_ENDPOINT.to_string(),
                        "production" => APNS_PRODUCTION_ENDPOINT.to_string(),
                        other => return Err(PushError::InvalidEnvironment(other.to_string())),
                    },
                };
                match (
                    trim_ref(self.config.apns_topic.as_deref()),
                    trim_ref(self.config.apns_team_id.as_deref()),
                    trim_ref(self.config.apns_key_id.as_deref()),
                    self.config.apns_private_key_path.as_ref(),
                ) {
                    (Some(topic), Some(team_id), Some(key_id), Some(path)) => {
                        Ok(ProviderCredentials::Apns {
                            endpoint,
                            topic: topic.to_owned(),
                            team_id: team_id.to_owned(),
                            key_id: key_id.to_owned(),
                            private_key_path: path.clone(),
                        })
                    }
                    _ => Err(PushError::MissingCredentials { platform }),
                }
            }
        }
    }
    fn devices_for_account(&self, account_id: &str) -> Vec<DeviceRecord> {
        self.devices
            .iter()
            .filter(|entry| entry.account_id == account_id)
            .map(|entry| entry.clone())
            .take(self.storage_limits.max_devices)
            .collect()
    }
    fn remove_device_by_fingerprint(&self, token_fingerprint: &str) -> Result<(), PushError> {
        let _mutation = self.mutation_guard.lock();
        let Some((_, record)) = self.devices.remove(token_fingerprint) else {
            return Ok(());
        };
        remove_device_file(&self.device_path(&record.account_id, token_fingerprint))
            .map_err(storage_error)?;
        Ok(())
    }
    fn remove_job(&self, dedupe_key: &str) {
        let _mutation = self.mutation_guard.lock();
        self.queue.remove(dedupe_key);
        if let Err(error) = remove_file_if_exists(&self.job_path(dedupe_key)) {
            iroha_logger::warn!(?error, "failed to remove push queue file");
        }
    }
    fn persist_device(&self, record: &DeviceRecord) -> Result<(), PushError> {
        validate_device_record(record, self.effective_max_topics()).map_err(storage_error)?;
        write_json_atomic(
            &self.device_path(&record.account_id, &record.token_fingerprint),
            record,
            self.storage_limits.max_device_record_bytes,
        )
        .map_err(storage_error)
    }
    fn persist_job(&self, job: &DeliveryJob) -> Result<(), PushError> {
        validate_delivery_job(job).map_err(storage_error)?;
        write_json_atomic(
            &self.job_path(&job.dedupe_key),
            job,
            self.storage_limits.max_queue_job_bytes,
        )
        .map_err(storage_error)
    }
    fn load_from_disk(&self) {
        self.load_devices();
        self.load_queue();
    }
    fn load_devices(&self) {
        let root = self.data_dir.join(DEVICES_DIR);
        let accounts = match read_direct_directory(&root) {
            Ok(accounts) => accounts,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return,
            Err(error) => {
                iroha_logger::warn!(?error, path = %root.display(), "failed to enumerate push device accounts");
                return;
            }
        };
        let mut account_entries = 0_usize;
        let mut device_entries = 0_usize;
        'accounts: for account_dir in accounts {
            account_entries = account_entries.saturating_add(1);
            if account_entries > self.storage_limits.max_device_account_entries {
                iroha_logger::warn!(
                    maximum = self.storage_limits.max_device_account_entries,
                    "push device account directory entry limit reached"
                );
                break;
            }
            let account_dir = match account_dir {
                Ok(entry) => entry,
                Err(error) => {
                    iroha_logger::warn!(?error, path = %root.display(), "failed to inspect push device account entry");
                    continue;
                }
            };
            let account_path = account_dir.path();
            let account_type = match account_dir.file_type() {
                Ok(file_type) => file_type,
                Err(error) => {
                    iroha_logger::warn!(?error, path = %account_path.display(), "failed to inspect push device account type");
                    continue;
                }
            };
            if account_type.is_symlink() || !account_type.is_dir() {
                continue;
            }
            let files = match read_direct_directory(&account_path) {
                Ok(files) => files,
                Err(error) => {
                    iroha_logger::warn!(?error, path = %account_path.display(), "failed to enumerate push device files");
                    continue;
                }
            };
            for file in files {
                device_entries = device_entries.saturating_add(1);
                if device_entries > self.storage_limits.max_devices {
                    iroha_logger::warn!(
                        maximum = self.storage_limits.max_devices,
                        "push device file entry limit reached"
                    );
                    break 'accounts;
                }
                let file = match file {
                    Ok(entry) => entry,
                    Err(error) => {
                        iroha_logger::warn!(?error, path = %account_path.display(), "failed to inspect push device file entry");
                        continue;
                    }
                };
                let path = file.path();
                let file_type = match file.file_type() {
                    Ok(file_type) => file_type,
                    Err(error) => {
                        iroha_logger::warn!(?error, path = %path.display(), "failed to inspect push device file type");
                        continue;
                    }
                };
                if file_type.is_symlink() || !file_type.is_file() {
                    continue;
                }
                match read_json::<DeviceRecord>(&path, self.storage_limits.max_device_record_bytes)
                {
                    Ok(record) => {
                        if let Err(error) = validate_device_record(&record, MAX_PUSH_TOPICS) {
                            iroha_logger::warn!(?error, path = %path.display(), "invalid persisted push device");
                            continue;
                        }
                        if self.devices.len() >= self.storage_limits.max_devices
                            && !self.devices.contains_key(&record.token_fingerprint)
                        {
                            iroha_logger::warn!(
                                maximum = self.storage_limits.max_devices,
                                "push device capacity reached while loading"
                            );
                            break 'accounts;
                        }
                        self.devices
                            .insert(record.token_fingerprint.clone(), record);
                    }
                    Err(error) => {
                        iroha_logger::warn!(?error, path = %path.display(), "failed to load push device")
                    }
                }
            }
        }
    }
    fn load_queue(&self) {
        let root = self.data_dir.join(QUEUE_DIR);
        let files = match read_direct_directory(&root) {
            Ok(files) => files,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return,
            Err(error) => {
                iroha_logger::warn!(?error, path = %root.display(), "failed to enumerate push queue");
                return;
            }
        };
        let mut queue_entries = 0_usize;
        for file in files {
            queue_entries = queue_entries.saturating_add(1);
            if queue_entries > self.storage_limits.max_queue_jobs {
                iroha_logger::warn!(
                    maximum = self.storage_limits.max_queue_jobs,
                    "push queue file entry limit reached"
                );
                break;
            }
            let file = match file {
                Ok(entry) => entry,
                Err(error) => {
                    iroha_logger::warn!(?error, path = %root.display(), "failed to inspect push queue entry");
                    continue;
                }
            };
            let path = file.path();
            let file_type = match file.file_type() {
                Ok(file_type) => file_type,
                Err(error) => {
                    iroha_logger::warn!(?error, path = %path.display(), "failed to inspect push queue file type");
                    continue;
                }
            };
            if file_type.is_symlink() || !file_type.is_file() {
                continue;
            }
            match read_json::<DeliveryJob>(&path, self.storage_limits.max_queue_job_bytes) {
                Ok(job) => {
                    if let Err(error) = validate_delivery_job(&job) {
                        iroha_logger::warn!(?error, path = %path.display(), "invalid persisted push queue job");
                        continue;
                    }
                    if self.queue.len() >= self.storage_limits.max_queue_jobs
                        && !self.queue.contains_key(&job.dedupe_key)
                    {
                        iroha_logger::warn!(
                            maximum = self.storage_limits.max_queue_jobs,
                            "push queue capacity reached while loading"
                        );
                        break;
                    }
                    self.queue.insert(job.dedupe_key.clone(), job);
                }
                Err(error) => {
                    iroha_logger::warn!(?error, path = %path.display(), "failed to load push queue job")
                }
            }
        }
    }
    fn effective_max_topics(&self) -> usize {
        self.config.max_topics_per_device.get().min(MAX_PUSH_TOPICS)
    }
    fn device_path(&self, account_id: &str, token_fingerprint: &str) -> PathBuf {
        self.data_dir
            .join(DEVICES_DIR)
            .join(fingerprint(account_id.as_bytes()))
            .join(format!("{token_fingerprint}.json"))
    }
    fn job_path(&self, dedupe_key: &str) -> PathBuf {
        self.data_dir
            .join(QUEUE_DIR)
            .join(format!("{dedupe_key}.json"))
    }
}
#[derive(
    Clone,
    Debug,
    crate::json_macros::JsonDeserialize,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::NoritoSerialize,
)]
struct DeviceRecord {
    account_id: String,
    platform: String,
    token: String,
    token_fingerprint: String,
    topics: Vec<String>,
    updated_at_ms: u64,
}
#[derive(
    Clone,
    Debug,
    crate::json_macros::JsonDeserialize,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::NoritoSerialize,
)]
struct DeliveryJob {
    dedupe_key: String,
    account_id: String,
    token_fingerprint: String,
    payload: PushActivityPayload,
    attempts: u32,
    next_attempt_ms: u64,
    created_at_ms: u64,
    updated_at_ms: u64,
}
fn registered_device_from_record(record: &DeviceRecord) -> RegisteredDevice {
    RegisteredDevice {
        account_id: record.account_id.clone(),
        platform: Platform::from_stored(&record.platform).unwrap_or(Platform::Fcm),
        topics: record.topics.clone(),
        token_fingerprint: record.token_fingerprint.clone(),
    }
}
fn canonical_account(account_id: &str) -> Result<AccountId, PushError> {
    let trimmed = account_id.trim();
    AccountId::parse_encoded(trimmed)
        .map(|parsed| parsed.into_account_id())
        .map_err(|_| PushError::InvalidAccount(account_id.to_owned()))
}
fn validate_device_record(record: &DeviceRecord, max_topics: usize) -> io::Result<()> {
    let account = canonical_account(&record.account_id)
        .map_err(|_| invalid_data("persisted push device has an invalid account id"))?;
    if account.to_string() != record.account_id {
        return Err(invalid_data(
            "persisted push device account id is not canonical",
        ));
    }
    if Platform::from_stored(&record.platform).is_none() {
        return Err(invalid_data(
            "persisted push device has an invalid platform",
        ));
    }
    if record.token.is_empty()
        || record.token.trim() != record.token
        || record.token.len() > MAX_PUSH_TOKEN_BYTES
    {
        return Err(invalid_data(
            "persisted push device token violates the byte or whitespace bound",
        ));
    }
    if record.token_fingerprint != fingerprint(record.token.as_bytes()) {
        return Err(invalid_data(
            "persisted push device token fingerprint does not match its token",
        ));
    }
    if record.topics.len() > max_topics.min(MAX_PUSH_TOPICS) {
        return Err(invalid_data(
            "persisted push device topic count exceeds the configured hard bound",
        ));
    }
    let mut topic_bytes = 0_usize;
    for (index, topic) in record.topics.iter().enumerate() {
        if topic.is_empty() || topic.trim() != topic || topic.len() > MAX_PUSH_TOPIC_BYTES {
            return Err(invalid_data(
                "persisted push device topic violates the byte or whitespace bound",
            ));
        }
        if record.topics[..index].contains(topic) {
            return Err(invalid_data(
                "persisted push device contains duplicate topics",
            ));
        }
        topic_bytes = topic_bytes
            .checked_add(topic.len())
            .ok_or_else(|| invalid_data("persisted push device topic bytes overflow"))?;
        if topic_bytes > MAX_PUSH_TOPIC_BYTES_PER_DEVICE {
            return Err(invalid_data(
                "persisted push device aggregate topic bytes exceed the hard bound",
            ));
        }
    }
    Ok(())
}
fn validate_delivery_job(job: &DeliveryJob) -> io::Result<()> {
    let account = canonical_account(&job.account_id)
        .map_err(|_| invalid_data("persisted push job has an invalid account id"))?;
    if account.to_string() != job.account_id || job.payload.account_id != job.account_id {
        return Err(invalid_data(
            "persisted push job account id is non-canonical or inconsistent",
        ));
    }
    if !is_lower_hex_fingerprint(&job.token_fingerprint) {
        return Err(invalid_data(
            "persisted push job has an invalid token fingerprint",
        ));
    }
    if job.payload.activity_kind.len() > MAX_PUSH_ACTIVITY_KIND_BYTES
        || job.payload.tx_hash.len() > MAX_PUSH_TX_HASH_BYTES
        || job.payload.direction.len() > MAX_PUSH_DIRECTION_BYTES
    {
        return Err(invalid_data(
            "persisted push job payload field exceeds its hard byte bound",
        ));
    }
    if job.dedupe_key != delivery_dedupe_key(&job.payload, &job.token_fingerprint) {
        return Err(invalid_data(
            "persisted push job dedupe key does not match its payload",
        ));
    }
    Ok(())
}
fn is_lower_hex_fingerprint(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
fn normalize_topics(
    topics: Option<Vec<String>>,
    max_topics: usize,
) -> Result<Vec<String>, PushError> {
    let mut out = Vec::new();
    let mut topic_bytes = 0_usize;
    for topic in topics.unwrap_or_default() {
        let topic = topic.trim();
        if !topic.is_empty() && !out.iter().any(|seen| seen == topic) {
            if out.len() >= max_topics {
                return Err(PushError::TooManyTopics { max: max_topics });
            }
            if topic.len() > MAX_PUSH_TOPIC_BYTES {
                return Err(storage_limit_error(
                    "push topic bytes",
                    topic.len(),
                    MAX_PUSH_TOPIC_BYTES,
                ));
            }
            topic_bytes = topic_bytes.checked_add(topic.len()).ok_or_else(|| {
                storage_limit_error(
                    "aggregate push topic bytes",
                    usize::MAX,
                    MAX_PUSH_TOPIC_BYTES_PER_DEVICE,
                )
            })?;
            if topic_bytes > MAX_PUSH_TOPIC_BYTES_PER_DEVICE {
                return Err(storage_limit_error(
                    "aggregate push topic bytes",
                    topic_bytes,
                    MAX_PUSH_TOPIC_BYTES_PER_DEVICE,
                ));
            }
            out.push(topic.to_owned());
        }
    }
    Ok(out)
}
fn trim_ref(value: Option<&str>) -> Option<&str> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        (!trimmed.is_empty()).then_some(trimmed)
    })
}
fn delivery_dedupe_key(payload: &PushActivityPayload, token_fingerprint: &str) -> String {
    fingerprint(
        format!(
            "{}\0{}\0{}\0{}\0{}",
            payload.account_id,
            payload.tx_hash,
            payload.instruction_index,
            payload.activity_kind,
            token_fingerprint
        )
        .as_bytes(),
    )
}
fn retry_backoff(attempts: u32) -> Duration {
    let multiplier = 1u32.checked_shl(attempts.min(10)).unwrap_or(1 << 10);
    INITIAL_BACKOFF.saturating_mul(multiplier).min(MAX_BACKOFF)
}
fn duration_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(duration_ms)
        .unwrap_or_default()
}
fn fingerprint(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}
fn storage_error(error: io::Error) -> PushError {
    PushError::Storage(error.to_string())
}
fn storage_limit_error(label: &str, actual: usize, maximum: usize) -> PushError {
    PushError::Storage(format!(
        "{label} {actual} exceeds the push storage maximum {maximum}"
    ))
}
fn invalid_data(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}
fn write_json_atomic<T>(path: &Path, value: &T, maximum: usize) -> io::Result<()>
where
    T: norito::json::JsonSerialize + ?Sized,
{
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        let metadata = fs::symlink_metadata(parent)?;
        if push_metadata_is_symlink_or_reparse(&metadata) || !metadata.is_dir() {
            return Err(invalid_data("push output parent is not a direct directory"));
        }
    }
    let bytes = norito::json::to_vec(value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    if bytes.len() > maximum {
        return Err(invalid_data(format!(
            "push JSON has {} bytes, exceeding the maximum {maximum}",
            bytes.len()
        )));
    }
    let tmp = path.with_extension("json.tmp");
    write_direct_regular_file(&tmp, &bytes)?;
    fs::rename(tmp, path)
}
fn read_json<T>(path: &Path, maximum: usize) -> io::Result<T>
where
    T: norito::json::JsonDeserialize,
{
    let bytes = read_bounded_stable_file(path, maximum)?;
    norito::json::from_slice(&bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))
}
fn read_direct_directory(path: &Path) -> io::Result<fs::ReadDir> {
    let metadata = fs::symlink_metadata(path)?;
    if push_metadata_is_symlink_or_reparse(&metadata) || !metadata.is_dir() {
        return Err(invalid_data(
            "push persistence path is not a direct directory",
        ));
    }
    fs::read_dir(path)
}
fn read_bounded_stable_file(path: &Path, maximum: usize) -> io::Result<Vec<u8>> {
    read_bounded_stable_file_with(path, maximum, || Ok(()))
}
fn read_bounded_stable_file_with(
    path: &Path,
    maximum: usize,
    after_open: impl FnOnce() -> io::Result<()>,
) -> io::Result<Vec<u8>> {
    let named_before = fs::symlink_metadata(path)?;
    if push_metadata_is_symlink_or_reparse(&named_before) || !named_before.is_file() {
        return Err(invalid_data("push input must be a direct regular file"));
    }
    let maximum_u64 = u64::try_from(maximum).unwrap_or(u64::MAX);
    if named_before.len() > maximum_u64 {
        return Err(invalid_data(format!(
            "push input has {} bytes, exceeding the maximum {maximum}",
            named_before.len()
        )));
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    set_no_follow_open_options(&mut options);
    let mut file = options.open(path)?;
    let opened_before = file.metadata()?;
    if push_metadata_is_symlink_or_reparse(&opened_before) || !opened_before.is_file() {
        return Err(invalid_data(
            "opened push input is not a direct regular file",
        ));
    }
    if opened_before.len() > maximum_u64 {
        return Err(invalid_data(format!(
            "opened push input has {} bytes, exceeding the maximum {maximum}",
            opened_before.len()
        )));
    }
    if !push_file_metadata_unchanged(&named_before, &opened_before) {
        return Err(invalid_data("push input changed identity while opening"));
    }
    after_open()?;
    let capacity = usize::try_from(opened_before.len())
        .unwrap_or(maximum)
        .min(maximum);
    let mut bytes = Vec::with_capacity(capacity);
    std::io::Read::by_ref(&mut file)
        .take(maximum_u64.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() > maximum {
        return Err(invalid_data(format!(
            "push input exceeds the maximum {maximum} bytes"
        )));
    }
    let opened_after = file.metadata()?;
    let named_after = fs::symlink_metadata(path)?;
    if push_metadata_is_symlink_or_reparse(&named_after)
        || !named_after.is_file()
        || !push_file_metadata_unchanged(&opened_before, &opened_after)
        || !push_file_metadata_unchanged(&opened_after, &named_after)
        || opened_after.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
    {
        return Err(invalid_data("push input changed while reading"));
    }
    Ok(bytes)
}
fn write_direct_regular_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if push_metadata_is_symlink_or_reparse(&metadata) || !metadata.is_file() => {
            return Err(invalid_data(
                "push temporary output path is not a direct regular file",
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    set_no_follow_open_options(&mut options);
    let mut file = options.open(path)?;
    let opened_before = file.metadata()?;
    if push_metadata_is_symlink_or_reparse(&opened_before) || !opened_before.is_file() {
        return Err(invalid_data(
            "push temporary output is not a direct regular file",
        ));
    }
    file.write_all(bytes)?;
    let opened_after = file.metadata()?;
    let named_after = fs::symlink_metadata(path)?;
    let expected = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if push_metadata_is_symlink_or_reparse(&named_after)
        || !named_after.is_file()
        || !push_file_metadata_same_identity(&opened_before, &opened_after)
        || !push_file_metadata_same_identity(&opened_after, &named_after)
        || opened_after.len() != expected
        || named_after.len() != expected
    {
        return Err(invalid_data("push temporary output changed while writing"));
    }
    Ok(())
}
fn set_no_follow_open_options(options: &mut fs::OpenOptions) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(
            (rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::NONBLOCK
                | rustix::fs::OFlags::NOCTTY)
                .bits() as i32,
        );
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
}
fn push_metadata_is_symlink_or_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(windows))]
    false
}
#[cfg(unix)]
fn push_file_metadata_same_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev() && left.ino() == right.ino()
}
#[cfg(windows)]
fn push_file_metadata_same_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    left.volume_serial_number().is_some()
        && left.file_index().is_some()
        && left.volume_serial_number() == right.volume_serial_number()
        && left.file_index() == right.file_index()
}
#[cfg(not(any(unix, windows)))]
fn push_file_metadata_same_identity(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    false
}
#[cfg(unix)]
fn push_file_metadata_unchanged(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    push_file_metadata_same_identity(left, right)
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}
#[cfg(windows)]
fn push_file_metadata_unchanged(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    push_file_metadata_same_identity(left, right)
        && left.file_size() == right.file_size()
        && left.last_write_time() == right.last_write_time()
        && left.creation_time() == right.creation_time()
}
#[cfg(not(any(unix, windows)))]
fn push_file_metadata_unchanged(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len() && left.modified().ok() == right.modified().ok()
}
fn remove_file_if_exists(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
fn remove_device_file(path: &Path) -> io::Result<()> {
    remove_file_if_exists(path)?;
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    match fs::remove_dir(parent) {
        Ok(()) => Ok(()),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::DirectoryNotEmpty
            ) =>
        {
            Ok(())
        }
        Err(error) => {
            iroha_logger::warn!(?error, path = %parent.display(), "failed to remove empty push device account directory");
            Ok(())
        }
    }
}
fn committed_block_height(event: &EventBox) -> Option<u64> {
    match event {
        EventBox::Pipeline(PipelineEventBox::Block(block_event)) => match block_event.status() {
            BlockStatus::Committed | BlockStatus::Applied => {
                Some(block_event.header().height().get())
            }
            _ => None,
        },
        EventBox::PipelineBatch(events) => events.iter().find_map(|event| {
            let PipelineEventBox::Block(block_event) = event else {
                return None;
            };
            match block_event.status() {
                BlockStatus::Committed | BlockStatus::Applied => {
                    Some(block_event.header().height().get())
                }
                _ => None,
            }
        }),
        _ => None,
    }
}
fn external_signed_transaction_results(
    block: &SignedBlock,
) -> impl Iterator<
    Item = (
        HashOf<TransactionEntrypoint>,
        SignedTransaction,
        &TransactionResult,
    ),
> + '_ {
    let external_total = block.external_entrypoint_count();
    block
        .external_entrypoints_cloned()
        .take(external_total)
        .zip(block.results().take(external_total))
        .filter_map(|(entrypoint, result)| {
            let entrypoint_hash = entrypoint.hash();
            let signed = match entrypoint {
                TransactionEntrypoint::External(signed) => signed,
                TransactionEntrypoint::SealedReveal(reveal) => reveal.signed_transaction().clone(),
                TransactionEntrypoint::SealedCommitment(_) | TransactionEntrypoint::Time(_) => {
                    return None;
                }
            };
            Some((entrypoint_hash, signed, result))
        })
}
async fn send_fcm_http_v1(
    project_id: &str,
    service_account_path: &Path,
    delivery: &PushDelivery,
    settings: &DispatchSettings,
) -> DispatchOutcome {
    let client = match http_client(settings) {
        Ok(client) => client,
        Err(error) => return DispatchOutcome::Retry(error),
    };
    let token = match mint_fcm_access_token(&client, service_account_path, settings).await {
        Ok(token) => token,
        Err(error) => return DispatchOutcome::Retry(error),
    };
    let url = format!(
        "https://fcm.googleapis.com/v1/projects/{}/messages:send",
        project_id
    );
    let body = fcm_body(&delivery.token, &delivery.payload);
    let body = match norito::json::to_vec(&body) {
        Ok(body) => body,
        Err(error) => return DispatchOutcome::PermanentFailure(error.to_string()),
    };
    match client
        .post(url)
        .bearer_auth(token)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/json; charset=utf-8",
        )
        .body(body)
        .send()
        .await
    {
        Ok(response) => classify_fcm_response(response).await,
        Err(error) if error.is_timeout() => DispatchOutcome::Retry(error.to_string()),
        Err(error) => DispatchOutcome::Retry(error.to_string()),
    }
}
async fn send_apns_http2(
    endpoint: &str,
    topic: &str,
    team_id: &str,
    key_id: &str,
    private_key_path: &Path,
    delivery: &PushDelivery,
    settings: &DispatchSettings,
) -> DispatchOutcome {
    let client = match http_client(settings) {
        Ok(client) => client,
        Err(error) => return DispatchOutcome::Retry(error),
    };
    let token = match mint_apns_provider_token(team_id, key_id, private_key_path) {
        Ok(token) => token,
        Err(error) => return DispatchOutcome::Retry(error),
    };
    let url = format!(
        "{}/3/device/{}",
        endpoint.trim_end_matches('/'),
        delivery.token
    );
    let body = apns_body(&delivery.payload);
    let body = match norito::json::to_vec(&body) {
        Ok(body) => body,
        Err(error) => return DispatchOutcome::PermanentFailure(error.to_string()),
    };
    match client
        .post(url)
        .bearer_auth(token)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header("apns-topic", topic)
        .header("apns-push-type", "background")
        .header("apns-priority", "5")
        .body(body)
        .send()
        .await
    {
        Ok(response) => classify_apns_response(response).await,
        Err(error) if error.is_timeout() => DispatchOutcome::Retry(error.to_string()),
        Err(error) => DispatchOutcome::Retry(error.to_string()),
    }
}
fn http_client(settings: &DispatchSettings) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(settings.connect_timeout)
        .timeout(settings.request_timeout)
        .http2_adaptive_window(true)
        .build()
        .map_err(|error| error.to_string())
}
fn jwt_algorithm_name(algorithm: Algorithm) -> &'static str {
    match algorithm {
        Algorithm::HS256 => "HS256",
        Algorithm::HS384 => "HS384",
        Algorithm::HS512 => "HS512",
        Algorithm::ES256 => "ES256",
        Algorithm::ES384 => "ES384",
        Algorithm::RS256 => "RS256",
        Algorithm::RS384 => "RS384",
        Algorithm::RS512 => "RS512",
        Algorithm::PS256 => "PS256",
        Algorithm::PS384 => "PS384",
        Algorithm::PS512 => "PS512",
        Algorithm::EdDSA => "EdDSA",
    }
}
fn encode_jwt_claims(
    algorithm: Algorithm,
    key_id: Option<&str>,
    claims: norito::json::Value,
    key: &EncodingKey,
) -> Result<String, String> {
    let mut header = norito::json::Map::new();
    header.insert("typ".to_string(), norito::json::Value::from("JWT"));
    header.insert(
        "alg".to_string(),
        norito::json::Value::from(jwt_algorithm_name(algorithm)),
    );
    if let Some(key_id) = key_id {
        header.insert("kid".to_string(), norito::json::Value::from(key_id));
    }
    let encoded_header = BASE64_URL_SAFE_NO_PAD.encode(
        norito::json::to_vec(&norito::json::Value::Object(header))
            .map_err(|error| error.to_string())?,
    );
    let encoded_claims = BASE64_URL_SAFE_NO_PAD
        .encode(norito::json::to_vec(&claims).map_err(|error| error.to_string())?);
    let message = format!("{encoded_header}.{encoded_claims}");
    let signature = jsonwebtoken::crypto::sign(message.as_bytes(), key, algorithm)
        .map_err(|error| error.to_string())?;
    Ok(format!("{message}.{signature}"))
}
async fn read_push_provider_response_bounded(
    mut response: reqwest::Response,
    source: &'static str,
) -> Result<Vec<u8>, String> {
    let maximum = u64::try_from(MAX_PUSH_PROVIDER_RESPONSE_BYTES)
        .map_err(|_| "push provider response ceiling does not fit u64".to_string())?;
    let declared = response.content_length();
    if let Some(length) = declared
        && length > maximum
    {
        return Err(format!(
            "{source} declared a {length}-byte response, exceeding the {MAX_PUSH_PROVIDER_RESPONSE_BYTES}-byte ceiling"
        ));
    }
    let initial_capacity = declared
        .and_then(|length| usize::try_from(length).ok())
        .unwrap_or(0)
        .min(MAX_PUSH_PROVIDER_RESPONSE_BYTES);
    let mut body = Vec::new();
    body.try_reserve_exact(initial_capacity)
        .map_err(|error| format!("failed to reserve {source} response buffer: {error}"))?;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("failed to stream {source} response: {error}"))?
    {
        // The streamed count is authoritative for chunked responses and for
        // bodies that grow after transparent decompression. Stop at max + 1
        // before copying an oversized chunk into the retained buffer.
        let next_len = body
            .len()
            .checked_add(chunk.len())
            .ok_or_else(|| format!("{source} response length overflowed"))?;
        if next_len > MAX_PUSH_PROVIDER_RESPONSE_BYTES {
            return Err(format!(
                "{source} response exceeded the {MAX_PUSH_PROVIDER_RESPONSE_BYTES}-byte ceiling while streaming"
            ));
        }
        body.try_reserve_exact(chunk.len())
            .map_err(|error| format!("failed to grow {source} response buffer: {error}"))?;
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}
async fn mint_fcm_access_token(
    client: &reqwest::Client,
    service_account_path: &Path,
    settings: &DispatchSettings,
) -> Result<String, String> {
    let bytes = read_bounded_stable_file(service_account_path, MAX_FCM_SERVICE_ACCOUNT_BYTES)
        .map_err(|error| error.to_string())?;
    let value: norito::json::Value =
        norito::json::from_slice(&bytes).map_err(|error| error.to_string())?;
    let client_email = json_string_field(&value, "client_email")?;
    let private_key = json_string_field(&value, "private_key")?;
    let issued_at = now_ms() / 1000;
    let mut claims = norito::json::Map::new();
    claims.insert("iss".to_string(), norito::json::Value::from(client_email));
    claims.insert("scope".to_string(), norito::json::Value::from(FCM_SCOPE));
    claims.insert(
        "aud".to_string(),
        norito::json::Value::from(FCM_TOKEN_ENDPOINT),
    );
    claims.insert("iat".to_string(), norito::json::Value::from(issued_at));
    claims.insert(
        "exp".to_string(),
        norito::json::Value::from(issued_at.saturating_add(3600)),
    );
    let jwt = encode_jwt_claims(
        Algorithm::RS256,
        None,
        norito::json::Value::Object(claims),
        &EncodingKey::from_rsa_pem(private_key.as_bytes()).map_err(|error| error.to_string())?,
    )?;
    let response = client
        .post(FCM_TOKEN_ENDPOINT)
        .timeout(settings.request_timeout)
        .form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
            ("assertion", jwt.as_str()),
        ])
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();
    if !status.is_success() {
        // OAuth failure semantics depend only on the status. Avoid retaining
        // an attacker- or proxy-controlled diagnostic body.
        return Err(format!("FCM token endpoint returned {status}"));
    }
    let bytes = read_push_provider_response_bounded(response, "FCM token endpoint").await?;
    let value: norito::json::Value =
        norito::json::from_slice(&bytes).map_err(|error| error.to_string())?;
    json_string_field(&value, "access_token")
}
fn mint_apns_provider_token(
    team_id: &str,
    key_id: &str,
    private_key_path: &Path,
) -> Result<String, String> {
    let private_key = read_bounded_stable_file(private_key_path, MAX_APNS_PRIVATE_KEY_BYTES)
        .map_err(|error| error.to_string())?;
    let issued_at = now_ms() / 1000;
    let mut claims = norito::json::Map::new();
    claims.insert("iss".to_string(), norito::json::Value::from(team_id));
    claims.insert("iat".to_string(), norito::json::Value::from(issued_at));
    encode_jwt_claims(
        Algorithm::ES256,
        Some(key_id),
        norito::json::Value::Object(claims),
        &EncodingKey::from_ec_pem(&private_key).map_err(|error| error.to_string())?,
    )
}
fn json_string_field(value: &norito::json::Value, field: &str) -> Result<String, String> {
    value
        .as_object()
        .and_then(|object| object.get(field))
        .and_then(norito::json::Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("missing `{field}`"))
}
fn fcm_body(token: &str, payload: &PushActivityPayload) -> norito::json::Value {
    norito::json!({
        "message": {
            "token": token,
            "data": (push_data(payload)),
            "android": {
                "priority": "high"
            },
            "apns": {
                "headers": {
                    "apns-push-type": "background",
                    "apns-priority": "5"
                },
                "payload": {
                    "aps": {
                        "content-available": 1
                    }
                }
            }
        }
    })
}
fn apns_body(payload: &PushActivityPayload) -> norito::json::Value {
    norito::json!({
        "aps": {
            "content-available": 1
        },
        "iroha": (push_data(payload))
    })
}
fn push_data(payload: &PushActivityPayload) -> norito::json::Value {
    norito::json!({
        "account_id": (payload.account_id.clone()),
        "activity_kind": (payload.activity_kind.clone()),
        "tx_hash": (payload.tx_hash.clone()),
        "block_height": (payload.block_height.to_string()),
        "instruction_index": (payload.instruction_index.to_string()),
        "direction": (payload.direction.clone())
    })
}
async fn classify_fcm_response(response: reqwest::Response) -> DispatchOutcome {
    let status = response.status();
    if status.is_success() {
        return DispatchOutcome::Sent;
    }
    let body = read_push_provider_response_bounded(response, "FCM send endpoint")
        .await
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_else(|error| format!("response body unavailable: {error}"));
    classify_fcm_status_body(status, &body)
}
async fn classify_apns_response(response: reqwest::Response) -> DispatchOutcome {
    let status = response.status();
    if status.is_success() {
        return DispatchOutcome::Sent;
    }
    let body = read_push_provider_response_bounded(response, "APNs send endpoint")
        .await
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_else(|error| format!("response body unavailable: {error}"));
    classify_apns_status_body(status, &body)
}
fn classify_fcm_status_body(status: HttpStatusCode, body: &str) -> DispatchOutcome {
    if status.is_success() {
        return DispatchOutcome::Sent;
    }
    if status == HttpStatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
        return DispatchOutcome::Retry(format!("FCM returned {status}: {body}"));
    }
    if body.contains("UNREGISTERED")
        || body.contains("INVALID_ARGUMENT")
        || body.contains("registration token")
    {
        return DispatchOutcome::InvalidToken(format!("FCM returned {status}: {body}"));
    }
    DispatchOutcome::PermanentFailure(format!("FCM returned {status}: {body}"))
}
fn classify_apns_status_body(status: HttpStatusCode, body: &str) -> DispatchOutcome {
    if status.is_success() {
        return DispatchOutcome::Sent;
    }
    if status == HttpStatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
        return DispatchOutcome::Retry(format!("APNs returned {status}: {body}"));
    }
    if status == HttpStatusCode::GONE
        || body.contains("Unregistered")
        || body.contains("BadDeviceToken")
        || body.contains("DeviceTokenNotForTopic")
    {
        return DispatchOutcome::InvalidToken(format!("APNs returned {status}: {body}"));
    }
    DispatchOutcome::PermanentFailure(format!("APNs returned {status}: {body}"))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_dir::OverrideGuard;
    use base64::Engine as _;
    use tokio::{
        io::{AsyncReadExt as _, AsyncWriteExt as _},
        net::TcpListener,
    };
    const TEST_ACCOUNT_I105: &str = "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE";
    async fn raw_provider_response(response: Vec<u8>) -> reqwest::Response {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind push provider response listener");
        let address = listener
            .local_addr()
            .expect("push provider response address");
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept provider request");
            let mut request = [0_u8; 1024];
            let _ = socket.read(&mut request).await;
            socket
                .write_all(&response)
                .await
                .expect("write provider response");
        });
        reqwest::Client::new()
            .get(format!("http://{address}/push-provider"))
            .send()
            .await
            .expect("receive provider response headers")
    }
    fn chunked_provider_response(status: &str, body_len: usize) -> Vec<u8> {
        let mut response = format!(
            "HTTP/1.1 {status}\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{body_len:X}\r\n"
        )
        .into_bytes();
        response.resize(response.len() + body_len, b'x');
        response.extend_from_slice(b"\r\n0\r\n\r\n");
        response
    }
    fn test_bridge_config() -> actual::Push {
        actual::Push {
            enabled: true,
            fcm_project_id: Some("project".to_string()),
            fcm_service_account_path: Some(PathBuf::from("/tmp/service-account.json")),
            ..Default::default()
        }
    }
    fn test_storage_limits(max_devices: usize, max_queue_jobs: usize) -> PushStorageLimits {
        PushStorageLimits {
            max_devices,
            max_queue_jobs,
            max_device_account_entries: max_devices,
            dispatch_batch_size: max_queue_jobs,
            ..PUSH_STORAGE_LIMITS
        }
    }
    fn register_request(token: &str) -> RegisterDeviceRequest {
        RegisterDeviceRequest {
            account_id: TEST_ACCOUNT_I105.to_string(),
            platform: "FCM".to_string(),
            token: token.to_string(),
            topics: None,
        }
    }
    fn test_job(index: usize) -> DeliveryJob {
        let token_fingerprint = fingerprint(format!("token-{index}").as_bytes());
        let payload = PushActivityPayload {
            account_id: TEST_ACCOUNT_I105.to_string(),
            activity_kind: "Transfer".to_string(),
            tx_hash: format!("tx-{index}"),
            block_height: 7,
            instruction_index: u64::try_from(index).expect("test index fits u64"),
            direction: "incoming".to_string(),
        };
        DeliveryJob {
            dedupe_key: delivery_dedupe_key(&payload, &token_fingerprint),
            account_id: TEST_ACCOUNT_I105.to_string(),
            token_fingerprint,
            payload,
            attempts: 0,
            next_attempt_ms: 0,
            created_at_ms: 0,
            updated_at_ms: 0,
        }
    }
    #[test]
    fn encode_jwt_claims_builds_verifiable_compact_token() {
        let mut claims = norito::json::Map::new();
        claims.insert("iss".to_string(), norito::json::Value::from("issuer"));
        claims.insert(
            "iat".to_string(),
            norito::json::Value::from(1_700_000_000_u64),
        );
        let key = EncodingKey::from_secret(b"shared-secret");
        let token = encode_jwt_claims(
            Algorithm::HS256,
            Some("test-key"),
            norito::json::Value::Object(claims),
            &key,
        )
        .expect("JWT should encode");
        let parts: Vec<_> = token.split('.').collect();
        assert_eq!(parts.len(), 3);
        let header: norito::json::Value = norito::json::from_slice(
            &BASE64_URL_SAFE_NO_PAD
                .decode(parts[0])
                .expect("header should be base64url"),
        )
        .expect("header should decode");
        assert_eq!(header["typ"].as_str(), Some("JWT"));
        assert_eq!(header["alg"].as_str(), Some("HS256"));
        assert_eq!(header["kid"].as_str(), Some("test-key"));
        let payload: norito::json::Value = norito::json::from_slice(
            &BASE64_URL_SAFE_NO_PAD
                .decode(parts[1])
                .expect("payload should be base64url"),
        )
        .expect("payload should decode");
        assert_eq!(payload["iss"].as_str(), Some("issuer"));
        assert_eq!(payload["iat"].as_u64(), Some(1_700_000_000));
        assert!(
            jsonwebtoken::crypto::verify(
                parts[2],
                format!("{}.{}", parts[0], parts[1]).as_bytes(),
                &jsonwebtoken::DecodingKey::from_secret(b"shared-secret"),
                Algorithm::HS256,
            )
            .expect("signature should verify")
        );
    }
    #[test]
    fn bounded_push_file_read_accepts_exact_limit_and_rejects_limit_plus_one() {
        let directory = tempfile::tempdir().expect("push read directory");
        let path = directory.path().join("input.json");
        fs::write(&path, b"12345678").expect("write bounded input");
        assert_eq!(
            read_bounded_stable_file(&path, 8).expect("exact byte limit"),
            b"12345678"
        );
        let error = read_bounded_stable_file(&path, 7)
            .expect_err("one byte beyond the limit must fail before allocation");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }
    #[tokio::test]
    async fn provider_response_reader_accepts_exact_limit_and_rejects_max_plus_one() {
        let declared = raw_provider_response(
            format!(
                "HTTP/1.1 400 Bad Request\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                MAX_PUSH_PROVIDER_RESPONSE_BYTES + 1
            )
            .into_bytes(),
        )
        .await;
        let declared_error = read_push_provider_response_bounded(declared, "test provider")
            .await
            .expect_err("oversized Content-Length must fail before body allocation");
        assert!(declared_error.contains("declared"));
        let exact = raw_provider_response(chunked_provider_response(
            "400 Bad Request",
            MAX_PUSH_PROVIDER_RESPONSE_BYTES,
        ))
        .await;
        assert_eq!(
            read_push_provider_response_bounded(exact, "test provider")
                .await
                .expect("the exact provider response ceiling is accepted")
                .len(),
            MAX_PUSH_PROVIDER_RESPONSE_BYTES
        );
        let streamed = raw_provider_response(chunked_provider_response(
            "400 Bad Request",
            MAX_PUSH_PROVIDER_RESPONSE_BYTES + 1,
        ))
        .await;
        let streamed_error = read_push_provider_response_bounded(streamed, "test provider")
            .await
            .expect_err("chunked max plus one must fail while streaming");
        assert!(streamed_error.contains("while streaming"));
    }
    #[tokio::test]
    async fn successful_provider_responses_skip_oversized_bodies() {
        let oversized_success = || {
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                MAX_PUSH_PROVIDER_RESPONSE_BYTES + 1
            )
            .into_bytes()
        };
        assert_eq!(
            classify_fcm_response(raw_provider_response(oversized_success()).await).await,
            DispatchOutcome::Sent
        );
        assert_eq!(
            classify_apns_response(raw_provider_response(oversized_success()).await).await,
            DispatchOutcome::Sent
        );
    }
    #[test]
    fn bounded_json_write_rejects_oversize_before_creating_temp_file() {
        let directory = tempfile::tempdir().expect("push write directory");
        let path = directory.path().join("record.json");
        let error = write_json_atomic(&path, &"x".repeat(32), 8)
            .expect_err("oversized JSON must not reach the filesystem");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(!path.exists());
        assert!(!path.with_extension("json.tmp").exists());
    }
    #[cfg(unix)]
    #[test]
    fn bounded_push_file_read_rejects_path_replacement_race() {
        let directory = tempfile::tempdir().expect("push race directory");
        let path = directory.path().join("input.json");
        let replacement = directory.path().join("replacement.json");
        fs::write(&path, b"old").expect("write original input");
        fs::write(&replacement, b"replacement").expect("write replacement input");
        let error = read_bounded_stable_file_with(&path, 32, || fs::rename(&replacement, &path))
            .expect_err("path replacement after open must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }
    #[cfg(unix)]
    #[test]
    fn bounded_push_file_read_rejects_symlink() {
        use std::os::unix::fs::symlink;
        let directory = tempfile::tempdir().expect("push symlink directory");
        let target = directory.path().join("target.json");
        let link = directory.path().join("link.json");
        fs::write(&target, b"{}").expect("write symlink target");
        symlink(&target, &link).expect("create push symlink");
        assert!(read_bounded_stable_file(&link, 32).is_err());
    }
    #[test]
    fn write_side_device_capacity_is_hard() {
        let temp = tempfile::tempdir().expect("tempdir");
        let _guard = OverrideGuard::new(temp.path());
        let bridge = PushBridge::with_dispatcher_and_limits(
            test_bridge_config(),
            Arc::new(RealPushDispatcher),
            test_storage_limits(1, 1),
        );
        bridge
            .register_device(register_request("token-1"))
            .expect("first device fits");
        let error = bridge
            .register_device(register_request("token-2"))
            .expect_err("second device must cross the hard capacity");
        assert!(matches!(error, PushError::Storage(_)));
        assert_eq!(bridge.device_count(), 1);
    }
    #[test]
    fn startup_queue_enumeration_stops_at_hard_candidate_count() {
        let temp = tempfile::tempdir().expect("tempdir");
        let _guard = OverrideGuard::new(temp.path());
        let queue_root = temp.path().join(PUSH_DIR).join(QUEUE_DIR);
        for index in 0..3 {
            let job = test_job(index);
            write_json_atomic(
                &queue_root.join(format!("{}.json", job.dedupe_key)),
                &job,
                MAX_PUSH_QUEUE_JOB_BYTES,
            )
            .expect("write queue fixture");
        }
        let bridge = PushBridge::with_dispatcher_and_limits(
            test_bridge_config(),
            Arc::new(RealPushDispatcher),
            test_storage_limits(1, 2),
        );
        assert_eq!(bridge.queued_count(), 2);
    }
    #[tokio::test]
    async fn due_queue_dispatch_collects_only_one_bounded_batch() {
        let temp = tempfile::tempdir().expect("tempdir");
        let _guard = OverrideGuard::new(temp.path());
        let mut limits = test_storage_limits(1, 2);
        limits.dispatch_batch_size = 1;
        let bridge = PushBridge::with_dispatcher_and_limits(
            test_bridge_config(),
            Arc::new(RealPushDispatcher),
            limits,
        );
        for index in 0..2 {
            let job = test_job(index);
            bridge.queue.insert(job.dedupe_key.clone(), job);
        }
        bridge.dispatch_due_once().await;
        assert_eq!(
            bridge.queued_count(),
            1,
            "one dispatch pass must consume at most one configured batch"
        );
    }
    #[test]
    fn write_side_queue_capacity_is_hard() {
        let temp = tempfile::tempdir().expect("tempdir");
        let _guard = OverrideGuard::new(temp.path());
        let bridge = PushBridge::with_dispatcher_and_limits(
            test_bridge_config(),
            Arc::new(RealPushDispatcher),
            test_storage_limits(2, 1),
        );
        bridge
            .register_device(register_request("token-1"))
            .expect("first device");
        bridge
            .register_device(register_request("token-2"))
            .expect("second device");
        let account = AccountId::parse_encoded(TEST_ACCOUNT_I105)
            .expect("account")
            .into_account_id();
        let error = bridge
            .enqueue_activity(
                &account,
                "tx-hash",
                7,
                0,
                "Transfer",
                AccountActivityRole::Incoming,
            )
            .expect_err("second fan-out job must cross the hard queue capacity");
        assert!(matches!(error, PushError::Storage(_)));
        assert_eq!(bridge.queued_count(), 1);
    }
    #[test]
    fn disabled_rejected() {
        let bridge = PushBridge::new(actual::Push {
            enabled: false,
            ..Default::default()
        });
        let err = bridge
            .register_device(RegisterDeviceRequest {
                account_id: TEST_ACCOUNT_I105.to_string(),
                platform: "FCM".to_string(),
                token: "t0".to_string(),
                topics: None,
            })
            .expect_err("push disabled");
        matches!(err, PushError::Disabled);
    }
    #[test]
    fn missing_credentials_rejected() {
        let bridge = PushBridge::new(actual::Push {
            enabled: true,
            fcm_project_id: Some("project".to_string()),
            fcm_service_account_path: None,
            ..Default::default()
        });
        let err = bridge
            .register_device(RegisterDeviceRequest {
                account_id: TEST_ACCOUNT_I105.to_string(),
                platform: "FCM".to_string(),
                token: "t1".to_string(),
                topics: None,
            })
            .expect_err("missing creds");
        assert!(matches!(err, PushError::MissingCredentials { .. }));
    }
    #[test]
    fn too_many_topics_rejected() {
        let bridge = PushBridge::new(actual::Push {
            max_topics_per_device: nonzero!(2_usize),
            ..test_bridge_config()
        });
        let err = bridge
            .register_device(RegisterDeviceRequest {
                account_id: TEST_ACCOUNT_I105.to_string(),
                platform: "FCM".to_string(),
                token: "t2".to_string(),
                topics: Some(vec!["a".into(), "b".into(), "c".into()]),
            })
            .expect_err("too many topics");
        assert!(matches!(err, PushError::TooManyTopics { max: 2 }));
    }
    #[test]
    fn fcm_response_classification_covers_provider_outcomes() {
        assert_eq!(
            classify_fcm_status_body(HttpStatusCode::OK, ""),
            DispatchOutcome::Sent
        );
        assert!(matches!(
            classify_fcm_status_body(HttpStatusCode::TOO_MANY_REQUESTS, "quota"),
            DispatchOutcome::Retry(_)
        ));
        assert!(matches!(
            classify_fcm_status_body(HttpStatusCode::INTERNAL_SERVER_ERROR, "server"),
            DispatchOutcome::Retry(_)
        ));
        assert!(matches!(
            classify_fcm_status_body(HttpStatusCode::BAD_REQUEST, "UNREGISTERED"),
            DispatchOutcome::InvalidToken(_)
        ));
        assert!(matches!(
            classify_fcm_status_body(HttpStatusCode::UNAUTHORIZED, "bad credentials"),
            DispatchOutcome::PermanentFailure(_)
        ));
    }
    #[test]
    fn apns_response_classification_covers_provider_outcomes() {
        assert_eq!(
            classify_apns_status_body(HttpStatusCode::OK, ""),
            DispatchOutcome::Sent
        );
        assert!(matches!(
            classify_apns_status_body(HttpStatusCode::TOO_MANY_REQUESTS, "quota"),
            DispatchOutcome::Retry(_)
        ));
        assert!(matches!(
            classify_apns_status_body(HttpStatusCode::BAD_GATEWAY, "server"),
            DispatchOutcome::Retry(_)
        ));
        assert!(matches!(
            classify_apns_status_body(HttpStatusCode::GONE, "Unregistered"),
            DispatchOutcome::InvalidToken(_)
        ));
        assert!(matches!(
            classify_apns_status_body(HttpStatusCode::BAD_REQUEST, "BadDeviceToken"),
            DispatchOutcome::InvalidToken(_)
        ));
        assert!(matches!(
            classify_apns_status_body(HttpStatusCode::FORBIDDEN, "ExpiredProviderToken"),
            DispatchOutcome::PermanentFailure(_)
        ));
    }
    #[test]
    fn stores_device_on_success_and_loads_from_disk() {
        let temp = tempfile::tempdir().expect("tempdir");
        let _guard = OverrideGuard::new(temp.path());
        let bridge = PushBridge::new(test_bridge_config());
        bridge
            .register_device(RegisterDeviceRequest {
                account_id: TEST_ACCOUNT_I105.to_string(),
                platform: "FCM".to_string(),
                token: "token-123".to_string(),
                topics: Some(vec!["incoming".into()]),
            })
            .expect("should store device");
        assert_eq!(bridge.device_count(), 1);
        let loaded = PushBridge::new(test_bridge_config());
        assert_eq!(loaded.device_count(), 1);
        let device = loaded
            .registered_device("token-123")
            .expect("device loaded by fingerprint");
        assert_eq!(device.account_id, TEST_ACCOUNT_I105);
    }
    #[test]
    fn unregister_removes_device() {
        let temp = tempfile::tempdir().expect("tempdir");
        let _guard = OverrideGuard::new(temp.path());
        let bridge = PushBridge::new(test_bridge_config());
        let request = RegisterDeviceRequest {
            account_id: TEST_ACCOUNT_I105.to_string(),
            platform: "FCM".to_string(),
            token: "token-123".to_string(),
            topics: None,
        };
        bridge
            .register_device(request.clone())
            .expect("register device");
        bridge
            .unregister_device(request)
            .expect("unregister device");
        assert_eq!(bridge.device_count(), 0);
        assert!(bridge.registered_device("token-123").is_none());
    }
    #[test]
    fn alias_account_id_is_rejected() {
        let bridge = PushBridge::new(test_bridge_config());
        let err = bridge
            .register_device(RegisterDeviceRequest {
                account_id: "alice@wallets".to_string(),
                platform: "FCM".to_string(),
                token: "token-123".to_string(),
                topics: None,
            })
            .expect_err("aliases must be rejected");
        assert!(matches!(err, PushError::InvalidAccount(account) if account == "alice@wallets"));
    }
    #[tokio::test]
    async fn queue_dedupes_and_dispatches_success() {
        let temp = tempfile::tempdir().expect("tempdir");
        let _guard = OverrideGuard::new(temp.path());
        let dispatcher = Arc::new(MockDispatcher::new(vec![DispatchOutcome::Sent]));
        let bridge = PushBridge::with_dispatcher(test_bridge_config(), dispatcher.clone());
        bridge
            .register_device(RegisterDeviceRequest {
                account_id: TEST_ACCOUNT_I105.to_string(),
                platform: "FCM".to_string(),
                token: "token-1".to_string(),
                topics: None,
            })
            .expect("register");
        let account = AccountId::parse_encoded(TEST_ACCOUNT_I105)
            .expect("account")
            .into_account_id();
        bridge
            .enqueue_activity(
                &account,
                "tx-hash",
                7,
                0,
                "Transfer",
                AccountActivityRole::Incoming,
            )
            .expect("enqueue");
        bridge
            .enqueue_activity(
                &account,
                "tx-hash",
                7,
                0,
                "Transfer",
                AccountActivityRole::Incoming,
            )
            .expect("dedupe");
        assert_eq!(bridge.queued_count(), 1);
        bridge.dispatch_due_once().await;
        assert_eq!(bridge.queued_count(), 0);
        assert_eq!(dispatcher.calls.lock().unwrap().len(), 1);
    }
    #[tokio::test]
    async fn retryable_failure_reschedules_job() {
        let temp = tempfile::tempdir().expect("tempdir");
        let _guard = OverrideGuard::new(temp.path());
        let dispatcher = Arc::new(MockDispatcher::new(vec![DispatchOutcome::Retry(
            "timeout".to_string(),
        )]));
        let bridge = PushBridge::with_dispatcher(test_bridge_config(), dispatcher);
        bridge
            .register_device(RegisterDeviceRequest {
                account_id: TEST_ACCOUNT_I105.to_string(),
                platform: "FCM".to_string(),
                token: "token-2".to_string(),
                topics: None,
            })
            .expect("register");
        let account = AccountId::parse_encoded(TEST_ACCOUNT_I105)
            .expect("account")
            .into_account_id();
        bridge
            .enqueue_activity(
                &account,
                "tx-hash",
                7,
                0,
                "Transfer",
                AccountActivityRole::Incoming,
            )
            .expect("enqueue");
        bridge.dispatch_due_once().await;
        assert_eq!(bridge.queued_count(), 1);
        let job = bridge.queue.iter().next().expect("job remains");
        assert_eq!(job.attempts, 1);
        assert!(job.next_attempt_ms > now_ms());
    }
    #[tokio::test]
    async fn invalid_token_removes_device_and_job() {
        let temp = tempfile::tempdir().expect("tempdir");
        let _guard = OverrideGuard::new(temp.path());
        let dispatcher = Arc::new(MockDispatcher::new(vec![DispatchOutcome::InvalidToken(
            "unregistered".to_string(),
        )]));
        let bridge = PushBridge::with_dispatcher(test_bridge_config(), dispatcher);
        bridge
            .register_device(RegisterDeviceRequest {
                account_id: TEST_ACCOUNT_I105.to_string(),
                platform: "FCM".to_string(),
                token: "token-3".to_string(),
                topics: None,
            })
            .expect("register");
        let account = AccountId::parse_encoded(TEST_ACCOUNT_I105)
            .expect("account")
            .into_account_id();
        bridge
            .enqueue_activity(
                &account,
                "tx-hash",
                7,
                0,
                "Transfer",
                AccountActivityRole::Incoming,
            )
            .expect("enqueue");
        bridge.dispatch_due_once().await;
        assert_eq!(bridge.queued_count(), 0);
        assert_eq!(bridge.device_count(), 0);
    }
    struct MockDispatcher {
        outcomes: Mutex<Vec<DispatchOutcome>>,
        calls: Arc<Mutex<Vec<PushDelivery>>>,
    }
    impl MockDispatcher {
        fn new(outcomes: Vec<DispatchOutcome>) -> Self {
            Self {
                outcomes: Mutex::new(outcomes),
                calls: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }
    #[async_trait]
    impl PushDispatcher for MockDispatcher {
        async fn send(
            &self,
            delivery: &PushDelivery,
            _settings: &DispatchSettings,
            _credentials: &ProviderCredentials,
        ) -> DispatchOutcome {
            self.calls.lock().unwrap().push(delivery.clone());
            self.outcomes
                .lock()
                .unwrap()
                .pop()
                .unwrap_or(DispatchOutcome::Sent)
        }
    }
}
