#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    fn docs_roots() -> [PathBuf; 2] {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        [
            manifest_dir.join("../../docs/source"),
            manifest_dir.join("../ivm/docs"),
        ]
    }

    fn kotodama_doc_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();
        for root in docs_roots() {
            let entries = fs::read_dir(&root).unwrap_or_else(|err| {
                panic!("read {}: {err}", root.display());
            });
            paths.extend(
                entries
                    .filter_map(|entry| entry.ok().map(|item| item.path()))
                    .filter(|path| {
                        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                            return false;
                        };
                        name.starts_with("kotodama_grammar")
                            || name.starts_with("kotodama_examples")
                    }),
            );
        }
        paths.sort();
        paths
    }

    #[test]
    fn kotodama_docs_do_not_advertise_removed_helper_spellings() {
        for path in kotodama_doc_paths() {
            let text = fs::read_to_string(&path).unwrap_or_else(|err| {
                panic!("read {}: {err}", path.display());
            });
            for needle in [
                "get_or_insert_default(",
                ".get_or_insert_default(",
                ".json_get_",
                ".path_map_key(",
                ".path_map_key_norito(",
                ".has(",
                " json_get_",
                " path_map_key(",
                " path_map_key_norito(",
            ] {
                assert!(
                    !text.contains(needle),
                    "{} still contains removed helper spelling `{needle}`",
                    path.display()
                );
            }
        }
    }
}
