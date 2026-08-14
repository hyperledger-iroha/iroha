//! Strict public binding and resource bounds for stream-token admission.
use super::*;
pub(super) fn decode_policy_digest(
    value: Option<&str>,
    field: &'static str,
    emitter: &mut Emitter<ParseError>,
) -> Option<[u8; 32]> {
    value.and_then(|value| {
        let canonical = value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
        if !canonical {
            emitter.emit(Report::new(ParseError::InvalidSorafsConfig).attach(format!(
                "sorafs.storage.stream_tokens.{field} must be exactly 64 lowercase hexadecimal characters"
            )));
            return None;
        }
        let digest: [u8; 32] = hex::decode(value)
            .expect("validated lowercase policy digest hex")
            .try_into()
            .expect("validated 32-byte policy digest");
        if digest == [0; 32] {
            emitter.emit(Report::new(ParseError::InvalidSorafsConfig).attach(format!(
                "sorafs.storage.stream_tokens.{field} must be non-zero"
            )));
            return None;
        }
        Some(digest)
    })
}
pub(super) fn validate_binding_and_bounds(
    config: &SorafsStreamTokenConfig,
    emitter: &mut Emitter<ParseError>,
) {
    if config.enabled {
        match config.admission_provider_handle.as_deref() {
            Some(handle) if is_production_runtime_handle(handle) => {}
            Some(_) => emitter.emit(Report::new(ParseError::InvalidSorafsConfig).attach(
                "sorafs.storage.stream_tokens.admission_provider_handle must be a canonical credential-free production runtime handle",
            )),
            None => emitter.emit(Report::new(ParseError::InvalidSorafsConfig).attach(
                "sorafs.storage.stream_tokens.admission_provider_handle is required when issuance is enabled",
            )),
        }
        match config.admission_provider_revision {
            Some(0) => emitter.emit(Report::new(ParseError::InvalidSorafsConfig).attach(
                "sorafs.storage.stream_tokens.admission_provider_revision must be non-zero",
            )),
            None => emitter.emit(Report::new(ParseError::InvalidSorafsConfig).attach(
                "sorafs.storage.stream_tokens.admission_provider_revision is required when issuance is enabled",
            )),
            Some(_) => {}
        }
        if config.admission_provider_policy_digest_hex.is_none() {
            emitter.emit(Report::new(ParseError::InvalidSorafsConfig).attach(
                "sorafs.storage.stream_tokens.admission_provider_policy_digest_hex is required when issuance is enabled",
            ));
        }
    }
    for (field, value, maximum) in [
        (
            "admission_max_pending",
            config.admission_max_pending,
            1_000_000,
        ),
        (
            "admission_max_tracked_tokens",
            config.admission_max_tracked_tokens,
            1_000_000,
        ),
        (
            "admission_reconcile_max_items",
            config.admission_reconcile_max_items,
            1_024,
        ),
    ] {
        if value == 0 || value > maximum {
            emitter.emit(Report::new(ParseError::InvalidSorafsConfig).attach(format!(
                "sorafs.storage.stream_tokens.{field} must be within 1..={maximum}"
            )));
        }
    }
    if config.admission_lease_ttl_ms == 0 || config.admission_lease_ttl_ms > 300_000 {
        emitter.emit(Report::new(ParseError::InvalidSorafsConfig).attach(
            "sorafs.storage.stream_tokens.admission_lease_ttl_ms must be within 1..=300000",
        ));
    }
}
