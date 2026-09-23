//! Data-availability reads using the client's reusable blocking runtime.

use super::{AccountClient, Client, RuntimeOwner};
use crate::Result;
use iroha_data_model::da::types::StorageTicketId;
use iroha_data_model::da::{
    commitment::{DaCommitmentProof, DaProofPolicyBundle},
    pin_intent::DaPinIntentProof,
};
use iroha_torii_shared::da::DaManifestResponse;
use iroha_torii_shared::da::{
    DaCommitmentListRequest, DaCommitmentListResponse, DaCommitmentProofRequest,
    DaCommitmentProofResponse, DaCommitmentVerifyResponse, DaPinIntentListRequest,
    DaPinIntentListResponse, DaPinIntentQueryRequest, DaPinIntentVerifyResponse,
};

/// Blocking data-availability reads backed by the asynchronous capability.
#[derive(Clone, Copy, Debug)]
pub struct DataAvailability<'a> {
    inner: crate::client::data_availability::DataAvailability<'a>,
    runtime: &'a RuntimeOwner,
}

impl Client {
    /// Access public data-availability records through this facade's reusable runtime.
    #[must_use]
    pub fn da(&self) -> DataAvailability<'_> {
        DataAvailability {
            inner: self.inner.da(),
            runtime: &self.runtime,
        }
    }
}

impl DataAvailability<'_> {
    /// Discover the active proof policies.
    /// # Errors
    /// Returns the asynchronous operation error or a typed runtime rejection.
    pub fn proof_policies(&self) -> Result<DaProofPolicyBundle> {
        self.runtime.block_on(self.inner.proof_policies())?
    }
    /// Read a bounded commitment page.
    /// # Errors
    /// Returns the asynchronous operation error or a typed runtime rejection.
    pub fn commitments(
        &self,
        request: &DaCommitmentListRequest,
    ) -> Result<DaCommitmentListResponse> {
        self.runtime.block_on(self.inner.commitments(request))?
    }
    /// Read a bounded pin-intent page.
    /// # Errors
    /// Returns the asynchronous operation error or a typed runtime rejection.
    pub fn pin_intents(&self, request: &DaPinIntentListRequest) -> Result<DaPinIntentListResponse> {
        self.runtime.block_on(self.inner.pin_intents(request))?
    }
    /// Fetch the canonical manifest response bound to one storage ticket.
    ///
    /// # Errors
    /// Returns the asynchronous operation error or a typed runtime rejection.
    pub fn manifest(&self, ticket: &StorageTicketId) -> Result<DaManifestResponse> {
        self.runtime.block_on(self.inner.manifest(ticket))?
    }
}

/// Account-authorized DA proofs using the facade's reusable runtime.
#[derive(Clone, Copy, Debug)]
pub struct AccountDataAvailability<'a> {
    inner: crate::client::data_availability::AccountDataAvailability<'a>,
    runtime: &'a RuntimeOwner,
}

impl AccountClient {
    /// Access account-authorized DA proof operations.
    #[must_use]
    pub fn da(&self) -> AccountDataAvailability<'_> {
        AccountDataAvailability {
            inner: self.inner.da(),
            runtime: &self.runtime,
        }
    }
}

impl AccountDataAvailability<'_> {
    /// Request an exact commitment proof.
    /// # Errors
    /// Returns the asynchronous operation error or a typed runtime rejection.
    pub fn prove_commitment(
        &self,
        request: &DaCommitmentProofRequest,
    ) -> Result<Option<DaCommitmentProofResponse>> {
        self.runtime
            .block_on(self.inner.prove_commitment(request))?
    }
    /// Ask Torii to verify a commitment proof.
    /// # Errors
    /// Returns the asynchronous operation error or a typed runtime rejection.
    pub fn verify_commitment(
        &self,
        proof: &DaCommitmentProof,
    ) -> Result<DaCommitmentVerifyResponse> {
        self.runtime.block_on(self.inner.verify_commitment(proof))?
    }
    /// Request an exact pin-intent proof.
    /// # Errors
    /// Returns the asynchronous operation error or a typed runtime rejection.
    pub fn prove_pin_intent(
        &self,
        request: &DaPinIntentQueryRequest,
    ) -> Result<Option<DaPinIntentProof>> {
        self.runtime
            .block_on(self.inner.prove_pin_intent(request))?
    }
    /// Ask Torii to verify a pin-intent proof.
    /// # Errors
    /// Returns the asynchronous operation error or a typed runtime rejection.
    pub fn verify_pin_intent(&self, proof: &DaPinIntentProof) -> Result<DaPinIntentVerifyResponse> {
        self.runtime.block_on(self.inner.verify_pin_intent(proof))?
    }
}
