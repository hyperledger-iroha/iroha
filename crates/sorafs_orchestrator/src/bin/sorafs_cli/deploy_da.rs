//! Project one finalized producer-owned DA ticket into the native paid publisher.

use super::{
    AccountId, DeployClientConfig, PinPolicy, RegistryStorageClass, StorageClass, StorageTicketId,
};
use iroha_data_model::{
    da::{commitment::DaCommitmentRecord, manifest::DaManifestV1, types::RetentionPolicy},
    sorafs::pin_registry::SORAFS_AUTO_REPLICATION_ORDER_INGEST_DEADLINE_SECS_V1,
    sumeragi_finality::VerifiedSumeragiBlock,
};
use iroha_fs::{OwnerDirectory, PrivateDirectory, PublishMode};
use std::path::Path;

pub(super) struct DaPackage {
    // Keep the staged package alive until the publisher finishes reading it.
    pub(super) directory: PrivateDirectory,
    _temporary_root: tempfile::TempDir,
    pub(super) ticket: String,
    pub(super) manifest_hash: String,
    pub(super) policy: PinPolicy,
}

fn publication_directory() -> std::io::Result<(tempfile::TempDir, PrivateDirectory)> {
    let temporary_root = tempfile::Builder::new()
        .prefix("iroha-da-publication-")
        .tempdir()?;
    let root = OwnerDirectory::open(temporary_root.path().canonicalize()?)?;
    let directory = root.create_private_child("package")?;
    Ok((temporary_root, directory))
}

fn pin_policy(
    policy: &RetentionPolicy,
    issued_at: u64,
    floor_epoch: u64,
) -> Result<PinPolicy, String> {
    let lifetime = policy.hot_retention_secs.max(policy.cold_retention_secs);
    if policy.required_replicas == 0 || lifetime == 0 {
        return Err("DA retention requires a positive replica count and lifetime".into());
    }
    let retention_epoch = issued_at
        .checked_add(lifetime)
        .ok_or("DA retention deadline overflow")?;
    let earliest_ingest_deadline = floor_epoch
        .checked_add(u64::from(
            SORAFS_AUTO_REPLICATION_ORDER_INGEST_DEADLINE_SECS_V1,
        ))
        .ok_or("native replication deadline overflow")?;
    if earliest_ingest_deadline >= retention_epoch {
        return Err("negotiated DA retention cannot accommodate the native replication ingest window after its finalized checkpoint".into());
    }
    Ok(PinPolicy {
        min_replicas: policy.required_replicas,
        storage_class: match policy.storage_class {
            RegistryStorageClass::Hot => StorageClass::Hot,
            RegistryStorageClass::Warm => StorageClass::Warm,
            RegistryStorageClass::Cold => StorageClass::Cold,
        },
        retention_epoch,
    })
}

pub(super) fn prepare(
    ticket_path: &Path,
    tip: &VerifiedSumeragiBlock,
    config: &DeployClientConfig,
) -> Result<DaPackage, String> {
    let ticket = ticket_path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("expected DA ticket directory")?;
    let id = sorafs_car::fetch_plan::parse_digest_hex(ticket).map_err(|e| e.to_string())?;
    if hex::encode(id) != ticket {
        return Err("DA source directory must name the canonical storage ticket".into());
    }
    let ticket_id = StorageTicketId::new(id);
    let block = tip.block();
    let intent = block
        .da_pin_intents()
        .and_then(|bundle| {
            bundle
                .intents
                .iter()
                .find(|intent| intent.storage_ticket == ticket_id)
        })
        .ok_or("selected finalized checkpoint does not include the DA pin intent")?;
    let record = block
        .da_commitments()
        .and_then(|bundle| {
            bundle
                .commitments()
                .iter()
                .find(|record| record.storage_ticket == ticket_id)
        })
        .ok_or("selected finalized checkpoint does not include the DA commitment")?;
    let scope = &intent.pin_scope_authorization.scope;
    if intent.authorization.owner != AccountId::new(config.public_key.clone())
        || intent.authorization.network_id != config.network_id
        || !scope.matches_authorization(&intent.authorization)
        || scope.storage_ticket != ticket_id
        || scope.manifest_hash != record.manifest_hash
        // The producer's witnesses sign the digest of this exact finalized commitment.
        || !intent
            .pin_scope_authorization
            .has_valid_canonical_signatures(&record.pin_scope_digest())
        || !intent.authorization.has_valid_canonical_signatures()
    {
        return Err("DA publication does not match the finalized producer authorization".into());
    }
    export(
        ticket_path,
        ticket,
        record,
        &intent.authorization.payload_hash,
        intent.authorization.payload_bytes,
        block.header().creation_time().as_secs(),
    )
}

fn export(
    ticket_path: &Path,
    ticket: &str,
    record: &DaCommitmentRecord,
    payload_hash: &iroha_data_model::da::types::BlobDigest,
    payload_bytes: u64,
    floor_epoch: u64,
) -> Result<DaPackage, String> {
    let error = |e: std::io::Error| e.to_string();
    let root = OwnerDirectory::open(ticket_path).map_err(error)?;
    let source = root
        .open_private_child("source")
        .map_err(|e| format!("open retained DA source: {e}"))?;
    let bytes = root
        .read_regular("manifest.norito", DaManifestV1::MAX_ENCODED_BYTES)
        .map_err(error)?;
    if blake3::hash(&bytes).as_bytes() != record.manifest_hash.as_bytes()
        || source
            .read("manifest.blake3", 32)
            .map_err(error)?
            .as_slice()
            != record.manifest_hash.as_bytes()
    {
        return Err("DA source manifest differs from its certified digest".into());
    }
    let manifest = DaManifestV1::decode_canonical(&bytes).map_err(|e| e.to_string())?;
    if manifest.version != DaManifestV1::VERSION
        || manifest.storage_ticket != record.storage_ticket
        || manifest.retention_policy != record.retention_class
        || manifest.blob_hash != *payload_hash
        || manifest.total_size != payload_bytes
        || manifest.lane_id != record.lane_id
        || manifest.epoch != record.epoch
        || manifest.chunk_root != record.chunk_root
    {
        return Err("DA manifest fields disagree with finalized inclusion".into());
    }
    let policy = pin_policy(
        &manifest.retention_policy,
        manifest.issued_at_unix,
        floor_epoch,
    )?;
    // Validate the complete inventory and the original data before constructing provider input.
    sorafs_car::da_recovery::recover_payload(&manifest, |chunk| {
        let name = format!("chunk_{:05}.bin", chunk.index);
        source
            .read_optional(name, chunk.length as usize)
            .map(|bytes| bytes.map(|v| v.to_vec()))
    })
    .map_err(|e| e.to_string())?;
    let (temporary_root, target) = publication_directory()
        .map_err(|e| format!("create private DA publication directory: {e}"))?;
    target
        .write_atomic("manifest.norito", &bytes, PublishMode::CreateNew)
        .map_err(error)?;
    for chunk in &manifest.chunks {
        let name = format!("chunk_{:05}.bin", chunk.index);
        let bytes = source.read(&name, chunk.length as usize).map_err(error)?;
        if bytes.len() != chunk.length as usize
            || blake3::hash(&bytes).as_bytes() != chunk.commitment.as_bytes()
        {
            return Err(format!(
                "retained DA shard {} differs from the manifest",
                chunk.index
            ));
        }
        target
            .write_atomic(&name, &bytes, PublishMode::CreateNew)
            .map_err(error)?;
    }
    Ok(DaPackage {
        directory: target,
        _temporary_root: temporary_root,
        ticket: ticket.to_owned(),
        manifest_hash: hex::encode(record.manifest_hash.as_bytes()),
        policy,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publication_staging_uses_a_native_private_directory_until_drop() {
        let (root, package) = publication_directory().unwrap();
        let path = package.path().to_owned();
        package
            .write_atomic("manifest.norito", b"manifest", PublishMode::CreateNew)
            .unwrap();
        let reopened = PrivateDirectory::open(&path).unwrap();
        assert_eq!(
            reopened.read("manifest.norito", 8).unwrap().as_slice(),
            b"manifest"
        );
        drop(reopened);
        drop(package);
        drop(root);
        assert!(!path.exists());
    }

    #[test]
    fn native_pin_retains_negotiated_replica_count_tier_and_deadline() {
        let mut retention = RetentionPolicy {
            storage_class: RegistryStorageClass::Cold,
            required_replicas: 4,
            ..RetentionPolicy::default()
        };
        let policy = pin_policy(&retention, 77, 77).unwrap();
        assert_eq!(policy.min_replicas, 4);
        assert_eq!(policy.storage_class, StorageClass::Cold);
        assert_eq!(
            policy.retention_epoch,
            77 + retention
                .cold_retention_secs
                .max(retention.hot_retention_secs)
        );
        assert!(pin_policy(&retention, u64::MAX, 77).is_err());
        retention.required_replicas = 0;
        assert!(pin_policy(&retention, 77, 77).is_err());
    }

    #[test]
    fn negotiated_retention_must_outlast_the_native_ingest_window_after_inclusion() {
        let window = u64::from(SORAFS_AUTO_REPLICATION_ORDER_INGEST_DEADLINE_SECS_V1);
        let mut retention = RetentionPolicy {
            hot_retention_secs: window,
            cold_retention_secs: 0,
            ..Default::default()
        };
        assert!(pin_policy(&retention, 10, 10).is_err());
        retention.hot_retention_secs += 2;
        assert_eq!(
            pin_policy(&retention, 10, 11).unwrap().retention_epoch,
            12 + window
        );
        assert!(pin_policy(&retention, 10, 12).is_err());
        assert!(pin_policy(&retention, 10, u64::MAX).is_err());
    }
}
