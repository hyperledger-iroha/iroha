//! Strict native inspection for JavaScript SoraFS orderbook submission.
#[rustfmt::skip]
use iroha_data_model::{sorafs::orderbook_submission::{SorafsOrderbookSubmissionRouteV1, decode_and_verify_sorafs_orderbook_submission_receipt_v1, inspect_sorafs_orderbook_submission_for_discriminant_v1 as inspect_submission, parse_sorafs_orderbook_receipt_signer_v1, parse_sorafs_orderbook_submission_identity_v1}, transaction::TransactionSubmissionReceipt};
use napi::bindgen_prelude::Uint8Array;
use napi_derive::napi;
fn invalid(message: impl Into<String>) -> napi::Error {
    napi::Error::new(napi::Status::InvalidArg, message.into())
}
/// Exact identities derived from one authenticated orderbook transaction.
#[napi(object)]
pub struct JsSorafsOrderbookSubmissionIdentityV1 {
    /// Legacy transaction identity.
    pub tx_hash: String,
    /// Canonical entrypoint identity.
    pub entrypoint_hash: String,
    /// Canonical signed-transaction identity.
    pub signed_transaction_hash: String,
}
/// Reject any noncanonical, unauthenticated, nonsingleton, or wrong-route wire.
#[napi]
#[allow(clippy::needless_pass_by_value)]
#[rustfmt::skip]
pub fn inspect_sorafs_orderbook_submission_v1(
    route: String,
    expected_network_id: Uint8Array,
    expected_chain_discriminant: u32,
    expected_receipt_signer: String,
    signed_transaction_versioned: Uint8Array,
) -> napi::Result<JsSorafsOrderbookSubmissionIdentityV1> {
    let route = SorafsOrderbookSubmissionRouteV1::parse_sdk_label(&route).map_err(|error| invalid(error.to_string()))?;
    let expected_network_id = crate::parse_transaction_network_id_bytes(expected_network_id.as_ref())?;
    let expected_chain_discriminant: u16 = expected_chain_discriminant.try_into().map_err(|_| invalid("expected_chain_discriminant must fit in u16"))?;
    parse_sorafs_orderbook_receipt_signer_v1(&expected_receipt_signer).ok_or_else(|| invalid("expected_receipt_signer must be exact canonical text"))?;
    let validated = inspect_submission(signed_transaction_versioned.as_ref(), route, &expected_network_id, expected_chain_discriminant).map_err(|error| invalid(error.to_string()))?;
    let identity = validated.identity;
    Ok(JsSorafsOrderbookSubmissionIdentityV1 {
        tx_hash: identity.tx_hash.to_string(),
        entrypoint_hash: identity.entrypoint_hash.to_string(),
        signed_transaction_hash: identity.signed_transaction_hash.to_string(),
    })
}
/// Verify and bind one exact Norito receipt to the submitted transaction and signer.
#[napi]
#[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
#[rustfmt::skip]
pub fn verify_sorafs_orderbook_submission_receipt_v1(
    receipt_norito: Uint8Array,
    tx_hash: String,
    entrypoint_hash: String,
    signed_transaction_hash: String,
    expected_receipt_signer: String,
) -> napi::Result<String> {
    let identity = parse_sorafs_orderbook_submission_identity_v1(&tx_hash, &entrypoint_hash, &signed_transaction_hash).ok_or_else(|| invalid("receipt identities must be exact canonical text"))?;
    let signer = parse_sorafs_orderbook_receipt_signer_v1(&expected_receipt_signer).ok_or_else(|| invalid("expected_receipt_signer must be exact canonical text"))?;
    let receipt = decode_and_verify_sorafs_orderbook_submission_receipt_v1(receipt_norito.as_ref(), &identity, &signer).map_err(|error| invalid(error.to_string()))?;
    norito::json::to_json(&receipt).map_err(|error| invalid(error.to_string()))
}
/// Decode a Norito-framed transaction submission receipt into JSON.
#[napi]
#[allow(clippy::needless_pass_by_value)]
pub fn decode_transaction_receipt_json(bytes: Uint8Array) -> napi::Result<String> {
    let receipt: TransactionSubmissionReceipt =
        norito::decode_from_bytes(bytes.as_ref()).map_err(|error| invalid(error.to_string()))?;
    norito::json::to_json(&receipt).map_err(|error| invalid(error.to_string()))
}
