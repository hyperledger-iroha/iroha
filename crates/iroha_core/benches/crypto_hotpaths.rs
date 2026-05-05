//! Benchmarks for hash and Poseidon hot paths used by block admission.

use std::hint::black_box;

use criterion::Criterion;
use iroha_core::fastpq;
use iroha_crypto::Hash;
use iroha_data_model::{
    asset::AssetDefinitionId,
    domain::DomainId,
    fastpq::{FastpqPublicInputs, TransferDeltaTranscript, TransferSmtWitness, TransferTranscript},
};
use iroha_primitives::numeric::Numeric;
use iroha_test_samples::{ALICE_ID, BOB_ID};
use iroha_zkp_halo2::poseidon::{self, PoseidonByteHasher};

fn sample_transfer_delta() -> TransferDeltaTranscript {
    TransferDeltaTranscript {
        from_account: (*ALICE_ID).clone(),
        to_account: (*BOB_ID).clone(),
        asset_definition: AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("valid domain"),
            "rose".parse().expect("valid asset name"),
        ),
        amount: Numeric::from(42u32),
        from_balance_before: Numeric::from(200u32),
        from_balance_after: Numeric::from(158u32),
        to_balance_before: Numeric::from(1u32),
        to_balance_after: Numeric::from(43u32),
        from_smt_witness: TransferSmtWitness::default(),
        to_smt_witness: TransferSmtWitness::default(),
    }
}

fn sample_public_inputs() -> FastpqPublicInputs {
    fastpq::FastpqPublicInputsTemplate {
        dsid: [0x01; 16],
        slot: 42,
        old_root: [0x02; 32],
        new_root: [0x03; 32],
        perm_root: [0x04; 32],
    }
    .with_tx_set_hash([0x05; 32])
}

fn sample_transfer_transcripts(count: usize, precompute_digest: bool) -> Vec<TransferTranscript> {
    let delta = sample_transfer_delta();
    let authority_digest = fastpq::authority_digest(&ALICE_ID);
    (0..count)
        .map(|idx| {
            let batch_hash = Hash::prehashed(
                [u8::try_from(idx).expect("crypto hotpath bench sample count fits in u8"); 32],
            );
            let poseidon_preimage_digest =
                precompute_digest.then(|| fastpq::poseidon_preimage_digest(&delta, &batch_hash));
            TransferTranscript {
                batch_hash,
                deltas: vec![delta.clone()],
                authority_digest,
                poseidon_preimage_digest,
            }
        })
        .collect()
}

fn bench_poseidon_hash_bytes(c: &mut Criterion) {
    for &len in &[32usize, 33, 128, 129, 512, 4096] {
        let bytes = (0..len)
            .map(|idx| u8::try_from(idx & 0xff).expect("byte fits"))
            .collect::<Vec<_>>();
        c.bench_function(&format!("crypto_hotpaths/poseidon/hash_bytes/{len}"), |b| {
            b.iter(|| black_box(poseidon::hash_bytes(black_box(&bytes))))
        });
        c.bench_function(
            &format!("crypto_hotpaths/poseidon/byte_hasher_streaming/{len}"),
            |b| {
                b.iter(|| {
                    let mut hasher = PoseidonByteHasher::new();
                    for chunk in bytes.chunks(17) {
                        hasher.update(black_box(chunk));
                    }
                    black_box(hasher.finalize())
                })
            },
        );
    }
}

fn bench_poseidon_fixed_width(c: &mut Criterion) {
    c.bench_function("crypto_hotpaths/poseidon/hash2_u64", |b| {
        b.iter(|| black_box(poseidon::hash2_u64(black_box(42), black_box(99))))
    });
    c.bench_function("crypto_hotpaths/poseidon/hash6_u64", |b| {
        b.iter(|| black_box(poseidon::hash6_u64(black_box([1_u64, 2, 3, 4, 5, 6]))))
    });
    for &len in &[2usize, 24, 64] {
        let words = (0..len)
            .map(|idx| (idx as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15))
            .collect::<Vec<_>>();
        c.bench_function(
            &format!("crypto_hotpaths/poseidon/hash_u64_words_bytes/{len}"),
            |b| b.iter(|| black_box(poseidon::hash_u64_words_bytes(black_box(&words)))),
        );
    }
}

fn bench_fastpq_poseidon_preimage_digest(c: &mut Criterion) {
    let delta = sample_transfer_delta();
    let batch_hash = Hash::prehashed([0x11; 32]);

    c.bench_function("crypto_hotpaths/fastpq/poseidon_preimage_digest", |b| {
        b.iter(|| fastpq::poseidon_preimage_digest(black_box(&delta), black_box(&batch_hash)))
    });
}

fn bench_fastpq_batch_from_transcripts(c: &mut Criterion) {
    let public_inputs = sample_public_inputs();
    for (label, precompute_digest) in [("missing_digests", false), ("precomputed_digests", true)] {
        let transcripts = sample_transfer_transcripts(64, precompute_digest);
        c.bench_function(
            &format!("crypto_hotpaths/fastpq/batch_from_transcripts/{label}/64"),
            |b| {
                b.iter(|| {
                    let batch = fastpq::batch_from_transcripts(
                        fastpq::FASTPQ_CANONICAL_PARAMETER_SET,
                        black_box(public_inputs),
                        black_box(transcripts.iter()),
                    )
                    .expect("valid transcript batch");
                    black_box(batch)
                })
            },
        );
    }
}

fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_poseidon_hash_bytes(&mut c);
    bench_poseidon_fixed_width(&mut c);
    bench_fastpq_poseidon_preimage_digest(&mut c);
    bench_fastpq_batch_from_transcripts(&mut c);
    c.final_summary();
}
