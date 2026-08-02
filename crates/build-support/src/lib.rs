#![deny(warnings)]
//! Shared build utilities for the Iroha workspace.

use std::{
    env, fs,
    path::{Component, Path, PathBuf},
};

const VERGEN_GIT_SHA_ENV: &str = "VERGEN_GIT_SHA";
const VERGEN_CARGO_FEATURES_ENV: &str = "VERGEN_CARGO_FEATURES";
const VERGEN_CARGO_TARGET_TRIPLE_ENV: &str = "VERGEN_CARGO_TARGET_TRIPLE";
const IROHA_DPN_VALIDATOR_RELEASE_COMMIT_ENV: &str = "IROHA_DPN_VALIDATOR_RELEASE_COMMIT";
const GIT_RERUN_ENV_VARS: &[&str] = &[VERGEN_GIT_SHA_ENV, IROHA_DPN_VALIDATOR_RELEASE_COMMIT_ENV];

#[derive(Debug)]
struct GitDirectories {
    worktree: PathBuf,
    common: PathBuf,
}

/// Emit git and cargo-related metadata expected by workspace crates.
pub fn emit_git_info() {
    emit_cargo_target_triple();
    emit_cargo_features();
    emit_git_sha();
    emit_git_rerun_hints();
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
    if let Some(sha) = env_git_commit_hash() {
        return Some(sha);
    }

    let git_dirs = resolve_git_directories()?;
    read_head_commit_hash(&git_dirs)
}

fn env_git_commit_hash() -> Option<String> {
    let sha = env::var(VERGEN_GIT_SHA_ENV).ok()?;
    let trimmed = sha.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

fn emit_git_rerun_hints() {
    for env_var in GIT_RERUN_ENV_VARS {
        println!("cargo:rerun-if-env-changed={env_var}");
    }

    let Some(git_dirs) = resolve_git_directories() else {
        return;
    };

    let head_path = git_dirs.worktree.join("HEAD");
    emit_existing_rerun_if_changed(&head_path);
    emit_existing_rerun_if_changed(&git_dirs.worktree.join("commondir"));
    emit_existing_rerun_if_changed(&git_dirs.common.join("packed-refs"));

    if let Some(head_ref) = read_head_reference(&head_path) {
        let Some(head_ref) = head_ref.to_str() else {
            return;
        };
        let reference_root = if reference_is_worktree_local(head_ref) {
            &git_dirs.worktree
        } else {
            &git_dirs.common
        };
        if let Some(path) = reference_watch_path(reference_root, head_ref) {
            emit_existing_rerun_if_changed(&path);
        }
    }
}

fn resolve_git_directories() -> Option<GitDirectories> {
    let worktree = resolve_git_dir()?;
    let common = resolve_common_git_dir(&worktree);
    Some(GitDirectories { worktree, common })
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

fn resolve_common_git_dir(git_dir: &Path) -> PathBuf {
    let Some(path) = fs::read_to_string(git_dir.join("commondir"))
        .ok()
        .and_then(|contents| parse_commondir_declaration(&contents).map(PathBuf::from))
    else {
        return git_dir.to_path_buf();
    };
    let common = if path.is_absolute() {
        path
    } else {
        git_dir.join(path)
    };
    fs::canonicalize(&common).unwrap_or(common)
}

fn parse_commondir_declaration(contents: &str) -> Option<&str> {
    let path = contents.trim();
    if path.is_empty() { None } else { Some(path) }
}

fn read_head_commit_hash(git_dirs: &GitDirectories) -> Option<String> {
    let head_contents = fs::read_to_string(git_dirs.worktree.join("HEAD")).ok()?;
    parse_head_reference(&head_contents).map_or_else(
        || parse_commit_hash(&head_contents).map(ToOwned::to_owned),
        |reference| read_reference_hash(git_dirs, reference),
    )
}

fn read_reference_hash(git_dirs: &GitDirectories, reference: &str) -> Option<String> {
    for git_dir in [&git_dirs.worktree, &git_dirs.common] {
        let loose_ref_path = safe_reference_path(git_dir, reference)?;
        if let Ok(contents) = fs::read_to_string(loose_ref_path)
            && let Some(hash) = parse_commit_hash(&contents)
        {
            return Some(hash.to_owned());
        }
    }

    read_packed_reference_hash(&git_dirs.common, reference)
}

fn safe_reference_path(git_dir: &Path, reference: &str) -> Option<PathBuf> {
    let reference = Path::new(reference);
    if reference.is_absolute()
        || !reference
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return None;
    }
    Some(git_dir.join(reference))
}

fn reference_is_worktree_local(reference: &str) -> bool {
    ["refs/bisect/", "refs/worktree/", "refs/rewritten/"]
        .iter()
        .any(|prefix| reference.starts_with(prefix))
}

fn reference_watch_path(git_dir: &Path, reference: &str) -> Option<PathBuf> {
    let mut candidate = safe_reference_path(git_dir, reference)?;
    let refs_root = git_dir.join("refs");
    let boundary = if candidate.starts_with(&refs_root) {
        refs_root
    } else {
        git_dir.to_path_buf()
    };
    loop {
        if candidate.exists() {
            return Some(candidate);
        }
        if candidate == boundary || !candidate.pop() {
            break;
        }
    }
    boundary
        .exists()
        .then_some(boundary)
        .or_else(|| git_dir.exists().then(|| git_dir.to_path_buf()))
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

fn emit_existing_rerun_if_changed(path: &Path) {
    if path.exists() {
        println!("cargo:rerun-if-changed={}", path.display());
    }
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
        emit_git_info();
    }

    #[test]
    fn git_rerun_env_vars_tracks_git_sha_override() {
        assert_eq!(
            GIT_RERUN_ENV_VARS,
            &[VERGEN_GIT_SHA_ENV, IROHA_DPN_VALIDATOR_RELEASE_COMMIT_ENV,]
        );
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
    fn parse_commondir_declaration_parses_linked_worktree_path() {
        assert_eq!(parse_commondir_declaration("../..\n"), Some("../.."));
    }

    #[test]
    fn parse_commondir_declaration_rejects_empty_path() {
        assert_eq!(parse_commondir_declaration(" \n"), None);
    }

    #[test]
    fn linked_worktree_reads_head_from_common_packed_refs() {
        const SHA: &str = "6f4e5a2d3a9ab7cd61234b1234f8aadeadbeef00";

        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time follows the Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "iroha-build-support-{}-{unique}",
            std::process::id()
        ));
        let common = root.join("repo.git");
        let worktree = common.join("worktrees").join("linked");
        fs::create_dir_all(&worktree).expect("create linked-worktree metadata fixture");
        fs::create_dir_all(common.join("refs").join("heads"))
            .expect("create common loose-ref parent");
        fs::write(worktree.join("commondir"), "../..\n").expect("write linked-worktree commondir");
        fs::write(worktree.join("HEAD"), "ref: refs/heads/optimizations\n")
            .expect("write linked-worktree HEAD");
        fs::write(
            common.join("packed-refs"),
            format!("{SHA} refs/heads/optimizations\n"),
        )
        .expect("write common packed refs");

        let resolved_common = resolve_common_git_dir(&worktree);
        assert_eq!(
            resolved_common,
            fs::canonicalize(&common).expect("canonical common git directory")
        );
        let git_dirs = GitDirectories {
            worktree,
            common: resolved_common.clone(),
        };
        assert_eq!(read_head_commit_hash(&git_dirs).as_deref(), Some(SHA));
        assert_eq!(
            reference_watch_path(&resolved_common, "refs/heads/optimizations"),
            Some(resolved_common.join("refs").join("heads"))
        );

        fs::remove_dir_all(root).expect("remove linked-worktree metadata fixture");
    }

    #[test]
    fn unborn_branch_watches_existing_loose_ref_parent() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time follows the Unix epoch")
            .as_nanos();
        let git_dir = std::env::temp_dir().join(format!(
            "iroha-build-support-unborn-{}-{unique}",
            std::process::id()
        ));
        let heads = git_dir.join("refs").join("heads");
        fs::create_dir_all(&heads).expect("create unborn branch ref parent");

        assert_eq!(
            reference_watch_path(&git_dir, "refs/heads/main"),
            Some(heads.clone())
        );
        fs::write(
            heads.join("main"),
            "1111111111111111111111111111111111111111\n",
        )
        .expect("publish first loose branch ref");
        assert_eq!(
            reference_watch_path(&git_dir, "refs/heads/main"),
            Some(heads.join("main"))
        );

        fs::remove_dir_all(git_dir).expect("remove unborn branch fixture");
    }

    #[test]
    fn worktree_local_reference_namespaces_are_classified_exactly() {
        assert!(reference_is_worktree_local("refs/bisect/good"));
        assert!(reference_is_worktree_local("refs/worktree/private"));
        assert!(reference_is_worktree_local("refs/rewritten/topic"));
        assert!(!reference_is_worktree_local("refs/heads/optimizations"));
    }

    #[test]
    fn git_reference_paths_reject_directory_escape() {
        let git_dir = Path::new("/repo.git");
        assert_eq!(
            safe_reference_path(git_dir, "refs/heads/main"),
            Some(git_dir.join("refs/heads/main"))
        );
        assert_eq!(safe_reference_path(git_dir, "../HEAD"), None);
        assert_eq!(safe_reference_path(git_dir, "/outside"), None);
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
