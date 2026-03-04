//! Helpers for building Norito JSON values in the CLI

use eyre::eyre;
use norito::json::{self, JsonSerialize};

/// Convert any [`JsonSerialize`] value into a [`json::Value`].
pub fn json_value<T: JsonSerialize + ?Sized>(value: &T) -> eyre::Result<json::Value> {
    json::to_value(value).map_err(|err| eyre!("serialize Norito JSON value: {err}"))
}

/// Build a JSON array from the provided values.
pub fn json_array<V, I>(values: I) -> eyre::Result<json::Value>
where
    V: JsonSerialize,
    I: IntoIterator<Item = V>,
{
    json::array(values).map_err(|err| eyre!("serialize Norito JSON array: {err}"))
}

/// Build a JSON object from precomputed value pairs.
pub fn json_object<K, I>(pairs: I) -> eyre::Result<json::Value>
where
    K: Into<String>,
    I: IntoIterator<Item = (K, json::Value)>,
{
    json::object(pairs).map_err(|err| eyre!("serialize Norito JSON object: {err}"))
}
