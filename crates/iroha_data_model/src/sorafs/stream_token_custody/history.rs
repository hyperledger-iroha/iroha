//! Canonical native custody index and exact typed State paths shared by writers and readers.
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize, sorafs::capacity::ProviderId};
use iroha_crypto::{Hash, PublicKey};
use iroha_model_base::state_path::StatePath;
use std::str::FromStr;

/// Complete coordinates of one immutable native custody-control transition.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    norito::Encode,
    norito::Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(
    name = "iroha_data_model::sorafs::stream_token_custody::history::StreamTokenCustodyControlIndexV1"
)]
pub struct StreamTokenCustodyControlIndexV1 {
    /// One-based native control revision.
    pub revision: u64,
    /// Complete original control-record digest.
    pub digest: [u8; 32],
    /// Execution block height.
    pub height: u64,
    /// Provider control transition ordinal within that block.
    pub ordinal: u32,
}
/// Exact retained custody namespace for one provider.
#[must_use]
pub fn scope(provider: ProviderId) -> String {
    format!(
        "sorafs_stream_token_custody_v1_{}",
        hex::encode(provider.as_bytes())
    )
}
/// Exact current-head path.
#[must_use]
pub fn head_key(provider: ProviderId) -> StatePath {
    StatePath::from_str(&format!("{}_head", scope(provider))).expect("bounded typed custody head")
}
/// Exact immutable revision path, preserving the full u64 width.
#[must_use]
pub fn record_key(provider: ProviderId, revision: u64) -> StatePath {
    StatePath::from_str(&format!("{}_revision_{revision:020}", scope(provider)))
        .expect("bounded typed custody revision")
}
/// Exact execution-height and ordinal index path.
#[must_use]
pub fn height_key(provider: ProviderId, height: u64, ordinal: u32) -> StatePath {
    StatePath::from_str(&format!(
        "{}_height_{height:020}_{ordinal:010}",
        scope(provider)
    ))
    .expect("bounded typed custody height")
}
/// Derive the exact native historical key-generation tombstone path.
/// # Errors
/// Rejects key encoding failure or an invalid bounded path.
pub fn key_path(
    provider: ProviderId,
    signer: bool,
    key: &PublicKey,
) -> Result<StatePath, super::StreamTokenCustodyCommitmentErrorV1> {
    let invalid = super::StreamTokenCustodyCommitmentErrorV1;
    let digest = Hash::new(norito::encode_canonical(key).map_err(|_| invalid)?);
    StatePath::from_str(&format!(
        "{}_{}_key_{}",
        scope(provider),
        if signer { "signer" } else { "attester" },
        hex::encode(digest.as_ref())
    ))
    .map_err(|_| invalid)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_width_selectors_and_index_roundtrip() {
        let provider = ProviderId::new([0xff; 32]);
        assert!(
            record_key(provider, u64::MAX)
                .to_string()
                .ends_with("18446744073709551615")
        );
        assert!(
            height_key(provider, u64::MAX, u32::MAX)
                .to_string()
                .ends_with("18446744073709551615_4294967295")
        );
        assert_ne!(head_key(provider), record_key(provider, 1));
        let index = StreamTokenCustodyControlIndexV1 {
            revision: u64::MAX,
            digest: [1; 32],
            height: u64::MAX,
            ordinal: u32::MAX,
        };
        let bytes = norito::encode_canonical(&index).unwrap();
        assert_eq!(
            norito::decode_canonical::<StreamTokenCustodyControlIndexV1>(&bytes).unwrap(),
            index
        );
    }
}
