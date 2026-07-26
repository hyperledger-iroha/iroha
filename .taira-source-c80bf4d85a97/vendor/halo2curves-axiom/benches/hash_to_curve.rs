use std::iter;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use halo2curves_axiom as halo2curves;
use pasta_curves::arithmetic::CurveExt;
use rand_core::{OsRng, RngCore};

fn hash_to_secp256k1(c: &mut Criterion) {
    hash_to_curve::<halo2curves::secp256k1::Secp256k1>(c, "Secp256k1");
}

fn hash_to_secq256k1(c: &mut Criterion) {
    hash_to_curve::<halo2curves::secq256k1::Secq256k1>(c, "Secq256k1");
}

fn hash_to_secp256r1(c: &mut Criterion) {
    hash_to_curve::<halo2curves::secp256r1::Secp256r1>(c, "Secp256r1");
}

fn hash_to_pallas(c: &mut Criterion) {
    hash_to_curve::<halo2curves::pasta::Ep>(c, "Pallas");
}

fn hash_to_vesta(c: &mut Criterion) {
    hash_to_curve::<halo2curves::pasta::Eq>(c, "Vesta");
}

fn hash_to_bn256(c: &mut Criterion) {
    hash_to_curve::<halo2curves::bn256::G1>(c, "Bn256");
}

fn hash_to_grumpkin(c: &mut Criterion) {
    hash_to_curve::<halo2curves::grumpkin::G1>(c, "Grumpkin");
}

fn hash_to_curve<G: CurveExt>(c: &mut Criterion, name: &'static str) {
    {
        let hasher = G::hash_to_curve("test");
        let mut rng = OsRng;
        let message = iter::repeat_with(|| rng.next_u32().to_be_bytes())
            .take(1024)
            .flatten()
            .collect::<Vec<_>>();

        c.bench_function(&format!("Hash to {name}"), move |b| {
            b.iter(|| hasher(black_box(&message)))
        });
    }
}

criterion_group!(
    benches,
    hash_to_secp256k1,
    hash_to_secq256k1,
    hash_to_secp256r1,
    hash_to_pallas,
    hash_to_vesta,
    hash_to_bn256,
    hash_to_grumpkin,
);
criterion_main!(benches);
