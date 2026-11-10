//! Non-signing export of existing fee originals from the exact matched offline replay.
//!
//! Normal configuration parsing is unchanged and can read configured node-key bytes. This
//! continuation opens no signer/provider, creates no fee, publishes no State and uses no network.
//! Independent source/build/copied-input custody and shipping-root admission remain required.

use super::*;
use iroha_allocation::{AllocationCharge, AllocationReservation};
use iroha_core::sumeragi::{
    certified_chain::{CurrentCutSourceLimits, read_current_certified_cut},
    finality::{NativeFinalityProofBudget, NativeFinalityProofIntervalLimits},
};
use iroha_data_model::{
    block::{BlockHeader, consensus::SumeragiRootScope},
    sumeragi::finality::{
        NATIVE_FINALITY_MAX_BLOCK_BYTES, NATIVE_FINALITY_MAX_BLOCK_COUNT,
        NATIVE_FINALITY_MAX_JOURNAL_BYTES,
    },
    sumeragi_finality::{MAX_FINALITY_BLOCK_BYTES, MAX_WORLD_STATE_SNAPSHOT_BYTES_V1},
};
use std::{
    alloc::Layout, io::Read as _, num::NonZeroU64, sync::atomic::AtomicBool, time::Duration,
};

const REQUEST_SCHEMA: &str = "iroha.kagemusha.world-payout-export-request.v1";
const MAX_JSON: usize = 64 * 1024;
const MAX_PROOFS: u64 = 256;
const MAX_PROOF_BYTES: usize = 64 * 1024 * 1024;
const MAX_SINGLE_PROOF: usize = 36 * 1024 * 1024;
const MAX_OUTPUT: usize = MAX_WORLD_STATE_SNAPSHOT_BYTES_V1 + MAX_PROOF_BYTES + 1024;
const MIB: usize = 1024 * 1024;

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ExportRequest {
    schema: String,
    scheme_id_hex: String,
    credit_id_hex: String,
    network_id: NetworkId,
    genesis_hash: iroha_crypto::HashOf<BlockHeader>,
    chain_id: String,
    height: u64,
    block_hash: iroha_crypto::HashOf<BlockHeader>,
    proof_from_height: u64,
}

fn selector(value: &str) -> Result<[u8; 32], String> {
    let decoded: [u8; 32] = hex::decode(value)
        .map_err(|error| error.to_string())?
        .try_into()
        .map_err(|_| "selector must contain exactly 32 bytes")?;
    if decoded == [0; 32] || hex::encode(decoded) != value {
        return Err("selector must be nonzero lowercase 64-hex".into());
    }
    Ok(decoded)
}

impl ExportRequest {
    fn validate(&self) -> Result<([u8; 32], [u8; 32]), String> {
        if self.schema != REQUEST_SCHEMA
            || self.height < 2
            || self.proof_from_height == 0
            || self.proof_from_height > self.height
            || self.height - self.proof_from_height >= MAX_PROOFS
            || self.network_id.as_genesis_hash() != &self.genesis_hash
            || self.chain_id.is_empty()
            || self.chain_id.len() > 1024
        {
            return Err(
                "invalid closed export request, genesis binding or proof range (1..256)".into(),
            );
        }
        Ok((
            selector(&self.scheme_id_hex)?,
            selector(&self.credit_id_hex)?,
        ))
    }
}

#[derive(Debug, JsonSerialize)]
pub(super) struct Descriptor {
    pub(super) name: String,
    size: String,
    sha256: String,
}

pub(super) struct Original {
    pub(super) descriptor: Descriptor,
    pub(super) bytes: Vec<u8>,
    _charge: AllocationCharge,
}

// Reserve the one concrete byte layout before copying or encoding. Move its credits
// to the original's lifetime without acquiring a second grant from the output pool.
fn charge_original(budget: &AllocationBudget, length: usize) -> Result<AllocationCharge, String> {
    let layout = Layout::array::<u8>(length).map_err(|error| error.to_string())?;
    let mut reservation = budget
        .try_reserve(layout)
        .map_err(|error| error.to_string())?;
    reservation
        .try_split(layout)
        .map_err(|error| error.to_string())
}

fn retain_bytes(
    name: String,
    source: &[u8],
    budget: &AllocationBudget,
) -> Result<Original, String> {
    let charge = charge_original(budget, source.len())?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(source.len())
        .map_err(|error| error.to_string())?;
    bytes.extend_from_slice(source);
    Ok(original(name, bytes, charge))
}

fn original(name: String, bytes: Vec<u8>, charge: AllocationCharge) -> Original {
    Original {
        descriptor: Descriptor {
            name,
            size: bytes.len().to_string(),
            sha256: hex::encode(iroha_crypto::sha256(&bytes)),
        },
        bytes,
        _charge: charge,
    }
}

pub(super) fn retain_native<T: norito::NoritoSerialize>(
    name: String,
    value: &T,
    maximum: usize,
    budget: &AllocationBudget,
) -> Result<Original, String> {
    let length = norito::canonical_frame_len(value).map_err(|error| error.to_string())?;
    if length == 0 || length > maximum {
        return Err("native original exceeds its explicit frame bound".into());
    }
    let charge = charge_original(budget, length)?;
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let bytes = norito::core::to_bytes_bounded(value, length).map_err(|error| error.to_string())?;
    if bytes.len() != length {
        return Err("native frame changed between counted and encoded pass".into());
    }
    Ok(original(name, bytes, charge))
}

fn retain_suffix_proof<T: norito::NoritoSerialize>(
    ordinal: u64,
    proof: &T,
    maximum: usize,
    budget: &AllocationBudget,
    deadline: Instant,
    mut now: impl FnMut() -> Instant,
) -> Result<Original, String> {
    let mut check_deadline = || {
        if now() >= deadline {
            Err("finality proof suffix deadline expired")
        } else {
            Ok(())
        }
    };
    check_deadline()?;
    let original = retain_native(format!("proof-{ordinal:04}.norito"), proof, maximum, budget)?;
    // The interval reader returned before local frame counting, encoding and hashing.
    check_deadline()?;
    Ok(original)
}

#[derive(JsonSerialize)]
struct Bundle<'a> {
    schema: &'static str,
    network_id: NetworkId,
    genesis_hash_hex: String,
    chain_id: String,
    root_scope: &'static str,
    scheme_id_hex: &'a str,
    credit_id_hex: &'a str,
    height: u64,
    block_hash_hex: String,
    world_root_hex: String,
    world_schema_hash_hex: String,
    proof_from_height: u64,
    proof_count: usize,
    request_sha256: String,
    request_size: String,
    files: Vec<&'a Descriptor>,
    limitations: &'static str,
}

pub(super) struct Prepared {
    pub(super) originals: Vec<Original>,
    pub(super) manifest: Box<[u8]>,
    // Keep the complete original finite grant until the output and all originals retire.
    pub(super) _reservation: AllocationReservation,
}

fn prepare(
    matched: &MatchedReplay<'_>,
    request: &ExportRequest,
    request_bytes: &[u8],
) -> Result<Prepared, String> {
    let (scheme, credit) = request.validate()?;
    if !matches!(matched.report.outcome, ReplayCheckOutcome::Matched)
        || matched.tip_height != request.height
        || matched.report.replayed_to != matched.tip_height
        || matched.genesis_hash != request.genesis_hash
    {
        return Err(
            "export requires a complete matched replay at the exact requested tip/genesis".into(),
        );
    }
    // Core also returns Matched when the restored snapshot already owns the tip. In that
    // case no block was re-executed, so this invocation cannot publish replay evidence.
    if matched.report.start.height() >= matched.tip_height {
        return Err("fee export requires a block re-executed after the replay start".into());
    }
    // One original grant from the replay State pool, partitioned into disjoint finite
    // source, graph, World and output corridors. These are not replacement decoder limits.
    let reservation = matched
        .state
        .ivm_execution_budget()
        .try_reserve_bytes(640 * MIB)
        .map_err(|error| error.to_string())?;
    let frames = AllocationBudget::new(128 * MIB);
    let graphs = AllocationBudget::new(256 * MIB);
    let world = AllocationBudget::new(128 * MIB);
    let outputs = AllocationBudget::new(128 * MIB);
    let view = matched
        .state
        .try_view_once()
        .map_err(|error| error.to_string())?;
    if view.network_id != request.network_id
        || view.network_id.as_genesis_hash() != &matched.genesis_hash
        || view.chain_id.to_string() != request.chain_id
        || u64::try_from(view.height()).ok() != Some(request.height)
        || view.latest_block_hash() != Some(request.block_hash)
    {
        return Err(
            "actual replay State differs from expected network/chain/genesis/height/hash".into(),
        );
    }
    let actual_network = view.network_id;
    let actual_chain = view.chain_id.to_string();
    let tip = read_current_certified_cut(
        &view,
        request.height,
        SumeragiRootScope::Global,
        CurrentCutSourceLimits::new(
            NATIVE_FINALITY_MAX_BLOCK_BYTES,
            NATIVE_FINALITY_MAX_JOURNAL_BYTES,
            NATIVE_FINALITY_MAX_BLOCK_COUNT,
        )?,
        &frames,
        &graphs,
    )
    .map_err(|error| error.to_string())?;
    drop(view);
    if tip.block_hash() != request.block_hash || tip.height() != request.height {
        return Err("native certified cut differs from matched replay tip".into());
    }
    let mut originals = Vec::new();
    originals
        .try_reserve_exact((request.height - request.proof_from_height + 3) as usize)
        .map_err(|error| error.to_string())?;
    let (snapshot, payout, schema_hash) = matched
        .state
        .with_native_kagemusha_fee_snapshot_v1(&tip, scheme, credit, &world, |snapshot, payout| {
            Ok((
                retain_native(
                    "world-snapshot.norito".into(),
                    snapshot,
                    MAX_WORLD_STATE_SNAPSHOT_BYTES_V1,
                    &outputs,
                )?,
                retain_bytes("fee-payout.norito".into(), payout, &outputs)?,
                snapshot.schema_hash,
            ))
        })
        .map_err(|error| error.to_string())?;
    originals.extend([snapshot, payout]);
    let owner = NativeFinalityProofBudget::new(
        frames,
        graphs,
        norito::canonical_decode_limits(MAX_FINALITY_BLOCK_BYTES),
    );
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(300))
        .ok_or("deadline overflow")?;
    let cancelled = AtomicBool::new(false);
    let mut first = request.proof_from_height;
    let mut proof_bytes = 0usize;
    while first <= request.height {
        let last = first.saturating_add(63).min(request.height);
        let limits = NativeFinalityProofIntervalLimits::new(
            NonZeroU64::new(first).ok_or("zero first proof height")?,
            NonZeroU64::new(last).ok_or("zero final proof height")?,
            NonZeroUsize::new(MAX_FINALITY_BLOCK_BYTES).ok_or("zero block bound")?,
            NonZeroUsize::new(MAX_PROOF_BYTES - proof_bytes).ok_or("proof byte bound exhausted")?,
            deadline,
        )
        .map_err(|error| error.to_string())?;
        let interval = matched
            .state
            .read_finality_proof_interval(&owner, &limits, &cancelled)
            .map_err(|error| error.to_string())?;
        if interval.from_height() != first || interval.len() as u64 != last - first + 1 {
            return Err("native producer did not return the requested contiguous suffix".into());
        }
        for index in 0..interval.len() {
            let ordinal = first - request.proof_from_height + index as u64 + 1;
            let proof = interval.proof(index).ok_or("native proof omitted")?;
            let artifact = retain_suffix_proof(
                ordinal,
                proof,
                MAX_SINGLE_PROOF.min(MAX_PROOF_BYTES - proof_bytes),
                &outputs,
                deadline,
                Instant::now,
            )?;
            proof_bytes = proof_bytes
                .checked_add(artifact.bytes.len())
                .ok_or("proof size overflow")?;
            originals.push(artifact);
        }
        if last == request.height {
            break;
        }
        first = last.checked_add(1).ok_or("proof height overflow")?;
    }
    let view = matched
        .state
        .try_view_once()
        .map_err(|error| error.to_string())?;
    if u64::try_from(view.height()).ok() != Some(request.height)
        || view.latest_block_hash() != Some(request.block_hash)
        || view.network_id != actual_network
        || view.chain_id.to_string() != actual_chain
    {
        return Err("replay State changed during original production".into());
    }
    drop(view);
    let total = originals
        .iter()
        .try_fold(0usize, |size, item| size.checked_add(item.bytes.len()))
        .ok_or("package size overflow")?;
    if total > MAX_OUTPUT {
        return Err("package originals exceed finite cap".into());
    }
    let bundle = Bundle {
        schema: "iroha.kagemusha.world-payout-export.v1",
        network_id: actual_network,
        genesis_hash_hex: hex::encode(matched.genesis_hash.as_ref()),
        chain_id: actual_chain,
        root_scope: "global",
        scheme_id_hex: &request.scheme_id_hex,
        credit_id_hex: &request.credit_id_hex,
        height: tip.height(),
        block_hash_hex: hex::encode(tip.block_hash().as_ref()),
        world_root_hex: hex::encode(tip.world_state_root().as_ref()),
        world_schema_hash_hex: hex::encode(schema_hash.as_ref()),
        proof_from_height: request.proof_from_height,
        proof_count: originals.len() - 2,
        request_sha256: hex::encode(iroha_crypto::sha256(request_bytes)),
        request_size: request_bytes.len().to_string(),
        files: originals.iter().map(|item| &item.descriptor).collect(),
        limitations: "Native original export only. Independent final source/build/run/copied-input receipt and exact shipping genesis/root/schema/artifact admission are required. No wallet/session/custody, Payment or Request is fabricated. Manifest hashes are byte identities, not authority. Normal config parsing may read configured node keys; this export never signs or publishes State.",
    };
    let manifest = norito::json::to_json_bounded_boxed(&bundle, MAX_JSON)
        .map_err(|error| error.to_string())?;
    Ok(Prepared {
        originals,
        manifest,
        _reservation: reservation,
    })
}

pub(super) fn read_request_bytes(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_JSON as u64 {
        return Err("request must be a nonempty regular non-symlink file up to 64 KiB".into());
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(rustix::fs::OFlags::NOFOLLOW.bits().cast_signed());
    }
    let mut file = options.open(path).map_err(|error| error.to_string())?;
    let opened = file.metadata().map_err(|error| error.to_string())?;
    if !opened.is_file() || opened.len() != metadata.len() {
        return Err("request changed while opening".into());
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(MAX_JSON + 1)
        .map_err(|error| error.to_string())?;
    std::io::Read::by_ref(&mut file)
        .take((MAX_JSON + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() != opened.len() as usize || bytes.len() > MAX_JSON {
        return Err("request changed or exceeded bound".into());
    }
    Ok(bytes)
}

fn read_request(path: &Path) -> Result<(ExportRequest, Vec<u8>), String> {
    let bytes = read_request_bytes(path)?;
    let request: ExportRequest =
        norito::json::from_slice(&bytes).map_err(|error| error.to_string())?;
    request.validate()?;
    Ok((request, bytes))
}

fn publish(output: &Path, request: &[u8], prepared: &Prepared) -> Result<String, String> {
    let mut directory = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        directory.mode(0o700);
    }
    #[cfg(not(unix))]
    return Err("offline export requires Unix output custody".into());
    directory
        .create(output)
        .map_err(|error| error.to_string())?;
    sorafs_authority_inspect::publish_file(output, "request.json", request)?;
    for item in &prepared.originals {
        sorafs_authority_inspect::publish_file(output, &item.descriptor.name, &item.bytes)?;
    }
    // Last, exclusive completeness marker. A partial directory cannot qualify.
    sorafs_authority_inspect::publish_file(output, "manifest.json", &prepared.manifest)?;
    std::fs::File::open(output)
        .and_then(|file| file.sync_all())
        .map_err(|error| error.to_string())?;
    Ok(hex::encode(iroha_crypto::sha256(&prepared.manifest)))
}

#[derive(JsonSerialize)]
struct RunReport {
    replay: ReplayCheckReportV1,
    manifest_sha256: Option<String>,
    export_failure: Option<String>,
}

/// Export only existing native fee/World/finality originals after a complete replay match.
///
/// # Errors
/// Refuses invalid paths/selectors, absent payout, mismatched or incomplete replay, native
/// verification/resource failure and incomplete publication. No transaction is executed.
pub fn run_world_payout_export(
    config: &Config,
    request: &Path,
    output: &Path,
    config_path: Option<&Path>,
) -> ReportResult<(), MainError> {
    let (request, bytes) = read_request(request).map_err(failure)?;
    let output = sorafs_authority_inspect::output_destination(config, output, config_path)
        .map_err(failure)?;
    let mut manifest_sha256 = None;
    let mut export_failure = None;
    let replay = check_replay_with_matched(config, |matched| {
        match prepare(&matched, &request, &bytes).and_then(|prepared| {
            let output =
                sorafs_authority_inspect::output_destination(config, &output, config_path)?;
            publish(&output, &bytes, &prepared)
        }) {
            Ok(hash) => manifest_sha256 = Some(hash),
            Err(reason) => export_failure = Some(reason),
        }
        Ok(())
    })?;
    if manifest_sha256.is_none() && export_failure.is_none() {
        export_failure = Some("export requires a complete nonempty matched replay".into());
    }
    let report = RunReport {
        replay,
        manifest_sha256,
        export_failure,
    };
    println!(
        "{}",
        norito::json::to_json(&report).map_err(|error| failure(error.to_string()))?
    );
    if report.replay.is_ok() && report.manifest_sha256.is_some() {
        Ok(())
    } else {
        Err(failure(
            report
                .export_failure
                .unwrap_or_else(|| report.replay.failure_summary()),
        ))
    }
}

#[cfg(test)]
mod tests;
