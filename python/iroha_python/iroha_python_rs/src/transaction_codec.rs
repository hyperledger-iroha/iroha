//! Explicit-layout Python entry points into the canonical Rust transaction codec.

use iroha_crypto::HashOf;
use iroha_data_model::{
    account::address::ChainDiscriminantGuard,
    transaction::{SignedTransaction, TransactionBuilder, TransactionPayload},
};
use norito::{NoritoDeserialize, NoritoSerialize, core::WireLayout, json};
use pyo3::{
    Bound, Py, PyResult, Python,
    exceptions::PyValueError,
    types::{PyBytes, PyModule, PyModuleMethods},
    wrap_pyfunction,
};

pub(super) fn layout(value: &str) -> PyResult<WireLayout> {
    match value {
        "current" => Ok(WireLayout::Compact),
        "dense" => Ok(WireLayout::Dense),
        _ => Err(PyValueError::new_err("layout must be 'current' or 'dense'")),
    }
}

pub(super) fn encode<T: NoritoSerialize>(
    value: &T,
    layout: WireLayout,
    framed: bool,
) -> PyResult<Vec<u8>> {
    let bytes = if framed {
        norito::encode_canonical_with_layout(value, layout)
    } else {
        norito::codec::encode_bare_with_layout(value, layout)
    }
    .map_err(|error| PyValueError::new_err(format!("transaction encoding failed: {error}")))?;
    super::require_canonical_signed_transaction_wire_size_v1_native(&bytes)
        .map_err(super::NativeValidationError::into_py)?;
    Ok(bytes)
}

fn decode<T>(bytes: &[u8], layout: WireLayout, framed: bool) -> PyResult<T>
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    super::require_canonical_signed_transaction_wire_size_v1_native(bytes)
        .map_err(super::NativeValidationError::into_py)?;
    // The framed reader verifies the advertised minor/flags against the expected
    // layout. Bare payloads have no header, so their owner must supply the layout.
    if framed {
        norito::decode_canonical_with_layout(bytes, layout)
    } else {
        norito::codec::decode_bare_with_layout(bytes, layout)
    }
    .map_err(|error| PyValueError::new_err(format!("transaction decoding failed: {error}")))
}

pub(super) fn payload_hash(
    payload: &TransactionPayload,
    layout: WireLayout,
) -> PyResult<[u8; iroha_crypto::Hash::LENGTH]> {
    HashOf::try_new_with_layout(payload, layout)
        .map(|hash| *hash.as_ref())
        .map_err(|error| PyValueError::new_err(format!("transaction hashing failed: {error}")))
}

fn decode_json<T>(bytes: &[u8], layout: &str, framed: bool, discriminant: u16) -> PyResult<String>
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de> + json::JsonSerialize,
{
    let value: T = decode(bytes, self::layout(layout)?, framed)?;
    let _guard = ChainDiscriminantGuard::enter(discriminant);
    json::to_json(&value).map_err(|error| {
        PyValueError::new_err(format!("transaction JSON encoding failed: {error}"))
    })
}

fn encode_json<T>(text: &str, layout: &str, framed: bool, discriminant: u16) -> PyResult<Vec<u8>>
where
    T: NoritoSerialize + json::JsonDeserialize,
{
    super::require_canonical_signed_transaction_wire_size_v1_native(text.as_bytes())
        .map_err(super::NativeValidationError::into_py)?;
    let layout = self::layout(layout)?;
    let _guard = ChainDiscriminantGuard::enter(discriminant);
    let value: T = json::from_str(text).map_err(|error| {
        PyValueError::new_err(format!("transaction JSON decoding failed: {error}"))
    })?;
    encode(&value, layout, framed)
}

/// Decode one typed payload to JSON without changing its selected wire layout.
#[pyo3::pyfunction]
#[pyo3(signature = (payload, *, layout, chain_discriminant, framed=false))]
fn decode_transaction_payload_with_layout(
    payload: &[u8],
    layout: &str,
    #[pyo3(from_py_with = super::exact_chain_discriminant)] chain_discriminant: u16,
    framed: bool,
) -> PyResult<String> {
    decode_json::<TransactionPayload>(payload, layout, framed, chain_discriminant)
}

/// Encode the existing typed payload JSON using one explicit layout.
#[pyo3::pyfunction]
#[pyo3(signature = (payload_json, *, layout, chain_discriminant, framed=false))]
fn encode_transaction_payload_with_layout(
    py: Python<'_>,
    payload_json: &str,
    layout: &str,
    #[pyo3(from_py_with = super::exact_chain_discriminant)] chain_discriminant: u16,
    framed: bool,
) -> PyResult<Py<PyBytes>> {
    let bytes =
        encode_json::<TransactionPayload>(payload_json, layout, framed, chain_discriminant)?;
    Ok(PyBytes::new(py, &bytes).unbind())
}

/// Decode a complete signed value; codec acceptance does not authenticate its signature.
#[pyo3::pyfunction]
#[pyo3(signature = (transaction, *, layout, chain_discriminant, framed=false))]
fn decode_signed_transaction_with_layout(
    transaction: &[u8],
    layout: &str,
    #[pyo3(from_py_with = super::exact_chain_discriminant)] chain_discriminant: u16,
    framed: bool,
) -> PyResult<String> {
    decode_json::<SignedTransaction>(transaction, layout, framed, chain_discriminant)
}

/// Encode the existing signed transaction model; this does not select a network wire.
#[pyo3::pyfunction]
#[pyo3(signature = (transaction_json, *, layout, chain_discriminant, framed=false))]
fn encode_signed_transaction_with_layout(
    py: Python<'_>,
    transaction_json: &str,
    layout: &str,
    #[pyo3(from_py_with = super::exact_chain_discriminant)] chain_discriminant: u16,
    framed: bool,
) -> PyResult<Py<PyBytes>> {
    let bytes =
        encode_json::<SignedTransaction>(transaction_json, layout, framed, chain_discriminant)?;
    Ok(PyBytes::new(py, &bytes).unbind())
}

/// Compute the typed signing prehash after exact decoding in the declared layout.
#[pyo3::pyfunction]
#[pyo3(signature = (payload, *, layout, framed=false))]
fn transaction_payload_hash_with_layout(
    py: Python<'_>,
    payload: &[u8],
    layout: &str,
    framed: bool,
) -> PyResult<Py<PyBytes>> {
    let layout = self::layout(layout)?;
    let payload = decode::<TransactionPayload>(payload, layout, framed)?;
    Ok(PyBytes::new(py, &payload_hash(&payload, layout)?).unbind())
}

/// Rebuild and sign one typed payload, retaining its explicit layout in the output.
#[pyo3::pyfunction]
#[pyo3(signature = (payload, private_key, *, layout, framed=false))]
fn sign_transaction_payload_with_layout(
    py: Python<'_>,
    payload: &[u8],
    private_key: &[u8],
    layout: &str,
    framed: bool,
) -> PyResult<Py<PyBytes>> {
    let layout = self::layout(layout)?;
    let payload = decode::<TransactionPayload>(payload, layout, framed)?;
    let private_key = super::parse_private_key(private_key)?;
    let signed = TransactionBuilder::from_payload(payload)
        .map_err(|error| PyValueError::new_err(format!("invalid transaction payload: {error}")))?
        .try_sign_with_layout(&private_key, layout)
        .map_err(|error| PyValueError::new_err(format!("transaction signing failed: {error}")))?;
    Ok(PyBytes::new(py, &encode(&signed, layout, framed)?).unbind())
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        decode_transaction_payload_with_layout,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        encode_transaction_payload_with_layout,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        decode_signed_transaction_with_layout,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        encode_signed_transaction_with_layout,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        transaction_payload_hash_with_layout,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        sign_transaction_payload_with_layout,
        module
    )?)?;
    Ok(())
}

#[cfg(test)]
mod tests;
