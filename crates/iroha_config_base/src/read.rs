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

    /// Reads a TOML file and handles its `extends` field, implementing mixins mechanism.
    ///
    /// # Errors
    ///
    /// If files reading error occurs
    pub fn read_toml_with_extends<P: AsRef<Path>>(mut self, path: P) -> Result<Self, Error> {
        #[derive(Debug)]
        struct StackEntry {
            path: PathBuf,
            depth: u8,
            parent: Option<PathBuf>,
            source: Option<TomlSource>,
        }

        let result = (|| -> Result<(), Error> {
            let mut stack = vec![StackEntry {
                path: path.as_ref().to_path_buf(),
                depth: 0,
                parent: None,
                source: None,
            }];

            while let Some(StackEntry {
                path,
                depth,
                parent,
                mut source,
            }) = stack.pop()
            {
                let mut src = match source.take() {
                    Some(src) => src,
                    None => TomlSource::from_file(&path)
                        .attach_with(|| attach::FilePath::new(path.clone()))
                        .change_context(Error::ReadFile)
                        .map_err(|err| match &parent {
                            Some(parent_path) => err.attach(attach::ExtendsChain::new(
                                parent_path.clone(),
                                path.clone(),
                                depth,
                            )),
                            None => err,
                        })?,
                };

                let table = src.table_mut();

                if let Some(extends) = table.remove("extends") {
                    let parsed = ExtendsPaths::try_from(extends.clone())
                        .map_err(Report::new)
                        .attach_with(|| attach::Expected::new(r#"a single path ("./file.toml") or an array of paths (["a.toml", "b.toml", "c.toml"])"#))
                        .attach_with(|| attach::ActualValue::new(extends))
                        .change_context(Error::InvalidExtends)?;
                    log::trace!("found `extends`: {parsed:?}");

                    stack.push(StackEntry {
                        path: path.clone(),
                        depth,
                        parent: parent.clone(),
                        source: Some(src),
                    });

                    let mut paths = parsed.iter().collect::<Vec<_>>();
                    paths.reverse();
                    for extends_path in paths {
                        let full_path = path
                            .parent()
                            .expect("it cannot be root or empty")
                            .join(extends_path);
                        stack.push(StackEntry {
                            path: full_path,
                            depth: depth + 1,
                            parent: Some(path.clone()),
                            source: None,
                        });
                    }

                    continue;
                }

                self.sources.push(src);
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
    use super::*;

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
