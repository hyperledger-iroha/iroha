//! Fault-injection utilities used by Izanami to emulate Byzantine peers.

use std::{
    io::{self, Write},
    ops::RangeInclusive,
    path::PathBuf,
    pin::Pin,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

use color_eyre::{
    Result,
    eyre::{WrapErr, eyre},
};
use iroha_config::base::toml::WriteExt;
use iroha_data_model::prelude::*;
use iroha_genesis::GenesisBlock;
use iroha_test_network::NetworkPeer;
use iroha_test_samples::ALICE_ID;
use rand::{Rng, RngCore, SeedableRng, rngs::StdRng, seq::IndexedRandom};
use tokio::{sync::Notify, task, time::sleep};
use toml::Table;
use tracing::{debug, error, info, warn};

/// Configuration for periodic fault injection.
#[derive(Clone, Debug)]
pub struct FaultConfig {
    pub interval: RangeInclusive<Duration>,
    pub network_latency: Option<NetworkLatencyConfig>,
    pub network_partition: Option<NetworkPartitionConfig>,
    pub cpu_stress: Option<CpuStressConfig>,
    pub disk_saturation: Option<DiskSaturationConfig>,
}

#[derive(Clone, Debug)]
pub struct NetworkLatencyConfig {
    pub duration: RangeInclusive<Duration>,
    pub gossip_delay: RangeInclusive<Duration>,
}

impl Default for NetworkLatencyConfig {
    fn default() -> Self {
        Self {
            duration: Duration::from_secs(6)..=Duration::from_secs(12),
            gossip_delay: Duration::from_millis(750)..=Duration::from_millis(2_500),
        }
    }
}

#[derive(Clone, Debug)]
pub struct NetworkPartitionConfig {
    pub duration: RangeInclusive<Duration>,
}

impl Default for NetworkPartitionConfig {
    fn default() -> Self {
        Self {
            duration: Duration::from_secs(5)..=Duration::from_secs(10),
        }
    }
}

#[derive(Clone, Debug)]
pub struct CpuStressConfig {
    pub duration: RangeInclusive<Duration>,
    pub workers: RangeInclusive<usize>,
}

impl Default for CpuStressConfig {
    fn default() -> Self {
        Self {
            duration: Duration::from_secs(4)..=Duration::from_secs(8),
            workers: 1..=2,
        }
    }
}

#[derive(Clone, Debug)]
pub struct DiskSaturationConfig {
    pub duration: RangeInclusive<Duration>,
    pub bytes: RangeInclusive<u64>,
}

impl Default for DiskSaturationConfig {
    fn default() -> Self {
        const MI: u64 = 1_048_576;
        Self {
            duration: Duration::from_secs(8)..=Duration::from_secs(12),
            bytes: 4 * MI..=8 * MI,
        }
    }
}

struct TableRef<'a>(&'a Table);

impl AsRef<Table> for TableRef<'_> {
    fn as_ref(&self) -> &Table {
        self.0
    }
}

impl FaultConfig {
    pub fn sample_interval<R: Rng>(&self, rng: &mut R) -> Duration {
        let start = *self.interval.start();
        let end = *self.interval.end();
        if start == end {
            return start;
        }
        let delta = end.saturating_sub(start);
        let upper = u64::try_from(delta.as_millis()).unwrap_or(u64::MAX);
        start + Duration::from_millis(rng.random_range(0..=upper))
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn run_fault_loop<P: FaultPeer>(
    peer: P,
    config: FaultConfig,
    genesis: Arc<GenesisBlock>,
    config_layers: Arc<Vec<Table>>,
    base_domain: DomainId,
    stop: Arc<AtomicBool>,
    stop_notify: Arc<Notify>,
    deadline: Instant,
    seed: u64,
) {
    let mut rng = StdRng::seed_from_u64(seed);
    while Instant::now() < deadline && !stop.load(std::sync::atomic::Ordering::Relaxed) {
        let scenario = FaultScenario::random(&mut rng, &config);
        let ctx = FaultApplyCtx {
            peer: &peer,
            config: &config,
            config_layers: &config_layers,
            genesis: &genesis,
            base_domain: &base_domain,
            rng: &mut rng,
            deadline,
        };
        if let Err(err) = scenario.apply(ctx).await {
            warn!(target: "izanami::faults", peer = peer.mnemonic(), ?scenario, "fault scenario failed: {err:?}");
        }
        if stop.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        let delay = config.sample_interval(&mut rng);
        let now = Instant::now();
        let Some(delay) = bounded_delay(now, deadline, delay) else {
            break;
        };
        debug!(target: "izanami::faults", peer = peer.mnemonic(), ?scenario, ?delay, "scheduling next fault");
        tokio::select! {
            () = sleep(delay) => {},
            () = stop_notify.notified() => break,
        }
    }
}

fn bounded_delay(now: Instant, deadline: Instant, delay: Duration) -> Option<Duration> {
    let remaining = deadline.checked_duration_since(now)?;
    if remaining.is_zero() {
        None
    } else {
        Some(delay.min(remaining))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
enum FaultScenario {
    CrashRestart,
    WipeStorage,
    SpamInvalidTransactions,
    NetworkLatencySpike,
    NetworkPartition,
    CpuStress,
    DiskSaturation,
}

struct FaultApplyCtx<'a, P: FaultPeer> {
    peer: &'a P,
    config: &'a FaultConfig,
    config_layers: &'a Arc<Vec<Table>>,
    genesis: &'a Arc<GenesisBlock>,
    base_domain: &'a DomainId,
    rng: &'a mut StdRng,
    deadline: Instant,
}

impl FaultScenario {
    fn random<R: Rng>(rng: &mut R, config: &FaultConfig) -> Self {
        let mut scenarios = vec![
            Self::CrashRestart,
            Self::WipeStorage,
            Self::SpamInvalidTransactions,
        ];
        if config.network_latency.is_some() {
            scenarios.push(Self::NetworkLatencySpike);
        }
        if config.network_partition.is_some() {
            scenarios.push(Self::NetworkPartition);
        }
        if config.cpu_stress.is_some() {
            scenarios.push(Self::CpuStress);
        }
        if config.disk_saturation.is_some() {
            scenarios.push(Self::DiskSaturation);
        }
        *scenarios
            .choose(rng)
            .expect("at least one fault scenario must be available")
    }

    async fn apply<P: FaultPeer>(self, ctx: FaultApplyCtx<'_, P>) -> Result<()> {
        match self {
            FaultScenario::CrashRestart => {
                crash_and_restart(ctx.peer, ctx.config_layers, ctx.genesis, ctx.rng).await
            }
            FaultScenario::WipeStorage => {
                wipe_and_restart(ctx.peer, ctx.config_layers, ctx.genesis, ctx.rng).await
            }
            FaultScenario::SpamInvalidTransactions => {
                spam_invalid_transactions(ctx.peer, ctx.base_domain, ctx.rng)
            }
            FaultScenario::NetworkLatencySpike => {
                if let Some(cfg) = &ctx.config.network_latency {
                    network_latency_spike(
                        ctx.peer,
                        ctx.config_layers,
                        ctx.genesis,
                        ctx.rng,
                        cfg,
                        ctx.deadline,
                    )
                    .await
                } else {
                    Ok(())
                }
            }
            FaultScenario::NetworkPartition => {
                if let Some(cfg) = &ctx.config.network_partition {
                    network_partition(
                        ctx.peer,
                        ctx.config_layers,
                        ctx.genesis,
                        ctx.rng,
                        cfg,
                        ctx.deadline,
                    )
                    .await
                } else {
                    Ok(())
                }
            }
            FaultScenario::CpuStress => {
                if let Some(cfg) = &ctx.config.cpu_stress {
                    cpu_stress(ctx.deadline, ctx.rng, cfg).await
                } else {
                    Ok(())
                }
            }
            FaultScenario::DiskSaturation => {
                if let Some(cfg) = &ctx.config.disk_saturation {
                    disk_saturation(ctx.peer, ctx.rng, cfg, ctx.deadline).await
                } else {
                    Ok(())
                }
            }
        }
    }
}

async fn crash_and_restart<P: FaultPeer>(
    peer: &P,
    config_layers: &Arc<Vec<Table>>,
    genesis: &Arc<GenesisBlock>,
    rng: &mut StdRng,
) -> Result<()> {
    info!(target: "izanami::faults", peer = peer.mnemonic(), "crashing peer");
    peer.shutdown().await;
    let sleep_ms = rng.random_range(1_000..=4_000);
    sleep(Duration::from_millis(sleep_ms)).await;
    info!(target: "izanami::faults", peer = peer.mnemonic(), "restarting peer");
    peer.restart_with_layers(config_layers, &[], genesis).await
}

async fn wipe_and_restart<P: FaultPeer>(
    peer: &P,
    config_layers: &Arc<Vec<Table>>,
    genesis: &Arc<GenesisBlock>,
    rng: &mut StdRng,
) -> Result<()> {
    info!(target: "izanami::faults", peer = peer.mnemonic(), "wiping storage and restarting");
    peer.shutdown().await;
    let storage_path = peer.kura_store_dir();
    let delay = rng.random_range(500..=2_500);
    sleep(Duration::from_millis(delay)).await;
    task::spawn_blocking(move || {
        if storage_path.exists()
            && let Err(err) = std::fs::remove_dir_all(&storage_path)
        {
            error!(target: "izanami::faults", path = ?storage_path, ?err, "failed to remove storage directory");
        }
        std::fs::create_dir_all(&storage_path).ok();
    })
    .await
    .map_err(|e| eyre!("failed to join storage wipe task: {e}"))?;
    info!(target: "izanami::faults", peer = peer.mnemonic(), "restarting peer after wipe");
    peer.restart_with_layers(config_layers, &[], genesis).await
}

fn spam_invalid_transactions<P: FaultPeer>(
    peer: &P,
    base_domain: &DomainId,
    rng: &mut StdRng,
) -> Result<()> {
    let client = peer.client();
    for _ in 0..3 {
        let bogus_name: Name = format!("ghost_{}", rng.random_range(0..9999))
            .parse()
            .map_err(|_| eyre!("failed to parse bogus asset name"))?;
        let bogus_definition: AssetDefinitionId =
            AssetDefinitionId::new(base_domain.clone(), bogus_name);
        let asset = AssetId::new(bogus_definition, ALICE_ID.clone());
        let instruction = Mint::asset_numeric(1_u32, asset);
        let res = client.submit_instruction(instruction);
        if let Err(err) = res {
            debug!(target: "izanami::faults", peer = peer.mnemonic(), ?err, "invalid tx rejected as expected");
        }
    }
    Ok(())
}

async fn network_latency_spike<P: FaultPeer>(
    peer: &P,
    config_layers: &Arc<Vec<Table>>,
    genesis: &Arc<GenesisBlock>,
    rng: &mut StdRng,
    cfg: &NetworkLatencyConfig,
    deadline: Instant,
) -> Result<()> {
    let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
        return Ok(());
    };
    let duration = remaining.min(sample_duration(rng, &cfg.duration));
    if duration.is_zero() {
        return Ok(());
    }

    let gossip_delay = sample_duration(rng, &cfg.gossip_delay);
    let millis = gossip_delay.as_millis().try_into().unwrap_or(i64::MAX);

    info!(
        target: "izanami::faults",
        peer = peer.mnemonic(),
        ?duration,
        ?gossip_delay,
        "injecting network latency spike"
    );

    peer.shutdown().await;
    let mut overrides = Table::new();
    overrides = overrides.write(["network", "block_gossip_period_ms"], millis);
    overrides = overrides.write(["network", "transaction_gossip_period_ms"], millis);
    let result = peer
        .restart_with_layers(config_layers, std::slice::from_ref(&overrides), genesis)
        .await;
    if let Err(err) = result {
        warn!(
            target: "izanami::faults",
            peer = peer.mnemonic(),
            ?err,
            "failed to restart peer with latency overrides; attempting recovery"
        );
        peer.shutdown().await;
        let _ = peer.restart_with_layers(config_layers, &[], genesis).await;
        return Err(err);
    }

    sleep(duration).await;
    info!(
        target: "izanami::faults",
        peer = peer.mnemonic(),
        "restoring normal network latency"
    );
    peer.shutdown().await;
    peer.restart_with_layers(config_layers, &[], genesis).await
}

async fn network_partition<P: FaultPeer>(
    peer: &P,
    config_layers: &Arc<Vec<Table>>,
    genesis: &Arc<GenesisBlock>,
    rng: &mut StdRng,
    cfg: &NetworkPartitionConfig,
    deadline: Instant,
) -> Result<()> {
    let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
        return Ok(());
    };
    let duration = remaining.min(sample_duration(rng, &cfg.duration));
    if duration.is_zero() {
        return Ok(());
    }

    info!(
        target: "izanami::faults",
        peer = peer.mnemonic(),
        ?duration,
        "isolating peer from trusted network"
    );

    peer.shutdown().await;
    let overrides = Table::new()
        .write(["trusted_peers"], Vec::<String>::new())
        .write(["trusted_peers_pop"], Table::new());
    let result = peer
        .restart_with_layers(config_layers, std::slice::from_ref(&overrides), genesis)
        .await;
    if let Err(err) = result {
        warn!(
            target: "izanami::faults",
            peer = peer.mnemonic(),
            ?err,
            "failed to restart peer with partition overrides; attempting recovery"
        );
        peer.shutdown().await;
        let _ = peer.restart_with_layers(config_layers, &[], genesis).await;
        return Err(err);
    }

    sleep(duration).await;
    info!(
        target: "izanami::faults",
        peer = peer.mnemonic(),
        "rejoining peer to network"
    );
    peer.shutdown().await;
    peer.restart_with_layers(config_layers, &[], genesis).await
}

async fn cpu_stress(deadline: Instant, rng: &mut StdRng, cfg: &CpuStressConfig) -> Result<()> {
    let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
        return Ok(());
    };
    let duration = remaining.min(sample_duration(rng, &cfg.duration));
    if duration.is_zero() {
        return Ok(());
    }

    let workers = sample_usize(rng, &cfg.workers).max(1);
    info!(
        target: "izanami::faults",
        ?duration,
        workers,
        "spawning CPU stress workers"
    );
    let deadline = Instant::now() + duration;
    let mut handles = Vec::with_capacity(workers);
    for _ in 0..workers {
        let until = deadline;
        handles.push(task::spawn_blocking(move || {
            while Instant::now() < until {
                std::hint::spin_loop();
            }
        }));
    }
    for handle in handles {
        let _ = handle.await;
    }
    Ok(())
}

async fn disk_saturation<P: FaultPeer>(
    peer: &P,
    rng: &mut StdRng,
    cfg: &DiskSaturationConfig,
    deadline: Instant,
) -> Result<()> {
    let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
        return Ok(());
    };
    let duration = remaining.min(sample_duration(rng, &cfg.duration));
    let bytes = sample_u64(rng, &cfg.bytes);
    if duration.is_zero() || bytes == 0 {
        return Ok(());
    }

    let path = peer
        .kura_store_dir()
        .join(format!("fault-fill-{}.bin", rng.next_u64()));
    info!(
        target: "izanami::faults",
        peer = peer.mnemonic(),
        ?duration,
        bytes,
        path = ?path,
        "saturating disk"
    );

    write_fill_file(path.clone(), bytes).await?;
    sleep(duration).await;
    remove_fill_file(path).await?;
    Ok(())
}

async fn write_fill_file(path: PathBuf, bytes: u64) -> Result<()> {
    task::spawn_blocking(move || -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .wrap_err("failed to ensure storage directory exists")?;
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .wrap_err("failed to create saturation file")?;
        let mut remaining = bytes;
        let buffer = vec![0_u8; 1024 * 512];
        while remaining > 0 {
            let upper = usize::try_from(remaining).unwrap_or(usize::MAX);
            let to_write = buffer.len().min(upper);
            file.write_all(&buffer[..to_write])
                .wrap_err("failed to write saturation chunk")?;
            remaining -= to_write as u64;
        }
        file.sync_all().wrap_err("failed to sync saturation file")?;
        Ok(())
    })
    .await
    .map_err(|err| eyre!("failed to join disk saturation task: {err}"))??;
    Ok(())
}

async fn remove_fill_file(path: PathBuf) -> Result<()> {
    task::spawn_blocking(move || -> Result<()> {
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err).wrap_err("failed to remove saturation file"),
        }
    })
    .await
    .map_err(|err| eyre!("failed to join cleanup task: {err}"))??;
    Ok(())
}

fn sample_duration<R: Rng>(rng: &mut R, range: &RangeInclusive<Duration>) -> Duration {
    let start = *range.start();
    let end = *range.end();
    if start == end {
        return start;
    }
    let delta = end.saturating_sub(start);
    let upper = u64::try_from(delta.as_millis()).unwrap_or(u64::MAX);
    start + Duration::from_millis(rng.random_range(0..=upper))
}

fn sample_usize<R: Rng>(rng: &mut R, range: &RangeInclusive<usize>) -> usize {
    let start = *range.start();
    let end = *range.end();
    if start >= end {
        return start;
    }
    rng.random_range(start..=end)
}

fn sample_u64<R: Rng>(rng: &mut R, range: &RangeInclusive<u64>) -> u64 {
    let start = *range.start();
    let end = *range.end();
    if start >= end {
        return start;
    }
    rng.random_range(start..=end)
}

pub trait FaultClient: Clone + Send + Sync + 'static {
    fn submit_instruction<I>(&self, instruction: I) -> Result<()>
    where
        I: Into<InstructionBox>;
}

impl FaultClient for iroha::client::Client {
    fn submit_instruction<I>(&self, instruction: I) -> Result<()>
    where
        I: Into<InstructionBox>,
    {
        self.submit_blocking(instruction).map(|_| ())
    }
}

pub trait FaultPeer: Clone + Send + Sync + 'static {
    type Client: FaultClient;

    fn mnemonic(&self) -> &str;
    fn kura_store_dir(&self) -> PathBuf;
    fn client(&self) -> Self::Client;
    fn shutdown(&self) -> Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>>;
    fn restart_with_layers<'a>(
        &'a self,
        config_layers: &'a Arc<Vec<Table>>,
        extra_layers: &'a [Table],
        genesis: &'a Arc<GenesisBlock>,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>>;
}

impl FaultPeer for NetworkPeer {
    type Client = iroha::client::Client;

    fn mnemonic(&self) -> &str {
        self.mnemonic()
    }

    fn kura_store_dir(&self) -> PathBuf {
        NetworkPeer::kura_store_dir(self)
    }

    fn client(&self) -> Self::Client {
        NetworkPeer::client(self)
    }

    fn shutdown(&self) -> Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            let _ = NetworkPeer::shutdown_if_started(self).await;
        })
    }

    fn restart_with_layers<'a>(
        &'a self,
        config_layers: &'a Arc<Vec<Table>>,
        extra_layers: &'a [Table],
        genesis: &'a Arc<GenesisBlock>,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            self.start_checked(
                config_layers
                    .iter()
                    .map(TableRef)
                    .chain(extra_layers.iter().map(TableRef)),
                Some(genesis),
            )
            .await?;
            self.once_block(1).await;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        sync::{
            Arc, Mutex as StdMutex,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
    };

    use iroha_primitives::unique_vec::UniqueVec;
    use iroha_test_network::genesis_factory;
    use tokio::{
        sync::{Mutex as AsyncMutex, Notify},
        time::{sleep, timeout},
    };

    use super::*;

    #[derive(Debug, Clone, Default)]
    struct MockClient {
        submissions: Arc<StdMutex<usize>>,
    }

    impl FaultClient for MockClient {
        fn submit_instruction<I>(&self, _instruction: I) -> Result<()>
        where
            I: Into<InstructionBox>,
        {
            if let Ok(mut guard) = self.submissions.lock() {
                *guard += 1;
            }
            Err(eyre!("mock rejection"))
        }
    }

    #[derive(Debug, Clone)]
    struct MockPeer {
        name: String,
        dir: PathBuf,
        events: Arc<AsyncMutex<Vec<MockEvent>>>,
        client: MockClient,
        restart_failures_remaining: Arc<AtomicUsize>,
    }

    #[derive(Debug, Clone)]
    enum MockEvent {
        Shutdown,
        Restart { extra_layers: Vec<Table> },
    }

    impl MockPeer {
        fn new(name: impl Into<String>) -> Self {
            let name = name.into();
            let dir = std::env::temp_dir().join(format!("izanami-test-{name}"));
            let storage = dir.join("storage");
            let _ = std::fs::create_dir_all(&storage);
            Self {
                name,
                dir,
                events: Arc::new(AsyncMutex::new(Vec::new())),
                client: MockClient::default(),
                restart_failures_remaining: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn with_restart_failures(self, failures: usize) -> Self {
            self.restart_failures_remaining
                .store(failures, Ordering::Relaxed);
            self
        }

        async fn events(&self) -> Vec<MockEvent> {
            self.events.lock().await.clone()
        }
    }

    impl FaultPeer for MockPeer {
        type Client = MockClient;

        fn mnemonic(&self) -> &str {
            &self.name
        }

        fn kura_store_dir(&self) -> PathBuf {
            self.dir.join("storage")
        }

        fn client(&self) -> Self::Client {
            self.client.clone()
        }

        fn shutdown(&self) -> Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
            let events = Arc::clone(&self.events);
            Box::pin(async move {
                events.lock().await.push(MockEvent::Shutdown);
            })
        }

        fn restart_with_layers(
            &self,
            _: &Arc<Vec<Table>>,
            extra_layers: &[Table],
            _: &Arc<GenesisBlock>,
        ) -> Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + '_>> {
            let events = Arc::clone(&self.events);
            let layers = extra_layers.to_vec();
            let restart_failures_remaining = Arc::clone(&self.restart_failures_remaining);
            Box::pin(async move {
                events.lock().await.push(MockEvent::Restart {
                    extra_layers: layers,
                });
                let should_fail = restart_failures_remaining
                    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |remaining| {
                        (remaining > 0).then(|| remaining - 1)
                    })
                    .is_ok();
                if should_fail {
                    return Err(eyre!("planned restart failure"));
                }
                Ok(())
            })
        }
    }

    #[test]
    fn interval_sampling_within_bounds() {
        let cfg = FaultConfig {
            interval: Duration::from_secs(1)..=Duration::from_secs(3),
            network_latency: None,
            network_partition: None,
            cpu_stress: None,
            disk_saturation: None,
        };
        let mut rng = StdRng::seed_from_u64(42);
        for _ in 0..16 {
            let sampled = cfg.sample_interval(&mut rng);
            assert!(sampled >= Duration::from_secs(1));
            assert!(sampled <= Duration::from_secs(3));
        }
    }

    #[test]
    fn random_includes_enabled_scenarios() {
        let config = FaultConfig {
            interval: Duration::from_secs(1)..=Duration::from_secs(1),
            network_latency: Some(NetworkLatencyConfig::default()),
            network_partition: Some(NetworkPartitionConfig::default()),
            cpu_stress: Some(CpuStressConfig::default()),
            disk_saturation: Some(DiskSaturationConfig::default()),
        };
        let mut rng = StdRng::seed_from_u64(7);
        let mut observed = HashSet::new();
        for _ in 0..256 {
            observed.insert(FaultScenario::random(&mut rng, &config));
        }
        let expected = HashSet::from([
            FaultScenario::CrashRestart,
            FaultScenario::WipeStorage,
            FaultScenario::SpamInvalidTransactions,
            FaultScenario::NetworkLatencySpike,
            FaultScenario::NetworkPartition,
            FaultScenario::CpuStress,
            FaultScenario::DiskSaturation,
        ]);
        assert_eq!(observed, expected);
    }

    #[test]
    fn bounded_delay_clamps_to_deadline() {
        let now = Instant::now();
        let deadline = now + Duration::from_millis(50);
        let delay = Duration::from_millis(200);
        let clamped = bounded_delay(now, deadline, delay).expect("delay should be bounded");
        assert!(clamped <= Duration::from_millis(50));
        assert!(clamped > Duration::from_millis(0));
    }

    #[test]
    fn bounded_delay_returns_none_when_expired() {
        let now = Instant::now();
        let deadline = now
            .checked_sub(Duration::from_millis(1))
            .expect("deadline should be in the past");
        assert!(bounded_delay(now, deadline, Duration::from_secs(1)).is_none());
    }

    fn dummy_genesis() -> Arc<GenesisBlock> {
        Arc::new(genesis_factory(Vec::new(), UniqueVec::new(), Vec::new()))
    }

    #[tokio::test]
    async fn run_fault_loop_respects_stop_flag() {
        let peer = MockPeer::new("stop");
        let stop = Arc::new(AtomicBool::new(true));
        let stop_notify = Arc::new(Notify::new());
        let config = FaultConfig {
            interval: Duration::from_secs(1)..=Duration::from_secs(1),
            network_latency: None,
            network_partition: None,
            cpu_stress: None,
            disk_saturation: None,
        };
        let config_layers = Arc::new(Vec::new());
        let genesis = dummy_genesis();
        let domain: DomainId = "wonderland".parse().expect("domain");
        run_fault_loop(
            peer.clone(),
            config,
            genesis,
            config_layers,
            domain,
            stop,
            stop_notify,
            Instant::now() + Duration::from_secs(1),
            7,
        )
        .await;
        let events = peer.events().await;
        assert!(
            events.is_empty(),
            "stop flag should prevent fault loop work"
        );
    }

    #[tokio::test]
    async fn run_fault_loop_wakes_on_stop_notify() {
        let peer = MockPeer::new("stop-notify");
        let stop = Arc::new(AtomicBool::new(false));
        let stop_notify = Arc::new(Notify::new());
        let config = FaultConfig {
            interval: Duration::from_secs(10)..=Duration::from_secs(10),
            network_latency: None,
            network_partition: None,
            cpu_stress: None,
            disk_saturation: None,
        };
        let config_layers = Arc::new(Vec::new());
        let genesis = dummy_genesis();
        let domain: DomainId = "wonderland".parse().expect("domain");
        let deadline = Instant::now() + Duration::from_secs(30);

        let handle = tokio::spawn(run_fault_loop(
            peer.clone(),
            config,
            genesis,
            config_layers,
            domain,
            Arc::clone(&stop),
            Arc::clone(&stop_notify),
            deadline,
            99,
        ));

        sleep(Duration::from_millis(10)).await;
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        stop_notify.notify_waiters();

        timeout(Duration::from_secs(1), handle)
            .await
            .expect("fault loop should stop promptly")
            .expect("fault loop task should complete");
    }

    #[tokio::test]
    async fn network_latency_fault_reconfigures_peer() {
        let peer = MockPeer::new("latency");
        let config_layers = Arc::new(Vec::new());
        let genesis = dummy_genesis();
        let mut rng = StdRng::seed_from_u64(9);
        let domain: DomainId = "wonderland".parse().expect("domain");
        let config = FaultConfig {
            interval: Duration::from_secs(1)..=Duration::from_secs(1),
            network_latency: Some(NetworkLatencyConfig {
                duration: Duration::from_millis(5)..=Duration::from_millis(5),
                gossip_delay: Duration::from_millis(10)..=Duration::from_millis(10),
            }),
            network_partition: None,
            cpu_stress: None,
            disk_saturation: None,
        };
        let ctx = FaultApplyCtx {
            peer: &peer,
            config: &config,
            config_layers: &config_layers,
            genesis: &genesis,
            base_domain: &domain,
            rng: &mut rng,
            deadline: Instant::now() + Duration::from_secs(1),
        };
        FaultScenario::NetworkLatencySpike
            .apply(ctx)
            .await
            .expect("latency fault");
        let events = peer.events().await;
        assert_eq!(events.len(), 4);
        match &events[1] {
            MockEvent::Restart { extra_layers } => {
                assert_eq!(extra_layers.len(), 1);
                let network = extra_layers[0]
                    .get("network")
                    .and_then(|value| value.as_table())
                    .expect("network table");
                assert_eq!(
                    network
                        .get("block_gossip_period_ms")
                        .and_then(toml::Value::as_integer),
                    Some(10)
                );
                assert_eq!(
                    network
                        .get("transaction_gossip_period_ms")
                        .and_then(toml::Value::as_integer),
                    Some(10)
                );
            }
            other => panic!("unexpected event: {other:?}"),
        }
        assert!(matches!(events[0], MockEvent::Shutdown));
        assert!(matches!(events[2], MockEvent::Shutdown));
        match &events[3] {
            MockEvent::Restart { extra_layers } => assert!(extra_layers.is_empty()),
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn network_latency_recovery_shuts_down_before_restarting_without_overrides() {
        let peer = MockPeer::new("latency-recovery").with_restart_failures(1);
        let config_layers = Arc::new(Vec::new());
        let genesis = dummy_genesis();
        let mut rng = StdRng::seed_from_u64(17);
        let domain: DomainId = "wonderland".parse().expect("domain");
        let config = FaultConfig {
            interval: Duration::from_secs(1)..=Duration::from_secs(1),
            network_latency: Some(NetworkLatencyConfig {
                duration: Duration::from_millis(5)..=Duration::from_millis(5),
                gossip_delay: Duration::from_millis(10)..=Duration::from_millis(10),
            }),
            network_partition: None,
            cpu_stress: None,
            disk_saturation: None,
        };
        let ctx = FaultApplyCtx {
            peer: &peer,
            config: &config,
            config_layers: &config_layers,
            genesis: &genesis,
            base_domain: &domain,
            rng: &mut rng,
            deadline: Instant::now() + Duration::from_secs(1),
        };

        let _ = FaultScenario::NetworkLatencySpike
            .apply(ctx)
            .await
            .expect_err("fault should report initial override restart failure");

        let events = peer.events().await;
        assert_eq!(events.len(), 4);
        assert!(matches!(events[0], MockEvent::Shutdown));
        assert!(matches!(events[1], MockEvent::Restart { .. }));
        assert!(matches!(events[2], MockEvent::Shutdown));
        match &events[3] {
            MockEvent::Restart { extra_layers } => assert!(
                extra_layers.is_empty(),
                "recovery restart must remove temporary latency overrides"
            ),
            other => panic!("unexpected final recovery event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn disk_saturation_creates_and_removes_file() {
        let peer = MockPeer::new("disk");
        let config_layers = Arc::new(Vec::new());
        let genesis = dummy_genesis();
        let mut rng = StdRng::seed_from_u64(11);
        let domain: DomainId = "wonderland".parse().expect("domain");
        let storage = peer.kura_store_dir();
        if storage.exists() {
            std::fs::remove_dir_all(&storage).unwrap();
        }
        std::fs::create_dir_all(&storage).unwrap();
        let config = FaultConfig {
            interval: Duration::from_secs(1)..=Duration::from_secs(1),
            network_latency: None,
            network_partition: None,
            cpu_stress: None,
            disk_saturation: Some(DiskSaturationConfig {
                duration: Duration::from_millis(5)..=Duration::from_millis(5),
                bytes: 1_024..=1_024,
            }),
        };
        let ctx = FaultApplyCtx {
            peer: &peer,
            config: &config,
            config_layers: &config_layers,
            genesis: &genesis,
            base_domain: &domain,
            rng: &mut rng,
            deadline: Instant::now() + Duration::from_secs(1),
        };
        FaultScenario::DiskSaturation
            .apply(ctx)
            .await
            .expect("disk fault");
        let entries: Vec<_> = std::fs::read_dir(&storage)
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn cpu_stress_respects_deadline() {
        let mut rng = StdRng::seed_from_u64(21);
        let cfg = CpuStressConfig {
            duration: Duration::from_millis(5)..=Duration::from_millis(5),
            workers: 1..=1,
        };
        let deadline = Instant::now() + Duration::from_millis(25);
        timeout(Duration::from_secs(1), cpu_stress(deadline, &mut rng, &cfg))
            .await
            .expect("cpu stress should respect timeout")
            .expect("cpu stress must finish without error");
    }

    #[tokio::test]
    async fn network_partition_isolates_and_rejoins_peer() {
        let peer = MockPeer::new("partition");
        let config_layers = Arc::new(Vec::new());
        let genesis = dummy_genesis();
        let mut rng = StdRng::seed_from_u64(25);
        let domain: DomainId = "wonderland".parse().expect("domain");
        let config = FaultConfig {
            interval: Duration::from_secs(1)..=Duration::from_secs(1),
            network_latency: None,
            network_partition: Some(NetworkPartitionConfig {
                duration: Duration::from_millis(5)..=Duration::from_millis(5),
            }),
            cpu_stress: None,
            disk_saturation: None,
        };
        let ctx = FaultApplyCtx {
            peer: &peer,
            config: &config,
            config_layers: &config_layers,
            genesis: &genesis,
            base_domain: &domain,
            rng: &mut rng,
            deadline: Instant::now() + Duration::from_secs(1),
        };

        FaultScenario::NetworkPartition
            .apply(ctx)
            .await
            .expect("network partition fault should succeed");

        let events = peer.events().await;
        assert_eq!(events.len(), 4);
        assert!(matches!(events[0], MockEvent::Shutdown));
        match &events[1] {
            MockEvent::Restart { extra_layers } => {
                assert_eq!(extra_layers.len(), 1);
                let trusted = extra_layers[0]
                    .get("trusted_peers")
                    .and_then(toml::Value::as_array)
                    .expect("trusted_peers array");
                assert!(
                    trusted.is_empty(),
                    "partition should clear trusted peers temporarily"
                );
                let trusted_pop = extra_layers[0]
                    .get("trusted_peers_pop")
                    .and_then(toml::Value::as_table)
                    .expect("trusted_peers_pop table");
                assert!(
                    trusted_pop.is_empty(),
                    "partition should clear trusted_peers_pop to keep config valid"
                );
            }
            other => panic!("unexpected restart payload: {other:?}"),
        }
        assert!(matches!(events[2], MockEvent::Shutdown));
        match &events[3] {
            MockEvent::Restart { extra_layers } => assert!(
                extra_layers.is_empty(),
                "rejoin restart should not carry partition overrides"
            ),
            other => panic!("unexpected final event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn network_partition_recovery_shuts_down_before_restarting_without_overrides() {
        let peer = MockPeer::new("partition-recovery").with_restart_failures(1);
        let config_layers = Arc::new(Vec::new());
        let genesis = dummy_genesis();
        let mut rng = StdRng::seed_from_u64(51);
        let domain: DomainId = "wonderland".parse().expect("domain");
        let config = FaultConfig {
            interval: Duration::from_secs(1)..=Duration::from_secs(1),
            network_latency: None,
            network_partition: Some(NetworkPartitionConfig {
                duration: Duration::from_millis(5)..=Duration::from_millis(5),
            }),
            cpu_stress: None,
            disk_saturation: None,
        };
        let ctx = FaultApplyCtx {
            peer: &peer,
            config: &config,
            config_layers: &config_layers,
            genesis: &genesis,
            base_domain: &domain,
            rng: &mut rng,
            deadline: Instant::now() + Duration::from_secs(1),
        };

        let _ = FaultScenario::NetworkPartition
            .apply(ctx)
            .await
            .expect_err("fault should report initial override restart failure");

        let events = peer.events().await;
        assert_eq!(events.len(), 4);
        assert!(matches!(events[0], MockEvent::Shutdown));
        assert!(matches!(events[1], MockEvent::Restart { .. }));
        assert!(matches!(events[2], MockEvent::Shutdown));
        match &events[3] {
            MockEvent::Restart { extra_layers } => assert!(
                extra_layers.is_empty(),
                "recovery restart must remove temporary partition overrides"
            ),
            other => panic!("unexpected final recovery event: {other:?}"),
        }
    }
}
