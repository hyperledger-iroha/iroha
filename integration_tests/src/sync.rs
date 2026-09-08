//! Synchronization helpers for integration tests.
use eyre::{Result, WrapErr};
use iroha::{blocking::Client, client::ClientBuilder};
use iroha_test_network::{BlockHeight, Network};
use iroha_torii_shared::status::Status;
use std::{
    env,
    thread::sleep,
    time::{Duration, Instant},
};
use tokio::runtime::Runtime;
// Integration submissions occasionally need more time to commit under DA-enabled consensus;
// give the network a bounded window before failing. Keep bounded to avoid long hangs when
// Torii is unreachable, but allow env overrides for slower hosts.
const STATUS_RETRY_DELAY: Duration = Duration::from_millis(100);
const STATUS_RETRY_DEFAULT: Duration = Duration::from_secs(120);
/// Create a fresh blocking context from an explicitly configured client builder.
///
/// This keeps integration-test configuration changes from mutating or escaping
/// through the blocking facade while preserving its account-binding validation.
#[must_use]
pub fn rebind_blocking_client(
    client: &Client,
    configure: impl FnOnce(&mut ClientBuilder),
) -> Client {
    try_rebind_blocking_client(client, configure)
        .expect("reconfigured integration-test client must remain valid")
}
fn try_rebind_blocking_client(
    client: &Client,
    configure: impl FnOnce(&mut ClientBuilder),
) -> Result<Client> {
    let mut inner = client.client().to_builder();
    configure(&mut inner);
    Client::from_client(inner.build()?)
}
/// Poll `/status` with a bounded retry budget to tolerate startup jitter.
///
/// # Errors
///
/// Returns the final status error when retries are exhausted or a sandbox denial is detected.
pub fn get_status_with_retry(client: &Client) -> Result<Status> {
    get_status_with_retry_at_least(client, 0)
}
/// Poll `/status` until the authoritative applied height reaches `minimum_blocks`.
///
/// # Errors
///
/// Returns the final status error, a below-height exhaustion error, or a sandbox denial.
pub fn get_status_with_retry_at_least(client: &Client, minimum_blocks: u64) -> Result<Status> {
    enum LastObservation {
        BelowHeight(u64),
        Error(eyre::Report),
    }
    let retry_budget = status_retry_budget_env();
    let deadline = Instant::now() + retry_budget;
    let mut last_observation = None;
    while Instant::now() < deadline {
        match client.status().get().map_err(eyre::Report::from) {
            Ok(status) => {
                if status_reaches_height(&status, minimum_blocks) {
                    return Ok(status);
                }
                last_observation = Some(LastObservation::BelowHeight(status.blocks));
            }
            Err(err) => {
                if let Some(reason) = crate::sandbox::sandbox_reason(&err) {
                    return Err(eyre::eyre!(
                        "sandboxed network restriction detected while polling /status: {reason}"
                    ));
                }
                last_observation = Some(LastObservation::Error(err));
            }
        }
        sleep(STATUS_RETRY_DELAY);
    }
    let terminal = match last_observation {
        Some(LastObservation::BelowHeight(height)) => eyre::eyre!(
            "authoritative status remained at block height {height}, below required height {minimum_blocks}"
        ),
        Some(LastObservation::Error(err)) => err,
        None => eyre::eyre!("status retry budget exhausted"),
    };
    Err(terminal).wrap_err_with(|| {
        format!(
            "status retry budget exhausted after {:?} hitting {}",
            retry_budget,
            client.client().endpoint()
        )
    })
}
fn status_reaches_height(status: &Status, minimum_blocks: u64) -> bool {
    status.blocks >= minimum_blocks
}
/// Poll `/status` with a bounded retry budget, falling back to storage-derived
/// heights when Torii stalls.
///
/// # Errors
///
/// Returns the final status error when retries are exhausted and no storage snapshot is available.
pub fn get_status_with_retry_or_storage(
    network: &Network,
    client: &Client,
    context: &str,
) -> Result<Status> {
    let fallback = best_effort_status_from_network(network);
    apply_storage_fallback(get_status_with_retry(client), fallback, client, context)
}
fn apply_storage_fallback(
    status_result: Result<Status>,
    fallback: Option<Status>,
    client: &Client,
    context: &str,
) -> Result<Status> {
    match status_result {
        Ok(status) => Ok(status),
        Err(err) => {
            if let Some(status) = fallback {
                eprintln!("warning: {context} status poll failed; using storage snapshot: {err}");
                Ok(status)
            } else {
                Err(err).wrap_err_with(|| {
                    format!(
                        "status retry failed and no storage snapshot available ({context}); torii={}",
                        client.client().endpoint()
                    )
                })
            }
        }
    }
}
/// Poll the status endpoint asynchronously with the same bounded retry policy
/// as the synchronous status helper.
///
/// # Errors
///
/// Returns the final status error when retries are exhausted or a sandbox denial is detected.
pub async fn get_status_with_retry_async(client: &Client) -> Result<Status> {
    get_status_with_retry_at_least_async(client.client().endpoint().as_str(), 0, || async {
        client
            .client()
            .status()
            .get()
            .await
            .map_err(eyre::Report::from)
    })
    .await
}
/// Poll asynchronously until the authoritative applied height reaches the target.
///
/// # Errors
/// Returns the final request error, below-height exhaustion, or sandbox denial.
pub(crate) async fn get_status_with_retry_at_least_async<F, Fut>(
    endpoint: &str,
    minimum_blocks: u64,
    mut read_status: F,
) -> Result<Status>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<Status>>,
{
    enum LastObservation {
        BelowHeight(u64),
        Error(eyre::Report),
    }
    let retry_budget = status_retry_budget_env();
    let deadline = Instant::now() + retry_budget;
    let mut last_observation = None;
    while Instant::now() < deadline {
        match read_status().await {
            Ok(status) => {
                if status_reaches_height(&status, minimum_blocks) {
                    return Ok(status);
                }
                last_observation = Some(LastObservation::BelowHeight(status.blocks));
            }
            Err(err) => {
                if let Some(reason) = crate::sandbox::sandbox_reason(&err) {
                    return Err(eyre::eyre!(
                        "sandboxed network restriction detected while polling /status: {reason}"
                    ));
                }
                last_observation = Some(LastObservation::Error(err));
            }
        }
        tokio::time::sleep(STATUS_RETRY_DELAY).await;
    }
    let terminal = match last_observation {
        Some(LastObservation::BelowHeight(height)) => eyre::eyre!(
            "authoritative status remained at block height {height}, below required height {minimum_blocks}"
        ),
        Some(LastObservation::Error(err)) => err,
        None => eyre::eyre!("status retry budget exhausted"),
    };
    Err(terminal).wrap_err_with(|| {
        format!(
            "status retry budget exhausted after {:?} hitting {}",
            retry_budget, endpoint
        )
    })
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
            target_height,
            client.client().endpoint()
        )
    })
}
fn best_effort_status_from_network(network: &Network) -> Option<Status> {
    let peers = network
        .peers()
        .iter()
        .find_map(iroha_test_network::NetworkPeer::last_known_peers)
        .unwrap_or_else(|| network.peers().len().saturating_sub(1) as u64);
    status_from_storage_snapshot(best_effort_block_height(network), peers)
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
fn status_from_storage_snapshot(height: Option<BlockHeight>, peers: u64) -> Option<Status> {
    let height = height?;
    Some(Status {
        blocks: height.total,
        blocks_non_empty: height.non_empty,
        peers,
        ..Status::default()
    })
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
    use super::*;
    use iroha::crypto::{Hash, HashOf};
    use iroha::data_model::{ChainId, NetworkId};
    use iroha::{client::Client as AsyncClient, config::Config};
    use iroha_service_model::soranet::AnonymityPolicy;
    use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID, BOB_KEYPAIR};
    use sorafs_manifest::alias_cache::AliasCachePolicy;
    use std::{
        collections::HashMap,
        sync::{Mutex, MutexGuard, OnceLock},
    };
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
            network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
                b"integration_tests::sync::dummy_client",
            ))),
            key_pair: ALICE_KEYPAIR.clone(),
            account: ALICE_ID.clone(),
            account_chain_discriminant:
                iroha_config::parameters::defaults::common::chain_discriminant(),
            torii_api_url: "http://127.0.0.1:1".parse().expect("valid url"),
            torii_request_timeout: Duration::from_millis(50),
            basic_auth: None,
            transaction_add_nonce: false,
            transaction_ttl: ttl,
            transaction_status_timeout: ttl,
            sorafs_alias_cache: AliasCachePolicy::new(ttl, ttl, ttl, ttl, ttl, ttl, ttl, ttl),
            sorafs_anonymity_policy: AnonymityPolicy::default(),
            sorafs_rollout_phase: iroha_service_model::soranet::RolloutPhase::default(),
        };
        let mut builder = AsyncClient::builder(config);
        builder.headers = HashMap::new();
        Client::from_client(builder.build().expect("valid status fixture"))
            .expect("blocking status fixture client")
    }
    #[test]
    fn rebind_blocking_client_isolates_configuration_and_rebinds_account() {
        let original = dummy_client();
        let original_timeout = original.client().torii_request_timeout();
        let updated_timeout = original_timeout + Duration::from_millis(1);
        let rebound = rebind_blocking_client(&original, |client| {
            client.torii_request_timeout = updated_timeout;
            client.account = BOB_ID.clone();
            client.key_pair = BOB_KEYPAIR.clone();
        });
        assert_eq!(original.client().torii_request_timeout(), original_timeout);
        assert_eq!(original.client().account(), &*ALICE_ID);
        assert_eq!(
            original.client().key_pair().public_key(),
            ALICE_KEYPAIR.public_key()
        );
        assert_eq!(rebound.client().torii_request_timeout(), updated_timeout);
        assert_eq!(rebound.account_client().authority(), &*BOB_ID);
        assert_eq!(
            rebound.client().key_pair().public_key(),
            BOB_KEYPAIR.public_key()
        );

        let error = try_rebind_blocking_client(&original, |client| {
            client.account = BOB_ID.clone();
        })
        .expect_err("mismatched account and signing key must be rejected");
        assert!(
            error
                .to_string()
                .contains("account authority does not match the configured signing key"),
            "unexpected mismatched-key error: {error:#}"
        );
        assert_eq!(original.client().account(), &*ALICE_ID);
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
    #[test]
    fn startup_status_predicate_rejects_height_zero_and_accepts_target_height() {
        let height_zero = Status::default();
        assert!(!status_reaches_height(&height_zero, 1));
        let height_one = Status {
            blocks: 1,
            blocks_non_empty: 1,
            ..Status::default()
        };
        assert!(status_reaches_height(&height_one, 1));
        assert!(!status_reaches_height(&height_one, 2));
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
        let error = result.expect_err("unreachable status endpoint must fail");
        assert!(
            matches!(
                error.downcast_ref::<iroha::Error>(),
                Some(iroha::Error::Transport { .. } | iroha::Error::Timeout { .. })
            ),
            "async polling must retain the network error rather than enter the blocking facade: {error:#}"
        );
    }
    #[test]
    fn status_retry_source_preserves_height_barrier_and_sandbox_rejection() {
        let _env_guard = lock_env_guard();
        let _restore = EnvRestore::remove("IROHA_TEST_STATUS_RETRY_BUDGET_MS");
        set_env_var("IROHA_TEST_STATUS_RETRY_BUDGET_MS", "2");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("status source test runtime");
        let mut attempts = 0;
        let status = runtime
            .block_on(get_status_with_retry_at_least_async(
                "http://status-source.invalid/",
                1,
                || {
                    let blocks = attempts;
                    attempts += 1;
                    std::future::ready(Ok(Status {
                        blocks,
                        ..Status::default()
                    }))
                },
            ))
            .expect("source must reach the applied-height barrier");
        assert_eq!(status.blocks, 1);
        assert_eq!(
            attempts, 2,
            "height zero is not an applied genesis observation"
        );
        let mut denied_attempts = 0;
        let error = runtime
            .block_on(get_status_with_retry_at_least_async(
                "http://status-source.invalid/",
                1,
                || {
                    denied_attempts += 1;
                    std::future::ready(Err(eyre::Report::from(iroha::Error::Transport {
                        operation: "diagnostic.status",
                        kind: iroha::TransportErrorKind::Io(std::io::ErrorKind::PermissionDenied),
                        details: "opaque transport failure".to_owned(),
                    })))
                },
            ))
            .expect_err("sandbox denials must terminate the source retry loop");
        assert_eq!(denied_attempts, 1);
        assert!(
            error
                .to_string()
                .contains("sandboxed network restriction detected")
        );
    }
    #[test]
    fn best_effort_status_is_none_without_storage_heights() {
        assert!(status_from_storage_snapshot(None, 0).is_none());
    }
    #[test]
    fn status_retry_or_storage_errors_without_snapshot() {
        let _env_guard = lock_env_guard();
        let _restore = EnvRestore::remove("IROHA_TEST_STATUS_RETRY_BUDGET_MS");
        set_env_var("IROHA_TEST_STATUS_RETRY_BUDGET_MS", "5ms");
        let client = dummy_client();
        let err = apply_storage_fallback(
            Err(eyre::eyre!("status retry budget exhausted")),
            None,
            &client,
            "status test",
        )
        .expect_err("should fail without storage snapshot");
        let msg = err.to_string();
        assert!(
            msg.contains("status retry failed"),
            "expected status retry failure, got: {msg}"
        );
    }
}
