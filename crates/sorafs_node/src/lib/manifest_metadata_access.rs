/// Retrieve stored manifest metadata by digest.
pub fn manifest_metadata_by_digest(
    &self,
    digest: &[u8; 32],
) -> Result<StoredManifest, NodeStorageError> {
    let storage = self.storage_backend()?;
    storage.manifest_by_digest(digest).ok_or_else(|| {
        NodeStorageError::from(StorageError::ManifestNotFound {
            manifest_id: hex::encode(digest),
        })
    })
}

/// Retrieve the deterministic preferred local manifest for a content CID.
///
/// The backend scans borrowed metadata and clones only the selected result, so a CID lookup
/// cannot allocate a snapshot proportional to every stored manifest.
pub fn manifest_metadata_by_cid(
    &self,
    cid: &[u8],
) -> Result<Option<StoredManifest>, NodeStorageError> {
    let storage = self.storage_backend()?;
    Ok(storage.manifest_by_cid_prefer_site(cid))
}

/// Return stored manifest metadata ordered deterministically by manifest digest then identifier.
pub fn stored_manifests(&self) -> Result<Vec<StoredManifest>, NodeStorageError> {
    let storage = self.storage_backend()?;
    let mut manifests = storage.manifests();
    manifests.sort_by(|left, right| {
        left.manifest_digest()
            .cmp(right.manifest_digest())
            .then_with(|| left.manifest_id().cmp(right.manifest_id()))
    });
    Ok(manifests)
}
