impl NodeHandle {
    /// Retrieve a shared handle to stored manifest metadata.
    ///
    /// Stored descriptors are immutable, so the handle is a stable snapshot that costs one
    /// pointer copy instead of copying the manifest's file and chunk records.
    pub fn manifest_metadata(
        &self,
        manifest_id: &str,
    ) -> Result<Arc<StoredManifest>, NodeStorageError> {
        let storage = self.storage_backend()?;
        storage.manifest(manifest_id).ok_or_else(|| {
            NodeStorageError::from(StorageError::ManifestNotFound {
                manifest_id: manifest_id.to_owned(),
            })
        })
    }
    /// Retrieve a shared handle to stored manifest metadata by digest.
    pub fn manifest_metadata_by_digest(
        &self,
        digest: &[u8; 32],
    ) -> Result<Arc<StoredManifest>, NodeStorageError> {
        let storage = self.storage_backend()?;
        storage.manifest_by_digest(digest).ok_or_else(|| {
            NodeStorageError::from(StorageError::ManifestNotFound {
                manifest_id: hex::encode(digest),
            })
        })
    }
    /// Retrieve the deterministic preferred local manifest for a content CID.
    ///
    /// The backend scans borrowed metadata and returns a handle to the selected result only, so a
    /// CID lookup cannot allocate a snapshot proportional to every stored manifest.
    pub fn manifest_metadata_by_cid(
        &self,
        cid: &[u8],
    ) -> Result<Option<Arc<StoredManifest>>, NodeStorageError> {
        let storage = self.storage_backend()?;
        Ok(storage.manifest_by_cid_prefer_site(cid))
    }
    /// Resolve a client manifest reference (identifier or hex CID) to its stored identifier.
    pub fn resolve_manifest_reference(
        &self,
        reference: &str,
    ) -> Result<Option<String>, NodeStorageError> {
        let storage = self.storage_backend()?;
        Ok(storage.resolve_manifest_reference(reference))
    }
    /// Return stored manifest metadata ordered deterministically by manifest digest then identifier.
    pub fn stored_manifests(&self) -> Result<Vec<Arc<StoredManifest>>, NodeStorageError> {
        let storage = self.storage_backend()?;
        let mut manifests = storage.manifests();
        manifests.sort_by(|left, right| {
            left.manifest_digest()
                .cmp(right.manifest_digest())
                .then_with(|| left.manifest_id().cmp(right.manifest_id()))
        });
        Ok(manifests)
    }
}
