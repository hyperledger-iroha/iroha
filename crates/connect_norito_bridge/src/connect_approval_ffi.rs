//! Exact-network Connect identity and approval C ABI bindings.

use std::ptr;

use iroha_crypto::{Algorithm, PublicKey, Signature};
use iroha_data_model::{NetworkId, account::AccountId};
use iroha_torii_shared::{connect as proto, connect_sdk};
use libc::{c_char, c_int, c_uchar, c_ulong, malloc};

use super::{
    ERR_CONNECT_APPROVAL, ERR_CONNECT_IDENTITY, network_id_from_raw_bytes, parse_permissions_bytes,
    parse_proof_bytes,
};

pub(super) fn parse_connect_wallet_signature_algorithm_label(
    alg_str: &str,
) -> Result<Algorithm, c_int> {
    if alg_str != "ed25519" {
        return Err(-8);
    }
    Ok(Algorithm::Ed25519)
}

pub(super) fn connect_wallet_signature_from_algorithm_bytes(
    algorithm: Algorithm,
    signature: &[u8],
) -> Option<proto::WalletSignatureV1> {
    connect_signature_from_algorithm_bytes(algorithm, signature)
        .map(|signature| proto::WalletSignatureV1::new(algorithm, signature))
}

pub(super) fn connect_signature_from_algorithm_bytes(
    algorithm: Algorithm,
    signature: &[u8],
) -> Option<Signature> {
    match algorithm {
        Algorithm::Ed25519 => iroha_crypto::ed25519_parse_signature(signature).ok(),
        Algorithm::MlDsa => iroha_crypto::mldsa65_parse_signature(signature).ok(),
        _ => Signature::try_from_bytes(signature).ok(),
    }
}

pub(super) unsafe fn parse_algorithm_cstr(
    alg_ptr: *const c_char,
    alg_len: c_ulong,
) -> Result<Algorithm, c_int> {
    if alg_ptr.is_null() {
        return Err(-6);
    }
    let bytes = unsafe { std::slice::from_raw_parts(alg_ptr as *const u8, alg_len as usize) };
    let alg_str = std::str::from_utf8(bytes).map_err(|_| -7)?;
    parse_connect_wallet_signature_algorithm_label(alg_str)
}

pub(super) fn validate_exact_connect_identity(
    network_id_bytes: &[u8],
    sid: &[u8],
    app_public_key: &[u8],
    nonce: &[u8],
) -> Result<(NetworkId, [u8; 32], [u8; 32], [u8; 16]), c_int> {
    let network_id =
        network_id_from_raw_bytes(network_id_bytes).map_err(|_| ERR_CONNECT_IDENTITY)?;
    let sid: [u8; 32] = sid.try_into().map_err(|_| ERR_CONNECT_IDENTITY)?;
    let app_public_key: [u8; 32] = app_public_key
        .try_into()
        .map_err(|_| ERR_CONNECT_IDENTITY)?;
    let nonce: [u8; 16] = nonce.try_into().map_err(|_| ERR_CONNECT_IDENTITY)?;
    if app_public_key.iter().all(|byte| *byte == 0) || nonce.iter().all(|byte| *byte == 0) {
        return Err(ERR_CONNECT_IDENTITY);
    }
    if connect_sdk::derive_session_id(&network_id, &app_public_key, &nonce) != sid {
        return Err(ERR_CONNECT_IDENTITY);
    }
    Ok((network_id, sid, app_public_key, nonce))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_connect_derive_session_id(
    network_id_ptr: *const c_uchar,
    network_id_len: c_ulong,
    app_pk_ptr: *const c_uchar,
    app_pk_len: c_ulong,
    nonce_ptr: *const c_uchar,
    nonce_len: c_ulong,
    out_sid_ptr: *mut c_uchar,
    out_sid_len: c_ulong,
) -> c_int {
    unsafe {
        if network_id_ptr.is_null()
            || app_pk_ptr.is_null()
            || nonce_ptr.is_null()
            || out_sid_ptr.is_null()
        {
            return -1;
        }
        if network_id_len != 32 || app_pk_len != 32 || nonce_len != 16 || out_sid_len != 32 {
            return ERR_CONNECT_IDENTITY;
        }
        let network_id = match network_id_from_raw_bytes(std::slice::from_raw_parts(
            network_id_ptr,
            network_id_len as usize,
        )) {
            Ok(network_id) => network_id,
            Err(_) => return ERR_CONNECT_IDENTITY,
        };
        let app_pk = std::slice::from_raw_parts(app_pk_ptr, app_pk_len as usize);
        let nonce = std::slice::from_raw_parts(nonce_ptr, nonce_len as usize);
        if app_pk.iter().all(|byte| *byte == 0) || nonce.iter().all(|byte| *byte == 0) {
            return ERR_CONNECT_IDENTITY;
        }
        let app_pk: [u8; 32] = app_pk.try_into().expect("length checked");
        let nonce: [u8; 16] = nonce.try_into().expect("length checked");
        let sid = connect_sdk::derive_session_id(&network_id, &app_pk, &nonce);
        ptr::copy_nonoverlapping(sid.as_ptr(), out_sid_ptr, sid.len());
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_connect_relay_auth_hash(
    sid_ptr: *const c_uchar,
    sid_len: c_ulong,
    relay_token_ptr: *const c_char,
    relay_token_len: c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    unsafe {
        if sid_ptr.is_null() || relay_token_ptr.is_null() || out_hash_ptr.is_null() {
            return -1;
        }
        if sid_len != 32 || out_hash_len != 32 || relay_token_len == 0 {
            return ERR_CONNECT_APPROVAL;
        }
        let sid: [u8; 32] = std::slice::from_raw_parts(sid_ptr, sid_len as usize)
            .try_into()
            .expect("length checked");
        let relay_token = match std::str::from_utf8(std::slice::from_raw_parts(
            relay_token_ptr as *const u8,
            relay_token_len as usize,
        )) {
            Ok(value) if !value.is_empty() && value.trim() == value => value,
            _ => return ERR_CONNECT_APPROVAL,
        };
        let relay_auth = connect_sdk::relay_auth_hash(&sid, relay_token);
        ptr::copy_nonoverlapping(relay_auth.as_ptr(), out_hash_ptr, relay_auth.len());
        0
    }
}

struct ParsedConnectApprovalInputs {
    network_id: NetworkId,
    sid: [u8; 32],
    app_public_key: [u8; 32],
    wallet_public_key: [u8; 32],
    account_id: String,
    account_signatory: PublicKey,
    permissions: Option<proto::PermissionsV1>,
    proof: Option<proto::SignInProofV1>,
    relay_auth: [u8; 32],
}

#[allow(clippy::too_many_arguments)]
unsafe fn parse_connect_approval_inputs(
    network_id_ptr: *const c_uchar,
    network_id_len: c_ulong,
    sid_ptr: *const c_uchar,
    sid_len: c_ulong,
    app_pk_ptr: *const c_uchar,
    app_pk_len: c_ulong,
    nonce_ptr: *const c_uchar,
    nonce_len: c_ulong,
    wallet_pk_ptr: *const c_uchar,
    wallet_pk_len: c_ulong,
    account_id_ptr: *const c_char,
    account_id_len: c_ulong,
    permissions_ptr: *const c_uchar,
    permissions_len: c_ulong,
    proof_ptr: *const c_uchar,
    proof_len: c_ulong,
    relay_token_ptr: *const c_char,
    relay_token_len: c_ulong,
) -> Result<ParsedConnectApprovalInputs, c_int> {
    if network_id_ptr.is_null()
        || sid_ptr.is_null()
        || app_pk_ptr.is_null()
        || nonce_ptr.is_null()
        || wallet_pk_ptr.is_null()
        || account_id_ptr.is_null()
        || relay_token_ptr.is_null()
        || (permissions_len > 0 && permissions_ptr.is_null())
        || (proof_len > 0 && proof_ptr.is_null())
    {
        return Err(-1);
    }
    if network_id_len != 32
        || sid_len != 32
        || app_pk_len != 32
        || nonce_len != 16
        || wallet_pk_len != 32
        || account_id_len == 0
        || relay_token_len == 0
    {
        return Err(ERR_CONNECT_APPROVAL);
    }
    let (network_id, sid, app_public_key, _) = validate_exact_connect_identity(
        unsafe { std::slice::from_raw_parts(network_id_ptr, network_id_len as usize) },
        unsafe { std::slice::from_raw_parts(sid_ptr, sid_len as usize) },
        unsafe { std::slice::from_raw_parts(app_pk_ptr, app_pk_len as usize) },
        unsafe { std::slice::from_raw_parts(nonce_ptr, nonce_len as usize) },
    )?;
    let wallet_public_key: [u8; 32] =
        unsafe { std::slice::from_raw_parts(wallet_pk_ptr, wallet_pk_len as usize) }
            .try_into()
            .expect("length checked");
    if wallet_public_key.iter().all(|byte| *byte == 0) {
        return Err(ERR_CONNECT_APPROVAL);
    }
    let account_id = std::str::from_utf8(unsafe {
        std::slice::from_raw_parts(account_id_ptr as *const u8, account_id_len as usize)
    })
    .map_err(|_| ERR_CONNECT_APPROVAL)?;
    let account = account_id
        .parse::<AccountId>()
        .map_err(|_| ERR_CONNECT_APPROVAL)?;
    if account.to_string() != account_id {
        return Err(ERR_CONNECT_APPROVAL);
    }
    let account_signatory = account
        .try_signatory()
        .filter(|key| key.try_algorithm().ok() == Some(Algorithm::Ed25519))
        .cloned()
        .ok_or(ERR_CONNECT_APPROVAL)?;
    let permissions = unsafe { parse_permissions_bytes(permissions_ptr, permissions_len) }
        .map_err(|_| ERR_CONNECT_APPROVAL)?;
    let proof =
        unsafe { parse_proof_bytes(proof_ptr, proof_len) }.map_err(|_| ERR_CONNECT_APPROVAL)?;
    let relay_token = std::str::from_utf8(unsafe {
        std::slice::from_raw_parts(relay_token_ptr as *const u8, relay_token_len as usize)
    })
    .map_err(|_| ERR_CONNECT_APPROVAL)?;
    if relay_token.is_empty() || relay_token.trim() != relay_token {
        return Err(ERR_CONNECT_APPROVAL);
    }
    Ok(ParsedConnectApprovalInputs {
        network_id,
        sid,
        app_public_key,
        wallet_public_key,
        account_id: account_id.to_owned(),
        account_signatory,
        permissions,
        proof,
        relay_auth: connect_sdk::relay_auth_hash(&sid, relay_token),
    })
}

#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn connect_norito_connect_approval_preimage(
    network_id_ptr: *const c_uchar,
    network_id_len: c_ulong,
    sid_ptr: *const c_uchar,
    sid_len: c_ulong,
    app_pk_ptr: *const c_uchar,
    app_pk_len: c_ulong,
    nonce_ptr: *const c_uchar,
    nonce_len: c_ulong,
    wallet_pk_ptr: *const c_uchar,
    wallet_pk_len: c_ulong,
    account_id_ptr: *const c_char,
    account_id_len: c_ulong,
    permissions_ptr: *const c_uchar,
    permissions_len: c_ulong,
    proof_ptr: *const c_uchar,
    proof_len: c_ulong,
    relay_token_ptr: *const c_char,
    relay_token_len: c_ulong,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if out_ptr.is_null() || out_len.is_null() {
            return -1;
        }
        *out_ptr = ptr::null_mut();
        *out_len = 0;
        let inputs = match parse_connect_approval_inputs(
            network_id_ptr,
            network_id_len,
            sid_ptr,
            sid_len,
            app_pk_ptr,
            app_pk_len,
            nonce_ptr,
            nonce_len,
            wallet_pk_ptr,
            wallet_pk_len,
            account_id_ptr,
            account_id_len,
            permissions_ptr,
            permissions_len,
            proof_ptr,
            proof_len,
            relay_token_ptr,
            relay_token_len,
        ) {
            Ok(inputs) => inputs,
            Err(code) => return code,
        };
        let preimage = connect_sdk::build_approve_preimage(
            &proto::Constraints {
                network_id: inputs.network_id,
            },
            &inputs.sid,
            &inputs.app_public_key,
            &inputs.wallet_public_key,
            &inputs.account_id,
            inputs.permissions.as_ref(),
            inputs.proof.as_ref(),
            &inputs.relay_auth,
        );
        let mem = malloc(preimage.len());
        if mem.is_null() {
            return -5;
        }
        ptr::copy_nonoverlapping(preimage.as_ptr(), mem as *mut u8, preimage.len());
        *out_ptr = mem as *mut u8;
        *out_len = preimage.len() as c_ulong;
        0
    }
}

#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn connect_norito_connect_verify_approval(
    network_id_ptr: *const c_uchar,
    network_id_len: c_ulong,
    sid_ptr: *const c_uchar,
    sid_len: c_ulong,
    app_pk_ptr: *const c_uchar,
    app_pk_len: c_ulong,
    nonce_ptr: *const c_uchar,
    nonce_len: c_ulong,
    wallet_pk_ptr: *const c_uchar,
    wallet_pk_len: c_ulong,
    account_id_ptr: *const c_char,
    account_id_len: c_ulong,
    permissions_ptr: *const c_uchar,
    permissions_len: c_ulong,
    proof_ptr: *const c_uchar,
    proof_len: c_ulong,
    relay_token_ptr: *const c_char,
    relay_token_len: c_ulong,
    algorithm_ptr: *const c_char,
    algorithm_len: c_ulong,
    signature_ptr: *const c_uchar,
    signature_len: c_ulong,
) -> c_int {
    unsafe {
        if algorithm_ptr.is_null() || signature_ptr.is_null() {
            return -1;
        }
        let inputs = match parse_connect_approval_inputs(
            network_id_ptr,
            network_id_len,
            sid_ptr,
            sid_len,
            app_pk_ptr,
            app_pk_len,
            nonce_ptr,
            nonce_len,
            wallet_pk_ptr,
            wallet_pk_len,
            account_id_ptr,
            account_id_len,
            permissions_ptr,
            permissions_len,
            proof_ptr,
            proof_len,
            relay_token_ptr,
            relay_token_len,
        ) {
            Ok(inputs) => inputs,
            Err(code) => return code,
        };
        let algorithm = match std::str::from_utf8(std::slice::from_raw_parts(
            algorithm_ptr as *const u8,
            algorithm_len as usize,
        )) {
            Ok("ed25519") => Algorithm::Ed25519,
            _ => return ERR_CONNECT_APPROVAL,
        };
        let signature_bytes = std::slice::from_raw_parts(signature_ptr, signature_len as usize);
        let signature =
            match connect_wallet_signature_from_algorithm_bytes(algorithm, signature_bytes) {
                Some(signature) => signature,
                None => return ERR_CONNECT_APPROVAL,
            };
        match connect_sdk::verify_wallet_approval_signature(
            &inputs.account_signatory,
            &proto::Constraints {
                network_id: inputs.network_id,
            },
            &inputs.sid,
            &inputs.app_public_key,
            &inputs.wallet_public_key,
            &inputs.account_id,
            inputs.permissions.as_ref(),
            inputs.proof.as_ref(),
            &inputs.relay_auth,
            &signature,
        ) {
            Ok(()) => 0,
            Err(_) => ERR_CONNECT_APPROVAL,
        }
    }
}
