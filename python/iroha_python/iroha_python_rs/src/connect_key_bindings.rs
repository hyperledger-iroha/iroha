//! Python bindings for Connect key and exact-network session derivation.

use iroha_crypto::{
    KeyGenOption,
    kex::{KeyExchangeScheme, X25519Sha256},
};
use iroha_torii_shared::connect::{Constraints, WalletSignatureV1};
use iroha_torii_shared::connect_sdk;
use pyo3::{
    Bound, Py, PyResult, Python,
    exceptions::{PyRuntimeError, PyValueError},
    types::{PyAny, PyBytes, PyModule},
    wrap_pyfunction,
};
use x25519_dalek::StaticSecret;

use super::{
    PyNetworkId, ensure_ed25519_account, fixed_array, parse_exact_i105_account_id,
    parse_permissions, parse_sign_in_proof, require_non_blank_unpadded, require_single_signatory,
};

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(generate_connect_keypair_py, module)?)?;
    module.add_function(wrap_pyfunction!(derive_connect_sid_py, module)?)?;
    module.add_function(wrap_pyfunction!(
        connect_public_key_from_private_py,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(derive_connect_direction_keys_py, module)?)?;
    module.add_function(wrap_pyfunction!(build_connect_approve_preimage_py, module)?)?;
    module.add_function(wrap_pyfunction!(connect_relay_auth_hash_py, module)?)?;
    module.add_function(wrap_pyfunction!(
        verify_connect_approval_signature_py,
        module
    )?)?;
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
    if app_pk.iter().all(|byte| *byte == 0) || nonce.iter().all(|byte| *byte == 0) {
        return Err(PyValueError::new_err(
            "Connect app_public_key and nonce must not be all zero",
        ));
    }
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

#[pyo3::pyfunction]
#[pyo3(name = "build_connect_approve_preimage")]
/// Build the canonical approval preimage for wallet signatures.
fn build_connect_approve_preimage_py(
    py: Python<'_>,
    network_id: &PyNetworkId,
    sid: &[u8],
    app_public_key: &[u8],
    nonce: &[u8],
    wallet_public_key: &[u8],
    account_id: &str,
    permissions: Option<&Bound<'_, PyAny>>,
    proof: Option<&Bound<'_, PyAny>>,
    relay_auth: &[u8],
) -> PyResult<Py<PyBytes>> {
    let sid_arr = fixed_array::<32>(sid, "sid")?;
    let app_pk = fixed_array::<32>(app_public_key, "app_public_key")?;
    let nonce = fixed_array::<16>(nonce, "nonce")?;
    let wallet_pk = fixed_array::<32>(wallet_public_key, "wallet_public_key")?;
    let relay_auth = fixed_array::<32>(relay_auth, "relay_auth")?;
    if app_pk.iter().all(|byte| *byte == 0) {
        return Err(PyValueError::new_err("app_public_key must not be all zero"));
    }
    if wallet_pk.iter().all(|byte| *byte == 0) {
        return Err(PyValueError::new_err(
            "wallet_public_key must not be all zero",
        ));
    }
    if nonce.iter().all(|byte| *byte == 0) {
        return Err(PyValueError::new_err("nonce must not be all zero"));
    }
    if connect_sdk::derive_session_id(network_id.as_inner(), &app_pk, &nonce) != sid_arr {
        return Err(PyValueError::new_err(
            "sid does not match exact network_id, app_public_key, and nonce",
        ));
    }
    let account = parse_exact_i105_account_id(account_id, "account_id")?;
    if account.to_string() != account_id {
        return Err(PyValueError::new_err(
            "account_id must be an exact canonical I105 account id",
        ));
    }
    ensure_ed25519_account(&account)?;

    let permissions_parsed = parse_permissions(permissions.cloned(), "permissions")?;
    let proof_parsed = parse_sign_in_proof(proof.cloned())?;

    let preimage = connect_sdk::build_approve_preimage(
        &Constraints {
            network_id: *network_id.as_inner(),
        },
        &sid_arr,
        &app_pk,
        &wallet_pk,
        account_id,
        permissions_parsed.as_ref(),
        proof_parsed.as_ref(),
        &relay_auth,
    );
    Ok(Py::from(PyBytes::new(py, &preimage)))
}

#[pyo3::pyfunction]
#[pyo3(name = "connect_relay_auth_hash")]
/// Hash the exact relay token binding used by Connect approvals.
fn connect_relay_auth_hash_py(
    py: Python<'_>,
    sid: &[u8],
    relay_token: &str,
) -> PyResult<Py<PyBytes>> {
    let sid = fixed_array::<32>(sid, "sid")?;
    require_non_blank_unpadded(relay_token, "relay_token")?;
    let relay_auth = connect_sdk::relay_auth_hash(&sid, relay_token);
    Ok(Py::from(PyBytes::new(py, &relay_auth)))
}

#[pyo3::pyfunction]
#[pyo3(name = "verify_connect_approval_signature")]
/// Verify one approval against the exact session identity and account key.
fn verify_connect_approval_signature_py(
    network_id: &PyNetworkId,
    sid: &[u8],
    app_public_key: &[u8],
    nonce: &[u8],
    wallet_public_key: &[u8],
    account_id: &str,
    permissions: Option<&Bound<'_, PyAny>>,
    proof: Option<&Bound<'_, PyAny>>,
    relay_token: &str,
    algorithm: &str,
    signature: &[u8],
) -> PyResult<bool> {
    if algorithm != "ed25519" {
        return Err(PyValueError::new_err(
            "Connect approval algorithm must be exactly `ed25519`",
        ));
    }
    let sid = fixed_array::<32>(sid, "sid")?;
    let app_pk = fixed_array::<32>(app_public_key, "app_public_key")?;
    let nonce = fixed_array::<16>(nonce, "nonce")?;
    let wallet_pk = fixed_array::<32>(wallet_public_key, "wallet_public_key")?;
    if app_pk.iter().all(|byte| *byte == 0) || wallet_pk.iter().all(|byte| *byte == 0) {
        return Err(PyValueError::new_err(
            "Connect public keys must not be all zero",
        ));
    }
    if nonce.iter().all(|byte| *byte == 0) {
        return Err(PyValueError::new_err("nonce must not be all zero"));
    }
    if connect_sdk::derive_session_id(network_id.as_inner(), &app_pk, &nonce) != sid {
        return Err(PyValueError::new_err(
            "sid does not match exact network_id, app_public_key, and nonce",
        ));
    }
    let account = parse_exact_i105_account_id(account_id, "account_id")?;
    if account.to_string() != account_id {
        return Err(PyValueError::new_err(
            "account_id must be an exact canonical I105 account id",
        ));
    }
    ensure_ed25519_account(&account)?;
    let account_signatory = require_single_signatory(&account, "Connect approval")?;
    let permissions = parse_permissions(permissions.cloned(), "permissions")?;
    let proof = parse_sign_in_proof(proof.cloned())?;
    require_non_blank_unpadded(relay_token, "relay_token")?;
    let relay_auth = connect_sdk::relay_auth_hash(&sid, relay_token);
    let signature = WalletSignatureV1::from_ed25519_bytes(signature)
        .ok_or_else(|| PyValueError::new_err("signature must be exactly 64 Ed25519 bytes"))?;
    Ok(connect_sdk::verify_wallet_approval_signature(
        account_signatory,
        &Constraints {
            network_id: *network_id.as_inner(),
        },
        &sid,
        &app_pk,
        &wallet_pk,
        account_id,
        permissions.as_ref(),
        proof.as_ref(),
        &relay_auth,
        &signature,
    )
    .is_ok())
}
