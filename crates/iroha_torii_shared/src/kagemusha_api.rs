//! Public Torii DTO boundary for KAGEMUSHA V1.
//!
//! KAGEMUSHA V1 peer-handoff and chain-operation types are defined once in
//! `iroha_data_model` and re-exported here without transport wrappers. Every
//! binary ingress helper installs a byte ceiling before canonical Norito
//! decoding. Applied KAGEMUSHA V1 results additionally require a
//! caller-pinned consensus context; an untrusted response can never select the
//! trust root used to validate its own finality proof.
//!
//! An applied top-up response is an idempotent join of the immutable consensus
//! intent/receipt with the durable local finality-and-mint outbox. This DTO
//! boundary does not imply a post-finality mutation of world state.

use iroha_data_model::{
    NetworkId,
    block::consensus_v2::HeightContextId,
    isi::kagemusha_v1::TopUpKagemushaV1,
    transaction::{Executable, SignedTransaction, TransactionAdmissionIntent},
};
pub use iroha_data_model::{
    isi::kagemusha_v1::{
        KAGEMUSHA_CHAIN_VERSION_V1, KAGEMUSHA_REDEMPTION_REQUEST_SCHEMA_NAME_V1,
        KAGEMUSHA_TOP_UP_REQUEST_SCHEMA_NAME_V1, KagemushaFinalityTrustAnchorV1,
        KagemushaIsiValidationErrorV1, KagemushaOperationFinalityV1, KagemushaOperationKindV1,
        KagemushaOperationLookupV1, KagemushaOperationRejectionCodeV1,
        KagemushaOperationRejectionV1, KagemushaOperationResultV1, KagemushaOperationStateV1,
        KagemushaOperationStatusV1, KagemushaRedemptionRequestV1, KagemushaRedemptionResultV1,
        KagemushaReserveReceiptV1, KagemushaReserveReceiptWitnessV1, KagemushaTopUpRequestV1,
        KagemushaTopUpResultV1,
    },
    kagemusha::{
        KAGEMUSHA_ACKNOWLEDGEMENT_MAX_BYTES_V1, KAGEMUSHA_COMPLETE_EXCHANGE_MAX_BYTES_V1,
        KAGEMUSHA_COMPLETE_TEXT_EXCHANGE_MAX_BYTES_V1, KAGEMUSHA_DEVICE_LIFECYCLE_VERSION_V1,
        KAGEMUSHA_HANDOFF_CAPABILITY_V1, KAGEMUSHA_MINT_CREDIT_MAX_BYTES_V1,
        KAGEMUSHA_PAYMENT_MAX_BYTES_V1, KAGEMUSHA_PAYMENT_REQUEST_MAX_BYTES_V1,
        KAGEMUSHA_REDEMPTION_VOUCHER_MAX_BYTES_V1, KAGEMUSHA_WIRE_VERSION_V1,
        KagemushaAcknowledgementV1, KagemushaMintCreditV1, KagemushaPaymentRequestV1,
        KagemushaPaymentV1, KagemushaRedemptionVoucherV1, KagemushaValidationErrorV1,
    },
};
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};

/// Stable schema name for the four-field readiness response.
pub const KAGEMUSHA_READINESS_SCHEMA_NAME_V1: &str = "iroha.torii.v1.kagemusha.readiness.response";
/// Stable schema name for an operation lookup selector.
pub const KAGEMUSHA_OPERATION_LOOKUP_SCHEMA_NAME_V1: &str =
    "iroha.torii.v1.kagemusha.operation.lookup";
/// Stable schema name for a pollable operation response.
pub const KAGEMUSHA_OPERATION_STATUS_SCHEMA_NAME_V1: &str =
    "iroha.torii.v1.kagemusha.operation.status";
/// Stable schema name for the payer-signed top-up transaction submitted to Torii.
pub const KAGEMUSHA_TOP_UP_SIGNED_TRANSACTION_SCHEMA_NAME_V1: &str =
    "iroha.torii.v1.kagemusha.top_up.signed_transaction";
/// Canonical relative route prefix for one KAGEMUSHA V1 operation resource.
pub const KAGEMUSHA_OPERATION_STATUS_ROUTE_PREFIX_V1: &str = "/v1/kagemusha/operations/";

/// Maximum canonical readiness response bytes.
pub const KAGEMUSHA_READINESS_MAX_BYTES_V1: usize = 4 * 1024;
/// Maximum canonical embedded top-up request bytes.
///
/// The fixed-shape ceiling includes the complete hardware credential,
/// encrypted credit opening, and both maximum-size proof parities in
/// `KagemushaMintAuthorizationV1`. It is independent of payment history. The
/// HTTP body is a versioned [`SignedTransaction`] and therefore uses Torii's
/// normal signed-transaction ingress limit rather than this inner-request cap.
pub const KAGEMUSHA_TOP_UP_REQUEST_MAX_BYTES_V1: usize = 16 * 1024;
/// Minimum Torii signed-transaction ingress capacity when KAGEMUSHA commands are enabled.
///
/// This provisioning floor leaves a full 16 KiB of framing headroom around a
/// maximum-shape embedded top-up request. It is not a protocol maximum; nodes
/// may configure a larger ordinary transaction ingress limit.
pub const KAGEMUSHA_TOP_UP_SIGNED_TRANSACTION_MIN_INGRESS_BYTES_V1: usize = 32 * 1024;
/// Maximum canonical redemption request bytes.
///
/// This includes one constant-size redemption voucher and request framing. It
/// is independent of ancestry, receipt count, and proof depth.
pub const KAGEMUSHA_REDEMPTION_REQUEST_MAX_BYTES_V1: usize = 8 * 1024;
/// Maximum canonical operation lookup bytes.
pub const KAGEMUSHA_OPERATION_LOOKUP_MAX_BYTES_V1: usize = 128;
/// Maximum canonical operation-status response bytes.
///
/// The bound covers the consensus roster certificate and one fixed-depth
/// ordinary-write witness. Neither component grows with KAGEMUSHA handoff history.
pub const KAGEMUSHA_OPERATION_STATUS_MAX_BYTES_V1: usize = 4 * 1024 * 1024;
/// Maximum JSON operation-status response bytes.
///
/// JSON byte arrays expand relative to canonical Norito. The binary limit
/// remains the authoritative protocol representation.
pub const KAGEMUSHA_OPERATION_STATUS_JSON_MAX_BYTES_V1: usize = 16 * 1024 * 1024;

/// Exact first-release readiness response.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(schema_name = "iroha.torii.v1.kagemusha.readiness.response")]
#[norito(deny_unknown_fields)]
pub struct KagemushaReadinessV1 {
    /// Exact irreversible KAGEMUSHA peer-handoff contract.
    pub kagemusha_handoff_capability: String,
    /// Canonical KAGEMUSHA wire version accepted by the node.
    pub wire_version: u16,
    /// Exact secure-device lifecycle contract required by the node.
    pub device_lifecycle_version: u16,
    /// Whether this build serves the universal V1 codec and route contract.
    ///
    /// This is protocol-surface discovery, not proof-release, hardware-profile,
    /// asset, reserve, or device admission. Those checks remain operation-local.
    pub ready: bool,
}

/// Untrusted finality coordinates advertised by an applied operation response.
///
/// This value is only a lookup hint. A wallet must resolve the coordinates
/// through release-pinned state or its already authenticated context chain and
/// then supply the resulting [`KagemushaFinalityTrustAnchorV1`] to
/// [`UnverifiedKagemushaOperationStatusV1::verify_against`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KagemushaFinalityAnchorHintV1 {
    /// Advertised genesis-derived network identity.
    pub network_id: NetworkId,
    /// Advertised finalized block height.
    pub block_height: u64,
    /// Advertised context identifier at `block_height`.
    pub height_context_id: HeightContextId,
}

/// Bounded canonical operation response whose monetary result is still untrusted.
///
/// The inner status is deliberately private. Before finality verification,
/// callers may inspect only routing metadata needed to resolve an external
/// trust anchor. In particular, no applied result, mint credit, redemption
/// receipt, reserve amount, or finality certificate is exposed.
pub struct UnverifiedKagemushaOperationStatusV1 {
    inner: KagemushaOperationStatusV1,
}

impl UnverifiedKagemushaOperationStatusV1 {
    fn from_decoded(inner: KagemushaOperationStatusV1) -> Result<Self, KagemushaApiErrorV1> {
        match inner.validate() {
            Ok(()) => {}
            Err(KagemushaIsiValidationErrorV1::MissingTrustAnchor) => {
                let Some(result) = inner.result.as_ref() else {
                    return Err(KagemushaIsiValidationErrorV1::InvalidStatus.into());
                };
                if inner.state != KagemushaOperationStateV1::Applied
                    || inner.rejection.is_some()
                    || result.kind() != inner.kind
                    || result.operation_id() != inner.operation_id
                {
                    return Err(KagemushaIsiValidationErrorV1::InvalidStatus.into());
                }
            }
            Err(error) => return Err(error.into()),
        }
        Ok(Self { inner })
    }

    /// Return the non-authoritative operation identity.
    #[must_use]
    pub const fn operation_id(&self) -> [u8; 32] {
        self.inner.operation_id
    }

    /// Return the non-authoritative operation kind.
    #[must_use]
    pub const fn kind(&self) -> KagemushaOperationKindV1 {
        self.inner.kind
    }

    /// Return the non-monetary lifecycle state.
    #[must_use]
    pub const fn state(&self) -> KagemushaOperationStateV1 {
        self.inner.state
    }

    /// Return untrusted coordinates for resolving an external finality anchor.
    ///
    /// Pending and rejected responses return `None`. Coordinates from an
    /// applied response have not yet been authenticated and grant no monetary
    /// authority.
    #[must_use]
    pub fn finality_anchor_hint(&self) -> Option<KagemushaFinalityAnchorHintV1> {
        let result = self.inner.result.as_ref()?;
        let finality = match result {
            KagemushaOperationResultV1::TopUp(result) => &result.finality,
            KagemushaOperationResultV1::Redemption(result) => &result.finality,
        };
        Some(KagemushaFinalityAnchorHintV1 {
            network_id: finality.finality_artifact.height_context.network_id,
            block_height: finality.finality_artifact.height,
            height_context_id: finality.finality_artifact.context_id(),
        })
    }

    /// Authenticate this response and release its complete operation result.
    ///
    /// # Errors
    ///
    /// Returns an error unless the complete response validates against the
    /// caller-pinned network, block height, context, certificate, and reserve
    /// receipt witness.
    pub fn verify_against(
        self,
        trust_anchor: &KagemushaFinalityTrustAnchorV1,
    ) -> Result<KagemushaOperationStatusV1, KagemushaApiErrorV1> {
        self.inner.validate_against(trust_anchor)?;
        Ok(self.inner)
    }
}

impl KagemushaReadinessV1 {
    /// Validate the closed four-field capability contract.
    ///
    /// The advertised protocol identity must always be exact. `ready` reports
    /// only this universal codec/route surface and grants no monetary authority.
    ///
    /// # Errors
    ///
    /// Returns an error if this is not the sole KAGEMUSHA V1 capability.
    pub fn validate(&self) -> Result<(), KagemushaApiErrorV1> {
        if self.kagemusha_handoff_capability != KAGEMUSHA_HANDOFF_CAPABILITY_V1
            || self.wire_version != KAGEMUSHA_WIRE_VERSION_V1
            || self.device_lifecycle_version != KAGEMUSHA_DEVICE_LIFECYCLE_VERSION_V1
        {
            return Err(KagemushaApiErrorV1::InvalidReadiness);
        }
        let encoded = norito::encode_canonical(self).map_err(KagemushaApiErrorV1::Codec)?;
        ensure_size(encoded.len(), KAGEMUSHA_READINESS_MAX_BYTES_V1)?;
        Ok(())
    }
}

/// Failure at the bounded public Torii KAGEMUSHA V1 boundary.
#[derive(Debug)]
pub enum KagemushaApiErrorV1 {
    /// Canonical Norito encoding or decoding failed.
    Codec(norito::Error),
    /// Strict JSON decoding failed.
    Json(String),
    /// A peer handoff value failed its exact wire validation.
    Wire(KagemushaValidationErrorV1),
    /// A chain-facing value failed its exact V1 validation.
    Chain(KagemushaIsiValidationErrorV1),
    /// The encoded value exceeded its pre-decode resource ceiling.
    EncodedSizeExceeded {
        /// Observed encoded bytes.
        actual: usize,
        /// Maximum accepted encoded bytes.
        max: usize,
    },
    /// A readiness response advertised a different protocol contract.
    InvalidReadiness,
    /// The payer-signed top-up transaction targets a different network.
    TopUpTransactionWrongNetwork,
    /// The payer-signed top-up transaction signature is invalid.
    TopUpTransactionSignatureInvalid,
    /// The top-up transaction does not contain exactly one native top-up instruction.
    TopUpTransactionShapeInvalid,
    /// The signed transaction authority is not the embedded top-up payer.
    TopUpTransactionAuthorityMismatch,
    /// The signed transaction does not require globally certified queue-plan admission.
    TopUpTransactionAdmissionIntentInvalid,
}

impl core::fmt::Display for KagemushaApiErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Codec(error) => {
                write!(formatter, "canonical KAGEMUSHA V1 codec failed: {error}")
            }
            Self::Json(error) => write!(formatter, "KAGEMUSHA V1 JSON decode failed: {error}"),
            Self::Wire(error) => {
                write!(formatter, "KAGEMUSHA V1 wire validation failed: {error}")
            }
            Self::Chain(error) => {
                write!(formatter, "KAGEMUSHA V1 chain validation failed: {error}")
            }
            Self::EncodedSizeExceeded { actual, max } => write!(
                formatter,
                "KAGEMUSHA V1 encoded size {actual} exceeds limit {max}"
            ),
            Self::InvalidReadiness => {
                formatter.write_str("invalid KAGEMUSHA V1 readiness capability")
            }
            Self::TopUpTransactionWrongNetwork => {
                formatter.write_str("KAGEMUSHA V1 top-up transaction targets a different network")
            }
            Self::TopUpTransactionSignatureInvalid => formatter
                .write_str("KAGEMUSHA V1 top-up transaction signature or authority is invalid"),
            Self::TopUpTransactionShapeInvalid => formatter.write_str(
                "KAGEMUSHA V1 top-up transaction must contain exactly one native top-up instruction",
            ),
            Self::TopUpTransactionAuthorityMismatch => formatter.write_str(
                "KAGEMUSHA V1 top-up transaction authority must equal its embedded payer",
            ),
            Self::TopUpTransactionAdmissionIntentInvalid => formatter.write_str(
                "KAGEMUSHA V1 top-up transaction must bind QueuePlanSynced admission",
            ),
        }
    }
}

impl std::error::Error for KagemushaApiErrorV1 {}

impl From<KagemushaValidationErrorV1> for KagemushaApiErrorV1 {
    fn from(error: KagemushaValidationErrorV1) -> Self {
        Self::Wire(error)
    }
}

impl From<KagemushaIsiValidationErrorV1> for KagemushaApiErrorV1 {
    fn from(error: KagemushaIsiValidationErrorV1) -> Self {
        Self::Chain(error)
    }
}

fn ensure_size(actual: usize, max: usize) -> Result<(), KagemushaApiErrorV1> {
    if actual > max {
        return Err(KagemushaApiErrorV1::EncodedSizeExceeded { actual, max });
    }
    Ok(())
}

fn decode_bounded_canonical<T>(bytes: &[u8], max: usize) -> Result<T, KagemushaApiErrorV1>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    ensure_size(bytes.len(), max)?;
    norito::decode_canonical_with_limits(bytes, norito::canonical_decode_limits(bytes.len()))
        .map_err(KagemushaApiErrorV1::Codec)
}

/// Validate and borrow the request from one payer-signed top-up transaction.
///
/// The transaction and embedded request must both target `expected_network`,
/// carry a valid signature, contain exactly one native [`TopUpKagemushaV1`]
/// instruction, name the transaction authority as the request payer, and signature-bind
/// [`TransactionAdmissionIntent::QueuePlanSynced`]. These checks make the
/// normal transaction signature the sole online debit authorization and force
/// the globally certified durable admission path; Torii does not rebuild or
/// re-sign the transaction.
///
/// # Errors
///
/// Returns an error for a wrong network, invalid signature, any executable
/// other than one native top-up instruction, an invalid embedded request, or
/// an authority/payer mismatch, or ordinary queue admission.
pub fn validate_kagemusha_top_up_signed_transaction_v1<'a>(
    expected_network: &NetworkId,
    transaction: &'a SignedTransaction,
) -> Result<&'a KagemushaTopUpRequestV1, KagemushaApiErrorV1> {
    if transaction.network_id() != Some(expected_network) {
        return Err(KagemushaApiErrorV1::TopUpTransactionWrongNetwork);
    }
    transaction
        .verify_signature()
        .map_err(|_| KagemushaApiErrorV1::TopUpTransactionSignatureInvalid)?;
    if transaction.admission_intent() != TransactionAdmissionIntent::QueuePlanSynced {
        return Err(KagemushaApiErrorV1::TopUpTransactionAdmissionIntentInvalid);
    }
    let Executable::Instructions(instructions) = transaction.instructions() else {
        return Err(KagemushaApiErrorV1::TopUpTransactionShapeInvalid);
    };
    if instructions.len() != 1 {
        return Err(KagemushaApiErrorV1::TopUpTransactionShapeInvalid);
    }
    let instruction = instructions[0]
        .as_any()
        .downcast_ref::<TopUpKagemushaV1>()
        .ok_or(KagemushaApiErrorV1::TopUpTransactionShapeInvalid)?;
    instruction.validate_shape()?;
    let request = instruction.request();
    if &request.network_id != expected_network {
        return Err(KagemushaApiErrorV1::TopUpTransactionWrongNetwork);
    }
    let request_bytes = norito::encode_canonical(request).map_err(KagemushaApiErrorV1::Codec)?;
    ensure_size(request_bytes.len(), KAGEMUSHA_TOP_UP_REQUEST_MAX_BYTES_V1)?;
    if transaction.authority() != &request.payer {
        return Err(KagemushaApiErrorV1::TopUpTransactionAuthorityMismatch);
    }
    Ok(request)
}

/// Decode one exact first-release wallet payment request.
///
/// # Errors
///
/// Returns an error when the body is oversized, malformed, non-canonical, or
/// does not carry the exact KAGEMUSHA V1 contract.
pub fn decode_kagemusha_payment_request_v1(
    bytes: &[u8],
) -> Result<KagemushaPaymentRequestV1, KagemushaValidationErrorV1> {
    KagemushaPaymentRequestV1::decode_canonical_exact(bytes)
}

/// Decode one exact payment against its caller-retained request.
///
/// # Errors
///
/// Returns an error when the body is oversized, malformed, non-canonical, or
/// is not bound to the exact request and commit certificate.
/// This boundary checks the post-commit proof shape without granting proof authority.
pub fn decode_kagemusha_payment_v1(
    bytes: &[u8],
    request: &KagemushaPaymentRequestV1,
) -> Result<KagemushaPaymentV1, KagemushaValidationErrorV1> {
    KagemushaPaymentV1::decode_canonical_shape_exact_against(bytes, request)
}

/// Decode message 3 against the exact request and committed payment.
///
/// # Errors
///
/// Returns an error when the body is oversized, malformed, non-canonical, or
/// is not bound to `request` and `payment` under KAGEMUSHA V1.
pub fn decode_kagemusha_acknowledgement_v1(
    bytes: &[u8],
    request: &KagemushaPaymentRequestV1,
    payment: &KagemushaPaymentV1,
) -> Result<KagemushaAcknowledgementV1, KagemushaValidationErrorV1> {
    KagemushaAcknowledgementV1::decode_canonical_shape_exact_against(bytes, request, payment)
}

/// Validate the complete ordered request/payment/acknowledgement exchange.
///
/// # Errors
///
/// Returns an error for any malformed message, cross-message substitution, or
/// raw/text size overrun. This performs shape and binding validation only; it
/// does not grant proof or monetary authority.
pub fn validate_kagemusha_complete_exchange_v1(
    request: &KagemushaPaymentRequestV1,
    payment: &KagemushaPaymentV1,
    acknowledgement: &KagemushaAcknowledgementV1,
) -> Result<usize, KagemushaValidationErrorV1> {
    iroha_data_model::kagemusha::validate_kagemusha_complete_exchange_shape_v1(
        request,
        payment,
        acknowledgement,
    )
}

/// Decode one exact top-up intent under its history-independent resource cap.
///
/// # Errors
///
/// Returns an error for an oversized, non-canonical, or invalid V1 intent.
pub fn decode_kagemusha_top_up_request_v1(
    bytes: &[u8],
) -> Result<KagemushaTopUpRequestV1, KagemushaApiErrorV1> {
    let request: KagemushaTopUpRequestV1 =
        decode_bounded_canonical(bytes, KAGEMUSHA_TOP_UP_REQUEST_MAX_BYTES_V1)?;
    request.validate_shape()?;
    Ok(request)
}

/// Decode one exact redemption intent under its history-independent resource cap.
///
/// # Errors
///
/// Returns an error for an oversized, non-canonical, or invalid V1 intent.
pub fn decode_kagemusha_redemption_request_v1(
    bytes: &[u8],
) -> Result<KagemushaRedemptionRequestV1, KagemushaApiErrorV1> {
    let request: KagemushaRedemptionRequestV1 =
        decode_bounded_canonical(bytes, KAGEMUSHA_REDEMPTION_REQUEST_MAX_BYTES_V1)?;
    request.validate_shape()?;
    Ok(request)
}

/// Decode one exact operation lookup selector.
///
/// # Errors
///
/// Returns an error for an oversized, non-canonical, or invalid V1 selector.
pub fn decode_kagemusha_operation_lookup_v1(
    bytes: &[u8],
) -> Result<KagemushaOperationLookupV1, KagemushaApiErrorV1> {
    let lookup: KagemushaOperationLookupV1 =
        decode_bounded_canonical(bytes, KAGEMUSHA_OPERATION_LOOKUP_MAX_BYTES_V1)?;
    lookup.validate()?;
    Ok(lookup)
}

/// Bounded-canonically decode one operation response without trusting its result.
///
/// The returned wrapper exposes only operation routing metadata. For an
/// applied response, use its finality hint to resolve an independently trusted
/// context, then call
/// [`UnverifiedKagemushaOperationStatusV1::verify_against`]. There is no
/// accessor for the unverified monetary result.
///
/// # Errors
///
/// Returns an error for an oversized, non-canonical, or structurally invalid
/// V1 response. Applied results deliberately return a wrapper rather than
/// performing self-anchored finality validation.
pub fn decode_unverified_kagemusha_operation_status_v1(
    bytes: &[u8],
) -> Result<UnverifiedKagemushaOperationStatusV1, KagemushaApiErrorV1> {
    let status: KagemushaOperationStatusV1 =
        decode_bounded_canonical(bytes, KAGEMUSHA_OPERATION_STATUS_MAX_BYTES_V1)?;
    UnverifiedKagemushaOperationStatusV1::from_decoded(status)
}

/// Bounded-decode one JSON operation response without trusting its result.
///
/// This is the JSON counterpart of
/// [`decode_unverified_kagemusha_operation_status_v1`]. It returns the same
/// restricted wrapper and never exposes an applied result before anchored
/// verification.
///
/// # Errors
///
/// Returns an error for an oversized, malformed, unknown-field, or
/// structurally invalid V1 JSON response.
pub fn decode_unverified_kagemusha_operation_status_json_v1(
    bytes: &[u8],
) -> Result<UnverifiedKagemushaOperationStatusV1, KagemushaApiErrorV1> {
    ensure_size(bytes.len(), KAGEMUSHA_OPERATION_STATUS_JSON_MAX_BYTES_V1)?;
    let status: KagemushaOperationStatusV1 = norito::json::from_slice(bytes)
        .map_err(|error| KagemushaApiErrorV1::Json(error.to_string()))?;
    UnverifiedKagemushaOperationStatusV1::from_decoded(status)
}

/// Decode and authenticate one operation status against caller-pinned finality.
///
/// Requiring `trust_anchor` at this public terminal boundary prevents an
/// applied response from choosing the network roster or height context used
/// to validate itself. Clients that first need the result's advertised height
/// use [`decode_unverified_kagemusha_operation_status_v1`] only to resolve a
/// matching independently trusted context, then call this function (or the
/// wrapper's `verify_against`) to unlock the complete result.
///
/// # Errors
///
/// Returns an error for an oversized, non-canonical, invalid, or untrusted V1
/// operation response.
pub fn decode_kagemusha_operation_status_v1(
    bytes: &[u8],
    trust_anchor: &KagemushaFinalityTrustAnchorV1,
) -> Result<KagemushaOperationStatusV1, KagemushaApiErrorV1> {
    decode_unverified_kagemusha_operation_status_v1(bytes)?.verify_against(trust_anchor)
}

/// Decode and authenticate one JSON operation status against pinned finality.
///
/// # Errors
///
/// Returns an error for an oversized, malformed, invalid, or untrusted V1 JSON
/// operation response.
pub fn decode_kagemusha_operation_status_json_v1(
    bytes: &[u8],
    trust_anchor: &KagemushaFinalityTrustAnchorV1,
) -> Result<KagemushaOperationStatusV1, KagemushaApiErrorV1> {
    decode_unverified_kagemusha_operation_status_json_v1(bytes)?.verify_against(trust_anchor)
}

#[cfg(test)]
#[path = "kagemusha_v1_api_tests.rs"]
mod kagemusha_v1_api_tests;
