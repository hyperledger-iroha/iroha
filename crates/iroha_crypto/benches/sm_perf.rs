//! Criterion benchmarks comparing SM2/SM3/SM4 primitives against baseline algorithms.

#![cfg(feature = "sm")]

use std::{hint::black_box, time::Duration};

use chacha20poly1305::{
    ChaCha20Poly1305, KeyInit, Nonce, Tag,
    aead::{AeadInOut, inout::InOutBuf},
};
use criterion::{BenchmarkId, Criterion, Throughput};
use iroha_crypto::{
    Algorithm, KeyPair, Signature,
    sm::{Sm2PrivateKey, Sm3Digest, Sm4Key, SmIntrinsicPolicy},
};
use sha2::{Digest, Sha256};

/// Compares deterministic SM2 signing/verification performance with Ed25519.
fn bench_sm2_vs_ed25519(c: &mut Criterion) {
    let mut sign_group = c.benchmark_group("sm2_vs_ed25519_sign");
    sign_group.sample_size(60);
    sign_group.measurement_time(Duration::from_secs(6));

    let message = b"iroha sm2 benchmark payload";
    let sm2_private = Sm2PrivateKey::from_seed(
        iroha_crypto::sm::Sm2PublicKey::default_distid(),
        b"iroha-sm2-bench-seed",
    )
    .expect("SM2 seed must produce a valid key");
    let sm2_public = sm2_private.public_key();

    sign_group.bench_function("sm2_sign", |b| {
        b.iter(|| {
            black_box(sm2_private.sign(black_box(message)));
        });
    });

    let ed_pair = KeyPair::from_seed(b"iroha-ed25519-bench-seed".to_vec(), Algorithm::Ed25519);
    let ed_private = ed_pair.private_key().clone();
    sign_group.bench_function("ed25519_sign", |b| {
        b.iter(|| {
            black_box(Signature::new(&ed_private, black_box(message)));
        });
    });
    sign_group.finish();

    let mut verify_group = c.benchmark_group("sm2_vs_ed25519_verify");
    verify_group.sample_size(60);
    verify_group.measurement_time(Duration::from_secs(6));

    let sm2_signature = sm2_private.sign(message);
    verify_group.bench_function(BenchmarkId::new("verify", "sm2"), |b| {
        b.iter(|| {
            sm2_public
                .verify(black_box(message), black_box(&sm2_signature))
                .expect("SM2 signature should verify");
        });
    });

    let ed_public = ed_pair.public_key().clone();
    let ed_signature = Signature::new(&ed_private, message);
    verify_group.bench_function(BenchmarkId::new("verify", "ed25519"), |b| {
        b.iter(|| {
            ed_signature
                .verify(&ed_public, black_box(message))
                .expect("Ed25519 signature should verify");
        });
    });
    verify_group.finish();
}

/// Measures SM3 hashing throughput against SHA-256 on typical payloads.
fn bench_sm3_vs_sha256(c: &mut Criterion) {
    let mut group = c.benchmark_group("sm3_vs_sha256_hash");
    group.sample_size(80);
    group.measurement_time(Duration::from_secs(6));

    let payload = vec![0xA5; 4096];
    group.throughput(Throughput::Bytes(payload.len() as u64));

    group.bench_function("sm3_hash", |b| {
        b.iter(|| {
            black_box(Sm3Digest::hash(black_box(&payload)));
        });
    });

    group.bench_function("sha256_hash", |b| {
        b.iter(|| {
            let mut hasher = Sha256::new();
            hasher.update(black_box(&payload));
            black_box(hasher.finalize());
        });
    });

    group.finish();
}

/// Evaluates SM4-GCM seal/open speed relative to ChaCha20-Poly1305.
fn bench_sm4_vs_chacha20poly1305(c: &mut Criterion) {
    let mut encrypt_group = c.benchmark_group("sm4_vs_chacha20poly1305_encrypt");
    encrypt_group.sample_size(80);
    encrypt_group.measurement_time(Duration::from_secs(6));

    let sm4_key = Sm4Key::new([0x11; 16]);
    let sm4_nonce = [0x22; 12];
    let aad = [0x33; 16];
    let plaintext = vec![0x44; 1024];
    encrypt_group.throughput(Throughput::Bytes(plaintext.len() as u64));

    encrypt_group.bench_function("sm4_gcm_encrypt", |b| {
        b.iter(|| {
            let (cipher, tag) = sm4_key
                .encrypt_gcm(
                    black_box(&sm4_nonce),
                    black_box(&aad),
                    black_box(&plaintext),
                )
                .expect("SM4-GCM encryption should succeed");
            black_box((cipher, tag));
        });
    });

    let chacha = ChaCha20Poly1305::new_from_slice(&[0x55; 32])
        .expect("ChaCha20-Poly1305 key should be valid");
    let chacha_nonce = Nonce::from([0x66; 12]);

    encrypt_group.bench_function("chacha20poly1305_encrypt", |b| {
        b.iter(|| {
            let mut buffer = black_box(plaintext.clone());
            let mut inout = InOutBuf::from(buffer.as_mut_slice());
            let tag = chacha
                .encrypt_inout_detached(&chacha_nonce, black_box(&aad), inout.reborrow())
                .expect("ChaCha20-Poly1305 encryption should succeed");
            black_box((buffer, tag));
        });
    });
    encrypt_group.finish();

    let (sm4_cipher, sm4_tag) = sm4_key
        .encrypt_gcm(&sm4_nonce, &aad, &plaintext)
        .expect("SM4-GCM encryption should succeed");
    let mut chacha_cipher = plaintext.clone();
    let mut chacha_buf = InOutBuf::from(chacha_cipher.as_mut_slice());
    let chacha_tag: Tag = chacha
        .encrypt_inout_detached(&chacha_nonce, &aad, chacha_buf.reborrow())
        .expect("ChaCha20-Poly1305 encryption should succeed");

    let mut decrypt_group = c.benchmark_group("sm4_vs_chacha20poly1305_decrypt");
    decrypt_group.sample_size(80);
    decrypt_group.measurement_time(Duration::from_secs(6));
    decrypt_group.throughput(Throughput::Bytes(plaintext.len() as u64));

    decrypt_group.bench_function("sm4_gcm_decrypt", |b| {
        b.iter(|| {
            let plain = sm4_key
                .decrypt_gcm(
                    black_box(&sm4_nonce),
                    black_box(&aad),
                    black_box(&sm4_cipher),
                    black_box(&sm4_tag),
                )
                .expect("SM4-GCM decryption should succeed");
            black_box(plain);
        });
    });

    decrypt_group.bench_function("chacha20poly1305_decrypt", |b| {
        b.iter(|| {
            let mut buffer = chacha_cipher.clone();
            let mut inout = InOutBuf::from(buffer.as_mut_slice());
            chacha
                .decrypt_inout_detached(
                    &chacha_nonce,
                    black_box(&aad),
                    inout.reborrow(),
                    &chacha_tag,
                )
                .expect("ChaCha20-Poly1305 decryption should succeed");
            black_box(buffer);
        });
    });
    decrypt_group.finish();
}

fn apply_intrinsic_policy_from_env() {
    let raw_policy = match std::env::var("CRYPTO_SM_INTRINSICS") {
        Ok(value) => value,
        Err(_) => return,
    };

    let policy = match raw_policy.trim().to_ascii_lowercase().as_str() {
        "force-enable" | "force_enable" | "enable" | "on" | "true" | "1" => {
            SmIntrinsicPolicy::ForceEnable
        }
        "force-disable" | "force_disable" | "disable" | "off" | "false" | "0" => {
            SmIntrinsicPolicy::ForceDisable
        }
        _ => SmIntrinsicPolicy::Auto,
    };

    iroha_crypto::sm::set_intrinsic_policy(policy);
    eprintln!("sm_perf: applied SM intrinsic policy from CRYPTO_SM_INTRINSICS={raw_policy}");
}

/// Runs the SM performance benchmarks with Criterion's default harness.
fn main() {
    apply_intrinsic_policy_from_env();
    let mut criterion = Criterion::default().configure_from_args();
    bench_sm2_vs_ed25519(&mut criterion);
    bench_sm3_vs_sha256(&mut criterion);
    bench_sm4_vs_chacha20poly1305(&mut criterion);
    criterion.final_summary();
}
