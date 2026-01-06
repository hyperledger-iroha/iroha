//! Synchronization helpers for integration tests.

use std::{
    env,
    thread::sleep,
    time::{Duration, Instant},
};

use eyre::{Result, WrapErr};
use iroha::client::{Client, Status};
use iroha_test_network::Network;
use tokio::runtime::Runtime;

// Integration submissions occasionally need more time to commit under DA-enabled consensus;
// give the network a bounded window before failing. Keep bounded to avoid long hangs when
// Torii is unreachable, but allow env overrides for slower hosts.
const STATUS_RETRY_DELAY: Duration = Duration::from_millis(300);
const STATUS_RETRY_DEFAULT: Duration = Duration::from_secs(30);

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
    get_status_with_retry(client).wrap_err_with(|| {
        format!(
            "failed to refresh status after submission ({context}); target height={}, torii={}",
            target_height, client.torii_url
        )
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
