//! Bounded Cargo protocol admission; an event never replaces retained file custody.

use super::{super::invalid, Artifact};
use norito::json::{self, Value};
use std::{
    io,
    path::{Path, PathBuf},
};

pub(super) const EVENT_LIMIT: usize = 2 * 1024 * 1024;
pub(super) const OUTPUT_LIMIT: usize = 128 * 1024 * 1024;

pub(super) struct Expected {
    pub(super) artifact: Artifact,
    pub(super) package: String,
    pub(super) manifest: PathBuf,
    pub(super) source: PathBuf,
    pub(super) executable: PathBuf,
}

fn text<'a>(value: &'a Value, name: &str) -> io::Result<&'a str> {
    value
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("Cargo protocol omitted a required text field"))
}

fn path_matches(value: &str, expected: &Path) -> bool {
    // Cargo's transport may use native separators, but does not gain authority to
    // choose a different spelling, relative path, traversal, or executable.
    let actual = Path::new(value);
    actual.is_absolute() && actual.as_os_str() == expected.as_os_str()
}

fn library_kinds(values: &[Value]) -> bool {
    values.len() == 2 && values[0].as_str() == Some("cdylib") && values[1].as_str() == Some("rlib")
}

pub(super) fn package_identity(
    bytes: &[u8],
    manifest: &Path,
    source: &Path,
    artifact: Artifact,
) -> io::Result<String> {
    if bytes.len() > EVENT_LIMIT {
        return Err(invalid("Cargo metadata exceeds its bound"));
    }
    let metadata: Value =
        json::from_slice(bytes).map_err(|_| invalid("invalid Cargo metadata JSON"))?;
    if metadata.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(invalid("unsupported Cargo metadata format"));
    }
    let packages = metadata
        .get("packages")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("Cargo metadata omitted packages"))?;
    let mut selected = None;
    for package in packages {
        if package.get("name").and_then(Value::as_str) != Some(artifact.package()) {
            continue;
        }
        if selected.is_some() || !path_matches(text(package, "manifest_path")?, manifest) {
            return Err(invalid(
                "Cargo metadata selected an ambiguous or foreign publisher package",
            ));
        }
        let targets = package
            .get("targets")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("Cargo metadata omitted publisher targets"))?;
        let mut binary = false;
        for target in targets {
            if target.get("name").and_then(Value::as_str) != Some(artifact.package()) {
                continue;
            }
            let kinds = target
                .get("kind")
                .and_then(Value::as_array)
                .ok_or_else(|| invalid("Cargo metadata omitted target kinds"))?;
            let selected = match artifact {
                Artifact::Publisher => kinds.len() == 1 && kinds[0].as_str() == Some("bin"),
                Artifact::ProcessBridge => {
                    library_kinds(kinds)
                        && target
                            .get("crate_types")
                            .and_then(Value::as_array)
                            .is_some_and(|types| library_kinds(types))
                }
            };
            if selected {
                if binary || !path_matches(text(target, "src_path")?, source) {
                    return Err(invalid(
                        "Cargo metadata publisher source is ambiguous or foreign",
                    ));
                }
                binary = true;
            }
        }
        if !binary {
            return Err(invalid("Cargo metadata omitted the exact publisher binary"));
        }
        let id = text(package, "id")?;
        if id.is_empty() || id.len() > 8192 {
            return Err(invalid("Cargo package identity exceeds its bound"));
        }
        selected = Some(id.to_owned());
    }
    selected.ok_or_else(|| invalid("Cargo metadata omitted the publisher package"))
}

pub(super) struct Observer {
    expected: Expected,
    artifact: bool,
    finished: bool,
}

impl Observer {
    pub(super) fn new(expected: Expected) -> Self {
        Self {
            expected,
            artifact: false,
            finished: false,
        }
    }

    pub(super) fn line(&mut self, bytes: &[u8]) -> io::Result<()> {
        if bytes.is_empty() || bytes.len() > EVENT_LIMIT || self.finished {
            return Err(invalid(
                "Cargo emitted an empty, excessive or post-terminal event",
            ));
        }
        let event: Value =
            json::from_slice(bytes).map_err(|_| invalid("invalid Cargo artifact event JSON"))?;
        match text(&event, "reason")? {
            "compiler-message" | "build-script-executed" => Ok(()),
            "build-finished" => {
                if event.get("success").and_then(Value::as_bool) != Some(true) || !self.artifact {
                    return Err(invalid(
                        "Cargo did not finish with the exact successful publisher artifact",
                    ));
                }
                self.finished = true;
                Ok(())
            }
            "compiler-artifact" => {
                if text(&event, "package_id")? != self.expected.package {
                    return Ok(());
                }
                let target = event
                    .get("target")
                    .ok_or_else(|| invalid("Cargo artifact omitted target"))?;
                let kinds = target
                    .get("kind")
                    .and_then(Value::as_array)
                    .ok_or_else(|| invalid("Cargo artifact omitted target kinds"))?;
                if self.expected.artifact == Artifact::Publisher
                    && kinds.len() == 1
                    && kinds[0].as_str() == Some("lib")
                {
                    return Ok(());
                }
                let profile = event
                    .get("profile")
                    .ok_or_else(|| invalid("Cargo artifact omitted profile"))?;
                let filenames = event
                    .get("filenames")
                    .and_then(Value::as_array)
                    .ok_or_else(|| invalid("Cargo artifact omitted filenames"))?;
                let selected = match self.expected.artifact {
                    Artifact::Publisher => {
                        kinds.len() == 1
                            && kinds[0].as_str() == Some("bin")
                            && path_matches(text(&event, "executable")?, &self.expected.executable)
                    }
                    Artifact::ProcessBridge => {
                        library_kinds(kinds)
                            && target
                                .get("crate_types")
                                .and_then(Value::as_array)
                                .is_some_and(|types| library_kinds(types))
                            && event.get("executable").is_some_and(Value::is_null)
                            && event
                                .get("features")
                                .and_then(Value::as_array)
                                .is_some_and(Vec::is_empty)
                    }
                };
                let matching_filenames = filenames
                    .iter()
                    .filter(|name| {
                        name.as_str()
                            .is_some_and(|name| path_matches(name, &self.expected.executable))
                    })
                    .count();
                let filenames_match = match self.expected.artifact {
                    Artifact::Publisher => matching_filenames > 0,
                    Artifact::ProcessBridge => matching_filenames == 1,
                };
                if self.artifact
                    || text(target, "name")? != self.expected.artifact.package()
                    || !selected
                    || !path_matches(text(&event, "manifest_path")?, &self.expected.manifest)
                    || !path_matches(text(target, "src_path")?, &self.expected.source)
                    || !filenames_match
                    || text(profile, "opt_level")? != "3"
                    || profile.get("debug_assertions").and_then(Value::as_bool) != Some(false)
                    || profile.get("test").and_then(Value::as_bool) != Some(false)
                    || event.get("fresh").and_then(Value::as_bool) != Some(false)
                {
                    return Err(invalid(
                        "Cargo publisher artifact differs from the fresh native release build",
                    ));
                }
                self.artifact = true;
                Ok(())
            }
            _ => Err(invalid("unknown Cargo message reason")),
        }
    }

    pub(super) fn finish(self) -> io::Result<()> {
        if self.artifact && self.finished {
            Ok(())
        } else {
            Err(invalid(
                "Cargo stream ended before successful publisher completion",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expected() -> Expected {
        let root = std::env::temp_dir();
        Expected {
            artifact: Artifact::Publisher,
            package: "path+file:///candidate#publisher@1.0.0".into(),
            manifest: root.join("source").join("Cargo.toml"),
            source: root.join("source").join("main.rs"),
            executable: root.join("target").join(if cfg!(windows) {
                "publisher.exe"
            } else {
                "publisher"
            }),
        }
    }
    fn artifact(expected: &Expected) -> Value {
        norito::json!({"reason":"compiler-artifact","package_id":(expected.package.clone()),
            "manifest_path":(expected.manifest.to_str().unwrap()),
            "target":{"name":"iroha_release_bundle","kind":["bin"],"src_path":(expected.source.to_str().unwrap())},
            "profile":{"opt_level":"3","debug_assertions":false,"test":false},
            "filenames":[(expected.executable.to_str().unwrap())],"executable":(expected.executable.to_str().unwrap()),"fresh":false})
    }
    fn bytes(value: &Value) -> Vec<u8> {
        json::to_vec(value).unwrap()
    }
    const SUCCESS: &[u8] = br#"{"reason":"build-finished","success":true}"#;

    #[test]
    fn exact_fresh_artifact_requires_successful_terminal_event() {
        let expected = expected();
        let event = bytes(&artifact(&expected));
        let mut observer = Observer::new(expected);
        observer.line(&event).unwrap();
        observer.line(SUCCESS).unwrap();
        observer.finish().unwrap();
    }

    #[test]
    fn plausible_artifact_without_success_never_grants_authority() {
        let expected = expected();
        let event = bytes(&artifact(&expected));
        let mut observer = Observer::new(expected);
        observer.line(&event).unwrap();
        assert!(observer.finish().is_err());
        assert!(Observer::new(self::expected()).line(SUCCESS).is_err());
    }

    #[test]
    fn duplicate_fresh_cached_foreign_and_test_artifacts_refuse() {
        for change in 0..7 {
            let expected = expected();
            let mut event = artifact(&expected);
            match change {
                0 => *event.get_mut("fresh").unwrap() = true.into(),
                1 => *event.get_mut("executable").unwrap() = "foreign".into(),
                2 => {
                    *event
                        .get_mut("target")
                        .unwrap()
                        .get_mut("src_path")
                        .unwrap() = "foreign".into()
                }
                3 => *event.get_mut("manifest_path").unwrap() = "foreign".into(),
                4 => *event.get_mut("profile").unwrap().get_mut("test").unwrap() = true.into(),
                5 => {
                    *event
                        .get_mut("profile")
                        .unwrap()
                        .get_mut("debug_assertions")
                        .unwrap() = true.into()
                }
                _ => {
                    *event
                        .get_mut("profile")
                        .unwrap()
                        .get_mut("opt_level")
                        .unwrap() = "0".into()
                }
            }
            assert!(Observer::new(expected).line(&bytes(&event)).is_err());
        }
        let expected = expected();
        let event = bytes(&artifact(&expected));
        let mut observer = Observer::new(expected);
        observer.line(&event).unwrap();
        assert!(observer.line(&event).is_err());
    }

    #[test]
    fn false_terminal_unknown_and_trailing_events_refuse() {
        let expected = expected();
        let event = bytes(&artifact(&expected));
        let mut observer = Observer::new(expected);
        observer.line(&event).unwrap();
        assert!(
            observer
                .line(br#"{"reason":"build-finished","success":false}"#)
                .is_err()
        );
        assert!(observer.line(br#"{"reason":"future-alias"}"#).is_err());
        observer.line(SUCCESS).unwrap();
        assert!(observer.line(SUCCESS).is_err());
    }

    #[test]
    fn metadata_joins_exact_package_manifest_and_binary_source() {
        let expected = expected();
        let metadata = norito::json!({"version":1,"packages":[{"name":"iroha_release_bundle","id":(expected.package.clone()),
            "manifest_path":(expected.manifest.to_str().unwrap()),"targets":[{"name":"iroha_release_bundle","kind":["lib"],"src_path":"unused"},
            {"name":"iroha_release_bundle","kind":["bin"],"src_path":(expected.source.to_str().unwrap())}]}]});
        assert_eq!(
            package_identity(
                &bytes(&metadata),
                &expected.manifest,
                &expected.source,
                Artifact::Publisher
            )
            .unwrap(),
            expected.package
        );
        assert!(
            package_identity(
                &bytes(&metadata),
                &expected.source,
                &expected.manifest,
                Artifact::Publisher
            )
            .is_err()
        );
    }

    fn bridge_expected() -> Expected {
        let mut expected = expected();
        expected.artifact = Artifact::ProcessBridge;
        expected.source = expected.source.with_file_name("lib.rs");
        expected.executable = expected
            .executable
            .with_file_name(Artifact::ProcessBridge.filename());
        expected.package = "path+file:///candidate#iroha_process_bridge@1.0.0".into();
        expected
    }

    fn bridge_artifact(expected: &Expected) -> Value {
        let mut event = artifact(expected);
        *event.get_mut("target").unwrap() = norito::json!({
            "name":"iroha_process_bridge", "kind":["cdylib","rlib"],
            "crate_types":["cdylib","rlib"], "src_path":(expected.source.to_str().unwrap())});
        *event.get_mut("executable").unwrap() = Value::Null;
        event
            .as_object_mut()
            .unwrap()
            .insert("features".into(), norito::json!([]));
        event
    }

    #[test]
    fn bridge_metadata_requires_exact_package_library_kinds_and_source() {
        let expected = bridge_expected();
        let metadata = norito::json!({"version":1,"packages":[{
            "name":"iroha_process_bridge","id":(expected.package.clone()),
            "manifest_path":(expected.manifest.to_str().unwrap()),"targets":[{
                "name":"iroha_process_bridge","kind":["cdylib","rlib"],
                "crate_types":["cdylib","rlib"],"src_path":(expected.source.to_str().unwrap())}]}]});
        assert_eq!(
            package_identity(
                &bytes(&metadata),
                &expected.manifest,
                &expected.source,
                Artifact::ProcessBridge
            )
            .unwrap(),
            expected.package
        );
        assert!(
            package_identity(
                &bytes(&metadata),
                &expected.manifest,
                &expected.source,
                Artifact::Publisher
            )
            .is_err()
        );
        for field in ["kind", "crate_types"] {
            let mut bad = metadata.clone();
            *bad.get_mut("packages")
                .unwrap()
                .get_mut(0)
                .unwrap()
                .get_mut("targets")
                .unwrap()
                .get_mut(0)
                .unwrap()
                .get_mut(field)
                .unwrap() = norito::json!(["rlib"]);
            assert!(
                package_identity(
                    &bytes(&bad),
                    &expected.manifest,
                    &expected.source,
                    Artifact::ProcessBridge
                )
                .is_err()
            );
        }
        assert!(
            package_identity(
                &bytes(&metadata),
                &expected.source,
                &expected.manifest,
                Artifact::ProcessBridge
            )
            .is_err()
        );
    }

    #[test]
    fn bridge_exact_shared_artifact_requires_fresh_successful_terminal() {
        let expected = bridge_expected();
        let event = bytes(&bridge_artifact(&expected));
        let mut observer = Observer::new(expected);
        observer.line(&event).unwrap();
        observer.line(SUCCESS).unwrap();
        observer.finish().unwrap();
        let mut unfinished = Observer::new(bridge_expected());
        unfinished.line(&event).unwrap();
        assert!(unfinished.finish().is_err());
    }

    #[test]
    fn bridge_wrong_kind_features_executable_filename_and_profile_refuse() {
        for change in 0..10 {
            let expected = bridge_expected();
            let mut event = bridge_artifact(&expected);
            match change {
                0 => {
                    *event.get_mut("target").unwrap().get_mut("kind").unwrap() =
                        norito::json!(["bin"])
                }
                1 => {
                    *event
                        .get_mut("target")
                        .unwrap()
                        .get_mut("crate_types")
                        .unwrap() = norito::json!(["rlib"])
                }
                2 => *event.get_mut("features").unwrap() = norito::json!(["unreviewed"]),
                3 => {
                    *event.get_mut("executable").unwrap() =
                        expected.executable.to_str().unwrap().into()
                }
                4 => *event.get_mut("filenames").unwrap() = norito::json!(["foreign.so"]),
                5 => *event.get_mut("fresh").unwrap() = true.into(),
                6 => *event.get_mut("profile").unwrap().get_mut("test").unwrap() = true.into(),
                7 => {
                    *event
                        .get_mut("target")
                        .unwrap()
                        .get_mut("src_path")
                        .unwrap() = "foreign.rs".into()
                }
                8 => event
                    .get_mut("filenames")
                    .unwrap()
                    .as_array_mut()
                    .unwrap()
                    .push(expected.executable.to_str().unwrap().into()),
                _ => {
                    *event.get_mut("target").unwrap().get_mut("name").unwrap() =
                        "iroha_release_bundle".into()
                }
            }
            assert!(Observer::new(expected).line(&bytes(&event)).is_err());
        }
        let expected = bridge_expected();
        let event = bytes(&bridge_artifact(&expected));
        let mut observer = Observer::new(expected);
        observer.line(&event).unwrap();
        assert!(observer.line(&event).is_err());
    }
}
