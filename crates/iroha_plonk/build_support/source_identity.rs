//! Standalone workspace source provenance used by packaging and capture tooling.
//!
//! This is an identity of canonical native inputs and explicit special
//! membership, not external dependency/toolchain attestation. Native mathematical
//! sources/assets belong under crate `src`/`build_support` roots, which are walked
//! regardless of ignore rules, as is root `.cargo`. Cargo.lock pins the external
//! package selection. Target/backend support is a separate predicate.
//! Unrelated client owners and prose are omitted; native package trees and declared
//! external inputs take precedence. Compiler/generated-input containment remains
//! a separate build check, not a claim made by the conservative reference scan.
//! Generated registry DATA is excluded from the digest to permit generation;
//! packaging separately pins its exact bytes. This digest is never a proof ABI
//! or a compiled admission condition.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    process::Command,
};

use sha2::{Digest, Sha256};

/// The sole generated registry DATA file; all surrounding admission logic is hashed.
pub const GENERATED: &str = "crates/iroha_plonk/src/keys/keygen/builtin/generated.rs";
const SWIFT_OUTPUT_LINK: &str = "IrohaSwift/NoritoBridge.xcframework";
const SWIFT_OUTPUT_TARGET: &str = "../dist/NoritoBridge.xcframework";
const OPTIONAL_DOCS: &str = "iroha-docs";
const CLIENT_ROOTS: &[&str] = &[
    "python/",
    "IrohaSwift/",
    "kotlin/",
    "javascript/",
    "examples/android/",
    "examples/ios/",
];

// These public inputs are consumed by native code/tests despite their owner or
// prose suffix. Keep fixture families whole, including future inventory members.
const EXTERNAL_INPUTS: &[&str] = &[
    "norito.md",
    "generatediso20022.md",
    "specs/kotodama_grammar.md",
    "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift",
];
const EXTERNAL_INPUT_ROOTS: &[&str] = &["javascript/iroha_js/test/fixtures/"];

fn package_roots(entries: &BTreeMap<String, Entry>) -> BTreeSet<String> {
    entries
        .keys()
        .filter_map(|name| name.strip_suffix("/Cargo.toml").map(str::to_owned))
        .collect()
}

// Find the longest package prefix at an exact slash boundary. Walking the
// spelling's ancestors preserves ownership without scanning every package root.
fn package_owner<'a>(name: &str, packages: &'a BTreeSet<String>) -> Option<&'a String> {
    let mut ancestor = name;
    while let Some((parent, _)) = ancestor.rsplit_once('/') {
        if let Some(package) = packages.get(parent) {
            return Some(package);
        }
        ancestor = parent;
    }
    None
}

// Classify known formats without normalizing the exact path used by the census.
// A dotfile with no extension and any unknown extension remain unknown formats.
fn has_extension(name: &str, extension: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|value| value.eq_ignore_ascii_case(extension))
}

fn selected(name: &str, packages: &BTreeSet<String>) -> bool {
    if name == SWIFT_OUTPUT_LINK
        || EXTERNAL_INPUTS.contains(&name)
        || EXTERNAL_INPUT_ROOTS
            .iter()
            .any(|prefix| name.starts_with(prefix))
        || package_owner(name, packages).is_some()
    {
        return true;
    }
    if name == OPTIONAL_DOCS || CLIENT_ROOTS.iter().any(|prefix| name.starts_with(prefix)) {
        return false;
    }
    // Only Markdown prose (including mixed-case extensions) in documentation owners
    // is omitted. Unknown formats, scripts, configuration, fixtures, tests and assets
    // remain inputs by default.
    !(has_extension(name, "md")
        && (!name.contains('/') || name.starts_with("docs/") || name.starts_with("specs/")))
}

/// A complete stable source observation for an external provenance record.
#[derive(Debug)]
pub struct Snapshot {
    /// Domain-separated identity of sorted paths, kinds and complete contents.
    pub digest: [u8; 32],
    /// Individually observed inputs, including excluded generated DATA.
    /// Membership is rescanned explicitly by the provenance evaluator, never by a Cargo build script.
    pub watched_paths: BTreeSet<PathBuf>,
}

#[derive(Clone, PartialEq, Eq)]
enum Entry {
    File,
    Link,
    DocsGitlink(String),
}

fn source_tree(
    root: &Path,
    directory: &Path,
    entries: &mut BTreeMap<String, Entry>,
) -> Result<(), String> {
    let metadata = match fs::symlink_metadata(directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err("unreadable native source root".to_owned()),
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("native source root is not a regular directory".to_owned());
    }
    let children = fs::read_dir(directory).map_err(|_| "unreadable native source directory")?;
    for child in children {
        let child = child.map_err(|_| "incomplete native source directory")?;
        let path = child.path();
        let metadata = fs::symlink_metadata(&path).map_err(|_| "missing native source input")?;
        if metadata.is_dir() {
            source_tree(root, &path, entries)?;
        } else {
            let name = native_relative_name(
                path.strip_prefix(root)
                    .map_err(|_| "native source escapes workspace")?,
            )?;
            let kind = if metadata.file_type().is_symlink() {
                Entry::Link
            } else if metadata.is_file() {
                Entry::File
            } else {
                return Err("unsupported native source input kind".to_owned());
            };
            if entries.get(&name).is_some_and(|old| old != &kind) {
                return Err("native source changed kind".to_owned());
            }
            entries.insert(name, kind);
        }
    }
    Ok(())
}

fn required_source_roots(root: &Path, entries: &mut BTreeMap<String, Entry>) -> Result<(), String> {
    source_tree(root, &root.join(".cargo"), entries)?;
    // Native owners can also live under vendor, nested accelerators or an SDK
    // directory. Ownership wins over the surrounding non-native directory.
    for package in package_roots(entries) {
        source_tree(root, &root.join(&package).join("src"), entries)?;
        source_tree(root, &root.join(&package).join("build_support"), entries)?;
    }
    let crates = root.join("crates");
    let metadata = match fs::symlink_metadata(&crates) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err("unreadable crates directory".to_owned()),
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("crates directory is not a regular directory".to_owned());
    }
    for package in fs::read_dir(&crates).map_err(|_| "unreadable crates directory")? {
        let package = package.map_err(|_| "incomplete crates directory")?;
        let metadata =
            fs::symlink_metadata(package.path()).map_err(|_| "missing crate directory")?;
        if metadata.file_type().is_symlink() {
            return Err("crate directory traverses a symlink".to_owned());
        }
        if metadata.is_dir() {
            source_tree(root, &package.path().join("src"), entries)?;
            source_tree(root, &package.path().join("build_support"), entries)?;
        }
    }
    Ok(())
}

fn relative_reference(base: &Path, value: &str) -> Option<String> {
    let mut parts = PathBuf::new();
    for part in base.join(value).components() {
        match part {
            Component::Normal(value) => parts.push(value),
            Component::CurDir => (),
            Component::ParentDir => {
                if !parts.pop() {
                    return None;
                }
            }
            _ => return None,
        }
    }
    native_relative_name(&parts).ok()
}

// Keep the old conservative token matching and token-family order, including
// matches in comments, strings and longer identifiers. Searching delimiter
// characters avoids repeated slow substring scans in unoptimized build scripts.
fn literal_references(source: &str) -> Vec<&str> {
    let tokens = ["include", "include_str", "include_bytes"];
    let mut macros: [Vec<&str>; 3] = std::array::from_fn(|_| Vec::new());
    for (index, _) in source.match_indices('!') {
        let before = source[..index].trim_end();
        let Some(kind) = tokens.iter().position(|token| before.ends_with(token)) else {
            continue;
        };
        let tail = source[index + 1..].trim_start();
        if tail.starts_with('(') {
            macros[kind].push(&tail[..tail.find(';').unwrap_or(tail.len())]);
        }
    }
    let mut references: Vec<_> = macros.into_iter().flatten().collect();
    for (index, _) in source.match_indices('#') {
        let tail = &source[index..];
        if tail.starts_with("#[path") {
            references.push(&tail[..tail.find(']').unwrap_or(tail.len())]);
        }
    }
    references
}

// Conservative restoration, not a Rust parser or a proof of compiler closure.
// Literal include/path references (including doc attributes and concat! fragments) may
// bring an otherwise unrelated owner back into the census. False positives are
// retained. Actual compiler/build-producer containment is checked separately.
fn restore_references(
    root: &Path,
    all: &BTreeMap<String, Entry>,
    packages: &BTreeSet<String>,
    entries: &mut BTreeMap<String, Entry>,
) -> Result<(), String> {
    // Classify initial and restored Rust sources consistently while preserving
    // exact path spelling in the source identity.
    let mut pending: Vec<_> = entries
        .keys()
        .filter(|name| has_extension(name, "rs"))
        .cloned()
        .collect();
    let mut visited = BTreeSet::new();
    while let Some(name) = pending.pop() {
        if !visited.insert(name.clone()) || entries.get(&name) != Some(&Entry::File) {
            continue;
        }
        let (path, _) = regular_file(root, &name)?;
        let source = fs::read_to_string(path).map_err(|_| "unreadable native source input")?;
        let references = literal_references(&source);
        if references.is_empty() {
            continue;
        }
        let parent = Path::new(&name)
            .parent()
            .ok_or("native source has no parent")?;
        let package = package_owner(&name, packages);
        for value in references
            .iter()
            .flat_map(|source| source.split('"').skip(1).step_by(2))
        {
            if value.contains(['\n', '\r', '\\']) || value.len() > 4096 {
                continue;
            }
            let mut candidates = BTreeSet::new();
            for base in [parent, Path::new("")] {
                if let Some(candidate) = relative_reference(base, value) {
                    candidates.insert(candidate);
                }
            }
            if let Some(package) = package {
                // CARGO_MANIFEST_DIR + "/../../..." has a leading separator
                // in the literal fragment, but is relative to that package.
                if let Some(candidate) =
                    relative_reference(Path::new(package), value.trim_start_matches('/'))
                {
                    candidates.insert(candidate);
                }
            }
            for candidate in candidates {
                if entries.contains_key(&candidate) {
                    continue;
                }
                if selected(&candidate, packages) {
                    // Deleted tracked inputs are absent from the current census,
                    // but a surviving source must not reference their old paths.
                    if all.contains_key(&candidate) && tracked_input_absent(root, &candidate)? {
                        return Err("missing referenced source input".to_owned());
                    }
                    continue;
                }
                let entry = if let Some(entry) = all.get(&candidate) {
                    entry.clone()
                } else {
                    let metadata = match fs::symlink_metadata(root.join(&candidate)) {
                        Ok(metadata) => metadata,
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                        Err(_) => return Err("unreadable referenced native input".to_owned()),
                    };
                    if metadata.file_type().is_symlink() {
                        Entry::Link
                    } else if metadata.is_file() {
                        Entry::File
                    } else {
                        continue;
                    }
                };
                if has_extension(&candidate, "rs") {
                    pending.push(candidate.clone());
                }
                entries.insert(candidate, entry);
            }
        }
    }
    Ok(())
}

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|_| "cannot read Git source inventory".to_owned())?;
    if !output.status.success() {
        return Err("cannot read Git source inventory".to_owned());
    }
    Ok(output.stdout)
}

fn relative_name(bytes: &[u8]) -> Result<String, String> {
    let name = std::str::from_utf8(bytes).map_err(|_| "non-UTF8 source path")?;
    if name.is_empty()
        || name.contains(['\n', '\r', '\\'])
        || Path::new(name)
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err("noncanonical source path".to_owned());
    }
    Ok(name.to_owned())
}

// Filesystem walks and resolved references produce native PathBuf separators.
// Serialize their validated components into the same names Git supplies; never
// normalize untrusted Git bytes or literal backslashes inside Unix filenames.
fn native_relative_name(path: &Path) -> Result<String, String> {
    let mut name = String::new();
    for component in path.components() {
        let Component::Normal(part) = component else {
            return Err("noncanonical source path".to_owned());
        };
        let part = part.to_str().ok_or("non-UTF8 native source path")?;
        if !name.is_empty() {
            name.push('/');
        }
        name.push_str(part);
    }
    relative_name(name.as_bytes())
}

fn tracked_input_absent(root: &Path, name: &str) -> Result<bool, String> {
    let path = root.join(name);
    match fs::symlink_metadata(&path) {
        Ok(_) => return Ok(false),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(_) => return Err("unreadable source input".to_owned()),
    }
    // An absent path beneath a dangling or redirected ancestor is not a source
    // deletion. Preserve the regular-directory boundary even for removed trees.
    for parent in path
        .ancestors()
        .skip(1)
        .take_while(|parent| *parent != root)
    {
        match fs::symlink_metadata(parent) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => (),
            Ok(_) => return Err("source parent traverses a symlink or changed kind".to_owned()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(_) => return Err("unreadable source parent".to_owned()),
        }
    }
    Ok(true)
}

fn inventory(root: &Path) -> Result<BTreeMap<String, Entry>, String> {
    let mut entries = BTreeMap::new();
    for record in git(root, &["ls-files", "--stage", "--cached", "-z"])?
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let tab = record
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or("invalid Git record")?;
        let header = std::str::from_utf8(&record[..tab]).map_err(|_| "invalid Git record")?;
        let fields: Vec<_> = header.split(' ').collect();
        if fields.len() != 3 || fields[2] != "0" {
            return Err("unmerged source inventory".to_owned());
        }
        let name = relative_name(&record[tab + 1..])?;
        let entry = match fields[0] {
            "100644" | "100755" => Entry::File,
            "120000" => Entry::Link,
            "160000" if name == OPTIONAL_DOCS => Entry::DocsGitlink(fields[1].to_owned()),
            _ => return Err("unsupported source inventory kind".to_owned()),
        };
        if entries.insert(name, entry).is_some() {
            return Err("duplicate source inventory path".to_owned());
        }
    }
    for record in git(root, &["ls-files", "--others", "--exclude-standard", "-z"])?
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let name = relative_name(record)?;
        let metadata =
            fs::symlink_metadata(root.join(&name)).map_err(|_| "missing source input")?;
        let entry = if metadata.file_type().is_symlink() {
            Entry::Link
        } else {
            Entry::File
        };
        if entries.insert(name, entry).is_some() {
            return Err("duplicate source inventory path".to_owned());
        }
    }
    // Git's cached inventory still names intentional, unstaged deletions. Bind
    // the files that actually exist, just as for an already staged deletion,
    // while retaining old paths for the surviving-source reference check below.
    let mut all = entries.clone();
    for (name, entry) in &all {
        if !matches!(entry, Entry::DocsGitlink(_))
            && !matches!(name.as_str(), "Cargo.toml" | "Cargo.lock" | GENERATED)
            && !EXTERNAL_INPUTS.contains(&name.as_str())
            && tracked_input_absent(root, name)?
        {
            entries.remove(name);
        }
    }
    required_source_roots(root, &mut entries)?;
    let packages = package_roots(&entries);
    all.extend(
        entries
            .iter()
            .map(|(name, entry)| (name.clone(), entry.clone())),
    );
    entries.retain(|name, _| selected(name, &packages));
    restore_references(root, &all, &packages, &mut entries)?;
    if entries.is_empty() {
        return Err("empty source inventory".to_owned());
    }
    Ok(entries)
}

fn frame(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}

fn regular_file(root: &Path, name: &str) -> Result<(PathBuf, fs::Metadata), String> {
    let path = root.join(name);
    let metadata = fs::symlink_metadata(&path).map_err(|_| "missing source input")?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("source input changed kind".to_owned());
    }
    let resolved = path
        .canonicalize()
        .map_err(|_| "cannot resolve source input")?;
    if resolved != path {
        return Err("source input traverses a symlink or escapes the workspace".to_owned());
    }
    Ok((path, metadata))
}

/// Compute the current complete input identity, failing closed on partial observations.
pub fn snapshot(root: &Path) -> Result<Snapshot, String> {
    let root = root.canonicalize().map_err(|_| "missing workspace root")?;
    let reported = git(&root, &["rev-parse", "--show-toplevel"])?;
    let reported = std::str::from_utf8(&reported)
        .map_err(|_| "non-UTF8 workspace root")?
        .trim_end();
    if Path::new(reported)
        .canonicalize()
        .map_err(|_| "missing Git root")?
        != root
    {
        return Err("source root is not the Git workspace root".to_owned());
    }
    let entries = inventory(&root)?;
    let mut hash = Sha256::new();
    hash.update(b"iroha:plonk:closed-native-workspace-inputs:v2\0");
    let mut watched_paths = BTreeSet::new();
    let mut observed = Vec::new();
    let mut observed_links = Vec::new();
    for (name, entry) in &entries {
        let path = root.join(name);
        if !matches!(entry, Entry::DocsGitlink(_)) {
            if matches!(entry, Entry::File) {
                watched_paths.insert(path.clone());
            }
            let parent = path.parent().ok_or("source input has no parent")?;
            if parent.canonicalize().map_err(|_| "missing source parent")? != parent {
                return Err("source parent traverses a symlink".to_owned());
            }
        }
        if name == GENERATED {
            if !matches!(entry, Entry::File) {
                return Err("generated registry must be regular DATA".to_owned());
            }
            regular_file(&root, name)?;
            continue;
        }
        frame(&mut hash, name.as_bytes());
        match entry {
            Entry::File => {
                let (path, metadata) = regular_file(&root, name)?;
                frame(&mut hash, b"file");
                hash.update(metadata.len().to_le_bytes());
                let mut file = fs::File::open(&path).map_err(|_| "unreadable source input")?;
                let mut buffer = [0; 16384];
                let mut read = 0_u64;
                loop {
                    let size = file
                        .read(&mut buffer)
                        .map_err(|_| "unreadable source input")?;
                    if size == 0 {
                        break;
                    }
                    read = read
                        .checked_add(size as u64)
                        .ok_or("source input length overflow")?;
                    hash.update(&buffer[..size]);
                }
                if read != metadata.len() {
                    return Err("source changed during identity computation".to_owned());
                }
                observed.push((
                    path,
                    metadata.len(),
                    metadata
                        .modified()
                        .map_err(|_| "missing source timestamp")?,
                ));
            }
            Entry::Link => {
                let target = fs::read_link(&path).map_err(|_| "source symlink changed kind")?;
                let target_name = target.to_str().ok_or("non-UTF8 source symlink")?;
                frame(&mut hash, b"symlink");
                frame(&mut hash, target_name.as_bytes());
                if name == SWIFT_OUTPUT_LINK {
                    // This exact shipped-output link is not a mathematical input.
                    // Following it would hash the newly linked native library into
                    // its own source identity. Bind its spelling, never its target.
                    if target_name != SWIFT_OUTPUT_TARGET {
                        return Err("changed generated Swift bridge symlink".to_owned());
                    }
                } else {
                    let resolved = path
                        .canonicalize()
                        .map_err(|_| "unresolved source symlink")?;
                    let relative = resolved
                        .strip_prefix(&root)
                        .map_err(|_| "source symlink escapes workspace")?;
                    let relative = native_relative_name(relative)?;
                    if relative == GENERATED || entries.get(&relative) != Some(&Entry::File) {
                        return Err(
                            "source symlink target is not an included regular input".to_owned()
                        );
                    }
                    regular_file(&root, &relative)?;
                }
                observed_links.push((path, target));
            }
            Entry::DocsGitlink(object) => {
                frame(&mut hash, b"optional-docs-gitlink:160000");
                frame(&mut hash, object.as_bytes());
            }
        }
    }
    for (path, target) in observed_links {
        if fs::read_link(path).ok().as_ref() != Some(&target) {
            return Err("source symlink changed during identity computation".to_owned());
        }
    }
    for (path, len, modified) in observed {
        let metadata = fs::symlink_metadata(path)
            .map_err(|_| "source disappeared during identity computation")?;
        if !metadata.is_file()
            || metadata.len() != len
            || metadata.modified().ok() != Some(modified)
        {
            return Err("source changed during identity computation".to_owned());
        }
    }
    if inventory(&root)? != entries {
        return Err("source membership changed during identity computation".to_owned());
    }
    watched_paths.insert(root.join(GENERATED));
    watched_paths.insert(root.join(".git/index"));
    watched_paths.insert(root.join(".git/info/exclude"));
    Ok(Snapshot {
        digest: hash.finalize().into(),
        watched_paths,
    })
}

/// Reviewed native target families; this predicate is not evidence of target execution.
pub fn supported_target(arch: &str, os: &str, width: &str, endian: &str) -> bool {
    endian == "little"
        && matches!(
            (arch, os, width),
            ("aarch64", "android" | "ios" | "macos" | "linux", "64")
                | (
                    "x86_64",
                    "android" | "ios" | "macos" | "linux" | "windows",
                    "64"
                )
                | ("arm", "android", "32")
        )
}

#[cfg(test)]
mod native_path_tests {
    //! Host paths share canonical census spelling without weakening Git admission.

    use super::{native_relative_name, relative_name, relative_reference};
    use std::path::{Path, PathBuf};

    #[test]
    fn native_components_preserve_case_and_use_git_separators() {
        let path = PathBuf::from("crates")
            .join("proof")
            .join("src")
            .join("Circuit.Rs");
        assert_eq!(
            native_relative_name(&path).unwrap(),
            "crates/proof/src/Circuit.Rs"
        );
    }

    #[test]
    fn literal_references_restore_canonical_workspace_names() {
        assert_eq!(
            relative_reference(Path::new("crates/proof/src"), "../../../docs/Profile.MD"),
            Some("docs/Profile.MD".to_owned())
        );
        assert_eq!(
            relative_reference(Path::new("crates"), "../../outside"),
            None
        );
    }

    #[test]
    fn native_conversion_keeps_git_wire_paths_strict() {
        for bytes in [
            b"crates\\proof\\src\\circuit.rs".as_slice(),
            b"crates/proof/../circuit.rs",
            b"crates/proof/line\nbreak.rs",
        ] {
            assert!(relative_name(bytes).is_err());
        }
        for path in ["", "../outside", "crates/../outside"] {
            assert!(native_relative_name(Path::new(path)).is_err());
        }
    }

    #[cfg(unix)]
    #[test]
    fn unix_literal_backslashes_are_not_reinterpreted_as_separators() {
        assert!(native_relative_name(Path::new("crates/proof\\substitute.rs")).is_err());
    }
}

#[cfg(test)]
mod reference_tests {
    use super::literal_references;

    fn reference_extraction_oracle(source: &str) -> Vec<&str> {
        let mut references = Vec::new();
        for token in ["include", "include_str", "include_bytes"] {
            for (index, _) in source.match_indices(token) {
                let tail = source[index + token.len()..].trim_start();
                let Some(tail) = tail.strip_prefix('!').map(str::trim_start) else {
                    continue;
                };
                if tail.starts_with('(') {
                    references.push(&tail[..tail.find(';').unwrap_or(tail.len())]);
                }
            }
        }
        for (index, _) in source.match_indices("#[path") {
            let tail = &source[index..];
            references.push(&tail[..tail.find(']').unwrap_or(tail.len())]);
        }
        references
    }

    #[test]
    fn extraction_preserves_reference_order_and_conservative_false_positives() {
        let examples = [
            r##"include_bytes!("third"); include!("first"); include_str!("second"); #[path = "last"] mod m;"##,
            r##"// include!("comment");
const TEXT: &str = "include_str!(\"quoted\");";
#[pathology = "false-positive"] struct Any;
"##,
            r##"include!(concat!(env!("CARGO_MANIFEST_DIR"), "/file.rs"));
include_bytes!("semicolon;inside"); #[path = "bracket]inside"]
include_str!("unterminated""##,
            "include \u{2003}!\u{a0}(\"unicode\"); include\u{200b}!(\"not-whitespace\");",
            r##"includeinclude!("overlap"); include_bytesinclude_str!("nested-name");
notinclude!("suffix"); include!!("double-bang");
include!("include_str!(\"nested-string\");");"##,
            r##"#[path = "one"] #[path="two"] # [path="unmatched"]
#[path #[path="nested"] #[path = "unterminated""##,
        ];
        for source in examples {
            assert_eq!(
                literal_references(source),
                reference_extraction_oracle(source),
                "{source:?}"
            );
        }
    }

    #[test]
    fn extraction_matches_reference_grammar_across_token_boundaries() {
        for prefix in [
            "",
            "pre",
            "include",
            "include_str",
            "include_bytes",
            "é",
            "#!",
        ] {
            for token in [
                "include",
                "include_str",
                "include_bytes",
                "include_strange",
                "INCLUDE",
            ] {
                for before in [
                    "", " ", "\n", "\r\n", "\t", "\u{a0}", "\u{2003}", "\u{200b}",
                ] {
                    for bang in ["!", "!!", "! /* comment */", "", "!\n"] {
                        for after in ["", " ", "\u{2003}"] {
                            for tail in [
                                "(\"file\");",
                                "[\"file\"];",
                                "{\"file\"};",
                                "(\"semi;colon\")",
                            ] {
                                let source = format!("{prefix}{token}{before}{bang}{after}{tail}");
                                assert_eq!(
                                    literal_references(&source),
                                    reference_extraction_oracle(&source),
                                    "{source:?}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod owner_lookup_tests {
    use super::*;

    fn linear_owner_oracle<'a>(name: &str, packages: &'a BTreeSet<String>) -> Option<&'a String> {
        packages
            .iter()
            .filter(|package| name.starts_with(&format!("{package}/")))
            .max_by_key(|package| package.len())
    }

    fn linear_selection_oracle(name: &str, packages: &BTreeSet<String>) -> bool {
        if name == SWIFT_OUTPUT_LINK
            || EXTERNAL_INPUTS.contains(&name)
            || EXTERNAL_INPUT_ROOTS
                .iter()
                .any(|prefix| name.starts_with(prefix))
            || packages.iter().any(|package| {
                name.strip_prefix(package)
                    .is_some_and(|rest| rest.starts_with('/'))
            })
        {
            return true;
        }
        if name == OPTIONAL_DOCS || CLIENT_ROOTS.iter().any(|prefix| name.starts_with(prefix)) {
            return false;
        }
        !(has_extension(name, "md")
            && (!name.contains('/') || name.starts_with("docs/") || name.starts_with("specs/")))
    }

    #[test]
    fn manifest_roots_keep_strict_parent_and_nested_package_ownership() {
        let entries = [
            "Cargo.toml",
            "native/Cargo.toml",
            "native/child/Cargo.toml",
            "kotlin/bridge/Cargo.toml",
            "docs/native/Cargo.toml",
        ]
        .into_iter()
        .map(|name| (name.to_owned(), Entry::File))
        .collect();
        let packages = package_roots(&entries);
        assert!(
            !packages.contains(""),
            "the workspace root is not a blanket package owner"
        );
        for (name, expected) in [
            ("Cargo.toml", None),
            ("native", None),
            ("native/Cargo.toml", Some("native")),
            ("native/child", Some("native")),
            ("native/child/Cargo.toml", Some("native/child")),
            ("native/child/src/lib.rs", Some("native/child")),
            ("native_suffix/src/lib.rs", None),
            ("kotlin/bridge/README.md", Some("kotlin/bridge")),
            ("docs/native/README.MD", Some("docs/native")),
        ] {
            assert_eq!(
                package_owner(name, &packages).map(String::as_str),
                expected,
                "{name}"
            );
        }
        assert!(selected("kotlin/bridge/README.md", &packages));
        assert!(selected("docs/native/README.MD", &packages));
        assert!(!selected("kotlin/README.md", &packages));
        assert!(!selected("docs/README.md", &packages));
    }

    #[test]
    fn ancestor_lookup_and_selection_match_linear_oracles() {
        let roots = [
            "",
            "native",
            "native/child",
            "native/child/grandchild",
            "native_suffix",
            "docs/native",
            "kotlin/bridge",
            "é",
            "é/所有者",
            "trailing/",
        ];
        let names = [
            "",
            "/",
            "/Cargo.toml",
            "Cargo.toml",
            "README.md",
            "README.MD",
            ".md",
            "native",
            "native/",
            "native/Cargo.toml",
            "native/child",
            "native/child/",
            "native/child/Cargo.toml",
            "native/child/grandchild/src/lib.rs",
            "native_suffix/src/lib.rs",
            "native_suffixed/src/lib.rs",
            "native//source.rs",
            "docs/native",
            "docs/native/README.MD",
            "docs/README.md",
            "specs/README.md",
            "kotlin/bridge/README.md",
            "kotlin/README.md",
            "é/所有者/src/lib.rs",
            "é/所有者",
            "é/所有者suffix/a.rs",
            "trailing//source.rs",
            "trailing/source.rs",
            "iroha-docs",
            "iroha-docs/file.rs",
            "javascript/iroha_js/test/fixtures/new.bin",
            "norito.md",
            "IrohaSwift/NoritoBridge.xcframework",
            "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift",
        ];
        for mask in 0..1_u16 << roots.len() {
            let packages = roots
                .iter()
                .enumerate()
                .filter(|(index, _)| mask & (1 << index) != 0)
                .map(|(_, root)| (*root).to_owned())
                .collect();
            for name in names {
                assert_eq!(
                    package_owner(name, &packages),
                    linear_owner_oracle(name, &packages),
                    "owner mismatch for {name:?} and {packages:?}"
                );
                assert_eq!(
                    selected(name, &packages),
                    linear_selection_oracle(name, &packages),
                    "selection mismatch for {name:?} and {packages:?}"
                );
            }
        }
    }
}
