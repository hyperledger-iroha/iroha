//! Exact V1 proof retention, immutable Core admission and independent replay.
//!
//! This typed owner accepts an independently retained launch plan and separately
//! supplied input digests. It returns no artifact until both consuming owners
//! finish. No exported field, digest, roster or boolean can supply launch trust.
//! TODO: connect secure retained input handles, pre-submit signed-byte retention,
//! the pinned executable invocation and a descriptor-owned atomic output sink.

pub(crate) mod filesystem;
pub(crate) mod launcher;

use super::*;
use iroha_core::kura::{
    CanonicalKuraEvidenceComplete, CanonicalKuraEvidenceError, CanonicalKuraEvidenceLimits,
    CanonicalKuraEvidenceReader, CanonicalKuraMergeRequest,
};
use std::path::Path;

/// Exact externally owned finality and query bytes for one global height.
///
/// These bytes and their separate bindings must be retained independently of the
/// exported artifact. The adapter neither reads paths nor infers trusted hashes.
pub struct SuppliedHeightEvidence {
    /// Next height in the independently selected contiguous interval.
    pub height: u64,
    /// Canonical framed BridgeFinalityProof bytes.
    pub finality: Vec<u8>,
    /// Canonical CommittedTransaction bytes in complete merged leaf order.
    pub queries: Vec<Vec<u8>>,
}

/// Independently supplied input identities, never decoded from an export.
pub struct HeightInputBinding {
    /// Exact height, in strictly increasing interval order.
    pub height: u64,
    /// Iroha Hash of the retained canonical finality bytes.
    pub finality_hash: Hash,
    /// Iroha Hash of each retained canonical query, in exact leaf order.
    pub query_hashes: Vec<Hash>,
}

/// One result in independent schedule order; sequence is one-based within phase.
#[derive(Debug, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_kagami::scaling_evidence::ExportRowV1")]
pub struct ExportRowV1 {
    /// Warmup and measurement each start at sequence one.
    pub sequence: u64,
    /// Complete authenticated logical request and canonical application identity.
    pub request: AuthenticatedRequest,
}

#[derive(norito::Encode, norito::Decode)]
struct HeightProofV1 {
    height: u64,
    finality: Vec<u8>,
    carrier: Vec<u8>,
    merge_entry: Option<Vec<u8>>,
    queries: Vec<Vec<u8>>,
}

// One exact uncompressed Norito V1 frame. Inner version is mandatory as well;
// no headerless, compressed, JSON, version-guessing or summary-only acceptor exists.
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_kagami::scaling_evidence::ExportEnvelopeV1")]
struct ExportEnvelopeV1 {
    version: u16,
    heights: Vec<HeightProofV1>,
    rows: Vec<ExportRowV1>,
}

/// Completed proof artifact. Only complete export or independent replay creates it.
///
/// Original proofs occur once per height. This is not a filesystem publication
/// token, a trusted launcher plan, or a state-membership proof for Account reads.
pub struct VerifiedExport {
    disk_completion: Option<CanonicalKuraEvidenceComplete>,
    canonical: Vec<u8>,
    rows: Vec<ExportRowV1>,
    output_limit: u64,
}
impl VerifiedExport {
    /// Exact replayable V1 framed proof bytes, bounded by the admitted allocation.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }
    /// Complete rows in trusted logical offer order; never a successful prefix.
    pub fn rows(&self) -> &[ExportRowV1] {
        &self.rows
    }
    /// Strict JSON interop projection derived only from this completed owner.
    ///
    /// The JSON array is not proof authority. A consumer must obtain it from the
    /// pinned verifier, retain the canonical artifact, and join every row to its
    /// independently admitted schedule/trace. Hashes use canonical lowercase hex;
    /// authority uses canonical domainless I105; lane and dataspace are integers.
    pub fn json_projection(&self) -> Result<Vec<u8>> {
        // Bound all row temporaries before constructing JSON values. The bounded
        // serializer counts before allocating each <=1 KiB row destination.
        let reserve = u64::try_from(self.rows.len())?
            .checked_mul(ROW_RESERVATION + 1)
            .and_then(|n| n.checked_add(2))
            .ok_or_else(|| eyre!("projection reservation overflow"))?;
        ensure!(
            reserve
                .checked_add(u64::try_from(self.canonical.len())?)
                .is_some_and(|n| n <= self.output_limit),
            "projection output reservation exceeded"
        );
        let mut output = Vec::with_capacity(usize::try_from(reserve)?);
        output.push(b'[');
        for (index, row) in self.rows.iter().enumerate() {
            let bytes = projection_row(row)?;
            if index > 0 {
                output.push(b',');
            }
            output.extend_from_slice(bytes.as_bytes());
        }
        output.push(b']');
        Ok(output)
    }
}

fn projection_row(row: &ExportRowV1) -> Result<String> {
    let r = &row.request;
    let value = norito::json!({
        "logical_id": (r.logical_id),
        "phase": (match r.phase { WorkloadPhase::Warmup => "warmup", WorkloadPhase::Measurement => "measurement" }),
        "sequence": (row.sequence),
        "authority": (r.authority.canonical_i105()?),
        "entrypoint_hash": (r.entrypoint_hash.to_string()),
        "carrier_height": (r.carrier_height),
        "carrier_hash": (r.carrier_hash.to_string()),
        "merge_entry_hash": (r.merge_entry_hash.to_string()),
        "merge_epoch": (r.merge_epoch),
        "leaf_index": (r.leaf_index),
        "lane_id": (r.lane_id.as_u32()),
        "dataspace_id": (r.dataspace_id.as_u64()),
        "incarnation": (r.incarnation.to_string()),
    });
    Ok(norito::json::to_json_bounded(
        &value,
        ROW_RESERVATION as usize,
    )?)
}

/// Export one immutable disk interval using independent typed launch authority.
///
/// All supplied counts, byte limits and digest bindings are checked before Core
/// opens any path. Core derives requests from actual carrier references, scans the
/// complete log exactly once, and rechecks all retained identities at finish.
/// Neither provisional callback rows nor an output sink escape on any failure.
#[allow(clippy::too_many_arguments)]
pub fn export_from_kura(
    plan: TrustedRunPlan,
    limits: VerificationLimits,
    block_store: &Path,
    merge_log: &Path,
    reader_limits: CanonicalKuraEvidenceLimits,
    bindings: &[HeightInputBinding],
    supplied: Vec<SuppliedHeightEvidence>,
) -> Result<VerifiedExport> {
    export_with_finish_hook(
        plan,
        limits,
        block_store,
        merge_log,
        reader_limits,
        bindings,
        supplied,
        || {},
    )
}

#[allow(clippy::too_many_arguments)]
fn export_with_finish_hook(
    plan: TrustedRunPlan,
    limits: VerificationLimits,
    block_store: &Path,
    merge_log: &Path,
    reader_limits: CanonicalKuraEvidenceLimits,
    bindings: &[HeightInputBinding],
    supplied: Vec<SuppliedHeightEvidence>,
    before_disk_finish: impl FnOnce(),
) -> Result<VerifiedExport> {
    let first = plan.first_height;
    let last = plan.last_height;
    let verifier = admit(plan, limits, bindings)?;
    ensure!(
        reader_limits.first_height == first && reader_limits.last_height == last,
        "reader and launch intervals differ"
    );
    ensure!(
        supplied.len() == bindings.len(),
        "incomplete supplied interval"
    );
    let mut input_bytes = verifier.input_bytes;
    let mut query_count = 0usize;
    for (height, binding) in supplied.iter().zip(bindings) {
        input_bytes = check_supplied(
            height.height,
            &height.finality,
            &height.queries,
            binding,
            input_bytes,
            limits,
            &mut query_count,
        )?;
    }
    let remaining = limits
        .input_bytes
        .checked_sub(input_bytes)
        .ok_or_else(|| eyre!("input reservation underflow"))?;
    ensure!(
        reader_limits.max_output_bytes > 0 && reader_limits.max_output_bytes <= remaining,
        "reader returned-byte reservation exceeds remaining input"
    );
    // Core separately bounds its complete cold journal/log scan and decoder work.
    // Retained canonical bytes and all adapter copies use the smaller run budget.
    ensure!(
        reader_limits.max_store_data_bytes <= limits.input_bytes
            && reader_limits.max_merge_log_bytes <= limits.input_bytes
            && reader_limits.max_decode_allocation_bytes as u64 <= limits.admitted_proof_bytes * 2,
        "reader work bound exceeds admitted run scope"
    );
    let mut reader = CanonicalKuraEvidenceReader::open(block_store, merge_log, reader_limits)?;
    let mut heights = Vec::with_capacity(supplied.len());
    let mut requests = Vec::new();
    let mut reference_bytes = 0u64;
    for input in supplied {
        let wire = reader.read_carrier(input.height)?;
        input_bytes = charged(input_bytes, wire.len(), limits.input_bytes)?;
        let block = norito::with_decode_limits_scope(decode_limits(wire.len()), || {
            decode_versioned_signed_block(&wire)
        })?;
        if let Some(reference) = block
            .execution_context()
            .and_then(|c| c.merge_entry.as_ref())
        {
            reference_bytes = charged(
                reference_bytes,
                norito::canonical_frame_len(reference)?,
                limits.output_bytes,
            )?;
            requests.push(CanonicalKuraMergeRequest {
                carrier_height: input.height,
                reference: reference.clone(),
            });
        }
        heights.push(HeightProofV1 {
            height: input.height,
            finality: input.finality,
            carrier: wire,
            merge_entry: None,
            queries: input.queries,
        });
    }
    reader.scan_merge_entries(&requests, |height, _entry, bytes| {
        let index = height
            .checked_sub(first)
            .and_then(|n| usize::try_from(n).ok())
            .ok_or(CanonicalKuraEvidenceError::Invalid("adapter merge height"))?;
        let slot = heights
            .get_mut(index)
            .ok_or(CanonicalKuraEvidenceError::Invalid(
                "adapter merge interval",
            ))?;
        if slot.height != height || slot.merge_entry.is_some() {
            return Err(CanonicalKuraEvidenceError::Invalid(
                "adapter duplicate merge",
            ));
        }
        // Reserve both the retained bytes and this callback's live source copy
        // before allocating the one retained full entry. No per-transaction copy.
        input_bytes = input_bytes
            .checked_add(bytes.len() as u64)
            .filter(|n| *n <= limits.input_bytes)
            .ok_or(CanonicalKuraEvidenceError::Invalid(
                "adapter merge input budget",
            ))?;
        if reference_bytes
            .checked_add(bytes.len() as u64)
            .is_none_or(|n| n > limits.output_bytes)
        {
            return Err(CanonicalKuraEvidenceError::Invalid(
                "adapter merge copy budget",
            ));
        }
        slot.merge_entry = Some(bytes.to_vec());
        Ok(())
    })?;
    drop(requests);
    let authenticated = authenticate(verifier, &heights)?;
    before_disk_finish();
    let completed = reader.finish()?;
    ensure!(
        completed.carrier_count() == last - first + 1,
        "incomplete disk carrier interval"
    );
    let mut result = seal(heights, authenticated, limits)?;
    result.disk_completion = Some(completed);
    Ok(result)
}

/// Independently replay exact retained proof bytes under a fresh launch plan.
///
/// The expected artifact digest and input bindings are supplied by the caller's
/// retained authority. Size and digest admission precede decoding/copying. The
/// envelope's rows are compared field-for-field with recomputed authenticated
/// rows; no decoded or caller-constructed verdict is trusted.
pub fn replay_export(
    plan: TrustedRunPlan,
    limits: VerificationLimits,
    bindings: &[HeightInputBinding],
    expected_digest: Hash,
    canonical_bytes: &[u8],
) -> Result<VerifiedExport> {
    ensure!(
        !canonical_bytes.is_empty()
            && canonical_bytes.len() as u64 <= limits.output_bytes
            && limits.output_bytes <= limits.admitted_proof_bytes
            && limits.admitted_proof_bytes <= MAX_PROOF_BYTES,
        "export frame exceeds admitted allocation"
    );
    ensure!(
        Hash::new(canonical_bytes) == expected_digest,
        "export digest mismatch"
    );
    let verifier = admit(plan, limits, bindings)?;
    let projection = (verifier.expected.len() as u64)
        .checked_mul(ROW_RESERVATION + 1)
        .and_then(|n| n.checked_add(2))
        .ok_or_else(|| eyre!("projection reservation overflow"))?;
    ensure!(
        (canonical_bytes.len() as u64)
            .checked_add(projection)
            .is_some_and(|n| n <= limits.output_bytes),
        "replayable export and projection exceed output reservation"
    );
    // Reserve the entire decoded outer frame, not merely its inner bodies,
    // alongside the independently retained signed plan before the first decode.
    charged(
        verifier.input_bytes,
        canonical_bytes.len(),
        limits.input_bytes,
    )?;
    let standard = decode_limits(canonical_bytes.len());
    let allocation = usize::try_from(
        limits
            .admitted_proof_bytes
            .checked_mul(2)
            .ok_or_else(|| eyre!("export decode budget overflow"))?,
    )?;
    let envelope: ExportEnvelopeV1 = norito::decode_canonical_with_limits(
        canonical_bytes,
        norito::DecodeLimits::new(
            standard.max_sequence_elements(),
            standard.max_field_bytes(),
            standard.max_total_elements(),
            standard.max_total_allocated_bytes().min(allocation),
            128,
        ),
    )?;
    ensure!(
        envelope.version == 1
            && envelope.heights.len() == bindings.len()
            && envelope.rows.len() == verifier.expected.len(),
        "export V1 shape mismatch"
    );
    let mut input_bytes = verifier.input_bytes;
    let mut query_count = 0usize;
    for (height, binding) in envelope.heights.iter().zip(bindings) {
        input_bytes = check_supplied(
            height.height,
            &height.finality,
            &height.queries,
            binding,
            input_bytes,
            limits,
            &mut query_count,
        )?;
        bounded(&height.carrier, MAX_CARRIER_BYTES)?;
        input_bytes = charged(input_bytes, height.carrier.len(), limits.input_bytes)?;
        if let Some(entry) = &height.merge_entry {
            bounded(entry, MAX_MERGE_LEDGER_ENTRY_BYTES)?;
            input_bytes = charged(input_bytes, entry.len(), limits.input_bytes)?;
        }
    }
    let authenticated = authenticate(verifier, &envelope.heights)?;
    let AuthenticatedRun {
        rows,
        canonical: row_bytes,
        input_bytes: _,
    } = authenticated;
    drop(row_bytes);
    let rows = schedule_rows(rows);
    ensure!(
        same_rows(&rows, &envelope.rows),
        "export rows differ from independent authentication"
    );
    for row in &rows {
        drop(projection_row(row)?);
    }
    drop(envelope);
    // The borrowed frame already exists. Bound the returned frame copy before it
    // is made; no full proof re-encoding or full-entry-per-row expansion occurs.
    ensure!(
        canonical_bytes.len() as u64 <= limits.output_bytes,
        "export copy bound"
    );
    Ok(VerifiedExport {
        disk_completion: None,
        canonical: canonical_bytes.to_vec(),
        rows,
        output_limit: limits.output_bytes,
    })
}

fn admit(
    plan: TrustedRunPlan,
    limits: VerificationLimits,
    bindings: &[HeightInputBinding],
) -> Result<ScalingProofVerifier> {
    let verifier = ScalingProofVerifier::new(plan, limits)?;
    let count = verifier.plan.last_height - verifier.plan.first_height + 1;
    ensure!(
        bindings.len() as u64 == count,
        "input binding interval mismatch"
    );
    let mut query_count = 0usize;
    for (index, binding) in bindings.iter().enumerate() {
        ensure!(
            binding.height == verifier.plan.first_height + index as u64,
            "input binding height order"
        );
        query_count = query_count
            .checked_add(binding.query_hashes.len())
            .ok_or_else(|| eyre!("binding count overflow"))?;
        ensure!(query_count <= limits.requests, "input binding count bound");
    }
    let mut measurement = false;
    for expected in &verifier.expected {
        match expected.request.phase {
            WorkloadPhase::Measurement => measurement = true,
            WorkloadPhase::Warmup => ensure!(!measurement, "warmup follows measurement"),
        }
    }
    ensure!(measurement, "missing measurement schedule");
    Ok(verifier)
}

#[allow(clippy::too_many_arguments)]
fn check_supplied(
    height: u64,
    finality: &[u8],
    queries: &[Vec<u8>],
    binding: &HeightInputBinding,
    mut input_bytes: u64,
    limits: VerificationLimits,
    query_count: &mut usize,
) -> Result<u64> {
    ensure!(
        height == binding.height && queries.len() == binding.query_hashes.len(),
        "supplied input shape differs from binding"
    );
    bounded(finality, MAX_FINALITY_BYTES)?;
    input_bytes = charged(input_bytes, finality.len(), limits.input_bytes)?;
    *query_count = query_count
        .checked_add(queries.len())
        .ok_or_else(|| eyre!("query count overflow"))?;
    ensure!(
        *query_count <= limits.requests,
        "supplied query count bound"
    );
    // Size admission precedes hashing or decoding every externally supplied body.
    for query in queries {
        bounded(query, MAX_TRANSACTION_BYTES)?;
        input_bytes = charged(input_bytes, query.len(), limits.input_bytes)?;
    }
    ensure!(
        Hash::new(finality) == binding.finality_hash,
        "finality input digest mismatch"
    );
    for (query, digest) in queries.iter().zip(&binding.query_hashes) {
        ensure!(Hash::new(query) == *digest, "query input digest mismatch");
    }
    Ok(input_bytes)
}

fn authenticate(
    mut verifier: ScalingProofVerifier,
    heights: &[HeightProofV1],
) -> Result<AuthenticatedRun> {
    for height in heights {
        let queries: Vec<_> = height.queries.iter().map(Vec::as_slice).collect();
        verifier.push_height(
            &height.finality,
            &height.carrier,
            height.merge_entry.as_deref(),
            &queries,
        )?;
    }
    verifier.finish()
}

fn schedule_rows(rows: Vec<AuthenticatedRequest>) -> Vec<ExportRowV1> {
    let mut warmup = 0u64;
    let mut measurement = 0u64;
    rows.into_iter()
        .map(|request| {
            let counter = match request.phase {
                WorkloadPhase::Warmup => &mut warmup,
                WorkloadPhase::Measurement => &mut measurement,
            };
            // `admit` bounds the entire schedule to at most one million requests.
            *counter += 1;
            ExportRowV1 {
                sequence: *counter,
                request,
            }
        })
        .collect()
}

fn same_rows(left: &[ExportRowV1], right: &[ExportRowV1]) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(a, b)| {
            let (a_seq, b_seq) = (a.sequence, b.sequence);
            let (a, b) = (&a.request, &b.request);
            a_seq == b_seq
                && a.logical_id == b.logical_id
                && a.phase == b.phase
                && a.authority == b.authority
                && a.entrypoint_hash == b.entrypoint_hash
                && a.carrier_height == b.carrier_height
                && a.carrier_hash == b.carrier_hash
                && a.merge_entry_hash == b.merge_entry_hash
                && a.merge_epoch == b.merge_epoch
                && a.leaf_index == b.leaf_index
                && a.lane_id == b.lane_id
                && a.dataspace_id == b.dataspace_id
                && a.incarnation == b.incarnation
        })
}

fn seal(
    heights: Vec<HeightProofV1>,
    authenticated: AuthenticatedRun,
    limits: VerificationLimits,
) -> Result<VerifiedExport> {
    let AuthenticatedRun {
        rows,
        canonical: row_bytes,
        input_bytes,
    } = authenticated;
    drop(row_bytes);
    let mut raw_bytes = 0u64;
    for height in &heights {
        for bytes in std::iter::once(&height.finality)
            .chain(std::iter::once(&height.carrier))
            .chain(height.merge_entry.iter())
            .chain(height.queries.iter())
        {
            raw_bytes = charged(raw_bytes, bytes.len(), limits.input_bytes)?;
        }
    }
    let plan_bytes = input_bytes
        .checked_sub(raw_bytes)
        .ok_or_else(|| eyre!("authenticated input accounting mismatch"))?;
    let envelope = ExportEnvelopeV1 {
        version: 1,
        heights,
        rows: schedule_rows(rows),
    };
    // Reserve the mandatory JSON projection as well, so a successful artifact
    // always permits the strict bounded interop output from this same owner.
    let projection = (envelope.rows.len() as u64)
        .checked_mul(ROW_RESERVATION + 1)
        .and_then(|n| n.checked_add(2))
        .ok_or_else(|| eyre!("projection reservation overflow"))?;
    for row in &envelope.rows {
        drop(projection_row(row)?);
    }
    let size = norito::canonical_frame_len(&envelope)?;
    charged(plan_bytes, size, limits.input_bytes)?;
    ensure!(
        (size as u64)
            .checked_add(projection)
            .is_some_and(|n| n <= limits.output_bytes),
        "replayable export and projection exceed output reservation"
    );
    let canonical = norito::encode_canonical(&envelope)?;
    ensure!(canonical.len() == size, "export serialization changed size");
    Ok(VerifiedExport {
        disk_completion: None,
        canonical,
        rows: envelope.rows,
        output_limit: limits.output_bytes,
    })
}

#[cfg(test)]
#[path = "export/tests.rs"]
mod tests;
