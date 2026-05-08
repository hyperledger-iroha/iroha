//! Best-effort Torii push notification bridge.

#[cfg(test)]
use std::sync::Mutex;
use std::{
    fs, io,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use dashmap::{DashMap, mapref::entry::Entry};
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
    transaction::{
        executable::Executable,
        signed::{SignedTransaction, TransactionEntrypoint, TransactionResult},
    },
};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
#[cfg(test)]
use nonzero_ext::nonzero;
use reqwest::StatusCode as HttpStatusCode;
use sha2::{Digest as _, Sha256};

use crate::account_activity::AccountActivityRole;

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
}

impl PushBridge {
    pub fn new(config: actual::Push) -> Self {
        Self::with_dispatcher(config, Arc::new(RealPushDispatcher))
    }

    pub fn with_dispatcher(config: actual::Push, dispatcher: Arc<dyn PushDispatcher>) -> Self {
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
        let token = request.token.trim().to_owned();
        if token.is_empty() {
            return Err(PushError::EmptyToken);
        }
        let topics = normalize_topics(request.topics);
        let max_topics = self.config.max_topics_per_device.get();
        if topics.len() > max_topics {
            return Err(PushError::TooManyTopics { max: max_topics });
        }

        let token_fingerprint = fingerprint(token.as_bytes());
        let record = DeviceRecord {
            account_id: account_id.to_string(),
            platform: platform.label().to_string(),
            token,
            token_fingerprint: token_fingerprint.clone(),
            topics,
            updated_at_ms: now_ms(),
        };

        if let Some(old) = self.devices.get(&token_fingerprint) {
            let old_path = self.device_path(&old.account_id, &token_fingerprint);
            remove_file_if_exists(&old_path).map_err(storage_error)?;
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
            match self.queue.entry(dedupe_key) {
                Entry::Occupied(_) => {}
                Entry::Vacant(entry) => {
                    self.persist_job(&job)?;
                    entry.insert(job);
                    queued += 1;
                }
            }
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
            let Executable::Instructions(instructions) = tx.instructions() else {
                continue;
            };
            let tx_hash = entrypoint_hash.to_string();
            for (instruction_index, instruction) in instructions.iter().enumerate() {
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
            .collect()
    }

    fn remove_device_by_fingerprint(&self, token_fingerprint: &str) -> Result<(), PushError> {
        let Some((_, record)) = self.devices.remove(token_fingerprint) else {
            return Ok(());
        };
        remove_file_if_exists(&self.device_path(&record.account_id, token_fingerprint))
            .map_err(storage_error)?;
        Ok(())
    }

    fn remove_job(&self, dedupe_key: &str) {
        self.queue.remove(dedupe_key);
        if let Err(error) = remove_file_if_exists(&self.job_path(dedupe_key)) {
            iroha_logger::warn!(?error, "failed to remove push queue file");
        }
    }

    fn persist_device(&self, record: &DeviceRecord) -> Result<(), PushError> {
        write_json_atomic(
            &self.device_path(&record.account_id, &record.token_fingerprint),
            record,
        )
        .map_err(storage_error)
    }

    fn persist_job(&self, job: &DeliveryJob) -> Result<(), PushError> {
        write_json_atomic(&self.job_path(&job.dedupe_key), job).map_err(storage_error)
    }

    fn load_from_disk(&self) {
        self.load_devices();
        self.load_queue();
    }

    fn load_devices(&self) {
        let root = self.data_dir.join(DEVICES_DIR);
        let Ok(accounts) = fs::read_dir(&root) else {
            return;
        };
        for account_dir in accounts.flatten().filter(|entry| entry.path().is_dir()) {
            let Ok(files) = fs::read_dir(account_dir.path()) else {
                continue;
            };
            for file in files.flatten().filter(|entry| entry.path().is_file()) {
                match read_json::<DeviceRecord>(&file.path()) {
                    Ok(record) if Platform::from_stored(&record.platform).is_some() => {
                        self.devices
                            .insert(record.token_fingerprint.clone(), record);
                    }
                    Ok(_) => {}
                    Err(error) => {
                        iroha_logger::warn!(?error, path = %file.path().display(), "failed to load push device")
                    }
                }
            }
        }
    }

    fn load_queue(&self) {
        let root = self.data_dir.join(QUEUE_DIR);
        let Ok(files) = fs::read_dir(&root) else {
            return;
        };
        for file in files.flatten().filter(|entry| entry.path().is_file()) {
            match read_json::<DeliveryJob>(&file.path()) {
                Ok(job) => {
                    self.queue.insert(job.dedupe_key.clone(), job);
                }
                Err(error) => {
                    iroha_logger::warn!(?error, path = %file.path().display(), "failed to load push queue job")
                }
            }
        }
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

fn normalize_topics(topics: Option<Vec<String>>) -> Vec<String> {
    let mut out = Vec::new();
    for topic in topics.unwrap_or_default() {
        let topic = topic.trim();
        if !topic.is_empty() && !out.iter().any(|seen| seen == topic) {
            out.push(topic.to_owned());
        }
    }
    out
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

fn write_json_atomic<T>(path: &Path, value: &T) -> io::Result<()>
where
    T: norito::json::JsonSerialize + ?Sized,
{
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = norito::json::to_vec(value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(tmp, path)
}

fn read_json<T>(path: &Path) -> io::Result<T>
where
    T: norito::json::JsonDeserialize,
{
    let bytes = fs::read(path)?;
    norito::json::from_slice(&bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))
}

fn remove_file_if_exists(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
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
                TransactionEntrypoint::SealedCommitment(_)
                | TransactionEntrypoint::PrivateKaigi(_)
                | TransactionEntrypoint::Time(_) => return None,
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

#[derive(serde::Serialize)]
struct FcmClaims<'a> {
    iss: &'a str,
    scope: &'static str,
    aud: &'static str,
    iat: u64,
    exp: u64,
}

#[derive(serde::Serialize)]
struct ApnsClaims<'a> {
    iss: &'a str,
    iat: u64,
}

async fn mint_fcm_access_token(
    client: &reqwest::Client,
    service_account_path: &Path,
    settings: &DispatchSettings,
) -> Result<String, String> {
    let bytes = fs::read(service_account_path).map_err(|error| error.to_string())?;
    let value: norito::json::Value =
        norito::json::from_slice(&bytes).map_err(|error| error.to_string())?;
    let client_email = json_string_field(&value, "client_email")?;
    let private_key = json_string_field(&value, "private_key")?;
    let issued_at = now_ms() / 1000;
    let claims = FcmClaims {
        iss: &client_email,
        scope: FCM_SCOPE,
        aud: FCM_TOKEN_ENDPOINT,
        iat: issued_at,
        exp: issued_at.saturating_add(3600),
    };
    let jwt = jsonwebtoken::encode(
        &Header::new(Algorithm::RS256),
        &claims,
        &EncodingKey::from_rsa_pem(private_key.as_bytes()).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
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
    let bytes = response.bytes().await.map_err(|error| error.to_string())?;
    if !status.is_success() {
        return Err(format!(
            "FCM token endpoint returned {status}: {}",
            String::from_utf8_lossy(&bytes)
        ));
    }
    let value: norito::json::Value =
        norito::json::from_slice(&bytes).map_err(|error| error.to_string())?;
    json_string_field(&value, "access_token")
}

fn mint_apns_provider_token(
    team_id: &str,
    key_id: &str,
    private_key_path: &Path,
) -> Result<String, String> {
    let private_key = fs::read(private_key_path).map_err(|error| error.to_string())?;
    let mut header = Header::new(Algorithm::ES256);
    header.kid = Some(key_id.to_owned());
    let issued_at = now_ms() / 1000;
    let claims = ApnsClaims {
        iss: team_id,
        iat: issued_at,
    };
    jsonwebtoken::encode(
        &header,
        &claims,
        &EncodingKey::from_ec_pem(&private_key).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
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
    let body = response
        .bytes()
        .await
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default();
    classify_fcm_status_body(status, &body)
}

async fn classify_apns_response(response: reqwest::Response) -> DispatchOutcome {
    let status = response.status();
    let body = response
        .bytes()
        .await
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default();
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

    const TEST_ACCOUNT_I105: &str = "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE";

    fn test_bridge_config() -> actual::Push {
        actual::Push {
            enabled: true,
            fcm_project_id: Some("project".to_string()),
            fcm_service_account_path: Some(PathBuf::from("/tmp/service-account.json")),
            ..Default::default()
        }
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
