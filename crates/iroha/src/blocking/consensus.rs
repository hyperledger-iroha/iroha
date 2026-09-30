//! Operator consensus diagnostics on the facade's reusable runtime.

use super::{OperatorClient, RuntimeOwner};
use crate::Result;
use iroha_data_model::block::consensus::SumeragiDiagnosticsStatus;

/// Blocking consensus observations authorized by an explicit operator.
#[derive(Clone, Copy, Debug)]
pub struct Consensus<'a> {
    inner: crate::client::consensus::Consensus<'a>,
    runtime: &'a RuntimeOwner,
}

impl OperatorClient {
    /// Access consensus observations through this operator's reusable runtime.
    #[must_use]
    pub fn consensus(&self) -> Consensus<'_> {
        Consensus {
            inner: self.inner.consensus(),
            runtime: &self.runtime,
        }
    }
}

impl Consensus<'_> {
    /// Read typed consensus diagnostics through the asynchronous implementation.
    ///
    /// # Errors
    /// Returns the operation's structured error or typed async-runtime rejection.
    pub fn diagnostics(&self) -> Result<SumeragiDiagnosticsStatus> {
        self.runtime.block_on(self.inner.diagnostics())?
    }
}
