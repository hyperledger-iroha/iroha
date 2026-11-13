//! Strict prefix parsing and original decoder-refusal regression controls.

use super::*;
use crate::block::BlockSignature;
use iroha_crypto::{Algorithm, KeyPair, SignatureOf};
use std::num::NonZeroU64;

fn proposal() -> (BlockHeader, Vec<u8>) {
    let header = BlockHeader::new(NonZeroU64::new(9).unwrap(), None, None, 123_456, 2);
    let key = KeyPair::from_seed(vec![0x63; 32], Algorithm::Ed25519);
    let signature = BlockSignature::new(
        0,
        SignatureOf::try_from_hash(key.private_key(), header.hash()).unwrap(),
    );
    let block = SignedBlock::presigned(signature, header, Vec::new());
    let mut bytes = Vec::new();
    block.write_resultless_proposal_wire(&mut bytes).unwrap();
    (header, bytes)
}

fn length_prefix(value: usize) -> Vec<u8> {
    let _flags = DecodeFlagsGuard::enter(default_encode_flags());
    let mut bytes = Vec::new();
    norito::core::write_len_header_to_vec(&mut bytes, value as u64);
    bytes
}

fn replace_length(bytes: &mut Vec<u8>, offset: usize, replacement: &[u8]) {
    let _flags = DecodeFlagsGuard::enter(default_encode_flags());
    let (_, used) = inspect_len_from_slice(&bytes[offset..]).unwrap();
    bytes.splice(offset..offset + used, replacement.iter().copied());
    let payload_len = (bytes.len() - 1 - Header::SIZE) as u64;
    bytes[24..32].copy_from_slice(&payload_len.to_le_bytes());
}

#[test]
fn proposal_prefix_decodes_original_header_at_every_partial_boundary() {
    let (header, bytes) = proposal();
    let parsed = parse_proposal_prefix(&bytes, bytes.len()).unwrap().unwrap();
    assert_eq!(parsed.header(), &header);
    let end = parsed.consumed_len();
    assert!(end < bytes.len());
    for length in 0..end {
        assert_eq!(
            parse_proposal_prefix(&bytes[..length], bytes.len()).unwrap(),
            None
        );
    }
    assert_eq!(
        parse_proposal_prefix(&bytes[..end], bytes.len()).unwrap(),
        Some(parsed)
    );
    let mut changed_body = bytes.clone();
    *changed_body.last_mut().unwrap() ^= 1;
    assert_eq!(
        parse_proposal_prefix(&changed_body, bytes.len()).unwrap(),
        Some(parsed)
    );
    assert!(crate::block::decode_framed_signed_block(&changed_body).is_err());
}

#[test]
fn proposal_prefix_rejects_archive_metadata_and_total_length_substitution() {
    let (_, original) = proposal();
    for offset in [0, 1, 5, 6, 7, 23, 40] {
        let mut changed = original.clone();
        changed[offset] ^= 1;
        assert!(
            parse_proposal_prefix(&changed, changed.len()).is_err(),
            "offset {offset}"
        );
    }
    let mut changed = original.clone();
    changed[24] ^= 1;
    assert!(parse_proposal_prefix(&changed, changed.len()).is_err());
    assert!(parse_proposal_prefix(&original, original.len() + 1).is_err());
    assert!(parse_proposal_prefix(&original, original.len() - 1).is_err());
    assert!(parse_proposal_prefix(&[], 1 + Header::SIZE).is_err());
}

#[test]
fn proposal_prefix_rejects_impossible_parent_extents_and_nonminimal_lengths() {
    let (_, original) = proposal();
    let _flags = DecodeFlagsGuard::enter(default_encode_flags());
    let signatures = extent(&original, 1 + Header::SIZE, original.len())
        .unwrap()
        .unwrap();
    let payload = extent(&original, signatures.end, original.len())
        .unwrap()
        .unwrap();
    for (offset, value) in [
        (1 + Header::SIZE, MAX_PROPOSAL_PREFIX_BYTES),
        (signatures.end, original.len()),
        (payload.start, payload.len() + 1),
    ] {
        let mut changed = original.clone();
        replace_length(&mut changed, offset, &length_prefix(value));
        assert!(parse_proposal_prefix(&changed, changed.len()).is_err());
    }
    let mut changed = original.clone();
    let mut nonminimal = length_prefix(signatures.len());
    *nonminimal.last_mut().unwrap() |= 0x80;
    nonminimal.push(0);
    replace_length(&mut changed, 1 + Header::SIZE, &nonminimal);
    assert!(parse_proposal_prefix(&changed, changed.len()).is_err());
    assert!(extent(&[], 3, 3).is_err());
    assert!(bounded_prefix(MAX_PROPOSAL_PREFIX_BYTES + 1).is_err());
}

#[test]
fn proposal_prefix_rejects_signature_count_overflow_and_invalid_canonical_header() {
    let (_, original) = proposal();
    let _flags = DecodeFlagsGuard::enter(default_encode_flags());
    let signatures = extent(&original, 1 + Header::SIZE, original.len())
        .unwrap()
        .unwrap();
    let payload = extent(&original, signatures.end, original.len())
        .unwrap()
        .unwrap();
    let header = extent(&original, payload.start, payload.end)
        .unwrap()
        .unwrap();
    let height = extent(&original, header.start, header.end)
        .unwrap()
        .unwrap();
    let mut changed = original.clone();
    changed[signatures.start..signatures.start + 8].copy_from_slice(&32_u64.to_le_bytes());
    assert!(parse_proposal_prefix(&changed, changed.len()).is_err());
    let mut changed = original.clone();
    changed[height].fill(0); // NonZeroU64 must reject zero, even in a partial frame.
    assert!(parse_proposal_prefix(&changed, changed.len()).is_err());
}

#[test]
fn proposal_prefix_preserves_original_enclosing_decoder_refusal() {
    let (_, bytes) = proposal();
    let refusal = norito::core::with_decode_limits_scope(
        norito::core::DecodeLimits::new(usize::MAX, 1, usize::MAX, usize::MAX, usize::MAX),
        || parse_proposal_prefix(&bytes, bytes.len()),
    )
    .unwrap_err();
    assert_eq!(
        refusal.kind(),
        norito::core::DecodeAttemptErrorKind::EnclosingLimit
    );
    assert!(refusal.local_refusal().is_some());
    assert!(
        parse_proposal_prefix(&bytes, bytes.len())
            .unwrap()
            .is_some()
    );
}

#[test]
fn proposal_prefix_rejects_exhausted_or_overflowing_partial_length_headers() {
    let (_, mut bytes) = proposal();
    let start = 1 + Header::SIZE;
    bytes[start..start + 10].fill(0x80);
    assert!(parse_proposal_prefix(&bytes[..start + 10], bytes.len()).is_err());
    assert_eq!(
        parse_proposal_prefix(&bytes[..start + 9], bytes.len()).unwrap(),
        None
    );
    assert!(
        parse_proposal_prefix(
            &vec![0; MAX_PROPOSAL_PREFIX_BYTES + 1],
            MAX_PROPOSAL_PREFIX_BYTES + 1
        )
        .is_err()
    );
}
