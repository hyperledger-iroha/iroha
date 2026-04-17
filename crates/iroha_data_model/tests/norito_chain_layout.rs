//! Regression coverage for Norito chain wire layout candidates.

use std::fmt::Debug;

use iroha_crypto::{PrivateKey, PublicKey};
use iroha_data_model::{
    ChainId, Level,
    account::AccountId,
    block::SignedBlock,
    isi::{InstructionBox, Log},
    transaction::signed::{SignedTransaction, TransactionBuilder, TransactionEntrypoint},
};
use norito::core::{DecodeFlagsGuard, Header, frame_bare_with_header_flags, header_flags};

#[derive(Clone, Copy)]
struct LayoutCandidate {
    name: &'static str,
    requested_flags: u8,
}

const LAYOUT_CANDIDATES: &[LayoutCandidate] = &[
    LayoutCandidate {
        name: "canonical",
        requested_flags: 0,
    },
    LayoutCandidate {
        name: "compact_len",
        requested_flags: header_flags::COMPACT_LEN,
    },
    LayoutCandidate {
        name: "packed_struct",
        requested_flags: header_flags::COMPACT_LEN
            | header_flags::PACKED_STRUCT
            | header_flags::FIELD_BITSET,
    },
    LayoutCandidate {
        name: "packed_all",
        requested_flags: header_flags::COMPACT_LEN
            | header_flags::PACKED_STRUCT
            | header_flags::FIELD_BITSET
            | header_flags::PACKED_SEQ,
    },
];

fn fixed_public_key() -> PublicKey {
    "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
        .parse()
        .expect("fixed public key")
}

fn fixed_private_key() -> PrivateKey {
    "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
        .parse()
        .expect("fixed private key")
}

fn sample_transaction(instruction_count: usize) -> SignedTransaction {
    let private_key = fixed_private_key();
    let authority = AccountId::new(fixed_public_key());
    let chain: ChainId = "norito-chain-layout-test".parse().expect("chain id");
    let instructions = (0..instruction_count)
        .map(|index| {
            InstructionBox::from(Log::new(
                Level::INFO,
                format!("layout instruction {index}").into(),
            ))
        })
        .collect::<Vec<_>>();

    TransactionBuilder::new(chain, authority)
        .with_instructions(instructions)
        .sign(&private_key)
}

fn sample_block(transaction_count: usize, instruction_count: usize) -> SignedBlock {
    let private_key = fixed_private_key();
    let transactions = (0..transaction_count)
        .map(|_| sample_transaction(instruction_count))
        .collect::<Vec<_>>();

    SignedBlock::genesis(transactions, &private_key, None, None)
}

fn candidate_by_name(name: &str) -> LayoutCandidate {
    LAYOUT_CANDIDATES
        .iter()
        .copied()
        .find(|candidate| candidate.name == name)
        .expect("known layout candidate")
}

fn layout_payload_with_flags<T>(value: &T, candidate: LayoutCandidate) -> (Vec<u8>, u8)
where
    T: norito::NoritoSerialize,
{
    let _guard = DecodeFlagsGuard::enter(candidate.requested_flags);
    let (payload, flags) = norito::codec::encode_with_header_flags(value);
    let expected = candidate.requested_flags;
    assert_eq!(
        flags & expected,
        expected,
        "{} did not advertise requested layout flags",
        candidate.name
    );
    (payload, flags)
}

fn framed_with_layout<T>(value: &T, candidate: LayoutCandidate) -> Vec<u8>
where
    T: norito::NoritoSerialize,
{
    let (payload, flags) = layout_payload_with_flags(value, candidate);
    frame_bare_with_header_flags::<T>(&payload, flags).expect("frame layout payload")
}

fn header_flags_from(bytes: &[u8]) -> u8 {
    bytes[Header::SIZE - 1]
}

fn assert_default_frame_matches_compact<T>(label: &str, value: &T)
where
    T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de> + PartialEq + Debug,
{
    let compact = framed_with_layout(value, candidate_by_name("compact_len"));
    let default = norito::core::to_bytes(value).expect("encode default framed payload");

    assert_eq!(default, compact, "{label} default frame must be compact");

    let flags = header_flags_from(&default);
    assert_eq!(
        flags & header_flags::COMPACT_LEN,
        header_flags::COMPACT_LEN,
        "{label} default frame must advertise compact lengths"
    );
    assert_eq!(
        flags
            & (header_flags::PACKED_STRUCT | header_flags::FIELD_BITSET | header_flags::PACKED_SEQ),
        0,
        "{label} default frame must not advertise experimental packed layouts"
    );

    let decoded: T = norito::decode_from_bytes(&default).expect("decode default framed payload");
    assert_eq!(decoded, *value, "{label} default frame roundtrip");
}

fn assert_compact_payload_is_smaller<T>(label: &str, value: &T)
where
    T: norito::NoritoSerialize,
{
    let (canonical, _) = layout_payload_with_flags(value, candidate_by_name("canonical"));
    let (compact, _) = layout_payload_with_flags(value, candidate_by_name("compact_len"));
    assert!(
        compact.len() < canonical.len(),
        "{label} compact payload should be smaller than canonical: compact={} canonical={}",
        compact.len(),
        canonical.len()
    );
}

fn assert_wrong_header_rejects_packed_payload<T>(label: &str, value: &T)
where
    T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>,
{
    let (packed_payload, packed_flags) =
        layout_payload_with_flags(value, candidate_by_name("packed_all"));
    assert_eq!(
        packed_flags & header_flags::PACKED_SEQ,
        header_flags::PACKED_SEQ,
        "{label} packed payload must advertise packed sequences before the rejection check"
    );

    let wrong_frame = frame_bare_with_header_flags::<T>(&packed_payload, header_flags::COMPACT_LEN)
        .expect("frame packed payload");
    assert!(
        norito::decode_from_bytes::<T>(&wrong_frame).is_err(),
        "{label} packed payload decoded despite missing packed-layout header flags"
    );
}

fn assert_truncated_layout_candidates_reject<T>(label: &str, value: &T)
where
    T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>,
{
    for &candidate in LAYOUT_CANDIDATES {
        let mut framed = framed_with_layout(value, candidate);
        framed.pop().expect("non-empty framed payload");
        assert!(
            norito::decode_from_bytes::<T>(&framed).is_err(),
            "{label}/{} truncated frame decoded successfully",
            candidate.name
        );
    }
}

#[test]
fn signed_transaction_layout_candidates_decode_from_framed_bytes() {
    let tx = sample_transaction(8);

    for &candidate in LAYOUT_CANDIDATES {
        let framed = framed_with_layout(&tx, candidate);
        let decoded: SignedTransaction = norito::decode_from_bytes(&framed).unwrap_or_else(|err| {
            panic!(
                "{} signed transaction decode failed: {err:?}",
                candidate.name
            )
        });

        assert_eq!(
            decoded, tx,
            "{} signed transaction roundtrip",
            candidate.name
        );
    }
}

#[test]
fn transaction_entrypoint_layout_candidates_decode_from_framed_bytes() {
    let entrypoint = TransactionEntrypoint::from(sample_transaction(8));

    for &candidate in LAYOUT_CANDIDATES {
        let framed = framed_with_layout(&entrypoint, candidate);
        let decoded: TransactionEntrypoint =
            norito::decode_from_bytes(&framed).unwrap_or_else(|err| {
                panic!(
                    "{} transaction entrypoint decode failed: {err:?}",
                    candidate.name
                )
            });

        assert_eq!(
            decoded, entrypoint,
            "{} transaction entrypoint roundtrip",
            candidate.name
        );
    }
}

#[test]
fn signed_block_layout_candidates_decode_from_framed_bytes() {
    let block = sample_block(4, 4);

    for &candidate in LAYOUT_CANDIDATES {
        let framed = framed_with_layout(&block, candidate);
        let decoded: SignedBlock = norito::decode_from_bytes(&framed)
            .unwrap_or_else(|err| panic!("{} signed block decode failed: {err:?}", candidate.name));

        assert_eq!(decoded, block, "{} signed block roundtrip", candidate.name);
    }
}

#[test]
fn default_framed_chain_payloads_advertise_compact_lengths() {
    assert_default_frame_matches_compact("signed_transaction", &sample_transaction(8));
    assert_default_frame_matches_compact(
        "transaction_entrypoint",
        &TransactionEntrypoint::from(sample_transaction(8)),
    );
    assert_default_frame_matches_compact("signed_block", &sample_block(4, 4));
}

#[test]
fn compact_lengths_reduce_chain_payload_sizes() {
    assert_compact_payload_is_smaller("signed_transaction", &sample_transaction(8));
    assert_compact_payload_is_smaller(
        "transaction_entrypoint",
        &TransactionEntrypoint::from(sample_transaction(8)),
    );
    assert_compact_payload_is_smaller("signed_block", &sample_block(4, 4));
}

#[test]
fn wrong_header_flags_reject_packed_chain_payloads() {
    assert_wrong_header_rejects_packed_payload("signed_transaction", &sample_transaction(8));
    assert_wrong_header_rejects_packed_payload(
        "transaction_entrypoint",
        &TransactionEntrypoint::from(sample_transaction(8)),
    );
    assert_wrong_header_rejects_packed_payload("signed_block", &sample_block(4, 4));
}

#[test]
fn truncated_chain_layout_frames_reject() {
    assert_truncated_layout_candidates_reject("signed_transaction", &sample_transaction(8));
    assert_truncated_layout_candidates_reject(
        "transaction_entrypoint",
        &TransactionEntrypoint::from(sample_transaction(8)),
    );
    assert_truncated_layout_candidates_reject("signed_block", &sample_block(4, 4));
}
