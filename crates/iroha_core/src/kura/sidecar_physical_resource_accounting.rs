// Included at Kura module scope. Observation never grants sidecar publication authority.

impl Kura {
    /// Complete bounded footprint of an indexed sidecar's append, prepend, and rewrite owners.
    /// The caller holds the sidecar owner lock across observation and mutation.
    fn sidecar_physical_resource_paths(data_path: &Path, index_path: &Path) -> Vec<PathBuf> {
        vec![
            data_path.to_path_buf(),
            index_path.to_path_buf(),
            data_path.with_extension("norito.tmp"),
            index_path.with_extension("index.tmp"),
            index_path.with_extension("index.prepend.tmp"),
            Self::bound_progress_append_build_path(index_path),
            Self::bound_progress_append_intent_path(index_path),
        ]
    }

    /// Account read-triggered rewrite recovery under the caller's sidecar lock.
    /// A later enclosing write owns a separate delta after recovery has completed.
    fn recover_indexed_sidecar_with_physical_resources(
        &self,
        data_path: &Path,
        index_path: &Path,
        kind: &str,
    ) -> bool {
        // Preserve the read-only fast path: without a rewrite pair there is no
        // mutation, no stable index observation, and no accounting generation.
        let pending = [
            data_path.with_extension("norito.tmp"),
            index_path.with_extension("index.tmp"),
        ]
        .iter()
        .any(|path| !matches!(std::fs::symlink_metadata(path), Err(error) if error.kind() == ErrorKind::NotFound));
        if !pending {
            return true;
        }
        let mutation = self
            .begin_total_disk_usage_mutation()
            .with_resource_paths(Self::sidecar_physical_resource_paths(data_path, index_path));
        if !Self::recover_indexed_sidecar_artifacts(data_path, index_path, kind) {
            return false;
        }
        // Recovery can replace or remove stable and temporary data as well as
        // index slots. Publish all physical deltas; preserve old disk-cache invalidation.
        mutation.finish_resources_before_disk_rescan();
        true
    }
}
