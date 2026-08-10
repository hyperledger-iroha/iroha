//! Python bindings for Connect key and exact-network session derivation.

use iroha_crypto::{
    KeyGenOption,
    kex::{KeyExchangeScheme, X25519Sha256},
};
use iroha_torii_shared::connect_sdk;
use pyo3::{
    Bound, Py, PyResult, Python,
    exceptions::{PyRuntimeError, PyValueError},
    types::{PyBytes, PyModule},
    wrap_pyfunction,
};
use x25519_dalek::StaticSecret;

use super::{PyNetworkId, fixed_array};

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(generate_connect_keypair_py, module)?)?;
    module.add_function(wrap_pyfunction!(derive_connect_sid_py, module)?)?;
    module.add_function(wrap_pyfunction!(
        connect_public_key_from_private_py,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(derive_connect_direction_keys_py, module)?)?;
    Ok(())
}

#[pyo3::pyfunction]
#[pyo3(name = "generate_connect_keypair")]
/// Generate an X25519 keypair for Connect.
fn generate_connect_keypair_py(py: Python<'_>) -> PyResult<(Py<PyBytes>, Py<PyBytes>)> {
    let scheme = X25519Sha256::new();
    let (public, secret) = scheme.try_keypair(KeyGenOption::Random).map_err(|err| {
        PyRuntimeError::new_err(format!("failed to generate X25519 keypair: {err}"))
    })?;
    let public_bytes = Py::from(PyBytes::new(py, public.as_bytes()));
    let private_bytes = Py::from(PyBytes::new(py, secret.to_bytes().as_ref()));
    Ok((private_bytes, public_bytes))
}

#[pyo3::pyfunction]
#[pyo3(name = "connect_public_key_from_private")]
/// Derive the public key corresponding to an X25519 private key.
fn connect_public_key_from_private_py(py: Python<'_>, private_key: &[u8]) -> PyResult<Py<PyBytes>> {
    let secret_bytes = fixed_array::<32>(private_key, "private_key")?;
    let scheme = X25519Sha256::new();
    let static_secret = StaticSecret::from(secret_bytes);
    let (public, _) = scheme.keypair(KeyGenOption::FromPrivateKey(static_secret));
    Ok(Py::from(PyBytes::new(py, public.as_bytes())))
}

#[pyo3::pyfunction]
#[pyo3(name = "derive_connect_sid")]
/// Derive the exact-network Connect session identifier.
fn derive_connect_sid_py(
    py: Python<'_>,
    network_id: &PyNetworkId,
    app_public_key: &[u8],
    nonce: &[u8],
) -> PyResult<Py<PyBytes>> {
    let app_pk = fixed_array::<32>(app_public_key, "app_public_key")?;
    let nonce = fixed_array::<16>(nonce, "nonce")?;
    let sid = connect_sdk::derive_session_id(network_id.as_inner(), &app_pk, &nonce);
    Ok(Py::from(PyBytes::new(py, &sid)))
}

#[pyo3::pyfunction]
#[pyo3(name = "derive_connect_direction_keys")]
/// Derive per-direction ChaCha20-Poly1305 keys from X25519 session material.
fn derive_connect_direction_keys_py(
    py: Python<'_>,
    local_private_key: &[u8],
    peer_public_key: &[u8],
    sid: &[u8],
) -> PyResult<(Py<PyBytes>, Py<PyBytes>)> {
    let local_sk = fixed_array::<32>(local_private_key, "local_private_key")?;
    let peer_pk = fixed_array::<32>(peer_public_key, "peer_public_key")?;
    let sid_arr = fixed_array::<32>(sid, "sid")?;
    let (k_app, k_wallet) = connect_sdk::x25519_derive_keys(&local_sk, &peer_pk, &sid_arr)
        .map_err(|err| PyValueError::new_err(format!("x25519 derive keys failed: {err}")))?;
    let app_bytes = Py::from(PyBytes::new(py, &k_app));
    let wallet_bytes = Py::from(PyBytes::new(py, &k_wallet));
    Ok((app_bytes, wallet_bytes))
}
