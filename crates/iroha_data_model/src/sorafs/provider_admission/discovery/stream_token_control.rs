//! Current native StreamToken control for independently bound enrollment preparation.
//!
//! Reading committed policy is separate from enrollment and current signing eligibility.
//! Configured, rotated, expired and revoked controls remain inspectable at a certified cut.

use super::*;
use crate::sorafs::stream_token_custody::StreamTokenCustodyControlRecordV1;
use sorafs_manifest::signer::{
    custody::{SignerCustodyAnchorV1, SignerCustodyBindingV1},
    custody_control::SignerCustodyControlStateV1,
};

/// Current control authenticated under one independently selected native decision and binding.
///
/// This is public control evidence for preparing enrollment. It grants no signing, enrollment,
/// provider, account spending, transaction submission or download authority. The caller must
/// separately enforce policy, revocation, freshness and eligibility for its intended operation.
#[derive(Clone, Debug)]
pub struct VerifiedStreamTokenCustodyControlV1 {
    pub(super) discovery: VerifiedProviderDiscoveryV1,
    pub(super) control: SignerCustodyControlStateV1,
    revision: u64,
    pub(super) anchor: SignerCustodyAnchorV1,
}

impl VerifiedStreamTokenCustodyControlV1 {
    /// Current admitted provider and signed advertisement at the same certified cut.
    #[must_use]
    pub fn discovery(&self) -> &VerifiedProviderDiscoveryV1 {
        &self.discovery
    }

    /// Exact committed policy, next sequence, predecessor, enrollment head and revocations.
    /// These facts do not assert that enrollment or signing is currently permitted.
    #[must_use]
    pub fn control(&self) -> &SignerCustodyControlStateV1 {
        &self.control
    }

    /// Current native control revision authenticated by the head and height index.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Approval anchor binding this native control record to the independently selected decision.
    /// The state digest is the original control record digest, not its enrollment digest.
    #[must_use]
    pub fn anchor(&self) -> SignerCustodyAnchorV1 {
        self.anchor
    }
}

impl ProviderDiscoveryProofV1 {
    /// Authenticate current native signer control without requiring an active enrollment.
    ///
    /// The caller independently supplies the exact complete signer binding, network, qualified
    /// native schema, certified decision and current time. It must enforce finality freshness
    /// before this call; no response field selects those trust inputs. The provider admission
    /// and advert must be current. Custody policy expiry and revocation remain readable facts,
    /// allowing enrollment preparation and diagnosis without granting signing eligibility.
    ///
    /// # Errors
    /// Rejects missing or malformed control, changed preimages, wrong exact binding or native
    /// scope, a noncurrent control head, and failed provider admission or advertisement checks.
    pub fn verify_stream_token_custody_control(
        &self,
        expected_network: NetworkId,
        expected_provider: ProviderId,
        expected_schema: Hash,
        expected_binding: &SignerCustodyBindingV1,
        block: &VerifiedSumeragiBlock,
        now_unix_ms: u64,
    ) -> Result<VerifiedStreamTokenCustodyControlV1, FinalityError> {
        expected_binding.validate().map_err(map_invalid)?;
        let discovery = self.verify(
            expected_network,
            expected_provider,
            expected_schema,
            block,
            now_unix_ms / 1000,
        )?;
        let (verified, _) = self.verify_stream_token_control(
            &expected_binding.chain_id,
            expected_network,
            expected_provider,
            block,
            discovery,
        )?;
        if verified.control.policy.binding != *expected_binding {
            return Err(invalid(
                "Token custody differs from the independently selected binding",
            ));
        }
        Ok(verified)
    }

    pub(super) fn verify_stream_token_control(
        &self,
        expected_chain: &str,
        expected_network: NetworkId,
        expected_provider: ProviderId,
        block: &VerifiedSumeragiBlock,
        discovery: VerifiedProviderDiscoveryV1,
    ) -> Result<
        (
            VerifiedStreamTokenCustodyControlV1,
            StreamTokenCustodyControlRecordV1,
        ),
        FinalityError,
    > {
        let proof = self
            .stream_token
            .as_ref()
            .ok_or_else(|| invalid("Provider has no current token custody projection"))?;
        let world = self.world.authenticate(block)?;
        let verified = proof.verify_current(
            &world,
            expected_chain,
            expected_network,
            expected_provider,
            block,
        )?;
        Ok((
            VerifiedStreamTokenCustodyControlV1 {
                discovery,
                control: verified.control,
                revision: verified.record.revision,
                anchor: verified.anchor,
            },
            verified.record,
        ))
    }
}
