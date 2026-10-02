//! Immutable native stream-token delivery recipes and permanent dispositions.
//!
//! Shape validation never establishes source execution, governance, finality or current authority.
//! Core creates each recipe atomically with its original gateway admission. Local custody may
//! reconstruct its exact payload; it may never choose a new lifetime, fee payer or transaction id.

use std::num::{NonZeroU32, NonZeroU64};

use iroha_crypto::{Algorithm, Hash};
use iroha_model_base::metadata::Metadata;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use super::{
    REPUTATION_JOURNAL_MAX_SOURCE_AGE_MS_V1, ReputationJournalAuthorityPolicyRecordV1,
    ReputationJournalEntryV1, ReputationJournalPayloadV1,
};
use crate::{
    DeriveJsonDeserialize, DeriveJsonSerialize, NetworkId,
    isi::sorafs::AppendSorafsStreamTokenReputationJournalEntry,
    sorafs::stream_token_gateway::{
        StreamTokenGatewayAdmissionRecordV1 as Record,
        native::StreamTokenGatewayExecutionV1 as Execution,
    },
    transaction::{
        Executable, FeePaymentIntent, TransactionBuilder, TransactionDomain, TransactionPayload,
    },
};

/// Hard number of exact gateway identities delegated recorder fee authority by one policy.
pub const STREAM_TOKEN_REPUTATION_MAX_GATEWAYS_V1: usize = 16;
/// Hard canonical encoded size of one governed delivery template.
pub const STREAM_TOKEN_REPUTATION_MAX_TEMPLATE_BYTES_V1: usize = 16 * 1024;
/// Hard canonical encoded size of one exact unsigned append payload.
pub const STREAM_TOKEN_REPUTATION_MAX_PAYLOAD_BYTES_V1: usize = 32 * 1024;
/// Hard canonical encoded size of one immutable source-bound delivery intent.
pub const STREAM_TOKEN_REPUTATION_MAX_INTENT_BYTES_V1: usize = 64 * 1024;
/// Finite maximum number of blocks from source execution to append expiry.
pub const STREAM_TOKEN_REPUTATION_MAX_HEIGHT_TTL_V1: u64 = 65_536;

/// Governed exact fee authority and finite lifetime for approved native gateway sources.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(
    name = "iroha_data_model::sorafs::reputation::stream_token_delivery::StreamTokenReputationDeliveryTemplateV1"
)]
pub struct StreamTokenReputationDeliveryTemplateV1 {
    /// Strictly increasing, nonzero exact gateway identifiers; an empty list authorizes none.
    #[norito(json = "crate::json_helpers::fixed_bytes::vec")]
    pub allowed_gateways: Vec<[u8; 32]>,
    /// Exact signed fee payer, charge ceilings and gas allowance, chosen by governance.
    pub fee_payment: FeePaymentIntent,
    /// Finite maximum lifetime beginning at the authenticated source execution's block time.
    pub time_to_live_ms: u64,
    /// Finite inclusive height lifetime beginning at the authenticated source execution height.
    pub height_ttl: u64,
}
impl Default for StreamTokenReputationDeliveryTemplateV1 {
    /// A closed template grants no gateway a recorder fee-spending capability.
    fn default() -> Self {
        Self {
            allowed_gateways: Vec::new(),
            fee_payment: FeePaymentIntent::authority(Vec::new(), None),
            time_to_live_ms: 60_000,
            height_ttl: 128,
        }
    }
}
impl StreamTokenReputationDeliveryTemplateV1 {
    /// Validate bounded canonical shape without granting a gateway or signer any authority.
    ///
    /// # Errors
    /// Rejects oversized, duplicate, unordered or inert identities and unsafe fee/lifetime fields.
    pub fn validate(&self) -> Result<(), StreamTokenReputationDeliveryErrorV1> {
        let invalid = StreamTokenReputationDeliveryErrorV1::Invalid;
        if norito::canonical_frame_len(self).map_err(|_| invalid)?
            > STREAM_TOKEN_REPUTATION_MAX_TEMPLATE_BYTES_V1
            || self.allowed_gateways.len() > STREAM_TOKEN_REPUTATION_MAX_GATEWAYS_V1
            || self.allowed_gateways.contains(&[0; 32])
            || !self
                .allowed_gateways
                .windows(2)
                .all(|pair| pair[0] < pair[1])
            || !(1..=REPUTATION_JOURNAL_MAX_SOURCE_AGE_MS_V1).contains(&self.time_to_live_ms)
            || !(1..=STREAM_TOKEN_REPUTATION_MAX_HEIGHT_TTL_V1).contains(&self.height_ttl)
        {
            return Err(invalid);
        }
        self.fee_payment.validate().map_err(|_| invalid)
    }
}

/// Closed reasons for explicit journal-policy-manager cancellation of an undelivered source.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(
    tag = "reason",
    content = "detail",
    rename_all = "snake_case",
    deny_unknown_fields
)]
#[norito_schema(
    name = "iroha_data_model::sorafs::reputation::stream_token_delivery::StreamTokenReputationCancellationReasonV1"
)]
pub enum StreamTokenReputationCancellationReasonV1 {
    /// Governance determined the original recorder credential is unavailable.
    CredentialUnavailable,
    /// Governance withdrew the original source's permission to incur a journal fee.
    AuthorityWithdrawn,
}

/// Canonical complete source-bound unsigned append recipe; the native owner authenticates it.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(
    name = "iroha_data_model::sorafs::reputation::stream_token_delivery::StreamTokenReputationDeliveryIntentV1"
)]
pub struct StreamTokenReputationDeliveryIntentV1 {
    /// Exact genesis-derived network.
    pub network_id: NetworkId,
    /// Original gateway record, including its physical serving attempt and original policy.
    pub record: Record,
    /// Commitment to the complete original gateway request, including its serving attempt.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub request_digest: [u8; 32],
    /// Native direct execution that created both the original source and this intent.
    pub source_execution: Execution,
    /// Exact source-time recorder policy retained from the authenticated native history.
    pub recorder_policy: ReputationJournalAuthorityPolicyRecordV1,
    /// Governed transaction TTL maximum read from the same source execution's State.
    pub transaction_ttl_limit_ms: u64,
    /// Complete original unsigned payload. No recovery action may alter any of these bytes.
    pub payload: TransactionPayload,
}

/// A permanent terminal delivery fact. Pending is an explicit native row, not inferred absence.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(
    tag = "state",
    content = "detail",
    rename_all = "snake_case",
    deny_unknown_fields
)]
#[norito_schema(
    name = "iroha_data_model::sorafs::reputation::stream_token_delivery::StreamTokenReputationDeliveryDispositionV1"
)]
pub enum StreamTokenReputationDeliveryDispositionV1 {
    /// The immutable counted intent exists and has no terminal transition.
    Pending,
    /// The exact original append succeeded and owns its permanent journal event.
    Delivered {
        /// Native global journal sequence allocated by the actual append execution.
        journal_sequence: u64,
        /// Canonical journal entry identity, independently checked against the exact intent.
        event_id: super::ReputationJournalEventIdV1,
        /// Exact successful direct signed append execution, not an event-list ordinal.
        execution: Execution,
    },
    /// The original gateway status was Excluded; this is never a counted journal success.
    Excluded,
    /// Consensus time or height exceeded the original finite lifetime before any append.
    Expired {
        /// Exact gateway acknowledgement execution that recorded expiry and drained the row.
        execution: Execution,
    },
    /// An authorized journal-policy manager permanently cancelled an undelivered intent.
    GovernanceCancelled {
        /// Exact active recorder-policy digest checked at cancellation.
        #[norito(json = "crate::json_helpers::fixed_bytes")]
        recorder_policy_digest: [u8; 32],
        /// Exact gateway qualification whose CAS was authenticated by the cancelling execution.
        gateway_qualification:
            crate::sorafs::stream_token_gateway::StreamTokenGatewayAdmissionQualificationV1,
        /// Closed governed cancellation reason.
        reason: StreamTokenReputationCancellationReasonV1,
        /// Exact successful direct signed governance cancellation execution.
        execution: Execution,
    },
}

/// Payload-free structural recipe failure; values do not constitute authority proofs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StreamTokenReputationDeliveryErrorV1 {
    /// A shape, bounded-size, arithmetic or canonical-encoding check failed.
    #[error("invalid native stream-token reputation delivery recipe")]
    Invalid,
    /// Original source, policy, recorder, lifetime or complete payload was substituted.
    #[error("native stream-token reputation delivery binding mismatch")]
    BindingMismatch,
}
use StreamTokenReputationDeliveryErrorV1 as Error;

fn commitment<T: norito::core::NoritoSerialize>(
    domain: &[u8],
    value: &T,
) -> Result<[u8; 32], Error> {
    let bytes = norito::encode_canonical(value).map_err(|_| Error::Invalid)?;
    let mut material = Vec::with_capacity(domain.len() + bytes.len());
    material.extend_from_slice(domain);
    material.extend_from_slice(&bytes);
    Ok(*Hash::new(material).as_ref())
}

impl StreamTokenReputationDeliveryIntentV1 {
    /// Derive one exact unsigned recipe from authenticated source inputs.
    ///
    /// Core must supply every argument from the same native source transition. This method only
    /// validates shape; even a correctly derived value cannot authorize signing or queue exposure.
    ///
    /// # Errors
    /// Rejects unapproved gateways, non-counted outcomes, unsupported recorder keys or bounds.
    pub fn derive(
        network_id: NetworkId,
        record: Record,
        request_digest: [u8; 32],
        source_execution: Execution,
        recorder_policy: ReputationJournalAuthorityPolicyRecordV1,
        transaction_ttl_limit_ms: u64,
    ) -> Result<Self, Error> {
        let payload = Self::derive_payload(
            network_id,
            &record,
            request_digest,
            &source_execution,
            &recorder_policy,
            transaction_ttl_limit_ms,
        )?;
        let value = Self {
            network_id,
            record,
            request_digest,
            source_execution,
            recorder_policy,
            transaction_ttl_limit_ms,
            payload,
        };
        value.validate()?;
        Ok(value)
    }

    fn derive_payload(
        network_id: NetworkId,
        record: &Record,
        request_digest: [u8; 32],
        source: &Execution,
        policy: &ReputationJournalAuthorityPolicyRecordV1,
        transaction_ttl_limit_ms: u64,
    ) -> Result<TransactionPayload, Error> {
        record
            .validate_shape(record.admitted_under)
            .map_err(|_| Error::Invalid)?;
        policy.validate().map_err(|_| Error::Invalid)?;
        let template = &policy.policy.stream_token_delivery;
        template.validate()?;
        let recorder = &policy.policy.token_recorder_authority;
        if network_id.as_bytes()[31] & 1 == 0
            || network_id.as_bytes() == Hash::prehashed([0; 32]).as_ref()
            || request_digest == [0; 32]
            || source.height <= 1
            || source.transaction_hash == [0; 32]
            || source.recorded_at_unix_ms == 0
            || source.recorded_at_unix_ms == u64::MAX
            || record.outcome.validated_at_unix_ms > source.recorded_at_unix_ms
            || policy.activated_at_unix_ms > record.outcome.validated_at_unix_ms
            || !record.outcome.status.counts_for_provider()
            || template
                .allowed_gateways
                .binary_search(&record.admitted_under.gateway_id)
                .is_err()
            || recorder
                .try_signatory()
                .is_none_or(|key| key.algorithm() != Algorithm::Ed25519)
            || transaction_ttl_limit_ms == 0
        {
            return Err(Error::BindingMismatch);
        }
        let source_expiry = record
            .outcome
            .validated_at_unix_ms
            .checked_add(policy.policy.max_source_age_ms)
            .ok_or(Error::Invalid)?;
        let ttl = source_expiry
            .checked_sub(source.recorded_at_unix_ms)
            .ok_or(Error::BindingMismatch)?
            .min(template.time_to_live_ms)
            .min(transaction_ttl_limit_ms);
        let ttl = NonZeroU64::new(ttl).ok_or(Error::BindingMismatch)?;
        let expires_at_height = source
            .height
            .checked_add(template.height_ttl)
            .ok_or(Error::Invalid)?;
        let entry = ReputationJournalEntryV1::try_new(
            record.provider_id,
            policy.policy_digest,
            recorder.clone(),
            record.outcome.validated_at_unix_ms,
            None,
            ReputationJournalPayloadV1::StreamTokenValidation(record.outcome),
        )
        .map_err(|_| Error::Invalid)?;
        // This id precedes the payload and complete intent commitment; it cannot self-reference.
        let recipe_id = commitment(
            b"iroha.sorafs.token-reputation.recipe.v1\0",
            &(
                network_id,
                *record,
                request_digest,
                source.clone(),
                policy.policy_digest,
                transaction_ttl_limit_ms,
            ),
        )?;
        let nonce = NonZeroU32::new(
            u32::from_be_bytes(recipe_id[..4].try_into().map_err(|_| Error::Invalid)?) | 1,
        )
        .ok_or(Error::Invalid)?;
        let mut metadata = Metadata::default();
        metadata.insert(
            "stream_token_reputation_recipe"
                .parse()
                .map_err(|_| Error::Invalid)?,
            iroha_primitives::json::Json::new(hex::encode(recipe_id)),
        );
        metadata.insert(
            "expires_at_height".parse().map_err(|_| Error::Invalid)?,
            iroha_primitives::json::Json::new(expires_at_height),
        );
        let payload = TransactionPayload {
            domain: TransactionDomain::Network(network_id),
            authority: recorder.clone(),
            creation_time_ms: source.recorded_at_unix_ms,
            instructions: Executable::Instructions(
                vec![AppendSorafsStreamTokenReputationJournalEntry::new(entry).into()].into(),
            ),
            time_to_live_ms: Some(ttl),
            nonce: Some(nonce),
            fee_payment: template.fee_payment.clone(),
            metadata,
            attachments: None,
        };
        let _ = TransactionBuilder::from_payload(payload.clone()).map_err(|_| Error::Invalid)?;
        if norito::canonical_frame_len(&payload).map_err(|_| Error::Invalid)?
            > STREAM_TOKEN_REPUTATION_MAX_PAYLOAD_BYTES_V1
        {
            return Err(Error::Invalid);
        }
        Ok(payload)
    }

    /// Reject any change to the original source-derived complete unsigned payload.
    ///
    /// # Errors
    /// Rejects malformed inputs, oversized frames or any original recipe substitution.
    pub fn validate(&self) -> Result<(), Error> {
        if norito::canonical_frame_len(self).map_err(|_| Error::Invalid)?
            > STREAM_TOKEN_REPUTATION_MAX_INTENT_BYTES_V1
        {
            return Err(Error::Invalid);
        }
        let expected = Self::derive_payload(
            self.network_id,
            &self.record,
            self.request_digest,
            &self.source_execution,
            &self.recorder_policy,
            self.transaction_ttl_limit_ms,
        )?;
        if self.payload != expected {
            return Err(Error::BindingMismatch);
        }
        Ok(())
    }

    /// Commit to the complete immutable source and unsigned payload without conferring authority.
    ///
    /// # Errors
    /// Rejects an invalid or oversized recipe before computing its canonical commitment.
    pub fn digest(&self) -> Result<[u8; 32], Error> {
        self.validate()?;
        commitment(b"iroha.sorafs.token-reputation.intent.v1\0", self)
    }

    /// Determine expiry using authenticated consensus coordinates, with inclusive transaction TTL.
    ///
    /// # Errors
    /// Rejects malformed original recipes or invalid supplied coordinates.
    pub fn expired_at(&self, height: u64, now_unix_ms: u64) -> Result<bool, Error> {
        self.validate()?;
        if height < self.source_execution.height
            || now_unix_ms < self.source_execution.recorded_at_unix_ms
        {
            return Err(Error::BindingMismatch);
        }
        let end_time = self
            .payload
            .creation_time_ms
            .checked_add(self.payload.time_to_live_ms.ok_or(Error::Invalid)?.get())
            .ok_or(Error::Invalid)?;
        let end_height = self
            .source_execution
            .height
            .checked_add(self.recorder_policy.policy.stream_token_delivery.height_ttl)
            .ok_or(Error::Invalid)?;
        Ok(now_unix_ms > end_time || height > end_height)
    }
}

#[cfg(test)]
mod tests;
