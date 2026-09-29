//! Exact mutable runtime path projection for the internal fixed generator.
use super::*;

/// Bind remaining default-owned writable paths to the original role namespace.
pub(in crate::localnet) fn bind(
    rendered: Zeroizing<String>,
    paths: &LocalnetPeerStoragePaths,
) -> Result<Zeroizing<String>> {
    let mut root = crate::secret_toml::Table::new(crate::secret_toml::parse_table(
        &rendered,
        "fixed runtime paths",
    )?);
    let mut snapshot = Table::new();
    snapshot.insert(
        "store_dir".into(),
        Value::String(paths.state.join("snapshot").to_string_lossy().into_owned()),
    );
    crate::secret_toml::insert(&mut root, "snapshot".into(), Value::Table(snapshot));
    let sorafs = root
        .get_mut("sorafs")
        .and_then(Value::as_table_mut)
        .ok_or_else(|| eyre!("fixed runtime lacks SoraFS"))?;
    let mut discovery = Table::new();
    discovery.insert(
        "replay_checkpoint_path".into(),
        Value::String(
            paths
                .torii
                .join("sorafs_discovery_replay.nrt")
                .to_string_lossy()
                .into_owned(),
        ),
    );
    crate::secret_toml::insert(sorafs, "discovery".into(), Value::Table(discovery));
    toml::to_string(&*root)
        .map(Zeroizing::new)
        .map_err(|_| eyre!("fixed runtime config rendering failed"))
}

/// Verify actual resolved paths before freezing final genesis authority.
/// Optional persisted subsystems not provisioned by this fixed producer must remain absent.
pub(in crate::localnet) fn validate(
    config: &actual::Root,
    paths: &LocalnetPeerStoragePaths,
) -> Result<()> {
    let snapshot = paths.state.join("snapshot");
    let discovery = paths.torii.join("sorafs_discovery_replay.nrt");
    let vrf = paths
        .sorafs_por
        .join(iroha_config::parameters::defaults::sorafs::por::VRF_STATE_FILE);
    let drand = paths
        .sorafs_por
        .join(iroha_config::parameters::defaults::sorafs::por::DRAND_STATE_FILE);
    ensure!(
        config.kura.store_dir.value() == &paths.kura
            && config.snapshot.store_dir.value() == &snapshot
            && config.soracloud_runtime.state_dir == paths.soracloud_runtime
            && config.tiered_state.cold_store_root.as_ref() == Some(&paths.tiered_state)
            && config.tiered_state.da_store_root.as_ref() == Some(&paths.da_store)
            && config.streaming.session_store_dir == paths.streaming_sessions
            && Path::new(
                config
                    .network
                    .soranet_handshake
                    .pow
                    .revocation_store_path
                    .as_ref()
            ) == paths.soranet_ticket_revocations
            && config.torii.data_dir == paths.torii
            && config.torii.da_ingest.replay_cache_store_dir == paths.torii_da_replay_cache
            && config.torii.da_ingest.manifest_store_dir == paths.torii_da_manifests
            && config.torii.sorafs_discovery.replay_checkpoint_path == discovery
            && config.torii.sorafs_storage.data_dir == paths.sorafs
            && config.torii.sorafs_por.state_dir == paths.sorafs_por
            && config.torii.sorafs_por.vrf_state_path == vrf
            && config.torii.sorafs_por.drand.state_path == drand,
        "fixed runtime path differs from its original role namespace"
    );
    ensure!(
        config.torii.account_onboarding.is_none()
            && config.torii.faucet.is_none()
            && config.torii.sorafs_storage.pop_credentials.is_none()
            && config
                .torii
                .sorafs_storage
                .moderation_orchestrator
                .is_none()
            && config.torii.sorafs_storage.evidence_viewer.is_none()
            && config.torii.sorafs_gc.state_dir.is_none()
            && config.torii.sorafs_gateway.compliance.is_none()
            && config.torii.privacy_bootle_lantern_issuer.is_none()
            && config.telemetry_integrity.state_dir.is_none()
            && config.dev_telemetry.out_file.is_none()
            && config.torii.sorafs_storage.reputation_runtime.is_none()
            && config
                .torii
                .sorafs_storage
                .reserve_transparency_runtime
                .is_none()
            && config.torii.sorafs_storage.por_replay_archive.is_none()
            && config
                .torii
                .sorafs_storage
                .provider_ingest_runtime
                .is_none()
            && config
                .torii
                .sorafs_storage
                .hedging_billing_runtime
                .is_none()
            && config.torii.sorafs_storage.governance_dag_dir.is_none()
            && config
                .torii
                .sorafs_storage
                .governance_dag_service
                .state_dir
                .is_none()
            && !config.torii.iso_bridge.enabled,
        "fixed runtime has an unprovisioned persisted subsystem"
    );
    Ok(())
}
