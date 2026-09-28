//! Network and dataspace definition files (`specs/network_deployment.md` §3).
//!
//! A definition is the only hand-written deploy input. It is TOML read through
//! `iroha_config_base`: tables are `ReadConfig` sections, arrays of tables are
//! decoded as Norito JSON with unknown fields denied, and a failed read reports
//! every missing, unknown or mistyped key together with the file it came from.
//! A definition that reads cleanly is then validated as a whole, and every
//! rule violation is returned at once as an [`Issue`] naming its key.
//!
//! Relative paths are resolved against the directory of the definition file,
//! and a leading `~` is expanded to the operator's home directory. Paths that
//! name files on remote hosts (such as `edge.tls_certificate`) are not
//! resolved; they must be absolute.

mod dataspace;
mod hosts;
mod network;
mod value;

use std::{
    fmt::{self, Write as _},
    path::{Path, PathBuf},
};

use error_stack::Report;
use iroha_config_base::{
    attach::FilePath,
    read::{ConfigReader, FinalWrap, ReadConfig},
    toml::TomlSource,
};
use norito::json;

pub use self::{dataspace::*, hosts::*, network::*, value::*};

/// Error context of the `iroha_config_base` reader.
pub use iroha_config_base::read::Error as ReadError;

/// One validation rule a definition breaks, attributed to a key such as
/// `node[2].host_key`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    /// The offending key, with array indices.
    pub key: String,
    /// What is wrong and, where useful, how to fix it.
    pub message: String,
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.key, self.message)
    }
}

/// Why a definition could not be loaded.
#[derive(Debug, thiserror::Error)]
pub enum DefinitionError {
    /// The file could not be read or is not TOML, or keys are unknown, missing
    /// or mistyped. The report names the file and every offending key.
    #[error("cannot read definition `{}`:\n{report:?}", path.display())]
    Read {
        /// The definition file.
        path: PathBuf,
        /// The reader's report.
        report: Report<[ReadError]>,
    },
    /// The file reads cleanly but breaks validation rules.
    #[error("invalid definition `{}`:{}", path.display(), render_issues(issues))]
    Invalid {
        /// The definition file.
        path: PathBuf,
        /// Every broken rule.
        issues: Vec<Issue>,
    },
}

impl DefinitionError {
    /// The definition file the error is about.
    pub fn path(&self) -> &Path {
        match self {
            Self::Read { path, .. } | Self::Invalid { path, .. } => path,
        }
    }

    /// The validation issues; empty for read errors.
    pub fn issues(&self) -> &[Issue] {
        match self {
            Self::Read { .. } => &[],
            Self::Invalid { issues, .. } => issues,
        }
    }
}

fn render_issues(issues: &[Issue]) -> String {
    issues.iter().fold(String::new(), |mut out, issue| {
        let _ = write!(out, "\n  - {issue}");
        out
    })
}

/// Accumulates validation issues so that one pass reports all of them.
#[derive(Debug, Default)]
struct Issues(Vec<Issue>);

impl Issues {
    fn push(&mut self, key: impl Into<String>, message: impl Into<String>) {
        self.0.push(Issue {
            key: key.into(),
            message: message.into(),
        });
    }

    /// Record `error` under `key` if `result` failed; return the value otherwise.
    fn take<T, E: fmt::Display>(
        &mut self,
        key: impl Into<String>,
        result: Result<T, E>,
    ) -> Option<T> {
        result
            .map_err(|error| self.push(key, error.to_string()))
            .ok()
    }

    fn finish<T>(self, path: &Path, value: T) -> Result<T, DefinitionError> {
        if self.0.is_empty() {
            Ok(value)
        } else {
            Err(DefinitionError::Invalid {
                path: path.to_path_buf(),
                issues: self.0,
            })
        }
    }
}

/// Where a definition came from; resolves the paths written in it.
#[derive(Debug)]
struct Origin {
    /// The absolute definition file path.
    file: PathBuf,
    /// The operator's home directory, for `~` expansion.
    home: Option<PathBuf>,
}

impl Origin {
    fn new(file: &Path, home: Option<&Path>) -> Self {
        let file = std::path::absolute(file).unwrap_or_else(|_| file.to_path_buf());
        Self {
            file,
            home: home.map(Path::to_path_buf),
        }
    }

    fn base_dir(&self) -> &Path {
        self.file.parent().unwrap_or_else(|| Path::new("/"))
    }

    /// Resolve `path` in place, recording an issue under `key` on failure.
    fn resolve(&self, key: &str, path: &mut PathBuf, issues: &mut Issues) {
        if let Some(resolved) = issues.take(
            key,
            resolve_path(path, self.base_dir(), self.home.as_deref()),
        ) {
            *path = resolved;
        }
    }
}

/// Resolve a controller-side path written in a definition.
///
/// A leading `~` component expands to `home`, a relative path is joined to
/// `base_dir`, and an absolute path is kept.
///
/// # Errors
///
/// When the path is empty, uses `~user`, or needs `~` expansion without a home
/// directory.
pub fn resolve_path(
    path: &Path,
    base_dir: &Path,
    home: Option<&Path>,
) -> Result<PathBuf, ValueError> {
    if path.as_os_str().is_empty() {
        return Err(ValueError::new("path must not be empty"));
    }
    if let Ok(rest) = path.strip_prefix("~") {
        let home = home.ok_or_else(|| {
            ValueError::new(format!(
                "cannot expand `{}`: home directory unknown",
                path.display()
            ))
        })?;
        return Ok(if rest.as_os_str().is_empty() {
            home.to_path_buf()
        } else {
            home.join(rest)
        });
    }
    if path.to_str().is_some_and(|text| text.starts_with('~')) {
        return Err(ValueError::new(format!(
            "`{}`: only a leading `~/` is expanded",
            path.display()
        )));
    }
    Ok(base_dir.join(path))
}

/// Read a TOML file into a definition source.
fn source_from_file(path: &Path) -> Result<TomlSource, DefinitionError> {
    TomlSource::from_file(path).map_err(|report| DefinitionError::Read {
        path: path.to_path_buf(),
        report: report
            .attach(FilePath::new(path.to_path_buf()))
            .change_context(ReadError::ReadFile)
            .expand(),
    })
}

/// Parse TOML text into a definition source attributed to `path`.
fn source_from_str(text: &str, path: &Path) -> Result<TomlSource, DefinitionError> {
    text.parse::<toml::Table>()
        .map(|table| TomlSource::new(path.to_path_buf(), table))
        .map_err(|error| DefinitionError::Read {
            path: path.to_path_buf(),
            report: Report::new(error)
                .attach(FilePath::new(path.to_path_buf()))
                .change_context(ReadError::ReadFile)
                .expand(),
        })
}

/// Run the `iroha_config_base` reader over one source, without environment overlays.
fn read_source<T: ReadConfig>(source: TomlSource, path: &Path) -> Result<T, DefinitionError> {
    ConfigReader::new()
        .without_env()
        .with_toml_source(source)
        .read_and_complete::<T>()
        .map_err(|report| DefinitionError::Read {
            path: path.to_path_buf(),
            report,
        })
}

/// Read the optional table `section`; absent tables yield `None`.
fn read_optional<T: ReadConfig + 'static>(
    reader: &mut ConfigReader,
    section: &str,
) -> FinalWrap<Option<T>> {
    if reader.contains_toml_parameter([section]) {
        let value = reader.read_nested::<T>(section);
        FinalWrap::value_fn(move || Some(value.unwrap()))
    } else {
        FinalWrap::value_fn(|| None)
    }
}

/// A TOML array of tables whose decode errors name the failing entry index.
#[derive(Debug)]
struct Entries<T>(Vec<T>);

impl<T> Default for Entries<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T: json::JsonDeserialize> json::JsonDeserialize for Entries<T> {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        Vec::<T>::json_deserialize(parser).map(Self)
    }

    fn json_from_value(value: &json::Value) -> Result<Self, json::Error> {
        let items = value
            .as_array()
            .ok_or_else(|| json::Error::Message("expected an array of tables".to_owned()))?;
        items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                T::json_from_value(item).map_err(|error| {
                    let message = error.to_string();
                    let message = message.strip_prefix("JSON error: ").unwrap_or(&message);
                    json::Error::Message(format!("[{index}]: {message}"))
                })
            })
            .collect::<Result<_, _>>()
            .map(Self)
    }
}

/// Report every value that occurs more than once, under `key`.
fn check_unique<'a, T: Ord + fmt::Display + 'a>(
    key: &str,
    what: &str,
    values: impl IntoIterator<Item = &'a T>,
    issues: &mut Issues,
) {
    let mut seen = std::collections::BTreeSet::new();
    let mut reported = std::collections::BTreeSet::new();
    for value in values {
        if !seen.insert(value) && reported.insert(value) {
            issues.push(key, format!("duplicate {what} `{value}`"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_path_handles_home_relative_and_absolute() {
        let base = Path::new("/defs/networks");
        let home = Some(Path::new("/home/op"));
        assert_eq!(
            resolve_path(Path::new("~/.ssh/key"), base, home).unwrap(),
            PathBuf::from("/home/op/.ssh/key")
        );
        assert_eq!(
            resolve_path(Path::new("~"), base, home).unwrap(),
            PathBuf::from("/home/op")
        );
        assert_eq!(
            resolve_path(Path::new("../keys/a.key"), base, home).unwrap(),
            PathBuf::from("/defs/networks/../keys/a.key")
        );
        assert_eq!(
            resolve_path(Path::new("/etc/key"), base, home).unwrap(),
            PathBuf::from("/etc/key")
        );
        assert!(resolve_path(Path::new(""), base, home).is_err());
        assert!(resolve_path(Path::new("~other/key"), base, home).is_err());
        assert!(resolve_path(Path::new("~/key"), base, None).is_err());
    }

    #[test]
    fn origin_resolves_in_place_and_records_failures() {
        let origin = Origin::new(
            Path::new("/defs/networks/dev.toml"),
            Some(Path::new("/home/op")),
        );
        assert_eq!(origin.base_dir(), Path::new("/defs/networks"));
        let mut issues = Issues::default();
        let mut path = PathBuf::from("keys/admin.key");
        origin.resolve("network.admin_key", &mut path, &mut issues);
        assert_eq!(path, PathBuf::from("/defs/networks/keys/admin.key"));
        let mut bad = PathBuf::from("~root/x");
        origin.resolve("ssh.identity", &mut bad, &mut issues);
        assert_eq!(issues.0.len(), 1);
        assert_eq!(issues.0[0].key, "ssh.identity");
    }

    #[test]
    fn issues_finish_and_error_accessors() {
        let path = Path::new("/defs/x.toml");
        assert_eq!(Issues::default().finish(path, 7).unwrap(), 7);
        let mut issues = Issues::default();
        issues.push("node", "bad");
        assert_eq!(issues.take::<u8, _>("a", Err("worse")), None);
        assert_eq!(issues.take::<u8, &str>("b", Ok(1)), Some(1));
        let error = issues.finish(path, ()).unwrap_err();
        assert_eq!(error.path(), path);
        assert_eq!(error.issues().len(), 2);
        let rendered = error.to_string();
        assert!(rendered.contains("/defs/x.toml"));
        assert!(rendered.contains("  - node: bad"));
        assert!(rendered.contains("  - a: worse"));
    }

    #[test]
    fn unique_check_reports_each_duplicate_once() {
        let mut issues = Issues::default();
        check_unique("node", "name", &["a", "b", "a", "a"], &mut issues);
        assert_eq!(issues.0.len(), 1);
        assert_eq!(issues.0[0].to_string(), "node: duplicate name `a`");
    }

    #[test]
    fn toml_sources_report_the_file() {
        let path = Path::new("/defs/broken.toml");
        let error = source_from_str("[network\n", path).unwrap_err();
        assert!(error.issues().is_empty());
        assert!(error.to_string().contains("/defs/broken.toml"));
        let missing = source_from_file(Path::new("/nonexistent/iroha-deploy.toml")).unwrap_err();
        assert!(
            missing
                .to_string()
                .contains("/nonexistent/iroha-deploy.toml")
        );
        assert!(source_from_str("a = 1", path).is_ok());
    }

    #[test]
    fn entries_name_the_failing_index() {
        #[derive(Debug, norito::JsonDeserialize)]
        #[norito(deny_unknown_fields)]
        struct Entry {
            a: u8,
        }
        let value: json::Value = json::from_json(r#"[{"a": 1}, {"a": "x"}]"#).unwrap();
        let error = <Entries<Entry> as json::JsonDeserialize>::json_from_value(&value).unwrap_err();
        assert!(error.to_string().starts_with("[1]: "), "{error}");
        let ok: json::Value = json::from_json(r#"[{"a": 1}]"#).unwrap();
        let entries = <Entries<Entry> as json::JsonDeserialize>::json_from_value(&ok).unwrap();
        assert_eq!(entries.0.len(), 1);
        assert_eq!(entries.0[0].a, 1);
        assert!(Entries::<Entry>::default().0.is_empty());
    }
}
