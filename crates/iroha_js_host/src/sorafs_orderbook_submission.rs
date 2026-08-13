//! Strict native inspection for JavaScript SoraFS orderbook submission.

use std::str::FromStr as _;

use iroha_crypto::{HashOf, PublicKey};
use iroha_data_model::{
    sorafs::orderbook_submission::{
        SorafsOrderbookSubmissionIdentityV1, SorafsOrderbookSubmissionRouteV1,
        decode_and_verify_sorafs_orderbook_submission_receipt_v1,
        inspect_sorafs_orderbook_submission_v1 as inspect_submission,
    },
    transaction::{SignedTransaction, TransactionEntrypoint, TransactionSubmissionReceipt},
};
use napi::bindgen_prelude::Uint8Array;
use napi_derive::napi;

fn invalid(message: impl Into<String>) -> napi::Error {
    napi::Error::new(napi::Status::InvalidArg, message.into())
}

fn parse_exact<T>(literal: &str, label: &str) -> napi::Result<T>
where
    T: FromStr + ToString,
{
    let parsed = literal
        .parse::<T>()
        .map_err(|_| invalid(format!("{label} is invalid")))?;
    if parsed.to_string() != literal {
        return Err(invalid(format!("{label} must be exact canonical text")));
    }
    Ok(parsed)
}

fn parse_identity(
    tx_hash: &str,
    entrypoint_hash: &str,
    signed_transaction_hash: &str,
) -> napi::Result<SorafsOrderbookSubmissionIdentityV1> {
    Ok(SorafsOrderbookSubmissionIdentityV1 {
        tx_hash: parse_exact::<HashOf<SignedTransaction>>(tx_hash, "tx_hash")?,
        entrypoint_hash: parse_exact::<HashOf<TransactionEntrypoint>>(
            entrypoint_hash,
            "entrypoint_hash",
        )?,
        signed_transaction_hash: parse_exact::<HashOf<SignedTransaction>>(
            signed_transaction_hash,
            "signed_transaction_hash",
        )?,
    })
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
pub fn inspect_sorafs_orderbook_submission_v1(
    route: String,
    expected_network_id: Uint8Array,
    expected_receipt_signer: String,
    signed_transaction_versioned: Uint8Array,
) -> napi::Result<JsSorafsOrderbookSubmissionIdentityV1> {
    let route = SorafsOrderbookSubmissionRouteV1::parse_sdk_label(&route)
        .map_err(|error| invalid(error.to_string()))?;
    let expected_network_id =
        crate::parse_transaction_network_id_bytes(expected_network_id.as_ref())?;
    let _expected_receipt_signer =
        parse_exact::<PublicKey>(&expected_receipt_signer, "expected_receipt_signer")?;
    let identity = inspect_submission(
        signed_transaction_versioned.as_ref(),
        route,
        &expected_network_id,
    )
    .map_err(|error| invalid(error.to_string()))?;
    Ok(JsSorafsOrderbookSubmissionIdentityV1 {
        tx_hash: identity.tx_hash.to_string(),
        entrypoint_hash: identity.entrypoint_hash.to_string(),
        signed_transaction_hash: identity.signed_transaction_hash.to_string(),
    })
}

/// Verify and bind one exact Norito receipt to the submitted transaction and signer.
#[napi]
#[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
pub fn verify_sorafs_orderbook_submission_receipt_v1(
    receipt_norito: Uint8Array,
    tx_hash: String,
    entrypoint_hash: String,
    signed_transaction_hash: String,
    expected_receipt_signer: String,
) -> napi::Result<String> {
    let identity = parse_identity(&tx_hash, &entrypoint_hash, &signed_transaction_hash)?;
    let signer = parse_exact::<PublicKey>(&expected_receipt_signer, "expected_receipt_signer")?;
    let receipt = decode_and_verify_sorafs_orderbook_submission_receipt_v1(
        receipt_norito.as_ref(),
        &identity,
        &signer,
    )
    .map_err(|error| invalid(error.to_string()))?;
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
