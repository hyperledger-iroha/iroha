//! Canonical Norito representation of configuration fragments.
//!
//! Profile digests hash this representation, never TOML text, so comments, key order, table
//! style and number spelling cannot change a digest. A fragment is flattened into its leaves in
//! key order; each leaf carries its full path, so the encoding is unambiguous without any
//! recursive type.

use norito::codec::{Decode, Encode};
use thiserror::Error;

/// One segment of a leaf path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_config::profile::canonical::CanonicalPathSegmentV1")]
pub enum CanonicalPathSegmentV1 {
    /// Table key.
    Key(String),
    /// Array index.
    Index(u32),
}

/// One leaf value.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_config::profile::canonical::CanonicalLeafV1")]
pub enum CanonicalLeafV1 {
    /// Boolean.
    Bool(bool),
    /// Signed 64-bit integer.
    Integer(i64),
    /// IEEE-754 binary64 bit pattern; `-0.0` is folded into `0.0` and NaN is rejected.
    Float(u64),
    /// UTF-8 string.
    String(String),
    /// An empty array.
    EmptyArray,
    /// An empty table.
    EmptyTable,
}

/// One leaf and its full path.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_config::profile::canonical::CanonicalEntryV1")]
pub struct CanonicalEntryV1 {
    /// Keys and indices from the fragment root to the leaf.
    pub path: Vec<CanonicalPathSegmentV1>,
    /// Leaf value.
    pub value: CanonicalLeafV1,
}

/// A configuration fragment in canonical form: its leaves in key order.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_config::profile::canonical::CanonicalTableV1")]
pub struct CanonicalTableV1 {
    /// Leaves, ordered by table key (byte order) and then array index.
    pub entries: Vec<CanonicalEntryV1>,
}

/// A TOML value that has no canonical form.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CanonicalValueError {
    /// Floats must be comparable to be hashed.
    #[error("`{path}` is NaN, which has no canonical form")]
    NaN {
        /// Dotted key of the value.
        path: String,
    },
    /// Profiles carry no dates.
    #[error("`{path}` is a TOML datetime, which profiles do not use")]
    Datetime {
        /// Dotted key of the value.
        path: String,
    },
    /// An array is too long for a `u32` index.
    #[error("`{path}` has more elements than a canonical index can address")]
    ArrayTooLong {
        /// Dotted key of the value.
        path: String,
    },
}

impl CanonicalTableV1 {
    /// Flatten one TOML table into canonical form.
    ///
    /// # Errors
    ///
    /// [`CanonicalValueError`] for NaN floats, datetimes and oversized arrays.
    pub fn from_table(table: &toml::Table) -> Result<Self, CanonicalValueError> {
        let mut entries = Vec::new();
        let mut path = Vec::new();
        flatten_table(table, &mut path, &mut entries)?;
        Ok(Self { entries })
    }
}

fn flatten_table(
    table: &toml::Table,
    path: &mut Vec<CanonicalPathSegmentV1>,
    entries: &mut Vec<CanonicalEntryV1>,
) -> Result<(), CanonicalValueError> {
    if table.is_empty() {
        entries.push(CanonicalEntryV1 {
            path: path.clone(),
            value: CanonicalLeafV1::EmptyTable,
        });
        return Ok(());
    }
    let mut keys: Vec<&String> = table.keys().collect();
    keys.sort();
    for key in keys {
        path.push(CanonicalPathSegmentV1::Key(key.clone()));
        flatten_value(&table[key.as_str()], path, entries)?;
        path.pop();
    }
    Ok(())
}

fn flatten_value(
    value: &toml::Value,
    path: &mut Vec<CanonicalPathSegmentV1>,
    entries: &mut Vec<CanonicalEntryV1>,
) -> Result<(), CanonicalValueError> {
    let leaf = match value {
        toml::Value::Boolean(value) => CanonicalLeafV1::Bool(*value),
        toml::Value::Integer(value) => CanonicalLeafV1::Integer(*value),
        toml::Value::Float(value) => {
            if value.is_nan() {
                return Err(CanonicalValueError::NaN {
                    path: display_path(path),
                });
            }
            let value = if *value == 0.0 { 0.0_f64 } else { *value };
            CanonicalLeafV1::Float(value.to_bits())
        }
        toml::Value::String(value) => CanonicalLeafV1::String(value.clone()),
        toml::Value::Array(values) if values.is_empty() => CanonicalLeafV1::EmptyArray,
        toml::Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                let index =
                    u32::try_from(index).map_err(|_| CanonicalValueError::ArrayTooLong {
                        path: display_path(path),
                    })?;
                path.push(CanonicalPathSegmentV1::Index(index));
                flatten_value(value, path, entries)?;
                path.pop();
            }
            return Ok(());
        }
        toml::Value::Table(table) => return flatten_table(table, path, entries),
        toml::Value::Datetime(_) => {
            return Err(CanonicalValueError::Datetime {
                path: display_path(path),
            });
        }
    };
    entries.push(CanonicalEntryV1 {
        path: path.clone(),
        value: leaf,
    });
    Ok(())
}

fn display_path(path: &[CanonicalPathSegmentV1]) -> String {
    use std::fmt::Write as _;
    let mut rendered = String::new();
    for segment in path {
        match segment {
            CanonicalPathSegmentV1::Key(key) => rendered = join_path(&rendered, key),
            CanonicalPathSegmentV1::Index(index) => {
                write!(rendered, "[{index}]").expect("writing to a String cannot fail");
            }
        }
    }
    rendered
}

/// Join a dotted key prefix and one segment.
pub(crate) fn join_path(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_owned()
    } else {
        format!("{prefix}.{key}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatting_does_not_change_the_canonical_form() {
        let spaced: toml::Table =
            toml::from_str("# comment\nb = 1_000\n[a]\nz = \"x\"\ny = [1, 2]\nfloat = -0.0\n")
                .unwrap();
        let inline: toml::Table =
            toml::from_str("a = { y = [1,2], float = 0.0, z = 'x' }\nb = 1000").unwrap();
        let left = CanonicalTableV1::from_table(&spaced).unwrap();
        let right = CanonicalTableV1::from_table(&inline).unwrap();
        assert_eq!(left, right);
        assert_eq!(
            norito::encode_canonical(&left).unwrap(),
            norito::encode_canonical(&right).unwrap()
        );
    }

    #[test]
    fn leaves_carry_their_full_path_in_key_order() {
        let table: toml::Table =
            toml::from_str("z = []\n[a]\nm = {}\n[[a.list]]\nx = true\n").unwrap();
        let key = |key: &str| CanonicalPathSegmentV1::Key(key.to_owned());
        assert_eq!(
            CanonicalTableV1::from_table(&table).unwrap().entries,
            [
                CanonicalEntryV1 {
                    path: vec![
                        key("a"),
                        key("list"),
                        CanonicalPathSegmentV1::Index(0),
                        key("x")
                    ],
                    value: CanonicalLeafV1::Bool(true),
                },
                CanonicalEntryV1 {
                    path: vec![key("a"), key("m")],
                    value: CanonicalLeafV1::EmptyTable,
                },
                CanonicalEntryV1 {
                    path: vec![key("z")],
                    value: CanonicalLeafV1::EmptyArray,
                },
            ]
        );
        // A dotted key is not the same leaf as a nested key.
        let quoted: toml::Table = toml::from_str("\"a.b\" = 1\n").unwrap();
        let nested: toml::Table = toml::from_str("a.b = 1\n").unwrap();
        assert_ne!(
            CanonicalTableV1::from_table(&quoted).unwrap(),
            CanonicalTableV1::from_table(&nested).unwrap()
        );
    }

    #[test]
    fn values_roundtrip_through_norito() {
        let table: toml::Table = toml::from_str(
            "flag = true\nratio = 1.5\nname = \"n\"\nempty = []\n[[items]]\nindex = 3\n[nested.deep]\nvalue = -7\n",
        )
        .unwrap();
        let value = CanonicalTableV1::from_table(&table).unwrap();
        let bytes = norito::encode_canonical(&value).unwrap();
        let decoded: CanonicalTableV1 = norito::decode_canonical(&bytes).unwrap();
        assert_eq!(decoded, value);
    }

    #[test]
    fn nan_and_datetimes_are_rejected_with_their_path() {
        let nan: toml::Table = toml::from_str("[a]\nb = nan\n").unwrap();
        assert_eq!(
            CanonicalTableV1::from_table(&nan),
            Err(CanonicalValueError::NaN {
                path: "a.b".to_owned()
            })
        );
        let date: toml::Table = toml::from_str("list = [1979-05-27]\n").unwrap();
        assert_eq!(
            CanonicalTableV1::from_table(&date),
            Err(CanonicalValueError::Datetime {
                path: "list[0]".to_owned()
            })
        );
    }

    #[test]
    fn join_path_handles_the_root() {
        assert_eq!(join_path("", "a"), "a");
        assert_eq!(join_path("a", "b"), "a.b");
    }
}
