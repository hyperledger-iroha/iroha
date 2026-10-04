//! Original complete canonical frame identity and signature custody through shared block clones.
use super::*;
use crate::block::{BlockHeader, BlockSignature, BlockSignatures, SharedSignedBlock};
use iroha_crypto::{Signature, SignatureOf};
fn fixture(count: usize) -> SignedBlock {
    let header = BlockHeader::new(std::num::NonZeroU64::new(2).unwrap(), None, None, 1_000, 0);
    SignedBlock {
        signatures: BlockSignatures::try_from_iter((0..count).map(|index| {
            BlockSignature::new(
                index as u64,
                SignatureOf::from_signature(
                    Signature::try_from_bytes(&vec![index as u8 + 1; 64]).unwrap(),
                ),
            )
        }))
        .unwrap(),
        payload: BlockPayload {
            header,
            external_entrypoints: Vec::new(),
            execution_context: None,
            da_commitments: None,
            da_proof_policies: None,
            da_pin_intents: None,
            npos_consensus_effects: None,
            global_beacon_pulse: None,
        },
        result: None,
        commit_certificate: None,
    }
}
fn source_for(block: &SignedBlock, pool: &AllocationBudget) -> (ChargedBuffer<u8>, SequenceSpan) {
    let wire = block.encode_wire().unwrap();
    let mut source = ChargedBuffer::new(wire.len() + 8, pool).unwrap();
    source.append(&[0xe1; 8]).unwrap();
    source.append(&wire).unwrap();
    (
        source,
        SequenceSpan {
            start: 8,
            end: 8 + wire.len(),
        },
    )
}
#[test]
fn complete_signed_block_prepared_signatures_preserve_canonical_nominal_frame_and_shared_owners() {
    for count in [0, 1, 4, 7, 31] {
        let pool = AllocationBudget::new(1024 * 1024);
        let block = fixture(count);
        let canonical_wire = block.encode_wire().unwrap();
        let ordinary = crate::block::decode_framed_signed_block(&canonical_wire).unwrap();
        assert_eq!(ordinary, block);
        assert_eq!(ordinary.encode_wire().unwrap(), canonical_wire);
        assert!(!ordinary.signatures.admitted_to(&pool));
        let (source, span) = source_for(&block, &pool);
        let source_floor = pool.reserved_bytes();
        let mut decoder = PreparedSignedBlockSignaturesDecode::new(&pool).unwrap();
        let controls = pool.reserved_bytes() - source_floor;
        assert_eq!(
            controls,
            PreparedDecodeWorkspace::allocation_layouts()
                .iter()
                .map(std::alloc::Layout::size)
                .sum()
        );
        let admitted = decoder
            .decode(
                &source,
                span,
                norito::canonical_decode_limits(span.end - span.start),
            )
            .unwrap();
        assert_eq!(admitted, block);
        assert_eq!(
            admitted.encode_wire().unwrap(),
            block.encode_wire().unwrap()
        );
        assert!(admitted.signatures.admitted_to(&pool));
        assert!(BlockSignatures::ptr_eq(
            &admitted.signatures,
            decoder.retained_signatures(&source).unwrap().unwrap()
        ));
        let retried = decoder
            .decode(
                &source,
                span,
                norito::canonical_decode_limits(span.end - span.start),
            )
            .unwrap();
        assert!(retried.same_signature_custody(&admitted));
        drop(retried);
        let pointers = admitted
            .signatures()
            .map(|sig| sig.signature().payload().as_ptr())
            .collect::<Vec<_>>();
        let retained = SharedSignedBlock::reserve(&pool)
            .unwrap()
            .initialize(admitted);
        let shared = retained.clone();
        let candidate_clone = retained.as_ref().clone();
        assert_eq!(
            candidate_clone
                .signatures()
                .map(|sig| sig.signature().payload().as_ptr())
                .collect::<Vec<_>>(),
            pointers
        );
        drop(retained);
        drop(shared);
        assert!(candidate_clone.signatures.admitted_to(&pool));
        drop(candidate_clone);
        drop(decoder);
        assert_eq!(pool.reserved_bytes(), source_floor);
        drop(source);
        assert_eq!(pool.reserved_bytes(), 0);
    }
}
#[test]
fn complete_frame_signature_capacity_refusal_retains_same_source_and_partial_collection() {
    let pool = AllocationBudget::new(1024 * 1024);
    let block = fixture(4);
    let (mut source, span) = source_for(&block, &pool);
    let (copy, _) = source_for(&block, &pool);
    let mut decoder = PreparedSignedBlockSignaturesDecode::new(&pool).unwrap();
    let backing: usize = BlockSignatures::backing_layouts(4)
        .unwrap()
        .iter()
        .map(std::alloc::Layout::size)
        .sum();
    let blocker = pool
        .try_reserve_bytes(pool.limit_bytes() - pool.reserved_bytes() - backing - 64)
        .unwrap();
    let error = decoder
        .decode(
            &source,
            span,
            norito::canonical_decode_limits(span.end - span.start),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        PreparedSignatureBlockError::Decode(PreparedDecodeError::Destination(
            BlockSignatureCustodyError::Buffer(ChargedBufferError::Admission(_))
        ))
    ));
    let pointer = decoder
        .pending
        .as_ref()
        .unwrap()
        .initialized(&source)
        .unwrap()[0]
        .signature()
        .payload()
        .as_ptr();
    assert!(matches!(
        decoder.decode(
            &copy,
            span,
            norito::canonical_decode_limits(span.end - span.start)
        ),
        Err(PreparedSignatureBlockError::SourceChanged)
    ));
    source.as_mut_slice()[0] ^= 1;
    assert!(matches!(
        decoder.decode(
            &source,
            span,
            norito::canonical_decode_limits(span.end - span.start)
        ),
        Err(PreparedSignatureBlockError::SourceChanged)
    ));
    source.as_mut_slice()[0] ^= 1;
    drop(blocker);
    let admitted = decoder
        .decode(
            &source,
            span,
            norito::canonical_decode_limits(span.end - span.start),
        )
        .unwrap();
    assert_eq!(
        admitted
            .signatures()
            .next()
            .unwrap()
            .signature()
            .payload()
            .as_ptr(),
        pointer
    );
    assert_eq!(admitted, block);
    assert!(decoder.belongs_to(&pool));
    drop(admitted);
    decoder.clear_consumed();
    assert!(decoder.pending.is_none());
    assert!(decoder.completed.is_none());
    assert!(decoder.source.is_none());
}
#[test]
fn prepared_signature_frame_rejects_foreign_input_and_malformed_canonical_bytes() {
    let pool = AllocationBudget::new(1024 * 1024);
    let foreign = AllocationBudget::new(1024 * 1024);
    let block = fixture(4);
    let (source, span) = source_for(&block, &foreign);
    let mut decoder = PreparedSignedBlockSignaturesDecode::new(&pool).unwrap();
    assert!(matches!(
        decoder.decode(
            &source,
            span,
            norito::canonical_decode_limits(span.end - span.start)
        ),
        Err(PreparedSignatureBlockError::Source)
    ));
    let (mut source, span) = source_for(&block, &pool);
    source.as_mut_slice()[span.end - 1] ^= 1;
    let error = decoder
        .decode(
            &source,
            span,
            norito::canonical_decode_limits(span.end - span.start),
        )
        .unwrap_err();
    assert!(
        matches!(error,PreparedSignatureBlockError::Frame(ref cause) if cause.kind()==norito::core::DecodeAttemptErrorKind::Invalid)
    );
}

#[test]
fn ordinary_and_prepared_blocks_share_the_single_generated_canonical_record_walk() {
    for count in [0, 1, 4, 7, 31] {
        let pool = AllocationBudget::new(1024 * 1024);
        let original = fixture(count);
        let wire = original.encode_wire().unwrap();
        let ordinary = crate::block::decode_framed_signed_block(&wire).unwrap();
        assert_eq!(ordinary, original);
        assert_eq!(ordinary.encode_wire().unwrap(), wire);
        assert!(!ordinary.signatures_admitted_to(&pool));
        let (source, span) = source_for(&original, &pool);
        let mut decoder = PreparedSignedBlockSignaturesDecode::new(&pool).unwrap();
        let prepared = decoder
            .decode(
                &source,
                span,
                norito::canonical_decode_limits(span.end - span.start),
            )
            .unwrap();
        assert_eq!(prepared, ordinary);
        assert_eq!(prepared.encode_wire().unwrap(), wire);
        assert!(prepared.signatures_admitted_to(&pool));
        assert!(BlockSignatures::ptr_eq(
            &prepared.signatures,
            decoder.retained_signatures(&source).unwrap().unwrap()
        ));
        let mut trailing = wire.clone();
        trailing.push(0);
        assert!(crate::block::decode_framed_signed_block(&trailing).is_err());
        let mut wrong_version = wire;
        wrong_version[0] = 2;
        assert!(crate::block::decode_framed_signed_block(&wrong_version).is_err());
    }
}
