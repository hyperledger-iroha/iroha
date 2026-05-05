#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Ensure SoraFS reconciliation reports are emitted across peers and surface divergence.

use std::{
    collections::BTreeSet,
    fs,
    path::Path,
    time::{Duration, Instant},
};

use eyre::{Result, WrapErr, eyre};
use integration_tests::sandbox::start_network_async_or_skip;
use iroha_test_network::NetworkBuilder;
use sorafs_car::{CarBuildPlan, CarWriter};
use sorafs_manifest::{
    DagCodecId, GovernanceProofs, ManifestBuilder, ManifestV1, PinPolicy,
    SorafsReconciliationReportV1, StorageClass, chunker_registry,
};

const REPORT_TIMEOUT: Duration = Duration::from_secs(20);
const REPORT_POLL_INTERVAL: Duration = Duration::from_millis(200);
const RECONCILIATION_DIR: &str = "sorafs/governance/reconciliation";

fn build_manifest(payload: &[u8]) -> Result<(ManifestV1, CarBuildPlan)> {
    let descriptor = chunker_registry::default_descriptor();
    let plan = CarBuildPlan::single_file_with_profile(payload, descriptor.profile)?;
    let mut car_bytes = Vec::new();
    let stats = CarWriter::new(&plan, payload)?.write_to(&mut car_bytes)?;
    let root = stats
        .root_cids
        .first()
        .cloned()
        .ok_or_else(|| eyre!("car emission produced no root CID"))?;
    let manifest = ManifestBuilder::new()
        .root_cid(root)
        .dag_codec(DagCodecId(stats.dag_codec))
        .chunking_from_registry(descriptor.id)
        .content_length(plan.content_length)
        .car_digest(stats.car_archive_digest.into())
        .car_size(stats.car_size)
        .pin_policy(PinPolicy {
            min_replicas: 1,
            storage_class: StorageClass::Hot,
            retention_epoch: 0,
        })
        .governance(GovernanceProofs {
            council_signatures: Vec::new(),
        })
        .build()?;
    Ok((manifest, plan))
}

fn load_reconciliation_reports(dir: &Path) -> Result<Vec<SorafsReconciliationReportV1>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err.into()),
    };
    let mut reports = Vec::new();
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("to") {
            continue;
        }
        let bytes = fs::read(&path)?;
        let report = norito::decode_from_bytes::<SorafsReconciliationReportV1>(&bytes)
            .wrap_err_with(|| format!("decode reconciliation report {}", path.display()))?;
        report
            .validate()
            .wrap_err_with(|| format!("validate reconciliation report {}", path.display()))?;
        reports.push(report);
    }
    Ok(reports)
}

async fn wait_for_reconciliation_report<F>(
    dir: &Path,
    timeout: Duration,
    predicate: F,
) -> Result<SorafsReconciliationReportV1>
where
    F: Fn(&SorafsReconciliationReportV1) -> bool,
{
    let deadline = Instant::now() + timeout;
    loop {
        let reports = load_reconciliation_reports(dir)?;
        if let Some(report) = reports
            .into_iter()
            .filter(|report| predicate(report))
            .max_by_key(|report| report.generated_at_unix)
        {
            return Ok(report);
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "timed out waiting for reconciliation report in {}",
                dir.display()
            ));
        }
        tokio::time::sleep(REPORT_POLL_INTERVAL).await;
    }
}

#[tokio::test]
async fn sorafs_reconciliation_reports_detect_divergence() -> Result<()> {
    let builder = NetworkBuilder::new()
        .with_min_peers(4)
        .with_config_layer(|layer| {
            layer
                .write(["sorafs", "storage", "enabled"], true)
                .write(
                    ["sorafs", "storage", "governance_dag_dir"],
                    "storage/sorafs/governance",
                )
                .write(["sorafs", "gc", "enabled"], true)
                .write(["sorafs", "gc", "interval_secs"], 1);
        });
    let Some(network) = start_network_async_or_skip(
        builder,
        stringify!(sorafs_reconciliation_reports_detect_divergence),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let payload = b"sorafs reconciliation divergence payload";
    let (manifest, _plan) = build_manifest(payload)?;
    let manifest_bytes = manifest.encode()?;
    let client = network.client();
    let response = client.post_sorafs_storage_pin(&manifest_bytes, payload, None)?;
    assert!(
        response.status().is_success(),
        "storage pin rejected: {}",
        response.status()
    );

    let pinned_peer = network.peers().first().expect("peer");
    let pinned_dir = pinned_peer.kura_store_dir().join(RECONCILIATION_DIR);
    let pinned_report = wait_for_reconciliation_report(&pinned_dir, REPORT_TIMEOUT, |report| {
        report.retention_manifest_count > 0
    })
    .await
    .wrap_err("pinned peer reconciliation report missing")?;

    let mut reports = Vec::with_capacity(network.peers().len());
    reports.push(pinned_report.clone());
    for peer in network.peers().iter().skip(1) {
        let dir = peer.kura_store_dir().join(RECONCILIATION_DIR);
        let report = wait_for_reconciliation_report(&dir, REPORT_TIMEOUT, |report| {
            report.generated_at_unix >= pinned_report.generated_at_unix
        })
        .await
        .wrap_err_with(|| format!("reconciliation report missing for {}", dir.display()))?;
        reports.push(report);
    }

    let manifest_counts: BTreeSet<u32> = reports
        .iter()
        .map(|report| report.retention_manifest_count)
        .collect();
    assert!(
        manifest_counts.len() > 1,
        "expected divergent retention manifest counts across peers; got {manifest_counts:?}"
    );

    let retention_hashes: BTreeSet<[u8; 32]> = reports
        .iter()
        .map(|report| report.retention_snapshot_hash)
        .collect();
    assert!(
        retention_hashes.len() > 1,
        "expected divergent retention snapshot hashes across peers"
    );

    Ok(())
}
