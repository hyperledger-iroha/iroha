//! Genesis bootstrap request/response frames used during peer-to-peer genesis exchange.

use iroha_crypto::{HashOf, PublicKey};
use iroha_data_model::{ChainId, block::BlockHeader};
use iroha_macro::FromVariant;
use norito::codec::{Decode, Encode};

/// Request discriminator used to decouple metadata preflights from full blob fetches.
#[derive(Debug, Clone, Copy, Encode, Decode, PartialEq, Eq)]
pub enum GenesisRequestKind {
    /// Ask the responder to return only metadata (hash, public key, size hint).
    Preflight,
    /// Ask the responder to return the full genesis blob.
    Fetch,
}

/// Request frame sent to peers when bootstrapping a missing genesis.
#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq)]
pub struct GenesisRequest {
    /// Chain identifier expected by the requester.
    pub chain_id: ChainId,
    /// Correlation id used to deduplicate responses.
    pub request_id: u64,
    /// Optional hash asserted by the requester (mismatches are rejected).
    pub expected_hash: Option<HashOf<BlockHeader>>,
    /// Optional genesis public key asserted by the requester (mismatches are rejected).
    pub expected_pubkey: Option<PublicKey>,
    /// Whether this is a preflight or full fetch.
    pub kind: GenesisRequestKind,
}

/// Error taxonomy for genesis bootstrap responses.
#[derive(Debug, Clone, Copy, Encode, Decode, PartialEq, Eq)]
pub enum GenesisResponseError {
    /// Requester is not allowed to fetch genesis from this peer.
    NotAllowed,
    /// Request was rate-limited; requester should retry after a backoff.
    RateLimited,
    /// Request with the same id was already served.
    DuplicateRequest,
    /// Responder cannot serve genesis because it is missing locally.
    MissingGenesis,
    /// Request chain id does not match the responder's chain id.
    MismatchedChain,
    /// Request public key does not match the responder's configured genesis key.
    MismatchedPubkey,
    /// Request hash does not match the responder's genesis hash.
    MismatchedHash,
    /// Genesis blob exceeds the configured maximum size.
    TooLarge,
}

/// Response frame carrying metadata and optionally the full genesis blob.
#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq)]
pub struct GenesisResponse {
    /// Chain identifier advertised by the responder.
    pub chain_id: ChainId,
    /// Correlation id echoed from the request.
    pub request_id: u64,
    /// Genesis public key advertised by the responder.
    pub public_key: Option<PublicKey>,
    /// Hash of the genesis block header.
    pub hash: Option<HashOf<BlockHeader>>,
    /// Size of the encoded genesis block (bytes).
    pub size_hint: Option<u64>,
    /// Encoded genesis block payload when `kind == Fetch`.
    pub payload: Option<Vec<u8>>,
    /// Optional error explaining why the responder could not serve the request.
    pub error: Option<GenesisResponseError>,
}

/// Envelope for genesis bootstrap request/response frames.
#[derive(Debug, Clone, Encode, Decode, FromVariant, PartialEq, Eq)]
pub enum GenesisMessage {
    /// Request genesis metadata or the full blob.
    Request(GenesisRequest),
    /// Response containing metadata or the blob plus any error.
    Response(GenesisResponse),
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Hash, KeyPair};
    use norito::codec::{Decode as NoritoDecode, Encode as NoritoEncode};

    use super::*;

    #[test]
    fn request_roundtrip() {
        let request = GenesisRequest {
            chain_id: ChainId::from("chain"),
            request_id: 7,
            expected_hash: Some(HashOf::<BlockHeader>::from_untyped_unchecked(
                Hash::prehashed([0u8; 32]),
            )),
            expected_pubkey: Some(KeyPair::random().public_key().clone()),
            kind: GenesisRequestKind::Preflight,
        };
        let encoded = NoritoEncode::encode(&request);
        let mut cursor = encoded.as_slice();
        let decoded: GenesisRequest = NoritoDecode::decode(&mut cursor).expect("decode");
        assert_eq!(decoded, request);
    }

    #[test]
    fn response_roundtrip_with_error() {
        let response = GenesisResponse {
            chain_id: ChainId::from("chain"),
            request_id: 11,
            public_key: None,
            hash: None,
            size_hint: Some(42),
            payload: None,
            error: Some(GenesisResponseError::NotAllowed),
        };
        let encoded = NoritoEncode::encode(&response);
        let mut cursor = encoded.as_slice();
        let decoded: GenesisResponse = NoritoDecode::decode(&mut cursor).expect("decode");
        assert_eq!(decoded, response);
    }
}
