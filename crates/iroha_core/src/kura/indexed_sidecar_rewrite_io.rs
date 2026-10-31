// Storage ownership for the shared deterministic sidecar rewrite algorithm.
trait IndexedSidecarRewriteIo {
    fn open_index(&mut self) -> std::io::Result<std::fs::File>;
    fn open_data(&mut self) -> std::io::Result<std::fs::File>;
    fn create_data(&mut self) -> std::io::Result<std::fs::File>;
    fn create_index(&mut self) -> std::io::Result<std::fs::File>;
    fn publish(&mut self) -> bool;
}

#[cfg(not(windows))]
struct PathSidecarRewrite<'a> {
    data_path: &'a Path,
    index_path: &'a Path,
    kind: &'a str,
}

#[cfg(not(windows))]
impl IndexedSidecarRewriteIo for PathSidecarRewrite<'_> {
    fn open_index(&mut self) -> std::io::Result<std::fs::File> {
        std::fs::File::open(self.index_path)
    }
    fn open_data(&mut self) -> std::io::Result<std::fs::File> {
        std::fs::File::open(self.data_path)
    }
    fn create_data(&mut self) -> std::io::Result<std::fs::File> {
        std::fs::File::create(self.data_path.with_extension("norito.tmp"))
    }
    fn create_index(&mut self) -> std::io::Result<std::fs::File> {
        std::fs::File::create(self.index_path.with_extension("index.tmp"))
    }
    fn publish(&mut self) -> bool {
        let data_path = self.data_path;
        let index_path = self.index_path;
        let kind = self.kind;
        let temp_data_path = data_path.with_extension("norito.tmp");
        let temp_index_path = index_path.with_extension("index.tmp");
        // Make both temp directory entries durable before the first promotion. The temp index is
        // the recovery marker if the process crashes after publishing the new data file but before
        // publishing its matching index.
        let Some(data_parent) = temp_data_path.parent() else {
            iroha_logger::warn!(?temp_data_path, kind, "sidecar temp data has no parent");
            return false;
        };
        if let Err(err) = sync_sidecar_temp_marker_dir(data_parent) {
            iroha_logger::warn!(
                ?err,
                ?data_parent,
                kind,
                "failed to sync sidecar temp recovery marker before prune promotion"
            );
            return false;
        }
        if let Some(index_parent) = temp_index_path.parent()
            && index_parent != data_parent
            && let Err(err) = sync_sidecar_temp_marker_dir(index_parent)
        {
            iroha_logger::warn!(
                ?err,
                ?index_parent,
                kind,
                "failed to sync sidecar temp index recovery marker before prune promotion"
            );
            return false;
        }
        if let Err(err) = std::fs::rename(&temp_data_path, data_path) {
            if err.kind() == std::io::ErrorKind::AlreadyExists {
                if let Err(remove_err) = std::fs::remove_file(data_path) {
                    iroha_logger::warn!(
                        ?remove_err,
                        ?data_path,
                        kind,
                        "failed to remove sidecar store before pruned replace"
                    );
                    return false;
                }
                if let Err(rename_err) = std::fs::rename(&temp_data_path, data_path) {
                    iroha_logger::warn!(
                        ?rename_err,
                        ?temp_data_path,
                        ?data_path,
                        kind,
                        "failed to replace sidecar store after removal"
                    );
                    return false;
                }
            } else {
                iroha_logger::warn!(
                    ?err,
                    ?temp_data_path,
                    ?data_path,
                    kind,
                    "failed to replace sidecar store with pruned data"
                );
                let _ = std::fs::remove_file(&temp_data_path);
                let _ = std::fs::remove_file(&temp_index_path);
                return false;
            }
        }
        // The data rename must be stable before the index rename can become visible. This barrier
        // rules out a recovered state with a new index pointing into the old data file.
        if let Err(err) = sync_sidecar_promotion_dir(data_parent) {
            iroha_logger::warn!(
                ?err,
                ?data_parent,
                kind,
                "failed to sync pruned sidecar data before index promotion"
            );
            return false;
        }
        if let Err(err) = std::fs::rename(&temp_index_path, index_path) {
            if err.kind() == std::io::ErrorKind::AlreadyExists {
                if let Err(remove_err) = std::fs::remove_file(index_path) {
                    iroha_logger::warn!(
                        ?remove_err,
                        ?index_path,
                        kind,
                        "failed to remove sidecar index before pruned replace"
                    );
                    return false;
                }
                if let Err(rename_err) = std::fs::rename(&temp_index_path, index_path) {
                    iroha_logger::warn!(
                        ?rename_err,
                        ?temp_index_path,
                        ?index_path,
                        kind,
                        "failed to replace sidecar index after removal"
                    );
                    iroha_logger::warn!(
                        ?temp_index_path,
                        kind,
                        "leaving temp index for sidecar recovery"
                    );
                    return false;
                }
            } else {
                iroha_logger::warn!(
                    ?err,
                    ?temp_index_path,
                    ?index_path,
                    kind,
                    "failed to replace sidecar index with pruned entries"
                );
                let _ = std::fs::remove_file(&temp_data_path);
                iroha_logger::warn!(
                    ?temp_index_path,
                    kind,
                    "leaving temp index for sidecar recovery"
                );
                return false;
            }
        }
        if let Some(parent) = data_path.parent() {
            if let Err(err) = sync_dir(parent) {
                iroha_logger::warn!(
                    ?err,
                    ?parent,
                    kind,
                    "failed to sync sidecar parent directory after prune"
                );
                return false;
            }
        }
        if let Some(parent) = index_path.parent() {
            if let Err(err) = sync_dir(parent) {
                iroha_logger::warn!(
                    ?err,
                    ?parent,
                    kind,
                    "failed to sync sidecar parent directory after prune"
                );
                return false;
            }
        }
        true
    }
}
