//! Bounded acquisition and decode boundary for remote Hugging Face model metadata.
use super::SoracloudError;
use futures_util::{Stream, StreamExt as _};
use iroha_config::parameters::{actual::SoracloudRuntimeHuggingFace, defaults};
use iroha_data_model::soracloud::{SoraHfWeightSelectionV1, derive_hf_weight_selection_v1};
use std::fmt;
const INITIAL_ALLOCATION_BYTES: usize = 16 * 1024;
/// Read provider-controlled model metadata under its source configuration cap.
///
/// The same `soracloud_runtime.hf.model_info_max_response_bytes` value bounds
/// both Torii profile derivation and the runtime importer. At most `max + 1`
/// logical bytes are retained so a missing or dishonest `Content-Length` cannot
/// turn a signed deployment draft into an unbounded allocation.
pub(super) async fn read(
    response: reqwest::Response,
    config: &SoracloudRuntimeHuggingFace,
    repo_id: &str,
    resolved_revision: &str,
) -> Result<Vec<u8>, SoracloudError> {
    let maximum_bytes = configured_maximum_bytes(config)?;
    let declared_length = declared_content_length(&response, repo_id, resolved_revision)?;
    if let Some(length) = declared_length
        && length > maximum_bytes
    {
        return Err(SoracloudError::conflict(format!(
            "Hugging Face model info for `{repo_id}@{resolved_revision}` declares {length} bytes, exceeding the configured {maximum_bytes}-byte limit"
        )));
    }
    collect_stream_bounded(
        response.bytes_stream(),
        declared_length,
        maximum_bytes,
        repo_id,
        resolved_revision,
    )
    .await
}
/// Decode model metadata only after reapplying the acquisition byte limit.
///
/// Keeping this check adjacent to the parser prevents a future alternate fetch
/// path from bypassing the configured boundary.
pub(super) fn decode(
    body: &[u8],
    config: &SoracloudRuntimeHuggingFace,
    repo_id: &str,
    resolved_revision: &str,
) -> Result<norito::json::Value, SoracloudError> {
    let maximum_bytes = configured_maximum_bytes(config)?;
    let actual_length = u64::try_from(body.len()).unwrap_or(u64::MAX);
    if actual_length > maximum_bytes {
        return Err(SoracloudError::conflict(format!(
            "Hugging Face model info for `{repo_id}@{resolved_revision}` contains {actual_length} bytes before JSON decode, exceeding the configured {maximum_bytes}-byte limit"
        )));
    }
    norito::json::from_slice(body).map_err(|err| {
        SoracloudError::internal(format!(
            "failed to decode Hugging Face model info JSON for `{repo_id}@{resolved_revision}`: {err}"
        ))
    })
}
/// Derive the shared immutable weight selection used by Torii and the runtime importer.
pub(super) fn derive_weight_selection(
    model_info: &norito::json::Value,
    config: &SoracloudRuntimeHuggingFace,
    repo_id: &str,
    resolved_revision: &str,
) -> Result<Option<SoraHfWeightSelectionV1>, SoracloudError> {
    configured_maximum_weight_files(config)?;
    derive_hf_weight_selection_v1(
        model_info,
        config.import_max_files,
        config.import_max_file_bytes,
        config.import_max_total_bytes,
    )
    .map_err(|error| {
        SoracloudError::conflict(format!(
            "invalid immutable Hugging Face weight metadata for `{repo_id}@{resolved_revision}`: {error}"
        ))
    })
}
fn configured_maximum_bytes(config: &SoracloudRuntimeHuggingFace) -> Result<u64, SoracloudError> {
    let maximum_bytes = config.model_info_max_response_bytes;
    let hard_maximum = defaults::soracloud_runtime::hf::MODEL_INFO_MAX_RESPONSE_BYTES_LIMIT;
    if maximum_bytes == 0 || maximum_bytes > hard_maximum {
        return Err(SoracloudError::internal(format!(
            "soracloud_runtime.hf.model_info_max_response_bytes must be within 1..={hard_maximum}, found {maximum_bytes}"
        )));
    }
    Ok(maximum_bytes)
}
fn configured_maximum_weight_files(
    config: &SoracloudRuntimeHuggingFace,
) -> Result<(), SoracloudError> {
    let maximum_files = config.import_max_files;
    let hard_maximum = defaults::soracloud_runtime::hf::IMPORT_MAX_FILES_LIMIT;
    if maximum_files == 0 || maximum_files > hard_maximum {
        return Err(SoracloudError::internal(format!(
            "soracloud_runtime.hf.import_max_files must be within 1..={hard_maximum}, found {maximum_files}"
        )));
    }
    if config.import_max_file_bytes == 0
        || config.import_max_file_bytes
            > defaults::soracloud_runtime::hf::IMPORT_MAX_FILE_BYTES_LIMIT
        || config.import_max_total_bytes == 0
        || config.import_max_total_bytes
            > defaults::soracloud_runtime::hf::IMPORT_MAX_TOTAL_BYTES_LIMIT
    {
        return Err(SoracloudError::internal(
            "configured Hugging Face import byte limits are outside their first-release bounds",
        ));
    }
    Ok(())
}
fn declared_content_length(
    response: &reqwest::Response,
    repo_id: &str,
    resolved_revision: &str,
) -> Result<Option<u64>, SoracloudError> {
    let mut values = response
        .headers()
        .get_all(reqwest::header::CONTENT_LENGTH)
        .iter();
    let Some(value) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(SoracloudError::conflict(format!(
            "Hugging Face model info for `{repo_id}@{resolved_revision}` contains duplicate Content-Length headers"
        )));
    }
    let value = value.to_str().map_err(|_| {
        SoracloudError::conflict(format!(
            "Hugging Face model info for `{repo_id}@{resolved_revision}` contains a non-ASCII Content-Length header"
        ))
    })?;
    value.trim().parse::<u64>().map(Some).map_err(|_| {
        SoracloudError::conflict(format!(
            "Hugging Face model info for `{repo_id}@{resolved_revision}` contains an invalid Content-Length header"
        ))
    })
}
async fn collect_stream_bounded<S, C, E>(
    chunks: S,
    declared_length: Option<u64>,
    maximum_bytes: u64,
    repo_id: &str,
    resolved_revision: &str,
) -> Result<Vec<u8>, SoracloudError>
where
    S: Stream<Item = Result<C, E>>,
    C: AsRef<[u8]>,
    E: fmt::Display,
{
    let maximum = usize::try_from(maximum_bytes).map_err(|_| {
        SoracloudError::internal(format!(
            "configured Hugging Face model-info response limit {maximum_bytes} does not fit this host"
        ))
    })?;
    let retained_limit = maximum.checked_add(1).ok_or_else(|| {
        SoracloudError::internal("configured Hugging Face model-info response limit overflow")
    })?;
    let mut body = Vec::new();
    futures_util::pin_mut!(chunks);
    while let Some(chunk) = chunks.next().await {
        let chunk = chunk.map_err(|err| {
            SoracloudError::internal(format!(
                "failed to stream Hugging Face model info for `{repo_id}@{resolved_revision}` after {} bytes: {err}",
                body.len()
            ))
        })?;
        let chunk = chunk.as_ref();
        let remaining = retained_limit.checked_sub(body.len()).ok_or_else(|| {
            SoracloudError::internal("Hugging Face model-info response length overflow")
        })?;
        let retained = remaining.min(chunk.len());
        let required_length = body.len().checked_add(retained).ok_or_else(|| {
            SoracloudError::internal("Hugging Face model-info response length overflow")
        })?;
        reserve_to(&mut body, required_length, retained_limit)?;
        body.extend_from_slice(&chunk[..retained]);
        if retained < chunk.len() || body.len() > maximum {
            return Err(SoracloudError::conflict(format!(
                "Hugging Face model info for `{repo_id}@{resolved_revision}` exceeded the configured {maximum_bytes}-byte limit while streaming"
            )));
        }
    }
    let actual_length = u64::try_from(body.len()).unwrap_or(u64::MAX);
    if let Some(length) = declared_length
        && length != actual_length
    {
        return Err(SoracloudError::conflict(format!(
            "Hugging Face model info for `{repo_id}@{resolved_revision}` streamed {actual_length} bytes, which does not match Content-Length {length}"
        )));
    }
    Ok(body)
}
fn reserve_to(
    body: &mut Vec<u8>,
    required_length: usize,
    retained_limit: usize,
) -> Result<(), SoracloudError> {
    if body.capacity() >= required_length {
        return Ok(());
    }
    let target = if body.capacity() == 0 {
        INITIAL_ALLOCATION_BYTES
    } else {
        body.capacity().saturating_mul(2)
    }
    .max(required_length)
    .min(retained_limit);
    body.try_reserve_exact(target.saturating_sub(body.len()))
        .map_err(|err| {
            SoracloudError::internal(format!(
                "failed to grow bounded Hugging Face model-info response buffer: {err}"
            ))
        })
}
#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::stream;
    use http::header::{CONTENT_LENGTH, TRANSFER_ENCODING};
    use std::{io, task::Poll};
    const REPO_ID: &str = "org/model";
    const REVISION: &str = "revision";
    fn config_with_limit(maximum_bytes: u64) -> SoracloudRuntimeHuggingFace {
        SoracloudRuntimeHuggingFace {
            model_info_max_response_bytes: maximum_bytes,
            ..SoracloudRuntimeHuggingFace::default()
        }
    }
    fn streaming_response(chunks: &[&[u8]], declared_length: Option<u64>) -> reqwest::Response {
        let chunks = chunks
            .iter()
            .map(|chunk| Ok::<Vec<u8>, io::Error>(chunk.to_vec()))
            .collect::<Vec<_>>();
        let body = reqwest::Body::wrap_stream(stream::iter(chunks));
        let mut response = http::Response::builder().status(200);
        response = if let Some(length) = declared_length {
            response.header(CONTENT_LENGTH, length)
        } else {
            response.header(TRANSFER_ENCODING, "chunked")
        };
        response.body(body).expect("build response fixture").into()
    }
    #[tokio::test]
    async fn accepts_exact_content_length_boundary() {
        let response = streaming_response(&[b"1234", b"5678"], Some(8));
        let body = read(response, &config_with_limit(8), REPO_ID, REVISION)
            .await
            .expect("exact declared limit must be accepted");
        assert_eq!(body, b"12345678");
    }
    #[tokio::test]
    async fn chunked_response_is_bounded_at_max_plus_one() {
        let exact = streaming_response(&[b"12", b"345", b"678"], None);
        assert_eq!(
            read(exact, &config_with_limit(8), REPO_ID, REVISION)
                .await
                .expect("exact chunked limit must be accepted"),
            b"12345678"
        );
        let oversized = streaming_response(&[b"12", b"345", b"6789"], None);
        let error = read(oversized, &config_with_limit(8), REPO_ID, REVISION)
            .await
            .expect_err("chunked max-plus-one body must fail");
        assert!(error.message.contains("while streaming"));
    }
    #[tokio::test]
    async fn content_length_preflight_does_not_poll_an_oversized_body() {
        let body = reqwest::Body::wrap_stream(stream::poll_fn(
            |_| -> Poll<Option<Result<Vec<u8>, io::Error>>> {
                panic!("oversized Content-Length must reject before polling the body")
            },
        ));
        let response: reqwest::Response = http::Response::builder()
            .status(200)
            .header(CONTENT_LENGTH, 9)
            .body(body)
            .expect("build oversized response fixture")
            .into();
        let error = read(response, &config_with_limit(8), REPO_ID, REVISION)
            .await
            .expect_err("oversized declaration must fail");
        assert!(error.message.contains("declares 9 bytes"));
    }
    #[tokio::test]
    async fn rejects_content_length_lies_in_both_directions() {
        for declared_length in [7, 9] {
            let response = streaming_response(&[b"1234", b"5678"], Some(declared_length));
            let error = read(response, &config_with_limit(16), REPO_ID, REVISION)
                .await
                .expect_err("misreported declaration must fail");
            assert!(error.message.contains("does not match Content-Length"));
        }
    }
    #[test]
    fn decode_reapplies_the_configured_body_limit() {
        let config = config_with_limit(2);
        assert_eq!(
            decode(b"{}", &config, REPO_ID, REVISION).expect("exact limit JSON"),
            norito::json!({})
        );
        let error = decode(b"{} ", &config, REPO_ID, REVISION)
            .expect_err("max-plus-one input must fail before JSON decode");
        assert!(error.message.contains("before JSON decode"));
    }
    #[test]
    fn weight_selection_uses_shared_authenticated_precedence_and_sorted_set() {
        let model_info = norito::json!({
            "siblings": [
                {"rfilename": "fallback.safetensors", "lfs": {"sha256": ("33".repeat(32)), "size": 3}},
                {"rfilename": "shard-2.gguf", "lfs": {"sha256": ("22".repeat(32)), "size": 2}},
                {"rfilename": "notes.txt"},
                {"rfilename": "shard-1.gguf", "lfs": {"sha256": ("11".repeat(32)), "size": 1}}
            ]
        });
        let config = config_with_limit(1024);
        let selected = derive_weight_selection(&model_info, &config, REPO_ID, REVISION)
            .expect("valid selection")
            .expect("supported weights");
        assert_eq!(
            selected.backend_family,
            iroha_data_model::soracloud::SoraHfBackendFamilyV1::Gguf
        );
        assert_eq!(
            selected.model_format,
            iroha_data_model::soracloud::SoraHfModelFormatV1::Gguf
        );
        assert_eq!(selected.required_model_bytes, 3);
        assert_eq!(
            selected
                .required_weight_files
                .iter()
                .map(|weight| weight.path.as_str())
                .collect::<Vec<_>>(),
            ["shard-1.gguf", "shard-2.gguf"]
        );
        assert!(
            derive_weight_selection(
                &norito::json!({"siblings": [{"rfilename": "notes.txt"}]}),
                &config,
                REPO_ID,
                REVISION,
            )
            .expect("valid unsupported selection")
            .is_none()
        );
    }
    #[test]
    fn weight_selection_rejects_uppercase_artifact_suffix() {
        let model_info = norito::json!({
            "siblings": [
                {"rfilename": "model.GGUF", "lfs": {"sha256": ("11".repeat(32)), "size": 1}}
            ]
        });
        let error =
            derive_weight_selection(&model_info, &config_with_limit(1024), REPO_ID, REVISION)
                .expect_err("uppercase GGUF compatibility spelling must be rejected");
        assert!(
            error
                .message
                .contains("must use an exact canonical lowercase file extension")
        );
    }
    #[test]
    fn weight_selection_deduplicates_then_enforces_the_import_file_cap() {
        let model_info = norito::json!({
            "siblings": [
                {"rfilename": "same.gguf", "lfs": {"sha256": ("11".repeat(32)), "size": 1}},
                {"rfilename": "same.gguf", "lfs": {"sha256": ("11".repeat(32)), "size": 1}},
                {"rfilename": "second.gguf", "lfs": {"sha256": ("22".repeat(32)), "size": 1}}
            ]
        });
        let mut config = config_with_limit(1024);
        config.import_max_files = 1;
        let error = derive_weight_selection(&model_info, &config, REPO_ID, REVISION)
            .expect_err("second unique weight must exceed the source file cap");
        assert!(error.message.contains("outside 1..=1"));
    }
}
