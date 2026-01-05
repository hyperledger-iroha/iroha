//! Various utilities

use std::{path::PathBuf, time::Duration};

use derive_more::Display;
use drop_bomb::DropBomb;
use error_stack::Report;
use norito::{
    NoritoDeserialize, NoritoSerialize,
    json::{self, JsonDeserialize, JsonSerialize},
};
use num_traits::{FromPrimitive, ToPrimitive};
use thiserror::Error;
use toml::Value as TomlValue;

const U64_BYTES: usize = core::mem::size_of::<u64>();
const DURATION_OVERFLOW: &str = "duration does not fit into u64 milliseconds";
const BYTES_RANGE_ERROR: &str = "byte count out of range for target integer";

/// Serialize [`Duration`] as a number of milliseconds.
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Display)]
#[display("{_0:?}")]
#[repr(transparent)]
pub struct DurationMs(pub Duration);

/// Error produced when parsing a [`DurationMs`] from a string fails.
#[derive(Debug, Copy, Clone, thiserror::Error)]
#[error("failed to parse duration in milliseconds")]
pub struct ParseDurationMsError;

/// Error produced when parsing a [`Bytes`] value from a string fails.
#[derive(Debug, Copy, Clone, thiserror::Error)]
#[error("failed to parse byte count")]
pub struct ParseBytesError;

impl DurationMs {
    /// Access the wrapped [`Duration`].
    #[inline]
    pub fn get(self) -> Duration {
        self.0
    }

    #[inline]
    fn to_millis(self) -> Result<u64, norito::core::Error> {
        self.0
            .as_millis()
            .try_into()
            .map_err(|_| norito::core::Error::Message(DURATION_OVERFLOW.into()))
    }
}

impl From<Duration> for DurationMs {
    fn from(value: Duration) -> Self {
        Self(value)
    }
}

impl NoritoSerialize for DurationMs {
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
        let millis = self.to_millis()?;
        <u64 as NoritoSerialize>::serialize(&millis, writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        Some(U64_BYTES)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        Some(U64_BYTES)
    }
}

impl<'de> NoritoDeserialize<'de> for DurationMs {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        let millis = <u64 as NoritoDeserialize>::deserialize(archived.cast());
        Self(Duration::from_millis(millis))
    }

    fn try_deserialize(
        archived: &'de norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let millis = <u64 as NoritoDeserialize>::deserialize(archived.cast());
        Ok(Self(Duration::from_millis(millis)))
    }
}

impl JsonSerialize for DurationMs {
    fn json_serialize(&self, out: &mut String) {
        match self.to_millis() {
            Ok(millis) => out.push_str(&millis.to_string()),
            Err(_) => out.push('0'),
        }
    }
}

impl JsonDeserialize for DurationMs {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let millis = parser.parse_u64()?;
        Ok(Self(Duration::from_millis(millis)))
    }
}

impl core::str::FromStr for DurationMs {
    type Err = ParseDurationMsError;

    fn from_str(s: &str) -> core::result::Result<Self, Self::Err> {
        let millis = s.parse::<u64>().map_err(|_| ParseDurationMsError)?;
        Ok(Self(Duration::from_millis(millis)))
    }
}

/// A number of bytes stored in the provided integer type.
#[derive(Debug, Copy, Clone)]
pub struct Bytes<T: num_traits::int::PrimInt>(pub T);

impl<T: num_traits::int::PrimInt> Bytes<T> {
    /// Access the wrapped value.
    #[inline]
    pub fn get(self) -> T {
        self.0
    }
}

impl<T: num_traits::int::PrimInt> From<T> for Bytes<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<T> core::str::FromStr for Bytes<T>
where
    T: num_traits::int::PrimInt + FromPrimitive,
{
    type Err = ParseBytesError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = s.parse::<u64>().map_err(|_| ParseBytesError)?;
        let Some(inner) = T::from_u64(value) else {
            return Err(ParseBytesError);
        };
        Ok(Self(inner))
    }
}

impl<T> NoritoSerialize for Bytes<T>
where
    T: num_traits::int::PrimInt + ToPrimitive + NoritoSerialize + Copy,
{
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
        self.0.serialize(writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        self.0.encoded_len_hint()
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        self.0.encoded_len_exact()
    }
}

impl<'de, T> NoritoDeserialize<'de> for Bytes<T>
where
    T: num_traits::int::PrimInt + FromPrimitive + NoritoDeserialize<'de> + Copy,
{
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        let inner = T::deserialize(archived.cast());
        Self(inner)
    }

    fn try_deserialize(
        archived: &'de norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let inner = T::try_deserialize(archived.cast())?;
        Ok(Self(inner))
    }
}

impl<T> JsonSerialize for Bytes<T>
where
    T: num_traits::int::PrimInt + ToPrimitive,
{
    fn json_serialize(&self, out: &mut String) {
        match self.0.to_u64() {
            Some(value) => out.push_str(&value.to_string()),
            None => out.push('0'),
        }
    }
}

impl<T> JsonDeserialize for Bytes<T>
where
    T: num_traits::int::PrimInt + FromPrimitive,
{
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_u64()?;
        let Some(inner) = T::from_u64(value) else {
            return Err(json::Error::InvalidField {
                field: "bytes".into(),
                message: BYTES_RANGE_ERROR.into(),
            });
        };
        Ok(Self(inner))
    }
}

/// A tool to implement the `extends` mixin mechanism.
#[derive(Debug, Eq, PartialEq)]
pub enum ExtendsPaths {
    /// A single path to extend from
    Single(PathBuf),
    /// A chain of paths to extend from
    Chain(Vec<PathBuf>),
}

/// Iterator over [`ExtendsPaths`] for convenience.
pub enum ExtendsPathsIter<'a> {
    /// Single file path case.
    Single(Option<&'a PathBuf>),
    /// Iterator over a chain of file paths.
    Chain(std::slice::Iter<'a, PathBuf>),
}

impl ExtendsPaths {
    /// Normalize into an iterator over a chain of paths to extend from.
    #[allow(clippy::iter_without_into_iter)]
    pub fn iter(&self) -> ExtendsPathsIter<'_> {
        match self {
            Self::Single(path) => ExtendsPathsIter::Single(Some(path)),
            Self::Chain(paths) => ExtendsPathsIter::Chain(paths.iter()),
        }
    }
}

impl<'a> Iterator for ExtendsPathsIter<'a> {
    type Item = &'a PathBuf;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            ExtendsPathsIter::Single(path) => path.take(),
            ExtendsPathsIter::Chain(iter) => iter.next(),
        }
    }
}

/// Errors produced when parsing the `extends` directive in configuration files.
#[derive(Debug, Error, Copy, Clone, Eq, PartialEq)]
pub enum ExtendsPathsError {
    /// The `extends` value must be either a string or an array of strings.
    #[error("expected a string or an array of strings")]
    InvalidType,
    /// One of the array elements is not a string.
    #[error("array element at index {index} must be a string")]
    InvalidArrayElement {
        /// Zero-based index of the offending element.
        index: usize,
    },
}

impl TryFrom<TomlValue> for ExtendsPaths {
    type Error = ExtendsPathsError;

    fn try_from(value: TomlValue) -> Result<Self, Self::Error> {
        match value {
            TomlValue::String(path) => Ok(Self::Single(PathBuf::from(path))),
            TomlValue::Array(values) => {
                let mut paths = Vec::with_capacity(values.len());
                for (index, value) in values.into_iter().enumerate() {
                    match value {
                        TomlValue::String(path) => paths.push(PathBuf::from(path)),
                        _ => return Err(ExtendsPathsError::InvalidArrayElement { index }),
                    }
                }
                Ok(Self::Chain(paths))
            }
            _ => Err(ExtendsPathsError::InvalidType),
        }
    }
}

/// A tool to collect multiple [`Report`]s.
///
/// Will panic on [`Drop`] unless [`Emitter::into_result`] is called.
#[derive(Debug)]
pub struct Emitter<C> {
    report: Option<Report<[C]>>,
    bomb: DropBomb,
}

impl<C> Default for Emitter<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C> Emitter<C> {
    /// Constructor
    pub fn new() -> Self {
        Self {
            report: None,
            bomb: DropBomb::new("Emitter dropped without calling `into_result()`"),
        }
    }

    /// Emit a single report
    pub fn emit(&mut self, report: Report<C>) {
        match &mut self.report {
            Some(existing) => existing.extend(core::iter::once(report)),
            None => {
                self.report = Some(report.expand());
            }
        }
    }

    /// Convert into [`Err`] if any report was emitted, otherwise [`Ok`].
    ///
    /// # Errors
    /// If at least one report was emitted.
    pub fn into_result(mut self) -> core::result::Result<(), Report<[C]>> {
        self.bomb.defuse();
        self.report.map_or_else(|| Ok(()), Err)
    }
}

/// An extension of [`Result`] to add convenience methods to work with [`Emitter`].
pub trait EmitterResultExt<T, C> {
    /// If [`Ok`], return [`Some`]; otherwise, emit an error and return [`None`].
    fn ok_or_emit(self, emitter: &mut Emitter<C>) -> Option<T>;
}

impl<T, C> EmitterResultExt<T, C> for core::result::Result<T, Report<C>> {
    fn ok_or_emit(self, emitter: &mut Emitter<C>) -> Option<T> {
        self.map_or_else(
            |report| {
                emitter.emit(report);
                None
            },
            Some,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extends_paths_single() {
        let value = TomlValue::String("./path".into());
        let parsed = ExtendsPaths::try_from(value).expect("single path");
        assert_eq!(parsed, ExtendsPaths::Single(PathBuf::from("./path")));
    }

    #[test]
    fn extends_paths_chain() {
        let value = TomlValue::Array(vec![
            TomlValue::String("foo".into()),
            TomlValue::String("bar".into()),
            TomlValue::String("baz".into()),
        ]);
        let parsed = ExtendsPaths::try_from(value).expect("chain path");
        assert_eq!(
            parsed,
            ExtendsPaths::Chain(vec![
                PathBuf::from("foo"),
                PathBuf::from("bar"),
                PathBuf::from("baz")
            ])
        );
    }

    #[test]
    fn extends_paths_invalid_type() {
        let value = TomlValue::Integer(42);
        assert_eq!(
            ExtendsPaths::try_from(value).unwrap_err(),
            ExtendsPathsError::InvalidType
        );
    }

    #[test]
    fn extends_paths_invalid_element() {
        let value = TomlValue::Array(vec![TomlValue::Integer(5)]);
        assert_eq!(
            ExtendsPaths::try_from(value).unwrap_err(),
            ExtendsPathsError::InvalidArrayElement { index: 0 }
        );
    }

    #[test]
    fn extends_iter_helpers() {
        let single = ExtendsPaths::Single(PathBuf::from("one"));
        let collected: Vec<_> = single.iter().map(|p| p.to_str().unwrap()).collect();
        assert_eq!(collected, vec!["one"]);

        let chain = ExtendsPaths::Chain(vec![
            PathBuf::from("a"),
            PathBuf::from("b"),
            PathBuf::from("c"),
        ]);
        let collected: Vec<_> = chain.iter().map(|p| p.to_str().unwrap()).collect();
        assert_eq!(collected, vec!["a", "b", "c"]);
    }

    #[test]
    fn duration_ms_json_roundtrip() {
        let original = DurationMs(Duration::from_millis(42));
        let json = json::to_json(&original).expect("serialize");
        assert_eq!(json, "42");
        let parsed: DurationMs = json::from_json(&json).expect("deserialize");
        assert_eq!(parsed.get(), Duration::from_millis(42));
    }

    #[test]
    fn bytes_json_roundtrip() {
        let original: Bytes<u64> = Bytes(1024);
        let json = json::to_json(&original).expect("serialize");
        assert_eq!(json, "1024");
        let parsed: Bytes<u64> = json::from_json(&json).expect("deserialize");
        assert_eq!(parsed.get(), 1024);
    }

    #[test]
    fn bytes_from_str_parses_numeric() {
        let parsed: Bytes<u64> = "2048".parse().expect("parse bytes");
        assert_eq!(parsed.get(), 2048);
    }

    #[test]
    fn bytes_from_str_rejects_invalid_input() {
        let parsed = "nope".parse::<Bytes<u64>>();
        assert!(parsed.is_err());
    }
}
