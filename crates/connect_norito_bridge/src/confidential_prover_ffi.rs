//! Local wallet proving with opaque, retained Core key ownership for mobile callers.
//!
//! This C/JNI boundary marshals typed arguments; it does not introduce a private wire codec.
//! Only the public result uses canonical Norito JSON. A proof never authorizes ledger mutation.

use iroha_core::zk::{
    ProofRelation,
    confidential::{
        ConfidentialProof, ConfidentialProver, ConfidentialProverError, ConfidentialTree,
    },
    confidential_v2::{
        ConfidentialMerklePathV2, ConfidentialTransferInputV2, ConfidentialTransferOutputV2,
        ConfidentialUnshieldInputV2, ConfidentialUnshieldOutputV3,
    },
};
use iroha_data_model::asset::AssetDefinitionId;
use libc::{c_int, c_uchar, c_ulong};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, OnceLock},
};
use zeroize::Zeroizing;

const MAX_OWNERS: usize = 64;
const MAX_TREE_BYTES: usize = 65_536 * 32;
const MAX_RESULT_BYTES: usize = 16 * 1024 * 1024;
pub(super) const INVALID: c_int = -1;
pub(super) const CLOSED: c_int = -2;
pub(super) const RESOURCE: c_int = -3;
pub(super) const INTERNAL: c_int = -100;
type Result<T> = std::result::Result<T, c_int>;

struct Job {
    prover: Arc<ConfidentialProver>,
    operation: u8,
    root: [u8; 32],
    public_amount: u128,
    inputs: Vec<ConfidentialTransferInputV2>,
    outputs: Vec<ConfidentialTransferOutputV2>,
    evidence: Option<Evidence>,
}
enum Evidence {
    Commitments(Vec<[u8; 32]>),
    Paths(Vec<ConfidentialMerklePathV2>),
}
#[derive(Default)]
struct Registry {
    next: u64,
    provers: BTreeMap<u64, Arc<ConfidentialProver>>,
    jobs: BTreeMap<u64, Job>,
}
impl Registry {
    fn next(&mut self) -> Result<u64> {
        // Handles never repeat, even after close. Stay positive across Java's signed ABI.
        self.next = self
            .next
            .checked_add(1)
            .filter(|n| *n <= i64::MAX as u64)
            .ok_or(RESOURCE)?;
        Ok(self.next)
    }
}
fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(Registry::default()))
}
fn locked<T>(f: impl FnOnce(&mut Registry) -> Result<T>) -> Result<T> {
    let mut registry = registry().lock().map_err(|_| INTERNAL)?;
    f(&mut registry)
}
fn word(bytes: &[u8]) -> Result<[u8; 32]> {
    bytes.try_into().map_err(|_| INVALID)
}
fn amount(lo: u64, hi: u64) -> u128 {
    u128::from(lo) | (u128::from(hi) << 64)
}
fn native_error(error: ConfidentialProverError) -> c_int {
    use ConfidentialProverError as E;
    match error {
        E::InvalidSpendKey => -10,
        E::InputCount => -11,
        E::TreeCapacity => -12,
        E::PathCount => -13,
        E::InvalidPath => -14,
        E::InputIndex => -15,
        E::PathIndexMismatch => -16,
        E::DuplicateInput => -17,
        E::OutputCount => -18,
        E::InvalidTransferAmounts => -19,
        E::InvalidInputAmounts => -20,
        E::InvalidPublicAmount => -21,
        E::InvalidChange => -22,
        E::KeyPreparation(_) => -23,
        E::Proving(_) => -24,
    }
}
pub(super) fn create(network: &[u8], asset: &[u8], key: &[u8]) -> Result<u64> {
    if asset.is_empty() || asset.len() > 512 || key.len() != 32 {
        return Err(INVALID);
    }
    // Own the only copied key before subsequent fallible parsing.
    let mut key_owner = Zeroizing::new([0; 32]);
    key_owner.copy_from_slice(key);
    let network = super::network_id_from_raw_bytes(network).map_err(|_| INVALID)?;
    let text = std::str::from_utf8(asset).map_err(|_| INVALID)?;
    let asset: AssetDefinitionId = text.parse().map_err(|_| INVALID)?;
    if asset.to_string() != text {
        return Err(INVALID);
    }
    let prover =
        Arc::new(ConfidentialProver::new(network, &asset, key_owner).map_err(native_error)?);
    locked(|registry| {
        if registry.provers.len() >= MAX_OWNERS {
            return Err(RESOURCE);
        }
        let id = registry.next()?;
        registry.provers.insert(id, prover);
        Ok(id)
    })
}
pub(super) fn close(id: u64) -> Result<()> {
    let owner = locked(|registry| registry.provers.remove(&id).ok_or(CLOSED))?;
    drop(owner);
    Ok(())
}
pub(super) fn job_create(id: u64, operation: u8, root: &[u8], lo: u64, hi: u64) -> Result<u64> {
    if operation > 1 || (operation == 0 && (lo != 0 || hi != 0)) {
        return Err(INVALID);
    }
    let root = word(root)?;
    locked(|registry| {
        let prover = registry.provers.get(&id).cloned().ok_or(CLOSED)?;
        if registry.jobs.len() >= MAX_OWNERS {
            return Err(RESOURCE);
        }
        let job_id = registry.next()?;
        registry.jobs.insert(
            job_id,
            Job {
                prover,
                operation,
                root,
                public_amount: amount(lo, hi),
                inputs: Vec::with_capacity(2),
                outputs: Vec::with_capacity(2),
                evidence: None,
            },
        );
        Ok(job_id)
    })
}
pub(super) fn job_input(
    id: u64,
    lo: u64,
    hi: u64,
    rho: &[u8],
    diversifier: &[u8],
    index: u64,
) -> Result<()> {
    let mut input = ConfidentialTransferInputV2 {
        amount: amount(lo, hi),
        rho: [0; 32],
        diversifier: [0; 32],
        leaf_index: 0,
    };
    input.rho = word(rho)?;
    input.diversifier = word(diversifier)?;
    input.leaf_index = usize::try_from(index).map_err(|_| INVALID)?;
    if index >= 65_536 {
        return Err(-15);
    }
    locked(|registry| {
        let job = registry.jobs.get_mut(&id).ok_or(CLOSED)?;
        if job.evidence.is_some() {
            return Err(INVALID);
        }
        if job.inputs.len() >= 2 {
            return Err(-11);
        }
        job.inputs.push(input);
        Ok(())
    })
}
pub(super) fn job_output(id: u64, lo: u64, hi: u64, rho: &[u8], owner: &[u8]) -> Result<()> {
    let mut output = ConfidentialTransferOutputV2 {
        amount: amount(lo, hi),
        rho: [0; 32],
        owner_tag: [0; 32],
    };
    output.rho = word(rho)?;
    locked(|registry| {
        let job = registry.jobs.get_mut(&id).ok_or(CLOSED)?;
        if job.evidence.is_some() {
            return Err(INVALID);
        }
        if job.operation == 0 {
            output.owner_tag = word(owner)?;
            if job.outputs.len() >= 2 {
                return Err(-18);
            }
        } else if !owner.is_empty() {
            return Err(INVALID);
        } else if !job.outputs.is_empty() {
            return Err(-22);
        }
        job.outputs.push(output);
        Ok(())
    })
}
pub(super) fn job_commitments(id: u64, bytes: &[u8]) -> Result<()> {
    if bytes.len() > MAX_TREE_BYTES || !bytes.len().is_multiple_of(32) {
        return Err(-12);
    }
    locked(|registry| {
        let job = registry.jobs.get_mut(&id).ok_or(CLOSED)?;
        if job.evidence.is_some() {
            return Err(INVALID);
        }
        if !(1..=2).contains(&job.inputs.len()) {
            return Err(-11);
        }
        if job
            .inputs
            .iter()
            .any(|note| note.leaf_index >= bytes.len() / 32)
        {
            return Err(-15);
        }
        let leaves = bytes
            .chunks_exact(32)
            .map(|word| word.try_into().expect("exact chunk"))
            .collect();
        job.evidence = Some(Evidence::Commitments(leaves));
        Ok(())
    })
}
pub(super) fn job_paths(id: u64, siblings: &[u8], directions: &[u8]) -> Result<()> {
    locked(|registry| {
        let job = registry.jobs.get_mut(&id).ok_or(CLOSED)?;
        if job.evidence.is_some() {
            return Err(INVALID);
        }
        if !(1..=2).contains(&job.inputs.len()) {
            return Err(-11);
        }
        if siblings.len() != job.inputs.len() * 16 * 32 || directions.len() != job.inputs.len() * 16
        {
            return Err(-13);
        }
        if directions.iter().any(|v| *v > 1) {
            return Err(-14);
        }
        let mut paths = Vec::with_capacity(job.inputs.len());
        for (index, input) in job.inputs.iter().enumerate() {
            let directions = &directions[index * 16..(index + 1) * 16];
            if directions
                .iter()
                .enumerate()
                .any(|(level, &v)| usize::from(v) != ((input.leaf_index >> level) & 1))
            {
                return Err(-16);
            }
            // Establish the clearing owner before copying the first private path cell.
            let mut path = ConfidentialMerklePathV2 {
                root: job.root,
                siblings: Vec::with_capacity(16),
                directions: Vec::with_capacity(16),
                witness_nodes: Vec::new(),
            };
            path.siblings.extend(
                siblings[index * 512..(index + 1) * 512]
                    .chunks_exact(32)
                    .map(|word| <[u8; 32]>::try_from(word).expect("exact chunk")),
            );
            path.directions.extend_from_slice(directions);
            paths.push(path);
        }
        job.evidence = Some(Evidence::Paths(paths));
        Ok(())
    })
}
pub(super) fn job_close(id: u64) -> Result<()> {
    let job = locked(|registry| registry.jobs.remove(&id).ok_or(CLOSED))?;
    drop(job);
    Ok(())
}
pub(super) fn job_prove(id: u64) -> Result<Vec<u8>> {
    // Remove before expensive work. Closing the prover cannot invalidate this retained owner.
    let job = locked(|registry| registry.jobs.remove(&id).ok_or(CLOSED))?;
    let evidence = job.evidence.ok_or(INVALID)?;
    let tree = match &evidence {
        Evidence::Commitments(leaves) => ConfidentialTree::Commitments {
            root: job.root,
            leaves,
        },
        Evidence::Paths(paths) => ConfidentialTree::Paths {
            root: job.root,
            paths,
        },
    };
    let result = if job.operation == 0 {
        job.prover.prove_transfer(tree, job.inputs, job.outputs)
    } else {
        let inputs = job
            .inputs
            .into_iter()
            .map(|note| ConfidentialUnshieldInputV2 {
                amount: note.amount,
                rho: note.rho,
                diversifier: note.diversifier,
                leaf_index: note.leaf_index,
            })
            .collect();
        let change = job
            .outputs
            .into_iter()
            .next()
            .map(|note| ConfidentialUnshieldOutputV3 {
                amount: note.amount,
                rho: note.rho,
            });
        job.prover
            .prove_unshield(tree, inputs, job.public_amount, change)
    }
    .map_err(native_error)?;
    public_result(result)
}
fn public_result(result: ConfidentialProof) -> Result<Vec<u8>> {
    if result.proof.bytes.len() > (MAX_RESULT_BYTES - 4096) / 2
        || result.proof.backend.len() > 128
        || result.nullifiers.len() > 2
        || result.output_commitments.len() > 2
    {
        return Err(RESOURCE);
    }
    let relation = match result.relation {
        ProofRelation::ConfidentialTransfer => "confidential_transfer",
        ProofRelation::ConfidentialFullUnshield => "confidential_full_unshield",
        ProofRelation::ConfidentialChangeUnshield => "confidential_change_unshield",
        _ => return Err(INTERNAL),
    };
    let backend = result.proof.backend;
    let proof_hex = hex::encode(result.proof.bytes);
    let root_hex = hex::encode(result.root);
    let nullifiers_hex = result
        .nullifiers
        .into_iter()
        .map(hex::encode)
        .collect::<Vec<_>>();
    let output_commitments_hex = result
        .output_commitments
        .into_iter()
        .map(hex::encode)
        .collect::<Vec<_>>();
    norito::json::to_vec(&norito::json!({
        "relation": relation, "backend": backend,
        "proof_hex": proof_hex, "root_hex": root_hex,
        "nullifiers_hex": nullifiers_hex, "output_commitments_hex": output_commitments_hex
    }))
    .map_err(|_| INTERNAL)
}
fn boundary(f: impl FnOnce() -> Result<()>) -> c_int {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
        .unwrap_or(Err(INTERNAL))
        .err()
        .unwrap_or(0)
}
unsafe fn bytes<'a>(ptr: *const u8, len: c_ulong, max: usize) -> Result<&'a [u8]> {
    let len = usize::try_from(len).map_err(|_| INVALID)?;
    if len > max {
        return Err(INVALID);
    }
    if len == 0 {
        return Ok(&[]);
    }
    if ptr.is_null() {
        return Err(INVALID);
    }
    Ok(unsafe { std::slice::from_raw_parts(ptr, len) })
}
unsafe fn handle_out(out: *mut u64, f: impl FnOnce() -> Result<u64>) -> Result<()> {
    if out.is_null() {
        return Err(INVALID);
    }
    unsafe {
        *out = 0;
    }
    let handle = f()?;
    unsafe {
        *out = handle;
    }
    Ok(())
}
/// Exact first-release local confidential-prover contract revision.
#[unsafe(no_mangle)]
pub extern "C" fn connect_norito_confidential_prover_revision_v1() -> u32 {
    1
}
/// Create an owned key context. All pointers must be readable for their stated lengths.
/// # Safety
/// `out` must be writable; input slices must be valid for this call and must not overlap it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_confidential_prover_create_v1(
    network: *const u8,
    network_len: c_ulong,
    asset: *const u8,
    asset_len: c_ulong,
    key: *const u8,
    key_len: c_ulong,
    out: *mut u64,
) -> c_int {
    boundary(|| unsafe {
        handle_out(out, || {
            create(
                bytes(network, network_len, 32)?,
                bytes(asset, asset_len, 512)?,
                bytes(key, key_len, 32)?,
            )
        })
    })
}
/// Close a context; already accepted jobs retain their own reference.
#[unsafe(no_mangle)]
pub extern "C" fn connect_norito_confidential_prover_close_v1(id: u64) -> c_int {
    boundary(|| close(id))
}
/// Create one operation: 0 transfer, 1 redemption. Amount is an unsigned 128-bit pair.
/// # Safety
/// Root must be readable for its length; `out` must be writable and disjoint.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_confidential_prover_job_create_v1(
    id: u64,
    operation: u8,
    root: *const u8,
    root_len: c_ulong,
    lo: u64,
    hi: u64,
    out: *mut u64,
) -> c_int {
    boundary(|| unsafe {
        handle_out(out, || {
            job_create(id, operation, bytes(root, root_len, 32)?, lo, hi)
        })
    })
}
/// Add an actual input note before setting tree evidence; at most two are accepted.
/// # Safety
/// Byte pointers must be readable for their lengths for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_confidential_prover_job_input_v1(
    id: u64,
    lo: u64,
    hi: u64,
    rho: *const u8,
    rho_len: c_ulong,
    diversifier: *const u8,
    diversifier_len: c_ulong,
    index: u64,
) -> c_int {
    boundary(|| unsafe {
        job_input(
            id,
            lo,
            hi,
            bytes(rho, rho_len, 32)?,
            bytes(diversifier, diversifier_len, 32)?,
            index,
        )
    })
}
/// Add one output. Redemption accepts at most one change note and an empty owner slice.
/// # Safety
/// Byte pointers must be readable for their lengths for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_confidential_prover_job_output_v1(
    id: u64,
    lo: u64,
    hi: u64,
    rho: *const u8,
    rho_len: c_ulong,
    owner: *const u8,
    owner_len: c_ulong,
) -> c_int {
    boundary(|| unsafe {
        job_output(
            id,
            lo,
            hi,
            bytes(rho, rho_len, 32)?,
            bytes(owner, owner_len, 32)?,
        )
    })
}
/// Set the complete ordered leaf prefix (at most 65,536 consecutive 32-byte commitments).
/// # Safety
/// The byte pointer must be readable for its length for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_confidential_prover_job_commitments_v1(
    id: u64,
    leaves: *const u8,
    len: c_ulong,
) -> c_int {
    boundary(|| unsafe { job_commitments(id, bytes(leaves, len, MAX_TREE_BYTES)?) })
}
/// Set one ordered 16-level path per actual input, all against the job's expected root.
/// # Safety
/// Both byte pointers must be readable for their lengths for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_confidential_prover_job_paths_v1(
    id: u64,
    siblings: *const u8,
    siblings_len: c_ulong,
    directions: *const u8,
    directions_len: c_ulong,
) -> c_int {
    boundary(|| unsafe {
        job_paths(
            id,
            bytes(siblings, siblings_len, 1024)?,
            bytes(directions, directions_len, 32)?,
        )
    })
}
/// Consume a job once and return public Norito JSON; release output using `connect_norito_free`.
/// # Safety
/// Both output pointers must be writable and disjoint. No registry lock is held while proving.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_confidential_prover_job_prove_v1(
    id: u64,
    out: *mut *mut c_uchar,
    len: *mut c_ulong,
) -> c_int {
    super::clear_bridge_output(out, len);
    boundary(|| {
        if out.is_null() || len.is_null() {
            let _ = job_close(id);
            return Err(INVALID);
        }
        let result = job_prove(id)?;
        unsafe { super::write_bytes(out, len, &result).map_err(|_| RESOURCE) }
    })
}
/// Close an abandoned job and clear its owned private notes.
#[unsafe(no_mangle)]
pub extern "C" fn connect_norito_confidential_prover_job_close_v1(id: u64) -> c_int {
    boundary(|| job_close(id))
}

#[cfg(test)]
mod tests;
