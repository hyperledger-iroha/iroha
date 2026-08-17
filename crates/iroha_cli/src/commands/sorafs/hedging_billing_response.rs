//! Strict response binding for exact-checkpoint hedging and billing reads.
use crate::RunContext;
use eyre::{Result, eyre};
use iroha::http::{Response, StatusCode};
use norito::json::Value;
/// Validate and render one JSON page bound to the requested projection checkpoint.
pub(super) fn render<C: RunContext>(
    context: &mut C,
    response: Response<Vec<u8>>,
    expected_checkpoint: &str,
) -> Result<()> {
    let status = response.status();
    let exact_json_content_type = {
        let mut values = response.headers().get_all("content-type").iter();
        values.next().and_then(|value| value.to_str().ok()) == Some("application/json")
            && values.next().is_none()
    };
    let body = response.into_body();
    if status != StatusCode::OK {
        return Err(eyre!(
            "SoraFS hedging/billing exact-checkpoint response returned status {status}"
        ));
    }
    if !exact_json_content_type {
        return Err(eyre!(
            "SoraFS hedging/billing exact-checkpoint response must use application/json"
        ));
    }
    let value: Value = norito::json::from_slice(&body).map_err(|_| {
        eyre!("SoraFS hedging/billing exact-checkpoint response must contain valid JSON")
    })?;
    let returned_checkpoint = value
        .get("anchor")
        .and_then(Value::as_object)
        .and_then(|anchor| anchor.get("checkpoint_fingerprint"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            eyre!(
                "SoraFS hedging/billing exact-checkpoint response is missing anchor.checkpoint_fingerprint"
            )
        })?;
    if returned_checkpoint != expected_checkpoint.to_ascii_uppercase() {
        return Err(eyre!(
            "SoraFS hedging/billing exact-checkpoint response anchor does not match the request"
        ));
    }
    context.print_data(&value)
}
