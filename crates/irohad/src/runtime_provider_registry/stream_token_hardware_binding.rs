//! Exact public hardware and independent observer routing pins for one stream-token capability.

use super::{IrohaRuntimeProviderRegistryErrorV1, IrohaRuntimeProviderSlotV1};
use iroha_config::parameters::is_production_runtime_handle;
use sorafs_manifest::signer::{
    custody::SignerCustodyBindingV1, stream_token::stream_token_binding_digest_v1,
};

/// Canonical public catalog claim for one provider's configured hardware and observer clients.
///
/// Decoding or validating this value establishes no custody, device, finality or completion
/// authority. The issuer retains independent configuration and verifies challenged signed
/// evidence before release. The digest commits all signer, attester and observer configuration.
#[derive(Clone, Debug, PartialEq, Eq, norito::Decode, norito::Encode)]
pub struct StreamTokenHardwareRuntimeBindingV1 {
    custody: SignerCustodyBindingV1,
    observer_handle: String,
    trust_pins_digest: [u8; 32],
}

impl StreamTokenHardwareRuntimeBindingV1 {
    /// Construct public catalog metadata from independently configured custody and observer pins.
    ///
    /// # Errors
    /// Rejects non-hardware or wrong-role/provider custody, invalid keys/revisions/identities,
    /// unbounded or software observer handles, and a missing full configuration commitment.
    pub fn new(
        custody: SignerCustodyBindingV1,
        observer_handle: String,
        trust_pins_digest: [u8; 32],
    ) -> Result<Self, IrohaRuntimeProviderRegistryErrorV1> {
        let candidate = Self {
            custody,
            observer_handle,
            trust_pins_digest,
        };
        candidate.validate()?;
        Ok(candidate)
    }

    /// Validate structural public pins before catalog use; this is not a qualification probe.
    ///
    /// # Errors
    /// Rejects the same closed binding grammar and resource bounds as [`Self::new`].
    pub fn validate(&self) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
        let invalid = IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
            IrohaRuntimeProviderSlotV1::StreamTokenSigner,
        );
        stream_token_binding_digest_v1(&self.custody).map_err(|_| invalid)?;
        if self.trust_pins_digest == [0; 32]
            || self.observer_handle.len() > 256
            || self.observer_handle == self.custody.runtime_handle
            || !is_production_runtime_handle(&self.observer_handle)
            || self
                .observer_handle
                .to_ascii_lowercase()
                .split(|character: char| !character.is_ascii_alphanumeric())
                .any(|component| component == "software")
        {
            return Err(invalid);
        }
        Ok(())
    }

    /// Exact provider, chain/network, key generation, signing policy and service identities.
    #[must_use]
    pub const fn custody(&self) -> &SignerCustodyBindingV1 {
        &self.custody
    }

    /// Exact independently configured observer routing handle.
    #[must_use]
    pub fn observer_handle(&self) -> &str {
        &self.observer_handle
    }

    /// Commitment to every independently configured signer/attester/observer public trust pin.
    #[must_use]
    pub const fn trust_pins_digest(&self) -> [u8; 32] {
        self.trust_pins_digest
    }

    /// Match the enclosing authenticated catalog's exact chain label and genesis identity.
    ///
    /// # Errors
    /// Rejects a structurally invalid binding or either substituted network coordinate.
    pub fn validate_network(
        &self,
        chain_id: &str,
        network_id: &[u8; 32],
    ) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
        self.validate()?;
        if self.custody.chain_id != chain_id || &self.custody.network_id != network_id {
            return Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
        }
        Ok(())
    }
}

#[cfg(test)]
pub(super) mod tests;
