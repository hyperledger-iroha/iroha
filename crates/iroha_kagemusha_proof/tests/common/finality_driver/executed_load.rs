//! Independently pinned executed Load originals and complete H2..H5 proof production.
//!
//! This entry point has one exact grammar. Historical fixture captures cannot select it.
//! Native finality authenticates the original blocks and counted event before expensive
//! recursive proving; the installed graph still proves all source and history obligations.

use super::*;
use iroha_crypto::{Algorithm, Hash, HashOf, MerkleProof};
use iroha_data_model::{
    account::AccountId,
    block::decode_framed_signed_block,
    events::EventBox,
    isi::kagemusha_wallet::{
        KagemushaWalletLedgerActionV1 as Action, KagemushaWalletLedgerV1,
        KagemushaWalletLoadReceiptV1,
        load_finality::verify_finalized_kagemusha_wallet_load_event_v1,
    },
    kagemusha::{
        KagemushaWalletActivationV1, KagemushaWalletAssetScopeV1, KagemushaWalletLoadFinalityV1,
        kagemusha_wallet_account_digest_v1,
    },
    sumeragi_finality::{
        FinalityValidator, SumeragiFinalityProof, SumeragiFinalityVerifier, VerifiedSumeragiBlock,
        genesis_epoch,
    },
};
use std::collections::BTreeMap;

/// Prove the admitted executed history using the exact selected source originals.
#[path = "executed_load/production.rs"]
pub mod production;
#[path = "executed_load/target.rs"]
mod target;
#[cfg(test)]
#[path = "executed_load/tests.rs"]
mod tests;

const CAPTURE_MAX: usize = 16 << 20;
const ORIGINAL_MAX: usize = 16 << 20;

/// Independently retained selections; none is selected from a proof or candidate receipt.
#[derive(Clone, Copy)]
pub struct Selection<'a> {
    /// Canonically executed setup directory.
    pub setup_root: &'a Path,
    /// Exact external setup manifest digest.
    pub setup_sha256: [u8; 32],
    /// Actual A target manifest path.
    pub target: &'a Path,
    /// Independent target manifest digest.
    pub target_sha256: [u8; 32],
    /// Actual ledger execution capture path.
    pub capture: &'a Path,
    /// Independent execution capture digest.
    pub capture_sha256: [u8; 32],
    /// Independent original finalized receipt digest (SHA-256 of its complete frame).
    pub receipt_sha256: [u8; 32],
}

struct Executed {
    setup: source_only::Setup,
    capture: Vec<u8>,
    target: Vec<u8>,
    originals: BTreeMap<String, Vec<u8>>,
    target_originals: BTreeMap<String, Vec<u8>>,
    blocks: Vec<BlockWitnessInput>,
    load: LoadWitnessInput,
    receipt: KagemushaWalletLoadReceiptV1,
}

fn need(valid: bool) -> Result<(), Error> {
    if valid { Ok(()) } else { Err(Error::Input) }
}
fn canonical<T>(bytes: &[u8]) -> Result<T, Error>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    norito::decode_canonical_with_limits(bytes, norito::canonical_decode_limits(bytes.len()))
        .map_err(|_| Error::Input)
}
fn pinned(path: &Path, cap: usize, pin: [u8; 32]) -> Result<Vec<u8>, Error> {
    need(pin != [0; 32])?;
    let original = read_bounded(path, cap)?;
    need(!original.is_empty() && <[u8; 32]>::from(Sha256::digest(&original)) == pin)?;
    Ok(original)
}
fn originals(
    root: &Path,
    json: &Value,
    field: &str,
    names: &[(&str, usize)],
) -> Result<BTreeMap<String, Vec<u8>>, Error> {
    need(directory_exists(root)?)?;
    let rows = value(json, field)?.as_array().ok_or(Error::Input)?;
    need(rows.len() == names.len())?;
    let mut out = BTreeMap::new();
    for row in rows {
        let name = value(row, "name")?.as_str().ok_or(Error::Input)?;
        let maximum = names
            .iter()
            .find(|(candidate, _)| name == *candidate)
            .ok_or(Error::Input)?
            .1;
        need(!out.contains_key(name))?;
        let length = value(row, "bytes")?.as_u64().ok_or(Error::Input)?;
        need(length > 0 && length <= maximum as u64)?;
        let frame = pinned(&root.join(name), maximum, fixed(row, "sha256")?)?;
        need(frame.len() as u64 == length)?;
        out.insert(name.to_owned(), frame);
    }
    Ok(out)
}
fn native(setup: &source_only::Setup) -> Result<SumeragiFinalityVerifier, Error> {
    // `Setup` exists only after canonical manifest/genesis and H1/H2 admission.
    let genesis = decode_framed_signed_block(&setup.originals["signed-genesis.wire"])
        .map_err(|_| Error::Input)?;
    let epoch = genesis_epoch(&genesis).map_err(|_| Error::Input)?;
    let roster = epoch
        .committee
        .iter()
        .map(|member| FinalityValidator {
            public_key: member.validator.public_key().clone(),
            proof_of_possession: member.proof_of_possession.clone(),
        })
        .collect();
    let selected: Value =
        norito::json::from_slice(&setup.originals["capture.json"]).map_err(|_| Error::Input)?;
    SumeragiFinalityVerifier::new(
        &genesis,
        value(&selected, "chain_id")?.as_str().ok_or(Error::Input)?,
        roster,
    )
    .map_err(|_| Error::Input)
}

fn block_input(row: &Value, block: &VerifiedSumeragiBlock) -> Result<BlockWitnessInput, Error> {
    let certificate = block.block().commit_certificate().ok_or(Error::Input)?;
    let qc: iroha_sumeragi::message::Qc = canonical(certificate.commit_qc())?;
    let schedule = &block.commitment().schedule;
    need(schedule.boundary.is_none())?;
    let context = schedule.current.context_id().map_err(|_| Error::Input)?;
    let schedule_row = value(row, "authenticated_schedule")?;
    need(value(schedule_row, "boundary")?.is_null())?;
    need(fixed::<32>(value(schedule_row, "current")?, "context_id_hex")? == context)?;
    let frame = block.commitment().preimage().map_err(|_| Error::Input)?;
    need(frame.len() <= iroha_kagemusha_proof::finality::result::MAX_RESULT_BYTES as usize)?;
    need(
        value(row, "height")?.as_u64() == Some(block.height())
            && bytes(row, "result_preimage_hex")? == frame
            && fixed::<32>(row, "result_hash_hex")? == block.result().0
            && bytes(row, "commit_vote_preimage_hex")? == qc.preimage()
            && bytes(row, "qc_bitmap_hex")? == qc.signers.as_bytes()
            && fixed::<96>(row, "qc_aggregate_signature_hex")? == qc.agg_sig.0,
    )?;
    let keys = value(row, "committee_public_keys_hex")?
        .as_array()
        .ok_or(Error::Input)?;
    let pops = value(row, "committee_proofs_of_possession_hex")?
        .as_array()
        .ok_or(Error::Input)?;
    need(keys.len() == schedule.current.committee.len() && pops.len() == keys.len())?;
    let mut roster = Vec::with_capacity(keys.len());
    for ((member, key), pop) in schedule.current.committee.iter().zip(keys).zip(pops) {
        let (algorithm, original) = member
            .validator
            .public_key()
            .try_to_bytes()
            .map_err(|_| Error::Input)?;
        need(
            algorithm == Algorithm::BlsNormal
                && hex(key.as_str().ok_or(Error::Input)?)? == original
                && hex(pop.as_str().ok_or(Error::Input)?)? == member.proof_of_possession,
        )?;
        roster.push(original.try_into().map_err(|_| Error::Input)?);
    }
    Ok(BlockWitnessInput {
        result_frame: frame,
        message: qc.preimage().try_into().map_err(|_| Error::Input)?,
        roster,
        bitmap: qc.signers.as_bytes().to_vec(),
        signature: qc.agg_sig.0,
        current_context: context,
        authorized_context: context,
    })
}

/// Reject noncanonical missing/exhausted siblings rather than silently discard them.
fn event_path(
    count: u64,
    index: u32,
    siblings: &[[u8; 32]; 32],
) -> Result<MerkleProof<EventBox>, Error> {
    need(count > 0 && count <= 1_u64 << 32 && u64::from(index) < count)?;
    let mut width = count;
    let mut position = index;
    let mut path = Vec::new();
    for sibling in siblings {
        if width <= 1 {
            need(*sibling == [0; 32])?;
            continue;
        }
        if (u64::from(position) ^ 1) < width {
            let hash = Hash::from_marked_bytes(*sibling).ok_or(Error::Input)?;
            path.push(Some(HashOf::from_untyped_unchecked(hash)));
        } else {
            need(*sibling == [0; 32])?;
            path.push(None);
        }
        position >>= 1;
        width = (width >> 1) + (width & 1);
    }
    need(width == 1 && position == 0)?;
    Ok(MerkleProof::from_audit_path(index, path))
}
fn load_input(
    json: &Value,
    block: &VerifiedSumeragiBlock,
    receipt: &KagemushaWalletLoadReceiptV1,
    native: &SumeragiFinalityVerifier,
) -> Result<LoadWitnessInput, Error> {
    let row = value(json, "load")?;
    let frame = block.commitment().preimage().map_err(|_| Error::Input)?;
    let transcript = receipt.transcript().map_err(|_| Error::Input)?;
    need(
        bytes(row, "receipt_frame_hex")?
            == receipt.to_canonical_bytes().map_err(|_| Error::Input)?
            && bytes(row, "receipt_transcript_hex")? == transcript
            && fixed::<32>(row, "receipt_digest_hex")?
                == receipt.receipt_digest().map_err(|_| Error::Input)?
            && bytes(row, "result_preimage_hex")? == frame,
    )?;
    let commitment = block
        .execution()
        .event_commitment
        .as_ref()
        .ok_or(Error::Input)?;
    let event_root = fixed(row, "event_commitment_root_hex")?;
    let count = value(row, "event_commitment_count")?
        .as_u64()
        .ok_or(Error::Input)?;
    let index = u32::try_from(value(row, "event_index")?.as_u64().ok_or(Error::Input)?)
        .map_err(|_| Error::Input)?;
    need(event_root == *commitment.root().as_ref() && count == commitment.leaf_count().get())?;
    let siblings: [[u8; 32]; 32] = value(row, "event_siblings_hex")?
        .as_array()
        .ok_or(Error::Input)?
        .iter()
        .map(|entry| {
            hex(entry.as_str().ok_or(Error::Input)?)?
                .try_into()
                .map_err(|_| Error::Input)
        })
        .collect::<Result<Vec<_>, Error>>()?
        .try_into()
        .map_err(|_| Error::Input)?;
    let proof = event_path(count, index, &siblings)?;
    verify_finalized_kagemusha_wallet_load_event_v1(
        block,
        &proof,
        native.initial_epoch().network_id,
        native.chain_id(),
        receipt,
    )
    .map_err(|_| Error::Input)?;
    Ok(LoadWitnessInput {
        result_frame: frame,
        receipt: transcript,
        event_root,
        event_count: count,
        event_index: index,
        siblings,
    })
}

fn history(
    setup: &source_only::Setup,
    json: &Value,
    originals: &BTreeMap<String, Vec<u8>>,
    receipt: &KagemushaWalletLoadReceiptV1,
    native: &mut SumeragiFinalityVerifier,
) -> Result<(Vec<BlockWitnessInput>, LoadWitnessInput), Error> {
    let rows = value(json, "blocks")?.as_array().ok_or(Error::Input)?;
    need(rows.len() == 4 && receipt.block_height == 5)?;
    let initial: [SumeragiFinalityProof; 2] =
        canonical(&setup.originals["registration-proof.norito"])?;
    let mut blocks = Vec::with_capacity(4);
    let mut load = None;
    for height in 1..=5 {
        let proof: SumeragiFinalityProof =
            canonical(&originals[&format!("native-proof-{height}.norito")])?;
        need(proof.block_wire == originals[&format!("block-{height}.wire")])?;
        if height <= 2 {
            need(
                norito::encode_canonical(&proof).map_err(|_| Error::Input)?
                    == norito::encode_canonical(&initial[height - 1]).map_err(|_| Error::Input)?,
            )?;
        }
        let verified = native.verify(&proof).map_err(|_| Error::Input)?;
        need(verified.height() == height as u64)?;
        if height > 1 {
            blocks.push(block_input(&rows[height - 2], &verified)?);
        }
        if height == 5 {
            load = Some(load_input(json, &verified, receipt, native)?);
        }
    }
    Ok((blocks, load.ok_or(Error::Input)?))
}

fn validate(selection: &Selection<'_>) -> Result<Executed, Error> {
    let setup = source_only::validate(selection.setup_root, selection.setup_sha256)?;
    let capture = pinned(selection.capture, CAPTURE_MAX, selection.capture_sha256)?;
    let json: Value = norito::json::from_slice(&capture).map_err(|_| Error::Input)?;
    let source: Value =
        norito::json::from_slice(&setup.originals["capture.json"]).map_err(|_| Error::Input)?;
    for field in [
        "version",
        "chain_id",
        "signed_genesis_wire_hex",
        "history_anchor",
    ] {
        need(value(&json, field)? == value(&source, field)?)?;
    }
    need(fixed::<32>(&json, "native_target_sha256")? == selection.target_sha256)?;
    let names = (1..=5)
        .flat_map(|height| {
            [
                (format!("block-{height}.wire"), ORIGINAL_MAX),
                (format!("native-proof-{height}.norito"), ORIGINAL_MAX),
            ]
        })
        .chain([(String::from("receipt.norito"), 512)])
        .collect::<Vec<_>>();
    let names = names
        .iter()
        .map(|(name, cap)| (name.as_str(), *cap))
        .collect::<Vec<_>>();
    let originals = originals(
        selection.capture.parent().ok_or(Error::Input)?,
        &json,
        "originals",
        &names,
    )?;
    need(
        <[u8; 32]>::from(Sha256::digest(&originals["receipt.norito"])) == selection.receipt_sha256
            && selection.receipt_sha256 != [0; 32],
    )?;
    let receipt = KagemushaWalletLoadReceiptV1::decode_canonical(&originals["receipt.norito"])
        .map_err(|_| Error::Input)?;
    let mut native = native(&setup)?;
    let target = target::validate(selection, &setup, &native, &receipt)?;
    let (blocks, load) = history(&setup, &json, &originals, &receipt, &mut native)?;
    need(pinned(selection.capture, CAPTURE_MAX, selection.capture_sha256)? == capture)?;
    Ok(Executed {
        setup,
        capture,
        target: target.frame,
        originals,
        target_originals: target.originals,
        blocks,
        load,
        receipt,
    })
}

/// Check independently pinned actual target, complete native history and counted receipt inclusion.
/// This creates no keys, proofs, installation grant or monetary-state mutation.
pub fn check(selection: &Selection<'_>) {
    let selected = validate(selection).expect("exact native executed Load and contiguous H1..H5");
    tests::check_verified_mutations(&selected);
    eprintln!(
        "EXECUTED_LOAD_INTAKE complete_native_history=true block_count={} event_count={} event_index={} receipt_height={} no_new_proofs=true",
        selected.blocks.len(),
        selected.load.event_count,
        selected.load.event_index,
        selected.receipt.block_height
    );
}
