//! Current native StreamToken signer projection for the admitted account-read capability.

use super::*;
pub use sorafs_manifest::provider_advert::account_read::RegisteredAccountReadV1;
use sorafs_manifest::signer::custody::{SignerCustodyUseContextV1, verify_signer_custody_use_v1};

/// Exact original current native custody index and control record.
/// Token bodies and signer operation/audit journals are not part of this projection.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::Encode,
    norito::Decode,
    iroha_schema::IntoSchema,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields, no_fast_from_json)]
#[norito_schema(
    name = "iroha_data_model::sorafs::provider_admission::discovery::account_read::StreamTokenDiscoveryProofV1"
)]
pub struct StreamTokenDiscoveryProofV1 {
    /// Original complete canonical current-head index frame.
    pub head: Vec<u8>,
    /// Original complete canonical control record, including its enrolled public attestation.
    pub record: Vec<u8>,
}

/// Provider policy and current token key authenticated under one independently selected decision.
/// This grants no provider, signer, account spending, or transaction submission authority.
#[derive(Clone, Debug)]
pub struct VerifiedAccountReadProviderV1 {
    chain_id: String,
    discovery: VerifiedProviderDiscoveryV1,
    policy: RegisteredAccountReadV1,
    key: iroha_crypto::PublicKey,
    key_revision: u32,
    enrollment_expires_at_unix_ms: u64,
}
impl VerifiedAccountReadProviderV1 {
    /// Independently selected chain label checked against the native signer binding.
    #[must_use]
    pub fn chain_id(&self) -> &str {
        &self.chain_id
    }
    /// Exact current admitted provider and signed advertisement.
    #[must_use]
    pub fn discovery(&self) -> &VerifiedProviderDiscoveryV1 {
        &self.discovery
    }
    /// Explicit admitted account-read limits and exact HTTPS origin.
    #[must_use]
    pub fn policy(&self) -> &RegisteredAccountReadV1 {
        &self.policy
    }
    /// Current role-11 token verification key from authenticated native custody.
    #[must_use]
    pub fn token_public_key(&self) -> &iroha_crypto::PublicKey {
        &self.key
    }
    /// Exact token key version.
    #[must_use]
    pub fn token_key_revision(&self) -> u32 {
        self.key_revision
    }
    /// Exclusive signed enrollment expiry, independently verified against the current state.
    #[must_use]
    pub fn enrollment_expires_at_unix_ms(&self) -> u64 {
        self.enrollment_expires_at_unix_ms
    }
}

impl ProviderDiscoveryProofV1 {
    /// Verify the explicit account-read policy and current native signer at the same certified cut.
    ///
    /// The caller supplies the chain, network, qualified schema, decision and current time from
    /// its independently installed registry context. It must enforce its finality freshness policy
    /// before this call. No field or receipt in this response can choose those inputs.
    /// # Errors
    /// Rejects absent policy/custody, altered preimages, wrong scope, expired enrollment,
    /// revocation, malformed original attestation, wrong key generation, and failed admission.
    pub fn verify_account_read(
        &self,
        expected_chain: &str,
        expected_network: NetworkId,
        expected_provider: ProviderId,
        expected_schema: Hash,
        block: &VerifiedSumeragiBlock,
        now_unix_ms: u64,
    ) -> Result<VerifiedAccountReadProviderV1, FinalityError> {
        let discovery = self.verify(
            expected_network,
            expected_provider,
            expected_schema,
            block,
            now_unix_ms / 1000,
        )?;
        let policy =
            RegisteredAccountReadV1::from_capabilities(&discovery.advert.body.capabilities)
                .map_err(map_invalid)?
                .ok_or_else(|| invalid("Provider has no admitted account-read policy"))?;
        let (custody, record) = self.verify_stream_token_control(
            expected_chain,
            expected_network,
            expected_provider,
            block,
            discovery,
        )?;
        let control = &custody.control;
        let binding = &control.policy.binding;
        let verified = verify_signer_custody_use_v1(
            record
                .active_enrollment
                .as_ref()
                .ok_or_else(|| invalid("Token key is not enrolled"))?,
            binding,
            &control.policy.custody_trust(),
            &SignerCustodyUseContextV1 {
                now_unix_ms,
                anchor_observed_at_unix_ms: now_unix_ms,
                current_anchor: custody.anchor,
                active_head: control
                    .active_head
                    .ok_or_else(|| invalid("Token key is not active"))?,
                signer_revoked: control.signer_revoked,
                attester_revoked: control.attester_revoked,
            },
        )
        .map_err(map_invalid)?;
        let key_revision = u32::try_from(binding.key_revision)
            .ok()
            .filter(|n| *n > 0)
            .ok_or_else(|| invalid("Token key revision exceeds its wire field"))?;
        Ok(VerifiedAccountReadProviderV1 {
            chain_id: expected_chain.to_owned(),
            discovery: custody.discovery,
            policy,
            key: binding.public_key.clone(),
            key_revision,
            enrollment_expires_at_unix_ms: verified.statement().expires_at_unix_ms,
        })
    }
}
