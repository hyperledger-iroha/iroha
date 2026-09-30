//! Challenge-bound, data-only complete World publication at the native applied cut.

use super::*;
use iroha_core::{
    state::{AllocationBudget, StateReadOnly},
    sumeragi::certified_chain::{CertifiedChain, QcVerification},
};
use iroha_data_model::asset::AssetDefinitionId;
use iroha_torii_shared::kagemusha_state::{
    KAGEMUSHA_AUTHORITY_STATE_MAX_BYTES_V1, KagemushaAuthorityStateRefV1,
};
use norito::json::{BoundedJsonError, JsonSerialize as _, JsonWriteSink};

const ROUTE: &str = "/v1/kagemusha/authority-state/{asset_definition_id}";

/// The API-token, challenge and resource gates precede every native capture.
pub(super) async fn handler(
    State(app): State<SharedAppState>,
    axum::extract::Path(asset): axum::extract::Path<String>,
    headers: HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Result<AxResponse, Error> {
    let result = handle(app, asset, headers, remote).await;
    Ok(finalize_bridge_finality_attestation_response(result))
}

async fn handle(
    app: SharedAppState,
    asset: String,
    headers: HeaderMap,
    remote: std::net::SocketAddr,
) -> Result<AxResponse, Error> {
    let principal = validate_api_token(app.as_ref(), &headers)?.authenticated_principal();
    let challenge = bridge_finality_challenge(&headers)?;
    let asset_id: AssetDefinitionId = asset
        .parse()
        .map_err(|_| conversion_error("invalid canonical asset definition ID".into()))?;
    if asset_id.to_string() != asset {
        return Err(conversion_error(
            "asset definition ID must be canonical".into(),
        ));
    }
    let format = match negotiate_heavy_query_response_format(&headers) {
        Ok(format) => format,
        Err(response) => return Ok(response),
    };
    let key = rate_limit_key(&headers, Some(remote.ip()), ROUTE, principal);
    rate_limit_requests_with_cost(&app, &key, FINALITY_HEAVY_QUERY_RATE_COST).await?;
    let admission = acquire_query_admission(app.as_ref(), true).await?;
    // Reserve one complete configured operation from the real aggregate query
    // pool before spawning. The same permit follows output through its last byte owner.
    let bytes = app.query_fanout_working_set_bytes;
    let memory = app
        .query_fanout_inflight
        .try_acquire_parts([u64::try_from(bytes).map_err(|_| capacity())?])
        .ok_or_else(capacity)?;
    let memory = QueryFanoutMemoryReservation::new(memory);
    let budget = AllocationBudget::new(bytes);
    let max_response = app
        .torii_proxy_max_response_bytes
        .min(KAGEMUSHA_AUTHORITY_STATE_MAX_BYTES_V1);
    let driver = app.sumeragi.as_ref().ok_or_else(unavailable)?;
    if driver.restart_required() {
        return Err(unavailable());
    }
    let status = driver.status_dto().ok_or_else(unavailable)?;
    if status.unanchored || status.abstaining || status.halted.is_some() {
        return Err(unavailable());
    }
    let identity = driver.identity().clone();
    if status.signer.as_ref() != Some(identity.node_id.public_key()) {
        return Err(unavailable());
    }
    let fingerprint = iroha_crypto::Hash::new_from_chunks(&[
        app.build_status.version.as_bytes(),
        app.build_status.git_commit_sha.as_bytes(),
    ]);
    let state = app.state.clone();
    let signer = app.torii_proxy_bridge_signer.clone();
    let response = routing::run_admitted_blocking(
        admission,
        "native authority state worker failed",
        move || {
            let _memory = memory;
            let view = state.view();
            let height = u64::try_from(view.height()).map_err(|_| unavailable())?;
            if height < 2 {
                return Err(unavailable());
            }
            let chain = CertifiedChain::new(&view).map_err(|_| unavailable())?;
            let certified = chain.certified(height).map_err(|_| unavailable())?;
            if certified.verification() != QcVerification::Verified {
                return Err(unavailable());
            }
            let tip = certified.into_committed();
            // Proof frame copies and their finite committee/signer vectors are
            // prepaid from the same operation budget before the existing builder.
            let genesis = chain.committed(1).map_err(|_| unavailable())?;
            let proof_bytes = norito::canonical_frame_len(genesis.block().as_ref())
                .and_then(|len| {
                    norito::canonical_frame_len(tip.block().as_ref()).map(|tip_len| (len, tip_len))
                })
                .map_err(|_| unavailable())?;
            let proof_bytes = proof_bytes
                .0
                .checked_add(proof_bytes.1)
                .and_then(|len| len.checked_add(16 * 1024))
                .ok_or_else(capacity)?;
            let _proof_charge = budget
                .try_reserve_bytes(proof_bytes)
                .map_err(|_| capacity())?;
            let attestation = iroha_core::sumeragi::finality::build_attestation(
                &view,
                status,
                &identity,
                fingerprint,
                height,
                challenge,
                &signer,
            )
            .map_err(|_| unavailable())?;
            drop(chain);
            drop(view);
            let mut body = state
                .with_native_world_state_snapshot_v1(
                    &tip,
                    &asset_id,
                    &budget,
                    |snapshot, definition, incarnation, registry| {
                        let payload = KagemushaAuthorityStateRefV1::new(
                            &attestation,
                            snapshot,
                            definition,
                            incarnation,
                            registry,
                        );
                        encode(&payload, format, max_response, &budget)
                            .map_err(|_| "native authority state serialization refused".to_owned())
                    },
                )
                .map_err(|_| unavailable())?;
            body.memory = Some(_memory);
            let content_type = match format {
                ResponseFormat::Norito => "application/x-norito",
                ResponseFormat::Json => "application/json",
            };
            let mut response = AxResponse::new(Body::from(Bytes::from_owner(body)));
            response.headers_mut().insert(
                axum::http::header::CONTENT_TYPE,
                HeaderValue::from_static(content_type),
            );
            Ok(response)
        },
    )
    .await?;
    proof_response_with_exact_egress(
        app.as_ref(),
        &headers,
        Some(remote.ip()),
        ROUTE,
        response,
        true,
    )
    .await
}

fn unavailable() -> Error {
    Error::AppServiceUnavailable {
        code: "kagemusha_authority_state_unavailable",
        message: "Current certified native authority state is unavailable.".into(),
    }
}
fn capacity() -> Error {
    Error::Query(iroha_data_model::ValidationFail::QueryFailed(
        iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
    ))
}

struct EncodedBody {
    bytes: iroha_allocation::ChargedBuffer<u8>,
    memory: Option<QueryFanoutMemoryReservation>,
}
impl AsRef<[u8]> for EncodedBody {
    fn as_ref(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}
struct ChargedWriter(iroha_allocation::ChargedBuffer<u8>);
impl std::io::Write for ChargedWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.0.capacity().saturating_sub(self.0.as_slice().len()) {
            return Err(std::io::Error::other("native response length changed"));
        }
        for byte in bytes {
            self.0.push_reserved(*byte);
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl JsonWriteSink for ChargedWriter {
    fn push(&mut self, value: char) -> Result<(), BoundedJsonError> {
        let mut bytes = [0; 4];
        self.push_str(value.encode_utf8(&mut bytes))
    }
    fn push_str(&mut self, value: &str) -> Result<(), BoundedJsonError> {
        std::io::Write::write_all(self, value.as_bytes())
            .map_err(|_| BoundedJsonError::LengthMismatch)
    }
}
struct CountJson {
    length: usize,
    limit: usize,
    depth: usize,
}
impl JsonWriteSink for CountJson {
    fn push(&mut self, value: char) -> Result<(), BoundedJsonError> {
        let mut bytes = [0; 4];
        self.push_str(value.encode_utf8(&mut bytes))
    }
    fn push_str(&mut self, value: &str) -> Result<(), BoundedJsonError> {
        let next = self
            .length
            .checked_add(value.len())
            .ok_or(BoundedJsonError::BodyTooLarge)?;
        if next > self.limit {
            return Err(BoundedJsonError::BodyTooLarge);
        }
        self.length = next;
        Ok(())
    }
    fn begin_container(&mut self) -> Result<(), BoundedJsonError> {
        let next = self
            .depth
            .checked_add(1)
            .ok_or(BoundedJsonError::Unsupported)?;
        if next >= norito::json::MAX_JSON_VALUE_NESTING_DEPTH {
            return Err(BoundedJsonError::Unsupported);
        }
        self.depth = next;
        Ok(())
    }
    fn end_container(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }
}
fn encode(
    payload: &KagemushaAuthorityStateRefV1<'_>,
    format: ResponseFormat,
    limit: usize,
    budget: &AllocationBudget,
) -> Result<EncodedBody, Error> {
    let length = match format {
        ResponseFormat::Norito => {
            norito::canonical_frame_len(payload).map_err(|_| unavailable())?
        }
        ResponseFormat::Json => {
            let mut count = CountJson {
                length: 0,
                limit,
                depth: 0,
            };
            payload
                .json_serialize_to(&mut count)
                .map_err(|_| capacity())?;
            count.length
        }
    };
    if length > limit {
        return Err(capacity());
    }
    let bytes = iroha_allocation::ChargedBuffer::new(length, budget).map_err(|_| capacity())?;
    let mut writer = ChargedWriter(bytes);
    match format {
        ResponseFormat::Norito => norito::core::write_canonical_to_writer(payload, &mut writer)
            .map_err(|_| unavailable())?,
        ResponseFormat::Json => payload
            .json_serialize_to(&mut writer)
            .map_err(|_| unavailable())?,
    }
    if writer.0.as_slice().len() != length {
        return Err(unavailable());
    }
    Ok(EncodedBody {
        bytes: writer.0,
        memory: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    #[test]
    fn charged_native_output_rejects_growth_and_refunds_only_when_backing_drops() {
        let budget = AllocationBudget::new(3);
        let mut writer = ChargedWriter(iroha_allocation::ChargedBuffer::new(3, &budget).unwrap());
        writer.write_all(b"abc").unwrap();
        assert!(writer.write_all(b"d").is_err());
        assert_eq!(writer.0.as_slice(), b"abc");
        let bytes = Bytes::from_owner(EncodedBody {
            bytes: writer.0,
            memory: None,
        });
        let retained = bytes.clone();
        drop(bytes);
        assert_eq!(budget.reserved_bytes(), 3);
        drop(retained);
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn json_preflight_counts_utf8_and_rejects_length_and_depth_before_output() {
        let mut count = CountJson {
            length: 0,
            limit: 4,
            depth: 0,
        };
        count.push('😀').unwrap();
        assert_eq!(count.length, 4);
        assert_eq!(count.push('a'), Err(BoundedJsonError::BodyTooLarge));
        assert_eq!(count.length, 4);
        for _ in 1..norito::json::MAX_JSON_VALUE_NESTING_DEPTH {
            count.begin_container().unwrap();
        }
        assert_eq!(count.begin_container(), Err(BoundedJsonError::Unsupported));
    }

    #[test]
    fn native_response_last_byte_retains_real_aggregate_query_permit() {
        let pool = ByteWeightedMemoryPool::new(3).unwrap();
        let permit = pool.try_acquire_parts([3]).unwrap();
        let budget = AllocationBudget::new(3);
        let mut writer = ChargedWriter(iroha_allocation::ChargedBuffer::new(3, &budget).unwrap());
        writer.write_all(b"abc").unwrap();
        let bytes = Bytes::from_owner(EncodedBody {
            bytes: writer.0,
            memory: Some(QueryFanoutMemoryReservation::new(permit)),
        });
        let retained = bytes.slice(1..);
        drop(bytes);
        assert!(pool.try_acquire_parts([1]).is_none());
        assert_eq!(budget.reserved_bytes(), 3);
        drop(retained);
        assert!(pool.try_acquire_parts([3]).is_some());
        assert_eq!(budget.reserved_bytes(), 0);
    }
}
