//! Benchmarks for Norito encoding and decoding of real chain wire payloads.

use std::hint::black_box;

use criterion::Criterion;
use iroha_crypto::{PrivateKey, PublicKey};
use iroha_data_model::{
    ChainId, Level,
    account::AccountId,
    block::{BlockHeader, SignedBlock, decode_framed_signed_block},
    isi::{InstructionBox, Log},
    transaction::signed::{SignedTransaction, TransactionBuilder, TransactionEntrypoint},
};
use iroha_version::codec::{DecodeVersioned, EncodeVersioned};
use nonzero_ext::nonzero;
use norito::core::{DecodeFlagsGuard, header_flags};

#[derive(Clone, Copy)]
struct LayoutCandidate {
    name: &'static str,
    flags: u8,
}

const LAYOUT_CANDIDATES: &[LayoutCandidate] = &[
    LayoutCandidate {
        name: "canonical",
        flags: 0,
    },
    LayoutCandidate {
        name: "compact_len",
        flags: header_flags::COMPACT_LEN,
    },
    LayoutCandidate {
        name: "packed_struct",
        flags: header_flags::COMPACT_LEN | header_flags::PACKED_STRUCT | header_flags::FIELD_BITSET,
    },
    LayoutCandidate {
        name: "packed_all",
        flags: header_flags::COMPACT_LEN
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
    let chain: ChainId = "norito-chain-wire-bench".parse().expect("chain id");
    let instructions = (0..instruction_count)
        .map(|index| {
            InstructionBox::from(Log::new(
                Level::INFO,
                format!("bench instruction {index}").into(),
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

fn report_size(label: &str, bytes: &[u8]) {
    eprintln!("{label} bytes={}", bytes.len());
}

fn report_layout_size(
    label: &str,
    candidate: LayoutCandidate,
    bytes: &[u8],
    framed: &[u8],
    actual_flags: u8,
) {
    eprintln!(
        "{label}/{} flags=0x{actual_flags:02x} bare_bytes={} framed_bytes={}",
        candidate.name,
        bytes.len(),
        framed.len()
    );
}

fn encode_with_layout<T>(value: &T, candidate: LayoutCandidate) -> (Vec<u8>, u8)
where
    T: norito::NoritoSerialize,
{
    let _guard = DecodeFlagsGuard::enter(candidate.flags);
    norito::codec::encode_with_header_flags(value)
}

fn decode_framed_with_layout<T>(bytes: &[u8]) -> T
where
    T: for<'de> norito::NoritoDeserialize<'de>,
{
    norito::decode_from_bytes::<T>(bytes).expect("decode framed benchmark payload")
}

fn bench_layout_candidates<T>(c: &mut Criterion, label: &str, value: &T)
where
    T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>,
{
    for &candidate in LAYOUT_CANDIDATES {
        let (bytes, actual_flags) = encode_with_layout(value, candidate);
        let framed = norito::core::frame_bare_with_header_flags::<T>(&bytes, actual_flags)
            .expect("frame benchmark payload");
        report_layout_size(label, candidate, &bytes, &framed, actual_flags);

        let encode_name = format!("chain_wire/layout/{label}/{}/encode", candidate.name);
        c.bench_function(&encode_name, |b| {
            b.iter(|| black_box(encode_with_layout(black_box(value), candidate)))
        });

        let decode_name = format!("chain_wire/layout/{label}/{}/decode_framed", candidate.name);
        match norito::decode_from_bytes::<T>(&framed) {
            Ok(_) => {
                c.bench_function(&decode_name, |b| {
                    b.iter(|| black_box(decode_framed_with_layout::<T>(black_box(&framed))))
                });
            }
            Err(err) => {
                eprintln!(
                    "{label}/{} decode_framed=unsupported error={err:?}",
                    candidate.name
                );
            }
        }
    }
}

fn bench_signed_transaction(c: &mut Criterion) {
    let tx = sample_transaction(8);
    let bare = norito::codec::encode_adaptive(&tx);
    let versioned = tx.encode_versioned();
    report_size("signed_transaction/bare", &bare);
    report_size("signed_transaction/versioned", &versioned);

    c.bench_function("chain_wire/signed_transaction/bare_encode", |b| {
        b.iter(|| black_box(norito::codec::encode_adaptive(black_box(&tx))))
    });
    c.bench_function("chain_wire/signed_transaction/bare_decode_exact", |b| {
        b.iter(|| {
            black_box(
                norito::codec::decode_exact_from_slice::<SignedTransaction>(black_box(&bare))
                    .expect("decode signed transaction"),
            )
        })
    });
    c.bench_function("chain_wire/signed_transaction/versioned_decode", |b| {
        b.iter(|| {
            black_box(
                SignedTransaction::decode_all_versioned(black_box(&versioned))
                    .expect("decode versioned signed transaction"),
            )
        })
    });
    bench_layout_candidates(c, "signed_transaction", &tx);
}

fn bench_transaction_entrypoint(c: &mut Criterion) {
    let entrypoint = TransactionEntrypoint::from(sample_transaction(8));
    let bare = norito::codec::encode_adaptive(&entrypoint);
    let versioned = entrypoint.encode_versioned();
    report_size("transaction_entrypoint/bare", &bare);
    report_size("transaction_entrypoint/versioned", &versioned);

    c.bench_function("chain_wire/transaction_entrypoint/bare_encode", |b| {
        b.iter(|| black_box(norito::codec::encode_adaptive(black_box(&entrypoint))))
    });
    c.bench_function("chain_wire/transaction_entrypoint/bare_decode_exact", |b| {
        b.iter(|| {
            black_box(
                norito::codec::decode_exact_from_slice::<TransactionEntrypoint>(black_box(&bare))
                    .expect("decode transaction entrypoint"),
            )
        })
    });
    c.bench_function("chain_wire/transaction_entrypoint/versioned_decode", |b| {
        b.iter(|| {
            black_box(
                TransactionEntrypoint::decode_all_versioned(black_box(&versioned))
                    .expect("decode versioned transaction entrypoint"),
            )
        })
    });
    bench_layout_candidates(c, "transaction_entrypoint", &entrypoint);
}

fn bench_signed_block(c: &mut Criterion) {
    let block = sample_block(4, 4);
    let bare = norito::codec::encode_adaptive(&block);
    let versioned = block.encode_versioned();
    let wire = block.encode_wire().expect("encode signed block wire");
    report_size("signed_block/bare", &bare);
    report_size("signed_block/versioned", &versioned);
    report_size("signed_block/wire", &wire);

    c.bench_function("chain_wire/signed_block/bare_encode", |b| {
        b.iter(|| black_box(norito::codec::encode_adaptive(black_box(&block))))
    });
    c.bench_function("chain_wire/signed_block/bare_decode_exact", |b| {
        b.iter(|| {
            black_box(
                norito::codec::decode_exact_from_slice::<SignedBlock>(black_box(&bare))
                    .expect("decode signed block"),
            )
        })
    });
    c.bench_function("chain_wire/signed_block/versioned_decode", |b| {
        b.iter(|| {
            black_box(
                SignedBlock::decode_all_versioned(black_box(&versioned))
                    .expect("decode versioned signed block"),
            )
        })
    });
    c.bench_function("chain_wire/signed_block/wire_decode", |b| {
        b.iter(|| {
            black_box(
                decode_framed_signed_block(black_box(&wire)).expect("decode signed block wire"),
            )
        })
    });
    bench_layout_candidates(c, "signed_block", &block);
}

fn bench_empty_block_header(c: &mut Criterion) {
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let bare = norito::codec::encode_adaptive(&header);
    report_size("block_header/bare", &bare);

    c.bench_function("chain_wire/block_header/bare_encode", |b| {
        b.iter(|| black_box(norito::codec::encode_adaptive(black_box(&header))))
    });
    c.bench_function("chain_wire/block_header/bare_decode", |b| {
        b.iter(|| {
            black_box(
                norito::codec::decode_adaptive::<BlockHeader>(black_box(&bare))
                    .expect("decode block header"),
            )
        })
    });
}

fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_signed_transaction(&mut c);
    bench_transaction_entrypoint(&mut c);
    bench_signed_block(&mut c);
    bench_empty_block_header(&mut c);
    c.final_summary();
}
