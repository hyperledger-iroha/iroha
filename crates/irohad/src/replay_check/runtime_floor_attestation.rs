//! Sign one startup floor only after restoring its exact checkpoint and replaying the locked store.
//!
//! The ordinary `--check-replay` command remains non-signing. This explicit stopped-node action
//! keeps the original Kura lock and bound snapshot artifacts until Core creates the statement.

use super::*;
use iroha_core::snapshot::checkpoints::SnapshotCheckpoints;
use iroha_data_model::runtime::checkpoint::{SignedStartupFloorAttestationV1, StartupFloorV1};

/// The existing floor-statement JSON shape, for relay through the ordinary runtime CLI.
#[derive(Debug, JsonSerialize)]
struct FloorStatement {
    floor: StartupFloorV1,
    attestation: SignedStartupFloorAttestationV1,
}

/// Rehearse the exact requested floor on a stopped store, then print this validator's statement.
///
/// # Errors
/// Missing or live store, untrusted genesis, absent exact checkpoint, incomplete or divergent
/// replay, changed source custody, local resource refusal, or a signer outside the final committee.
pub(crate) fn run_runtime_floor_attestation(
    config: &Config,
    height: u64,
) -> ReportResult<(), MainError> {
    let statement = floor_statement(config, height)?;
    let encoded = norito::json::to_json(&statement)
        .map_err(|error| Report::new(error).change_context(MainError::CheckReplay))?;
    println!("{encoded}");
    Ok(())
}

fn floor_statement(config: &Config, height: u64) -> ReportResult<FloorStatement, MainError> {
    if height == 0 {
        return Err(failure("startup floor height must be positive"));
    }
    if !snapshot_mode_allows_restore(config.snapshot.mode) {
        return Err(failure(
            "startup floor attestation requires snapshot restoration",
        ));
    }
    let store_root = config.kura.store_dir.resolve_relative_path();
    // Emergency-Fast construction must never create a missing store for this action.
    std::fs::symlink_metadata(&store_root)
        .map_err(|error| Report::new(error).change_context(MainError::CheckReplay))?;
    let source = open_store_read_only(config)?;
    let tip = source
        .exact_durable_blocks_count()
        .map_err(|error| Report::new(error).change_context(MainError::CheckReplay))?;
    let tip_height = u64::try_from(tip)
        .map_err(|error| Report::new(error).change_context(MainError::CheckReplay))?;
    if height >= tip_height {
        return Err(failure(
            "startup floor attestation requires the checkpoint's certified successor",
        ));
    }
    let budget = AllocationBudget::new(config.pipeline.ivm_execution_max_bytes);
    let stored_genesis = source
        .get_block(NonZeroUsize::MIN, &budget)
        .map_err(|error| Report::new(error).change_context(MainError::CheckReplay))?
        .ok_or_else(|| failure("startup floor attestation requires original signed genesis"))?;
    let genesis: &SignedBlock = stored_genesis.as_ref();
    ResolvedGenesisTrustAnchor::resolve(
        &config.genesis.public_key,
        config.genesis.expected_hash,
        None,
    )
    .and_then(|anchor| anchor.verify(genesis))
    .change_context(MainError::CheckReplay)?;
    require_authenticated_genesis_crypto_implementations(genesis, &config.genesis.public_key)
        .change_context(MainError::CheckReplay)?;
    let (consensus_mode, context) = signed_genesis_context_metadata(genesis).map_err(failure)?;
    startup_root_topology::validate(&config.nexus, consensus_mode, context.root_scope)
        .map_err(failure)?;
    let network = NetworkId::from_genesis_hash(config.genesis.expected_hash);
    let manifests = freeze_lane_manifests_for_startup_replay(&config.nexus)
        .map_err(|error| Report::new(error).change_context(MainError::CheckReplay))?;
    let started = Instant::now();
    let mut reported = 0;
    eprintln!("attest-runtime-floor: copying the stopped store through height {tip_height}");
    let copy = replay_check::copy_store(
        &source,
        &config.kura,
        &config.nexus.lane_config,
        &config.nexus.configured_lane_catalog,
        &mut |bytes| {
            if bytes >= reported + COPY_PROGRESS_BYTES {
                reported = bytes;
                eprintln!(
                    "attest-runtime-floor: copied {} MiB ({} s)",
                    bytes >> 20,
                    started.elapsed().as_secs()
                );
            }
        },
    )
    .map_err(|error| Report::new(error).change_context(MainError::CheckReplay))?;
    if let Some(checkpoint) = config
        .snapshot
        .upgrade_checkpoint
        .checkpoint()
        .map_err(failure)?
    {
        copy.set_upgrade_checkpoint(&UpgradeCheckpointV1::from_config(&checkpoint))
            .map_err(failure)?;
    }
    // This empty object supplies configuration only. The opaque Core owner restores H into
    // a separate State; neither an empty fallback nor a different snapshot can satisfy it.
    let mut template = state_configuration_template(config, &copy, &budget, genesis, network)?;
    install_zk_config_before_kura_replay(&mut template, config)
        .change_context(MainError::CheckReplay)?;
    apply_state_runtime_config_before_snapshot_auth(&mut template, config)
        .map_err(|error| Report::new(error).change_context(MainError::CheckReplay))?;
    let verification_key = config
        .snapshot
        .verification_public_key
        .as_ref()
        .unwrap_or_else(|| config.common.key_pair.public_key());
    let checkpoints = SnapshotCheckpoints::from_config(
        &config.snapshot,
        verification_key.clone(),
        AllocationBudget::new(config.snapshot.max_read_buffer_bytes.get()),
    );
    let compliance = load_lane_compliance_for_startup_replay(&config.nexus)
        .change_context(MainError::CheckReplay)?;
    let restored = checkpoints
        .restore_exact_for_floor(
            height,
            source,
            &template,
            &config.nexus,
            &manifests,
            compliance,
            config.snapshot.resources,
        )
        .map_err(|error| Report::new(error).change_context(MainError::CheckReplay))?;
    drop(template);
    eprintln!("attest-runtime-floor: replaying exact checkpoint {height} to {tip_height}");
    let replayed = restored
        .replay(&mut |height, stored| {
            if height.is_multiple_of(PROGRESS_INTERVAL) || height == stored {
                eprintln!("attest-runtime-floor: replayed {height}/{stored}");
            }
        })
        .map_err(|error| Report::new(error).change_context(MainError::CheckReplay))?;
    let (floor, attestation) =
        iroha_core::runtime::checkpoint::attest_replayed_floor(replayed, &config.common.key_pair)
            .map_err(|error| Report::new(error).change_context(MainError::CheckReplay))?;
    Ok(FloorStatement { floor, attestation })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay_check::tests::{populate_store, store_config, tree};

    #[test]
    fn floor_attestation_refuses_zero_missing_and_empty_stores_without_writing() {
        let root = tempfile::tempdir().unwrap();
        let config = store_config(root.path());
        assert!(run_runtime_floor_attestation(&config, 0).is_err());
        assert!(run_runtime_floor_attestation(&config, 1).is_err());
        assert!(!config.kura.store_dir.resolve_relative_path().exists());
        populate_store(&config, 0);
        let before = tree(root.path());
        assert!(floor_statement(&config, 1).is_err());
        assert_eq!(tree(root.path()), before);
    }

    #[test]
    fn floor_attestation_refuses_live_and_untrusted_stores_without_writing() {
        let root = tempfile::tempdir().unwrap();
        let config = store_config(root.path());
        populate_store(&config, 3);
        let (owner, _) = Kura::new_with_configured_lane_catalog(
            &config.kura,
            &config.nexus.lane_config,
            &config.nexus.configured_lane_catalog,
        )
        .unwrap();
        let error = floor_statement(&config, 1).unwrap_err();
        assert!(format!("{error:?}").contains("cannot open Kura store"));
        drop(owner);
        let before = tree(root.path());
        let error = floor_statement(&config, 1).unwrap_err();
        assert!(format!("{error:?}").contains("genesis"));
        assert_eq!(tree(root.path()), before);
    }
}
