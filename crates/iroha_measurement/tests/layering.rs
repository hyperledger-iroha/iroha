//! The measurement crate stays below proof, Core and data-model crates, and
//! every crate that instruments with it is listed in a tracked inventory.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

/// Workspace crates the measurement crate may reach through normal and build
/// dependencies. Proof, Core, data-model, crypto, telemetry and node crates
/// must never appear here.
const PERMITTED_WORKSPACE_CLOSURE: &[&str] = &[
    "iroha_allocation",
    "iroha_audio",
    "iroha_derive",
    "iroha_derive_primitives",
    "iroha_measurement",
    "iroha_schema",
    "iroha_schema_derive",
    "norito",
    "norito_derive",
];

fn crates_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate lives under crates/")
        .to_path_buf()
}

/// Dependency names per manifest table, read without a TOML dependency.
///
/// Only `name = ...`, `name.workspace = true` and `[...dependencies.name]`
/// forms are used in this workspace's manifests.
fn dependency_tables(manifest: &str) -> BTreeMap<String, BTreeSet<String>> {
    let mut tables: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut current: Option<String> = None;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some(header) = line.strip_prefix('[') {
            let header = header.trim_end_matches(']').trim_matches('[');
            current = None;
            for kind in ["dev-dependencies", "build-dependencies", "dependencies"] {
                if header == kind || header.ends_with(&format!(".{kind}")) {
                    tables.entry(kind.to_owned()).or_default();
                    current = Some(kind.to_owned());
                    break;
                }
                if let Some((prefix, name)) = header.rsplit_once('.')
                    && (prefix == kind || prefix.ends_with(&format!(".{kind}")))
                {
                    tables
                        .entry(kind.to_owned())
                        .or_default()
                        .insert(name.to_owned());
                    break;
                }
            }
            continue;
        }
        if let (Some(kind), Some((key, _))) = (&current, line.split_once('=')) {
            let name = key.trim().split('.').next().unwrap_or_default().trim();
            if !name.is_empty() {
                tables
                    .entry(kind.clone())
                    .or_default()
                    .insert(name.to_owned());
            }
        }
    }
    tables
}

fn manifest_of(name: &str) -> Option<String> {
    std::fs::read_to_string(crates_directory().join(name).join("Cargo.toml")).ok()
}

fn names(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(|name| (*name).to_owned()).collect()
}

#[test]
fn dependency_table_scanner_reads_every_manifest_form() {
    let tables = dependency_tables(
        r#"
[package]
name = "example"

[dependencies]
plain = "1"
inherited.workspace = true
table = { path = "../table", features = ["a"] }
# commented = "1"

[target.'cfg(unix)'.dependencies]
libc = "0.2"

[dependencies.long_form]
version = "1"

[dev-dependencies]
tester = "1"

[build-dependencies]
builder = "1"

[features]
default = []
"#,
    );
    assert_eq!(
        tables["dependencies"],
        names(&["inherited", "libc", "long_form", "plain", "table"])
    );
    assert_eq!(tables["dev-dependencies"], names(&["tester"]));
    assert_eq!(tables["build-dependencies"], names(&["builder"]));
    assert_eq!(tables.len(), 3);
    assert!(dependency_tables("[package]\nname = \"x\"\n").is_empty());
}

#[test]
fn manifest_has_only_the_allocation_codec_and_c_library_dependencies() {
    let manifest = manifest_of("iroha_measurement").expect("own manifest");
    let tables = dependency_tables(&manifest);
    assert_eq!(
        tables["dependencies"],
        names(&["iroha_allocation", "libc", "norito"])
    );
    // No dev-dependencies either: a proof crate may depend on this crate, so a
    // dev-dependency on one would both invert the layering and form a cycle.
    assert!(!tables.contains_key("dev-dependencies"), "{tables:?}");
    assert!(!tables.contains_key("build-dependencies"), "{tables:?}");
    assert!(!manifest.contains("[features]"));
}

#[test]
fn workspace_dependency_closure_excludes_proof_core_and_model_crates() {
    let mut closure = BTreeSet::new();
    let mut pending = vec!["iroha_measurement".to_owned()];
    while let Some(name) = pending.pop() {
        let Some(manifest) = manifest_of(&name) else {
            continue;
        };
        if !closure.insert(name) {
            continue;
        }
        let tables = dependency_tables(&manifest);
        for kind in ["dependencies", "build-dependencies"] {
            pending.extend(tables.get(kind).into_iter().flatten().cloned());
        }
    }
    // Optional platform accelerators of the codec are not enabled by this
    // crate and are not workspace crates under `crates/<name>`.
    closure.remove("gpuzstd_metal");
    assert_eq!(closure, names(PERMITTED_WORKSPACE_CLOSURE));
    for forbidden in [
        "iroha_data_model",
        "iroha_model_base",
        "iroha_core",
        "iroha_core_privacy",
        "iroha_crypto",
        "iroha_telemetry",
        "iroha_config",
        "fastpq_prover",
        "fastpq_isi",
        "iroha_zkp_halo2",
        "ivm",
    ] {
        assert!(manifest_of(forbidden).is_some(), "{forbidden} exists");
        assert!(!closure.contains(forbidden), "{forbidden}");
    }
}

fn repository() -> PathBuf {
    crates_directory()
        .parent()
        .expect("repository root")
        .to_path_buf()
}

/// Text of the string elements of the `key = [ ... ]` array in `manifest`.
fn string_array(manifest: &str, key: &str) -> Vec<String> {
    let mut lines = manifest.lines();
    let mut items = Vec::new();
    while let Some(line) = lines.next() {
        if line.trim() != format!("{key} = [") {
            continue;
        }
        for item in lines.by_ref() {
            let item = item.trim();
            if item.starts_with(']') {
                return items;
            }
            if let Some(text) = item
                .trim_end_matches(',')
                .strip_prefix('"')
                .and_then(|text| text.strip_suffix('"'))
            {
                items.push(text.to_owned());
            }
        }
    }
    items
}

/// Manifest paths, relative to `root`, of the workspace members its manifest
/// declares. A `directory/*` member is every subdirectory with a manifest.
fn workspace_member_manifests(root: &Path) -> BTreeSet<String> {
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).expect("workspace manifest");
    let excluded = string_array(&manifest, "exclude");
    let mut members = BTreeSet::new();
    for member in string_array(&manifest, "members") {
        let directories = member.strip_suffix("/*").map_or_else(
            || vec![member.clone()],
            |parent| {
                std::fs::read_dir(root.join(parent))
                    .unwrap_or_else(|_| panic!("member directory {parent}"))
                    .map(|entry| {
                        format!(
                            "{parent}/{}",
                            entry.expect("member entry").file_name().to_string_lossy()
                        )
                    })
                    .collect()
            },
        );
        for directory in directories {
            if !excluded.contains(&directory) && root.join(&directory).join("Cargo.toml").is_file()
            {
                members.insert(format!("{directory}/Cargo.toml"));
            }
        }
    }
    members
}

/// Rust sources under `directory`, relative to `root`, that name `needle`.
/// Build output and nested packages, which are scanned as their own members,
/// are skipped.
fn sources_naming(root: &Path, directory: &Path, needle: &str, found: &mut BTreeSet<String>) {
    for entry in std::fs::read_dir(root.join(directory)).expect("source directory") {
        let entry = entry.expect("source entry");
        let path = directory.join(entry.file_name());
        let kind = entry.file_type().expect("source entry kind");
        if kind.is_dir() {
            if entry.file_name() != "target" && !root.join(&path).join("Cargo.toml").is_file() {
                sources_naming(root, &path, needle, found);
            }
        } else if path.extension().is_some_and(|extension| extension == "rs")
            && std::fs::read_to_string(root.join(&path)).is_ok_and(|text| text.contains(needle))
        {
            found.insert(path.to_string_lossy().into_owned());
        }
    }
}

/// Dependency tables of `manifest` that select the recorder package, and
/// whether any of them does so under another name.
///
/// A renamed dependency (`alias = { package = "iroha_measurement" }`) would
/// let sources use the recorder without naming it, so it is reported and no
/// inventory entry can admit it.
fn recorder_tables(manifest: &str) -> (BTreeSet<String>, bool) {
    const RENAME: &str = "package = \"iroha_measurement\"";
    let mut kinds = BTreeSet::new();
    let mut any_alias = false;
    // The table the current line belongs to and, for the long
    // `[dependencies.name]` form, the dependency name of that table.
    let mut current: Option<(&str, Option<&str>)> = None;
    for line in manifest.lines().map(str::trim) {
        if let Some(header) = line.strip_prefix('[') {
            let header = header.trim_end_matches(']').trim_matches('[');
            current = None;
            for kind in ["dev-dependencies", "build-dependencies", "dependencies"] {
                if header == kind || header.ends_with(&format!(".{kind}")) {
                    current = Some((kind, None));
                    break;
                }
                if let Some((prefix, name)) = header.rsplit_once('.')
                    && (prefix == kind || prefix.ends_with(&format!(".{kind}")))
                {
                    current = Some((kind, Some(name)));
                    if name == "iroha_measurement" {
                        kinds.insert(kind.to_owned());
                    }
                    break;
                }
            }
            continue;
        }
        let Some((kind, long_form)) = current else {
            continue;
        };
        if line.starts_with('#') {
            continue;
        }
        let key = line.split_once('=').map_or("", |(key, _)| key.trim());
        let named = long_form.is_none() && key.split('.').next() == Some("iroha_measurement");
        let alias = line.contains(RENAME) && long_form != Some("iroha_measurement") && !named;
        if named || alias {
            kinds.insert(kind.to_owned());
        }
        any_alias |= alias;
    }
    (kinds, any_alias)
}

/// One consumer of the recorder: its dependency table and its source files.
#[derive(Debug, PartialEq, Eq)]
struct Consumer {
    package: String,
    dependency: BTreeSet<String>,
    renamed: bool,
    sites: BTreeSet<String>,
}

/// Every workspace member under `root` whose manifest names the recorder,
/// keyed by manifest path, with the files that use it. The recorder's own
/// package is not a consumer.
fn actual_consumers(root: &Path) -> BTreeMap<String, Consumer> {
    let mut consumers = BTreeMap::new();
    for manifest_path in workspace_member_manifests(root) {
        let manifest = std::fs::read_to_string(root.join(&manifest_path)).expect("member manifest");
        let package = manifest
            .lines()
            .find_map(|line| line.trim().strip_prefix("name = "))
            .map(|name| name.trim_matches('"').to_owned())
            .unwrap_or_default();
        if package == "iroha_measurement" || !manifest.contains("iroha_measurement") {
            continue;
        }
        let (dependency, renamed) = recorder_tables(&manifest);
        let mut sites = BTreeSet::new();
        let directory = Path::new(&manifest_path)
            .parent()
            .expect("member directory");
        sources_naming(root, directory, "iroha_measurement", &mut sites);
        consumers.insert(
            manifest_path,
            Consumer {
                package,
                dependency,
                renamed,
                sites,
            },
        );
    }
    consumers
}

/// `fixtures/consumers.json`: the tracked inventory of recorder consumers.
fn inventoried_consumers() -> BTreeMap<String, Consumer> {
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/consumers.json"),
    )
    .expect("consumer inventory");
    let value = norito::json::parse_value(&text).expect("consumer inventory is JSON");
    let map = value.as_object().expect("inventory object");
    assert_eq!(
        map.keys().map(String::as_str).collect::<Vec<_>>(),
        ["consumers", "rule", "schema"]
    );
    assert_eq!(
        map["schema"].as_str(),
        Some("iroha.measurement.consumers.v1")
    );
    assert!(map["rule"].as_str().is_some_and(|rule| !rule.is_empty()));
    let mut consumers = BTreeMap::new();
    for entry in map["consumers"].as_array().expect("consumer list") {
        let entry = entry.as_object().expect("consumer entry");
        let text = |key: &str| entry[key].as_str().expect("text member").to_owned();
        let dependency = text("dependency");
        // A dev-dependency cannot be linked into a node. Any other kind is a
        // production dependency and must say why it cannot reach validation.
        let expected_keys: &[&str] = if dependency == "dev-dependencies" {
            &["dependency", "manifest", "package", "sites"]
        } else {
            &[
                "dependency",
                "manifest",
                "package",
                "production_use",
                "sites",
            ]
        };
        assert_eq!(
            entry.keys().map(String::as_str).collect::<Vec<_>>(),
            expected_keys,
            "{}",
            text("package")
        );
        if dependency != "dev-dependencies" {
            assert!(
                ["dependencies", "build-dependencies"].contains(&dependency.as_str())
                    && !text("production_use").is_empty(),
                "{dependency}"
            );
        }
        let mut sites = BTreeSet::new();
        let mut order = Vec::new();
        for site in entry["sites"].as_array().expect("site list") {
            let site = site.as_object().expect("site entry");
            assert_eq!(
                site.keys().map(String::as_str).collect::<Vec<_>>(),
                ["path", "use"]
            );
            assert!(!site["use"].as_str().expect("use").is_empty());
            let path = site["path"].as_str().expect("path").to_owned();
            order.push(path.clone());
            assert!(sites.insert(path), "duplicate site");
        }
        assert!(order.is_sorted(), "sites are sorted by path");
        let previous = consumers.insert(
            text("manifest"),
            Consumer {
                package: text("package"),
                dependency: BTreeSet::from([dependency]),
                // The inventory cannot admit a renamed dependency.
                renamed: false,
                sites,
            },
        );
        assert!(previous.is_none(), "duplicate consumer");
    }
    consumers
}

#[test]
fn every_recorder_consumer_dependency_kind_and_use_site_is_inventoried() {
    let actual = actual_consumers(&repository());
    assert_eq!(
        actual,
        inventoried_consumers(),
        "update crates/iroha_measurement/fixtures/consumers.json: every workspace member that \
         depends on the recorder, the dependency table it uses and every source file that names \
         it must be listed; validation code must never read a measurement"
    );
    // The members are taken from the workspace manifest, so packages outside
    // `crates/` are covered as well.
    let members = workspace_member_manifests(&repository());
    for member in [
        "crates/iroha_measurement/Cargo.toml",
        "crates/iroha_core_privacy/Cargo.toml",
        "crates/irohad/bins/Cargo.toml",
        "integration_tests/Cargo.toml",
        "xtask/Cargo.toml",
        "mochi/mochi-core/Cargo.toml",
    ] {
        assert!(members.contains(member), "{member}");
    }
    assert!(!members.contains("mochi/fixtures/Cargo.toml"));
}

/// One package of a scratch workspace: directory, manifest text and sources
/// as `(path, text)` pairs.
type ScratchMember<'a> = (&'a str, &'a str, &'a [(&'a str, &'a str)]);

/// A scratch workspace with the given members.
fn scratch_workspace(name: &str, members: &[ScratchMember<'_>]) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "iroha-measurement-layering-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\n    \"crates/*\",\n    \"tools/outside\",\n    \"skipped/*\",\n]\n\
         exclude = [\n    \"skipped/fixtures\",\n]\n",
    )
    .unwrap();
    for (directory, manifest, sources) in members {
        std::fs::create_dir_all(root.join(directory)).unwrap();
        std::fs::write(root.join(directory).join("Cargo.toml"), manifest).unwrap();
        for (path, text) in *sources {
            let path = root.join(directory).join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
    }
    root
}

#[test]
fn consumer_scan_detects_new_sites_dependency_kinds_and_members_outside_crates() {
    let uses = "use iroha_measurement::Session;\n";
    let plain = "pub fn nothing() {}\n";
    let root = scratch_workspace(
        "scan",
        &[
            (
                "crates/dev_only",
                "[package]\nname = \"dev_only\"\n\n[dev-dependencies]\niroha_measurement = { workspace = true }\n",
                &[
                    ("src/lib.rs", plain),
                    ("src/diagnostic.rs", uses),
                    ("tests/measure.rs", uses),
                    ("target/debug/generated.rs", uses),
                ],
            ),
            (
                "crates/dev_only/nested",
                "[package]\nname = \"nested\"\n",
                &[("src/lib.rs", uses)],
            ),
            (
                "crates/production",
                "[package]\nname = \"production\"\n\n[dependencies]\niroha_measurement.workspace = true\n",
                &[("src/lib.rs", uses)],
            ),
            (
                "tools/outside",
                "[package]\nname = \"outside\"\n\n[dependencies]\nrecorder = { package = \"iroha_measurement\", path = \"x\" }\n\n[dev-dependencies]\niroha_measurement = { path = \"x\" }\n",
                &[("src/main.rs", uses)],
            ),
            (
                "crates/unrelated",
                "[package]\nname = \"unrelated\"\n\n[dependencies]\nlibc = \"0.2\"\n",
                &[("src/lib.rs", plain)],
            ),
            (
                "crates/iroha_measurement",
                "[package]\nname = \"iroha_measurement\"\n",
                &[("src/lib.rs", uses)],
            ),
            (
                "skipped/fixtures",
                "[package]\nname = \"fixtures\"\n\n[dependencies]\niroha_measurement = \"1\"\n",
                &[("src/lib.rs", uses)],
            ),
            ("crates/not_a_package", "", &[]),
        ],
    );
    std::fs::remove_file(root.join("crates/not_a_package/Cargo.toml")).unwrap();
    assert_eq!(
        workspace_member_manifests(&root),
        names(&[
            "crates/dev_only/Cargo.toml",
            "crates/iroha_measurement/Cargo.toml",
            "crates/production/Cargo.toml",
            "crates/unrelated/Cargo.toml",
            "tools/outside/Cargo.toml",
        ])
    );
    let consumer = |package: &str, kinds: &[&str], renamed: bool, sites: &[&str]| Consumer {
        package: package.to_owned(),
        dependency: names(kinds),
        renamed,
        sites: names(sites),
    };
    assert_eq!(
        actual_consumers(&root),
        BTreeMap::from([
            (
                "crates/dev_only/Cargo.toml".to_owned(),
                consumer(
                    "dev_only",
                    &["dev-dependencies"],
                    false,
                    &[
                        "crates/dev_only/src/diagnostic.rs",
                        "crates/dev_only/tests/measure.rs"
                    ]
                )
            ),
            (
                "crates/production/Cargo.toml".to_owned(),
                consumer(
                    "production",
                    &["dependencies"],
                    false,
                    &["crates/production/src/lib.rs"]
                )
            ),
            (
                "tools/outside/Cargo.toml".to_owned(),
                consumer(
                    "outside",
                    &["dependencies", "dev-dependencies"],
                    true,
                    &["tools/outside/src/main.rs"]
                )
            ),
        ])
    );
    // Every manifest form that selects the package is attributed to its table.
    assert_eq!(
        recorder_tables(
            "[package]\nname = \"x\"\niroha_measurement = 1\n\n\
             [target.'cfg(unix)'.dependencies]\niroha_measurement.workspace = true\n\n\
             [build-dependencies.iroha_measurement]\npath = \"x\"\n\n\
             [dev-dependencies.alias]\npackage = \"iroha_measurement\"\n\n\
             [dependencies.other]\n# package = \"iroha_measurement\"\nversion = \"1\"\n"
        ),
        (
            names(&["build-dependencies", "dependencies", "dev-dependencies"]),
            true
        )
    );
    assert_eq!(
        recorder_tables("[dependencies]\nlibc = \"0.2\"\n"),
        (BTreeSet::new(), false)
    );
    assert_eq!(
        string_array(
            "members = [\n  \"a\",\n  # comment\n  \"b/*\"\n]\n",
            "members"
        ),
        ["a", "b/*"]
    );
    assert!(string_array("[workspace]\n", "members").is_empty());
    std::fs::remove_dir_all(root).unwrap();
}
