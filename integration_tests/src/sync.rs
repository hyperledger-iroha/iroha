//! Synchronization helpers for integration tests.

use std::{
    env,
    thread::sleep,
    time::{Duration, Instant},
};

use eyre::{Result, WrapErr};
use iroha::client::{Client, Status};
use iroha_test_network::{BlockHeight, Network};
use tokio::runtime::Runtime;
use tokio::task::spawn_blocking;

// Integration submissions occasionally need more time to commit under DA-enabled consensus;
// give the network a bounded window before failing. Keep bounded to avoid long hangs when
// Torii is unreachable, but allow env overrides for slower hosts.
const STATUS_RETRY_DELAY: Duration = Duration::from_millis(100);
const STATUS_RETRY_DEFAULT: Duration = Duration::from_secs(120);

/// Poll `/status` with a bounded retry budget to tolerate startup jitter.
///
/// # Errors
///
/// Returns the final status error when retries are exhausted or a sandbox denial is detected.
pub fn get_status_with_retry(client: &Client) -> Result<Status> {
    let retry_budget = status_retry_budget_env();
    let deadline = Instant::now() + retry_budget;
    let mut last_err = None;
    while Instant::now() < deadline {
        match client.get_status() {
            Ok(status) => return Ok(status),
            Err(err) => {
                if let Some(reason) = crate::sandbox::sandbox_reason(&err) {
                    return Err(eyre::eyre!(
                        "sandboxed network restriction detected while polling /status: {reason}"
                    ));
                }
                last_err = Some(err);
                sleep(STATUS_RETRY_DELAY);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| eyre::eyre!("status retry budget exhausted"))).wrap_err_with(
        || {
            format!(
                "status retry budget exhausted after {:?} hitting {}",
                retry_budget, client.torii_url
            )
        },
    )
}

/// Poll `/status` with a bounded retry budget, falling back to storage-derived
/// heights when Torii stalls.
///
/// # Errors
///
/// Returns the final status error when retries are exhausted and no storage
/// snapshot is available.
pub fn get_status_with_retry_or_storage(
    network: &Network,
    client: &Client,
    context: &str,
) -> Result<Status> {
    match get_status_with_retry(client) {
        Ok(status) => Ok(status),
        Err(err) => {
            if let Some(status) = best_effort_status_from_network(network) {
                eprintln!("warning: {context} status poll failed; using storage snapshot: {err}");
                Ok(status)
            } else {
                Err(err).wrap_err_with(|| {
                    format!(
                        "status retry failed and no storage snapshot available ({context}); torii={}",
                        client.torii_url
                    )
                })
            }
        }
    }
}

/// Async wrapper around [`get_status_with_retry`] to avoid blocking async runtimes.
///
/// # Errors
///
/// Returns the same errors as [`get_status_with_retry`], or a join error if the
/// blocking task panics.
pub async fn get_status_with_retry_async(client: &Client) -> Result<Status> {
    let client = client.clone();
    let status = spawn_blocking(move || get_status_with_retry(&client))
        .await
        .wrap_err("status retry task panicked")??;
    Ok(status)
}

/// Wait for the next non-empty block and return the refreshed status, tolerating timeouts.
///
/// # Errors
///
/// Propagates status fetch failures or timeouts while waiting for the target height.
pub fn sync_after_submission(
    network: &Network,
    rt: &Runtime,
    client: &Client,
    previous_non_empty_height: u64,
    context: &str,
) -> Result<Status> {
    let target_height = previous_non_empty_height.saturating_add(1);
    if let Err(err) = rt.block_on(async {
        tokio::time::timeout(
            status_retry_budget_env(),
            network.ensure_blocks_with(|h| h.non_empty >= target_height),
        )
        .await
    }) {
        eprintln!(
            "warning: ensure_blocks_with timed out after {context}; continuing with status poll: {err}"
        );
    }
    get_status_with_retry_or_storage(network, client, context).wrap_err_with(|| {
        format!(
            "failed to refresh status after submission ({context}); target height={}, torii={}",
            target_height, client.torii_url
        )
    })
}

fn best_effort_status_from_network(network: &Network) -> Option<Status> {
    let height = best_effort_block_height(network)?;
    let peers = network
        .peers()
        .iter()
        .find_map(iroha_test_network::NetworkPeer::last_known_peers)
        .unwrap_or_else(|| network.peers().len().saturating_sub(1) as u64);
    let status = Status {
        blocks: height.total,
        blocks_non_empty: height.non_empty,
        peers,
        ..Status::default()
    };
    Some(status)
}

fn best_effort_block_height(network: &Network) -> Option<BlockHeight> {
    let mut best: Option<BlockHeight> = None;
    for peer in network.peers() {
        if let Some(height) = peer.best_effort_block_height() {
            best = Some(best.map_or(height, |current| BlockHeight {
                total: current.total.max(height.total),
                non_empty: current.non_empty.max(height.non_empty),
            }));
        }
    }
    best
}

fn status_retry_budget_env() -> Duration {
    read_env_duration("IROHA_TEST_STATUS_RETRY_BUDGET_MS", STATUS_RETRY_DEFAULT)
}

fn read_env_duration(var: &str, default: Duration) -> Duration {
    if let Ok(raw) = env::var(var) {
        let trimmed = raw.trim();
        if let Some(ms) = trimmed.strip_suffix("ms")
            && let Ok(value) = ms.parse::<u64>()
        {
            return Duration::from_millis(value);
        }
        if let Ok(value) = trimmed.parse::<u64>() {
            return Duration::from_secs(value);
        }
    }
    default
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{Mutex, MutexGuard, OnceLock},
    };

    use super::*;
    use iroha::client::Client;
    use iroha::config::{
        AnonymityPolicy, Config, DEFAULT_TORII_API_MIN_PROOF_VERSION, default_connect_queue_root,
        default_torii_api_version,
    };
    use iroha::data_model::ChainId;
    use iroha_test_network::{NetworkBuilder, init_instruction_registry};
    use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
    use sorafs_manifest::alias_cache::AliasCachePolicy;

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    #[allow(unsafe_code)]
    fn remove_env_var(key: &str) {
        // Safety: tests serialize env mutation with ENV_LOCK.
        unsafe {
            std::env::remove_var(key);
        }
    }

    #[allow(unsafe_code)]
    fn set_env_var(key: &str, value: &str) {
        // Safety: tests serialize env mutation with ENV_LOCK.
        unsafe {
            std::env::set_var(key, value);
        }
    }

    struct EnvRestore {
        key: &'static str,
        value: Option<String>,
    }

    impl EnvRestore {
        fn remove(key: &'static str) -> Self {
            let value = std::env::var(key).ok();
            remove_env_var(key);
            Self { key, value }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            if let Some(value) = &self.value {
                set_env_var(self.key, value);
            } else {
                remove_env_var(self.key);
            }
        }
    }

    fn lock_env_guard() -> MutexGuard<'static, ()> {
        ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env lock")
    }

    fn dummy_client() -> Client {
        let ttl = Duration::from_secs(1);
        let config = Config {
            chain: ChainId::from("test"),
            key_pair: ALICE_KEYPAIR.clone(),
            account: ALICE_ID.clone(),
            torii_api_url: "http://127.0.0.1:1".parse().expect("valid url"),
            torii_api_version: default_torii_api_version(),
            torii_api_min_proof_version: DEFAULT_TORII_API_MIN_PROOF_VERSION.to_string(),
            torii_request_timeout: Duration::from_millis(50),
            basic_auth: None,
            transaction_add_nonce: false,
            transaction_ttl: ttl,
            transaction_status_timeout: ttl,
            connect_queue_root: default_connect_queue_root(),
            sorafs_alias_cache: AliasCachePolicy::new(ttl, ttl, ttl, ttl, ttl, ttl, ttl, ttl),
            sorafs_anonymity_policy: AnonymityPolicy::default(),
            sorafs_rollout_phase: iroha_config::parameters::actual::SorafsRolloutPhase::default(),
        };
        let mut client = Client::new(config);
        client.headers = HashMap::new();
        client
    }

    #[test]
    fn status_retry_budget_env_parses_ms_suffix() {
        let _env_guard = lock_env_guard();
        let _restore = EnvRestore::remove("IROHA_TEST_STATUS_RETRY_BUDGET_MS");

        set_env_var("IROHA_TEST_STATUS_RETRY_BUDGET_MS", "250ms");
        assert_eq!(status_retry_budget_env(), Duration::from_millis(250));
    }

    #[test]
    fn status_retry_budget_env_parses_seconds() {
        let _env_guard = lock_env_guard();
        let _restore = EnvRestore::remove("IROHA_TEST_STATUS_RETRY_BUDGET_MS");

        set_env_var("IROHA_TEST_STATUS_RETRY_BUDGET_MS", "2");
        assert_eq!(status_retry_budget_env(), Duration::from_secs(2));
    }

    #[test]
    fn status_retry_budget_env_uses_default_when_unset() {
        let _env_guard = lock_env_guard();
        let _restore = EnvRestore::remove("IROHA_TEST_STATUS_RETRY_BUDGET_MS");

        assert_eq!(status_retry_budget_env(), STATUS_RETRY_DEFAULT);
    }

    #[tokio::test]
    async fn status_retry_async_returns_error_for_unreachable_host() {
        let env_guard = lock_env_guard();
        let _restore = EnvRestore::remove("IROHA_TEST_STATUS_RETRY_BUDGET_MS");
        set_env_var("IROHA_TEST_STATUS_RETRY_BUDGET_MS", "5ms");

        let client = dummy_client();
        drop(env_guard);
        let result = get_status_with_retry_async(&client).await;
        assert!(result.is_err());
    }

    #[test]
    fn best_effort_status_is_none_without_storage_heights() {
        init_instruction_registry();
        let network = NetworkBuilder::new()
            .with_auto_populated_trusted_peers()
            .build();
        assert!(best_effort_status_from_network(&network).is_none());
    }

    #[test]
    fn status_retry_or_storage_errors_without_snapshot() {
        let _env_guard = lock_env_guard();
        let _restore = EnvRestore::remove("IROHA_TEST_STATUS_RETRY_BUDGET_MS");
        set_env_var("IROHA_TEST_STATUS_RETRY_BUDGET_MS", "5ms");

        init_instruction_registry();
        let network = NetworkBuilder::new()
            .with_auto_populated_trusted_peers()
            .build();
        let client = dummy_client();
        let err = get_status_with_retry_or_storage(&network, &client, "status test")
            .expect_err("should fail without storage snapshot");
        let msg = err.to_string();
        assert!(
            msg.contains("status retry failed"),
            "expected status retry failure, got: {msg}"
        );
    }
}
