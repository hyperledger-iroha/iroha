//! Canonical retained-value accounting, separate from consensus and process RSS.
use super::*;
use iroha_data_model::{
    account::AccountId,
    asset::AssetDefinitionId,
    nexus::{
        PrivateSettlementCapsulePaddingV1, PrivateSettlementProofIntentV1,
        PrivateSettlementProvisionalLegMaterialV1,
    },
};

// Payload-only adapters borrow their original allocations. These accounting frames have no
// decoder and cannot create a payment, wallet authorization or independently verified result.
#[derive(Clone, Copy)]
struct Borrowed<'a, T>(&'a T);
impl<T: norito::SerializePayload> norito::SerializePayload for Borrowed<'_, T> {
    fn serialize(&self, writer: &mut norito::Encoder<'_>) -> Result<(), norito::Error> {
        self.0.serialize(writer)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        self.0.encoded_len_hint()
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        self.0.encoded_len_exact()
    }
}

#[derive(norito::SerializePayload)]
struct BusinessTerms<'a> {
    asset: Borrowed<'a, AssetDefinitionId>,
    payer: Borrowed<'a, AccountId>,
    recipient: Borrowed<'a, AccountId>,
    amount: u128,
    reimbursement: u128,
    memo: &'a [u8],
}

#[derive(norito::SerializePayload, norito::NoritoSchema)]
#[norito_schema(name = "iroha_private_settlement::native_session::OriginalAccounting")]
struct OriginalAccounting<'a> {
    intent: Borrowed<'a, PrivateSettlementProofIntentV1>,
    terms: Vec<BusinessTerms<'a>>,
    schema: iroha_crypto::Hash,
    minimum_context_time_ms: u64,
    endpoints: Vec<Vec<&'a str>>,
    padding: PrivateSettlementCapsulePaddingV1,
    retention_height: u64,
    max_carrier_bytes: u64,
    max_artifact_bytes: u64,
    max_total_artifact_bytes: u64,
}

#[derive(norito::SerializePayload, norito::NoritoSchema)]
#[norito_schema(name = "iroha_private_settlement::native_session::PreparedAccounting")]
struct PreparedAccounting<'a> {
    materials: Vec<Borrowed<'a, PrivateSettlementProvisionalLegMaterialV1>>,
    endpoints: Vec<Vec<&'a str>>,
    max_carrier_bytes: u64,
}

pub(super) fn original(
    operation: &OneCallPrivateSettlementOperation,
    limits: NativePrivateSettlementLimits,
) -> Result<(), OneCallPrivateSettlementError> {
    let request = operation.driver.session_request();
    if request.0.max_total_artifact_bytes > limits.maximum_artifact_bytes.get() {
        return Err(OneCallPrivateSettlementError::Capacity);
    }
    let bytes = original_bytes(operation)?;
    if bytes > limits.maximum_original_bytes.get() {
        return Err(OneCallPrivateSettlementError::Capacity);
    }
    Ok(())
}

pub(super) fn prepared(
    operation: &OneCallPrivateSettlementOperation,
    prepared: &PrivateSettlementRequest,
    limits: NativePrivateSettlementLimits,
) -> Result<(), OneCallPrivateSettlementError> {
    let bytes = prepared_bytes(operation, prepared)?;
    if bytes > limits.maximum_prepared_bytes.get() {
        return Err(OneCallPrivateSettlementError::Capacity);
    }
    Ok(())
}

use url::Url;

pub(super) fn original_bytes(
    operation: &OneCallPrivateSettlementOperation,
) -> Result<usize, OneCallPrivateSettlementError> {
    let request = operation.driver.session_request();
    request_bytes(request)
}

pub(super) fn request_bytes(
    request: &crate::OneCallPrivateSettlementRequest,
) -> Result<usize, OneCallPrivateSettlementError> {
    // Borrow all original values. Counting does not create a second private memo or proof copy.
    let terms = request
        .0
        .terms
        .iter()
        .map(|term| BusinessTerms {
            asset: Borrowed(&term.asset),
            payer: Borrowed(&term.payer),
            recipient: Borrowed(&term.recipient),
            amount: term.amount,
            reimbursement: term.reimbursement,
            memo: &term.memo,
        })
        .collect::<Vec<_>>();
    let endpoints = request
        .0
        .endpoints
        .iter()
        .map(|row| row.iter().map(Url::as_str).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    norito::canonical_frame_len(&OriginalAccounting {
        intent: Borrowed(&request.0.intent),
        terms,
        schema: request.0.schema,
        minimum_context_time_ms: request.0.minimum_context_time_ms,
        endpoints,
        padding: request.0.padding,
        retention_height: request.0.retention_height,
        max_carrier_bytes: request.0.max_carrier_bytes,
        max_artifact_bytes: request.0.max_artifact_bytes as u64,
        max_total_artifact_bytes: request.0.max_total_artifact_bytes as u64,
    })
    .map_err(OneCallPrivateSettlementError::from)
}

pub(super) fn prepared_bytes(
    operation: &OneCallPrivateSettlementOperation,
    prepared: &PrivateSettlementRequest,
) -> Result<usize, OneCallPrivateSettlementError> {
    let request = operation.driver.session_request();
    // This representation includes each material's repeated full manifest and every exact URL.
    // References preserve the original values; only the bounded reference vectors are allocated.
    let materials = prepared
        .materials()
        .iter()
        .map(Borrowed)
        .collect::<Vec<_>>();
    let endpoints = request
        .0
        .endpoints
        .iter()
        .map(|row| row.iter().map(Url::as_str).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    norito::canonical_frame_len(&PreparedAccounting {
        materials,
        endpoints,
        max_carrier_bytes: request.0.max_carrier_bytes,
    })
    .map_err(OneCallPrivateSettlementError::from)
}
