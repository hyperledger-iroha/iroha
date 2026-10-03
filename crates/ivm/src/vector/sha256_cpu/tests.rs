//! Native admission, original-input fallback, and required physical SHA parity.

use super::*;
use std::cell::Cell;

#[test]
fn opt_out_unsupported_busy_and_mismatch_preserve_original_input() {
    let owner = NativeSha256::new();
    let original = [7; 8];
    let block = [0xa5; 64];
    let mut state = original;
    for (allowed, supported) in [(false, true), (true, false)] {
        assert!(!owner.try_compress(
            &mut state,
            &block,
            || allowed,
            supported,
            false,
            |_, _| {
                panic!("declined operation reached native code");
            }
        ));
        assert_eq!(state, original);
        assert_eq!(owner.phase.load(Ordering::Acquire), UNTESTED);
    }
    let held = owner.qualification.lock().unwrap();
    assert!(!owner.try_compress(
        &mut state,
        &block,
        || true,
        true,
        false,
        |_, _| {
            panic!("busy qualification must use fallback");
        }
    ));
    drop(held);
    assert!(!owner.try_compress(
        &mut state,
        &block,
        || true,
        true,
        false,
        |output, _| {
            output.fill(0xdead_beef);
        }
    ));
    assert_eq!(state, original);
    assert_eq!(owner.phase.load(Ordering::Acquire), QUARANTINED);
    assert!(!owner.try_compress(
        &mut state,
        &block,
        || true,
        true,
        false,
        |_, _| {
            panic!("mismatch quarantine cannot be cleared by a new caller");
        }
    ));
    assert_eq!(owner.completions.load(Ordering::Relaxed), 0);
}

#[test]
fn admission_is_sticky_and_probes_receive_no_completion_credit() {
    let owner = NativeSha256::new();
    let calls = Cell::new(0);
    let native = |state: &mut [u32; 8], block: &[u8; 64]| {
        calls.set(calls.get() + 1);
        sha256_compress_scalar_ref(state, block);
    };
    let mut actual = INITIAL;
    let mut expected = INITIAL;
    for index in 0..3 {
        let block = [index; 64];
        sha256_compress_scalar_ref(&mut expected, &block);
        assert!(owner.try_compress(&mut actual, &block, || true, true, false, native));
        assert_eq!(actual, expected);
        assert_eq!(calls.get(), 5 + u64::from(index));
        assert_eq!(
            owner.completions.load(Ordering::Relaxed),
            1 + u64::from(index)
        );
    }
    owner.completions.store(u64::MAX, Ordering::Relaxed);
    assert!(owner.try_compress(&mut actual, &[0; 64], || true, true, false, native));
    assert_eq!(owner.completions.load(Ordering::Relaxed), u64::MAX);
}

#[test]
fn completed_staging_is_discarded_after_quarantine_and_unwind_is_sticky() {
    let owner = NativeSha256::new();
    assert!(owner.qualify(&sha256_compress_scalar_ref));
    let original = INITIAL;
    let mut state = original;
    assert!(!owner.try_compress(
        &mut state,
        &[0; 64],
        || true,
        true,
        false,
        |out, block| {
            sha256_compress_scalar_ref(out, block);
            owner.phase.store(QUARANTINED, Ordering::Release);
        }
    ));
    assert_eq!(state, original);
    assert_eq!(owner.completions.load(Ordering::Relaxed), 0);

    for qualifying in [true, false] {
        let owner = NativeSha256::new();
        if !qualifying {
            assert!(owner.qualify(&sha256_compress_scalar_ref));
        }
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            owner.try_compress(
                &mut state,
                &[0; 64],
                || true,
                true,
                false,
                |out, _| {
                    out.fill(0);
                    panic!("native test unwind");
                },
            );
        }));
        assert!(panic.is_err());
        assert_eq!(state, original);
        assert_eq!(owner.phase.load(Ordering::Acquire), QUARANTINED);
        assert_eq!(owner.completions.load(Ordering::Relaxed), 0);
    }
}

#[test]
fn synthetic_and_production_work_use_distinct_equal_cost_completion_banks() {
    let owner = NativeSha256::new();
    assert!(owner.qualify(&sha256_compress_scalar_ref));
    let mut actual = INITIAL;
    let mut expected = INITIAL;
    for synthetic in [false, true, true, false] {
        sha256_compress_scalar_ref(&mut expected, &[0x63; 64]);
        assert!(owner.try_compress(
            &mut actual,
            &[0x63; 64],
            || true,
            true,
            synthetic,
            sha256_compress_scalar_ref,
        ));
        assert_eq!(actual, expected);
    }
    assert_eq!(owner.completions.load(Ordering::Relaxed), 2);
    assert_eq!(owner.synthetic_completions.load(Ordering::Relaxed), 2);
    owner
        .synthetic_completions
        .store(u64::MAX, Ordering::Relaxed);
    assert!(owner.try_compress(
        &mut actual,
        &[0; 64],
        || true,
        true,
        true,
        sha256_compress_scalar_ref
    ));
    assert_eq!(owner.completions.load(Ordering::Relaxed), 2);
    assert_eq!(
        owner.synthetic_completions.load(Ordering::Relaxed),
        u64::MAX
    );
}

#[test]
fn policy_refusal_after_native_completion_preserves_input_and_healthy_admission() {
    let owner = NativeSha256::new();
    assert!(owner.qualify(&sha256_compress_scalar_ref));
    for synthetic in [false, true] {
        let allowed = Cell::new(true);
        let mut state = INITIAL;
        assert!(!owner.try_compress(
            &mut state,
            &[0x53; 64],
            || allowed.get(),
            true,
            synthetic,
            |staged, block| {
                sha256_compress_scalar_ref(staged, block);
                allowed.set(false);
            }
        ));
        assert_eq!(state, INITIAL);
        assert_eq!(owner.phase.load(Ordering::Acquire), ADMITTED);
        assert_eq!(owner.completions.load(Ordering::Relaxed), 0);
        assert_eq!(owner.synthetic_completions.load(Ordering::Relaxed), 0);
    }
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
fn required_native(test_name: &str) {
    const CHILD: &str = "IVM_REQUIRED_NATIVE_SHA256";
    if std::env::var(CHILD).as_deref() != Ok(test_name) {
        let path = module_path!().split_once("::").unwrap().1;
        let exact = format!("{path}::{test_name}");
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &exact, "--ignored", "--nocapture"])
            .env(CHILD, test_name)
            .output()
            .expect("isolated required native SHA qualification");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        print!("{stdout}{stderr}");
        assert!(
            output.status.success(),
            "required native SHA qualification failed"
        );
        assert!(
            stdout.contains("1 passed; 0 failed; 0 ignored"),
            "required test did not execute"
        );
        return;
    }
    assert!(supported(), "required native SHA CPU capability is absent");
    crate::set_acceleration_config(crate::AccelerationConfig {
        enable_simd: true,
        enable_metal: false,
        enable_cuda: false,
        ..Default::default()
    });
    super::super::set_thread_forced_simd(None);
    assert_eq!(OWNER.completions.load(Ordering::Relaxed), 0);

    // Exercise the public production path: a scalar fallback cannot earn native receipts.
    let mut blocks = 0;
    for seed in 0..128_u32 {
        let block =
            std::array::from_fn(|index| (index as u8).wrapping_mul(37).wrapping_add(seed as u8));
        let original = std::array::from_fn(|index| {
            seed.wrapping_mul(0x9e37_79b9).rotate_left(index as u32 * 3)
        });
        let mut actual = original;
        let mut expected = original;
        for _ in 0..3 {
            sha256_compress_scalar_ref(&mut expected, &block);
            super::super::sha256_compress(&mut actual, &block);
            blocks += 1;
            assert_eq!(actual, expected, "seed {seed}");
            assert_eq!(OWNER.completions.load(Ordering::Relaxed), blocks);
        }
    }
    // Compare complete, correctly padded messages against the independent sha2 crate.
    use sha2::{Digest, Sha256};
    for length in [0, 1, 55, 56, 64, 65, 129, 4096] {
        let message: Vec<u8> = (0..length)
            .map(|index| (index as u8).wrapping_mul(53))
            .collect();
        let expected = Sha256::digest(&message);
        let mut padded = message.clone();
        padded.push(0x80);
        while padded.len() % 64 != 56 {
            padded.push(0);
        }
        padded.extend_from_slice(&((length as u64) * 8).to_be_bytes());
        let mut actual = INITIAL;
        for chunk in padded.chunks_exact(64) {
            super::super::sha256_compress(&mut actual, chunk.try_into().unwrap());
            blocks += 1;
        }
        let digest: Vec<u8> = actual.into_iter().flat_map(u32::to_be_bytes).collect();
        assert_eq!(
            digest.as_slice(),
            expected.as_slice(),
            "message length {length}"
        );
        assert_eq!(OWNER.completions.load(Ordering::Relaxed), blocks);
    }

    let block = [0x3c; 64];
    let mut expected = INITIAL;
    sha256_compress_scalar_ref(&mut expected, &block);
    for file_opt_out in [false, true] {
        if file_opt_out {
            crate::set_acceleration_config(crate::AccelerationConfig {
                enable_simd: false,
                enable_metal: false,
                enable_cuda: false,
                ..Default::default()
            });
            // A thread override cannot defeat the operator's file policy.
            super::super::set_thread_forced_simd(Some(super::super::detected_simd_choice()));
        } else {
            super::super::set_thread_forced_simd(Some(super::super::SimdChoice::Scalar));
        }
        let mut actual = INITIAL;
        super::super::sha256_compress(&mut actual, &block);
        assert_eq!(actual, expected);
        assert_eq!(OWNER.completions.load(Ordering::Relaxed), blocks);
        assert_eq!(OWNER.phase.load(Ordering::Acquire), ADMITTED);
    }
    super::super::set_thread_forced_simd(None);
    crate::set_acceleration_config(crate::AccelerationConfig {
        enable_simd: true,
        enable_metal: false,
        enable_cuda: false,
        ..Default::default()
    });
    let mut actual = INITIAL;
    super::super::sha256_compress(&mut actual, &block);
    assert_eq!(actual, expected);
    assert_eq!(OWNER.completions.load(Ordering::Relaxed), blocks + 1);
    println!(
        "IVM_NATIVE_SHA256_RECEIPT architecture={} completed_blocks={}",
        std::env::consts::ARCH,
        blocks + 1
    );
}

#[cfg(target_arch = "aarch64")]
#[test]
#[ignore = "requires an AArch64 SHA2 runner; mandatory explicit qualification"]
fn required_arm_sha2_executes_production_path_and_matches_scalar() {
    required_native("required_arm_sha2_executes_production_path_and_matches_scalar");
}

#[cfg(target_arch = "x86_64")]
#[test]
#[ignore = "requires an x86_64 SHA-NI runner; mandatory explicit qualification"]
fn required_x86_sha_ni_executes_production_path_and_matches_scalar() {
    required_native("required_x86_sha_ni_executes_production_path_and_matches_scalar");
}
