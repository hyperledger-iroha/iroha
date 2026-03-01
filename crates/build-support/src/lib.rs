#![deny(warnings)]
//! Shared build utilities for the Iroha workspace.

use std::{
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

const VERGEN_GIT_SHA_ENV: &str = "VERGEN_GIT_SHA";
const VERGEN_CARGO_FEATURES_ENV: &str = "VERGEN_CARGO_FEATURES";
const VERGEN_CARGO_TARGET_TRIPLE_ENV: &str = "VERGEN_CARGO_TARGET_TRIPLE";

/// Emit git and cargo-related metadata expected by workspace crates.
///
/// # Errors
/// This function currently does not return errors, but keeps the fallible
/// signature to preserve compatibility with existing build scripts.
pub fn emit_git_info() -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
    emit_cargo_target_triple();
    emit_cargo_features();
    emit_git_sha();
    emit_git_rerun_hints();
    Ok(())
}

fn emit_cargo_target_triple() {
    let target = env::var("TARGET").unwrap_or_else(|_| "unknown".to_owned());
    println!("cargo:rustc-env={VERGEN_CARGO_TARGET_TRIPLE_ENV}={target}");
}

fn emit_cargo_features() {
    let parsed_features = env::var("CARGO_CFG_FEATURE")
        .ok()
        .as_deref()
        .map(parse_cfg_features)
        .unwrap_or_default();
    let feature_list = if parsed_features.is_empty() {
        "unknown".to_owned()
    } else {
        parsed_features.join(",")
    };
    println!("cargo:rustc-env={VERGEN_CARGO_FEATURES_ENV}={feature_list}");
}

fn emit_git_sha() {
    let sha = git_commit_hash().unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env={VERGEN_GIT_SHA_ENV}={sha}");
}

fn git_commit_hash() -> Option<String> {
    let git_dir = resolve_git_dir()?;
    read_head_commit_hash(&git_dir)
}

fn emit_git_rerun_hints() {
    let Some(git_dir) = resolve_git_dir() else {
        return;
    };

    let head_path = git_dir.join("HEAD");
    emit_rerun_if_changed(&head_path);
    emit_rerun_if_changed(&git_dir.join("packed-refs"));

    if let Some(head_ref) = read_head_reference(&head_path) {
        emit_rerun_if_changed(&git_dir.join(head_ref));
    }
}

fn resolve_git_dir() -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").ok()?);
    let (workspace_root, git_entry) = find_git_entry(manifest_dir)?;

    if git_entry.is_dir() {
        return Some(git_entry);
    }

    let contents = fs::read_to_string(git_entry).ok()?;
    let git_dir = PathBuf::from(parse_gitdir_declaration(&contents)?);
    if git_dir.is_absolute() {
        Some(git_dir)
    } else {
        Some(workspace_root.join(git_dir))
    }
}

fn find_git_entry(mut current_dir: PathBuf) -> Option<(PathBuf, PathBuf)> {
    loop {
        let git_entry = current_dir.join(".git");
        if git_entry.is_dir() || git_entry.is_file() {
            return Some((current_dir, git_entry));
        }
        if !current_dir.pop() {
            return None;
        }
    }
}

fn parse_gitdir_declaration(contents: &str) -> Option<&str> {
    let path = contents.trim().strip_prefix("gitdir:")?.trim();
    if path.is_empty() { None } else { Some(path) }
}

fn read_head_commit_hash(git_dir: &Path) -> Option<String> {
    let head_contents = fs::read_to_string(git_dir.join("HEAD")).ok()?;
    if let Some(reference) = parse_head_reference(&head_contents) {
        read_reference_hash(git_dir, reference)
    } else {
        parse_commit_hash(&head_contents).map(ToOwned::to_owned)
    }
}

fn read_reference_hash(git_dir: &Path, reference: &str) -> Option<String> {
    let loose_ref_path = git_dir.join(reference);
    if let Ok(contents) = fs::read_to_string(loose_ref_path) {
        if let Some(hash) = parse_commit_hash(&contents) {
            return Some(hash.to_owned());
        }
    }

    read_packed_reference_hash(git_dir, reference)
}

fn read_packed_reference_hash(git_dir: &Path, reference: &str) -> Option<String> {
    let packed_refs_contents = fs::read_to_string(git_dir.join("packed-refs")).ok()?;
    parse_packed_ref_hash(&packed_refs_contents, reference).map(ToOwned::to_owned)
}

fn read_head_reference(head_path: &Path) -> Option<PathBuf> {
    let contents = fs::read_to_string(head_path).ok()?;
    parse_head_reference(&contents).map(PathBuf::from)
}

fn parse_head_reference(contents: &str) -> Option<&str> {
    let trimmed = contents.trim();
    let reference = trimmed.strip_prefix("ref: ")?;
    if reference.is_empty() {
        None
    } else {
        Some(reference)
    }
}

fn parse_commit_hash(contents: &str) -> Option<&str> {
    let trimmed = contents.trim();
    if trimmed.is_empty() || !trimmed.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        None
    } else {
        Some(trimmed)
    }
}

fn parse_packed_ref_hash<'a>(contents: &'a str, reference: &str) -> Option<&'a str> {
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('^') {
            continue;
        }

        let mut parts = trimmed.split_whitespace();
        let hash = parts.next()?;
        let packed_ref = parts.next()?;
        if parts.next().is_none() && packed_ref == reference && parse_commit_hash(hash).is_some() {
            return Some(hash);
        }
    }
    None
}

fn parse_cfg_features(contents: &str) -> Vec<String> {
    let mut features: Vec<String> = contents
        .split(',')
        .map(str::trim)
        .filter(|feature| !feature.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    features.sort_unstable();
    features.dedup();
    features
}

fn emit_rerun_if_changed(path: &Path) {
    println!("cargo:rerun-if-changed={}", path.display());
}

/// Warn if mutually exclusive FFI features are enabled simultaneously.
pub fn warn_if_ffi_conflict() {
    let ffi_import = std::env::var_os("CARGO_FEATURE_FFI_IMPORT").is_some();
    let ffi_export = std::env::var_os("CARGO_FEATURE_FFI_EXPORT").is_some();

    warn_if_ffi_conflict_with(ffi_import, ffi_export);
}

fn warn_if_ffi_conflict_with(ffi_import: bool, ffi_export: bool) {
    if ffi_import && ffi_export {
        println!("cargo:warning=Features `ffi_export` and `ffi_import` are mutually exclusive");
        println!("cargo:warning=When both active, `ffi_import` feature takes precedence");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_git_info_runs() {
        emit_git_info().unwrap();
    }

    #[test]
    fn parse_head_reference_parses_symbolic_head() {
        assert_eq!(
            parse_head_reference("ref: refs/heads/main\n"),
            Some("refs/heads/main")
        );
    }

    #[test]
    fn parse_head_reference_ignores_detached_head() {
        assert_eq!(
            parse_head_reference("6f4e5a2d3a9ab7cd61234b1234f8aadeadbeef00\n"),
            None
        );
    }

    #[test]
    fn parse_head_reference_rejects_empty_ref() {
        assert_eq!(parse_head_reference("ref: \n"), None);
    }

    #[test]
    fn parse_gitdir_declaration_parses_path() {
        assert_eq!(
            parse_gitdir_declaration("gitdir: /tmp/worktree/.git\n"),
            Some("/tmp/worktree/.git")
        );
    }

    #[test]
    fn parse_gitdir_declaration_rejects_empty_path() {
        assert_eq!(parse_gitdir_declaration("gitdir:\n"), None);
    }

    #[test]
    fn parse_commit_hash_accepts_hex() {
        assert_eq!(
            parse_commit_hash("6f4e5a2d3a9ab7cd61234b1234f8aadeadbeef00\n"),
            Some("6f4e5a2d3a9ab7cd61234b1234f8aadeadbeef00")
        );
    }

    #[test]
    fn parse_commit_hash_rejects_non_hex() {
        assert_eq!(parse_commit_hash("not-a-hash\n"), None);
    }

    #[test]
    fn parse_packed_ref_hash_finds_matching_reference() {
        let packed_refs = "\
# pack-refs with: peeled fully-peeled sorted\n\
6f4e5a2d3a9ab7cd61234b1234f8aadeadbeef00 refs/heads/main\n\
";
        assert_eq!(
            parse_packed_ref_hash(packed_refs, "refs/heads/main"),
            Some("6f4e5a2d3a9ab7cd61234b1234f8aadeadbeef00")
        );
    }

    #[test]
    fn parse_packed_ref_hash_ignores_peeled_hash_lines() {
        let packed_refs = "\
6f4e5a2d3a9ab7cd61234b1234f8aadeadbeef00 refs/tags/v1\n\
^1111111111111111111111111111111111111111\n\
";
        assert_eq!(parse_packed_ref_hash(packed_refs, "refs/heads/main"), None);
    }

    #[test]
    fn parse_cfg_features_sorts_and_deduplicates() {
        assert_eq!(
            parse_cfg_features("telemetry,zk,telemetry,"),
            vec!["telemetry".to_owned(), "zk".to_owned()]
        );
    }

    #[test]
    fn parse_cfg_features_skips_empty_entries() {
        assert_eq!(parse_cfg_features(" , ,"), Vec::<String>::new());
    }

    #[test]
    fn warn_if_ffi_conflict_emits() {
        warn_if_ffi_conflict_with(true, true);
    }
}
