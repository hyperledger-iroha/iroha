//! Exact public-content, occurrence-boundary and streaming-failure seal regressions.

use super::*;
use iroha_data_model::{
    asset::AssetDefinitionId,
    fastpq::{TransferDeltaTranscript, TransferSmtWitness},
};
use iroha_model_base::domain::DomainId;
use iroha_primitives::numeric::Quantity;
use iroha_test_samples::{ALICE_ID, BOB_ID};

fn delta(amount: u32, source_before: u32, destination_before: u32) -> TransferDeltaTranscript {
    TransferDeltaTranscript {
        from_account: (*ALICE_ID).clone(),
        to_account: (*BOB_ID).clone(),
        asset_definition: asset("rose"),
        amount: Quantity::from(amount),
        from_balance_before: Quantity::from(source_before),
        from_balance_after: Quantity::from(source_before - amount),
        to_balance_before: Quantity::from(destination_before),
        to_balance_after: Quantity::from(destination_before + amount),
        from_smt_witness: TransferSmtWitness::default(),
        to_smt_witness: TransferSmtWitness::default(),
    }
}

fn asset(name: &str) -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").unwrap(),
        name.parse().unwrap(),
    )
}

fn archive() -> BTreeMap<Hash, Vec<TransferTranscript>> {
    [1_u8, 2]
        .into_iter()
        .map(|seed| {
            let key = Hash::new([seed]);
            let first = TransferTranscript {
                batch_hash: key,
                deltas: vec![delta(1, 100, 0), delta(2, 99, 1)],
                authority_digest: Hash::new([seed, 3]),
                poseidon_preimage_digest: None,
            };
            let single = delta(3, 97, 3);
            let second = TransferTranscript {
                batch_hash: key,
                poseidon_preimage_digest: Some(crate::fastpq::poseidon_preimage_digest(
                    &single, &key,
                )),
                deltas: vec![single],
                authority_digest: Hash::new([seed, 4]),
            };
            (key, vec![first, second])
        })
        .collect()
}

fn append_frame<T: NoritoSerialize>(bytes: &mut Vec<u8>, value: &T) {
    bytes.extend(norito::encode_canonical(value).unwrap());
}

/// Independent buffered grammar reference, used only by these small test fixtures.
fn reference_stream(archive: &BTreeMap<Hash, Vec<TransferTranscript>>) -> Vec<u8> {
    // Intentionally repeat the specified domain rather than reusing the implementation constant.
    let mut bytes = b"iroha:fastpq:source-inventory:public-transcripts:v1\0".to_vec();
    append_frame(&mut bytes, &(archive.len() as u64));
    let mut transcript_count = 0_u64;
    let mut delta_count = 0_u64;
    for (key, bundle) in archive {
        append_frame(&mut bytes, key);
        append_frame(&mut bytes, &(bundle.len() as u64));
        transcript_count += bundle.len() as u64;
        for transcript in bundle {
            append_frame(&mut bytes, &transcript.batch_hash);
            append_frame(&mut bytes, &transcript.authority_digest);
            append_frame(&mut bytes, &transcript.poseidon_preimage_digest);
            append_frame(&mut bytes, &(transcript.deltas.len() as u64));
            delta_count += transcript.deltas.len() as u64;
            for delta in &transcript.deltas {
                append_frame(&mut bytes, &delta.from_account);
                append_frame(&mut bytes, &delta.to_account);
                append_frame(&mut bytes, &delta.asset_definition);
                append_frame(&mut bytes, &delta.amount);
                append_frame(&mut bytes, &delta.from_balance_before);
                append_frame(&mut bytes, &delta.from_balance_after);
                append_frame(&mut bytes, &delta.to_balance_before);
                append_frame(&mut bytes, &delta.to_balance_after);
            }
        }
    }
    append_frame(&mut bytes, &transcript_count);
    append_frame(&mut bytes, &delta_count);
    bytes
}

#[test]
fn streamed_seal_matches_complete_canonical_frame_reference() {
    let original = archive();
    let seal = seal_public_transcripts(&original).unwrap();
    assert_eq!(seal.transcript_count, 4);
    assert_eq!(seal.delta_count, 6);
    let reference = reference_stream(&original);
    assert_eq!(seal.digest, Hash::new(&reference));
    let mut written = Vec::new();
    assert_eq!(
        write_public_transcript_stream(&original, &mut written).unwrap(),
        (4, 6)
    );
    assert_eq!(written, reference);
    assert_ne!(
        seal.digest,
        Hash::new(&reference[PUBLIC_TRANSCRIPT_SEAL_DOMAIN.len()..])
    );
    assert_eq!(original, archive());
}

#[test]
fn seal_binds_every_public_field_and_exact_optional_digest() {
    let original = archive();
    let expected = seal_public_transcripts(&original).unwrap();
    let key = *original.keys().next().unwrap();
    for mutation in 0..14 {
        let mut changed = original.clone();
        let bundle = changed.get_mut(&key).unwrap();
        match mutation {
            0 => bundle[0].batch_hash = Hash::new(b"substituted batch"),
            1 => bundle[0].authority_digest = Hash::new(b"substituted authority"),
            2 => bundle[0].poseidon_preimage_digest = Some(Hash::new(b"inserted digest")),
            3 => bundle[1].poseidon_preimage_digest = None,
            4 => bundle[1].poseidon_preimage_digest = Some(Hash::new(b"substituted digest")),
            5 => bundle[0].deltas[0].from_account = (*BOB_ID).clone(),
            6 => bundle[0].deltas[0].to_account = (*ALICE_ID).clone(),
            7 => bundle[0].deltas[0].asset_definition = asset("lily"),
            8 => bundle[0].deltas[0].amount = Quantity::from(7_u32),
            9 => bundle[0].deltas[0].from_balance_before = Quantity::from(101_u32),
            10 => bundle[0].deltas[0].from_balance_after = Quantity::from(98_u32),
            11 => bundle[0].deltas[0].to_balance_before = Quantity::from(2_u32),
            12 => bundle[0].deltas[0].to_balance_after = Quantity::from(3_u32),
            13 => {
                let bundle = changed.remove(&key).unwrap();
                changed.insert(Hash::new(b"substituted archive key"), bundle);
            }
            _ => unreachable!(),
        }
        assert_ne!(
            seal_public_transcripts(&changed).unwrap().digest,
            expected.digest,
            "public substitution {mutation}"
        );
    }
}

#[test]
fn seal_binds_occurrence_order_grouping_and_all_counts() {
    let original = archive();
    let expected = seal_public_transcripts(&original).unwrap();
    let keys: Vec<_> = original.keys().copied().collect();
    for mutation in 0..10 {
        let mut changed = original.clone();
        let bundle = changed.get_mut(&keys[0]).unwrap();
        match mutation {
            0 => bundle.swap(0, 1),
            1 => bundle[0].deltas.swap(0, 1),
            2 => bundle.push(bundle[0].clone()),
            3 => {
                bundle.pop();
            }
            4 => {
                let duplicate = bundle[0].deltas[0].clone();
                bundle[0].deltas.push(duplicate);
            }
            5 => {
                bundle[0].deltas.pop();
            }
            6 => {
                // Preserve every delta and transcript header but change the atomic grouping.
                let moved = bundle[0].deltas.pop().unwrap();
                bundle[1].deltas.insert(0, moved);
            }
            7 => {
                let moved = bundle.pop().unwrap();
                changed.get_mut(&keys[1]).unwrap().push(moved);
            }
            8 => {
                let first = changed.remove(&keys[0]).unwrap();
                let second = changed.insert(keys[1], first).unwrap();
                changed.insert(keys[0], second);
            }
            9 => {
                changed.remove(&keys[0]);
            }
            _ => unreachable!(),
        }
        assert_ne!(
            seal_public_transcripts(&changed).unwrap().digest,
            expected.digest,
            "occurrence substitution {mutation}"
        );
    }
    let reverse_inserted = original.into_iter().rev().collect();
    assert_eq!(
        seal_public_transcripts(&reverse_inserted).unwrap(),
        expected
    );
}

#[test]
fn empty_maps_bundles_and_transcripts_have_distinct_exact_commitments() {
    let mut changed = BTreeMap::new();
    let empty = seal_public_transcripts(&changed).unwrap();
    assert_eq!((empty.transcript_count, empty.delta_count), (0, 0));
    assert_eq!(empty.digest, Hash::new(reference_stream(&changed)));
    let key = Hash::new(b"empty bundle");
    changed.insert(key, Vec::new());
    let bundle = seal_public_transcripts(&changed).unwrap();
    assert_eq!((bundle.transcript_count, bundle.delta_count), (0, 0));
    assert_ne!(bundle.digest, empty.digest);
    let mut transcript = archive().into_values().next().unwrap().remove(0);
    transcript.deltas.clear();
    changed.get_mut(&key).unwrap().push(transcript);
    let occurrence = seal_public_transcripts(&changed).unwrap();
    assert_eq!(
        (occurrence.transcript_count, occurrence.delta_count),
        (1, 0)
    );
    assert_ne!(occurrence.digest, bundle.digest);
    assert_eq!(occurrence.digest, Hash::new(reference_stream(&changed)));
}

#[test]
fn seal_excludes_only_private_smt_paths_and_retains_input_ownership() {
    let mut original = archive();
    let expected = seal_public_transcripts(&original).unwrap();
    for bundle in original.values_mut() {
        for transcript in bundle {
            for delta in &mut transcript.deltas {
                delta.from_smt_witness =
                    TransferSmtWitness::new([1; 32], [2; 32], vec![3; 1_024], vec![[4; 32]; 1_024]);
                delta.to_smt_witness =
                    TransferSmtWitness::new([5; 32], [6; 32], vec![7; 2_048], vec![[8; 32]; 2_048]);
            }
        }
    }
    let before = original.clone();
    assert_eq!(seal_public_transcripts(&original).unwrap(), expected);
    assert_eq!(original, before);
}

#[test]
fn seal_uses_canonical_flags_and_restores_every_ambient_layout() {
    let original = archive();
    let expected = seal_public_transcripts(&original).unwrap();
    for flags in u8::MIN..=u8::MAX {
        if norito::core::validate_header_flags(flags).is_err() {
            continue;
        }
        let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(seal_public_transcripts(&original).unwrap(), expected);
        assert_eq!(norito::core::effective_decode_flags(), Some(flags));
    }
}

#[test]
fn count_accumulation_rejects_overflow() {
    assert_eq!(add_count(0, 0).unwrap(), 0);
    assert_eq!(add_count(u64::MAX - 1, 1).unwrap(), u64::MAX);
    let error = add_count(u64::MAX, 1).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
}

struct FailAfter<'a> {
    inner: &'a mut dyn Write,
    remaining: usize,
}

impl Write for FailAfter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "injected writer failure",
            ));
        }
        let count = bytes.len().min(self.remaining);
        self.inner.write_all(&bytes[..count])?;
        self.remaining -= count;
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

#[test]
fn streaming_writer_errors_never_return_a_partial_digest() {
    let original = archive();
    let bytes = reference_stream(&original);
    for accepted in [
        0,
        PUBLIC_TRANSCRIPT_SEAL_DOMAIN.len(),
        bytes.len() / 2,
        bytes.len() - 1,
    ] {
        let error = Hash::new_from_writer(|writer| {
            let mut failing = FailAfter {
                inner: writer,
                remaining: accepted,
            };
            write_public_transcript_stream(&original, &mut failing).map(|_| ())
        })
        .unwrap_err();
        assert!(error.to_string().contains("injected writer failure"));
    }
    assert_eq!(
        seal_public_transcripts(&original).unwrap().digest,
        Hash::new(bytes)
    );
}

/// A serializer-owned allocation charge tests the adapter's inherited-budget error path.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "test::iroha_core::BudgetedField", frame = "u8")]
struct BudgetedField;

impl norito::SerializePayload for BudgetedField {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        norito::core::reserve_decode_allocation(1)?;
        writer.write_all(&[42])?;
        Ok(())
    }
}

#[test]
fn canonical_budgeted_field_has_the_exact_primitive_frame() {
    let mut actual = Vec::new();
    write_canonical_field(&mut actual, &BudgetedField).unwrap();
    let mut expected = Vec::new();
    write_canonical_field(&mut expected, &42_u8).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(norito::decode_canonical::<u8>(&actual).unwrap(), 42);
}

#[test]
fn canonical_field_preserves_inherited_serializer_resource_errors() {
    // Concrete borrowed public serializers need no archive-sized encode buffer. This stand-in
    // exercises a fallible serializer at the same boundary without claiming that a decoder
    // allocation budget accounts for every small numeric serializer allocation.
    let limits = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 32);
    let result = norito::core::with_decode_limits_scope(limits, || {
        Hash::new_from_writer(|writer| write_canonical_field(writer, &BudgetedField))
    });
    assert!(result.is_err());
    assert_eq!(
        Hash::new_from_writer(|writer| write_canonical_field(writer, &BudgetedField)).unwrap(),
        Hash::new_from_writer(|writer| write_canonical_field(writer, &42_u8)).unwrap(),
        "the budgeted byte serializer must retain its primitive frame identity"
    );
}

#[test]
fn ordinary_bundle_adapter_preserves_original_map_stream_and_field_ownership() {
    for original in [BTreeMap::new(), archive()] {
        let expected = seal_public_transcripts(&original).unwrap();
        let reference = reference_stream(&original);
        let mut bundles: Vec<TransferTranscriptBundle> = original
            .iter()
            .map(|(entry_hash, transcripts)| TransferTranscriptBundle {
                entry_hash: *entry_hash,
                transcripts: transcripts.clone(),
            })
            .collect();
        assert_eq!(seal_public_transcript_bundles(&bundles).unwrap(), expected);
        assert_eq!(expected.digest, Hash::new(reference));
        for bundle in &mut bundles {
            for transcript in &mut bundle.transcripts {
                for delta in &mut transcript.deltas {
                    delta.from_smt_witness = TransferSmtWitness::new(
                        [11; 32],
                        [12; 32],
                        vec![13; 64],
                        vec![[14; 32]; 64],
                    );
                    delta.to_smt_witness = TransferSmtWitness::new(
                        [21; 32],
                        [22; 32],
                        vec![23; 65],
                        vec![[24; 32]; 65],
                    );
                }
            }
        }
        let before = bundles.clone();
        assert_eq!(seal_public_transcript_bundles(&bundles).unwrap(), expected);
        assert_eq!(bundles, before);
    }
}

#[test]
fn ordinary_bundle_adapter_is_canonical_under_ambient_flags() {
    let original = archive();
    let expected = seal_public_transcripts(&original).unwrap();
    let bundles: Vec<_> = original
        .into_iter()
        .map(|(entry_hash, transcripts)| TransferTranscriptBundle {
            entry_hash,
            transcripts,
        })
        .collect();
    for flags in u8::MIN..=u8::MAX {
        if norito::core::validate_header_flags(flags).is_err() {
            continue;
        }
        let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(seal_public_transcript_bundles(&bundles).unwrap(), expected);
        assert_eq!(norito::core::effective_decode_flags(), Some(flags));
    }
}
