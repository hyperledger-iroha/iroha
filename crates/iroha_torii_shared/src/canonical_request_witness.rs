//! The single V1 signing preimage for canonical request witnesses.

use std::borrow::Cow;

use iroha_crypto::Hash;
use iroha_data_model::{account::AccountId, soracloud::CanonicalRequestWitnessV1};
use norito::core::{BoundedEncodeError, DecodeFlagsGuard, SerializePayload};

/// Encode the complete canonical frame signed by a request witness.
///
/// The preimage contains the version, subject account, timestamp, nonce and
/// canonical request hash in that order. It excludes the signature vector.
/// Account and nonce storage is borrowed, and the complete frame is counted
/// before reserving its bounded output buffer. Ambient layout flags do not
/// affect these signed bytes.
///
/// Callers retain responsibility for validating the witness version, nonce,
/// signatures and authorization before using this encoding.
///
/// # Errors
/// Returns a codec or allocation error, or rejects a complete frame larger
/// than `max_frame_bytes` before allocating the output buffer.
pub fn encode_signing_message(
    witness: &CanonicalRequestWitnessV1,
    max_frame_bytes: usize,
) -> Result<Vec<u8>, BoundedEncodeError> {
    let payload = CanonicalRequestWitnessPayloadV1 {
        schema_version: witness.schema_version,
        subject_account: BorrowedCanonicalRequestAccountId(&witness.subject_account),
        timestamp_ms: witness.timestamp_ms,
        nonce: Cow::Borrowed(&witness.nonce),
        canonical_request_hash: witness.canonical_request_hash,
    };
    let _flags = DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    norito::core::to_bytes_bounded(&payload, max_frame_bytes)
}

// This field adapter delegates the owned AccountId payload without cloning its
// controller. It is never an independently framed value.
struct BorrowedCanonicalRequestAccountId<'a>(&'a AccountId);

impl SerializePayload for BorrowedCanonicalRequestAccountId<'_> {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        norito::core::SerializePayload::serialize(self.0, writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        norito::core::SerializePayload::encoded_len_hint(self.0)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        norito::core::SerializePayload::encoded_len_exact(self.0)
    }
}

// The frame projection is the compiler-observed SDK signing contract. Moving
// ownership here must not change the bytes covered by existing signatures.
#[derive(norito::derive::Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_torii_shared::canonical_request_witness::CanonicalRequestWitnessPayloadV1",
    frame = "iroha::client::canonical_request_witness_message::CanonicalRequestWitnessPayloadV1"
)]
struct CanonicalRequestWitnessPayloadV1<'a> {
    schema_version: u16,
    subject_account: BorrowedCanonicalRequestAccountId<'a>,
    timestamp_ms: u64,
    nonce: Cow<'a, str>,
    canonical_request_hash: Hash,
}

#[cfg(test)]
mod tests;
