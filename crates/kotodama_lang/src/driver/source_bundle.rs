//! Confined file loading for source-declared includes and local module imports.

use super::*;
use crate::{
    ast::SourceDirectiveKind,
    source::{FrontendBudget, SourceId},
};
use std::collections::VecDeque;

pub(super) fn canonical_overlay_path(
    path: &Path,
    root: &Path,
    overlays: &BTreeMap<PathBuf, String>,
) -> Result<PathBuf, BuildError> {
    let canonical = match path.canonicalize() {
        Ok(canonical) => canonical,
        Err(_) if overlays.contains_key(path) => {
            let mut ancestor = path;
            while !ancestor.exists() {
                ancestor = ancestor.parent().ok_or_else(|| BuildError::InvalidPath {
                    path: path.into(),
                    message: "source has no existing parent".into(),
                })?;
            }
            let canonical = ancestor.canonicalize().map_err(|error| BuildError::Io {
                operation: "canonicalize Kotodama source ancestor",
                path: ancestor.into(),
                message: error.to_string(),
            })?;
            if !canonical.starts_with(root) {
                return Err(BuildError::InvalidPath {
                    path: path.into(),
                    message: "source resolves outside its source root".into(),
                });
            }
            path.to_path_buf()
        }
        Err(error) => {
            return Err(BuildError::Io {
                operation: "canonicalize Kotodama source",
                path: path.into(),
                message: error.to_string(),
            });
        }
    };
    if !canonical.starts_with(root) {
        return Err(BuildError::InvalidPath {
            path: path.into(),
            message: "source resolves outside its source root".into(),
        });
    }
    Ok(canonical)
}

/// Load the reachable companion files of explicit entry sources beneath one source root.
///
/// Paths are relative to their referring source. Only declared dependencies are opened;
/// unrelated files never participate in compilation. Open editor buffers take precedence
/// over the matching disk file, including newly created files not yet saved to disk.
pub fn load_source_companions(
    entries: &[SourceModuleUnit],
    source_root: &Path,
    overlays: &BTreeMap<PathBuf, String>,
) -> Result<Vec<SourceModuleUnit>, BuildError> {
    let canonical_root = source_root.canonicalize().map_err(|error| BuildError::Io {
        operation: "canonicalize Kotodama source root",
        path: source_root.to_path_buf(),
        message: error.to_string(),
    })?;
    let mut known = entries
        .iter()
        .map(|unit| unit.source_name.clone())
        .collect::<BTreeSet<_>>();
    let mut pending = entries
        .iter()
        .cloned()
        .map(|unit| (unit, false))
        .collect::<VecDeque<_>>();
    let mut total_bytes = entries.iter().map(|unit| unit.source.len()).sum::<usize>();
    let mut companions = Vec::new();
    while let Some((unit, fragment)) = pending.pop_front() {
        let file = SourceFile::new(SourceId(0), unit.source_name.as_str(), unit.source.as_str());
        let program = if fragment {
            crate::parser::parse_fragment_source(&file, FrontendBudget::v1())
        } else {
            crate::parser::parse_source(&file, FrontendBudget::v1())
        }
        .map_err(BuildError::Compile)?;
        for directive in program.directives {
            let directive_span = SourceSpan::from_range(&file, directive.source.range);
            let (relative, fragment) = match directive.kind {
                SourceDirectiveKind::Include { path } => (path, true),
                SourceDirectiveKind::Import { path, .. } => (path, false),
            };
            let name = crate::linker::resolve_source_path(&unit.source_name, &relative).map_err(
                |error| {
                    BuildError::Compile(DiagnosticBundle::single(Diagnostic::error(
                        error.diagnostic_code(),
                        DiagnosticPhase::Resolve,
                        error.to_string(),
                        Some(directive_span.clone()),
                    )))
                },
            )?;
            if !known.insert(name.clone()) {
                continue;
            }
            let physical = canonical_root.join(&name);
            let source = if let Some(source) = overlays.get(&physical) {
                // Check every existing ancestor even when the unsaved leaf has no inode yet.
                let mut ancestor = physical.as_path();
                while !ancestor.exists() {
                    ancestor = ancestor.parent().ok_or_else(|| BuildError::InvalidPath {
                        path: physical.clone(),
                        message: "source has no existing parent".into(),
                    })?;
                }
                let canonical = ancestor.canonicalize().map_err(|error| BuildError::Io {
                    operation: "canonicalize Kotodama source ancestor",
                    path: ancestor.into(),
                    message: error.to_string(),
                })?;
                if !canonical.starts_with(&canonical_root) {
                    return Err(BuildError::InvalidPath {
                        path: physical,
                        message: "source resolves outside its source root".into(),
                    });
                }
                source.clone()
            } else {
                let canonical = physical.canonicalize().map_err(|error| {
                    BuildError::Compile(DiagnosticBundle::single(Diagnostic::error(
                        "E_SOURCE_NOT_FOUND",
                        DiagnosticPhase::Resolve,
                        format!("cannot resolve source `{name}`: {error}"),
                        Some(directive_span.clone()),
                    )))
                })?;
                if !canonical.starts_with(&canonical_root) {
                    return Err(BuildError::InvalidPath {
                        path: physical,
                        message: "source resolves outside its source root".into(),
                    });
                }
                read_source_file(&canonical)?
            };
            total_bytes = total_bytes.saturating_add(source.len());
            if known.len() > MAX_MODULE_GRAPH_SOURCES || total_bytes > MAX_MODULE_GRAPH_SOURCE_BYTES
            {
                return Err(BuildError::SourceGraph(SourceGraphError::Budget {
                    sources: known.len(),
                    source_bytes: total_bytes,
                    max_sources: MAX_MODULE_GRAPH_SOURCES,
                    max_source_bytes: MAX_MODULE_GRAPH_SOURCE_BYTES,
                }));
            }
            let source = SourceModuleUnit {
                source_name: name,
                source,
            };
            pending.push_back((source.clone(), fragment));
            companions.push(source);
        }
    }
    companions.sort_by(|left, right| left.source_name.cmp(&right.source_name));
    Ok(companions)
}

/// Capture a deployable root and its declared local dependencies without package imports.
pub fn load_source_project(
    source_path: &Path,
    source_root: &Path,
    overlays: &BTreeMap<PathBuf, String>,
) -> Result<LoadedSourceProject, BuildError> {
    let canonical_root = source_root.canonicalize().map_err(|error| BuildError::Io {
        operation: "canonicalize Kotodama source root",
        path: source_root.into(),
        message: error.to_string(),
    })?;
    let physical = source_path
        .canonicalize()
        .or_else(|error| {
            if overlays.contains_key(source_path) && source_path.starts_with(&canonical_root) {
                Ok(source_path.to_path_buf())
            } else {
                Err(error)
            }
        })
        .map_err(|error| BuildError::Io {
            operation: "canonicalize Kotodama source",
            path: source_path.into(),
            message: error.to_string(),
        })?;
    let source_name = logical_source_name(&physical, &canonical_root)?;
    let source = overlays
        .get(&physical)
        .cloned()
        .map_or_else(|| read_source_file(&physical), Ok)?;
    let root = SourceModuleUnit {
        source_name,
        source,
    };
    let sources = load_source_companions(std::slice::from_ref(&root), &canonical_root, overlays)?;
    let source_paths = std::iter::once(&root)
        .chain(&sources)
        .map(|unit| {
            (
                ProjectSourceKey {
                    package_identity: None,
                    source_name: unit.source_name.clone(),
                },
                canonical_root.join(&unit.source_name),
            )
        })
        .collect();
    Ok(LoadedSourceProject {
        graph: SourceLinkRequest {
            root,
            sources,
            imports: Vec::new(),
            packages: Vec::new(),
        },
        source_paths,
        manifest: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "kotodama-source-set-{}-{}",
                std::process::id(),
                TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&root).unwrap();
            Self(root.canonicalize().unwrap())
        }
        fn write(&self, name: &str, source: &str) -> PathBuf {
            let path = self.0.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, source).unwrap();
            path
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn captures_only_declared_include_and_import_closure() {
        let directory = Directory::new();
        let root = directory.write(
            "contracts/app.ko",
            "seiyaku App { include \"parts/state.ko\"; import \"../math.ko\" as Math; }",
        );
        directory.write(
            "contracts/parts/state.ko",
            "include \"../shared.ko\"; state int total;",
        );
        directory.write("contracts/shared.ko", "const int INITIAL = 0;");
        directory.write("math.ko", "module Math { export fn value() -> int { 1 } }");
        directory.write("unrelated.ko", "deliberately invalid and never read");
        let loaded = load_source_project(&root, &directory.0, &BTreeMap::new()).unwrap();
        assert_eq!(
            loaded
                .graph
                .sources
                .iter()
                .map(|source| source.source_name.as_str())
                .collect::<Vec<_>>(),
            vec!["contracts/parts/state.ko", "contracts/shared.ko", "math.ko"]
        );
        assert_eq!(loaded.source_paths.len(), 4);
    }

    #[test]
    fn open_buffers_can_change_dependencies_and_supply_unsaved_files() {
        let directory = Directory::new();
        let root = directory.write("app.ko", "seiyaku App {}");
        let overlays = BTreeMap::from([
            (root.clone(), "seiyaku App { include \"new.ko\"; }".into()),
            (
                directory.0.join("new.ko"),
                "view fn value() -> int { 9 }".into(),
            ),
        ]);
        let loaded = load_source_project(&root, &directory.0, &overlays).unwrap();
        assert_eq!(loaded.graph.sources[0].source_name, "new.ko");
        assert!(loaded.graph.sources[0].source.contains('9'));
        assert!(!directory.0.join("new.ko").exists());
    }

    #[test]
    fn paths_cannot_escape_the_selected_source_root() {
        let directory = Directory::new();
        let root = directory.write("app.ko", "seiyaku App { include \"../outside.ko\"; }");
        let error = load_source_project(&root, &directory.0, &BTreeMap::new()).unwrap_err();
        assert!(error.to_string().contains("escape"));
    }

    #[cfg(unix)]
    #[test]
    fn symlink_dependencies_cannot_escape_the_selected_source_root() {
        let directory = Directory::new();
        let outside = Directory::new();
        let secret = outside.write("outside.ko", "state int hidden;");
        std::os::unix::fs::symlink(secret, directory.0.join("linked.ko")).unwrap();
        let root = directory.write("app.ko", "seiyaku App { include \"linked.ko\"; }");
        assert!(
            load_source_project(&root, &directory.0, &BTreeMap::new())
                .unwrap_err()
                .to_string()
                .contains("outside")
        );
    }
}
