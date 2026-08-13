//! Configuration reader API.
use std::{
    collections::{BTreeMap, BTreeSet},
    convert::identity,
    error::Error as StdError,
    fmt::{Debug, Write as _},
    path::{Path, PathBuf},
};
use drop_bomb::DropBomb;
use error_stack::{Report, ResultExt};
use norito::json::{self, JsonDeserializeOwned};
use thiserror::Error;
type Result<T, E> = core::result::Result<T, Report<[E]>>;
use crate::{
    ParameterId, ParameterOrigin, WithOrigin, attach,
    attach::{EnvValue, MissingParameter, UnknownParameter},
    env::{FromEnvStr, ReadEnv},
    toml::{self, TomlSource},
    util::{Emitter, ExtendsPaths},
};
const HEX_DIGITS: &[u8; 16] = b"0123456789ABCDEF";
/// Maximum number of `extends` edges plus the root source in one traversal.
///
/// Repeated diamond edges count toward this ceiling even though their already
/// visited target is loaded only once. This also bounds the explicit DFS stack.
pub const MAX_TOML_EXTENDS_SOURCES: usize = 64;
/// Maximum nesting depth of a TOML `extends` graph, with the root at depth zero.
pub const MAX_TOML_EXTENDS_DEPTH: u8 = 32;
/// Maximum aggregate encoded bytes across unique TOML sources in one traversal.
pub const MAX_TOML_EXTENDS_TOTAL_BYTES: u64 = 8 * toml::MAX_TOML_SOURCE_BYTES;
fn escape_json_string_plain(s: &str, out: &mut String) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str("\\u00");
                out.push(HEX_DIGITS[((c as u32 >> 4) & 0xF) as usize] as char);
                out.push(HEX_DIGITS[(c as u32 & 0xF) as usize] as char);
            }
            _ => out.push(ch),
        }
    }
    out.push('"');
}
fn serialize_json_value_plain(value: &json::Value, out: &mut String) {
    use norito::json::native::Number;
    match value {
        json::Value::Null => out.push_str("null"),
        json::Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        json::Value::Number(n) => match n {
            Number::I64(i) => out.push_str(&i.to_string()),
            Number::U64(u) => out.push_str(&u.to_string()),
            Number::F64(f) => {
                const F64_SAFE_INT: f64 = 9_007_199_254_740_992.0; // 2^53
                if f.is_finite() && f.fract() == 0.0 && f.abs() <= F64_SAFE_INT {
                    let _ = write!(out, "{f:.1}");
                } else {
                    let _ = write!(out, "{f:?}");
                }
            }
        },
        json::Value::String(s) => escape_json_string_plain(s, out),
        json::Value::Array(items) => {
            out.push('[');
            let mut iter = items.iter().peekable();
            while let Some(item) = iter.next() {
                serialize_json_value_plain(item, out);
                if iter.peek().is_some() {
                    out.push(',');
                }
            }
            out.push(']');
        }
        json::Value::Object(map) => {
            out.push('{');
            let mut iter = map.iter().peekable();
            while let Some((k, v)) = iter.next() {
                escape_json_string_plain(k, out);
                out.push(':');
                serialize_json_value_plain(v, out);
                if iter.peek().is_some() {
                    out.push(',');
                }
            }
            out.push('}');
        }
    }
}
fn deserialize_json_value_plain<T: json::JsonDeserialize>(
    value: &json::Value,
) -> std::result::Result<T, json::Error> {
    // Prefer the `Value`-aware path (avoids a string round-trip for many types).
    match json::from_value(value.clone()) {
        Ok(v) => Ok(v),
        Err(first_err) => {
            // Fallback to a minimal textual serialization to dodge any platform-specific
            // quirks in the fast `from_value` implementation.
            let mut buf = String::new();
            serialize_json_value_plain(value, &mut buf);
            match json::from_json(&buf) {
                Ok(v) => Ok(v),
                Err(_fallback_err) => Err(first_err),
            }
        }
    }
}
/// A type that implements reading from [`ConfigReader`]
pub trait ReadConfig: Sized {
    /// Returns the [`FinalWrap`] with self and the reader itself, transformed
    /// throughout the process of reading.
    ///
    /// The wrap is guaranteed to unwrap safely if the reader emits
    /// no error upon [`ConfigReader::into_result`].
    fn read(reader: &mut ConfigReader) -> FinalWrap<Self>;
}
/// An umbrella error for various cases related to [`ConfigReader`].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Failed to read configuration from file.
    #[error("Failed to read configuration from file")]
    ReadFile,
    /// The `extends` field is malformed or invalid.
    #[error("Invalid `extends` field")]
    InvalidExtends,
    /// Extending configuration files failed.
    #[error("Failed to extend configurations")]
    CannotExtend,
    /// Failed to parse a specific parameter.
    #[error("Failed to parse parameter `{0}`")]
    ParseParameter(ParameterId),
    /// Errors occurred while reading from a file.
    #[error("Errors occurred while reading from file: `{path}`", path = .0.display())]
    InSourceFile(PathBuf),
    /// Errors occurred while reading from environment variables.
    #[error("Errors occurred while reading from environment variables")]
    InEnvironment,
    /// Some required parameters are missing.
    #[error("Some required parameters are missing")]
    MissingParameters,
    /// Found unrecognised parameters that are not part of the schema.
    #[error("Found unrecognised parameters")]
    UnknownParameters,
    /// Other error with a descriptive message.
    #[error("{msg}")]
    Other {
        /// Explanatory message for the error variant.
        msg: String,
    },
}
#[derive(Debug, Error, Eq, PartialEq)]
enum ExtendsTraversalError {
    #[error("configuration `extends` cycle detected at `{}`", path.display())]
    Cycle { path: PathBuf },
    #[error("configuration `extends` depth {observed} exceeds the maximum {maximum}")]
    DepthLimit { observed: u8, maximum: u8 },
    #[error("configuration `extends` source references {observed} exceed the maximum {maximum}")]
    SourceLimit { observed: usize, maximum: usize },
    #[error("configuration `extends` encoded bytes {observed} exceed the maximum {maximum}")]
    ByteLimit { observed: u64, maximum: u64 },
}
#[derive(Debug)]
struct ExtendsTraversalBudget {
    source_references: usize,
    bytes_read: u64,
}
impl ExtendsTraversalBudget {
    fn new() -> Self {
        Self {
            source_references: 1,
            bytes_read: 0,
        }
    }
    fn check_depth(depth: u8) -> core::result::Result<(), ExtendsTraversalError> {
        if depth > MAX_TOML_EXTENDS_DEPTH {
            return Err(ExtendsTraversalError::DepthLimit {
                observed: depth,
                maximum: MAX_TOML_EXTENDS_DEPTH,
            });
        }
        Ok(())
    }
    fn schedule_sources(
        &mut self,
        additional: usize,
    ) -> core::result::Result<(), ExtendsTraversalError> {
        let observed = self.source_references.saturating_add(additional);
        if observed > MAX_TOML_EXTENDS_SOURCES {
            return Err(ExtendsTraversalError::SourceLimit {
                observed,
                maximum: MAX_TOML_EXTENDS_SOURCES,
            });
        }
        self.source_references = observed;
        Ok(())
    }
    fn record_bytes(&mut self, additional: u64) -> core::result::Result<(), ExtendsTraversalError> {
        let observed = self.bytes_read.saturating_add(additional);
        if observed > MAX_TOML_EXTENDS_TOTAL_BYTES {
            return Err(ExtendsTraversalError::ByteLimit {
                observed,
                maximum: MAX_TOML_EXTENDS_TOTAL_BYTES,
            });
        }
        self.bytes_read = observed;
        Ok(())
    }
    fn remaining_bytes(&self) -> u64 {
        MAX_TOML_EXTENDS_TOTAL_BYTES.saturating_sub(self.bytes_read)
    }
}
fn extends_traversal_report(error: ExtendsTraversalError) -> Report<Error> {
    Report::new(error).change_context(Error::CannotExtend)
}
fn extends_read_report(
    report: Report<toml::FromFileError>,
    path: &Path,
    parent: Option<&PathBuf>,
    depth: u8,
) -> Report<Error> {
    let report = report
        .attach(attach::FilePath::new(path.to_path_buf()))
        .change_context(Error::ReadFile);
    match parent {
        Some(parent_path) => report.attach(attach::ExtendsChain::new(
            parent_path.clone(),
            path.to_path_buf(),
            depth,
        )),
        None => report,
    }
}
fn read_toml_source_with_budget(
    path: &Path,
    parent: Option<&PathBuf>,
    depth: u8,
    expected_identity: &toml::RegularFileIdentity,
    budget: &mut ExtendsTraversalBudget,
) -> Result<TomlSource, Error> {
    let source_limit = toml::MAX_TOML_SOURCE_BYTES.min(budget.remaining_bytes());
    let (source, bytes_read, loaded_identity) =
        match TomlSource::from_file_with_limit(path, source_limit) {
            Ok(loaded) => loaded,
            Err(error) => {
                if let toml::FromFileError::TooLarge { actual, .. } = *error.current_context()
                    && source_limit < toml::MAX_TOML_SOURCE_BYTES
                {
                    return Err(extends_traversal_report(ExtendsTraversalError::ByteLimit {
                        observed: budget.bytes_read.saturating_add(actual),
                        maximum: MAX_TOML_EXTENDS_TOTAL_BYTES,
                    })
                    .into());
                }
                return Err(extends_read_report(error, path, parent, depth).into());
            }
        };
    if &loaded_identity != expected_identity {
        return Err(extends_read_report(
            Report::new(toml::FromFileError::ChangedWhileReading),
            path,
            parent,
            depth,
        )
        .into());
    }
    budget
        .record_bytes(bytes_read)
        .map_err(extends_traversal_report)?;
    Ok(source)
}
fn take_extends_paths(source: &mut TomlSource) -> Result<Vec<PathBuf>, Error> {
    let Some(extends) = source.table_mut().remove("extends") else {
        return Ok(Vec::new());
    };
    let parsed = ExtendsPaths::try_from(extends.clone())
        .map_err(Report::new)
        .attach_with(|| {
            attach::Expected::new(
                r#"a single path ("./file.toml") or an array of paths (["a.toml", "b.toml", "c.toml"])"#,
            )
        })
        .attach_with(|| attach::ActualValue::new(extends))
        .change_context(Error::InvalidExtends)?;
    log::trace!("found `extends`: {parsed:?}");
    Ok(match parsed {
        ExtendsPaths::Single(path) => vec![path],
        ExtendsPaths::Chain(paths) => paths,
    })
}
#[derive(Error, Debug)]
#[error("{0}")]
struct EnvError(String);
#[derive(Error, Debug)]
#[error("failed to deserialize config value: {message}")]
struct JsonValueError {
    message: String,
}
fn normalize_json_error_message(raw: &str) -> String {
    raw.strip_prefix("JSON error: ").unwrap_or(raw).to_string()
}
impl From<norito::json::Error> for JsonValueError {
    fn from(error: norito::json::Error) -> Self {
        let message = error.to_string();
        Self {
            message: normalize_json_error_message(&message),
        }
    }
}
impl From<Report<norito::json::Error>> for JsonValueError {
    fn from(report: Report<norito::json::Error>) -> Self {
        let message = report.to_string();
        Self {
            message: normalize_json_error_message(&message),
        }
    }
}
impl Error {
    /// Some other error message
    pub fn other(message: impl AsRef<str>) -> Self {
        Self::Other {
            msg: message.as_ref().to_string(),
        }
    }
}
#[expect(clippy::too_long_first_doc_paragraph)]
/// The reader, which provides an API to accumulate config sources,
/// read parameters from them, override with environment variables, fallback to default values,
/// and finally, construct an exhaustive error report with as many errors, accumulated along the
/// way, as possible.
pub struct ConfigReader {
    /// The namespace this [`ConfigReader`] is handling. All the `ParameterId` handled will be prefixed with it.
    nesting: Vec<String>,
    /// File sources for the config
    sources: Vec<TomlSource>,
    /// Environment variables source for the config
    env: Box<dyn ReadEnv>,
    /// Errors accumulated per each file
    errors_by_source: BTreeMap<PathBuf, Vec<Report<Error>>>,
    /// Errors accumulated from the environment variables
    errors_in_env: Vec<Report<EnvError>>,
    /// A list of all the parameters that have been requested from this reader. Used to report unused (unknown) parameters in the toml file
    existing_parameters: BTreeSet<ParameterId>,
    /// A list of all required parameters that have been requested, but were not found
    missing_parameters: BTreeSet<ParameterId>,
    /// A runtime guard to prevent dropping the [`ConfigReader`] without handing errors
    bomb: DropBomb,
}
impl Debug for ConfigReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ConfigReader")
    }
}
impl Default for ConfigReader {
    fn default() -> Self {
        Self::new()
    }
}
impl ConfigReader {
    /// Constructor
    pub fn new() -> Self {
        Self {
            sources: <_>::default(),
            nesting: <_>::default(),
            errors_by_source: <_>::default(),
            errors_in_env: <_>::default(),
            existing_parameters: <_>::default(),
            missing_parameters: <_>::default(),
            bomb: DropBomb::new("forgot to call `ConfigReader::finish()`, didn't you?"),
            env: Box::new(crate::env::std_env),
        }
    }
    /// Replace default environment reader ([`std::env::var`]) with a custom one
    #[must_use]
    pub fn with_env(mut self, env: impl ReadEnv + 'static) -> Self {
        self.env = Box::new(env);
        self
    }
    /// Add a data source to read parameters from.
    #[must_use]
    pub fn with_toml_source(mut self, source: TomlSource) -> Self {
        self.sources.push(source);
        self
    }
    /// Rewrite the ordered TOML sources before typed parameter deserialization.
    ///
    /// Sources are provided in resolution order: later entries have higher
    /// precedence. This hook is intended for schema-owned canonicalization of
    /// disabled optional subtrees, where dormant values must be removed before
    /// type and unknown-parameter validation run.
    #[must_use]
    pub fn rewrite_toml_sources(mut self, rewrite: impl FnOnce(&mut [TomlSource])) -> Self {
        rewrite(&mut self.sources);
        self
    }
    /// Return whether any loaded TOML source explicitly defines `id`.
    ///
    /// This distinguishes an operator-provided value from a value materialized
    /// by [`ReadConfig`] defaults. Sources loaded through
    /// [`Self::read_toml_with_extends`] are included.
    #[must_use]
    pub fn contains_toml_parameter(&self, id: impl Into<ParameterId>) -> bool {
        let id = self.full_id(id);
        self.sources
            .iter()
            .any(|source| source.fetch(&id).is_some())
    }
    /// Reads a TOML file and handles its `extends` field, implementing mixins mechanism.
    ///
    /// The traversal is depth-first in declared order. Canonically identical
    /// files reached through a diamond are applied once, while an edge back to
    /// an active source is rejected as a cycle. The traversal is bounded by
    /// [`MAX_TOML_EXTENDS_DEPTH`], [`MAX_TOML_EXTENDS_SOURCES`],
    /// [`MAX_TOML_EXTENDS_TOTAL_BYTES`], and
    /// [`toml::MAX_TOML_SOURCE_BYTES`]. Source paths must name stable regular
    /// files; symbolic links are rejected.
    ///
    /// # Errors
    ///
    /// If a source cannot be read or parsed, a cycle is found, or a traversal
    /// or encoded-byte ceiling is exceeded.
    pub fn read_toml_with_extends<P: AsRef<Path>>(mut self, path: P) -> Result<Self, Error> {
        #[derive(Debug)]
        enum StackEntry {
            Visit {
                path: PathBuf,
                depth: u8,
                parent: Option<PathBuf>,
            },
            Emit {
                identity: toml::RegularFileIdentity,
                source: TomlSource,
            },
        }
        let result = (|| -> Result<(), Error> {
            let mut stack = vec![StackEntry::Visit {
                path: path.as_ref().to_path_buf(),
                depth: 0,
                parent: None,
            }];
            let mut active = BTreeSet::new();
            let mut visited = BTreeSet::new();
            let mut budget = ExtendsTraversalBudget::new();
            while let Some(entry) = stack.pop() {
                let (path, depth, parent) = match entry {
                    StackEntry::Visit {
                        path,
                        depth,
                        parent,
                    } => (path, depth, parent),
                    StackEntry::Emit { identity, source } => {
                        let removed = active.remove(&identity);
                        debug_assert!(removed, "emitted TOML source must be active");
                        visited.insert(identity);
                        self.sources.push(source);
                        continue;
                    }
                };
                ExtendsTraversalBudget::check_depth(depth).map_err(extends_traversal_report)?;
                let (identity, canonical_path) = toml::canonical_regular_file_identity(&path)
                    .map_err(|error| extends_read_report(error, &path, parent.as_ref(), depth))?;
                if active.contains(&identity) {
                    return Err(extends_traversal_report(ExtendsTraversalError::Cycle {
                        path: canonical_path,
                    })
                    .into());
                }
                if visited.contains(&identity) {
                    continue;
                }
                let mut source = read_toml_source_with_budget(
                    &path,
                    parent.as_ref(),
                    depth,
                    &identity,
                    &mut budget,
                )?;
                let extends_paths = take_extends_paths(&mut source)?;
                budget
                    .schedule_sources(extends_paths.len())
                    .map_err(extends_traversal_report)?;
                active.insert(identity.clone());
                stack.push(StackEntry::Emit { identity, source });
                let child_depth = depth.saturating_add(1);
                let parent_dir = path.parent().unwrap_or_else(|| Path::new(""));
                for extends_path in extends_paths.into_iter().rev() {
                    stack.push(StackEntry::Visit {
                        path: parent_dir.join(extends_path),
                        depth: child_depth,
                        parent: Some(path.clone()),
                    });
                }
            }
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.bomb.defuse();
                Ok(self)
            }
            Err(e) => {
                self.bomb.defuse();
                Err(e)
            }
        }
    }
    /// Instantiate a parameter reading pipeline.
    #[must_use]
    pub fn read_parameter<T>(&mut self, id: impl Into<ParameterId>) -> ReadingParameter<'_, T>
    where
        T: JsonDeserializeOwned,
    {
        let id = self.full_id(id);
        self.collect_parameter(&id);
        ReadingParameter::new(self, id).fetch()
    }
    /// Delegate reading to another implementor of [`ReadConfig`] under a certain namespace.
    /// All parameter IDs in it will be resolved within that namespace.
    #[must_use]
    pub fn read_nested<T: ReadConfig>(&mut self, namespace: impl AsRef<str>) -> FinalWrap<T> {
        self.nesting.push(namespace.as_ref().to_string());
        let value = T::read(self);
        self.nesting.pop();
        value
    }
    /// Finally, complete the reading procedure and emit a collective report
    /// in case if any error occurred along the reading process.
    ///
    /// # Errors
    /// If any occurred while reading of data.
    pub fn into_result(mut self) -> Result<(), Error> {
        self.bomb.defuse();
        let mut emitter = Emitter::new();
        if !self.missing_parameters.is_empty() {
            let mut report = Report::new(Error::MissingParameters);
            for i in self.missing_parameters {
                report = report.attach(MissingParameter::new(i));
            }
            emitter.emit(report);
        }
        // looking for unknown parameters
        for source in &self.sources {
            let unknown_parameters = source.find_unknown(self.existing_parameters.iter());
            if !unknown_parameters.is_empty() {
                let mut report = Report::new(Error::UnknownParameters);
                for i in unknown_parameters {
                    report = report.attach(UnknownParameter::new(i));
                }
                self.errors_by_source
                    .entry(source.path().clone())
                    .or_default()
                    .push(report);
            }
        }
        // emit reports by source
        for (source, reports) in self.errors_by_source {
            let mut local_emitter = Emitter::new();
            for report in reports {
                local_emitter.emit(report);
            }
            let report = local_emitter
                .into_result()
                .expect_err("there should be at least one error");
            emitter.emit(report.change_context(Error::InSourceFile(source)))
        }
        // environment parsing errors
        if !self.errors_in_env.is_empty() {
            let mut local_emitter = Emitter::new();
            for report in self.errors_in_env {
                local_emitter.emit(report);
            }
            let report = local_emitter
                .into_result()
                .expect_err("there should be at least one error");
            emitter.emit(report.change_context(Error::InEnvironment));
        }
        emitter.into_result()
    }
    /// A shorthand to "just read the config and get an error or the value".
    /// # Errors
    /// See [`Self::into_result`]
    pub fn read_and_complete<T: ReadConfig>(mut self) -> Result<T, Error> {
        let value = T::read(&mut self);
        self.into_result()?;
        Ok(value.unwrap())
    }
    fn full_id(&self, id: impl Into<ParameterId>) -> ParameterId {
        self.nesting.iter().chain(id.into().segments.iter()).into()
    }
    fn collect_deserialize_error<C: StdError + Send + Sync + 'static>(
        &mut self,
        source_path: PathBuf,
        path: &ParameterId,
        report: Report<C>,
    ) {
        self.errors_by_source
            .entry(source_path)
            .or_default()
            .push(report.change_context(Error::ParseParameter(path.clone())));
    }
    fn collect_env_error(&mut self, report: Report<EnvError>) {
        self.errors_in_env.push(report)
    }
    fn collect_parameter(&mut self, id: &ParameterId) {
        self.existing_parameters.insert(id.clone());
    }
    fn collect_missing_parameter(&mut self, id: &ParameterId) {
        self.missing_parameters.insert(id.clone());
    }
    fn fetch_parameter<T>(
        &mut self,
        id: &ParameterId,
    ) -> std::result::Result<Option<WithOrigin<T>>, ()>
    where
        T: JsonDeserializeOwned,
    {
        self.collect_parameter(id);
        let mut errored = false;
        let mut value = None;
        let mut errors: Vec<(PathBuf, Report<JsonValueError>)> = Vec::new();
        for source in &self.sources {
            if let Some(toml_value) = source.fetch(id) {
                let source_path = source.path().clone();
                let printable = toml_value.to_string();
                let json_value = match toml::value_to_json(toml_value) {
                    Ok(value) => value,
                    Err(error) => {
                        errored = true;
                        value = None;
                        errors.push((
                            source_path.clone(),
                            Report::new(JsonValueError::from(error))
                                .attach(attach::ConfigValue::new(printable.clone())),
                        ));
                        continue;
                    }
                };
                let result: std::result::Result<T, _> = deserialize_json_value_plain(&json_value);
                match (result, errored) {
                    (Ok(v), false) => {
                        if value.is_none() {
                            log::trace!("parameter `{id}`: found in `{}`", source_path.display());
                        } else {
                            log::trace!(
                                "parameter `{id}`: found in `{}`, overwriting previous value",
                                source_path.display()
                            );
                        }
                        value = Some(WithOrigin::new(
                            v,
                            ParameterOrigin::file(id.clone(), source_path.clone()),
                        ));
                    }
                    // we don't care if there was an error before
                    (Ok(_), true) => {}
                    (Err(error), _) => {
                        errored = true;
                        value = None;
                        errors.push((
                            source_path.clone(),
                            Report::new(JsonValueError::from(error))
                                .attach(attach::ConfigValue::new(printable.clone())),
                        ));
                    }
                }
            } else {
                log::trace!(
                    "parameter `{id}`: not found in `{}`",
                    source.path().display()
                )
            }
        }
        for (source_path, report) in errors {
            self.collect_deserialize_error(source_path, id, report);
        }
        if errored { Err(()) } else { Ok(value) }
    }
}
/// A state of reading a certain configuration parameter.
pub struct ReadingParameter<'reader, T> {
    reader: &'reader mut ConfigReader,
    id: ParameterId,
    value: Option<WithOrigin<T>>,
    errored: bool,
}
impl<'reader, T> ReadingParameter<'reader, T> {
    fn new(reader: &'reader mut ConfigReader, id: ParameterId) -> Self {
        Self {
            reader,
            id,
            value: None,
            errored: false,
        }
    }
}
impl<T> ReadingParameter<'_, T>
where
    T: JsonDeserializeOwned,
{
    #[must_use]
    fn fetch(mut self) -> Self {
        match self.reader.fetch_parameter(&self.id) {
            Ok(value) => {
                self.value = value;
            }
            Err(()) => {
                self.errored = true;
            }
        }
        self
    }
}
impl<T> ReadingParameter<'_, T>
where
    T: FromEnvStr,
{
    /// Reads an environment variable and parses the value which is [`FromEnvStr`].
    #[must_use]
    pub fn env(mut self, var: impl AsRef<str>) -> Self {
        let var = var.as_ref();
        if let Some(raw_str) = self.reader.env.read_env(var) {
            match (T::from_env_str(raw_str.clone()), self.errored) {
                (Err(error), _) => {
                    self.errored = true;
                    self.reader.collect_env_error(
                        Report::new(error)
                            .attach(EnvValue::new(var.to_string(), raw_str.into_owned()))
                            .change_context(EnvError(format!(
                                "Failed to parse parameter `{}` from `{var}`",
                                self.id,
                            ))),
                    );
                }
                (Ok(value), false) => {
                    if self.value.is_none() {
                        log::trace!("parameter `{}`: found `{var}` env var", self.id,);
                    } else {
                        log::trace!(
                            "parameter `{}`: found `{var}` env var, overwriting previous value",
                            self.id,
                        );
                    }
                    self.value = Some(WithOrigin::new(
                        value,
                        ParameterOrigin::env(self.id.clone(), var.to_string()),
                    ));
                }
                (Ok(_ignore), true) => {
                    log::trace!(
                        "parameter `{}`: env var `{var}` found, ignore due to previous errors",
                        self.id,
                    );
                }
            }
        } else {
            log::trace!("parameter `{}`: env var `{var}` not found", self.id)
        }
        self
    }
}
impl<T> ReadingParameter<'_, T> {
    /// Finish reading, and if the value is not read so far, it will be reported later on [`ConfigReader::into_result`].
    #[must_use]
    pub fn value_required(self) -> ReadingDone<T> {
        match (self.errored, self.value) {
            (false, Some(value)) => ReadingDone(ReadingDoneValue::Fine(value)),
            (false, None) => {
                self.reader.collect_missing_parameter(&self.id);
                ReadingDone(ReadingDoneValue::Errored)
            }
            (true, _) => ReadingDone(ReadingDoneValue::Errored),
        }
    }
    /// Finish reading, falling back to a default value if it is absent
    #[must_use]
    pub fn value_or_else<F: FnOnce() -> T>(self, fun: F) -> ReadingDone<T> {
        match (self.errored, self.value) {
            (false, Some(value)) => ReadingDone(ReadingDoneValue::Fine(value)),
            (false, None) => {
                log::trace!("parameter `{}`: fallback to default value", self.id);
                ReadingDone(ReadingDoneValue::Fine(WithOrigin::new(
                    fun(),
                    ParameterOrigin::default(self.id.clone()),
                )))
            }
            (true, _) => ReadingDone(ReadingDoneValue::Errored),
        }
    }
    /// Finish reading, allowing value to be not present
    #[must_use]
    pub fn value_optional(self) -> OptionReadingDone<T> {
        match (self.errored, self.value) {
            (false, value) => OptionReadingDone(ReadingDoneValue::Fine(value)),
            (true, _) => OptionReadingDone(ReadingDoneValue::Errored),
        }
    }
}
// Lifetime is elided intentionally (`'_`) to avoid triggering the
// `single-use-lifetimes` lint while still binding the impl to the reader borrow.
impl<T: Default> ReadingParameter<'_, T> {
    /// Equivalent of [`ReadingParameter::value_or_else`] with [`Default::default`].
    #[must_use]
    pub fn value_or_default(self) -> ReadingDone<T> {
        self.value_or_else(Default::default)
    }
}
enum ReadingDoneValue<T> {
    Errored,
    Fine(T),
}
impl<T> ReadingDoneValue<T> {
    fn into_final(self) -> FinalWrap<T> {
        self.into_final_with(identity)
    }
    fn into_final_with<F, U>(self, f: F) -> FinalWrap<U>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            Self::Errored => FinalWrap(FinalWrapInner::Errored),
            Self::Fine(t) => FinalWrap(FinalWrapInner::Value(f(t))),
        }
    }
}
/// A state of reading when the parameter's value is read, and the next step is to finish it via
/// [`ReadingDone::finish`] or [`ReadingDone::finish_with_origin`]
pub struct ReadingDone<T>(ReadingDoneValue<WithOrigin<T>>);
/// Same as [`ReadingDone`], but holding an optional value.
pub struct OptionReadingDone<T>(ReadingDoneValue<Option<WithOrigin<T>>>);
impl<T> ReadingDone<T> {
    /// Finish with the value only.
    #[must_use]
    pub fn finish(self) -> FinalWrap<T> {
        self.0.into_final_with(WithOrigin::into_value)
    }
    /// Finish with the value and its origin
    #[must_use]
    pub fn finish_with_origin(self) -> FinalWrap<WithOrigin<T>> {
        self.0.into_final()
    }
}
impl<T> OptionReadingDone<T> {
    /// Finish with the value only
    #[must_use]
    pub fn finish(self) -> FinalWrap<Option<T>> {
        self.0.into_final_with(|x| x.map(WithOrigin::into_value))
    }
    /// Finish with the value and its origin
    #[must_use]
    pub fn finish_with_origin(self) -> FinalWrap<Option<WithOrigin<T>>> {
        self.0.into_final()
    }
}
/// A value that should be accessed only if overall configuration reading succeeded.
///
/// I.e. it is guaranteed that [`FinalWrap::unwrap`] will not panic after associated
/// [`ConfigReader::into_result`] returns [`Ok`].
/// Wrapper that yields a value only if overall configuration reading succeeds.
///
/// It defers actual computation or unwrap until the caller verifies that
/// `ConfigReader::into_result` returned `Ok`, preventing premature panics while
/// aggregating configuration errors.
pub struct FinalWrap<T>(FinalWrapInner<T>);
/// Exists to not expose enum variants if they were in [`FinalWrap`]
enum FinalWrapInner<T> {
    Errored,
    Value(T),
    ValueFn(Box<dyn FnOnce() -> T>),
}
impl<T> FinalWrap<T> {
    /// Pass a closure that will emit the value on [`Self::unwrap`].
    pub fn value_fn<F>(fun: F) -> Self
    where
        F: FnOnce() -> T + 'static,
    {
        Self(FinalWrapInner::ValueFn(Box::new(fun)))
    }
    /// Unwrap the value inside.
    ///
    /// Can be safely called only after the [`ConfigReader::into_result`] returned [Ok].
    ///
    /// # Panics
    /// Might panic if an error occurred while reading of this certain value.
    pub fn unwrap(self) -> T {
        match self.0 {
            FinalWrapInner::Errored => panic!(
                "`FinalWrap::unwrap` is supposed to be called only after `ConfigReader::into_result` returns OK; it is probably a bug"
            ),
            FinalWrapInner::Value(value) => value,
            FinalWrapInner::ValueFn(fun) => fun(),
        }
    }
}
#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };
    use super::*;
    static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);
    fn temp_config_dir(label: &str) -> PathBuf {
        let nonce = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "iroha_config_base_{label}_{}_{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temporary configuration directory");
        path
    }
    fn report_debug(report: &Report<[Error]>) -> String {
        format!("{report:#?}")
    }
    #[test]
    fn extends_cycle_is_rejected_before_reloading_active_source() {
        let dir = temp_config_dir("cycle");
        let config = dir.join("config.toml");
        fs::write(&config, "extends = \"./config.toml\"\n").expect("write cyclic configuration");
        let report = ConfigReader::new()
            .read_toml_with_extends(&config)
            .expect_err("self-cycle must be rejected");
        let rendered = report_debug(&report);
        assert!(
            rendered.contains("configuration `extends` cycle detected"),
            "unexpected report: {rendered}"
        );
        fs::remove_dir_all(dir).expect("remove temporary configuration directory");
    }
    #[test]
    fn extends_diamond_loads_canonical_source_once_in_declared_order() {
        let dir = temp_config_dir("diamond");
        fs::write(dir.join("base.toml"), "value = \"base\"\n").expect("write base configuration");
        fs::write(
            dir.join("left.toml"),
            "extends = \"base.toml\"\nvalue = \"left\"\n",
        )
        .expect("write left configuration");
        fs::write(
            dir.join("right.toml"),
            "extends = \"./base.toml\"\nvalue = \"right\"\n",
        )
        .expect("write right configuration");
        fs::write(
            dir.join("top.toml"),
            "extends = [\"left.toml\", \"right.toml\"]\n",
        )
        .expect("write top configuration");
        let mut reader = ConfigReader::new()
            .read_toml_with_extends(dir.join("top.toml"))
            .expect("diamond must load");
        let source_names: Vec<_> = reader
            .sources
            .iter()
            .map(|source| {
                source
                    .path()
                    .file_name()
                    .expect("source has a file name")
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(
            source_names,
            ["base.toml", "left.toml", "right.toml", "top.toml"]
        );
        let value = reader
            .read_parameter::<String>(["value"])
            .value_required()
            .finish();
        reader.into_result().expect("diamond config is valid");
        assert_eq!(value.unwrap(), "right");
        fs::remove_dir_all(dir).expect("remove temporary configuration directory");
    }
    #[test]
    fn extends_source_reference_ceiling_is_checked_before_children_are_opened() {
        let dir = temp_config_dir("source_limit");
        let children = (0..MAX_TOML_EXTENDS_SOURCES)
            .map(|index| format!("\"missing-{index}.toml\""))
            .collect::<Vec<_>>()
            .join(", ");
        fs::write(dir.join("root.toml"), format!("extends = [{children}]\n"))
            .expect("write excessive source fanout");
        let report = ConfigReader::new()
            .read_toml_with_extends(dir.join("root.toml"))
            .expect_err("excessive source fanout must fail");
        let rendered = report_debug(&report);
        let expected = format!(
            "source references {} exceed the maximum {}",
            MAX_TOML_EXTENDS_SOURCES + 1,
            MAX_TOML_EXTENDS_SOURCES
        );
        assert!(
            rendered.contains(&expected),
            "unexpected report: {rendered}"
        );
        fs::remove_dir_all(dir).expect("remove temporary configuration directory");
    }
    #[test]
    fn extends_depth_ceiling_is_enforced_on_the_first_excess_edge() {
        let dir = temp_config_dir("depth_limit");
        for depth in 0..=u16::from(MAX_TOML_EXTENDS_DEPTH) + 1 {
            let body = if depth <= u16::from(MAX_TOML_EXTENDS_DEPTH) {
                format!("extends = \"{}.toml\"\n", depth + 1)
            } else {
                String::new()
            };
            fs::write(dir.join(format!("{depth}.toml")), body)
                .expect("write deep configuration chain");
        }
        let report = ConfigReader::new()
            .read_toml_with_extends(dir.join("0.toml"))
            .expect_err("excessive depth must fail");
        let rendered = report_debug(&report);
        let expected = format!(
            "depth {} exceeds the maximum {}",
            MAX_TOML_EXTENDS_DEPTH + 1,
            MAX_TOML_EXTENDS_DEPTH
        );
        assert!(
            rendered.contains(&expected),
            "unexpected report: {rendered}"
        );
        fs::remove_dir_all(dir).expect("remove temporary configuration directory");
    }
    #[test]
    fn extends_traversal_budget_enforces_exact_boundaries() {
        ExtendsTraversalBudget::check_depth(MAX_TOML_EXTENDS_DEPTH)
            .expect("maximum depth is admitted");
        assert_eq!(
            ExtendsTraversalBudget::check_depth(MAX_TOML_EXTENDS_DEPTH + 1),
            Err(ExtendsTraversalError::DepthLimit {
                observed: MAX_TOML_EXTENDS_DEPTH + 1,
                maximum: MAX_TOML_EXTENDS_DEPTH,
            })
        );
        let mut sources = ExtendsTraversalBudget::new();
        sources
            .schedule_sources(MAX_TOML_EXTENDS_SOURCES - 1)
            .expect("exact source-reference ceiling is admitted");
        assert_eq!(
            sources.schedule_sources(1),
            Err(ExtendsTraversalError::SourceLimit {
                observed: MAX_TOML_EXTENDS_SOURCES + 1,
                maximum: MAX_TOML_EXTENDS_SOURCES,
            })
        );
        let mut bytes = ExtendsTraversalBudget::new();
        bytes
            .record_bytes(MAX_TOML_EXTENDS_TOTAL_BYTES)
            .expect("exact aggregate byte ceiling is admitted");
        assert_eq!(
            bytes.record_bytes(1),
            Err(ExtendsTraversalError::ByteLimit {
                observed: MAX_TOML_EXTENDS_TOTAL_BYTES + 1,
                maximum: MAX_TOML_EXTENDS_TOTAL_BYTES,
            })
        );
    }
    #[test]
    fn detects_explicit_toml_parameter_before_defaults_are_read() {
        let mut reader = ConfigReader::new().with_toml_source(TomlSource::inline(::toml::toml! {
            [sorafs.storage]
            enabled = false
        }));
        assert!(reader.contains_toml_parameter(["sorafs", "storage", "enabled"]));
        assert!(!reader.contains_toml_parameter(["sorafs", "storage", "missing"]));
        let enabled = reader
            .read_parameter::<bool>(["sorafs", "storage", "enabled"])
            .value_required()
            .finish();
        reader.into_result().expect("source must be fully consumed");
        assert!(!enabled.unwrap());
    }
    #[test]
    fn trims_json_error_prefix_in_messages() {
        let base_err = norito::json::Error::TrailingCharacters {
            byte: 1,
            line: 1,
            col: 2,
        };
        let simple = JsonValueError::from(base_err.clone());
        assert_eq!(
            simple.message,
            "trailing characters at byte 1 (line 1, col 2)"
        );
        let report = Report::new(base_err);
        let reported = JsonValueError::from(report);
        assert_eq!(
            reported.message,
            "trailing characters at byte 1 (line 1, col 2)"
        );
    }
    #[test]
    fn plain_serializer_matches_roundtrip() {
        use norito::json::{self, Value, native::Number};
        let values = [
            Value::String("00000000-0000-0000-0000-000000000000".to_owned()),
            Value::String("addr:127.0.0.1:33337#D694".to_owned()),
            Value::String(
                "ed01204164BF554923ECE1FD412D241036D863A6AE430476C898248B8237D77534CFC4".to_owned(),
            ),
            Value::Array(vec![
                Value::String("peer@addr:127.0.0.1:1337#FFFF".to_owned()),
                Value::String("peer2@addr:127.0.0.1:1338#EEEE".to_owned()),
            ]),
            Value::Object({
                let mut m = json::native::Map::new();
                m.insert("pop_hex".into(), Value::String("deadbeef".into()));
                m.insert("public_key".into(), Value::String("ea0130".into()));
                m
            }),
            Value::Number(Number::U64(42)),
        ];
        for value in values {
            let mut plain = String::new();
            serialize_json_value_plain(&value, &mut plain);
            let parsed = json::parse_value(&plain).expect("plain serialized value parses");
            assert_eq!(parsed, value, "mismatch for plain serializer");
            let canonical = json::to_json(&value).expect("canonical to_json");
            let reparsed =
                json::parse_value(&canonical).expect("canonical serialized value parses");
            assert_eq!(reparsed, value, "mismatch for canonical serializer");
        }
    }
}
