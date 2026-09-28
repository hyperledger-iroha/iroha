//! Ethereum beacon light-client API client (spec §4.13.3, §7.2, §8).
//!
//! [`BeaconClient`] fetches the light-client routes of the beacon API as SSZ
//! (`Accept: application/octet-stream`): bootstraps, sync-committee period
//! updates, and the latest finality and optimistic updates. It returns the raw
//! SSZ bytes with the fork context the endpoint declared: the
//! `Eth-Consensus-Version` header for single objects, and the per-chunk fork
//! digest for period updates. An endpoint that answers these routes with
//! anything but SSZ fails over to the next endpoint; there is no JSON fallback
//! for light-client objects.
//!
//! Header lookups (`/eth/v1/beacon/headers/{block_id}`) have no SSZ encoding in
//! the beacon API and are read as JSON.
//!
//! Nothing is decoded or verified here: SSZ decoding, fork handling and
//! sync-committee verification belong to `iroha_sccp`.

use crate::{
    evm::{HexError, format_data, parse_data_array},
    http::{
        HttpTransport, MEDIA_TYPE_SSZ, RpcError, expect_object, invalid_response, optional,
        required, required_str,
    },
};

/// Most period updates one `light_client/updates` request may ask for
/// (`MAX_REQUEST_LIGHT_CLIENT_UPDATES`).
pub const MAX_LIGHT_CLIENT_UPDATES: u64 = 128;
/// Length of the fork digest that prefixes every update chunk.
pub const FORK_DIGEST_BYTES: usize = 4;
/// Longest accepted `Eth-Consensus-Version` fork name.
const MAX_FORK_NAME_BYTES: usize = 32;

/// One SSZ object and its declared fork.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SszResponse {
    /// Fork name from `Eth-Consensus-Version` (for example `fulu`), when sent.
    pub consensus_version: Option<String>,
    /// Raw SSZ bytes.
    pub ssz: Vec<u8>,
}

/// One chunk of a `light_client/updates` response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SszChunk {
    /// Fork digest context of the chunk.
    pub fork_digest: [u8; FORK_DIGEST_BYTES],
    /// Raw SSZ bytes of one `LightClientUpdate`.
    pub ssz: Vec<u8>,
}

/// A `light_client/updates` response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LightClientUpdates {
    /// Fork names from `Eth-Consensus-Version` (one per chunk on most clients),
    /// empty when the header was not sent.
    pub consensus_versions: Vec<String>,
    /// Update chunks in period order.
    pub chunks: Vec<SszChunk>,
}

/// A beacon block identifier of the header route.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BeaconBlockId(String);

impl BeaconBlockId {
    /// `head`.
    pub fn head() -> Self {
        Self("head".to_owned())
    }

    /// `finalized`.
    pub fn finalized() -> Self {
        Self("finalized".to_owned())
    }

    /// `genesis`.
    pub fn genesis() -> Self {
        Self("genesis".to_owned())
    }

    /// A slot.
    pub fn slot(slot: u64) -> Self {
        Self(slot.to_string())
    }

    /// A block root.
    pub fn root(root: &[u8; 32]) -> Self {
        Self(format_data(root))
    }

    /// The path segment.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A beacon block header as returned by the header route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BeaconHeader {
    /// Block root reported by the endpoint.
    pub root: [u8; 32],
    /// Whether the endpoint considers the block canonical.
    pub canonical: bool,
    /// `slot`.
    pub slot: u64,
    /// `proposer_index`.
    pub proposer_index: u64,
    /// `parent_root`.
    pub parent_root: [u8; 32],
    /// `state_root`.
    pub state_root: [u8; 32],
    /// `body_root`.
    pub body_root: [u8; 32],
    /// Proposer signature.
    pub signature: [u8; 96],
    /// `execution_optimistic`, when reported.
    pub execution_optimistic: Option<bool>,
    /// `finalized`, when reported.
    pub finalized: Option<bool>,
}

/// Beacon API client of the light-client routes.
#[derive(Debug)]
pub struct BeaconClient {
    transport: HttpTransport,
}

impl BeaconClient {
    /// A client over `transport`.
    pub fn new(transport: HttpTransport) -> Self {
        Self { transport }
    }

    /// The underlying transport.
    pub fn transport(&self) -> &HttpTransport {
        &self.transport
    }

    /// `GET /eth/v1/beacon/light_client/bootstrap/{block_root}` as SSZ.
    ///
    /// # Errors
    /// Any [`RpcError`].
    pub fn light_client_bootstrap(&self, block_root: &[u8; 32]) -> Result<SszResponse, RpcError> {
        self.ssz_object(&format!(
            "/eth/v1/beacon/light_client/bootstrap/{}",
            format_data(block_root)
        ))
    }

    /// `GET /eth/v1/beacon/light_client/updates?start_period&count` as SSZ
    /// chunks, `count` in `1..=128`.
    ///
    /// # Errors
    /// [`RpcError::InvalidRequest`] for an out-of-range `count`,
    /// [`RpcError::InvalidResponse`] for malformed chunks or more chunks than
    /// requested, or any [`RpcError`].
    pub fn light_client_updates(
        &self,
        start_period: u64,
        count: u64,
    ) -> Result<LightClientUpdates, RpcError> {
        if !(1..=MAX_LIGHT_CLIENT_UPDATES).contains(&count) {
            return Err(RpcError::InvalidRequest(format!(
                "light-client updates are requested 1..={MAX_LIGHT_CLIENT_UPDATES} at a time"
            )));
        }
        if start_period.checked_add(count).is_none() {
            return Err(RpcError::InvalidRequest(
                "the requested period range overflows".to_owned(),
            ));
        }
        let response = self.transport.get_binary(
            &format!(
                "/eth/v1/beacon/light_client/updates?start_period={start_period}&count={count}"
            ),
            MEDIA_TYPE_SSZ,
        )?;
        let chunks = split_response_chunks(&response.body)?;
        if u64::try_from(chunks.len()).map_or(true, |len| len > count) {
            return Err(invalid_response(
                "light-client updates returned more chunks than requested",
            ));
        }
        let consensus_versions = match response.consensus_version.as_deref() {
            None => Vec::new(),
            Some(header) => header
                .split(',')
                .map(|name| parse_fork_name(name.trim()))
                .collect::<Result<_, _>>()?,
        };
        Ok(LightClientUpdates {
            consensus_versions,
            chunks,
        })
    }

    /// `GET /eth/v1/beacon/light_client/finality_update` as SSZ.
    ///
    /// # Errors
    /// Any [`RpcError`].
    pub fn light_client_finality_update(&self) -> Result<SszResponse, RpcError> {
        self.ssz_object("/eth/v1/beacon/light_client/finality_update")
    }

    /// `GET /eth/v1/beacon/light_client/optimistic_update` as SSZ.
    ///
    /// # Errors
    /// Any [`RpcError`].
    pub fn light_client_optimistic_update(&self) -> Result<SszResponse, RpcError> {
        self.ssz_object("/eth/v1/beacon/light_client/optimistic_update")
    }

    /// `GET /eth/v1/beacon/headers/finalized` (JSON).
    ///
    /// # Errors
    /// Any [`RpcError`].
    pub fn finalized_header(&self) -> Result<BeaconHeader, RpcError> {
        self.header(&BeaconBlockId::finalized())
    }

    /// `GET /eth/v1/beacon/headers/{block_id}` (JSON).
    ///
    /// # Errors
    /// Any [`RpcError`]; an unknown block is an HTTP 404 [`RpcError::Status`].
    pub fn header(&self, block: &BeaconBlockId) -> Result<BeaconHeader, RpcError> {
        let value = self
            .transport
            .get_json(&format!("/eth/v1/beacon/headers/{}", block.as_str()))?;
        parse_header_response(&value)
    }

    fn ssz_object(&self, path: &str) -> Result<SszResponse, RpcError> {
        let response = self.transport.get_binary(path, MEDIA_TYPE_SSZ)?;
        if response.body.is_empty() {
            return Err(invalid_response("an SSZ light-client object is empty"));
        }
        let consensus_version = response
            .consensus_version
            .as_deref()
            .map(parse_fork_name)
            .transpose()?;
        Ok(SszResponse {
            consensus_version,
            ssz: response.body,
        })
    }
}

/// Splits a `light_client/updates` SSZ body into chunks: each chunk is a
/// little-endian `u64` length followed by that many bytes, which are a 4-byte
/// fork digest and the SSZ payload.
///
/// # Errors
/// [`RpcError::InvalidResponse`] for a truncated or undersized chunk.
pub fn split_response_chunks(body: &[u8]) -> Result<Vec<SszChunk>, RpcError> {
    let mut chunks = Vec::new();
    let mut rest = body;
    while !rest.is_empty() {
        let Some((length, tail)) = rest.split_first_chunk::<8>() else {
            return Err(invalid_response("an update chunk length is truncated"));
        };
        let length = usize::try_from(u64::from_le_bytes(*length))
            .map_err(|_| invalid_response("an update chunk is too long"))?;
        if length <= FORK_DIGEST_BYTES {
            return Err(invalid_response("an update chunk has no payload"));
        }
        if length > tail.len() {
            return Err(invalid_response("an update chunk is truncated"));
        }
        let (chunk, next) = tail.split_at(length);
        let (fork_digest, ssz) = chunk.split_at(FORK_DIGEST_BYTES);
        let mut digest = [0_u8; FORK_DIGEST_BYTES];
        digest.copy_from_slice(fork_digest);
        chunks.push(SszChunk {
            fork_digest: digest,
            ssz: ssz.to_vec(),
        });
        rest = next;
    }
    Ok(chunks)
}

/// A fork name: 1..=32 lowercase ASCII letters and digits.
fn parse_fork_name(name: &str) -> Result<String, RpcError> {
    if name.is_empty()
        || name.len() > MAX_FORK_NAME_BYTES
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    {
        return Err(invalid_response(
            "Eth-Consensus-Version is not a lowercase fork name",
        ));
    }
    Ok(name.to_owned())
}

/// A beacon API `uint64`: a decimal string without sign or leading zeros.
///
/// # Errors
/// [`RpcError::InvalidResponse`] for anything else.
pub fn parse_decimal_u64(text: &str, what: &str) -> Result<u64, RpcError> {
    let canonical = !text.is_empty()
        && text.bytes().all(|byte| byte.is_ascii_digit())
        && (text == "0" || !text.starts_with('0'));
    if !canonical {
        return Err(invalid_response(format!(
            "{what} is not a canonical decimal uint64"
        )));
    }
    text.parse()
        .map_err(|_| invalid_response(format!("{what} exceeds uint64")))
}

fn root_field<const N: usize>(
    map: &norito::json::Map,
    key: &str,
    what: &str,
) -> Result<[u8; N], RpcError> {
    parse_data_array::<N>(required_str(map, key, what)?)
        .map_err(|error: HexError| invalid_response(format!("{what}.{key}: {error}")))
}

fn optional_bool(map: &norito::json::Map, key: &str, what: &str) -> Result<Option<bool>, RpcError> {
    optional(map, key)
        .map(|value| {
            value
                .as_bool()
                .ok_or_else(|| invalid_response(format!("{what}.{key} is not a boolean")))
        })
        .transpose()
}

fn parse_header_response(value: &norito::json::Value) -> Result<BeaconHeader, RpcError> {
    let outer = expect_object(value, "header response")?;
    let data = expect_object(required(outer, "data", "header response")?, "header data")?;
    let header = expect_object(required(data, "header", "header data")?, "header")?;
    let message = expect_object(required(header, "message", "header")?, "header message")?;
    let canonical = required(data, "canonical", "header data")?
        .as_bool()
        .ok_or_else(|| invalid_response("header data.canonical is not a boolean"))?;
    Ok(BeaconHeader {
        root: root_field(data, "root", "header data")?,
        canonical,
        slot: parse_decimal_u64(
            required_str(message, "slot", "header message")?,
            "header message.slot",
        )?,
        proposer_index: parse_decimal_u64(
            required_str(message, "proposer_index", "header message")?,
            "header message.proposer_index",
        )?,
        parent_root: root_field(message, "parent_root", "header message")?,
        state_root: root_field(message, "state_root", "header message")?,
        body_root: root_field(message, "body_root", "header message")?,
        signature: root_field(header, "signature", "header")?,
        execution_optimistic: optional_bool(outer, "execution_optimistic", "header response")?,
        finalized: optional_bool(outer, "finalized", "header response")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(digest: [u8; 4], payload: &[u8]) -> Vec<u8> {
        let length = u64::try_from(payload.len() + 4).expect("length");
        let mut out = length.to_le_bytes().to_vec();
        out.extend_from_slice(&digest);
        out.extend_from_slice(payload);
        out
    }

    #[test]
    fn update_chunks_split_on_length_prefixes() {
        let mut body = chunk([1, 2, 3, 4], b"first");
        body.extend(chunk([5, 6, 7, 8], b"second update"));
        let chunks = split_response_chunks(&body).expect("chunks");
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].fork_digest, [1, 2, 3, 4]);
        assert_eq!(chunks[0].ssz, b"first");
        assert_eq!(chunks[1].ssz, b"second update");
        assert!(split_response_chunks(&[]).expect("empty").is_empty());
    }

    #[test]
    fn malformed_update_chunks_are_rejected() {
        let whole = chunk([1, 2, 3, 4], b"payload");
        assert!(split_response_chunks(&whole[..5]).is_err());
        assert!(split_response_chunks(&whole[..whole.len() - 1]).is_err());
        let mut digest_only = 4_u64.to_le_bytes().to_vec();
        digest_only.extend_from_slice(&[1, 2, 3, 4]);
        assert!(split_response_chunks(&digest_only).is_err());
        let mut huge = u64::MAX.to_le_bytes().to_vec();
        huge.extend_from_slice(&[0; 8]);
        assert!(split_response_chunks(&huge).is_err());
    }

    #[test]
    fn fork_names_and_decimals_are_strict() {
        assert_eq!(parse_fork_name("fulu").expect("fork"), "fulu");
        assert_eq!(parse_fork_name("electra2").expect("fork"), "electra2");
        for bad in ["", "Fulu", "fu lu", "fu-lu", &"a".repeat(33)] {
            assert!(parse_fork_name(bad).is_err(), "{bad}");
        }
        assert_eq!(parse_decimal_u64("0", "x").expect("zero"), 0);
        assert_eq!(
            parse_decimal_u64("15301120", "x").expect("slot"),
            15_301_120
        );
        for bad in ["", "01", "+1", "-1", "1.0", "0x10", "18446744073709551616"] {
            assert!(parse_decimal_u64(bad, "x").is_err(), "{bad}");
        }
    }

    #[test]
    fn block_ids_spell_route_segments() {
        assert_eq!(BeaconBlockId::head().as_str(), "head");
        assert_eq!(BeaconBlockId::finalized().as_str(), "finalized");
        assert_eq!(BeaconBlockId::genesis().as_str(), "genesis");
        assert_eq!(BeaconBlockId::slot(42).as_str(), "42");
        assert_eq!(
            BeaconBlockId::root(&[0xab; 32]).as_str(),
            format!("0x{}", "ab".repeat(32))
        );
    }

    #[test]
    fn header_responses_parse_strictly() {
        let text = format!(
            r#"{{"data":{{"root":"0x{r}","canonical":true,"header":{{"message":{{"slot":"7","proposer_index":"9","parent_root":"0x{r}","state_root":"0x{r}","body_root":"0x{r}"}},"signature":"0x{s}"}}}},"execution_optimistic":false}}"#,
            r = "01".repeat(32),
            s = "02".repeat(96),
        );
        let value = norito::json::parse_value(&text).expect("json");
        let header = parse_header_response(&value).expect("header");
        assert_eq!(header.slot, 7);
        assert_eq!(header.proposer_index, 9);
        assert!(header.canonical);
        assert_eq!(header.signature, [2; 96]);
        assert_eq!(header.execution_optimistic, Some(false));
        assert_eq!(header.finalized, None);
        let bad_slot =
            norito::json::parse_value(&text.replace(r#""slot":"7""#, r#""slot":7"#)).expect("json");
        assert!(parse_header_response(&bad_slot).is_err());
        let short_root =
            norito::json::parse_value(&text.replacen(&"01".repeat(32), "01", 1)).expect("json");
        assert!(parse_header_response(&short_root).is_err());
    }
}
