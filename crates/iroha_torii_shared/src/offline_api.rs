//! Public Torii DTOs for the first-release Offline lifecycle.
use iroha_crypto::Hash;
pub use iroha_data_model::offline::{
    KagemushaRecursiveSpendRedeemRequestV4 as OfflineRedeemRequest,
    KagemushaRecursiveSpendTopUpRequestV4 as OfflineTopUpRequest, KagemushaValidationError,
    OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1, OFFLINE_CASH_PAYMENT_MAX_BYTES_V1,
    OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1, OFFLINE_CASH_WIRE_VERSION_V1,
    OFFLINE_REDEEM_REQUEST_SCHEMA_NAME, OFFLINE_TOP_UP_REQUEST_SCHEMA_NAME,
    OfflineActiveRecursiveStepEpVerifier, OfflineActiveRecursiveStepEqVerifier,
    OfflineActiveTopUpShieldVerifier, OfflineActiveTransferVerifier, OfflineActiveUnshieldVerifier,
    OfflineAuthenticatedArtifactSet, OfflineCashAcknowledgementV1, OfflineCashPaymentRequestV1,
    OfflineCashPaymentV1, OfflineReadiness, OfflineReadinessBlocker, OfflineStatus,
    OfflineVerifierId,
};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    asset::AssetDefinitionId,
    block::consensus_v2::{HeightContextId, finality::V2FinalityArtifact},
    bridge::{BridgeFinalityProof, BridgeFinalityVerifier},
    offline::{
        KAGEMUSHA_REQUEST_AUTHORIZATION_MAX_TTL_MS_V2, KagemushaActiveReceiverEntryV1,
        KagemushaActiveReceiverMembershipProofV1, KagemushaActiveReceiverSnapshotStatusV1,
        KagemushaActiveReceiverWitnessProofV1, KagemushaRecipientPaymentRequestV2,
    },
};
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};

/// Exact terminal rejection code for an admitted Offline operation.
pub const OFFLINE_OPERATION_REJECTION_CODE: &str = "offline_operation_rejected";
/// Maximum Unicode scalar count in an Offline operation rejection message.
pub const OFFLINE_OPERATION_REJECTION_MESSAGE_MAX_SCALARS: usize = 1_024;

/// Closed wire value for [`OFFLINE_OPERATION_REJECTION_CODE`].
///
/// The zero-sized Rust representation makes every constructible value valid;
/// the custom JSON and Norito decoders accept only the exact canonical string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OfflineOperationRejectionCode;

impl OfflineOperationRejectionCode {
    /// Return the one canonical wire spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        OFFLINE_OPERATION_REJECTION_CODE
    }
}

impl core::fmt::Display for OfflineOperationRejectionCode {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(OFFLINE_OPERATION_REJECTION_CODE)
    }
}

impl norito::core::NoritoSerialize for OfflineOperationRejectionCode {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        norito::core::NoritoSerialize::serialize(&OFFLINE_OPERATION_REJECTION_CODE, writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        norito::core::NoritoSerialize::encoded_len_hint(&OFFLINE_OPERATION_REJECTION_CODE)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        norito::core::NoritoSerialize::encoded_len_exact(&OFFLINE_OPERATION_REJECTION_CODE)
    }
}

impl<'a> norito::core::NoritoDeserialize<'a> for OfflineOperationRejectionCode {
    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .unwrap_or_else(|error| panic!("invalid Offline rejection code: {error}"))
    }

    fn try_deserialize(
        archived: &'a norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let value =
            <&'a str as norito::core::NoritoDeserialize>::try_deserialize(archived.cast::<&str>())?;
        if value == OFFLINE_OPERATION_REJECTION_CODE {
            Ok(Self)
        } else {
            Err(norito::core::Error::Message(format!(
                "Offline rejection code must be `{OFFLINE_OPERATION_REJECTION_CODE}`"
            )))
        }
    }
}

impl norito::json::JsonSerialize for OfflineOperationRejectionCode {
    fn json_serialize(&self, out: &mut String) {
        norito::json::write_json_string(OFFLINE_OPERATION_REJECTION_CODE, out);
    }
}

impl norito::json::JsonDeserialize for OfflineOperationRejectionCode {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let mut preflight = *parser;
        preflight.skip_string_bounded(OFFLINE_OPERATION_REJECTION_CODE.len())?;
        let value = parser.parse_string()?;
        if value == OFFLINE_OPERATION_REJECTION_CODE {
            Ok(Self)
        } else {
            Err(norito::json::Error::Message(format!(
                "Offline rejection code must be `{OFFLINE_OPERATION_REJECTION_CODE}`"
            )))
        }
    }
}

/// Canonical bounded terminal rejection message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflineOperationRejectionMessage(String);

impl OfflineOperationRejectionMessage {
    /// Validate and construct a rejection message.
    ///
    /// # Errors
    ///
    /// Returns an error unless `message` contains 1 through 1,024 trimmed,
    /// non-control Unicode scalar values.
    pub fn try_new(message: impl Into<String>) -> Result<Self, String> {
        let message = message.into();
        validate_offline_operation_rejection_message(&message)?;
        Ok(Self(message))
    }

    /// Borrow the canonical message.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for OfflineOperationRejectionMessage {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl core::fmt::Display for OfflineOperationRejectionMessage {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl norito::core::NoritoSerialize for OfflineOperationRejectionMessage {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        norito::core::NoritoSerialize::serialize(&self.0, writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        norito::core::NoritoSerialize::encoded_len_hint(&self.0)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        norito::core::NoritoSerialize::encoded_len_exact(&self.0)
    }
}

impl<'a> norito::core::NoritoDeserialize<'a> for OfflineOperationRejectionMessage {
    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .unwrap_or_else(|error| panic!("invalid Offline rejection message: {error}"))
    }

    fn try_deserialize(
        archived: &'a norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let value =
            <&'a str as norito::core::NoritoDeserialize>::try_deserialize(archived.cast::<&str>())?;
        validate_offline_operation_rejection_message(value)
            .map_err(norito::core::Error::Message)?;
        Ok(Self(value.to_owned()))
    }
}

impl norito::json::JsonSerialize for OfflineOperationRejectionMessage {
    fn json_serialize(&self, out: &mut String) {
        norito::json::write_json_string(self.as_str(), out);
    }
}

impl norito::json::JsonDeserialize for OfflineOperationRejectionMessage {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let mut preflight = *parser;
        preflight.skip_string_bounded(OFFLINE_OPERATION_REJECTION_MESSAGE_MAX_SCALARS * 4)?;
        Self::try_new(parser.parse_string()?).map_err(norito::json::Error::Message)
    }
}

/// Exact code-and-message error carried by a rejected Offline operation.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct OfflineOperationRejectionError {
    code: OfflineOperationRejectionCode,
    message: OfflineOperationRejectionMessage,
}

impl OfflineOperationRejectionError {
    /// Construct the exact rejection envelope from a canonical message.
    ///
    /// # Errors
    ///
    /// Returns an error when the message violates the public bound.
    pub fn try_new(message: impl Into<String>) -> Result<Self, String> {
        Ok(Self {
            code: OfflineOperationRejectionCode,
            message: OfflineOperationRejectionMessage::try_new(message)?,
        })
    }

    /// Return the exact stable rejection code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        self.code.as_str()
    }

    /// Borrow the validated rejection message.
    #[must_use]
    pub fn message(&self) -> &str {
        self.message.as_str()
    }
}

fn validate_offline_operation_rejection_message(message: &str) -> Result<(), String> {
    let scalar_count = message
        .chars()
        .take(OFFLINE_OPERATION_REJECTION_MESSAGE_MAX_SCALARS + 1)
        .count();
    if message.is_empty()
        || message.trim() != message
        || scalar_count > OFFLINE_OPERATION_REJECTION_MESSAGE_MAX_SCALARS
        || message.chars().any(char::is_control)
    {
        return Err(format!(
            "Offline rejection message must contain 1..={OFFLINE_OPERATION_REJECTION_MESSAGE_MAX_SCALARS} trimmed, non-control Unicode scalars"
        ));
    }
    Ok(())
}

/// Decode one exact first-release wallet payment request.
///
/// This is the public Torii/API decoding boundary for wallet-to-wallet request
/// bytes. It delegates only to the clean-slate V1 decoder, which checks the
/// outer request cap before reading a Norito header or allocating decoded
/// collections. Legacy Kagemusha V4/V5 values are not compatibility-decoded.
///
/// # Errors
///
/// Returns an error when the body is oversized, malformed, non-canonical,
/// invalid, or does not carry the exact Offline Cash V1 contract.
pub fn decode_offline_cash_payment_request_v1(
    bytes: &[u8],
) -> Result<OfflineCashPaymentRequestV1, KagemushaValidationError> {
    OfflineCashPaymentRequestV1::decode_canonical_exact(bytes)
}

/// Decode one exact first-release sender payment against its retained request.
///
/// The caller-retained request is mandatory context; the boundary does not
/// infer it from transport state. The clean-slate V1 decoder caps the payment
/// bytes before Norito can allocate, validates the signed request, and
/// reconstructs the exact request-owned proof statement. The compact payment
/// intentionally does not echo a second unauthenticated request digest, so
/// semantic request binding is decided by Core's paired-proof verification;
/// the derived payment digest remains context-specific. Legacy Kagemusha V4/V5
/// values are not compatibility-decoded.
///
/// # Errors
///
/// Returns an error when the body is oversized, malformed, non-canonical,
/// invalid, or cannot be structurally contextualized by `request` under the
/// exact Offline Cash V1 contract.
pub fn decode_offline_cash_payment_v1(
    bytes: &[u8],
    request: &OfflineCashPaymentRequestV1,
) -> Result<OfflineCashPaymentV1, KagemushaValidationError> {
    OfflineCashPaymentV1::decode_canonical_exact_against(bytes, request)
}

/// Decode one exact first-release acknowledgement against its retained session.
///
/// Both prior messages are mandatory context. The clean-slate V1 decoder caps
/// acknowledgement bytes before Norito can allocate and then verifies the
/// request, payment, persistence-head, time, and signature bindings. Legacy
/// Kagemusha V4/V5 values are not compatibility-decoded.
///
/// # Errors
///
/// Returns an error when the body is oversized, malformed, non-canonical,
/// invalid, or is not bound to `request` and `payment` under Offline Cash V1.
pub fn decode_offline_cash_acknowledgement_v1(
    bytes: &[u8],
    request: &OfflineCashPaymentRequestV1,
    payment: &OfflineCashPaymentV1,
) -> Result<OfflineCashAcknowledgementV1, KagemushaValidationError> {
    OfflineCashAcknowledgementV1::decode_canonical_exact_against(bytes, request, payment)
}

/// Current portable proof-bearing receiver-lineage layout.
pub const OFFLINE_RECIPIENT_LINEAGE_VERSION: u16 = 2;
/// Maximum number of consecutive finality proofs, including the trusted checkpoint proof.
pub const OFFLINE_RECIPIENT_LINEAGE_MAX_FINALITY_PROOFS: usize = 64;
/// Maximum canonical bytes occupied by the bounded finality chain.
pub const OFFLINE_RECIPIENT_LINEAGE_MAX_FINALITY_CHAIN_BYTES: usize = 3 * 1024 * 1024;
/// Defensive canonical archive bound shared by maintained mobile clients.
pub const OFFLINE_RECIPIENT_LINEAGE_MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
/// Maximum opaque publisher checkpoint envelope carried by a peer receive offer.
pub const OFFLINE_RECIPIENT_OFFER_MAX_PUBLISHER_ENVELOPE_BYTES: usize = 2 * 1024;
/// Maximum canonical offer body accepted by the shared IPM1 Kagemusha profile.
pub const OFFLINE_RECIPIENT_OFFER_MAX_PEER_BYTES: usize = 24_576;
/// Fixed Iroha peer-wire v1 header wrapped around a canonical offer body.
pub const OFFLINE_RECIPIENT_OFFER_PEER_WIRE_HEADER_BYTES: usize = 84;
/// Maximum uncompressed peer-wire message containing one canonical offer.
pub const OFFLINE_RECIPIENT_OFFER_MAX_PEER_WIRE_BYTES: usize =
    OFFLINE_RECIPIENT_OFFER_PEER_WIRE_HEADER_BYTES + OFFLINE_RECIPIENT_OFFER_MAX_PEER_BYTES;
/// Receiver tuple authenticated by a portable registration lineage.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct OfflineRecipientLineageSelectorV2 {
    /// Exact genesis-derived target network.
    pub network_id: NetworkId,
    /// Recipient account.
    pub recipient: AccountId,
    /// Platform device identifier.
    pub receiver_device_id: String,
    /// Offline-cash asset definition.
    pub asset: AssetDefinitionId,
}
/// Reusable proof-bearing active registration lineage for one receiver tuple.
///
/// Authorization comes exclusively from the end-of-block active-receiver
/// snapshot synthetic write and its Commit-QC-authenticated ordinary-write
/// root. Admission transaction metadata inside the active leaf is audit
/// provenance only; no caller may treat a header result root as authorization.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct OfflineRecipientRegistrationLineage {
    /// Portable lineage layout version.
    pub version: u16,
    /// Request-independent tuple authenticated by this reusable proof.
    pub selector: OfflineRecipientLineageSelectorV2,
    /// Unique active entry for the request tuple. Ambiguous entries are never returned.
    pub active_receiver_entry: KagemushaActiveReceiverEntryV1,
    /// Balanced-tree membership path from the active entry to the snapshot commitment.
    pub active_receiver_membership: KagemushaActiveReceiverMembershipProofV1,
    /// Fixed synthetic write and 256-level path to the evaluated ordinary-write root.
    pub active_receiver_witness: KagemushaActiveReceiverWitnessProofV1,
    /// Bounded consecutive chain beginning at the publisher-selected checkpoint.
    pub finality_chain: Vec<BridgeFinalityProof>,
    /// Context id of the last verified proof, suitable for durable checkpoint promotion.
    pub evaluated_context_id: HeightContextId,
    /// Height of the immutable end-of-block state snapshot.
    pub evaluated_block_height: u64,
    /// Canonical hash of the evaluated committed block.
    pub evaluated_block_hash: String,
}
impl OfflineRecipientRegistrationLineage {
    /// Verify every portable binding in this proof-bearing response.
    ///
    /// This is the maintained SDK/native-bridge verification boundary. The
    /// caller supplies an externally trusted checkpoint height/context; the
    /// response is never allowed to choose its own trust root.
    ///
    /// # Errors
    ///
    /// Returns an error when the request, trust anchor, finality chain, active
    /// receiver proofs, or request-lifetime binding is malformed or cannot be
    /// authenticated against the evaluated block.
    #[expect(
        clippy::too_many_lines,
        reason = "the ordered fail-closed checks preserve stable first-error precedence across the public V1 verification boundary"
    )]
    pub fn verify_against(
        &self,
        request: &KagemushaRecipientPaymentRequestV2,
        verified_at_ms: u64,
        trusted_checkpoint_height: u64,
        trusted_checkpoint_context_id: [u8; 32],
    ) -> Result<HeightContextId, String> {
        if self.version != OFFLINE_RECIPIENT_LINEAGE_VERSION
            || verified_at_ms == 0
            || trusted_checkpoint_height == 0
            || self.evaluated_block_height == 0
            || trusted_checkpoint_context_id.iter().all(|byte| *byte == 0)
            || trusted_checkpoint_context_id[31] & 1 == 0
        {
            return Err(
                "unsupported receiver-lineage version or non-canonical verification anchor"
                    .to_owned(),
            );
        }
        request
            .validate_at(verified_at_ms)
            .map_err(|error| format!("recipient request validation failed: {error}"))?;
        if self.selector.network_id != *request.network_id()
            || self.selector.recipient != *request.recipient()
            || self.selector.receiver_device_id != request.receiver_device_id()
            || self.selector.asset != *request.asset()
        {
            return Err("receiver-lineage selector does not match the signed request".to_owned());
        }
        let evaluated_block_hash =
            exact_lower_hex_32("evaluated_block_hash", &self.evaluated_block_hash)?;
        if evaluated_block_hash.iter().all(|byte| *byte == 0) || evaluated_block_hash[31] & 1 == 0 {
            return Err("evaluated block hash is not a canonical Iroha hash".to_owned());
        }
        if self.finality_chain.is_empty()
            || self.finality_chain.len() > OFFLINE_RECIPIENT_LINEAGE_MAX_FINALITY_PROOFS
        {
            return Err("receiver-lineage finality chain is empty or exceeds 64 proofs".to_owned());
        }
        let finality_bytes = norito::core::encoded_frame_len(&self.finality_chain)
            .map_err(|error| format!("finality chain encoding failed: {error}"))?;
        if finality_bytes > OFFLINE_RECIPIENT_LINEAGE_MAX_FINALITY_CHAIN_BYTES {
            return Err("receiver-lineage finality chain exceeds its byte bound".to_owned());
        }
        if self.finality_chain.windows(2).any(|pair| {
            pair[0].finality_artifact.height.checked_add(1)
                != Some(pair[1].finality_artifact.height)
        }) {
            return Err("receiver-lineage finality chain skips or reorders a height".to_owned());
        }
        let trusted_context = HeightContextId(iroha_crypto::HashOf::from_untyped_unchecked(
            Hash::prehashed(trusted_checkpoint_context_id),
        ));
        let trusted_index = self
            .finality_chain
            .iter()
            .position(|proof| proof.finality_artifact.height == trusted_checkpoint_height)
            .ok_or_else(|| {
                "finality chain does not contain the caller's durable checkpoint height".to_owned()
            })?;
        if self.finality_chain[trusted_index]
            .finality_artifact
            .context_id()
            != trusted_context
        {
            return Err(
                "finality chain checkpoint does not match the caller's durable context".to_owned(),
            );
        }
        // The caller-pinned context id authenticates the complete HeightContext,
        // including its exact genesis-derived network identity. Read the identity
        // only after that anchor comparison, then require the signed request and
        // reusable selector to name that same finality security domain.
        let trusted_network_id = self.finality_chain[trusted_index]
            .finality_artifact
            .height_context
            .network_id;
        if self.selector.network_id != trusted_network_id {
            return Err(
                "receiver-lineage selector network does not match the trusted finality network"
                    .to_owned(),
            );
        }
        let mut verifier =
            BridgeFinalityVerifier::with_context(trusted_network_id, trusted_context);
        for proof in &self.finality_chain[trusted_index..] {
            verifier
                .verify(proof)
                .map_err(|error| format!("receiver-lineage finality chain failed: {error}"))?;
        }
        let evaluated = self
            .finality_chain
            .last()
            .expect("non-empty finality chain");
        let artifact: &V2FinalityArtifact = &evaluated.finality_artifact;
        if artifact.height != self.evaluated_block_height
            || artifact.block_hash.as_ref() != &evaluated_block_hash
            || evaluated.block_header.height().get() != artifact.height
            || evaluated.block_header.hash() != artifact.block_hash
            || self.evaluated_context_id != artifact.context_id()
        {
            return Err(
                "finality chain tip does not match the evaluated readiness block".to_owned(),
            );
        }
        let evaluated_at_ms = u64::try_from(evaluated.block_header.creation_time().as_millis())
            .map_err(|_| "evaluated block creation time does not fit u64".to_owned())?;
        if evaluated_at_ms > verified_at_ms
            || verified_at_ms - evaluated_at_ms > KAGEMUSHA_REQUEST_AUTHORIZATION_MAX_TTL_MS_V2
        {
            return Err(
                "receiver-lineage snapshot is from the future or older than five minutes"
                    .to_owned(),
            );
        }
        artifact
            .commit_qc
            .execution_commitment
            .validate()
            .map_err(|error| format!("evaluated execution commitment is invalid: {error}"))?;
        if !self
            .active_receiver_witness
            .verify(artifact.commit_qc.execution_commitment.ordinary_writes_root)
        {
            return Err("active-receiver synthetic write proof is invalid".to_owned());
        }
        let snapshot = self.active_receiver_witness.commitment()?;
        if snapshot.evaluated_height != artifact.height
            || snapshot.evaluated_at_ms != evaluated_at_ms
        {
            return Err("active-receiver snapshot height/time differs from finality".to_owned());
        }
        let KagemushaActiveReceiverSnapshotStatusV1::Available(policy_hash) = snapshot.status
        else {
            return Err("active-receiver snapshot is unavailable".to_owned());
        };
        if !self
            .active_receiver_membership
            .verify(&self.active_receiver_entry, &snapshot)
        {
            return Err("active-receiver membership proof is invalid".to_owned());
        }
        let KagemushaActiveReceiverEntryV1::Active(active) = &self.active_receiver_entry else {
            return Err("ambiguous receiver tuples cannot be routed".to_owned());
        };
        let value = &active.value;
        if active.key.account_id != *request.recipient()
            || active.key.device_id != request.receiver_device_id()
            || active.key.asset_definition_id != *request.asset()
            || value.public_key != *request.receiver_public_key()
            || value
                .registration_hash
                .as_ref()
                .iter()
                .all(|byte| *byte == 0)
            || value.admission_policy_hash != policy_hash
            || value.current_policy_hash != policy_hash
            || value.admission_height == 0
            || value.admission_height > artifact.height
            || value
                .registration_state_hash
                .as_ref()
                .iter()
                .all(|byte| *byte == 0)
            || value
                .admission_transaction_hash
                .as_ref()
                .iter()
                .all(|byte| *byte == 0)
            || !value.account_exists
            || !value.asset_definition_exists
        {
            return Err(
                "active receiver does not match the exact governed request tuple".to_owned(),
            );
        }
        if value.expires_at_ms < request.expires_at_ms()
            || value.expires_at_ms <= evaluated_at_ms
            || value.expires_at_ms <= verified_at_ms
        {
            return Err(
                "active receiver registration does not cover the request lifetime".to_owned(),
            );
        }
        Ok(artifact.context_id())
    }
}
/// Canonical portable peer offer verified without a live Torii connection.
///
/// `publisher_checkpoint_envelope` is opaque to Iroha consensus/native
/// verification. Apps may verify it under their separately pinned publisher
/// key before selecting the trusted checkpoint supplied to `verify_against`.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct OfflineRecipientReceiveOfferV2 {
    /// Offer format version.
    pub version: u16,
    /// Exact later-created signed payment request.
    pub request: KagemushaRecipientPaymentRequestV2,
    /// Publisher-supplied reusable receiver lineage.
    pub lineage: OfflineRecipientRegistrationLineage,
    /// Optional app-publisher checkpoint update envelope, capped at 2 KiB.
    pub publisher_checkpoint_envelope: Option<Vec<u8>>,
}
impl OfflineRecipientReceiveOfferV2 {
    /// Enforce the transport-only structure before app policy verification.
    ///
    /// # Errors
    ///
    /// Returns an error when the offer version, publisher envelope, signed
    /// request, receiver lineage, canonical encoding, or peer-size bound is
    /// invalid.
    pub fn validate_structure(&self) -> Result<(), String> {
        if self.version != OFFLINE_RECIPIENT_LINEAGE_VERSION
            || self.lineage.version != OFFLINE_RECIPIENT_LINEAGE_VERSION
            || self
                .publisher_checkpoint_envelope
                .as_ref()
                .is_some_and(|bytes| {
                    bytes.is_empty()
                        || bytes.len() > OFFLINE_RECIPIENT_OFFER_MAX_PUBLISHER_ENVELOPE_BYTES
                })
        {
            return Err("portable receiver offer version or publisher envelope is invalid".into());
        }
        self.request
            .validate_public_binding()
            .map_err(|error| format!("portable receiver request is invalid: {error}"))?;
        let KagemushaActiveReceiverEntryV1::Active(active) = &self.lineage.active_receiver_entry
        else {
            return Err("portable receiver offer contains an ambiguous receiver tuple".into());
        };
        if self.lineage.selector.network_id != *self.request.network_id()
            || self.lineage.selector.recipient != *self.request.recipient()
            || self.lineage.selector.receiver_device_id != self.request.receiver_device_id()
            || self.lineage.selector.asset != *self.request.asset()
            || active.key.account_id != *self.request.recipient()
            || active.key.device_id != self.request.receiver_device_id()
            || active.key.asset_definition_id != *self.request.asset()
            || active.value.public_key != *self.request.receiver_public_key()
        {
            return Err("portable receiver offer request and reusable lineage do not match".into());
        }
        let encoded = norito::to_bytes(self)
            .map_err(|error| format!("portable receiver offer encoding failed: {error}"))?;
        if encoded.len() > OFFLINE_RECIPIENT_OFFER_MAX_PEER_BYTES {
            return Err(format!(
                "portable receiver offer exceeds the {OFFLINE_RECIPIENT_OFFER_MAX_PEER_BYTES}-byte direct peer limit"
            ));
        }
        Ok(())
    }
    /// Opaque publisher checkpoint bytes for app-owned signature/policy checks.
    #[must_use]
    pub fn publisher_checkpoint_envelope(&self) -> Option<&[u8]> {
        self.publisher_checkpoint_envelope.as_deref()
    }
}
fn exact_lower_hex_32(field: &str, value: &str) -> Result<[u8; 32], String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("{field} must be canonical lowercase 32-byte hex"));
    }
    let mut decoded = [0_u8; 32];
    hex::decode_to_slice(value, &mut decoded)
        .map_err(|_| format!("{field} must be canonical lowercase 32-byte hex"))?;
    Ok(decoded)
}
/// Finalized anchor returned by an applied offline top-up.
///
/// The underlying consensus wire type remains internally versioned, while the
/// first-release public transport surface exposes only this current name.
pub type OfflineTopUpAnchor = iroha_data_model::offline::KagemushaRecursiveSpendTopUpAnchorV4;
/// Finality proof returned with an applied offline top-up.
///
/// The first-release transport exposes the current typed consensus proof
/// directly. It is never wrapped as an opaque base64 payload and is required
/// before a wallet may initialize recursive spending from the returned anchor.
pub type OfflineTopUpFinalityProof = iroha_data_model::offline::KagemushaTopUpFinalityProofV2;
/// Offline lifecycle command selected by an operation.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    JsonDeserialize,
    JsonSerialize,
    NoritoDeserialize,
    NoritoSerialize,
)]
#[norito(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum OfflineOperationKind {
    /// Move online value into an offline spendable note.
    #[norito(rename = "top_up")]
    TopUp,
    /// Move offline value back into an online account.
    #[norito(rename = "redeem")]
    Redeem,
}
/// Initial state returned after an offline command is accepted.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    JsonDeserialize,
    JsonSerialize,
    NoritoDeserialize,
    NoritoSerialize,
)]
#[norito(tag = "state", content = "value", rename_all = "snake_case")]
pub enum OfflineOperationState {
    /// The signed transaction has been accepted for asynchronous processing.
    #[norito(rename = "pending")]
    Pending,
}
/// Reference returned by an accepted offline command.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
pub struct OfflineOperationReference {
    /// Lowercase hexadecimal operation identifier.
    pub operation_id: String,
    /// Offline command kind.
    pub kind: OfflineOperationKind,
    /// Initial operation state.
    pub state: OfflineOperationState,
    /// Canonical signed transaction hash.
    pub transaction_hash: String,
    /// Relative URI of the operation status resource.
    pub status_uri: String,
    /// Signed request issuance time in Unix milliseconds.
    pub submitted_at_ms: u64,
}
/// Final result of an applied top-up operation.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
pub struct OfflineTopUpResult {
    /// Canonical signed transaction hash.
    pub transaction_hash: String,
    /// Finalized block height.
    pub finalized_block_height: u64,
    /// Finalized chain time in Unix milliseconds.
    pub server_time_ms: u64,
    /// Typed finalized top-up anchor consumed by the local wallet prover.
    pub anchor: OfflineTopUpAnchor,
    /// Typed consensus proof bound to the exact finalized top-up anchor.
    pub finality_proof: OfflineTopUpFinalityProof,
}
/// Final result of an applied redemption operation.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
pub struct OfflineRedeemResult {
    /// Canonical signed transaction hash.
    pub transaction_hash: String,
    /// Finalized block height.
    pub finalized_block_height: u64,
    /// Finalized chain time in Unix milliseconds.
    pub server_time_ms: u64,
}
/// Applied offline operation result, discriminated by command kind.
#[expect(
    clippy::large_enum_variant,
    reason = "boxing a result variant would change the canonical public V1 Norito enum wire shape"
)]
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(tag = "kind", content = "result", rename_all = "snake_case")]
pub enum OfflineOperationResult {
    /// Applied top-up result.
    #[norito(rename = "top_up")]
    TopUp(OfflineTopUpResult),
    /// Applied redemption result.
    #[norito(rename = "redeem")]
    Redeem(OfflineRedeemResult),
}
/// Pollable terminal or non-terminal state of an offline operation.
#[expect(
    clippy::large_enum_variant,
    reason = "boxing a status variant would change the canonical public V1 Norito enum wire shape"
)]
#[derive(Debug, Clone, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize)]
#[norito(tag = "state", content = "value", rename_all = "snake_case")]
pub enum OfflineOperationStatus {
    /// The transaction is queued or awaiting finality.
    #[norito(rename = "pending")]
    Pending {
        /// Lowercase hexadecimal operation identifier.
        operation_id: String,
        /// Offline command kind.
        kind: OfflineOperationKind,
        /// Canonical signed transaction hash.
        transaction_hash: String,
        /// Signed request issuance time in Unix milliseconds.
        submitted_at_ms: u64,
    },
    /// The transaction was applied and finalized.
    #[norito(rename = "applied")]
    Applied {
        /// Lowercase hexadecimal operation identifier.
        operation_id: String,
        /// Operation-specific terminal result.
        result: OfflineOperationResult,
    },
    /// The transaction reached a terminal rejection.
    #[norito(rename = "rejected")]
    Rejected {
        /// Lowercase hexadecimal operation identifier.
        operation_id: String,
        /// Offline command kind.
        kind: OfflineOperationKind,
        /// Canonical signed transaction hash.
        transaction_hash: String,
        /// Exact code-and-message rejection; no details extension exists.
        error: OfflineOperationRejectionError,
    },
}
#[cfg(test)]
#[path = "offline_cash_v1_api_tests.rs"]
mod offline_cash_v1_api_tests;
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    #[derive(Debug, JsonDeserialize, JsonSerialize, PartialEq, Eq)]
    struct JsonDefaultByteMappingProbe {
        fixed: [u8; 4],
        dynamic: Vec<u8>,
        keyed: BTreeMap<[u8; 2], u8>,
    }
    #[test]
    fn norito_json_default_byte_and_map_key_mapping_is_exact() {
        let probe = JsonDefaultByteMappingProbe {
            fixed: [0x00, 0xab, 0x10, 0xff],
            dynamic: vec![0x00, 0xab, 0x10, 0xff],
            keyed: BTreeMap::from([([0x00, 0xff], 7)]),
        };
        let json = norito::json::to_string(&probe).expect("encode JSON mapping probe");
        assert_eq!(
            json,
            r#"{"fixed":"00AB10FF","dynamic":[0,171,16,255],"keyed":{"00FF":7}}"#
        );
        let decoded: JsonDefaultByteMappingProbe =
            norito::json::from_str(&json).expect("decode canonical JSON mapping probe");
        assert_eq!(decoded, probe);
        let lowercase: JsonDefaultByteMappingProbe = norito::json::from_str(
            r#"{"fixed":"00ab10ff","dynamic":[0,171,16,255],"keyed":{"00ff":7}}"#,
        )
        .expect("decode lowercase hexadecimal input");
        assert_eq!(lowercase, probe);
        let error = norito::json::from_str::<JsonDefaultByteMappingProbe>(
            r#"{"fixed":"00AB10FF","dynamic":[],"keyed":{"00FF":7,"00ff":8}}"#,
        )
        .expect_err("lexically distinct keys must not alias one typed map key");
        let message = error.to_string();
        assert!(
            message.contains("duplicate field") && message.contains("00ff"),
            "unexpected duplicate-key error: {message}"
        );
    }
    fn universal_capability_status() -> OfflineStatus {
        OfflineStatus {
            mandatory: false,
            cash_handoff_capability:
                iroha_data_model::offline::KAGEMUSHA_CASH_HANDOFF_CAPABILITY_V1.to_owned(),
            required_bridge_abi_version:
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
            max_hops: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2,
            ready: false,
            assets: Vec::new(),
            blockers: iroha_data_model::offline::offline_capability_activation_blockers_v1(),
        }
    }
    #[test]
    fn universal_capability_roundtrips_without_asset_enrollment() {
        let capability = universal_capability_status();
        assert!(!capability.mandatory);
        assert!(!capability.ready);
        assert!(capability.assets.is_empty());
        assert_eq!(
            capability.blockers,
            iroha_data_model::offline::offline_capability_activation_blockers_v1()
        );
        let json = norito::json::to_vec(&capability).expect("encode capability JSON");
        let decoded_json: OfflineStatus =
            norito::json::from_slice(&json).expect("decode capability JSON");
        assert_eq!(decoded_json, capability);
        let archive = norito::to_bytes(&capability).expect("encode capability Norito");
        let decoded_norito: OfflineStatus =
            norito::decode_from_bytes(&archive).expect("decode capability Norito");
        assert_eq!(decoded_norito, capability);
    }
    #[test]
    fn universal_capability_json_is_strict_and_asset_neutral() {
        let canonical = norito::json::to_string(&universal_capability_status())
            .expect("encode universal capability");
        assert!(!canonical.contains("asset_definition_id"));
        assert!(!canonical.contains("verifier"));
        assert!(!canonical.contains("artifact"));
        let unknown = canonical.replacen('{', r#"{"future_metadata":null,"#, 1);
        let error = norito::json::from_str::<OfflineStatus>(&unknown)
            .expect_err("unknown universal capability members fail closed");
        assert_eq!(
            error.to_string(),
            "JSON error: unknown field `future_metadata`"
        );
    }
    #[test]
    fn tagged_json_rejects_duplicate_discriminator_members() {
        for json in [
            r#"{"kind":"top_up","kind":"redeem","value":null}"#,
            r#"{"kind":"top_up","value":null,"value":null}"#,
        ] {
            let error = norito::json::from_str::<OfflineOperationKind>(json)
                .expect_err("duplicate enum envelope members must fail");
            assert!(
                error.to_string().contains("duplicate field"),
                "unexpected duplicate-member error: {error}"
            );
        }
    }
    #[test]
    fn operation_reference_is_direct_and_roundtrips() {
        let reference = OfflineOperationReference {
            operation_id: "11".repeat(32),
            kind: OfflineOperationKind::TopUp,
            state: OfflineOperationState::Pending,
            transaction_hash: "23".repeat(32),
            status_uri: format!("/v1/offline/operations/{}", "11".repeat(32)),
            submitted_at_ms: 1_725_000_000_123,
        };
        let json = norito::json::to_vec(&reference).expect("encode operation reference JSON");
        let json_text = core::str::from_utf8(&json).expect("JSON is UTF-8");
        assert!(!json_text.contains("base64"));
        let decoded_json: OfflineOperationReference =
            norito::json::from_slice(&json).expect("decode operation reference JSON");
        assert_eq!(decoded_json, reference);
        let archive = norito::to_bytes(&reference).expect("encode operation reference Norito");
        let decoded_norito: OfflineOperationReference =
            norito::decode_from_bytes(&archive).expect("decode operation reference Norito");
        assert_eq!(decoded_norito, reference);
    }
    #[test]
    fn operation_reference_json_mapping_is_exact_and_lossless() {
        let operation_id = "11".repeat(32);
        let reference = OfflineOperationReference {
            operation_id: operation_id.clone(),
            kind: OfflineOperationKind::TopUp,
            state: OfflineOperationState::Pending,
            transaction_hash: "23".repeat(32),
            status_uri: format!("/v1/offline/operations/{operation_id}"),
            submitted_at_ms: u64::MAX,
        };
        let json = norito::json::to_string(&reference).expect("encode operation reference JSON");
        assert_eq!(
            json,
            format!(
                concat!(
                    r#"{{"operation_id":"{operation_id}","kind":{{"kind":"top_up","value":null}},"#,
                    r#""state":{{"state":"pending","value":null}},"transaction_hash":"{transaction_hash}","#,
                    r#""status_uri":"/v1/offline/operations/{operation_id}","submitted_at_ms":18446744073709551615}}"#,
                ),
                operation_id = operation_id,
                transaction_hash = "23".repeat(32),
            )
        );
        let decoded: OfflineOperationReference =
            norito::json::from_str(&json).expect("decode lossless operation reference JSON");
        assert_eq!(decoded, reference);
    }
    #[test]
    fn operation_reference_json_rejects_duplicate_declared_fields() {
        let operation_id = "11".repeat(32);
        let json = format!(
            concat!(
                r#"{{"operation_id":"{operation_id}","operation_id":"{operation_id}","#,
                r#""kind":{{"kind":"top_up","value":null}},"state":{{"state":"pending","value":null}},"#,
                r#""transaction_hash":"{transaction_hash}","status_uri":"/v1/offline/operations/{operation_id}","#,
                r#""submitted_at_ms":1}}"#,
            ),
            operation_id = operation_id,
            transaction_hash = "23".repeat(32),
        );
        let error = norito::json::from_str::<OfflineOperationReference>(&json)
            .expect_err("duplicate operation_id must be rejected");
        assert!(error.to_string().contains("duplicate field `operation_id`"));
    }
    #[test]
    fn operation_kind_json_rejects_unknown_tags() {
        let error = norito::json::from_str::<OfflineOperationKind>(
            r#"{"kind":"unknown_command","value":null}"#,
        )
        .expect_err("unknown operation kind must be rejected");
        assert_eq!(error.to_string(), "unknown JSON enum variant");
    }
    #[test]
    fn operation_reference_golden_vector() {
        const EXPECTED_ARCHIVE_HEX: &str = "4e5254300000e8e2244e45e4be2a975e34957141128b00f000000000000000192c80a70843db66024140313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131310400000000040000000041403233323332333233323332333233323332333233323332333233323332333233323332333233323332333233323332333233323332333233323332333233323358572f76312f6f66666c696e652f6f7065726174696f6e732f3131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313108ffffffffffffffff";
        let reference = OfflineOperationReference {
            operation_id: "11".repeat(32),
            kind: OfflineOperationKind::TopUp,
            state: OfflineOperationState::Pending,
            transaction_hash: "23".repeat(32),
            status_uri: format!("/v1/offline/operations/{}", "11".repeat(32)),
            submitted_at_ms: u64::MAX,
        };
        let archive = norito::to_bytes(&reference).expect("encode golden operation reference");
        let archive_hex = hex::encode(archive);
        assert_eq!(archive_hex, EXPECTED_ARCHIVE_HEX);
    }
    #[test]
    fn operation_status_golden_vectors() {
        const PENDING_ARCHIVE_HEX: &str = "4e5254300000fb04214104df1bdcd39249bddd4db23a0096000000000000004ebc9d97c6fd018502000000000000000000000000414031313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131040000000041403233323332333233323332333233323332333233323332333233323332333233323332333233323332333233323332333233323332333233323332333233323308ffffffffffffffff";
        const REJECTED_ARCHIVE_HEX: &str = "4e5254300000fb04214104df1bdcd39249bddd4db23a00b4000000000000003f4069b756096f62020000000000000000020000004140313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131310401000000414032333233323332333233323332333233323332333233323332333233323332333233323332333233323332333233323332333233323332333233323332333233261b1a6f66666c696e655f6f7065726174696f6e5f72656a6563746564090872656a6563746564";
        const APPLIED_REDEEM_ARCHIVE_HEX: &str = "4e5254300000fb04214104df1bdcd39249bddd4db23a00a000000000000000983fb38fbc5b1f410200000000000000000100000041403131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313159010000005441403233323332333233323332333233323332333233323332333233323332333233323332333233323332333233323332333233323332333233323332333233323308ffffffffffffffff082a00000000000000";
        let operation_id = "11".repeat(32);
        let pending = OfflineOperationStatus::Pending {
            operation_id: operation_id.clone(),
            kind: OfflineOperationKind::TopUp,
            transaction_hash: "23".repeat(32),
            submitted_at_ms: u64::MAX,
        };
        let rejected = OfflineOperationStatus::Rejected {
            operation_id: operation_id.clone(),
            kind: OfflineOperationKind::Redeem,
            transaction_hash: "23".repeat(32),
            error: OfflineOperationRejectionError::try_new("rejected")
                .expect("canonical rejection"),
        };
        let applied_redeem = OfflineOperationStatus::Applied {
            operation_id,
            result: OfflineOperationResult::Redeem(OfflineRedeemResult {
                transaction_hash: "23".repeat(32),
                finalized_block_height: u64::MAX,
                server_time_ms: 42,
            }),
        };
        for (expected, status) in [
            (PENDING_ARCHIVE_HEX, pending),
            (REJECTED_ARCHIVE_HEX, rejected),
            (APPLIED_REDEEM_ARCHIVE_HEX, applied_redeem),
        ] {
            let archive = norito::to_bytes(&status).expect("encode golden operation status");
            let archive_hex = hex::encode(archive);
            assert_eq!(archive_hex, expected);
        }
    }

    #[test]
    fn operation_rejection_is_invariant_safe_at_every_public_decode_boundary() {
        const RETIRED_GENERIC_ERROR_ARCHIVE_HEX: &str = "4e5254300000fb04214104df1bdcd39249bddd4db23a00b600000000000000c0eea51f24d353d2020000000000000000020000004140313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131310401000000414032333233323332333233323332333233323332333233323332333233323332333233323332333233323332333233323332333233323332333233323332333233281b1a6f66666c696e655f6f7065726174696f6e5f72656a6563746564090872656a65637465640100";
        let operation_id = "11".repeat(32);
        let valid = OfflineOperationStatus::Rejected {
            operation_id,
            kind: OfflineOperationKind::Redeem,
            transaction_hash: "23".repeat(32),
            error: OfflineOperationRejectionError::try_new("界".repeat(1_024))
                .expect("1,024 Unicode scalars are canonical"),
        };
        let canonical_json = norito::json::to_string(&valid).expect("encode canonical rejection");
        let decoded: OfflineOperationStatus =
            norito::json::from_str(&canonical_json).expect("decode canonical rejection");
        match decoded {
            OfflineOperationStatus::Rejected { error, .. } => {
                assert_eq!(error.code(), OFFLINE_OPERATION_REJECTION_CODE);
                assert_eq!(error.message().chars().count(), 1_024);
            }
            other => panic!("wrong decoded state: {other:?}"),
        }

        for invalid_json in [
            canonical_json.replace(
                OFFLINE_OPERATION_REJECTION_CODE,
                "offline_operation_rejecteD",
            ),
            canonical_json.replace(&"界".repeat(1_024), " rejected"),
            canonical_json.replace(
                &format!(r#""message":"{}""#, "界".repeat(1_024)),
                &format!(r#""message":"{}","details":null"#, "界".repeat(1_024)),
            ),
            canonical_json.replace(&"界".repeat(1_024), &"界".repeat(1_025)),
        ] {
            norito::json::from_str::<OfflineOperationStatus>(&invalid_json)
                .expect_err("a non-canonical rejection JSON value must fail closed");
        }

        let mut invalid_code_archive = norito::to_bytes(&OfflineOperationStatus::Rejected {
            operation_id: "11".repeat(32),
            kind: OfflineOperationKind::Redeem,
            transaction_hash: "23".repeat(32),
            error: OfflineOperationRejectionError::try_new("rejected")
                .expect("canonical rejection"),
        })
        .expect("encode canonical rejection archive");
        let code = OFFLINE_OPERATION_REJECTION_CODE.as_bytes();
        let offset = invalid_code_archive
            .windows(code.len())
            .position(|window| window == code)
            .expect("code bytes in canonical archive");
        invalid_code_archive[offset + code.len() - 1] = b'D';
        norito::decode_from_bytes::<OfflineOperationStatus>(&invalid_code_archive)
            .expect_err("a non-canonical rejection code must fail Norito decoding");

        let retired = hex::decode(RETIRED_GENERIC_ERROR_ARCHIVE_HEX)
            .expect("decode retired generic-error archive");
        norito::decode_from_bytes::<OfflineOperationStatus>(&retired)
            .expect_err("the retired generic ErrorEnvelope wire shape must fail closed");

        for invalid_message in [
            String::new(),
            " rejected".to_owned(),
            "rejected\n".to_owned(),
            "\u{85}".to_owned(),
            "界".repeat(1_025),
        ] {
            OfflineOperationRejectionError::try_new(invalid_message)
                .expect_err("a non-canonical rejection message must be unconstructible");
        }
    }
}
