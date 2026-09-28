//! Benchmarks for mock world state view (WSV) host operations.
use criterion::Criterion;
use iroha_crypto::KeyPair;
use iroha_model_base::state_path::StatePath;
use ivm::{DurableStateSnapshot, MockWorldStateView, WsvHost, host::IVMHost, mock_wsv::AccountId};
use std::{
    collections::BTreeMap,
    fs,
    hint::black_box,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
fn mock_wsv_host_with_persisted_state(entries: usize, value_bytes: usize) -> (WsvHost, PathBuf) {
    let tmp_dir = std::env::temp_dir().join(format!(
        "ivm_bench_wsv_checkpoint_{}_{}",
        entries,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after UNIX epoch")
            .as_nanos()
    ));
    fs::create_dir_all(&tmp_dir).expect("create benchmark state directory");
    let persist_path = tmp_dir.join("state.json");
    let mut wsv = MockWorldStateView::with_state_store(persist_path)
        .expect("create persisted mock WSV state store");
    let mut state = BTreeMap::new();
    for idx in 0..entries {
        let path: StatePath = format!("bench/{idx:06}")
            .parse()
            .expect("benchmark state path should be valid");
        state.insert(path, vec![idx as u8; value_bytes]);
    }
    wsv.sc_restore(&DurableStateSnapshot::new(state))
        .expect("seed persisted mock WSV state");
    let caller = AccountId::new(KeyPair::random().public_key().clone());
    (WsvHost::new_with_subject(wsv, caller), tmp_dir)
}
fn bench_mock_wsv_checkpoint_restore(c: &mut Criterion) {
    let mut group = c.benchmark_group("mock_wsv_checkpoint_restore");
    for entries in [128_usize, 2_048] {
        group.bench_function(format!("persisted_entries_{entries}"), |b| {
            let (mut host, tmp_dir) = mock_wsv_host_with_persisted_state(entries, 128);
            b.iter(|| {
                let snapshot = IVMHost::checkpoint(&host).expect("checkpoint mock WSV host");
                IVMHost::restore(&mut host, snapshot.as_ref()).expect("restore mock WSV host");
                black_box(&host);
            });
            let _ = fs::remove_dir_all(tmp_dir);
        });
    }
    group.finish();
}
/// Entry point for the benchmark binary.
fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_mock_wsv_checkpoint_restore(&mut c);
    c.final_summary();
}
