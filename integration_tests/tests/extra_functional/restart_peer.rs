//! Tests that a restarted peer restores its state.
use eyre::{Result, WrapErr, ensure, eyre};
use integration_tests::{sandbox, sync::run_on_dedicated_thread};
use iroha::data_model::{
    prelude::*,
    query::error::{FindError, QueryExecutionFail},
};
use iroha::{blocking::Client, crypto::KeyPair, query::QueryError};
use iroha_config_base::toml::WriteExt as _;
use iroha_model_base::domain::DomainId;
use iroha_test_network::*;
use iroha_test_samples::ALICE_ID;
use std::{
    borrow::Cow,
    path::Path,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::time::{sleep, timeout};
use toml::Table;
async fn find_asset(client: &Client, id: &AssetId) -> Result<Option<Asset>> {
    match client
        .account_client()
        .query_single(FindAssetById::new(id.clone()))
        .await
    {
        Ok(asset) => {
            assert_eq!(asset.id(), id);
            Ok(Some(asset))
        }
        Err(QueryError::Validation(ValidationFail::QueryFailed(QueryExecutionFail::Find(
            FindError::Asset(missing),
        )))) if missing.as_ref() == id => Ok(None),
        Err(error) => Err(error.into()),
    }
}
#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn restarted_peer_should_restore_its_state() -> Result<()> {
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal")?,
        "xor".parse()?,
    );
    let quantity = Quantity::from(200_u32);
    let Some(network) = sandbox::start_network_async_or_skip(
        NetworkBuilder::new()
            .with_peers(4)
            .with_config_layer(|layer| {
                layer
                    .write(["snapshot", "mode"], "read_write")
                    // Keep the configured snapshot root explicit for the disk-readiness probe.
                    .write(["snapshot", "store_dir"], "./storage/snapshot")
                    .write(["snapshot", "create_every_ms"], 200_i64);
            }),
        stringify!(restarted_peer_should_restore_its_state),
    )
    .await?
    else {
        return Ok(());
    };
    let peers = network.peers();
    // create state on the first peer
    let peer_a = &peers[0];
    let peer_b = &peers[1];
    let client = peer_a.client();
    let client_for_submit = client.clone();
    let asset_definition_clone = asset_definition_id.clone();
    let mint_quantity = quantity.clone();
    let submit_res: eyre::Result<()> = run_on_dedicated_thread(move || {
        client_for_submit
            .submit_all::<InstructionBox>(
                [
                    Register::asset_definition({
                        let __asset_definition_id = asset_definition_clone.clone();
                        AssetDefinition::numeric(
                            __asset_definition_id.clone(),
                            "xor".to_owned(),
                            iroha_data_model::asset::AssetBalancePolicy::Global,
                            Some(DomainId::try_new("wonderland", "universal")?),
                        )
                    })
                    .into(),
                    Mint::asset_quantity(
                        mint_quantity,
                        AssetId::new(asset_definition_clone, ALICE_ID.clone()),
                    )
                    .into(),
                ],
                iroha::data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .map(|_| ())
    })
    .await?;
    if sandbox::handle_result(
        submit_res,
        stringify!(restarted_peer_should_restore_its_state),
    )?
    .is_none()
    {
        return Ok(());
    }
    if sandbox::handle_result(
        network.ensure_blocks(2).await,
        stringify!(restarted_peer_should_restore_its_state),
    )?
    .is_none()
    {
        return Ok(());
    }
    // Ensure the mint made it into the chain before shutting down peers.
    let mint_deadline = Instant::now() + network.sync_timeout();
    let asset_id = AssetId::new(asset_definition_id.clone(), ALICE_ID.clone());
    let minted = loop {
        let asset = sandbox::handle_result(
            find_asset(&client, &asset_id).await,
            stringify!(restarted_peer_should_restore_its_state),
        )?;
        let Some(asset) = asset else { return Ok(()) };
        if asset.is_some_and(|asset| asset.value() == &quantity) {
            break true;
        }
        if Instant::now() >= mint_deadline {
            break false;
        }
        sleep(Duration::from_millis(200)).await;
    };
    assert!(minted, "minted asset not observed before restart");
    // Ensure a post-mint snapshot persists before shutdown so restart can rebuild from disk.
    let snapshot_dir = peer_b.kura_store_dir().join("snapshot");
    let snapshot_deadline = Instant::now() + network.sync_timeout();
    let expected_snapshot_height = 2_u64;
    let snapshot_ready = loop {
        // Snapshots publish one immutable generation through `current`; flat
        // files in the snapshot root are not a published snapshot.
        if published_snapshot_height(&snapshot_dir)?
            .is_some_and(|height| height >= expected_snapshot_height)
        {
            break true;
        }
        if Instant::now() >= snapshot_deadline {
            break false;
        }
        sleep(Duration::from_millis(200)).await;
    };
    if !snapshot_ready {
        return Err(eyre!("snapshot not created before shutdown"));
    }
    // shutdown all
    network.shutdown().await;
    // restart another one, **without a genesis** even
    let config: Vec<_> = network.config_layers().collect();
    assert_ne!(peer_a, peer_b);
    let start_result = timeout(network.peer_startup_timeout(), async move {
        peer_b.start_checked(config.iter(), None).await?;
        peer_b.once_block(2).await;
        Ok::<(), eyre::Report>(())
    })
    .await;
    match start_result {
        Ok(Ok(())) => {}
        Ok(Err(err)) => {
            if let Some(reason) = sandbox::sandbox_reason(&err) {
                return Err(err.wrap_err(format!(
                    "sandboxed network restriction detected while restarting peer_b: {reason}"
                )));
            }
            return Err(err);
        }
        Err(err) => {
            let err = eyre::Report::new(err);
            if let Some(reason) = sandbox::sandbox_reason(&err) {
                return Err(err.wrap_err(format!(
                    "sandboxed network restriction detected while restarting peer_b: {reason}"
                )));
            }
            return Err(err);
        }
    }
    // ensure it has the state
    let client = peer_b.client();
    let deadline = Instant::now() + network.sync_timeout();
    let restored = loop {
        let asset = match sandbox::handle_result(
            find_asset(&client, &asset_id).await,
            stringify!(restarted_peer_should_restore_its_state),
        )? {
            Some(asset) => asset,
            None => return Ok(()),
        };
        if let Some(asset) = asset {
            break Some(asset.value().clone());
        }
        if Instant::now() >= deadline {
            break None;
        }
        sleep(Duration::from_millis(200)).await;
    };
    let Some(restored_value) = restored else {
        return Err(eyre!("restarted peer did not restore asset before timeout"));
    };
    assert_eq!(quantity, restored_value);
    Ok(())
}
#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn restarted_four_peers_rebuild_route_sensitive_state_from_kura_blocks() -> Result<()> {
    let test_name = stringify!(restarted_four_peers_rebuild_route_sensitive_state_from_kura_blocks);
    let manage_alias_permission =
        iroha_executor_data_model::permission::account::CanManageAccountAlias {
            scope:
                iroha_executor_data_model::permission::account::AccountAliasPermissionScope::Dataspace(
                    iroha_model_base::topology::DataSpaceId::UNIVERSAL,
                ),
        };
    let resolve_alias_permission =
        iroha_executor_data_model::permission::account::CanResolveAccountAlias {
            scope:
                iroha_executor_data_model::permission::account::AccountAliasPermissionScope::Alias(
                    iroha_data_model::alias_setup::ResolvedAccountAliasV1::new(
                        "merchant@universal".parse()?,
                        iroha_model_base::topology::DataSpaceId::UNIVERSAL,
                    ),
                ),
        };
    let Some(network) = sandbox::start_network_async_or_skip(
        NetworkBuilder::new()
            .with_peers(4)
            .with_genesis_instruction(Grant::account_permission(
                Permission::from(manage_alias_permission),
                ALICE_ID.clone(),
            ))
            .with_genesis_instruction(Grant::account_permission(
                Permission::from(resolve_alias_permission),
                ALICE_ID.clone(),
            )),
        test_name,
    )
    .await?
    else {
        return Ok(());
    };
    let domain_id = DomainId::try_new("paynet", "universal")?;
    let asset_definition_id =
        AssetDefinitionId::derive_from_components(domain_id.clone(), "routecoin".parse()?);
    let account_keypair = KeyPair::random();
    let account_id = AccountId::new(account_keypair.public_key().clone());
    let alias = iroha::data_model::account::rekey::AccountAlias::domainless(
        "merchant".parse()?,
        iroha_model_base::topology::DataSpaceId::UNIVERSAL,
    );
    let asset_id = AssetId::new(asset_definition_id.clone(), account_id.clone());
    let quantity = Quantity::from(321_u32);
    let client = network.client();
    let setup_domain = domain_setup_instruction(&domain_id, client.client().account())?;
    let setup_alias = account_alias_setup_instruction(
        "merchant@universal",
        &account_id,
        AccountProvisionV1::Create,
        AccountAliasRoleV1::Primary,
    )?;
    let submit_client = client.clone();
    let submit_definition = asset_definition_id.clone();
    let submit_asset = asset_id.clone();
    let submit_quantity = quantity.clone();
    let submit_domain = domain_id.clone();
    let submit_res: eyre::Result<()> = run_on_dedicated_thread(move || {
        submit_client
            .submit_all::<InstructionBox>(
                [
                    setup_domain,
                    setup_alias,
                    Register::asset_definition({
                        let definition_id = submit_definition.clone();
                        AssetDefinition::numeric(
                            definition_id.clone(),
                            "routecoin".to_owned(),
                            iroha_data_model::asset::AssetBalancePolicy::Global,
                            Some(submit_domain),
                        )
                    })
                    .into(),
                    Mint::asset_quantity(submit_quantity, submit_asset).into(),
                ],
                iroha::data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .map(|_| ())
    })
    .await?;
    if sandbox::handle_result(submit_res, test_name)?.is_none() {
        return Ok(());
    }
    if sandbox::handle_result(network.ensure_blocks(2).await, test_name)?.is_none() {
        return Ok(());
    }
    let expected_digest = wait_for_route_sensitive_state_digest(
        client.clone(),
        account_id.clone(),
        alias.clone(),
        domain_id.clone(),
        asset_definition_id.clone(),
        asset_id.clone(),
        quantity.clone(),
        network.sync_timeout(),
    )
    .await?;
    network.shutdown().await;
    for peer in network.peers() {
        remove_optional_recovery_sidecars(&peer.kura_store_dir())?;
    }
    let config_layers: Vec<_> = network.config_layers().collect();
    for peer in network.peers() {
        timeout(network.peer_startup_timeout(), async {
            peer.start_checked(config_layers.iter(), None).await?;
            peer.once_block(2).await;
            Ok::<(), eyre::Report>(())
        })
        .await
        .map_err(eyre::Report::new)??;
    }
    for peer in network.peers() {
        let digest = wait_for_route_sensitive_state_digest(
            peer.client(),
            account_id.clone(),
            alias.clone(),
            domain_id.clone(),
            asset_definition_id.clone(),
            asset_id.clone(),
            quantity.clone(),
            network.sync_timeout(),
        )
        .await?;
        assert_eq!(
            digest,
            expected_digest,
            "restarted peer {} rebuilt a different route-sensitive WSV surface",
            peer.id()
        );
    }
    Ok(())
}
#[expect(
    clippy::too_many_arguments,
    reason = "The poll receives every route-sensitive identity it digests explicitly."
)]
async fn wait_for_route_sensitive_state_digest(
    client: Client,
    account_id: AccountId,
    alias: iroha::data_model::account::rekey::AccountAlias,
    domain_id: DomainId,
    asset_definition_id: AssetDefinitionId,
    asset_id: AssetId,
    quantity: Quantity,
    timeout_after: Duration,
) -> Result<blake3::Hash> {
    let deadline = Instant::now() + timeout_after;
    let mut last_error = eyre!("route-sensitive state was not observed before timeout");
    loop {
        match route_sensitive_state_digest(
            client.clone(),
            account_id.clone(),
            alias.clone(),
            domain_id.clone(),
            asset_definition_id.clone(),
            asset_id.clone(),
            quantity.clone(),
        )
        .await
        {
            Ok(digest) => return Ok(digest),
            Err(err) => last_error = err,
        }
        if Instant::now() >= deadline {
            break;
        }
        sleep(Duration::from_millis(200)).await;
    }
    Err(last_error)
}
async fn route_sensitive_state_digest(
    client: Client,
    account_id: AccountId,
    alias: iroha::data_model::account::rekey::AccountAlias,
    domain_id: DomainId,
    asset_definition_id: AssetDefinitionId,
    asset_id: AssetId,
    quantity: Quantity,
) -> Result<blake3::Hash> {
    run_on_dedicated_thread(move || {
        let account = client
            .client()
            .query_single(FindAccountById::new(account_id.clone()))
            .wrap_err("read route-sensitive account")?;
        let alias_account = client
            .client()
            .query_single(FindAccountByAlias::new(alias.clone()))
            .wrap_err("read route-sensitive account alias")?;
        let domain = client
            .client()
            .query_single(FindDomainById::new(domain_id.clone()))
            .wrap_err("read route-sensitive domain")?;
        let definition = client
            .client()
            .query_single(FindAssetDefinitionById::new(asset_definition_id.clone()))
            .wrap_err("read route-sensitive asset definition")?;
        let asset = client
            .client()
            .query_single(FindAssetById::new(asset_id.clone()))
            .wrap_err("read route-sensitive asset")?;
        if asset.value() != &quantity {
            return Err(eyre!(
                "asset `{}` has value `{}`, expected `{}`",
                asset.id(),
                asset.value(),
                quantity
            ));
        }
        let mut aliases = client
            .client()
            .query_single(FindAliasesByAccountId::new(account_id.clone(), None, None))
            .wrap_err("read route-sensitive aliases by account")?;
        aliases.sort_by(|left, right| format!("{left:?}").cmp(&format!("{right:?}")));
        let mut surface = Vec::new();
        surface.push(format!("account={}", account.id()));
        surface.push(format!("alias_account={}", alias_account.id()));
        surface.push(format!("domain={}", domain.id()));
        surface.push(format!("asset_definition={}", definition.id()));
        surface.push(format!("asset={}:{}", asset.id(), asset.value()));
        for alias_record in aliases {
            surface.push(format!("alias_record={alias_record:?}"));
        }
        surface.sort();
        Ok(blake3::hash(surface.join("\n").as_bytes()))
    })
    .await
    .map_err(eyre::Report::from)?
}
/// Inspect publication readiness only; the native daemon authenticates the signed
/// generation and reconstructs the state during the actual restart below.
fn published_snapshot_height(root: &Path) -> Result<Option<u64>> {
    let pointer = match std::fs::read_to_string(root.join("current")) {
        Ok(pointer) => pointer,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let digest = pointer
        .strip_suffix('\n')
        .ok_or_else(|| eyre!("snapshot generation pointer lacks its canonical newline"))?;
    ensure!(
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "snapshot generation pointer is not canonical lowercase SHA-256 hex"
    );
    let generation = root.join("generations").join(digest);
    for artifact in [
        "snapshot.data",
        "snapshot.sha256",
        "snapshot.sig",
        "snapshot.fast.norito",
        "snapshot.merkle.json",
    ] {
        let metadata = std::fs::symlink_metadata(generation.join(artifact))?;
        ensure!(
            metadata.is_file() && metadata.len() > 0,
            "published snapshot lacks the complete signed artifact set"
        );
    }
    let snapshot: norito::json::Value =
        norito::json::from_slice(&std::fs::read(generation.join("snapshot.data"))?)?;
    let hashes = snapshot
        .get("block_hashes")
        .and_then(norito::json::Value::as_array)
        .ok_or_else(|| eyre!("published snapshot has no committed block-hash vector"))?;
    Ok(Some(u64::try_from(hashes.len())?))
}

#[test]
fn published_snapshot_readiness_requires_a_complete_canonical_generation() -> Result<()> {
    let root = tempfile::tempdir()?;
    assert_eq!(published_snapshot_height(root.path())?, None);
    let digest = "ab".repeat(32);
    for malformed in [
        digest.clone(),
        format!("{digest}\n\n"),
        format!("{}\n", digest.to_uppercase()),
        "../snapshot\n".to_owned(),
    ] {
        std::fs::write(root.path().join("current"), malformed)?;
        assert!(published_snapshot_height(root.path()).is_err());
    }
    std::fs::write(root.path().join("current"), format!("{digest}\n"))?;
    let generation = root.path().join("generations").join(&digest);
    std::fs::create_dir_all(&generation)?;
    for artifact in [
        "snapshot.data",
        "snapshot.sha256",
        "snapshot.sig",
        "snapshot.merkle.json",
    ] {
        std::fs::write(generation.join(artifact), b"fixture")?;
    }
    std::fs::write(
        generation.join("snapshot.data"),
        br#"{"block_hashes":["one","two"]}"#,
    )?;
    // The retired four-file layout cannot establish readiness.
    assert!(published_snapshot_height(root.path()).is_err());
    std::fs::write(generation.join("snapshot.fast.norito"), b"fixture")?;
    assert_eq!(published_snapshot_height(root.path())?, Some(2));
    std::fs::write(
        generation.join("snapshot.data"),
        br#"{"block_hashes":["one"]}"#,
    )?;
    assert_eq!(published_snapshot_height(root.path())?, Some(1));
    std::fs::write(generation.join("snapshot.data"), b"{}")?;
    assert!(published_snapshot_height(root.path()).is_err());
    std::fs::write(generation.join("snapshot.sig"), b"")?;
    assert!(published_snapshot_height(root.path()).is_err());
    Ok(())
}

fn remove_optional_recovery_sidecars(root: &Path) -> Result<()> {
    const SIDE_CAR_DIRS: [&str; 3] = ["snapshot", "wsv_checkpoints", "commit_manifests"];
    if !root.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name();
        if SIDE_CAR_DIRS
            .iter()
            .any(|sidecar| name == std::ffi::OsStr::new(sidecar))
        {
            std::fs::remove_dir_all(&path)?;
        } else {
            remove_optional_recovery_sidecars(&path)?;
        }
    }
    Ok(())
}
#[test]
fn remove_optional_recovery_sidecars_preserves_non_sidecar_payloads() -> Result<()> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let root = std::env::temp_dir().join(format!("iroha_sidecar_prune_negative_{nanos}"));
    let result = (|| -> Result<()> {
        std::fs::create_dir_all(root.join("blocks/1"))?;
        std::fs::create_dir_all(root.join("nested/retained"))?;
        std::fs::create_dir_all(root.join("snapshot"))?;
        std::fs::create_dir_all(root.join("wsv_checkpoints"))?;
        std::fs::create_dir_all(root.join("nested/commit_manifests"))?;
        std::fs::write(root.join("blocks/1/block.wire"), b"canonical block")?;
        std::fs::write(root.join("nested/retained/block.wire"), b"retained block")?;
        std::fs::write(root.join("snapshot/stale"), b"optional")?;
        std::fs::write(root.join("wsv_checkpoints/stale"), b"optional")?;
        std::fs::write(root.join("nested/commit_manifests/stale"), b"optional")?;
        remove_optional_recovery_sidecars(&root)?;
        assert!(root.join("blocks/1/block.wire").exists());
        assert!(root.join("nested/retained/block.wire").exists());
        assert!(!root.join("snapshot").exists());
        assert!(!root.join("wsv_checkpoints").exists());
        assert!(!root.join("nested/commit_manifests").exists());
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&root);
    result
}
#[test]
fn remove_optional_recovery_sidecars_preserves_similarly_named_payload_dirs() -> Result<()> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let root = std::env::temp_dir().join(format!("iroha_sidecar_prune_exact_name_{nanos}"));
    let result = (|| -> Result<()> {
        std::fs::create_dir_all(root.join("snapshot_backup"))?;
        std::fs::create_dir_all(root.join("wsv_checkpoints.tmp"))?;
        std::fs::create_dir_all(root.join("commit_manifests_old"))?;
        std::fs::create_dir_all(root.join("nested/snapshot"))?;
        std::fs::write(root.join("snapshot_backup/block.wire"), b"payload")?;
        std::fs::write(root.join("wsv_checkpoints.tmp/block.wire"), b"payload")?;
        std::fs::write(root.join("commit_manifests_old/block.wire"), b"payload")?;
        std::fs::write(root.join("nested/snapshot/stale"), b"optional")?;
        remove_optional_recovery_sidecars(&root)?;
        assert!(root.join("snapshot_backup/block.wire").exists());
        assert!(root.join("wsv_checkpoints.tmp/block.wire").exists());
        assert!(root.join("commit_manifests_old/block.wire").exists());
        assert!(!root.join("nested/snapshot").exists());
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&root);
    result
}
#[test]
fn remove_optional_recovery_sidecars_ignores_missing_root() -> Result<()> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let root = std::env::temp_dir().join(format!("iroha_sidecar_prune_missing_{nanos}"));
    remove_optional_recovery_sidecars(&root)
}
#[tokio::test]
async fn restarted_peer_with_mismatched_genesis_pubkey_is_rejected() -> Result<()> {
    let test_name = stringify!(restarted_peer_with_mismatched_genesis_pubkey_is_rejected);
    let Some(network) =
        sandbox::start_network_async_or_skip(NetworkBuilder::new().with_peers(4), test_name)
            .await?
    else {
        return Ok(());
    };
    let peer = &network.peers()[0];
    if sandbox::handle_result(network.ensure_blocks(1).await, test_name)?.is_none() {
        return Ok(());
    }
    let config_layers: Vec<_> = network.config_layers().collect();
    let wrong_genesis_pubkey = KeyPair::random().public_key().to_string();
    let override_layer = Table::new().write(["genesis", "public_key"], wrong_genesis_pubkey);
    let genesis = network.genesis();
    network.shutdown().await;
    let start_result = timeout(network.peer_startup_timeout(), async {
        peer.start_checked(
            config_layers
                .iter()
                .cloned()
                .chain(std::iter::once(Cow::Owned(override_layer))),
            Some(&genesis),
        )
        .await
    })
    .await;
    let rejection = match start_result {
        Ok(Ok(())) => {
            network.shutdown().await;
            return Err(eyre!(
                "peer accepted a stored genesis that does not match configured genesis.public_key"
            ));
        }
        Ok(Err(err)) => err,
        Err(err) => {
            let err = eyre::Report::new(err);
            if let Some(reason) = sandbox::sandbox_reason(&err) {
                return Err(err.wrap_err(format!(
                    "sandboxed network restriction detected while restarting peer with mismatched genesis pubkey: {reason}"
                )));
            }
            return Err(err.wrap_err(
                "timed out waiting for restart to reject mismatched genesis.public_key",
            ));
        }
    };
    if let Some(reason) = sandbox::sandbox_reason(&rejection) {
        return Err(rejection.wrap_err(format!(
            "sandboxed network restriction detected while restarting peer with mismatched genesis pubkey: {reason}"
        )));
    }
    assert!(
        format!("{rejection:?}").contains("does not match configured genesis.public_key"),
        "restart failed for an unexpected reason: {rejection:?}"
    );
    network.shutdown().await;
    Ok(())
}
