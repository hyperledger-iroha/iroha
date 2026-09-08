//! Operator configuration reads on the facade's reusable runtime.

use super::{OperatorClient, RuntimeOwner};
use crate::Result;
use iroha_torii_shared::configuration::Configuration as NodeConfiguration;

/// Blocking effective configuration reads with explicit operator authority.
#[derive(Clone, Copy, Debug)]
pub struct Configuration<'a> {
    inner: crate::client::configuration::Configuration<'a>,
    runtime: &'a RuntimeOwner,
}

impl OperatorClient {
    /// Access operator configuration through this facade's reusable runtime.
    #[must_use]
    pub fn configuration(&self) -> Configuration<'_> {
        Configuration {
            inner: self.inner.configuration(),
            runtime: &self.runtime,
        }
    }
}

impl Configuration<'_> {
    /// Read the node's effective configuration with the asynchronous implementation.
    ///
    /// # Errors
    /// Returns the operation's structured error or a typed runtime rejection.
    pub fn get(&self) -> Result<NodeConfiguration> {
        self.runtime.block_on(self.inner.get())?
    }
}
