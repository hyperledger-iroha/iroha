//! Driverless policy boundaries and canonical Merkle fallback regressions.

use std::cell::Cell;

use super::{DEFAULT_GPU_MIN_LEAVES, ResolvedMerklePolicy, cpu_preference};
use crate::{AccelerationConfig, ByteMerkleTree, VMError};

struct ConfigRestore(AccelerationConfig);

impl Drop for ConfigRestore {
    fn drop(&mut self) {
        crate::set_acceleration_config(self.0);
    }
}

#[test]
fn explicit_zero_is_not_generic_inheritance() {
    let generic = AccelerationConfig {
        merkle_min_leaves_gpu: Some(73),
        ..AccelerationConfig::default()
    };
    let inherited = ResolvedMerklePolicy::resolve(generic, None);
    assert!(
        inherited
            .try_metal::<()>(72, || panic!("below inherited floor"))
            .is_none()
    );
    assert!(
        inherited
            .try_cuda::<()>(72, || panic!("below inherited floor"))
            .is_none()
    );
    assert_eq!(inherited.try_metal(73, || Some(1)), Some(1));
    assert_eq!(inherited.try_cuda(73, || Some(2)), Some(2));
    let explicit = ResolvedMerklePolicy::resolve(
        AccelerationConfig {
            merkle_min_leaves_metal: Some(0),
            merkle_min_leaves_cuda: Some(0),
            ..generic
        },
        None,
    );
    assert_eq!(explicit.try_metal(1, || Some(3)), Some(3));
    assert_eq!(explicit.try_cuda(1, || Some(4)), Some(4));
    let generic_zero = ResolvedMerklePolicy::resolve(
        AccelerationConfig {
            merkle_min_leaves_gpu: Some(0),
            ..generic
        },
        None,
    );
    assert_eq!(generic_zero.metal.minimum, 0);
    assert_eq!(generic_zero.cuda.minimum, 0);
}

#[test]
fn backend_floors_override_generic_and_refuse_before_attempt() {
    let policy = ResolvedMerklePolicy::resolve(
        AccelerationConfig {
            merkle_min_leaves_gpu: Some(32),
            merkle_min_leaves_metal: Some(8),
            merkle_min_leaves_cuda: Some(64),
            ..AccelerationConfig::default()
        },
        None,
    );
    let attempts = Cell::new(0);
    let complete = || {
        attempts.set(attempts.get() + 1);
        Some(())
    };
    assert!(policy.try_metal(7, complete).is_none());
    assert_eq!(attempts.get(), 0);
    assert!(policy.try_metal(8, complete).is_some());
    assert_eq!(attempts.get(), 1);
    assert!(policy.try_cuda(63, complete).is_none());
    assert_eq!(attempts.get(), 1);
    assert!(policy.try_cuda(64, complete).is_some());
    assert_eq!(attempts.get(), 2);
    // A refusal from the admitted native owner remains a refusal, never success.
    assert!(policy.try_cuda::<()>(64, || None).is_none());
}

#[test]
fn cpu_ceiling_opt_out_and_zero_gpu_cap_prevent_attempts() {
    let enabled = AccelerationConfig {
        merkle_min_leaves_gpu: Some(0),
        ..AccelerationConfig::default()
    };
    let cpu = ResolvedMerklePolicy::resolve(enabled, Some(10));
    assert!(cpu.try_metal::<()>(10, || panic!("CPU ceiling")).is_none());
    assert!(cpu.try_cuda::<()>(10, || panic!("CPU ceiling")).is_none());
    assert_eq!(cpu.try_metal(11, || Some(1)), Some(1));
    assert_eq!(cpu.try_cuda(11, || Some(2)), Some(2));
    let zero = ResolvedMerklePolicy::resolve(enabled, Some(0));
    assert_eq!(zero.try_metal(1, || Some(1)), Some(1));
    assert_eq!(zero.try_cuda(1, || Some(2)), Some(2));
    for config in [
        AccelerationConfig {
            enable_metal: false,
            enable_cuda: false,
            ..enabled
        },
        AccelerationConfig {
            max_gpus: Some(0),
            ..enabled
        },
    ] {
        let refused = ResolvedMerklePolicy::resolve(config, None);
        assert!(
            refused
                .try_metal::<()>(usize::MAX, || panic!("disabled"))
                .is_none()
        );
        assert!(
            refused
                .try_cuda::<()>(usize::MAX, || panic!("disabled"))
                .is_none()
        );
    }
}

#[test]
fn complete_config_reapplication_resets_defaults_and_keeps_old_snapshot() {
    let _serial = crate::vector::forced_simd_test_lock();
    let _restore = ConfigRestore(crate::acceleration_config());
    let custom = AccelerationConfig {
        enable_metal: false,
        enable_cuda: false,
        merkle_min_leaves_gpu: Some(7),
        merkle_min_leaves_metal: Some(0),
        merkle_min_leaves_cuda: Some(19),
        prefer_cpu_sha2_max_leaves_aarch64: Some(17),
        prefer_cpu_sha2_max_leaves_x86: Some(17),
        ..crate::acceleration_config()
    };
    crate::set_acceleration_config(custom);
    let original = ResolvedMerklePolicy::current();
    assert_eq!(original.metal.minimum, 0);
    assert_eq!(original.cuda.minimum, 19);
    assert_eq!(original.cpu_prefer_max, cpu_preference(custom));
    let inherited = AccelerationConfig {
        merkle_min_leaves_metal: None,
        merkle_min_leaves_cuda: None,
        prefer_cpu_sha2_max_leaves_aarch64: None,
        prefer_cpu_sha2_max_leaves_x86: None,
        ..custom
    };
    crate::set_acceleration_config(inherited);
    let reset = ResolvedMerklePolicy::current();
    assert_eq!(reset.metal.minimum, 7);
    assert_eq!(reset.cuda.minimum, 7);
    assert_eq!(reset.cpu_prefer_max, cpu_preference(inherited));
    crate::set_acceleration_config(AccelerationConfig {
        merkle_min_leaves_gpu: None,
        ..inherited
    });
    let default = ResolvedMerklePolicy::current();
    assert_eq!(default.metal.minimum, DEFAULT_GPU_MIN_LEAVES);
    assert_eq!(default.cuda.minimum, DEFAULT_GPU_MIN_LEAVES);
    assert_eq!(original.metal.minimum, 0);
    assert_eq!(original.cuda.minimum, 19);
    assert_eq!(original.cpu_prefer_max, cpu_preference(custom));
}

#[test]
fn refused_acceleration_keeps_retained_tree_and_canonical_fallback() {
    let _serial = crate::vector::forced_simd_test_lock();
    let _restore = ConfigRestore(crate::acceleration_config());
    crate::set_acceleration_config(AccelerationConfig {
        enable_metal: false,
        enable_cuda: false,
        merkle_min_leaves_gpu: Some(0),
        merkle_min_leaves_metal: Some(0),
        merkle_min_leaves_cuda: Some(0),
        prefer_cpu_sha2_max_leaves_aarch64: Some(0),
        prefer_cpu_sha2_max_leaves_x86: Some(0),
        ..crate::acceleration_config()
    });
    let bytes = [0x53; 97];
    for chunk in [1, 16, 32] {
        for length in [0, 1, 31, 33, 97] {
            let data = &bytes[..length];
            let expected = ByteMerkleTree::from_bytes(data, chunk).unwrap();
            assert_eq!(
                ByteMerkleTree::from_bytes_accel(data, chunk)
                    .unwrap()
                    .root(),
                expected.root()
            );
            assert_eq!(
                ByteMerkleTree::root_from_bytes_accel(data, chunk).unwrap(),
                expected.root()
            );
            let retained = ByteMerkleTree::from_bytes(&[0x19; 97][..length], chunk).unwrap();
            let root = retained.root();
            let proof = retained.root_and_path(0).unwrap();
            assert!(!retained.recompute_all_leaves_accel(data));
            assert_eq!(retained.root(), root);
            assert_eq!(retained.root_and_path(0).unwrap(), proof);
            retained.recompute_all_leaves_parallel(data);
            assert_eq!(retained.root(), expected.root());
        }
    }
    for chunk in [0, 33] {
        assert!(matches!(
            ByteMerkleTree::from_bytes_accel(&bytes, chunk),
            Err(VMError::MemoryOutOfBounds)
        ));
        assert!(matches!(
            ByteMerkleTree::root_from_bytes_accel(&bytes, chunk),
            Err(VMError::MemoryOutOfBounds)
        ));
    }
}
