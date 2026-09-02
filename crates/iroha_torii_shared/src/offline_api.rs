//! Public Torii DTO boundary for Offline Cash V1.
//!
//! Offline Cash V1 peer-handoff and chain-operation types are defined once in
//! `iroha_data_model` and re-exported here without transport wrappers. Every
//! binary ingress helper installs a byte ceiling before canonical Norito
//! decoding. Applied Offline Cash V1 results additionally require a
//! caller-pinned consensus context; an untrusted response can never select the
//! trust root used to validate its own finality proof.
//!
//! An applied top-up response is an idempotent join of the immutable consensus
//! intent/receipt with the durable local finality-and-mint outbox. This DTO
//! boundary does not imply a post-finality mutation of world state.

use iroha_data_model::{NetworkId, block::consensus_v2::HeightContextId};
pub use iroha_data_model::{
    isi::offline_cash_v1::{
        OFFLINE_CASH_CHAIN_VERSION_V1, OFFLINE_CASH_REDEMPTION_REQUEST_SCHEMA_NAME_V1,
        OFFLINE_CASH_TOP_UP_REQUEST_SCHEMA_NAME_V1, OfflineCashFinalityTrustAnchorV1,
        OfflineCashIsiValidationErrorV1, OfflineCashOperationFinalityV1,
        OfflineCashOperationKindV1, OfflineCashOperationLookupV1,
        OfflineCashOperationRejectionCodeV1, OfflineCashOperationRejectionV1,
        OfflineCashOperationResultV1, OfflineCashOperationStateV1, OfflineCashOperationStatusV1,
        OfflineCashRedemptionRequestV1, OfflineCashRedemptionResultV1, OfflineCashReserveReceiptV1,
        OfflineCashReserveReceiptWitnessV1, OfflineCashTopUpRequestV1, OfflineCashTopUpResultV1,
    },
    offline::{
        OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1, OFFLINE_CASH_DEVICE_LIFECYCLE_VERSION_V1,
        OFFLINE_CASH_HANDOFF_CAPABILITY_V1, OFFLINE_CASH_MINT_CREDIT_MAX_BYTES_V1,
        OFFLINE_CASH_PAYMENT_MAX_BYTES_V1, OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1,
        OFFLINE_CASH_REDEMPTION_VOUCHER_MAX_BYTES_V1, OFFLINE_CASH_WIRE_VERSION_V1,
        OfflineCashAcknowledgementV1, OfflineCashMintCreditV1, OfflineCashPaymentRequestV1,
        OfflineCashPaymentV1, OfflineCashRedemptionVoucherV1, OfflineCashValidationErrorV1,
    },
};
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};

/// Stable schema name for the four-field readiness response.
pub const OFFLINE_CASH_READINESS_SCHEMA_NAME_V1: &str =
    "iroha.torii.v1.offline_cash.readiness.response";
/// Stable schema name for an operation lookup selector.
pub const OFFLINE_CASH_OPERATION_LOOKUP_SCHEMA_NAME_V1: &str =
    "iroha.torii.v1.offline_cash.operation.lookup";
/// Stable schema name for a pollable operation response.
pub const OFFLINE_CASH_OPERATION_STATUS_SCHEMA_NAME_V1: &str =
    "iroha.torii.v1.offline_cash.operation.status";
/// Canonical relative route prefix for one Offline Cash V1 operation resource.
pub const OFFLINE_CASH_OPERATION_STATUS_ROUTE_PREFIX_V1: &str = "/v1/offline/operations/";

/// Maximum canonical readiness response bytes.
pub const OFFLINE_CASH_READINESS_MAX_BYTES_V1: usize = 4 * 1024;
/// Maximum canonical top-up request bytes.
///
/// The request carries fixed metadata and at most one bounded encrypted credit
/// opening. This resource ceiling is independent of payment history.
pub const OFFLINE_CASH_TOP_UP_REQUEST_MAX_BYTES_V1: usize = 4 * 1024;
/// Maximum canonical redemption request bytes.
///
/// This includes one constant-size redemption voucher and request framing. It
/// is independent of ancestry, receipt count, and proof depth.
pub const OFFLINE_CASH_REDEMPTION_REQUEST_MAX_BYTES_V1: usize = 8 * 1024;
/// Maximum canonical operation lookup bytes.
pub const OFFLINE_CASH_OPERATION_LOOKUP_MAX_BYTES_V1: usize = 128;
/// Maximum canonical operation-status response bytes.
///
/// The bound covers the consensus roster certificate and one fixed-depth
/// ordinary-write witness. Neither component grows with cash handoff history.
pub const OFFLINE_CASH_OPERATION_STATUS_MAX_BYTES_V1: usize = 4 * 1024 * 1024;
/// Maximum JSON operation-status response bytes.
///
/// JSON byte arrays expand relative to canonical Norito. The binary limit
/// remains the authoritative protocol representation.
pub const OFFLINE_CASH_OPERATION_STATUS_JSON_MAX_BYTES_V1: usize = 16 * 1024 * 1024;

/// Exact first-release readiness response.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(schema_name = "iroha.torii.v1.offline_cash.readiness.response")]
#[norito(deny_unknown_fields)]
pub struct OfflineCashReadinessV1 {
    /// Exact irreversible peer-cash handoff contract.
    pub cash_handoff_capability: String,
    /// Canonical Offline Cash wire version accepted by the node.
    pub wire_version: u16,
    /// Exact secure-device lifecycle contract required by the node.
    pub device_lifecycle_version: u16,
    /// Whether this node is ready to serve the V1 lifecycle.
    pub ready: bool,
}

/// Untrusted finality coordinates advertised by an applied operation response.
///
/// This value is only a lookup hint. A wallet must resolve the coordinates
/// through release-pinned state or its already authenticated context chain and
/// then supply the resulting [`OfflineCashFinalityTrustAnchorV1`] to
/// [`UnverifiedOfflineCashOperationStatusV1::verify_against`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OfflineCashFinalityAnchorHintV1 {
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
pub struct UnverifiedOfflineCashOperationStatusV1 {
    inner: OfflineCashOperationStatusV1,
}

impl UnverifiedOfflineCashOperationStatusV1 {
    fn from_decoded(inner: OfflineCashOperationStatusV1) -> Result<Self, OfflineCashApiErrorV1> {
        match inner.validate() {
            Ok(()) => {}
            Err(OfflineCashIsiValidationErrorV1::MissingTrustAnchor) => {
                let Some(result) = inner.result.as_ref() else {
                    return Err(OfflineCashIsiValidationErrorV1::InvalidStatus.into());
                };
                if inner.state != OfflineCashOperationStateV1::Applied
                    || inner.rejection.is_some()
                    || result.kind() != inner.kind
                    || result.operation_id() != inner.operation_id
                {
                    return Err(OfflineCashIsiValidationErrorV1::InvalidStatus.into());
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
    pub const fn kind(&self) -> OfflineCashOperationKindV1 {
        self.inner.kind
    }

    /// Return the non-monetary lifecycle state.
    #[must_use]
    pub const fn state(&self) -> OfflineCashOperationStateV1 {
        self.inner.state
    }

    /// Return untrusted coordinates for resolving an external finality anchor.
    ///
    /// Pending and rejected responses return `None`. Coordinates from an
    /// applied response have not yet been authenticated and grant no monetary
    /// authority.
    #[must_use]
    pub fn finality_anchor_hint(&self) -> Option<OfflineCashFinalityAnchorHintV1> {
        let result = self.inner.result.as_ref()?;
        let finality = match result {
            OfflineCashOperationResultV1::TopUp(result) => &result.finality,
            OfflineCashOperationResultV1::Redemption(result) => &result.finality,
        };
        Some(OfflineCashFinalityAnchorHintV1 {
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
        trust_anchor: &OfflineCashFinalityTrustAnchorV1,
    ) -> Result<OfflineCashOperationStatusV1, OfflineCashApiErrorV1> {
        self.inner.validate_against(trust_anchor)?;
        Ok(self.inner)
    }
}

impl OfflineCashReadinessV1 {
    /// Validate the closed four-field capability contract.
    ///
    /// `ready` may be false while local service is unavailable, but the
    /// advertised protocol identity must always be exact.
    ///
    /// # Errors
    ///
    /// Returns an error if this is not the sole Offline Cash V1 capability.
    pub fn validate(&self) -> Result<(), OfflineCashApiErrorV1> {
        if self.cash_handoff_capability != OFFLINE_CASH_HANDOFF_CAPABILITY_V1
            || self.wire_version != OFFLINE_CASH_WIRE_VERSION_V1
            || self.device_lifecycle_version != OFFLINE_CASH_DEVICE_LIFECYCLE_VERSION_V1
        {
            return Err(OfflineCashApiErrorV1::InvalidReadiness);
        }
        let encoded = norito::encode_canonical(self).map_err(OfflineCashApiErrorV1::Codec)?;
        ensure_size(encoded.len(), OFFLINE_CASH_READINESS_MAX_BYTES_V1)?;
        Ok(())
    }
}

/// Failure at the bounded public Torii Offline Cash V1 boundary.
#[derive(Debug)]
pub enum OfflineCashApiErrorV1 {
    /// Canonical Norito encoding or decoding failed.
    Codec(norito::Error),
    /// Strict JSON decoding failed.
    Json(String),
    /// A peer handoff value failed its exact wire validation.
    Wire(OfflineCashValidationErrorV1),
    /// A chain-facing value failed its exact V1 validation.
    Chain(OfflineCashIsiValidationErrorV1),
    /// The encoded value exceeded its pre-decode resource ceiling.
    EncodedSizeExceeded {
        /// Observed encoded bytes.
        actual: usize,
        /// Maximum accepted encoded bytes.
        max: usize,
    },
    /// A readiness response advertised a different protocol contract.
    InvalidReadiness,
}

impl core::fmt::Display for OfflineCashApiErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Codec(error) => {
                write!(formatter, "canonical Offline Cash V1 codec failed: {error}")
            }
            Self::Json(error) => write!(formatter, "Offline Cash V1 JSON decode failed: {error}"),
            Self::Wire(error) => {
                write!(formatter, "Offline Cash V1 wire validation failed: {error}")
            }
            Self::Chain(error) => {
                write!(
                    formatter,
                    "Offline Cash V1 chain validation failed: {error}"
                )
            }
            Self::EncodedSizeExceeded { actual, max } => write!(
                formatter,
                "Offline Cash V1 encoded size {actual} exceeds limit {max}"
            ),
            Self::InvalidReadiness => {
                formatter.write_str("invalid Offline Cash V1 readiness capability")
            }
        }
    }
}

impl std::error::Error for OfflineCashApiErrorV1 {}

impl From<OfflineCashValidationErrorV1> for OfflineCashApiErrorV1 {
    fn from(error: OfflineCashValidationErrorV1) -> Self {
        Self::Wire(error)
    }
}

impl From<OfflineCashIsiValidationErrorV1> for OfflineCashApiErrorV1 {
    fn from(error: OfflineCashIsiValidationErrorV1) -> Self {
        Self::Chain(error)
    }
}

fn ensure_size(actual: usize, max: usize) -> Result<(), OfflineCashApiErrorV1> {
    if actual > max {
        return Err(OfflineCashApiErrorV1::EncodedSizeExceeded { actual, max });
    }
    Ok(())
}

fn decode_bounded_canonical<T>(bytes: &[u8], max: usize) -> Result<T, OfflineCashApiErrorV1>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    ensure_size(bytes.len(), max)?;
    norito::decode_canonical_with_limits(bytes, norito::canonical_decode_limits(bytes.len()))
        .map_err(OfflineCashApiErrorV1::Codec)
}

/// Decode one exact first-release wallet payment request.
///
/// # Errors
///
/// Returns an error when the body is oversized, malformed, non-canonical, or
/// does not carry the exact Offline Cash V1 contract.
pub fn decode_offline_cash_payment_request_v1(
    bytes: &[u8],
) -> Result<OfflineCashPaymentRequestV1, OfflineCashValidationErrorV1> {
    OfflineCashPaymentRequestV1::decode_canonical_exact(bytes)
}

/// Decode one exact payment against the caller-retained request.
///
/// # Errors
///
/// Returns an error when the body is oversized, malformed, non-canonical, or
/// is not bound to `request` under the exact Offline Cash V1 contract.
pub fn decode_offline_cash_payment_v1(
    bytes: &[u8],
    request: &OfflineCashPaymentRequestV1,
) -> Result<OfflineCashPaymentV1, OfflineCashValidationErrorV1> {
    OfflineCashPaymentV1::decode_canonical_shape_exact_against(bytes, request)
}

/// Decode one exact acknowledgement against its complete retained session.
///
/// # Errors
///
/// Returns an error when the body is oversized, malformed, non-canonical, or
/// is not bound to `request` and `payment` under Offline Cash V1.
pub fn decode_offline_cash_acknowledgement_v1(
    bytes: &[u8],
    request: &OfflineCashPaymentRequestV1,
    payment: &OfflineCashPaymentV1,
) -> Result<OfflineCashAcknowledgementV1, OfflineCashValidationErrorV1> {
    OfflineCashAcknowledgementV1::decode_canonical_shape_exact_against(bytes, request, payment)
}

/// Decode one exact top-up intent under its history-independent resource cap.
///
/// # Errors
///
/// Returns an error for an oversized, non-canonical, or invalid V1 intent.
pub fn decode_offline_cash_top_up_request_v1(
    bytes: &[u8],
) -> Result<OfflineCashTopUpRequestV1, OfflineCashApiErrorV1> {
    let request: OfflineCashTopUpRequestV1 =
        decode_bounded_canonical(bytes, OFFLINE_CASH_TOP_UP_REQUEST_MAX_BYTES_V1)?;
    request.validate_shape()?;
    Ok(request)
}

/// Decode one exact redemption intent under its history-independent resource cap.
///
/// # Errors
///
/// Returns an error for an oversized, non-canonical, or invalid V1 intent.
pub fn decode_offline_cash_redemption_request_v1(
    bytes: &[u8],
) -> Result<OfflineCashRedemptionRequestV1, OfflineCashApiErrorV1> {
    let request: OfflineCashRedemptionRequestV1 =
        decode_bounded_canonical(bytes, OFFLINE_CASH_REDEMPTION_REQUEST_MAX_BYTES_V1)?;
    request.validate_shape()?;
    Ok(request)
}

/// Decode one exact operation lookup selector.
///
/// # Errors
///
/// Returns an error for an oversized, non-canonical, or invalid V1 selector.
pub fn decode_offline_cash_operation_lookup_v1(
    bytes: &[u8],
) -> Result<OfflineCashOperationLookupV1, OfflineCashApiErrorV1> {
    let lookup: OfflineCashOperationLookupV1 =
        decode_bounded_canonical(bytes, OFFLINE_CASH_OPERATION_LOOKUP_MAX_BYTES_V1)?;
    lookup.validate()?;
    Ok(lookup)
}

/// Bounded-canonically decode one operation response without trusting its result.
///
/// The returned wrapper exposes only operation routing metadata. For an
/// applied response, use its finality hint to resolve an independently trusted
/// context, then call
/// [`UnverifiedOfflineCashOperationStatusV1::verify_against`]. There is no
/// accessor for the unverified monetary result.
///
/// # Errors
///
/// Returns an error for an oversized, non-canonical, or structurally invalid
/// V1 response. Applied results deliberately return a wrapper rather than
/// performing self-anchored finality validation.
pub fn decode_unverified_offline_cash_operation_status_v1(
    bytes: &[u8],
) -> Result<UnverifiedOfflineCashOperationStatusV1, OfflineCashApiErrorV1> {
    let status: OfflineCashOperationStatusV1 =
        decode_bounded_canonical(bytes, OFFLINE_CASH_OPERATION_STATUS_MAX_BYTES_V1)?;
    UnverifiedOfflineCashOperationStatusV1::from_decoded(status)
}

/// Bounded-decode one JSON operation response without trusting its result.
///
/// This is the JSON counterpart of
/// [`decode_unverified_offline_cash_operation_status_v1`]. It returns the same
/// restricted wrapper and never exposes an applied result before anchored
/// verification.
///
/// # Errors
///
/// Returns an error for an oversized, malformed, unknown-field, or
/// structurally invalid V1 JSON response.
pub fn decode_unverified_offline_cash_operation_status_json_v1(
    bytes: &[u8],
) -> Result<UnverifiedOfflineCashOperationStatusV1, OfflineCashApiErrorV1> {
    ensure_size(bytes.len(), OFFLINE_CASH_OPERATION_STATUS_JSON_MAX_BYTES_V1)?;
    let status: OfflineCashOperationStatusV1 = norito::json::from_slice(bytes)
        .map_err(|error| OfflineCashApiErrorV1::Json(error.to_string()))?;
    UnverifiedOfflineCashOperationStatusV1::from_decoded(status)
}

/// Decode and authenticate one operation status against caller-pinned finality.
///
/// Requiring `trust_anchor` at this public terminal boundary prevents an
/// applied response from choosing the network roster or height context used
/// to validate itself. Clients that first need the result's advertised height
/// use [`decode_unverified_offline_cash_operation_status_v1`] only to resolve a
/// matching independently trusted context, then call this function (or the
/// wrapper's `verify_against`) to unlock the complete result.
///
/// # Errors
///
/// Returns an error for an oversized, non-canonical, invalid, or untrusted V1
/// operation response.
pub fn decode_offline_cash_operation_status_v1(
    bytes: &[u8],
    trust_anchor: &OfflineCashFinalityTrustAnchorV1,
) -> Result<OfflineCashOperationStatusV1, OfflineCashApiErrorV1> {
    decode_unverified_offline_cash_operation_status_v1(bytes)?.verify_against(trust_anchor)
}

/// Decode and authenticate one JSON operation status against pinned finality.
///
/// # Errors
///
/// Returns an error for an oversized, malformed, invalid, or untrusted V1 JSON
/// operation response.
pub fn decode_offline_cash_operation_status_json_v1(
    bytes: &[u8],
    trust_anchor: &OfflineCashFinalityTrustAnchorV1,
) -> Result<OfflineCashOperationStatusV1, OfflineCashApiErrorV1> {
    decode_unverified_offline_cash_operation_status_json_v1(bytes)?.verify_against(trust_anchor)
}

#[cfg(test)]
#[path = "offline_cash_v1_api_tests.rs"]
mod offline_cash_v1_api_tests;
