//! Local commitment to the exact ordered public facts in a finalized transcript archive.
//!
//! This private stream is not a network wire type or a finality attestation. Its encoding is
//! the raw `PUBLIC_TRANSCRIPT_SEAL_DOMAIN` followed by complete canonical Norito frames:
//! map length (`u64`); then, in `BTreeMap` key order, key (`Hash`) and bundle length (`u64`);
//! then, in original bundle order, batch hash, authority digest, exact `Option<Hash>` Poseidon
//! digest and delta length (`u64`); then, in original delta order, source account, destination
//! account, asset definition, amount, source-before, source-after, destination-before and
//! destination-after. Finally, two `u64` frames commit the total transcript and delta counts.
//! Only the two private SMT witnesses of each delta are excluded. Empty bundles and transcripts
//! are committed exactly; semantic validation and resource preflight belong to the caller.
//!
//! Each field is borrowed and streamed directly into the hasher. Canonical framing uses a
//! count/checksum pass followed by a checked write pass, without retaining the complete archive,
//! a public projection or an archive-sized encoded buffer. Serializer scratch is not claimed to be zero:
//! account controllers use count-first canonical field/sequence writers, hashes and asset IDs
//! write fixed bytes, and `Quantity` serialization clones its bounded numeric mantissa and emits
//! bounded two's-complement scratch. None of these operations inspect or clone private paths.

use std::{
    collections::BTreeMap,
    io::{self, Write},
};

use iroha_crypto::Hash;
use iroha_data_model::fastpq::{TransferTranscript, TransferTranscriptBundle};
use norito::NoritoSerialize;

/// Fixed identity for this local public-content commitment grammar.
const PUBLIC_TRANSCRIPT_SEAL_DOMAIN: &[u8] =
    b"iroha:fastpq:source-inventory:public-transcripts:v1\0";

/// Exact local public-content digest and checked archive counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SourceTranscriptSeal {
    /// Domain-separated commitment to all public facts and occurrence boundaries.
    pub(super) digest: Hash,
    /// Complete number of transcript occurrences across every bundle.
    pub(super) transcript_count: u64,
    /// Complete number of deltas across every original transcript.
    pub(super) delta_count: u64,
}

/// Seal borrowed public facts without constructing a projection or copying private paths.
///
/// Canonical layout is independent of ambient flags. Serializer and writer errors discard the
/// entire in-progress hash; no partial seal is returned. This helper does not normalize quantities,
/// add missing digests, validate transfer arithmetic or authenticate the supplied archive.
pub(super) fn seal_public_transcripts(
    transcripts: &BTreeMap<Hash, Vec<TransferTranscript>>,
) -> Result<SourceTranscriptSeal, String> {
    seal_public_transcript_entries(
        transcripts
            .iter()
            .map(|(key, bundle)| (key, bundle.as_slice())),
    )
}

/// Seal an already canonical ordinary bundle sequence through the identical borrowed stream.
///
/// This adapter does not sort, merge, or repair bundles. The owning inventory verifier must
/// check exact sorted keys, ordinary outer/inner identities, shapes and counts before calling it.
pub(super) fn seal_public_transcript_bundles(
    bundles: &[TransferTranscriptBundle],
) -> Result<SourceTranscriptSeal, String> {
    seal_public_transcript_entries(
        bundles
            .iter()
            .map(|bundle| (&bundle.entry_hash, bundle.transcripts.as_slice())),
    )
}

fn seal_public_transcript_entries<'a>(
    entries: impl ExactSizeIterator<Item = (&'a Hash, &'a [TransferTranscript])>,
) -> Result<SourceTranscriptSeal, String> {
    let mut counts = (0, 0);
    let digest = Hash::new_from_writer(|writer| {
        counts = write_public_transcript_entries(entries, writer)?;
        Ok(())
    })
    .map_err(|error| format!("FASTPQ source public transcript sealing failed: {error}"))?;
    Ok(SourceTranscriptSeal {
        digest,
        transcript_count: counts.0,
        delta_count: counts.1,
    })
}

/// Retain the original map-stream test seam while sharing production grammar with bundles.
#[cfg(test)]
fn write_public_transcript_stream(
    transcripts: &BTreeMap<Hash, Vec<TransferTranscript>>,
    writer: &mut dyn Write,
) -> io::Result<(u64, u64)> {
    write_public_transcript_entries(
        transcripts
            .iter()
            .map(|(key, bundle)| (key, bundle.as_slice())),
        writer,
    )
}

/// Write the unchanged private grammar from a borrowed, exact-length entry sequence.
fn write_public_transcript_entries<'a>(
    entries: impl ExactSizeIterator<Item = (&'a Hash, &'a [TransferTranscript])>,
    writer: &mut dyn Write,
) -> io::Result<(u64, u64)> {
    writer.write_all(PUBLIC_TRANSCRIPT_SEAL_DOMAIN)?;
    write_canonical_field(writer, &add_count(0, entries.len())?)?;
    let mut transcript_count = 0;
    let mut delta_count = 0;
    for (key, bundle) in entries {
        write_canonical_field(writer, key)?;
        let bundle_count = add_count(0, bundle.len())?;
        transcript_count = add_count(transcript_count, bundle.len())?;
        write_canonical_field(writer, &bundle_count)?;
        for transcript in bundle {
            write_canonical_field(writer, &transcript.batch_hash)?;
            write_canonical_field(writer, &transcript.authority_digest)?;
            write_canonical_field(writer, &transcript.poseidon_preimage_digest)?;
            let deltas = add_count(0, transcript.deltas.len())?;
            delta_count = add_count(delta_count, transcript.deltas.len())?;
            write_canonical_field(writer, &deltas)?;
            for delta in &transcript.deltas {
                write_canonical_field(writer, &delta.from_account)?;
                write_canonical_field(writer, &delta.to_account)?;
                write_canonical_field(writer, &delta.asset_definition)?;
                write_canonical_field(writer, &delta.amount)?;
                write_canonical_field(writer, &delta.from_balance_before)?;
                write_canonical_field(writer, &delta.from_balance_after)?;
                write_canonical_field(writer, &delta.to_balance_before)?;
                write_canonical_field(writer, &delta.to_balance_after)?;
            }
        }
    }
    write_canonical_field(writer, &transcript_count)?;
    write_canonical_field(writer, &delta_count)?;
    Ok((transcript_count, delta_count))
}

/// Preserve count overflow as a fallible producer error before publishing a seal.
fn add_count(total: u64, length: usize) -> io::Result<u64> {
    u64::try_from(length)
        .ok()
        .and_then(|length| total.checked_add(length))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "FASTPQ source public transcript count exceeds u64",
            )
        })
}

/// Retain nominal framing and serializer errors through the incremental hash writer.
fn write_canonical_field<T: NoritoSerialize>(writer: &mut dyn Write, value: &T) -> io::Result<()> {
    norito::core::write_canonical_to_writer(value, writer).map_err(io::Error::other)
}

#[cfg(test)]
mod tests;
