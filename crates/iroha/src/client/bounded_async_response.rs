//! Bounded conversion for asynchronous transaction-ingress responses.

use eyre::{Result, WrapErr as _, eyre};
use http::Response;
use std::sync::OnceLock;

pub(super) fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(build_client)
}

pub(super) fn build_client() -> reqwest::Client {
    reqwest::Client::builder()
        // A redirect can arrive after ingress admitted a one-shot signed transaction.
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .expect("Failed to build async HTTP client")
}

pub(super) async fn into_response(
    mut response: reqwest::Response,
    maximum_body_bytes: usize,
) -> Result<Response<Vec<u8>>> {
    if maximum_body_bytes == 0 {
        return Err(eyre!("HTTP response byte limit must be positive"));
    }
    let status = response.status();
    let headers: Vec<_> = response
        .headers()
        .iter()
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    if response
        .content_length()
        .is_some_and(|length| length > u64::try_from(maximum_body_bytes).unwrap_or(u64::MAX))
    {
        return Err(eyre!(
            "async HTTP response exceeds the {maximum_body_bytes} byte limit"
        ));
    }
    let mut body = Vec::new();
    body.try_reserve_exact(
        response
            .content_length()
            .and_then(|length| usize::try_from(length).ok())
            .unwrap_or(0)
            .min(maximum_body_bytes),
    )?;
    while let Some(chunk) = response
        .chunk()
        .await
        .wrap_err("Failed to read bounded async response bytes")?
    {
        if body
            .len()
            .checked_add(chunk.len())
            .is_none_or(|length| length > maximum_body_bytes)
        {
            return Err(eyre!(
                "async HTTP response exceeds the {maximum_body_bytes} byte limit"
            ));
        }
        body.extend_from_slice(&chunk);
    }
    let mut builder = Response::builder().status(status);
    let headers_map = builder
        .headers_mut()
        .ok_or_else(|| eyre!("Failed to get headers map reference."))?;
    for (key, value) in headers {
        headers_map.insert(key, value);
    }
    builder
        .body(body)
        .wrap_err("Failed to construct response bytes body")
}
