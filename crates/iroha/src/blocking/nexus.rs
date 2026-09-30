//! Public Nexus reads through the facade's reusable asynchronous runtime.

use super::{Client, RuntimeOwner};
use crate::Result;
use iroha_data_model::nexus::{PublicLanePreparationRequestV1, PublicLanePreparationV1};

/// Blocking public Nexus reads using the asynchronous implementation.
#[derive(Clone, Copy, Debug)]
pub struct Nexus<'a> {
    inner: crate::client::nexus::Nexus<'a>,
    runtime: &'a RuntimeOwner,
}

impl Client {
    /// Access public Nexus operations through this facade's reusable runtime.
    #[must_use]
    pub fn nexus(&self) -> Nexus<'_> {
        Nexus {
            inner: self.inner.nexus(),
            runtime: &self.runtime,
        }
    }
}

impl Nexus<'_> {
    /// Read an observational validator committee status.
    ///
    /// Both finality attachments require independent native-chain and genesis
    /// authentication before the returned committee can authorize consensus.
    ///
    /// # Errors
    /// Returns structured transport, response-bound, decoding, response-binding
    /// or blocking-runtime errors.
    pub fn validator_committee(
        &self,
        target_epoch: Option<u64>,
    ) -> crate::Result<iroha_data_model::nexus::ValidatorCommitteeStatusV1> {
        self.runtime
            .block_on(self.inner.validator_committee(target_epoch))?
    }

    /// Prepare exact staking inputs using the asynchronous SDK implementation.
    ///
    /// This read is an observation; execution checks the exact signed effects.
    ///
    /// # Errors
    /// Returns the operation's structured error or a typed runtime rejection.
    pub fn prepare_public_lane_plan(
        &self,
        request: &PublicLanePreparationRequestV1,
    ) -> Result<PublicLanePreparationV1> {
        self.runtime
            .block_on(self.inner.prepare_public_lane_plan(request))?
    }
}
