//! Native-only confidential-note derivation shared by the mobile SDKs.
//!
//! The SDKs deliberately do not implement the Pasta/Poseidon transcript. This boundary keeps the
//! complete V3 permutation and its domain constants owned by `iroha_core`, so every language
//! derives byte-identical commitments and nullifiers from the same implementation.
use iroha_core::zk::confidential_v2::{
    CONFIDENTIAL_TREE_CAPACITY_V2, CONFIDENTIAL_TREE_DEPTH_V2, ConfidentialMerklePathV2,
    compute_confidential_merkle_path_v3, default_confidential_diversifier_v2,
    derive_confidential_asset_tag_v3, derive_confidential_diversifier_v2,
    derive_confidential_network_tag_v3, derive_confidential_note_v3,
    derive_confidential_nullifier_v3, derive_confidential_owner_tag_v3_with_diversifier,
    derive_confidential_sequential_append_paths_v3, validate_confidential_membership_path_v3,
    validate_confidential_next_zero_path_v3,
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{NetworkId, block::BlockHeader};
use libc::{c_int, c_uchar, c_ulong};
use std::{ptr, slice, str};
const DIGEST_BYTES: usize = 32;
const MAX_DIVERSIFIER_SEED_BYTES: usize = 4_096;
const MAX_ASSET_DEFINITION_ID_BYTES: usize = 512;
const MAX_U128_DECIMAL_BYTES: usize = 39;
const MAX_COMMITMENT_FRONTIER_BYTES: usize = CONFIDENTIAL_TREE_CAPACITY_V2 * DIGEST_BYTES;
const MERKLE_PATH_BYTES: usize =
    DIGEST_BYTES + CONFIDENTIAL_TREE_DEPTH_V2 * DIGEST_BYTES + CONFIDENTIAL_TREE_DEPTH_V2;
const MERKLE_ADVANCE_BYTES: usize = DIGEST_BYTES + MERKLE_PATH_BYTES;
const ERR_NULL_PTR: c_int = -1;
const ERR_UTF8: c_int = -2;
const ERR_HASH_OUT_LEN: c_int = -11;
const ERR_CONFIDENTIAL_DERIVATION: c_int = -15;
/// Revision of the first-release native confidential-note derivation contract.
pub const CONFIDENTIAL_NOTE_DERIVATION_CONTRACT_REVISION_V3: u32 = 1;
fn fixed_nonzero<const N: usize>(value: &[u8]) -> Result<[u8; N], c_int> {
    let value: [u8; N] = value.try_into().map_err(|_| ERR_CONFIDENTIAL_DERIVATION)?;
    if value == [0; N] {
        return Err(ERR_CONFIDENTIAL_DERIVATION);
    }
    Ok(value)
}
fn canonical_asset(value: &[u8]) -> Result<&str, c_int> {
    if value.is_empty() || value.len() > MAX_ASSET_DEFINITION_ID_BYTES {
        return Err(ERR_CONFIDENTIAL_DERIVATION);
    }
    let value = str::from_utf8(value).map_err(|_| ERR_UTF8)?;
    if value.trim() != value || value.contains('\0') {
        return Err(ERR_CONFIDENTIAL_DERIVATION);
    }
    Ok(value)
}
fn canonical_positive_u128(value: &[u8]) -> Result<u128, c_int> {
    if value.is_empty() || value.len() > MAX_U128_DECIMAL_BYTES {
        return Err(ERR_CONFIDENTIAL_DERIVATION);
    }
    if !value.iter().all(u8::is_ascii_digit) || (value.len() > 1 && value[0] == b'0') {
        return Err(ERR_CONFIDENTIAL_DERIVATION);
    }
    let value = str::from_utf8(value).map_err(|_| ERR_UTF8)?;
    value
        .parse::<u128>()
        .ok()
        .filter(|amount| *amount != 0)
        .ok_or(ERR_CONFIDENTIAL_DERIVATION)
}
fn exact_network_id(value: &[u8]) -> Result<NetworkId, c_int> {
    let value = fixed_nonzero::<DIGEST_BYTES>(value)?;
    if value[DIGEST_BYTES - 1] & 1 != 1 {
        return Err(ERR_CONFIDENTIAL_DERIVATION);
    }
    Ok(NetworkId::from_genesis_hash(
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(value)),
    ))
}
fn derive_default_diversifier_v3() -> [u8; DIGEST_BYTES] {
    default_confidential_diversifier_v2()
}
fn derive_diversifier_v3(seed: &[u8]) -> Result<[u8; DIGEST_BYTES], c_int> {
    if seed.is_empty() || seed.len() > MAX_DIVERSIFIER_SEED_BYTES {
        return Err(ERR_CONFIDENTIAL_DERIVATION);
    }
    let diversifier = derive_confidential_diversifier_v2(seed);
    if diversifier == [0; DIGEST_BYTES] {
        return Err(ERR_CONFIDENTIAL_DERIVATION);
    }
    Ok(diversifier)
}
fn derive_owner_tag_v3(spend_key: &[u8], diversifier: &[u8]) -> Result<[u8; DIGEST_BYTES], c_int> {
    let spend_key = fixed_nonzero::<DIGEST_BYTES>(spend_key)?;
    let diversifier = fixed_nonzero::<DIGEST_BYTES>(diversifier)?;
    derive_confidential_owner_tag_v3_with_diversifier(&spend_key, diversifier)
        .map_err(|_| ERR_CONFIDENTIAL_DERIVATION)
}
fn derive_asset_tag_v3(asset: &[u8]) -> Result<[u8; DIGEST_BYTES], c_int> {
    derive_confidential_asset_tag_v3(canonical_asset(asset)?)
        .map_err(|_| ERR_CONFIDENTIAL_DERIVATION)
}
fn derive_network_tag_v3(network_id: &[u8]) -> Result<[u8; DIGEST_BYTES], c_int> {
    derive_confidential_network_tag_v3(&exact_network_id(network_id)?)
        .map_err(|_| ERR_CONFIDENTIAL_DERIVATION)
}
fn derive_note_commitment_v3(
    asset: &[u8],
    amount: &[u8],
    rho: &[u8],
    owner_tag: &[u8],
) -> Result<[u8; DIGEST_BYTES], c_int> {
    let asset_tag = derive_asset_tag_v3(asset)?;
    let amount = canonical_positive_u128(amount)?;
    let rho = fixed_nonzero::<DIGEST_BYTES>(rho)?;
    let owner_tag = fixed_nonzero::<DIGEST_BYTES>(owner_tag)?;
    derive_confidential_note_v3(asset_tag, amount, rho, owner_tag)
        .map_err(|_| ERR_CONFIDENTIAL_DERIVATION)
}
fn derive_nullifier_v3(
    network_id: &[u8],
    asset: &[u8],
    spend_key: &[u8],
    rho: &[u8],
) -> Result<[u8; DIGEST_BYTES], c_int> {
    let network_tag = derive_network_tag_v3(network_id)?;
    let asset_tag = derive_asset_tag_v3(asset)?;
    let spend_key = fixed_nonzero::<DIGEST_BYTES>(spend_key)?;
    let rho = fixed_nonzero::<DIGEST_BYTES>(rho)?;
    derive_confidential_nullifier_v3(&spend_key, rho, asset_tag, network_tag)
        .map_err(|_| ERR_CONFIDENTIAL_DERIVATION)
}
fn decode_commitments(value: &[u8]) -> Result<Vec<[u8; DIGEST_BYTES]>, c_int> {
    if value.len() > MAX_COMMITMENT_FRONTIER_BYTES || !value.len().is_multiple_of(DIGEST_BYTES) {
        return Err(ERR_CONFIDENTIAL_DERIVATION);
    }
    value
        .chunks_exact(DIGEST_BYTES)
        .map(|chunk| chunk.try_into().map_err(|_| ERR_CONFIDENTIAL_DERIVATION))
        .collect()
}
fn derive_merkle_path_v3(
    commitments: &[u8],
    leaf_index: u64,
) -> Result<[u8; MERKLE_PATH_BYTES], c_int> {
    let commitments = decode_commitments(commitments)?;
    let leaf_index = usize::try_from(leaf_index).map_err(|_| ERR_CONFIDENTIAL_DERIVATION)?;
    if leaf_index > commitments.len() || leaf_index >= CONFIDENTIAL_TREE_CAPACITY_V2 {
        return Err(ERR_CONFIDENTIAL_DERIVATION);
    }
    let path = compute_confidential_merkle_path_v3(&commitments, leaf_index)
        .map_err(|_| ERR_CONFIDENTIAL_DERIVATION)?;
    let mut encoded = [0; MERKLE_PATH_BYTES];
    encoded[..DIGEST_BYTES].copy_from_slice(&path.root);
    let mut offset = DIGEST_BYTES;
    for sibling in &path.siblings {
        encoded[offset..offset + DIGEST_BYTES].copy_from_slice(sibling);
        offset += DIGEST_BYTES;
    }
    encoded[offset..].copy_from_slice(&path.directions);
    Ok(encoded)
}
fn verify_merkle_path_v3(
    commitment: &[u8],
    leaf_index: u64,
    siblings: &[u8],
    directions: &[u8],
    root: &[u8],
) -> Result<(), c_int> {
    let commitment: [u8; DIGEST_BYTES] = commitment
        .try_into()
        .map_err(|_| ERR_CONFIDENTIAL_DERIVATION)?;
    let leaf_index = usize::try_from(leaf_index).map_err(|_| ERR_CONFIDENTIAL_DERIVATION)?;
    if siblings.len() != CONFIDENTIAL_TREE_DEPTH_V2 * DIGEST_BYTES
        || directions.len() != CONFIDENTIAL_TREE_DEPTH_V2
    {
        return Err(ERR_CONFIDENTIAL_DERIVATION);
    }
    let siblings = siblings
        .chunks_exact(DIGEST_BYTES)
        .map(|chunk| chunk.try_into().map_err(|_| ERR_CONFIDENTIAL_DERIVATION))
        .collect::<Result<Vec<_>, _>>()?;
    let root = root.try_into().map_err(|_| ERR_CONFIDENTIAL_DERIVATION)?;
    let path = ConfidentialMerklePathV2 {
        siblings,
        directions: directions.to_vec(),
        witness_nodes: Vec::new(),
        root,
    };
    if commitment == [0; DIGEST_BYTES] {
        validate_confidential_next_zero_path_v3(leaf_index, &path)
    } else {
        validate_confidential_membership_path_v3(commitment, leaf_index, &path)
    }
    .map(|_| ())
    .map_err(|_| ERR_CONFIDENTIAL_DERIVATION)
}
fn advance_merkle_path_v3(
    leaf_index: u64,
    siblings: &[u8],
    directions: &[u8],
    root: &[u8],
    commitment: &[u8],
) -> Result<[u8; MERKLE_ADVANCE_BYTES], c_int> {
    let leaf_index = usize::try_from(leaf_index).map_err(|_| ERR_CONFIDENTIAL_DERIVATION)?;
    let commitment = fixed_nonzero::<DIGEST_BYTES>(commitment)?;
    if siblings.len() != CONFIDENTIAL_TREE_DEPTH_V2 * DIGEST_BYTES
        || directions.len() != CONFIDENTIAL_TREE_DEPTH_V2
    {
        return Err(ERR_CONFIDENTIAL_DERIVATION);
    }
    let siblings = siblings
        .chunks_exact(DIGEST_BYTES)
        .map(|chunk| chunk.try_into().map_err(|_| ERR_CONFIDENTIAL_DERIVATION))
        .collect::<Result<Vec<_>, _>>()?;
    let root = root.try_into().map_err(|_| ERR_CONFIDENTIAL_DERIVATION)?;
    let advanced = derive_confidential_sequential_append_paths_v3(
        leaf_index,
        &ConfidentialMerklePathV2 {
            siblings,
            directions: directions.to_vec(),
            witness_nodes: Vec::new(),
            root,
        },
        &[commitment],
    )
    .map_err(|_| ERR_CONFIDENTIAL_DERIVATION)?;
    let mut encoded = [0; MERKLE_ADVANCE_BYTES];
    encoded[..DIGEST_BYTES].copy_from_slice(&advanced.final_root);
    encoded[DIGEST_BYTES..DIGEST_BYTES * 2].copy_from_slice(&advanced.next_zero_path.root);
    let mut offset = DIGEST_BYTES * 2;
    for sibling in &advanced.next_zero_path.siblings {
        encoded[offset..offset + DIGEST_BYTES].copy_from_slice(sibling);
        offset += DIGEST_BYTES;
    }
    encoded[offset..].copy_from_slice(&advanced.next_zero_path.directions);
    Ok(encoded)
}
unsafe fn input<'a>(value_ptr: *const c_uchar, value_len: c_ulong) -> Result<&'a [u8], c_int> {
    if value_ptr.is_null() {
        return Err(ERR_NULL_PTR);
    }
    let value_len = usize::try_from(value_len).map_err(|_| ERR_CONFIDENTIAL_DERIVATION)?;
    Ok(unsafe { slice::from_raw_parts(value_ptr, value_len) })
}
unsafe fn input_allow_empty<'a>(
    value_ptr: *const c_uchar,
    value_len: c_ulong,
) -> Result<&'a [u8], c_int> {
    if value_len == 0 {
        return Ok(&[]);
    }
    unsafe { input(value_ptr, value_len) }
}
unsafe fn write_digest(
    out_digest_ptr: *mut c_uchar,
    out_digest_len: c_ulong,
    derive: impl FnOnce() -> Result<[u8; DIGEST_BYTES], c_int>,
) -> c_int {
    if out_digest_ptr.is_null() {
        return ERR_NULL_PTR;
    }
    if out_digest_len != DIGEST_BYTES as c_ulong {
        return ERR_HASH_OUT_LEN;
    }
    unsafe { ptr::write_bytes(out_digest_ptr, 0, DIGEST_BYTES) };
    let digest = match derive() {
        Ok(digest) => digest,
        Err(code) => return code,
    };
    unsafe { ptr::copy_nonoverlapping(digest.as_ptr(), out_digest_ptr, DIGEST_BYTES) };
    0
}
unsafe fn write_merkle_path(
    out_path_ptr: *mut c_uchar,
    out_path_len: c_ulong,
    derive: impl FnOnce() -> Result<[u8; MERKLE_PATH_BYTES], c_int>,
) -> c_int {
    if out_path_ptr.is_null() {
        return ERR_NULL_PTR;
    }
    if out_path_len != MERKLE_PATH_BYTES as c_ulong {
        return ERR_HASH_OUT_LEN;
    }
    unsafe { ptr::write_bytes(out_path_ptr, 0, MERKLE_PATH_BYTES) };
    let path = match derive() {
        Ok(path) => path,
        Err(code) => return code,
    };
    unsafe { ptr::copy_nonoverlapping(path.as_ptr(), out_path_ptr, MERKLE_PATH_BYTES) };
    0
}
unsafe fn write_merkle_advance(
    out_ptr: *mut c_uchar,
    out_len: c_ulong,
    derive: impl FnOnce() -> Result<[u8; MERKLE_ADVANCE_BYTES], c_int>,
) -> c_int {
    if out_ptr.is_null() {
        return ERR_NULL_PTR;
    }
    if out_len != MERKLE_ADVANCE_BYTES as c_ulong {
        return ERR_HASH_OUT_LEN;
    }
    unsafe { ptr::write_bytes(out_ptr, 0, MERKLE_ADVANCE_BYTES) };
    let result = match derive() {
        Ok(result) => result,
        Err(code) => return code,
    };
    unsafe { ptr::copy_nonoverlapping(result.as_ptr(), out_ptr, MERKLE_ADVANCE_BYTES) };
    0
}
/// Return the native confidential-note derivation contract revision.
#[unsafe(no_mangle)]
pub extern "C" fn connect_norito_confidential_note_derivation_revision_v3() -> u32 {
    CONFIDENTIAL_NOTE_DERIVATION_CONTRACT_REVISION_V3
}
/// Copy the canonical default V3 owner diversifier into a 32-byte output.
///
/// # Safety
///
/// `out_digest_ptr` must identify `out_digest_len` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_confidential_default_diversifier_v3(
    out_digest_ptr: *mut c_uchar,
    out_digest_len: c_ulong,
) -> c_int {
    unsafe {
        write_digest(out_digest_ptr, out_digest_len, || {
            Ok(derive_default_diversifier_v3())
        })
    }
}
/// Derive a canonical V3 owner diversifier from bounded non-empty seed bytes.
///
/// # Safety
///
/// Input and output pointers must identify their declared readable/writable ranges.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_confidential_diversifier_derive_v3(
    seed_ptr: *const c_uchar,
    seed_len: c_ulong,
    out_digest_ptr: *mut c_uchar,
    out_digest_len: c_ulong,
) -> c_int {
    unsafe {
        write_digest(out_digest_ptr, out_digest_len, || {
            derive_diversifier_v3(input(seed_ptr, seed_len)?)
        })
    }
}
/// Derive a canonical V3 owner tag from an exact spend key and diversifier.
///
/// # Safety
///
/// Input and output pointers must identify their declared readable/writable ranges.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_confidential_owner_tag_derive_v3(
    spend_key_ptr: *const c_uchar,
    spend_key_len: c_ulong,
    diversifier_ptr: *const c_uchar,
    diversifier_len: c_ulong,
    out_digest_ptr: *mut c_uchar,
    out_digest_len: c_ulong,
) -> c_int {
    unsafe {
        write_digest(out_digest_ptr, out_digest_len, || {
            derive_owner_tag_v3(
                input(spend_key_ptr, spend_key_len)?,
                input(diversifier_ptr, diversifier_len)?,
            )
        })
    }
}
/// Derive the canonical V3 asset-domain tag for one UTF-8 identifier.
///
/// # Safety
///
/// Input and output pointers must identify their declared readable/writable ranges.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_confidential_asset_tag_derive_v3(
    asset_ptr: *const c_uchar,
    asset_len: c_ulong,
    out_digest_ptr: *mut c_uchar,
    out_digest_len: c_ulong,
) -> c_int {
    unsafe {
        write_digest(out_digest_ptr, out_digest_len, || {
            derive_asset_tag_v3(input(asset_ptr, asset_len)?)
        })
    }
}
/// Derive the canonical V3 domain tag for an exact 32-byte `NetworkId`.
///
/// # Safety
///
/// Input and output pointers must identify their declared readable/writable ranges.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_confidential_network_tag_derive_v3(
    network_id_ptr: *const c_uchar,
    network_id_len: c_ulong,
    out_digest_ptr: *mut c_uchar,
    out_digest_len: c_ulong,
) -> c_int {
    unsafe {
        write_digest(out_digest_ptr, out_digest_len, || {
            derive_network_tag_v3(input(network_id_ptr, network_id_len)?)
        })
    }
}
/// Derive the canonical V3 note commitment from its exact public opening fields.
///
/// # Safety
///
/// Input and output pointers must identify their declared readable/writable ranges.
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_confidential_note_commitment_derive_v3(
    asset_ptr: *const c_uchar,
    asset_len: c_ulong,
    amount_ptr: *const c_uchar,
    amount_len: c_ulong,
    rho_ptr: *const c_uchar,
    rho_len: c_ulong,
    owner_tag_ptr: *const c_uchar,
    owner_tag_len: c_ulong,
    out_digest_ptr: *mut c_uchar,
    out_digest_len: c_ulong,
) -> c_int {
    unsafe {
        write_digest(out_digest_ptr, out_digest_len, || {
            derive_note_commitment_v3(
                input(asset_ptr, asset_len)?,
                input(amount_ptr, amount_len)?,
                input(rho_ptr, rho_len)?,
                input(owner_tag_ptr, owner_tag_len)?,
            )
        })
    }
}
/// Derive the canonical V3 nullifier bound to an exact 32-byte `NetworkId`.
///
/// # Safety
///
/// Input and output pointers must identify their declared readable/writable ranges.
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_confidential_nullifier_derive_v3(
    network_id_ptr: *const c_uchar,
    network_id_len: c_ulong,
    asset_ptr: *const c_uchar,
    asset_len: c_ulong,
    spend_key_ptr: *const c_uchar,
    spend_key_len: c_ulong,
    rho_ptr: *const c_uchar,
    rho_len: c_ulong,
    out_digest_ptr: *mut c_uchar,
    out_digest_len: c_ulong,
) -> c_int {
    unsafe {
        write_digest(out_digest_ptr, out_digest_len, || {
            derive_nullifier_v3(
                input(network_id_ptr, network_id_len)?,
                input(asset_ptr, asset_len)?,
                input(spend_key_ptr, spend_key_len)?,
                input(rho_ptr, rho_len)?,
            )
        })
    }
}
/// Derive one canonical V3 authentication path from a packed commitment frontier.
///
/// # Safety
///
/// Input and output pointers must identify their declared readable/writable ranges.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_confidential_merkle_path_derive_v3(
    commitments_ptr: *const c_uchar,
    commitments_len: c_ulong,
    leaf_index: u64,
    out_path_ptr: *mut c_uchar,
    out_path_len: c_ulong,
) -> c_int {
    unsafe {
        write_merkle_path(out_path_ptr, out_path_len, || {
            derive_merkle_path_v3(
                input_allow_empty(commitments_ptr, commitments_len)?,
                leaf_index,
            )
        })
    }
}
/// Verify one exact V3 confidential-tree membership path.
///
/// # Safety
///
/// Every pointer must identify its declared readable range.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_confidential_merkle_path_verify_v3(
    commitment_ptr: *const c_uchar,
    commitment_len: c_ulong,
    leaf_index: u64,
    siblings_ptr: *const c_uchar,
    siblings_len: c_ulong,
    directions_ptr: *const c_uchar,
    directions_len: c_ulong,
    root_ptr: *const c_uchar,
    root_len: c_ulong,
) -> c_int {
    let result = (|| unsafe {
        verify_merkle_path_v3(
            input(commitment_ptr, commitment_len)?,
            leaf_index,
            input(siblings_ptr, siblings_len)?,
            input(directions_ptr, directions_len)?,
            input(root_ptr, root_len)?,
        )
    })();
    result.map_or_else(|code| code, |()| 0)
}
/// Advance one authenticated next-zero V3 path by one non-zero commitment.
///
/// Output is `final_root[32] || next_zero_root[32] || siblings[16][32] || directions[16]`.
///
/// # Safety
///
/// Every pointer must identify its declared readable or writable range.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_confidential_merkle_path_advance_v3(
    leaf_index: u64,
    siblings_ptr: *const c_uchar,
    siblings_len: c_ulong,
    directions_ptr: *const c_uchar,
    directions_len: c_ulong,
    root_ptr: *const c_uchar,
    root_len: c_ulong,
    commitment_ptr: *const c_uchar,
    commitment_len: c_ulong,
    out_ptr: *mut c_uchar,
    out_len: c_ulong,
) -> c_int {
    unsafe {
        write_merkle_advance(out_ptr, out_len, || {
            advance_merkle_path_v3(
                leaf_index,
                input(siblings_ptr, siblings_len)?,
                input(directions_ptr, directions_len)?,
                input(root_ptr, root_len)?,
                input(commitment_ptr, commitment_len)?,
            )
        })
    }
}
#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
mod jni_exports {
    use super::*;
    use jni::{
        JNIEnv,
        objects::{JByteArray, JClass},
        sys::{JNI_FALSE, JNI_TRUE, jboolean, jbyteArray, jint, jlong},
    };
    use zeroize::Zeroizing;
    fn read(env: &mut JNIEnv<'_>, value: JByteArray<'_>, maximum: usize) -> Option<Vec<u8>> {
        let length = usize::try_from(env.get_array_length(&value).ok()?).ok()?;
        if length == 0 || length > maximum {
            return None;
        }
        env.convert_byte_array(&value).ok()
    }
    fn read_allow_empty(
        env: &mut JNIEnv<'_>,
        value: JByteArray<'_>,
        maximum: usize,
    ) -> Option<Vec<u8>> {
        let length = usize::try_from(env.get_array_length(&value).ok()?).ok()?;
        if length > maximum {
            return None;
        }
        env.convert_byte_array(&value).ok()
    }
    fn output(env: &mut JNIEnv<'_>, digest: Result<[u8; DIGEST_BYTES], c_int>) -> jbyteArray {
        digest
            .ok()
            .and_then(|value| env.byte_array_from_slice(&value).ok())
            .map_or(ptr::null_mut(), JByteArray::into_raw)
    }
    fn default_diversifier(env: &mut JNIEnv<'_>) -> jbyteArray {
        output(env, Ok(derive_default_diversifier_v3()))
    }
    fn diversifier(env: &mut JNIEnv<'_>, seed: JByteArray<'_>) -> jbyteArray {
        let Some(seed) = read(env, seed, MAX_DIVERSIFIER_SEED_BYTES) else {
            return ptr::null_mut();
        };
        output(env, derive_diversifier_v3(&seed))
    }
    fn owner_tag(
        env: &mut JNIEnv<'_>,
        spend_key: JByteArray<'_>,
        diversifier: JByteArray<'_>,
    ) -> jbyteArray {
        let Some(spend_key) = read(env, spend_key, DIGEST_BYTES) else {
            return ptr::null_mut();
        };
        let spend_key = Zeroizing::new(spend_key);
        let Some(diversifier) = read(env, diversifier, DIGEST_BYTES) else {
            return ptr::null_mut();
        };
        output(env, derive_owner_tag_v3(&spend_key, &diversifier))
    }
    fn asset_tag(env: &mut JNIEnv<'_>, asset: JByteArray<'_>) -> jbyteArray {
        let Some(asset) = read(env, asset, MAX_ASSET_DEFINITION_ID_BYTES) else {
            return ptr::null_mut();
        };
        output(env, derive_asset_tag_v3(&asset))
    }
    fn network_tag(env: &mut JNIEnv<'_>, network_id: JByteArray<'_>) -> jbyteArray {
        let Some(network_id) = read(env, network_id, DIGEST_BYTES) else {
            return ptr::null_mut();
        };
        output(env, derive_network_tag_v3(&network_id))
    }
    fn note_commitment(
        env: &mut JNIEnv<'_>,
        asset: JByteArray<'_>,
        amount: JByteArray<'_>,
        rho: JByteArray<'_>,
        owner_tag: JByteArray<'_>,
    ) -> jbyteArray {
        let Some(asset) = read(env, asset, MAX_ASSET_DEFINITION_ID_BYTES) else {
            return ptr::null_mut();
        };
        let Some(amount) = read(env, amount, MAX_U128_DECIMAL_BYTES) else {
            return ptr::null_mut();
        };
        let Some(rho) = read(env, rho, DIGEST_BYTES) else {
            return ptr::null_mut();
        };
        let Some(owner_tag) = read(env, owner_tag, DIGEST_BYTES) else {
            return ptr::null_mut();
        };
        output(
            env,
            derive_note_commitment_v3(&asset, &amount, &rho, &owner_tag),
        )
    }
    fn nullifier(
        env: &mut JNIEnv<'_>,
        network_id: JByteArray<'_>,
        asset: JByteArray<'_>,
        spend_key: JByteArray<'_>,
        rho: JByteArray<'_>,
    ) -> jbyteArray {
        let Some(network_id) = read(env, network_id, DIGEST_BYTES) else {
            return ptr::null_mut();
        };
        let Some(asset) = read(env, asset, MAX_ASSET_DEFINITION_ID_BYTES) else {
            return ptr::null_mut();
        };
        let Some(spend_key) = read(env, spend_key, DIGEST_BYTES) else {
            return ptr::null_mut();
        };
        let spend_key = Zeroizing::new(spend_key);
        let Some(rho) = read(env, rho, DIGEST_BYTES) else {
            return ptr::null_mut();
        };
        output(
            env,
            derive_nullifier_v3(&network_id, &asset, &spend_key, &rho),
        )
    }
    fn merkle_path(
        env: &mut JNIEnv<'_>,
        commitments: JByteArray<'_>,
        leaf_index: jlong,
    ) -> jbyteArray {
        let Some(commitments) = read_allow_empty(env, commitments, MAX_COMMITMENT_FRONTIER_BYTES)
        else {
            return ptr::null_mut();
        };
        let Ok(leaf_index) = u64::try_from(leaf_index) else {
            return ptr::null_mut();
        };
        derive_merkle_path_v3(&commitments, leaf_index)
            .ok()
            .and_then(|value| env.byte_array_from_slice(&value).ok())
            .map_or(ptr::null_mut(), JByteArray::into_raw)
    }
    fn verify_merkle_path(
        env: &mut JNIEnv<'_>,
        commitment: JByteArray<'_>,
        leaf_index: jlong,
        siblings: JByteArray<'_>,
        directions: JByteArray<'_>,
        root: JByteArray<'_>,
    ) -> jboolean {
        let Some(commitment) = read(env, commitment, DIGEST_BYTES) else {
            return JNI_FALSE;
        };
        let Ok(leaf_index) = u64::try_from(leaf_index) else {
            return JNI_FALSE;
        };
        let Some(siblings) = read(env, siblings, CONFIDENTIAL_TREE_DEPTH_V2 * DIGEST_BYTES) else {
            return JNI_FALSE;
        };
        let Some(directions) = read(env, directions, CONFIDENTIAL_TREE_DEPTH_V2) else {
            return JNI_FALSE;
        };
        let Some(root) = read(env, root, DIGEST_BYTES) else {
            return JNI_FALSE;
        };
        if verify_merkle_path_v3(&commitment, leaf_index, &siblings, &directions, &root).is_ok() {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    }
    macro_rules! revision_export {
        ($name:ident) => {
            #[allow(clippy::missing_safety_doc)]
            #[unsafe(no_mangle)]
            pub unsafe extern "system" fn $name(_env: JNIEnv<'_>, _class: JClass<'_>) -> jint {
                CONFIDENTIAL_NOTE_DERIVATION_CONTRACT_REVISION_V3 as jint
            }
        };
    }
    macro_rules! default_export {
        ($name:ident) => {
            #[allow(clippy::missing_safety_doc)]
            #[unsafe(no_mangle)]
            pub unsafe extern "system" fn $name(
                mut env: JNIEnv<'_>,
                _class: JClass<'_>,
            ) -> jbyteArray {
                default_diversifier(&mut env)
            }
        };
    }
    macro_rules! one_input_export {
        ($name:ident, $derive:ident) => {
            #[allow(clippy::missing_safety_doc)]
            #[unsafe(no_mangle)]
            pub unsafe extern "system" fn $name(
                mut env: JNIEnv<'_>,
                _class: JClass<'_>,
                value: JByteArray<'_>,
            ) -> jbyteArray {
                $derive(&mut env, value)
            }
        };
    }
    macro_rules! two_input_export {
        ($name:ident, $derive:ident) => {
            #[allow(clippy::missing_safety_doc)]
            #[unsafe(no_mangle)]
            pub unsafe extern "system" fn $name(
                mut env: JNIEnv<'_>,
                _class: JClass<'_>,
                first: JByteArray<'_>,
                second: JByteArray<'_>,
            ) -> jbyteArray {
                $derive(&mut env, first, second)
            }
        };
    }
    macro_rules! four_input_export {
        ($name:ident, $derive:ident) => {
            #[allow(clippy::missing_safety_doc)]
            #[unsafe(no_mangle)]
            pub unsafe extern "system" fn $name(
                mut env: JNIEnv<'_>,
                _class: JClass<'_>,
                first: JByteArray<'_>,
                second: JByteArray<'_>,
                third: JByteArray<'_>,
                fourth: JByteArray<'_>,
            ) -> jbyteArray {
                $derive(&mut env, first, second, third, fourth)
            }
        };
    }
    macro_rules! merkle_path_export {
        ($derive_name:ident, $verify_name:ident) => {
            #[allow(clippy::missing_safety_doc)]
            #[unsafe(no_mangle)]
            pub unsafe extern "system" fn $derive_name(
                mut env: JNIEnv<'_>,
                _class: JClass<'_>,
                commitments: JByteArray<'_>,
                leaf_index: jlong,
            ) -> jbyteArray {
                merkle_path(&mut env, commitments, leaf_index)
            }
            #[allow(clippy::missing_safety_doc)]
            #[unsafe(no_mangle)]
            pub unsafe extern "system" fn $verify_name(
                mut env: JNIEnv<'_>,
                _class: JClass<'_>,
                commitment: JByteArray<'_>,
                leaf_index: jlong,
                siblings: JByteArray<'_>,
                directions: JByteArray<'_>,
                root: JByteArray<'_>,
            ) -> jboolean {
                verify_merkle_path(&mut env, commitment, leaf_index, siblings, directions, root)
            }
        };
    }
    revision_export!(Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeConfidentialDerivationContractRevisionV3);
    default_export!(Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeDefaultConfidentialDiversifierV3);
    one_input_export!(Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeDeriveConfidentialDiversifierV3, diversifier);
    two_input_export!(Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeDeriveConfidentialOwnerTagV3, owner_tag);
    one_input_export!(Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeDeriveConfidentialAssetTagV3, asset_tag);
    one_input_export!(Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeDeriveConfidentialNetworkTagV3, network_tag);
    four_input_export!(Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeDeriveConfidentialNoteCommitmentV3, note_commitment);
    four_input_export!(Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeDeriveConfidentialNullifierV3, nullifier);
    merkle_path_export!(
        Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeDeriveConfidentialMerklePathV3,
        Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeVerifyConfidentialMerklePathV3
    );
    revision_export!(Java_org_hyperledger_iroha_android_privacy_PrivacyNativeBridge_nativeConfidentialDerivationContractRevisionV3);
    default_export!(Java_org_hyperledger_iroha_android_privacy_PrivacyNativeBridge_nativeDefaultConfidentialDiversifierV3);
    one_input_export!(Java_org_hyperledger_iroha_android_privacy_PrivacyNativeBridge_nativeDeriveConfidentialDiversifierV3, diversifier);
    two_input_export!(Java_org_hyperledger_iroha_android_privacy_PrivacyNativeBridge_nativeDeriveConfidentialOwnerTagV3, owner_tag);
    one_input_export!(Java_org_hyperledger_iroha_android_privacy_PrivacyNativeBridge_nativeDeriveConfidentialAssetTagV3, asset_tag);
    one_input_export!(Java_org_hyperledger_iroha_android_privacy_PrivacyNativeBridge_nativeDeriveConfidentialNetworkTagV3, network_tag);
    four_input_export!(Java_org_hyperledger_iroha_android_privacy_PrivacyNativeBridge_nativeDeriveConfidentialNoteCommitmentV3, note_commitment);
    four_input_export!(Java_org_hyperledger_iroha_android_privacy_PrivacyNativeBridge_nativeDeriveConfidentialNullifierV3, nullifier);
    merkle_path_export!(
        Java_org_hyperledger_iroha_android_privacy_PrivacyNativeBridge_nativeDeriveConfidentialMerklePathV3,
        Java_org_hyperledger_iroha_android_privacy_PrivacyNativeBridge_nativeVerifyConfidentialMerklePathV3
    );
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_v3_derivations_are_exact_network_separated() {
        let spend_key = [0x11; 32];
        let rho = [0x22; 32];
        let first_network = [0x33; 32];
        let second_network = [0x35; 32];
        let diversifier = derive_diversifier_v3(b"recipient").expect("diversifier");
        let owner = derive_owner_tag_v3(&spend_key, &diversifier).expect("owner tag");
        let asset = derive_asset_tag_v3(b"rose#wonderland").expect("asset tag");
        let note = derive_note_commitment_v3(b"rose#wonderland", b"7", &rho, &owner)
            .expect("note commitment");
        let first_nullifier =
            derive_nullifier_v3(&first_network, b"rose#wonderland", &spend_key, &rho)
                .expect("first nullifier");
        let second_nullifier =
            derive_nullifier_v3(&second_network, b"rose#wonderland", &spend_key, &rho)
                .expect("second nullifier");
        for digest in [
            diversifier,
            owner,
            asset,
            note,
            first_nullifier,
            second_nullifier,
        ] {
            assert_ne!(digest, [0; 32]);
        }
        assert_ne!(first_nullifier, second_nullifier);
        assert_ne!(
            derive_network_tag_v3(&first_network).expect("first network tag"),
            derive_network_tag_v3(&second_network).expect("second network tag")
        );
    }
    #[test]
    fn invalid_or_ambiguous_v3_inputs_are_rejected() {
        assert!(derive_diversifier_v3(&[]).is_err());
        assert!(derive_owner_tag_v3(&[0; 32], &derive_default_diversifier_v3()).is_err());
        assert!(derive_asset_tag_v3(b" rose#wonderland").is_err());
        assert!(derive_network_tag_v3(&[0; 32]).is_err());
        let mut unmarked_network = [0x33; 32];
        unmarked_network[DIGEST_BYTES - 1] &= !1;
        assert!(derive_network_tag_v3(&unmarked_network).is_err());
        assert!(
            derive_note_commitment_v3(b"rose#wonderland", b"0", &[0x22; 32], &[0x11; 32]).is_err()
        );
        assert!(
            derive_note_commitment_v3(b"rose#wonderland", b"07", &[0x22; 32], &[0x11; 32]).is_err()
        );
    }
    #[test]
    fn native_merkle_paths_are_canonical_and_fail_closed() {
        let first = derive_note_commitment_v3(
            b"rose#wonderland",
            b"7",
            &[0x22; 32],
            &derive_owner_tag_v3(&[0x11; 32], &derive_default_diversifier_v3()).expect("owner tag"),
        )
        .expect("commitment");
        let second = derive_note_commitment_v3(
            b"rose#wonderland",
            b"8",
            &[0x23; 32],
            &derive_owner_tag_v3(&[0x12; 32], &derive_default_diversifier_v3())
                .expect("second owner tag"),
        )
        .expect("second commitment");
        let encoded = derive_merkle_path_v3(&[first, second].concat(), 1).expect("path");
        let root = &encoded[..DIGEST_BYTES];
        let siblings_end = DIGEST_BYTES + CONFIDENTIAL_TREE_DEPTH_V2 * DIGEST_BYTES;
        let siblings = &encoded[DIGEST_BYTES..siblings_end];
        let directions = &encoded[siblings_end..];
        assert!(verify_merkle_path_v3(&second, 1, siblings, directions, root).is_ok());
        let mut wrong_root = root.to_vec();
        wrong_root[0] ^= 1;
        assert!(verify_merkle_path_v3(&second, 1, siblings, directions, &wrong_root).is_err());
        let zero_path = derive_merkle_path_v3(&[first, second].concat(), 2).expect("zero path");
        let zero_root = &zero_path[..DIGEST_BYTES];
        let zero_siblings = &zero_path[DIGEST_BYTES..siblings_end];
        let zero_directions = &zero_path[siblings_end..];
        assert!(
            verify_merkle_path_v3(&[0; 32], 2, zero_siblings, zero_directions, zero_root).is_ok()
        );
        let third = derive_note_commitment_v3(
            b"rose#wonderland",
            b"9",
            &[0x24; 32],
            &derive_owner_tag_v3(&[0x13; 32], &derive_default_diversifier_v3())
                .expect("third owner tag"),
        )
        .expect("third commitment");
        let advanced = advance_merkle_path_v3(2, zero_siblings, zero_directions, zero_root, &third)
            .expect("advance");
        assert_eq!(
            &advanced[..DIGEST_BYTES],
            &derive_merkle_path_v3(&[first, second, third].concat(), 0).expect("advanced root")
                [..DIGEST_BYTES]
        );
        let next_root = &advanced[DIGEST_BYTES..DIGEST_BYTES * 2];
        let next_siblings_end = DIGEST_BYTES * 2 + CONFIDENTIAL_TREE_DEPTH_V2 * DIGEST_BYTES;
        let next_siblings = &advanced[DIGEST_BYTES * 2..next_siblings_end];
        let next_directions = &advanced[next_siblings_end..];
        assert!(
            verify_merkle_path_v3(&[0; 32], 3, next_siblings, next_directions, next_root).is_ok()
        );
        assert!(derive_merkle_path_v3(&[first, second].concat(), 3).is_err());
    }
}
