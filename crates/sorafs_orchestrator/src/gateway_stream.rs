//! Consuming gateway retrieval into a caller-owned, bounded disk spool.
use super::*;
use std::io::{Read, Seek, SeekFrom, Write};

fn spool_error(error: impl std::fmt::Display) -> GatewayOrchestratorError {
    GatewayOrchestratorError::CarBuild(format!("payload spool: {error}"))
}

/// Retrieve and verify an object into an initially empty seekable writer.
///
/// The caller owns publication and cleanup: supply a private temporary file and publish it only
/// after this function returns successfully. Configured payload size is admitted before payload
/// requests. The scheduler consumes chunks in order and bounds the sum of reserved requests and
/// completed responses. Final native verification makes bounded passes over the spool, without a
/// complete payload or CAR copy. The writer is returned at offset zero only after verification.
#[allow(clippy::too_many_arguments)]
pub async fn fetch_via_gateway_to_writer<W: Read + Write + Seek + Send + 'static>(
    config: OrchestratorConfig,
    plan: &CarBuildPlan,
    gateway_config: GatewayFetchConfig,
    providers: impl IntoIterator<Item = GatewayProviderInput>,
    telemetry: Option<&TelemetrySnapshot>,
    max_peers: Option<usize>,
    mut writer: W,
) -> Result<(StreamFetchSession, W), GatewayOrchestratorError> {
    bounded_fetch_options(&config.fetch)?;
    let started = tokio::time::Instant::now();
    let timeout = config.fetch.session_timeout;
    let deadline = started
        .checked_add(timeout)
        .ok_or_else(|| spool_error("invalid deadline"))?;
    if writer.seek(SeekFrom::End(0)).map_err(spool_error)? != 0 {
        return Err(spool_error("writer must be empty"));
    }
    writer.rewind().map_err(spool_error)?;
    let (context, mut orchestrator, scoreboard) = prepare_gateway_fetch(
        config,
        plan,
        gateway_config,
        providers,
        telemetry,
        max_peers,
    )?;
    let manifest = tokio::time::timeout_at(deadline, context.fetch_manifest())
        .await
        .map_err(|_| OrchestratorError::from(multi_fetch::MultiSourceError::DeadlineExceeded))??;
    validate_gateway_manifest_context(plan, &ManifestVerificationContext::from(&manifest))?;
    orchestrator.config.fetch.session_timeout =
        deadline.saturating_duration_since(tokio::time::Instant::now());
    if orchestrator.config.fetch.session_timeout.is_zero() {
        return Err(
            OrchestratorError::from(multi_fetch::MultiSourceError::DeadlineExceeded).into(),
        );
    }
    let shared = Arc::new(Mutex::new(writer));
    let sink = Arc::clone(&shared);
    let fetcher = context.fetcher();
    let mut session = orchestrator
        .fetch_with_scoreboard_and_observer(
            plan,
            &scoreboard,
            fetcher.as_closure(),
            move |delivery: multi_fetch::ChunkDelivery<'_>| {
                sink.lock()
                    .map_err(|_| multi_fetch::ObserverError::new("payload spool mutex poisoned"))?
                    .write_all(delivery.bytes)
                    .map_err(|error| multi_fetch::ObserverError::new(error.to_string()))
            },
        )
        .await?;
    let mut writer = Arc::try_unwrap(shared)
        .map_err(|_| spool_error("sink retained after fetch"))?
        .into_inner()
        .map_err(|_| spool_error("payload spool mutex poisoned"))?;
    writer.flush().map_err(spool_error)?;
    session.car_verification = Some(verify_reader_against_manifest(
        plan,
        &mut writer,
        ManifestVerificationContext::from(&manifest),
    )?);
    if tokio::time::Instant::now() >= deadline {
        return Err(
            OrchestratorError::from(multi_fetch::MultiSourceError::DeadlineExceeded).into(),
        );
    }
    Ok((session, writer))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gateway_config() -> GatewayFetchConfig {
        GatewayFetchConfig {
            manifest_id_hex: "ab".repeat(32),
            chunker_handle: "sorafs.sf1@1.0.0".into(),
            manifest_envelope_b64: None,
            client_id: None,
            expected_manifest_cid_hex: None,
            blinded_cid_b64: None,
            salt_epoch: None,
            expected_cache_version: None,
        }
    }

    #[tokio::test]
    async fn spool_admission_rejects_nonempty_writer_and_cleans_it_up() {
        let plan = CarBuildPlan::single_file(b"payload").unwrap();
        let mut spool = tempfile::NamedTempFile::new().unwrap();
        let temporary_path = spool.path().to_owned();
        spool.write_all(b"existing bytes").unwrap();
        let error = fetch_via_gateway_to_writer(
            OrchestratorConfig::default(),
            &plan,
            gateway_config(),
            [],
            None,
            None,
            spool,
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("writer must be empty"));
        assert!(!temporary_path.exists());
    }

    #[tokio::test]
    async fn object_and_metadata_admission_precede_provider_selection_and_clean_spool() {
        let plan = CarBuildPlan::single_file(b"payload").unwrap();
        for metadata_limit in [false, true] {
            let mut config = OrchestratorConfig::default();
            if metadata_limit {
                config.fetch.max_metadata_entries = 1;
            } else {
                config.fetch.max_payload_bytes = 1;
            }
            let spool = tempfile::NamedTempFile::new().unwrap();
            let temporary_path = spool.path().to_owned();
            let error =
                fetch_via_gateway_to_writer(config, &plan, gateway_config(), [], None, None, spool)
                    .await
                    .unwrap_err();
            assert!(error.to_string().contains(if metadata_limit {
                "metadata inventory"
            } else {
                "object limit"
            }));
            assert!(!temporary_path.exists());
        }
    }
}
