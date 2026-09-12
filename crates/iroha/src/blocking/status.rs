//! Public status operations on the client's reusable blocking runtime.

use super::{Client, RuntimeOwner};
use crate::Result;
use iroha_torii_shared::status::Status as NodeStatus;

/// Blocking node diagnostics backed by the asynchronous status capability.
#[derive(Clone, Copy, Debug)]
pub struct Status<'a> {
    inner: crate::client::status::Status<'a>,
    runtime: &'a RuntimeOwner,
}

impl Client {
    /// Access public node diagnostics through this facade's reusable runtime.
    #[must_use]
    pub fn status(&self) -> Status<'_> {
        Status {
            inner: self.inner.status(),
            runtime: &self.runtime,
        }
    }
}

impl Status<'_> {
    /// Read one negotiated node status document.
    ///
    /// # Errors
    /// Returns the asynchronous operation error or a typed runtime rejection.
    pub fn get(&self) -> Result<NodeStatus> {
        self.runtime.block_on(self.inner.get())?
    }

    /// Read the node's active API version.
    ///
    /// # Errors
    /// Returns the asynchronous operation error or a typed runtime rejection.
    pub fn version(&self) -> Result<String> {
        self.runtime.block_on(self.inner.version())?
    }
}
