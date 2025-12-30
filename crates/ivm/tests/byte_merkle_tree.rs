use ivm::{
    AccelerationConfig, ByteMerkleTree, acceleration_runtime_status, set_acceleration_config,
};

struct AccelConfigGuard {
    original: AccelerationConfig,
}

impl AccelConfigGuard {
    fn new() -> Self {
        Self {
            original: ivm::acceleration_config(),
        }
    }
}

impl Drop for AccelConfigGuard {
    fn drop(&mut self) {
        set_acceleration_config(self.original);
    }
}

#[test]
fn from_bytes_matches_updates() {
    // Create tree directly from bytes
    let data = vec![1u8; 64];
    let tree_from = ByteMerkleTree::from_bytes(&data, 32);

    // Build equivalent tree using new() and update_leaf()
    let tree_update = ByteMerkleTree::new(2, 32);
    tree_update.update_leaf(0, &data[..32]);
    tree_update.update_leaf(1, &data[32..]);

    assert_eq!(tree_from.root(), tree_update.root());
}

#[test]
fn zero_update_keeps_root() {
    let tree = ByteMerkleTree::new(1, 32);
    let initial = tree.root();
    tree.update_leaf(0, &[0u8; 32]);
    assert_eq!(tree.root(), initial);
}

#[test]
fn parallel_matches_sequential() {
    let data = vec![3u8; 96];
    let seq = ByteMerkleTree::from_bytes(&data, 32).root();
    let par = ByteMerkleTree::from_bytes_parallel(&data, 32).root();
    assert_eq!(seq, par);
}

#[test]
fn parallel_updates_thread_safe() {
    use std::sync::Arc;

    use rayon::prelude::*;

    let tree = Arc::new(ByteMerkleTree::new(4, 32));
    (0..4usize).into_par_iter().for_each(|i| {
        let chunk = [i as u8; 32];
        tree.update_leaf(i, &chunk);
    });

    let seq = ByteMerkleTree::new(4, 32);
    for i in 0..4usize {
        seq.update_leaf(i, &[i as u8; 32]);
    }

    assert_eq!(tree.root(), seq.root());
}

#[test]
fn batch_parallel_update_matches_canonical() {
    // Build baseline data and compute canonical root via from_bytes
    let mut data = vec![0u8; 32 * 8];
    for (i, b) in data.iter_mut().enumerate() {
        *b = (i as u8).wrapping_mul(31).wrapping_add(7);
    }
    let canonical = ByteMerkleTree::from_bytes(&data, 32).root();

    // Create a tree of matching size and update a batch of leaves in parallel
    let tree = ByteMerkleTree::new(8, 32);
    let indices: Vec<usize> = (0..8).collect();
    tree.update_leaves_from_bytes_parallel(&data, &indices);

    assert_eq!(canonical, tree.root());
}

#[test]
fn root_and_path_combined_matches_separate() {
    // Build a tree from bytes
    let mut data = vec![0u8; 32 * 6 + 7];
    for (i, b) in data.iter_mut().enumerate() {
        *b = (i as u8).wrapping_mul(13).wrapping_add(2);
    }
    let tree = ByteMerkleTree::from_bytes(&data, 32);

    for &idx in &[0usize, 1, 3, 5] {
        let (root_c, path_c) = tree.root_and_path(idx);
        let root_s = tree.root();
        let path_s = tree.path(idx);
        assert_eq!(root_c.as_ref(), &root_s, "root mismatch at idx={idx}");
        assert_eq!(path_c, path_s, "path mismatch at idx={idx}");
    }
}

#[test]
fn merkle_roots_match_across_acceleration_configs() {
    // Use enough leaves to trigger the GPU thresholds when available.
    let leaves = 8_200;
    let mut data = vec![0u8; 32 * leaves];
    for (idx, byte) in data.iter_mut().enumerate() {
        let v = ((idx as u8).wrapping_mul(13)).wrapping_add(7);
        *byte = v;
    }

    let guard = AccelConfigGuard::new();

    // Force CPU-only path.
    set_acceleration_config(AccelerationConfig {
        enable_cuda: false,
        enable_metal: false,
        merkle_min_leaves_gpu: Some(usize::MAX),
        merkle_min_leaves_metal: Some(usize::MAX),
        merkle_min_leaves_cuda: Some(usize::MAX),
        ..guard.original
    });
    let cpu_root = ByteMerkleTree::from_bytes_parallel(&data, 32).root();
    let cpu_status = acceleration_runtime_status();
    assert!(!cpu_status.metal.configured, "metal should be disabled");
    assert!(!cpu_status.cuda.configured, "cuda should be disabled");

    // Allow acceleration again (respecting the caller's original toggles) and
    // drop thresholds so hardware offload is permitted when present.
    set_acceleration_config(AccelerationConfig {
        merkle_min_leaves_gpu: Some(0),
        merkle_min_leaves_metal: Some(0),
        merkle_min_leaves_cuda: Some(0),
        ..guard.original
    });
    let accel_root = ByteMerkleTree::from_bytes_parallel(&data, 32).root();
    let accel_status = acceleration_runtime_status();
    assert_eq!(
        accel_status.metal.configured, guard.original.enable_metal,
        "metal configured flag should reflect restored policy"
    );
    assert_eq!(
        accel_status.cuda.configured, guard.original.enable_cuda,
        "cuda configured flag should reflect restored policy"
    );

    assert_eq!(accel_root, cpu_root, "roots must be deterministic");
}
