//! Benchmarks for Ed25519 parse and verification hot paths.

use std::hint::black_box;

use criterion::Criterion;
use iroha_crypto::{
    Algorithm, Ed25519BatchScratch, KeyPair, PrivateKey, Signature, ed25519_parse_public_key,
    ed25519_verify_batch_preparsed_deterministic_with_scratch,
};

fn seeded_keypair(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .expect("bench Ed25519 seeded keypair should be valid")
}

fn checked_signature(private_key: &PrivateKey, message: &[u8]) -> Signature {
    Signature::try_new(private_key, message).expect("bench Ed25519 signature should succeed")
}

fn checked_ed25519_public_key_payload(keypair: &KeyPair) -> &[u8] {
    let (algorithm, payload) = keypair
        .public_key()
        .try_to_bytes()
        .expect("bench Ed25519 public key must be well-formed");
    assert_eq!(algorithm, Algorithm::Ed25519);
    payload
}

fn bench_public_key_parse(c: &mut Criterion) {
    let warm = seeded_keypair(7);
    let warm_payload = checked_ed25519_public_key_payload(&warm);
    ed25519_parse_public_key(warm_payload).expect("warm public key parse");

    c.bench_function("ed25519/public_key_parse/warm_same_key", |b| {
        b.iter(|| black_box(ed25519_parse_public_key(black_box(warm_payload)).unwrap()))
    });

    let keys = (0..=u8::MAX)
        .map(seeded_keypair)
        .map(|keypair| checked_ed25519_public_key_payload(&keypair).to_vec())
        .collect::<Vec<_>>();
    c.bench_function("ed25519/public_key_parse/many_keys", |b| {
        b.iter(|| {
            for payload in &keys {
                black_box(ed25519_parse_public_key(black_box(payload)).unwrap());
            }
        })
    });
}

fn bench_single_verify(c: &mut Criterion) {
    let keypair = seeded_keypair(11);
    let message = b"iroha-ed25519-hotpath-message";
    let signature = checked_signature(keypair.private_key(), message);

    c.bench_function("ed25519/verify/single", |b| {
        b.iter(|| {
            signature
                .verify(black_box(keypair.public_key()), black_box(message))
                .unwrap();
            black_box(())
        })
    });

    let cache_message = [0x42_u8; 32];
    let cache_signature = checked_signature(keypair.private_key(), &cache_message);
    cache_signature
        .verify(keypair.public_key(), &cache_message)
        .expect("prime exact verify cache");
    c.bench_function("ed25519/verify/exact_32_byte_cache_hit", |b| {
        b.iter(|| {
            cache_signature
                .verify(black_box(keypair.public_key()), black_box(&cache_message))
                .unwrap();
            black_box(())
        })
    });
}

fn bench_batch_verify(c: &mut Criterion) {
    for &count in &[16usize, 64, 256] {
        let keypairs = (0..count)
            .map(|idx| seeded_keypair(u8::try_from(idx % 251).expect("seed fits u8")))
            .collect::<Vec<_>>();
        let messages = (0..count)
            .map(|idx| format!("iroha-ed25519-batch-message-{idx}").into_bytes())
            .collect::<Vec<_>>();
        let signatures = keypairs
            .iter()
            .zip(messages.iter())
            .map(|(keypair, message)| {
                checked_signature(keypair.private_key(), message)
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        let parsed_public_keys = keypairs
            .iter()
            .map(|keypair| {
                let payload = checked_ed25519_public_key_payload(keypair);
                ed25519_parse_public_key(payload).expect("parse batch public key")
            })
            .collect::<Vec<_>>();
        let message_refs = messages.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let signature_refs = signatures.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let mut scratch = Ed25519BatchScratch::default();

        c.bench_function(&format!("ed25519/verify_batch_preparsed/{count}"), |b| {
            b.iter(|| {
                ed25519_verify_batch_preparsed_deterministic_with_scratch(
                    black_box(&message_refs),
                    black_box(&signature_refs),
                    black_box(&parsed_public_keys),
                    [0u8; 32],
                    black_box(&mut scratch),
                )
                .unwrap();
                black_box(())
            })
        });
    }
}

fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_public_key_parse(&mut c);
    bench_single_verify(&mut c);
    bench_batch_verify(&mut c);
    c.final_summary();
}
