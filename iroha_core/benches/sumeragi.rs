#![allow(missing_docs, clippy::restriction)]

use criterion::{criterion_group, criterion_main, Criterion};
use iroha_core::sumeragi::network_topology;
use iroha_crypto::{Hash, KeyPair};
use iroha_data_model::prelude::*;

const N_PEERS: usize = 255;

fn get_n_peers(n: usize) -> Vec<PeerId> {
    (0..n)
        .map(|i| PeerId {
            address: format!("127.0.0.{}", i),
            public_key: KeyPair::generate()
                .expect("Failed to generate KeyPair.")
                .public_key,
        })
        .collect()
}

fn sort_peers(criterion: &mut Criterion) {
    let peers = get_n_peers(N_PEERS);
    let _ = criterion.bench_function("sort_peers", |b| {
        b.iter(|| network_topology::sort_peers_by_hash(peers.clone(), Hash([0_u8; 32])));
    });
}

criterion_group!(benches, sort_peers);
criterion_main!(benches);
