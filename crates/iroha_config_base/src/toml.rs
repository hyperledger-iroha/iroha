//! TOML-specific tools.
//!
//! While it is definitely possible to support other formats than TOML, since there is no
//! need for this for now, TOML support is integrated in a non-generic way.
use std::{
    collections::{BTreeMap, BTreeSet},
    convert::TryFrom,
    fs::{self, File, Metadata},
    io::Read,
    path::{Path, PathBuf},
};
use error_stack::{Report, ResultExt};
use norito::json::{self, Map as JsonMap, Number as JsonNumber, Value as JsonValue};
use thiserror::Error;
use toml::Table;
type Result<T, E> = core::result::Result<T, Report<E>>;
use crate::ParameterId;
/// Maximum encoded size of one TOML configuration source.
///
/// This first-release ceiling bounds allocation before TOML parsing. Files are
/// read through a `limit + 1` reader so growth after the metadata check also
/// fails closed.
pub const MAX_TOML_SOURCE_BYTES: u64 = 1024 * 1024;
/// A source of configuration in TOML format
#[derive(Debug, Clone)]
pub struct TomlSource {
    path: PathBuf,
    table: Table,
}
/// Error of [`TomlSource::from_file`]
#[derive(Error, Debug, Copy, Clone, Eq, PartialEq)]
pub enum FromFileError {
    /// File system error while opening or reading the file.
    #[error("File system error")]
    Read,
    /// The source path is not a regular file or is itself a symbolic link.
    #[error("Configuration source is not a regular file")]
    NotRegularFile,
    /// The source exceeds the caller's encoded byte ceiling.
    #[error("Configuration source is {actual} bytes, exceeding the {limit}-byte limit")]
    TooLarge {
        /// Observed encoded source size.
        actual: u64,
        /// Maximum permitted encoded source size.
        limit: u64,
    },
    /// The source path or file contents changed while it was being read.
    #[error("Configuration source changed while being read")]
    ChangedWhileReading,
    /// Error while deserializing file contents as TOML.
    #[error("Error while deserializing file contents as TOML")]
    Parse,
}
#[cfg(unix)]
fn metadata_same_file(left: &Metadata, right: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev() && left.ino() == right.ino()
}
#[cfg(not(unix))]
fn metadata_same_file(left: &Metadata, right: &Metadata) -> bool {
    left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
        && left.created().ok() == right.created().ok()
}
#[cfg(unix)]
fn metadata_same_snapshot(left: &Metadata, right: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    metadata_same_file(left, right)
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}
#[cfg(not(unix))]
fn metadata_same_snapshot(left: &Metadata, right: &Metadata) -> bool {
    metadata_same_file(left, right)
}
fn regular_file_metadata(path: &Path) -> Result<Metadata, FromFileError> {
    let metadata = fs::symlink_metadata(path).change_context(FromFileError::Read)?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(Report::new(FromFileError::NotRegularFile));
    }
    Ok(metadata)
}
/// Stable identity of a regular TOML source on Unix.
#[cfg(unix)]
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct RegularFileIdentity {
    device: u64,
    inode: u64,
}
/// Stable canonical identity of a regular TOML source.
#[cfg(not(unix))]
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct RegularFileIdentity {
    canonical_path: PathBuf,
}
#[cfg(unix)]
fn regular_file_identity(metadata: &Metadata, _canonical_path: PathBuf) -> RegularFileIdentity {
    use std::os::unix::fs::MetadataExt as _;
    RegularFileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    }
}
#[cfg(not(unix))]
fn regular_file_identity(_metadata: &Metadata, canonical_path: PathBuf) -> RegularFileIdentity {
    RegularFileIdentity { canonical_path }
}
/// Inspect a source without following a final symlink and return its stable identity.
pub(crate) fn canonical_regular_file_identity(
    path: &Path,
) -> Result<(RegularFileIdentity, PathBuf), FromFileError> {
    let metadata = regular_file_metadata(path)?;
    let canonical_path = fs::canonicalize(path).change_context(FromFileError::Read)?;
    let identity = regular_file_identity(&metadata, canonical_path.clone());
    Ok((identity, canonical_path))
}
impl TomlSource {
    /// Constructor
    pub fn new(path: PathBuf, table: Table) -> Self {
        Self { path, table }
    }
    /// Read from a file
    ///
    /// The path must name a stable regular file (symbolic links are rejected),
    /// and its encoded size must not exceed [`MAX_TOML_SOURCE_BYTES`].
    ///
    /// # Errors
    /// If the path is not a stable regular file, the source is oversized, or a
    /// file-system or TOML parsing error occurs.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, FromFileError> {
        Self::from_file_with_limit(path, MAX_TOML_SOURCE_BYTES).map(|(source, _, _)| source)
    }
    /// Read a stable regular source under an explicit byte limit.
    pub(crate) fn from_file_with_limit<P: AsRef<Path>>(
        path: P,
        max_bytes: u64,
    ) -> Result<(Self, u64, RegularFileIdentity), FromFileError> {
        let path = path.as_ref().to_path_buf();
        log::trace!("reading TOML source: `{}`", path.display());
        let initial_path_metadata = regular_file_metadata(&path)?;
        if initial_path_metadata.len() > max_bytes {
            return Err(Report::new(FromFileError::TooLarge {
                actual: initial_path_metadata.len(),
                limit: max_bytes,
            }));
        }
        let canonical_path = fs::canonicalize(&path).change_context(FromFileError::Read)?;
        let identity = regular_file_identity(&initial_path_metadata, canonical_path);
        let mut file = File::open(&path).change_context(FromFileError::Read)?;
        let initial_open_metadata = file.metadata().change_context(FromFileError::Read)?;
        if !initial_open_metadata.file_type().is_file()
            || !metadata_same_snapshot(&initial_path_metadata, &initial_open_metadata)
        {
            return Err(Report::new(FromFileError::ChangedWhileReading));
        }
        let initial_len = initial_open_metadata.len();
        let capacity = usize::try_from(initial_len).unwrap_or(0);
        let mut raw_string = String::with_capacity(capacity);
        file.by_ref()
            .take(max_bytes.saturating_add(1))
            .read_to_string(&mut raw_string)
            .change_context(FromFileError::Read)?;
        let bytes_read = u64::try_from(raw_string.len()).unwrap_or(u64::MAX);
        if bytes_read > max_bytes {
            return Err(Report::new(FromFileError::TooLarge {
                actual: bytes_read,
                limit: max_bytes,
            }));
        }
        let final_open_metadata = file.metadata().change_context(FromFileError::Read)?;
        let final_path_metadata = regular_file_metadata(&path)?;
        if !metadata_same_snapshot(&initial_open_metadata, &final_open_metadata)
            || !metadata_same_snapshot(&final_open_metadata, &final_path_metadata)
            || initial_len != bytes_read
            || final_open_metadata.len() != bytes_read
            || final_path_metadata.len() != bytes_read
        {
            return Err(Report::new(FromFileError::ChangedWhileReading));
        }
        let table = raw_string
            .parse::<Table>()
            .change_context(FromFileError::Parse)?;
        Ok((TomlSource::new(path, table), bytes_read, identity))
    }
    /// Primarily for testing purposes: creates a source which will contain debug information
    /// about where this source was defined.
    #[track_caller]
    pub fn inline(table: Table) -> Self {
        Self::new(
            PathBuf::from(format!("inline:{}", std::panic::Location::caller())),
            table,
        )
    }
    /// Get an exclusive borrow of the TOML table inside
    pub fn table_mut(&mut self) -> &mut Table {
        &mut self.table
    }
    /// Fetch a value by parameter path
    pub fn fetch(&self, path: &ParameterId) -> Option<&toml::Value> {
        let mut segments = path.segments.iter();
        let first = segments.next()?;
        let mut value = self.table.get(first)?;
        for segment in segments {
            value = value.get(segment)?;
        }
        Some(value)
    }
    /// Get the file path of the source
    pub fn path(&self) -> &PathBuf {
        &self.path
    }
    pub(crate) fn find_unknown<'a, I>(&self, known: I) -> BTreeSet<ParameterId>
    where
        I: IntoIterator<Item = &'a ParameterId>,
    {
        let known_tree: ParamTree<'a> = known.into();
        find_unknown_parameters(&self.table, &known_tree)
    }
}
impl std::ops::Index<ParameterId> for TomlSource {
    type Output = toml::Value;
    fn index(&self, index: ParameterId) -> &Self::Output {
        self.fetch(&index)
            .unwrap_or_else(|| panic!("unknown parameter `{index}`"))
    }
}
#[derive(Default)]
struct ParamTree<'a>(BTreeMap<&'a str, ParamTree<'a>>);
impl std::fmt::Debug for ParamTree<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl<'a, I> From<I> for ParamTree<'a>
where
    I: IntoIterator<Item = &'a ParameterId>,
{
    fn from(iter: I) -> Self {
        let mut tree = Self(<_>::default());
        for path in iter {
            let mut current = &mut tree;
            for segment in &path.segments {
                current = current.0.entry(segment).or_default();
            }
        }
        tree
    }
}
fn find_unknown_parameters(table: &toml::Table, known: &ParamTree) -> BTreeSet<ParameterId> {
    #[derive(Default)]
    struct Traverse<'a> {
        current_path: Vec<&'a str>,
        unknown: BTreeSet<ParameterId>,
    }
    impl<'a> Traverse<'a> {
        fn run(mut self, table: &'a toml::Table, known: &ParamTree) -> Self {
            for (key, value) in table {
                if let Some(known) = known.0.get(key.as_str()) {
                    // we are in the "known"
                    if known.0.is_empty() {
                        // we reached the boundary of explicit "known".
                        // everything below is implied to be known
                    } else if let toml::Value::Table(nested) = value {
                        self.current_path.push(key.as_str());
                        self = self.run(nested, known);
                        self.current_path.pop();
                    } else {
                        // A known namespace with known descendants must be a
                        // table. Treat a scalar/array at that boundary as a
                        // structural configuration error instead of silently
                        // materializing every descendant from defaults.
                        let malformed_path = self
                            .current_path
                            .iter()
                            .chain(std::iter::once(&key.as_str()))
                            .into();
                        self.unknown.insert(malformed_path);
                    }
                } else {
                    // we are in the "unknown"
                    let unknown_path = self
                        .current_path
                        .iter()
                        .chain(std::iter::once(&key.as_str()))
                        .into();
                    self.unknown.insert(unknown_path);
                }
            }
            self
        }
    }
    Traverse::default().run(table, known).unknown
}
/// A utility, primarily for testing, to conveniently write content into a [`Table`].
///
/// ```
/// use iroha_config_base::toml::Writer;
/// use toml::Table;
///
/// let mut table = Table::new();
/// Writer::new(&mut table)
///     .write("foo", "some string")
///     .write("bar", "some other string")
///     .write(["baz", "foo", "bar"], 42);
///
/// assert_eq!(
///     table,
///     toml::toml! {
///         foo = "some string"
///         bar = "some other string"
///
///         [baz.foo]
///         bar = 42
///     }
/// );
/// ```
#[derive(Debug)]
pub struct Writer<'a> {
    table: &'a mut Table,
}
impl<'a> Writer<'a> {
    /// Constructor
    pub fn new(table: &'a mut Table) -> Self {
        Self { table }
    }
    /// Write a serializable value by path.
    /// Recursively creates all path segments as tables if they don't exist.
    ///
    /// # Panics
    ///
    /// - If there is existing non-table value along the path
    /// - If value cannot serialize into [`toml::Value`]
    pub fn write<P: WritePath, T: Into<toml::Value>>(
        &'a mut self,
        path: P,
        value: T,
    ) -> &'a mut Self {
        let mut current: Option<(&mut Table, &str)> = None;
        for i in path.path() {
            if let Some((table, key)) = current {
                let table = table
                    .entry(key)
                    .or_insert(toml::Value::Table(<_>::default()))
                    .as_table_mut()
                    .expect("expected a table");
                current = Some((table, i))
            } else {
                // IDK why Rust allows it
                current = Some((self.table, i))
            }
        }
        if let Some((table, key)) = current {
            table.insert(key.to_string(), value.into());
        }
        self
    }
}
/// Allows polymorphism for a field path in [`Writer::write`]:
///
/// ```
/// use iroha_config_base::toml::Writer;
///
/// let mut table = toml::Table::new();
/// Writer::new(&mut table)
///     // path: <root>.fine
///     .write("fine", 0)
///     // path: <root>.also.fine
///     .write(["also", "fine"], 1);
/// ```
pub trait WritePath {
    /// Provides an iterator over path segments
    fn path(self) -> impl IntoIterator<Item = &'static str>;
}
impl WritePath for &'static str {
    fn path(self) -> impl IntoIterator<Item = &'static str> {
        [self]
    }
}
impl<const N: usize> WritePath for [&'static str; N] {
    fn path(self) -> impl IntoIterator<Item = &'static str> {
        self
    }
}
impl<'a> From<&'a mut Table> for Writer<'a> {
    fn from(value: &'a mut Table) -> Self {
        Self::new(value)
    }
}
/// Extension trait to implement writing with [`Writer`] directly into [`Table`] in a chained manner.
pub trait WriteExt: Sized {
    /// See [`Writer::write`].
    #[must_use]
    fn write<P: WritePath, T: Into<toml::Value>>(self, path: P, value: T) -> Self;
}
impl WriteExt for Table {
    fn write<P: WritePath, T: Into<toml::Value>>(mut self, path: P, value: T) -> Self {
        Writer::new(&mut self).write(path, value);
        self
    }
}
/// Convert a TOML value into its Norito JSON equivalent.
///
/// # Errors
///
/// Returns [`json::Error`] if the input contains an invalid floating-point value or if any
/// nested element fails to convert into JSON.
pub fn value_to_json(value: &toml::Value) -> Result<JsonValue, json::Error> {
    Ok(match value {
        toml::Value::Boolean(b) => JsonValue::Bool(*b),
        toml::Value::Integer(i) => {
            if *i >= 0 {
                JsonValue::Number(JsonNumber::U64(
                    u64::try_from(*i).expect("non-negative integer"),
                ))
            } else {
                JsonValue::Number(JsonNumber::I64(*i))
            }
        }
        toml::Value::Float(f) => JsonValue::Number(JsonNumber::from_f64(*f).ok_or_else(|| {
            json::Error::InvalidField {
                field: "float".into(),
                message: format!("invalid float value {f} (NaN or infinite)"),
            }
        })?),
        toml::Value::String(s) => JsonValue::String(s.clone()),
        toml::Value::Datetime(dt) => JsonValue::String(dt.to_string()),
        toml::Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(value_to_json(item)?);
            }
            JsonValue::Array(out)
        }
        toml::Value::Table(table) => JsonValue::Object(table_to_json(table)?),
    })
}
fn table_to_json(table: &toml::Table) -> Result<JsonMap, json::Error> {
    let mut out = JsonMap::default();
    for (key, value) in table {
        out.insert(key.clone(), value_to_json(value)?);
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };
    use expect_test::expect;
    use toml::toml;
    use super::*;
    static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);
    fn temp_config_dir(label: &str) -> PathBuf {
        let nonce = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "iroha_config_base_toml_{label}_{}_{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temporary configuration directory");
        path
    }
    #[test]
    fn from_file_rejects_oversized_source_before_parsing() {
        let dir = temp_config_dir("oversized");
        let path = dir.join("oversized.toml");
        let file = File::create(&path).expect("create oversized source");
        file.set_len(MAX_TOML_SOURCE_BYTES + 1)
            .expect("extend oversized source");
        drop(file);
        let report = TomlSource::from_file(&path).expect_err("oversized source must fail");
        assert_eq!(
            *report.current_context(),
            FromFileError::TooLarge {
                actual: MAX_TOML_SOURCE_BYTES + 1,
                limit: MAX_TOML_SOURCE_BYTES,
            }
        );
        fs::remove_dir_all(dir).expect("remove temporary configuration directory");
    }
    #[cfg(unix)]
    #[test]
    fn from_file_rejects_symbolic_link() {
        use std::os::unix::fs::symlink;
        let dir = temp_config_dir("symlink");
        let target = dir.join("target.toml");
        let link = dir.join("link.toml");
        fs::write(&target, "value = 1\n").expect("write link target");
        symlink(&target, &link).expect("create source symlink");
        let report = TomlSource::from_file(&link).expect_err("source symlink must fail");
        assert_eq!(*report.current_context(), FromFileError::NotRegularFile);
        fs::remove_dir_all(dir).expect("remove temporary configuration directory");
    }
    #[test]
    fn toml_integer_to_json() {
        let value = toml::Value::Integer(42);
        let json = value_to_json(&value).expect("integer");
        assert_eq!(json, JsonValue::Number(JsonNumber::U64(42)));
    }
    #[test]
    fn toml_table_to_json() {
        let table = toml! {
            answer = 42
            nested = { flag = true }
        };
        let json = value_to_json(&toml::Value::Table(table)).expect("table");
        if let JsonValue::Object(ref map) = json {
            assert_eq!(map["answer"], JsonValue::Number(JsonNumber::U64(42)));
            let mut nested_expected = JsonMap::default();
            nested_expected.insert("flag".into(), JsonValue::Bool(true));
            assert_eq!(map["nested"], JsonValue::Object(nested_expected));
        } else {
            panic!("unexpected JSON value {json:?}");
        }
    }
    #[test]
    fn fetch_returns_value() {
        let table = toml! {
            [foo]
            bar = 42
        };
        let source = TomlSource::inline(table);
        let id = ParameterId::from(["foo", "bar"]);
        let value = source.fetch(&id).unwrap();
        assert_eq!(value, &toml::Value::Integer(42));
        assert_eq!(source[id], toml::Value::Integer(42));
    }
    #[test]
    fn create_param_tree() {
        let params = [
            ParameterId::from(["a", "b", "c"]),
            ParameterId::from(["a", "b", "d"]),
            ParameterId::from(["b", "a", "c"]),
            ParameterId::from(["foo", "bar"]),
        ];
        let map = ParamTree::from(params.iter());
        expect![[r#"
                {
                    "a": {
                        "b": {
                            "c": {},
                            "d": {},
                        },
                    },
                    "b": {
                        "a": {
                            "c": {},
                        },
                    },
                    "foo": {
                        "bar": {},
                    },
                }"#]]
        .assert_eq(&format!("{map:#?}"));
    }
    #[test]
    fn unknown_params_in_empty_are_empty() {
        let known = [
            ParameterId::from(["foo", "bar"]),
            ParameterId::from(["foo", "baz"]),
        ];
        let known: ParamTree = known.iter().into();
        let table = toml::Table::new();
        let unknown = find_unknown_parameters(&table, &known);
        assert_eq!(unknown, <_>::default());
    }
    #[test]
    fn with_empty_known_finds_root_unknowns() {
        let table = toml! {
            [foo]
            bar = "hey"
            [baz]
            foo = 412
        };
        let unknown = find_unknown_parameters(&table, &<_>::default());
        let expected = [ParameterId::from(["foo"]), ParameterId::from(["baz"])]
            .into_iter()
            .collect();
        assert_eq!(unknown, expected);
    }
    #[test]
    fn unknown_depth_2() {
        let known = [
            ParameterId::from(["foo", "bar"]),
            ParameterId::from(["foo", "baz"]),
        ];
        let known = ParamTree::from(known.iter());
        let table = toml! {
            [foo]
            bar = 42
            baz = "known"
            foo.bar = { unknown = true }
        };
        let unknown = find_unknown_parameters(&table, &known);
        let expected = vec![ParameterId::from(["foo", "foo"])]
            .into_iter()
            .collect();
        assert_eq!(unknown, expected);
    }
    #[test]
    fn nested_into_known_are_ok() {
        let known = [ParameterId::from(["a"])];
        let known = ParamTree::from(known.iter());
        let table = toml! {
            [a]
            b = 4
            c = 12
        };
        let unknown = find_unknown_parameters(&table, &known);
        assert_eq!(unknown, <_>::default());
    }
    #[test]
    fn writing_into_toml_works() {
        let mut table = Table::new();
        let complex = toml! {
            foo = false
            bar = true
        };
        Writer::new(&mut table)
            .write("foo", "test")
            .write(["bar", "foo"], 42)
            .write(["bar", "complex"], complex);
        expect![[r#"
            foo = "test"

            [bar]
            foo = 42

            [bar.complex]
            bar = true
            foo = false
        "#]]
        .assert_eq(&toml::to_string_pretty(&table).unwrap());
    }
}
