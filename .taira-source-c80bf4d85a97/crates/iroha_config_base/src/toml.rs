//! TOML-specific tools.
//!
//! While it is definitely possible to support other formats than TOML, since there is no
//! need for this for now, TOML support is integrated in a non-generic way.

use std::{
    collections::{BTreeMap, BTreeSet},
    convert::TryFrom,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use error_stack::{Report, ResultExt};
use norito::json::{self, Map as JsonMap, Number as JsonNumber, Value as JsonValue};
use thiserror::Error;
use toml::Table;

type Result<T, E> = core::result::Result<T, Report<E>>;

use crate::ParameterId;

/// A source of configuration in TOML format
#[derive(Debug, Clone)]
pub struct TomlSource {
    path: PathBuf,
    table: Table,
}

/// Error of [`TomlSource::from_file`]
#[derive(Error, Debug, Copy, Clone)]
pub enum FromFileError {
    /// File system error while opening or reading the file.
    #[error("File system error")]
    Read,
    /// Error while deserializing file contents as TOML.
    #[error("Error while deserializing file contents as TOML")]
    Parse,
}

impl TomlSource {
    /// Constructor
    pub fn new(path: PathBuf, table: Table) -> Self {
        Self { path, table }
    }

    /// Read from a file
    ///
    /// # Errors
    /// If a file system or a TOML parsing error occurs.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, FromFileError> {
        let path = path.as_ref().to_path_buf();

        log::trace!("reading TOML source: `{}`", path.display());

        let mut raw_string = String::new();
        File::open(&path)
            .change_context(FromFileError::Read)?
            .read_to_string(&mut raw_string)
            .change_context(FromFileError::Read)?;

        let table = raw_string
            .parse::<Table>()
            .change_context(FromFileError::Parse)?;

        Ok(TomlSource::new(path, table))
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
    use expect_test::expect;
    use toml::toml;

    use super::*;

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
