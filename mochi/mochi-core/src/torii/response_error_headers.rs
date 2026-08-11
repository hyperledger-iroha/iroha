fn reject_code_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-iroha-reject-code")
        .or_else(|| headers.get("x-iroha-axt-code"))
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

fn retry_after_from_headers(headers: &HeaderMap) -> Option<Duration> {
    headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
}
