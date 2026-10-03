//! Original-input admission/failure controls and required physical AES receipts.

use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

#[test]
fn unsupported_opt_out_and_busy_decline_before_native_without_changing_admission() {
    for direction in [Direction::Encrypt, Direction::Decrypt] {
        let owner = NativeRound::new();
        let original = ([0x7e; 16], [0xa3; 16]);
        for (allowed, supported) in [(false, true), (true, false)] {
            assert_eq!(
                owner.try_round(
                    direction,
                    original,
                    || allowed,
                    supported,
                    true,
                    |_, _| panic!("refused native entry")
                ),
                None
            );
        }
        let held = owner.qualification.lock().unwrap();
        assert_eq!(
            owner.try_round(
                direction,
                original,
                || true,
                true,
                true,
                |_, _| panic!("busy native entry")
            ),
            None
        );
        drop(held);
        assert_eq!(owner.phase.load(Ordering::Acquire), UNTESTED);
        assert_eq!(owner.completions.load(Ordering::Relaxed), 0);
        assert_eq!(original, ([0x7e; 16], [0xa3; 16]));
    }
}

#[test]
fn parity_failure_and_unwind_quarantine_only_the_original_direction() {
    let owners = [NativeRound::new(), NativeRound::new()];
    let input = ([0x53; 16], [0x71; 16]);
    assert_eq!(
        owners[0].try_round(
            Direction::Encrypt,
            input,
            || true,
            true,
            true,
            |_, _| [0; 16]
        ),
        None
    );
    assert_eq!(owners[0].phase.load(Ordering::Acquire), QUARANTINED);
    assert_eq!(
        owners[0].try_round(
            Direction::Encrypt,
            input,
            || true,
            true,
            true,
            |_, _| panic!("sticky quarantine")
        ),
        None
    );
    assert_eq!(
        owners[1].try_round(
            Direction::Decrypt,
            input,
            || true,
            true,
            true,
            |state, key| Direction::Decrypt.scalar(state, key)
        ),
        Some(Direction::Decrypt.scalar(input.0, input.1))
    );
    assert_eq!(owners[0].completions.load(Ordering::Relaxed), 0);
    assert_eq!(owners[1].completions.load(Ordering::Relaxed), 1);
    for qualifying in [true, false] {
        let owner = NativeRound::new();
        if !qualifying {
            assert!(owner.qualify(Direction::Encrypt, &super::super::aesenc_impl));
        }
        assert!(
            catch_unwind(AssertUnwindSafe(|| owner.try_round(
                Direction::Encrypt,
                input,
                || true,
                true,
                true,
                |_, _| panic!("native unwind")
            )))
            .is_err()
        );
        assert_eq!(owner.phase.load(Ordering::Acquire), QUARANTINED);
        assert_eq!(owner.completions.load(Ordering::Relaxed), 0);
    }
}

#[test]
fn completed_output_is_discarded_after_quarantine_or_policy_change() {
    let input = ([0x53; 16], [0x71; 16]);
    for quarantine in [false, true] {
        let owner = NativeRound::new();
        let enabled = Cell::new(true);
        assert!(owner.qualify(Direction::Encrypt, &super::super::aesenc_impl));
        let output = owner.try_round(
            Direction::Encrypt,
            input,
            || enabled.get(),
            true,
            true,
            |state, key| {
                let result = Direction::Encrypt.scalar(state, key);
                if quarantine {
                    owner.phase.store(QUARANTINED, Ordering::Release);
                } else {
                    enabled.set(false);
                }
                result
            },
        );
        assert_eq!(output, None);
        assert_eq!(owner.completions.load(Ordering::Relaxed), 0);
        assert_eq!(
            owner.phase.load(Ordering::Acquire),
            if quarantine { QUARANTINED } else { ADMITTED }
        );
        assert_eq!(
            output.unwrap_or_else(|| Direction::Encrypt.scalar(input.0, input.1)),
            super::super::aesenc_impl(input.0, input.1)
        );
        if !quarantine {
            enabled.set(true);
            assert!(
                owner
                    .try_round(
                        Direction::Encrypt,
                        input,
                        || enabled.get(),
                        true,
                        true,
                        super::super::aesenc_impl
                    )
                    .is_some()
            );
            assert_eq!(owner.completions.load(Ordering::Relaxed), 1);
        }
    }
}

#[test]
fn admission_is_sticky_and_only_accepted_production_rounds_receive_credit() {
    let owner = NativeRound::new();
    let calls = Cell::new(0);
    let native = |state, key| {
        calls.set(calls.get() + 1);
        Direction::Encrypt.scalar(state, key)
    };
    let input = ([0x15; 16], [0xca; 16]);
    assert!(
        owner
            .try_round(Direction::Encrypt, input, || true, true, false, native)
            .is_some()
    );
    assert_eq!(calls.get(), u64::from(PROBE_CASES) + 1);
    assert_eq!(owner.completions.load(Ordering::Relaxed), 0);
    assert_eq!(owner.synthetic_completions.load(Ordering::Relaxed), 1);
    for n in 1..=3 {
        assert!(
            owner
                .try_round(Direction::Encrypt, input, || true, true, true, native)
                .is_some()
        );
        assert_eq!(calls.get(), u64::from(PROBE_CASES) + 1 + n);
        assert_eq!(owner.completions.load(Ordering::Relaxed), n);
        assert_eq!(owner.synthetic_completions.load(Ordering::Relaxed), 1);
    }
    owner.completions.store(u64::MAX, Ordering::Relaxed);
    assert!(
        owner
            .try_round(Direction::Encrypt, input, || true, true, true, native)
            .is_some()
    );
    assert_eq!(owner.completions.load(Ordering::Relaxed), u64::MAX);
}

#[test]
fn nested_calibration_restores_prior_credit_mode_after_unwind() {
    assert!(!CALIBRATION.with(Cell::get));
    with_calibration(|| {
        assert!(CALIBRATION.with(Cell::get));
        assert!(catch_unwind(|| with_calibration(|| panic!("public calibration unwind"))).is_err());
        assert!(CALIBRATION.with(Cell::get));
    });
    assert!(!CALIBRATION.with(Cell::get));
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86", target_arch = "x86_64"))]
fn required_native(test_name: &str) {
    const CHILD: &str = "IVM_REQUIRED_NATIVE_AES";
    if std::env::var(CHILD).as_deref() != Ok(test_name) {
        let path = module_path!().split_once("::").expect("crate module").1;
        let exact = format!("{path}::{test_name}");
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &exact, "--ignored", "--nocapture"])
            .env(CHILD, test_name)
            .output()
            .expect("isolated native AES qualification");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        print!("{stdout}{stderr}");
        assert!(
            output.status.success(),
            "required native AES qualification failed"
        );
        assert!(
            stdout.contains("1 passed; 0 failed; 0 ignored"),
            "required AES test must execute"
        );
        return;
    }
    assert!(supported(), "required native AES capability is absent");
    crate::set_acceleration_config(crate::AccelerationConfig {
        enable_simd: true,
        enable_metal: false,
        enable_cuda: false,
        ..Default::default()
    });
    crate::vector::set_thread_forced_simd(None);
    let counts = || {
        OWNERS
            .each_ref()
            .map(|owner| owner.completions.load(Ordering::Relaxed))
    };
    assert_eq!(counts(), [0; 2]);
    for direction in [Direction::Encrypt, Direction::Decrypt] {
        assert_eq!(backend(direction), Backend::Native);
    }
    assert_eq!(counts(), [0; 2], "public probes earn no production credit");
    for seed in 0..256_u16 {
        let state = std::array::from_fn(|lane| (seed as u8).wrapping_add(lane as u8 * 13));
        let key = std::array::from_fn(|lane| (seed as u8).wrapping_mul(7).wrapping_add(lane as u8));
        assert_eq!(
            super::super::aesenc(state, key),
            Direction::Encrypt.scalar(state, key)
        );
        assert_eq!(
            super::super::aesdec(state, key),
            Direction::Decrypt.scalar(state, key)
        );
        assert_eq!(counts(), [u64::from(seed) + 1; 2]);
    }
    // Independent FIPS-197 cipher vector through actual ordinary batch calls.
    let key = std::array::from_fn(|lane| lane as u8);
    let plaintext = [std::array::from_fn(|lane| lane as u8 * 17)];
    let ciphertext = [[
        0x69, 0xc4, 0xe0, 0xd8, 0x6a, 0x7b, 0x04, 0x30, 0xd8, 0xcd, 0xb7, 0x80, 0x70, 0xb4, 0xc5,
        0x5a,
    ]];
    let keys = super::super::aes128_expand_key(key);
    let mut output = [[0; 16]];
    assert!(super::super::aes128_encrypt_many_into(
        &plaintext,
        &keys,
        &mut output
    ));
    assert_eq!(output, ciphertext);
    assert!(super::super::aes128_decrypt_many_into(
        &ciphertext,
        &keys,
        &mut output
    ));
    assert_eq!(output, plaintext);
    assert_eq!(counts(), [265; 2]);
    with_calibration(|| {
        for direction in [Direction::Encrypt, Direction::Decrypt] {
            assert_eq!(
                round(direction, plaintext[0], key),
                direction.scalar(plaintext[0], key)
            );
        }
    });
    assert_eq!(counts(), [265; 2], "cost samples earn no production credit");
    assert_eq!(
        OWNERS
            .each_ref()
            .map(|owner| owner.synthetic_completions.load(Ordering::Relaxed)),
        [1; 2]
    );
    for file_opt_out in [false, true] {
        if file_opt_out {
            crate::set_acceleration_config(crate::AccelerationConfig {
                enable_simd: false,
                enable_metal: false,
                enable_cuda: false,
                ..Default::default()
            });
            crate::vector::set_thread_forced_simd(Some(crate::vector::detected_simd_choice()));
        } else {
            crate::vector::set_thread_forced_simd(Some(crate::vector::SimdChoice::Scalar));
        }
        for direction in [Direction::Encrypt, Direction::Decrypt] {
            assert_eq!(backend(direction), Backend::Scalar);
            assert_eq!(
                round(direction, plaintext[0], key),
                direction.scalar(plaintext[0], key)
            );
            assert_eq!(
                OWNERS[direction.index()].phase.load(Ordering::Acquire),
                ADMITTED
            );
        }
        assert_eq!(counts(), [265; 2]);
    }
    crate::vector::set_thread_forced_simd(None);
    crate::set_acceleration_config(crate::AccelerationConfig {
        enable_simd: true,
        enable_metal: false,
        enable_cuda: false,
        ..Default::default()
    });
    for direction in [Direction::Encrypt, Direction::Decrypt] {
        assert_eq!(backend(direction), Backend::Native);
        assert_eq!(
            round(direction, plaintext[0], key),
            direction.scalar(plaintext[0], key)
        );
    }
    assert_eq!(counts(), [266; 2]);
    println!(
        "IVM_NATIVE_AES_RECEIPT architecture={} encrypted_rounds=266 decrypted_rounds=266",
        std::env::consts::ARCH
    );
}

#[cfg(target_arch = "aarch64")]
#[test]
#[ignore = "requires a physical AArch64 AES runner; mandatory explicit qualification"]
fn required_arm_aes_executes_production_paths_and_preserves_policy() {
    required_native("required_arm_aes_executes_production_paths_and_preserves_policy");
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[test]
#[ignore = "requires a physical x86 AES-NI runner; mandatory explicit qualification"]
fn required_x86_aesni_executes_production_paths_and_preserves_policy() {
    required_native("required_x86_aesni_executes_production_paths_and_preserves_policy");
}
