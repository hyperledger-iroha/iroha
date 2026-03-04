//! SORA runtime-upgrade governance resilience tests on 4-peer local networks.

#[path = "common/sora_runtime_governance.rs"]
mod sora_runtime_governance;

use std::time::Duration;

use eyre::{Result, WrapErr, eyre};
use iroha::data_model::{
    isi::governance::{ProposeRuntimeUpgradeProposal, VotingMode},
    runtime::RuntimeUpgradeManifest,
};

#[tokio::test]
async fn sora_runtime_upgrade_resilience_peer_restart_during_multibody_sortition() -> Result<()> {
    let Some(mut fixture) = sora_runtime_governance::setup_runtime_governance_fixture(stringify!(
        sora_runtime_upgrade_resilience_peer_restart_during_multibody_sortition
    ))
    .await?
    else {
        return Ok(());
    };

    let peer_count = fixture.network.peers().len();
    if peer_count < 2 {
        return Err(eyre!(
            "runtime resilience restart scenario requires at least 2 peers"
        ));
    }
    let restart_idx = (fixture.ready_peer_idx + 1) % peer_count;
    let outcome = sora_runtime_governance::enact_runtime_upgrade_round(
        &mut fixture,
        "restart-round",
        Some(restart_idx),
    )
    .await?;

    for (peer_idx, peer) in fixture.network.peers().iter().enumerate() {
        let mut peer_client = peer.client();
        sora_runtime_governance::tune_client_timeouts(&mut peer_client);
        let runtime_status = sora_runtime_governance::wait_for_runtime_upgrade_status(
            &peer_client,
            &outcome.runtime_upgrade_id_hex,
            "ActivatedAt",
            Duration::from_secs(180),
        )
        .await
        .wrap_err_with(|| {
            format!(
                "peer #{peer_idx} should observe runtime upgrade `{}` activation",
                outcome.runtime_upgrade_id_hex
            )
        })?;
        let activated_height = runtime_status
            .get("ActivatedAt")
            .and_then(norito::json::Value::as_u64)
            .ok_or_else(|| eyre!("peer #{peer_idx} runtime status missing ActivatedAt"))?;
        assert_eq!(
            activated_height, outcome.activated_height,
            "peer #{peer_idx} should agree on runtime activation height"
        );
    }

    Ok(())
}

#[tokio::test]
#[ignore = "long-running runtime governance chain soak"]
async fn sora_runtime_upgrade_resilience_chained_rounds_soak() -> Result<()> {
    let Some(mut fixture) = sora_runtime_governance::setup_runtime_governance_fixture(stringify!(
        sora_runtime_upgrade_resilience_chained_rounds_soak
    ))
    .await?
    else {
        return Ok(());
    };

    let first =
        sora_runtime_governance::enact_runtime_upgrade_round(&mut fixture, "chain-round-1", None)
            .await?;
    let second =
        sora_runtime_governance::enact_runtime_upgrade_round(&mut fixture, "chain-round-2", None)
            .await?;

    assert!(
        second.activated_height > first.activated_height,
        "second runtime upgrade should activate at a strictly later height than the first"
    );

    Ok(())
}

#[tokio::test]
async fn sora_runtime_upgrade_resilience_rejects_overlapping_runtime_window_proposal() -> Result<()>
{
    let Some(mut fixture) = sora_runtime_governance::setup_runtime_governance_fixture(stringify!(
        sora_runtime_upgrade_resilience_rejects_overlapping_runtime_window_proposal
    ))
    .await?
    else {
        return Ok(());
    };

    let baseline = sora_runtime_governance::enact_runtime_upgrade_round(
        &mut fixture,
        "overlap-baseline",
        None,
    )
    .await
    .wrap_err("enact baseline runtime upgrade before overlap rejection check")?;

    let abi_hash_hex = sora_runtime_governance::canonical_abi_hex();
    let overlapping_manifest = RuntimeUpgradeManifest {
        name: "parliament.runtime.upgrade.overlap.second".to_string(),
        description: format!(
            "overlaps baseline window {}..{} and should be rejected",
            baseline.scheduled_start_height, baseline.scheduled_end_height
        ),
        abi_version: 1,
        abi_hash: sora_runtime_governance::parse_hex32(&abi_hash_hex),
        added_syscalls: Vec::new(),
        added_pointer_types: Vec::new(),
        start_height: baseline.scheduled_start_height.saturating_add(1),
        end_height: baseline.scheduled_end_height.saturating_add(10),
        sbom_digests: Vec::new(),
        slsa_attestation: Vec::new(),
        provenance: Vec::new(),
    };
    let overlapping_proposal_id_hex = hex::encode(
        sora_runtime_governance::compute_runtime_upgrade_proposal_id(&overlapping_manifest),
    );

    let overlapping_tx_hash = fixture
        .alice
        .submit(ProposeRuntimeUpgradeProposal {
            manifest: overlapping_manifest,
            window: None,
            mode: Some(VotingMode::Plain),
        })
        .wrap_err("submit overlapping runtime-upgrade proposal")?;
    sora_runtime_governance::wait_for_tx_rejected(
        &fixture.http,
        &fixture.alice.torii_url,
        &hex::encode(overlapping_tx_hash.as_ref()),
        Duration::from_secs(180),
        "wait for overlapping runtime-upgrade proposal tx to be rejected",
    )
    .await
    .wrap_err("overlapping runtime-upgrade proposal should be rejected")?;

    let overlap_payload = fixture
        .alice
        .get_gov_proposal_json(&overlapping_proposal_id_hex)?;
    assert_ne!(
        overlap_payload
            .get("found")
            .and_then(norito::json::Value::as_bool),
        Some(true),
        "rejected overlapping runtime proposal should not be stored as found"
    );

    Ok(())
}
