const NORITO_MIME_TYPE: &str = "application/x-norito";
const MAX_ERROR_RESPONSE_BYTES: usize = 64 * 1024;
const MAX_JSON_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const MAX_QUERY_RESPONSE_BYTES: usize = 64 * 1024 * 1024;
const MAX_STATUS_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const MAX_METRICS_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const MAX_SUMERAGI_OPERATOR_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

async fn read_bounded_response(
    mut response: Response,
    maximum: usize,
    context: &'static str,
) -> ToriiResult<Vec<u8>> {
    let maximum_u64 = u64::try_from(maximum).unwrap_or(u64::MAX);
    if response
        .content_length()
        .is_some_and(|length| length > maximum_u64)
    {
        return Err(ToriiError::ResponseResourceLimit { context, maximum });
    }
    let capacity = response
        .content_length()
        .and_then(|length| usize::try_from(length).ok())
        .unwrap_or(0)
        .min(maximum)
        .min(64 * 1024);
    let mut body = Vec::new();
    body.try_reserve_exact(capacity)
        .map_err(|_| ToriiError::ResponseResourceLimit { context, maximum })?;
    while let Some(chunk) = response.chunk().await? {
        let Some(next_len) = body.len().checked_add(chunk.len()) else {
            return Err(ToriiError::ResponseResourceLimit { context, maximum });
        };
        if next_len > maximum {
            return Err(ToriiError::ResponseResourceLimit { context, maximum });
        }
        body.try_reserve_exact(chunk.len())
            .map_err(|_| ToriiError::ResponseResourceLimit { context, maximum })?;
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

async fn read_bounded_sumeragi_response(response: Response) -> ToriiResult<Vec<u8>> {
    read_bounded_response(
        response,
        MAX_SUMERAGI_OPERATOR_RESPONSE_BYTES,
        "Sumeragi operator",
    )
    .await
}
