//! Account-free wallet discovery on the SDK's explicit blocking runtime.

use super::{RuntimeOwner, reject_inside_async_runtime};
use eyre::Result;
use iroha_data_model::NetworkId;
use iroha_torii_shared::{
    account_capabilities::AccountCapabilitiesV1, account_faucet_policy::AccountFaucetAdvertisement,
};
use std::{sync::Arc, time::Duration};
use url::Url;

/// Blocking public network discovery; construction and requests never load or generate a signer.
#[derive(Clone, Debug)]
pub struct Client {
    inner: crate::account_bootstrap::Client,
    runtime: Arc<RuntimeOwner>,
}

impl Client {
    /// Construct an account-free discovery client with a bounded per-request deadline.
    ///
    /// # Errors
    /// Rejects invalid endpoint/deadline, nested async use and runtime construction failures.
    pub fn new(endpoint: Url, timeout: Duration) -> Result<Self> {
        reject_inside_async_runtime()?;
        Ok(Self {
            inner: crate::account_bootstrap::Client::new(endpoint, timeout)?,
            runtime: Arc::new(RuntimeOwner::new()?),
        })
    }

    /// Discover exact genesis/profile and the explicit admitted signing default.
    ///
    /// # Errors
    /// Returns the asynchronous discovery error or a blocking runtime rejection.
    pub fn capabilities(&self) -> Result<AccountCapabilitiesV1> {
        self.runtime.block_on(self.inner.capabilities())?
    }

    /// Fetch and verify operator faucet policy against the wallet's pinned network context.
    ///
    /// # Errors
    /// Returns policy/discovery failure or a blocking runtime rejection.
    pub fn faucet_policy(
        &self,
        network_id: NetworkId,
        network_prefix: u16,
    ) -> Result<AccountFaucetAdvertisement> {
        self.runtime
            .block_on(self.inner.faucet_policy(network_id, network_prefix))?
    }
}
