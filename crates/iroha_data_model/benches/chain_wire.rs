//! Benchmarks for Norito encoding and decoding of real chain wire payloads.

use std::hint::black_box;

use criterion::Criterion;
use iroha_crypto::KeyPair;
use iroha_data_model::{
    ChainId, Level,
    account::AccountId,
    block::{BlockHeader, SignedBlock, decode_framed_signed_block},
    isi::{InstructionBox, Log},
    transaction::signed::{SignedTransaction, TransactionBuilder, TransactionEntrypoint},
};
use iroha_version::codec::{DecodeVersioned, EncodeVersioned};
use nonzero_ext::nonzero;

fn sample_transaction(instruction_count: usize) -> SignedTransaction {
    let key_pair = KeyPair::random();
    let authority = AccountId::new(key_pair.public_key().clone());
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
        .sign(key_pair.private_key())
}

fn sample_block(transaction_count: usize, instruction_count: usize) -> SignedBlock {
    let key_pair = KeyPair::random();
    let transactions = (0..transaction_count)
        .map(|_| sample_transaction(instruction_count))
        .collect::<Vec<_>>();

    SignedBlock::genesis(transactions, key_pair.private_key(), None, None)
}

fn report_size(label: &str, bytes: &[u8]) {
    eprintln!("{label} bytes={}", bytes.len());
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
