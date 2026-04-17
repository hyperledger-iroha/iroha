//! Regression coverage for Norito chain wire layout candidates.

use iroha_crypto::{PrivateKey, PublicKey};
use iroha_data_model::{
    ChainId, Level,
    account::AccountId,
    block::SignedBlock,
    isi::{InstructionBox, Log},
    transaction::signed::{SignedTransaction, TransactionBuilder, TransactionEntrypoint},
};
use norito::core::{DecodeFlagsGuard, frame_bare_with_header_flags, header_flags};

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

fn framed_with_layout<T>(value: &T, candidate: LayoutCandidate) -> Vec<u8>
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
    frame_bare_with_header_flags::<T>(&payload, flags).expect("frame layout payload")
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
