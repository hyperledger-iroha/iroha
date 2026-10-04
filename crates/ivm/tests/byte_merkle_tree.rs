//! Canonical byte-Merkle roots, public leaf updates and acceleration parity.

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
    let tree_from = ByteMerkleTree::from_bytes(&data, 32).unwrap();
    // Build equivalent tree using new() and update_leaf()
    let tree_update = ByteMerkleTree::new(2, 32).unwrap();
    tree_update.update_leaf(0, &data[..32]).unwrap();
    tree_update.update_leaf(1, &data[32..]).unwrap();
    assert_eq!(tree_from.root(), tree_update.root());
}
#[test]
fn zero_update_keeps_root() {
    let tree = ByteMerkleTree::new(1, 32).unwrap();
    let initial = tree.root();
    tree.update_leaf(0, &[0u8; 32]).unwrap();
    assert_eq!(tree.root(), initial);
}
#[test]
fn parallel_matches_sequential() {
    let data = vec![3u8; 96];
    let seq = ByteMerkleTree::from_bytes(&data, 32).unwrap().root();
    let par = ByteMerkleTree::from_bytes_parallel(&data, 32)
        .unwrap()
        .root();
    assert_eq!(seq, par);
}
#[test]
fn parallel_updates_thread_safe() {
    use rayon::prelude::*;
    use std::sync::Arc;
    let tree = Arc::new(ByteMerkleTree::new(4, 32).unwrap());
    (0..4usize).into_par_iter().for_each(|i| {
        let chunk = [i as u8; 32];
        tree.update_leaf(i, &chunk).unwrap();
    });
    let seq = ByteMerkleTree::new(4, 32).unwrap();
    for i in 0..4usize {
        seq.update_leaf(i, &[i as u8; 32]).unwrap();
    }
    assert_eq!(tree.root(), seq.root());
}
#[test]
fn complete_public_leaf_updates_match_canonical() {
    // Build baseline data and compute canonical root via from_bytes
    let mut data = vec![0u8; 32 * 8];
    for (i, b) in data.iter_mut().enumerate() {
        *b = (i as u8).wrapping_mul(31).wrapping_add(7);
    }
    let canonical = ByteMerkleTree::from_bytes(&data, 32).unwrap().root();
    // Public leaf updates cover the complete image; the private memory bitmap
    // batch path has independent sparse/dense, duplicate and cache controls.
    let tree = ByteMerkleTree::new(8, 32).unwrap();
    for (index, leaf) in data.chunks_exact(32).enumerate() {
        tree.update_leaf(index, leaf).unwrap();
    }
    assert_eq!(canonical, tree.root());
}
#[test]
fn root_and_path_combined_matches_separate() {
    // Build a tree from bytes
    let mut data = vec![0u8; 32 * 6 + 7];
    for (i, b) in data.iter_mut().enumerate() {
        *b = (i as u8).wrapping_mul(13).wrapping_add(2);
    }
    let tree = ByteMerkleTree::from_bytes(&data, 32).unwrap();
    for &idx in &[0usize, 1, 3, 5] {
        let (root_c, path_c) = tree.root_and_path(idx).unwrap();
        let root_s = tree.root();
        let path_s = tree.path(idx).unwrap();
        assert_eq!(root_c.as_ref(), &root_s, "root mismatch at idx={idx}");
        assert_eq!(path_c, path_s, "path mismatch at idx={idx}");
    }
}

#[test]
fn leaf_index_apis_reject_out_of_bounds_and_narrowing_aliases() {
    let tree = ByteMerkleTree::new(2, 32).unwrap();
    let baseline = tree.root();

    for invalid in [2, usize::MAX] {
        assert!(tree.proof(invalid).is_err());
        assert!(tree.path(invalid).is_err());
        assert!(tree.root_and_proof(invalid).is_err());
        assert!(tree.root_and_path(invalid).is_err());
        assert!(tree.update_leaf(invalid, &[0xA5; 32]).is_err());
    }

    if let Ok(narrowing_alias) = usize::try_from(u64::from(u32::MAX) + 1) {
        assert!(tree.proof(narrowing_alias).is_err());
        assert!(tree.path(narrowing_alias).is_err());
        assert!(tree.root_and_proof(narrowing_alias).is_err());
        assert!(tree.root_and_path(narrowing_alias).is_err());
        assert!(tree.update_leaf(narrowing_alias, &[0x5A; 32]).is_err());
    }

    assert_eq!(tree.root(), baseline, "failed updates must be atomic");
}

#[test]
fn oversized_leaf_update_is_rejected_without_truncation_or_mutation() {
    let tree = ByteMerkleTree::new(1, 4).unwrap();
    tree.update_leaf(0, &[1, 2, 3, 4]).unwrap();
    let baseline = tree.root();

    assert!(tree.update_leaf(0, &[1, 2, 3, 4, 5]).is_err());
    assert_eq!(tree.root(), baseline, "failed update must be atomic");
}

#[test]
fn public_constructors_reject_invalid_chunk_sizes() {
    for invalid in [0, 33] {
        assert!(ByteMerkleTree::new(1, invalid).is_err());
        assert!(ByteMerkleTree::from_bytes(b"tree", invalid).is_err());
        assert!(ByteMerkleTree::from_bytes_parallel(b"tree", invalid).is_err());
        assert!(ByteMerkleTree::from_bytes_accel(b"tree", invalid).is_err());
        assert!(ByteMerkleTree::root_from_bytes_accel(b"tree", invalid).is_err());
    }

    assert!(ByteMerkleTree::new(1, 1).is_ok());
    assert!(ByteMerkleTree::new(1, 32).is_ok());
}
#[test]
fn merkle_roots_match_across_acceleration_configs() {
    const CHILD: &str = "IVM_BYTE_MERKLE_POLICY_PARITY_CHILD";
    if std::env::var(CHILD).as_deref() != Ok("1") {
        // Configuration is process-wide; isolate this test from the other grouped consumers.
        let module = module_path!().split_once("::").unwrap().1;
        let name = format!("{module}::merkle_roots_match_across_acceleration_configs");
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &name, "--nocapture"])
            .env(CHILD, "1")
            .output()
            .expect("isolated byte-Merkle consumer test");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        print!("{stdout}{stderr}");
        assert!(
            output.status.success(),
            "byte-Merkle consumer parity failed"
        );
        assert!(stdout.contains("1 passed; 0 failed; 0 ignored"));
        return;
    }

    let guard = AccelConfigGuard::new();
    // Ragged trees and partial last chunks exercise padding at both chunk widths.
    let leaves = 8_193;
    for (chunk, tail) in [(17, 7), (32, 31)] {
        let mut data = vec![0u8; chunk * (leaves - 1) + tail];
        for (idx, byte) in data.iter_mut().enumerate() {
            *byte = (idx as u8).wrapping_mul(13).wrapping_add(7);
        }
        let canonical = iroha_crypto::MerkleTree::<[u8; 32]>::from_byte_chunks(&data, chunk)
            .expect("valid canonical chunk geometry");
        let canonical_root = canonical.root().unwrap();
        let commitment = canonical.commitment().unwrap();
        let check_accelerated_apis = || {
            let tree = ByteMerkleTree::from_bytes_accel(&data, chunk).unwrap();
            let root = ByteMerkleTree::root_from_bytes_accel(&data, chunk).unwrap();
            assert_eq!(&tree.root(), canonical_root.as_ref());
            assert_eq!(&root, canonical_root.as_ref());
            for index in [0, leaves / 2, leaves - 1] {
                let leaf = canonical.leaves().nth(index).unwrap();
                let canonical_proof = canonical.get_proof(index as u32).unwrap();
                let proof = tree.proof(index).unwrap();
                assert_eq!(proof, canonical_proof, "proof differs at leaf {index}");
                assert!(proof.verify_sha256(&leaf, &commitment));
                let canonical_path: Vec<[u8; 32]> = canonical_proof
                    .audit_path()
                    .iter()
                    .map(|sibling| sibling.map(|hash| *hash.as_ref()).unwrap_or([0; 32]))
                    .collect();
                assert_eq!(tree.path(index).unwrap(), canonical_path);
            }
            root
        };

        // Force both accelerated APIs through their ordinary CPU fallback.
        set_acceleration_config(AccelerationConfig {
            enable_cuda: false,
            enable_metal: false,
            merkle_min_leaves_gpu: Some(usize::MAX),
            merkle_min_leaves_metal: Some(usize::MAX),
            merkle_min_leaves_cuda: Some(usize::MAX),
            ..guard.original
        });
        let cpu_root = ByteMerkleTree::from_bytes_parallel(&data, chunk)
            .unwrap()
            .root();
        assert_eq!(check_accelerated_apis(), cpu_root);
        let cpu_status = acceleration_runtime_status();
        assert!(!cpu_status.metal.configured, "metal should be disabled");
        assert!(!cpu_status.cuda.configured, "cuda should be disabled");

        // Restore the original backend policy with permissive size thresholds. The cost and
        // qualification owners still select the path; this is parity, not kernel-use evidence.
        set_acceleration_config(AccelerationConfig {
            merkle_min_leaves_gpu: Some(0),
            merkle_min_leaves_metal: Some(0),
            merkle_min_leaves_cuda: Some(0),
            ..guard.original
        });
        let accel_root = check_accelerated_apis();
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
}
