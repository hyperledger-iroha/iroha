const NORITO_MIME_TYPE: &str = "application/x-norito";
const MAX_SUMERAGI_OPERATOR_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

async fn read_bounded_sumeragi_response(mut response: Response) -> ToriiResult<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_SUMERAGI_OPERATOR_RESPONSE_BYTES as u64)
    {
        return Err(ToriiError::Decode(format!(
            "Sumeragi operator response exceeds the {}-byte bound",
            MAX_SUMERAGI_OPERATOR_RESPONSE_BYTES
        )));
    }
    let capacity = response
        .content_length()
        .and_then(|length| usize::try_from(length).ok())
        .unwrap_or(0)
        .min(MAX_SUMERAGI_OPERATOR_RESPONSE_BYTES);
    let mut body = Vec::with_capacity(capacity);
    while let Some(chunk) = response.chunk().await? {
        let next_len = body.len().checked_add(chunk.len()).ok_or_else(|| {
            ToriiError::Decode("Sumeragi operator response length overflowed".to_owned())
        })?;
        if next_len > MAX_SUMERAGI_OPERATOR_RESPONSE_BYTES {
            return Err(ToriiError::Decode(format!(
                "Sumeragi operator response exceeds the {}-byte bound",
                MAX_SUMERAGI_OPERATOR_RESPONSE_BYTES
            )));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}
