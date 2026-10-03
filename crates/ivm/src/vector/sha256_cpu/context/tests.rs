//! Explicit worker context, actual backend observation and global-pool scope.

use super::*;

#[test]
fn observation_rejects_mixed_backends_and_zero_work_is_not_native_evidence() {
    let empty = Sha256Observed::default();
    for backend in [Sha256Backend::Scalar, Sha256Backend::Native] {
        assert!(empty.matches(backend, false));
        assert!(!empty.matches(backend, true));
        assert!(Sha256Observed::completed(backend).matches(backend, true));
    }
    let mixed = Sha256Observed::completed(Sha256Backend::Scalar)
        .merge(Sha256Observed::completed(Sha256Backend::Native));
    assert!(!mixed.matches(Sha256Backend::Native, false));
    assert!(!mixed.matches(Sha256Backend::Scalar, false));
}

#[test]
fn caller_scalar_context_survives_worker_override_and_nested_pool() {
    let previous = super::super::super::set_thread_forced_simd(Some(SimdChoice::Scalar));
    let context = Sha256Context::production();
    super::super::super::set_thread_forced_simd(previous);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .unwrap();
    pool.broadcast(|_| {
        let previous = super::super::super::set_thread_forced_simd(Some(
            super::super::super::detected_simd_choice(),
        ));
        assert!(!context.allowed());
        assert!(!context.synthetic().allowed());
        let mut actual = super::super::INITIAL;
        let mut expected = actual;
        let block = [0x63; 64];
        crate::sha256_ref::sha256_compress_scalar_ref(&mut expected, &block);
        assert_eq!(context.compress(&mut actual, &block), Sha256Backend::Scalar);
        assert_eq!(actual, expected);
        assert!(Sha256Baseline::capture(context).is_none());
        super::super::super::set_thread_forced_simd(previous);
    });
    assert!(!context.is_synthetic());
    assert!(context.synthetic().is_synthetic());
}

#[cfg(target_arch = "aarch64")]
#[test]
#[ignore = "requires physical ARM SHA2 execution; run explicitly for native qualification"]
fn required_arm_parallel_sha_context_preserves_policy_and_exact_receipt_banks() {
    const CHILD: &str = "IVM_REQUIRED_ARM_PARALLEL_SHA";
    if std::env::var(CHILD).as_deref() != Ok("1") {
        let module = module_path!().split_once("::").unwrap().1;
        let test = format!(
            "{module}::required_arm_parallel_sha_context_preserves_policy_and_exact_receipt_banks"
        );
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &test, "--ignored", "--nocapture"])
            .env(CHILD, "1")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        print!("{stdout}{stderr}");
        assert!(
            output.status.success(),
            "required physical parallel SHA failed"
        );
        assert!(stdout.contains("1 passed; 0 failed; 0 ignored"));
        return;
    }
    use std::sync::atomic::Ordering;
    let config = crate::AccelerationConfig {
        enable_simd: true,
        enable_metal: false,
        enable_cuda: false,
        ..Default::default()
    };
    crate::set_acceleration_config(config);
    super::super::super::set_thread_forced_simd(None);
    assert!(
        super::super::supported(),
        "physical ARM SHA2 capability is required"
    );
    let context = Sha256Context::production();
    let baseline = Sha256Baseline::capture(context).expect("original global CPU owner");
    assert_eq!(baseline.backend, Sha256Backend::Native);
    let owner = &super::super::OWNER;
    assert_eq!(owner.completions.load(Ordering::Relaxed), 0);
    assert_eq!(owner.synthetic_completions.load(Ordering::Relaxed), 0);
    let leaves = 8_193;
    let chunk = 17;
    let input = vec![0x63; leaves * chunk - 7];
    let expected = iroha_crypto::MerkleTree::<[u8; 32]>::from_byte_chunks(&input, chunk).unwrap();
    let expected = *expected.root().unwrap().as_ref();
    let (tree, observed) =
        crate::ByteMerkleTree::from_bytes_parallel_in_context(&input, chunk, context.synthetic())
            .unwrap();
    assert_eq!(tree.root(), expected);
    assert!(baseline.accepts(observed, true));
    assert_eq!(owner.completions.load(Ordering::Relaxed), 0);
    assert_eq!(
        owner.synthetic_completions.load(Ordering::Relaxed),
        leaves as u64
    );
    let tree = crate::ByteMerkleTree::from_bytes_parallel(&input, chunk).unwrap();
    assert_eq!(tree.root(), expected);
    assert_eq!(owner.completions.load(Ordering::Relaxed), leaves as u64);
    assert_eq!(
        owner.synthetic_completions.load(Ordering::Relaxed),
        leaves as u64
    );
    super::super::super::set_thread_forced_simd(Some(SimdChoice::Scalar));
    let scalar = Sha256Context::production();
    super::super::super::set_thread_forced_simd(None);
    let (tree, observed) =
        crate::ByteMerkleTree::from_bytes_parallel_in_context(&input, chunk, scalar).unwrap();
    assert_eq!(tree.root(), expected);
    assert!(observed.matches(Sha256Backend::Scalar, true));
    assert_eq!(owner.completions.load(Ordering::Relaxed), leaves as u64);
    crate::set_acceleration_config(crate::AccelerationConfig {
        enable_simd: false,
        ..config
    });
    let (tree, observed) =
        crate::ByteMerkleTree::from_bytes_parallel_in_context(&input, chunk, context).unwrap();
    assert_eq!(tree.root(), expected);
    assert!(observed.matches(Sha256Backend::Scalar, true));
    assert!(!baseline.is_current(context));
    assert_eq!(owner.completions.load(Ordering::Relaxed), leaves as u64);
    crate::set_acceleration_config(config);
    assert!(baseline.is_current(context));
    let zero = vec![0; input.len()];
    let (_, observed) =
        crate::ByteMerkleTree::from_bytes_parallel_in_context(&zero, chunk, context.synthetic())
            .unwrap();
    assert!(baseline.accepts(observed, false));
    assert!(!baseline.accepts(observed, true));
    assert_eq!(
        owner.synthetic_completions.load(Ordering::Relaxed),
        leaves as u64
    );
    println!(
        "IVM_ARM_PARALLEL_SHA_RECEIPT workers={} production={} synthetic={}",
        baseline.workers,
        owner.completions.load(Ordering::Relaxed),
        owner.synthetic_completions.load(Ordering::Relaxed)
    );
}
