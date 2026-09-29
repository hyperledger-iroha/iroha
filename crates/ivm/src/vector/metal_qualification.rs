//! Required, non-skipping parity checks for every production Metal pipeline.

#[cfg(not(target_os = "macos"))]
#[test]
fn required_metal_hardware_needs_macos() {
    panic!("required Metal qualification needs a macOS Metal runner");
}

#[cfg(target_os = "macos")]
use super::*;

#[cfg(target_os = "macos")]
fn require_dispatch<T: std::fmt::Debug + PartialEq>(
    kernel: MetalKernel,
    expected: T,
    operation: impl FnOnce() -> T,
) {
    let health = metal_runtime::current_health().expect("physical qualification pin");
    let before = health.completions(kernel as usize);
    assert_eq!(operation(), expected, "{kernel:?}: scalar parity");
    let completed = health.completions(kernel as usize).saturating_sub(before);
    assert!(
        completed > 0,
        "{kernel:?}: no completed production dispatch"
    );
    assert!(
        !metal_disabled(),
        "{kernel:?}: backend quarantined during work"
    );
    println!(
        "IVM_METAL_RECEIPT device={} kernel={kernel:?} completed_batches={completed}",
        health.identity()
    );
}

#[cfg(target_os = "macos")]
#[test]
fn required_metal_hardware_covers_every_production_pipeline() {
    const CHILD: &str = "IVM_METAL_ALL_DEVICES_CONTROL";
    if std::env::var(CHILD).as_deref() != Ok("1") {
        let path = module_path!().split_once("::").expect("crate module").1;
        let test = format!("{path}::required_metal_hardware_covers_every_production_pipeline");
        let output = std::process::Command::new(std::env::current_exe().expect("test executable"))
            .args(["--exact", &test, "--nocapture"])
            .env(CHILD, "1")
            .output()
            .expect("isolated all-device qualification");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        print!("{stdout}{stderr}");
        assert!(
            output.status.success(),
            "physical all-device qualification failed"
        );
        assert!(
            stdout.contains("1 passed; 0 failed; 0 ignored"),
            "required control must execute"
        );
        return;
    }
    let count = metal_runtime::device_slots()
        .expect("complete allowed physical Metal inventory must have charged records");
    assert!(
        count > 0,
        "required physical Metal device inventory is empty"
    );
    for index in 0..count {
        metal_runtime::with_device_for_qualification(index, || required_on_device(index))
            .expect("every observed physical device must qualify");
    }
}

#[cfg(target_os = "macos")]
fn required_on_device(index: usize) {
    use ed25519_dalek::{Signer as _, SigningKey};
    use sha2::{Digest as _, Sha256};

    struct RestorePolicy(crate::AccelerationConfig);
    impl Drop for RestorePolicy {
        fn drop(&mut self) {
            crate::set_acceleration_config(self.0);
        }
    }
    let _restore = RestorePolicy(crate::acceleration_config());
    crate::set_acceleration_config(crate::AccelerationConfig {
        enable_simd: false,
        enable_metal: false,
        enable_cuda: false,
        ..Default::default()
    });
    let initial_sha = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut block = [0u8; 64];
    block[..3].copy_from_slice(b"abc");
    block[3] = 0x80;
    block[63] = 24;
    let mut expected_sha = initial_sha;
    sha256_compress(&mut expected_sha, &block);
    let digest: [u8; 32] = Sha256::digest(b"abc").into();
    let initial_keccak = std::array::from_fn(|index| (index as u64).wrapping_mul(0x1234_5678));
    let mut expected_keccak = initial_keccak;
    tiny_keccak::keccakf(&mut expected_keccak);
    let key = SigningKey::from_bytes(&[0x53; 32]);
    let public_key = key.verifying_key().to_bytes();
    let message = b"ivm required Metal qualification";
    let signature = key.sign(message).to_bytes();
    let mut bad_signature = signature;
    bad_signature[32] ^= 1;
    let hram = crate::signature::ed25519_challenge_scalar_bytes(&signature, &public_key, message);
    assert!(
        key.verifying_key()
            .verify_strict(message, &signature.into())
            .is_ok()
    );
    assert!(
        key.verifying_key()
            .verify_strict(message, &bad_signature.into())
            .is_err()
    );

    let startup_counts = MetalKernel::ALL.map(metal_completed_dispatches);
    crate::set_acceleration_config(crate::AccelerationConfig {
        enable_simd: false,
        enable_metal: true,
        enable_cuda: false,
        ..Default::default()
    });
    assert!(
        metal_available(),
        "required Metal unavailable: {:?}",
        metal_last_error_message()
    );
    assert!(
        metal_merkle_cost_profile().is_some(),
        "required isolated Metal Merkle cost calibration did not qualify"
    );
    assert!(
        with_metal_state_try(|ctx| Some(ctx.ed25519_signature.is_some())).unwrap_or(false),
        "required Ed25519 pipeline failed startup qualification"
    );
    let queue_identity = with_metal_state(|state| Retained::as_ptr(&state.queue) as usize)
        .expect("qualified process queue");
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(move || {
                    metal_runtime::with_device_for_qualification(index, || {
                        with_metal_state(|state| Retained::as_ptr(&state.queue) as usize)
                    })
                    .flatten()
                })
            })
            .collect();
        for worker in workers {
            assert_eq!(worker.join().expect("Metal worker"), Some(queue_identity));
        }
    });
    assert_eq!(
        MetalKernel::ALL.map(metal_completed_dispatches),
        startup_counts,
        "startup probes must not masquerade as production work"
    );

    for size in [1, 3, 257] {
        let lhs = [u32::MAX, 1, 0x1234_5678, size as u32];
        let rhs = [1, u32::MAX, 0xdead_beef, 7];
        require_dispatch(
            MetalKernel::Add32,
            Some(std::array::from_fn(|i| lhs[i].wrapping_add(rhs[i]))),
            || metal_vadd32(lhs, rhs),
        );
        let packed: [u64; 2] = std::array::from_fn(|i| {
            let a = u64::from(lhs[2 * i]) | (u64::from(lhs[2 * i + 1]) << 32);
            let b = u64::from(rhs[2 * i]) | (u64::from(rhs[2 * i + 1]) << 32);
            a.wrapping_add(b)
        });
        let expected64 = [
            packed[0] as u32,
            (packed[0] >> 32) as u32,
            packed[1] as u32,
            (packed[1] >> 32) as u32,
        ];
        require_dispatch(MetalKernel::Add64, Some(expected64), || {
            metal_vadd64(lhs, rhs)
        });
        require_dispatch(
            MetalKernel::And,
            Some(std::array::from_fn(|i| lhs[i] & rhs[i])),
            || metal_vand(lhs, rhs),
        );
        require_dispatch(
            MetalKernel::Xor,
            Some(std::array::from_fn(|i| lhs[i] ^ rhs[i])),
            || metal_vxor(lhs, rhs),
        );
        require_dispatch(
            MetalKernel::Or,
            Some(std::array::from_fn(|i| lhs[i] | rhs[i])),
            || metal_vor(lhs, rhs),
        );
        require_dispatch(MetalKernel::Sha256, (true, expected_sha), || {
            let mut state = initial_sha;
            (metal_sha256_compress(&mut state, &block), state)
        });
        require_dispatch(MetalKernel::Sha256Leaves, Some(vec![digest; size]), || {
            metal_sha256_leaves(&vec![block; size])
        });
        let leaves = vec![digest; size.max(2)];
        let mut level = leaves.clone();
        while level.len() > 1 {
            level = level
                .chunks(2)
                .map(|pair| {
                    if pair.len() == 1 {
                        pair[0]
                    } else {
                        let mut hash = Sha256::new();
                        hash.update(pair[0]);
                        hash.update(pair[1]);
                        hash.finalize().into()
                    }
                })
                .collect();
        }
        require_dispatch(MetalKernel::Sha256Pairs, Some(level[0]), || {
            metal_sha256_pairs_reduce(&leaves)
        });
        require_dispatch(MetalKernel::Keccak, (true, expected_keccak), || {
            let mut state = initial_keccak;
            (metal_keccak_f1600(&mut state), state)
        });
        let states: Vec<[u8; 16]> = (0..size)
            .map(|index| std::array::from_fn(|byte| (index + byte * 17) as u8))
            .collect();
        let round_key = [0xCA; 16];
        let expected_enc: Vec<_> = states
            .iter()
            .map(|&state| crate::aes::aesenc_impl(state, round_key))
            .collect();
        let expected_dec: Vec<_> = states
            .iter()
            .map(|&state| crate::aes::aesdec_impl(state, round_key))
            .collect();
        require_dispatch(MetalKernel::AesEnc, Some(expected_enc[0]), || {
            metal_aesenc_round(states[0], round_key)
        });
        require_dispatch(MetalKernel::AesDec, Some(expected_dec[0]), || {
            metal_aesdec_round(states[0], round_key)
        });
        require_dispatch(MetalKernel::AesEncBatch, (true, expected_enc), || {
            let mut output = vec![[0; 16]; states.len()];
            let completed = metal_aesenc_batch_into(&states, round_key, &mut output);
            (completed, output)
        });
        require_dispatch(MetalKernel::AesDecBatch, (true, expected_dec), || {
            let mut output = vec![[0; 16]; states.len()];
            let completed = metal_aesdec_batch_into(&states, round_key, &mut output);
            (completed, output)
        });
        let round_keys = [round_key, [0x35; 16], [0xF7; 16]];
        let expected_enc = states
            .iter()
            .map(|&state| round_keys.into_iter().fold(state, crate::aes::aesenc_impl))
            .collect();
        let expected_dec = states
            .iter()
            .map(|&state| round_keys.into_iter().fold(state, crate::aes::aesdec_impl))
            .collect();
        require_dispatch(MetalKernel::AesEncRounds, (true, expected_enc), || {
            let mut output = vec![[0; 16]; states.len()];
            let completed = metal_aesenc_rounds_batch_into(&states, &round_keys, &mut output);
            (completed, output)
        });
        require_dispatch(MetalKernel::AesDecRounds, (true, expected_dec), || {
            let mut output = vec![[0; 16]; states.len()];
            let completed = metal_aesdec_rounds_batch_into(&states, &round_keys, &mut output);
            (completed, output)
        });
        let signatures: Vec<_> = (0..size)
            .map(|index| {
                if index % 2 == 0 {
                    signature
                } else {
                    bad_signature
                }
            })
            .collect();
        let expected = (0..size).map(|index| index % 2 == 0).collect();
        require_dispatch(MetalKernel::Ed25519, (true, expected), || {
            let mut output = vec![false; size];
            let completed = metal_ed25519_verify_batch_into(
                &signatures,
                &vec![public_key; size],
                &vec![hram; size],
                &mut output,
            );
            (completed, output)
        });
    }
    let before_empty = MetalKernel::ALL.map(metal_completed_dispatches);
    assert_eq!(metal_sha256_leaves(&[]), Some(vec![]));
    assert_eq!(metal_sha256_pairs_reduce(&[]), None);
    assert_eq!(metal_sha256_pairs_reduce(&[digest]), Some(digest));
    assert!(metal_aesenc_batch_into(&[], [0; 16], &mut []));
    assert!(metal_aesdec_batch_into(&[], [0; 16], &mut []));
    assert!(metal_ed25519_verify_batch_into(&[], &[], &[], &mut []));
    assert_eq!(
        MetalKernel::ALL.map(metal_completed_dispatches),
        before_empty,
        "empty and rejected inputs must not report GPU work"
    );
    require_dispatch(MetalKernel::Ed25519, (true, [false]), || {
        let mut output = [true];
        let completed =
            metal_ed25519_verify_batch_into(&[[0; 64]], &[public_key], &[hram], &mut output);
        (completed, output)
    });
    let before_disable = MetalKernel::ALL.map(metal_completed_dispatches);
    set_metal_enabled(false);
    let mut unchanged = initial_sha;
    assert!(!metal_sha256_compress(&mut unchanged, &block));
    assert_eq!(unchanged, initial_sha);
    assert_eq!(
        MetalKernel::ALL.map(metal_completed_dispatches),
        before_disable
    );
}

/// This separate physical gate requires two GPUs; one-device evidence cannot pass it.
#[cfg(target_os = "macos")]
#[test]
fn required_metal_two_devices_isolate_quarantine() {
    const CHILD: &str = "IVM_METAL_TWO_DEVICES_CONTROL";
    if std::env::var(CHILD).as_deref() != Ok("1") {
        let path = module_path!().split_once("::").expect("crate module").1;
        let name = format!("{path}::required_metal_two_devices_isolate_quarantine");
        let output = std::process::Command::new(std::env::current_exe().expect("test executable"))
            .args(["--exact", &name, "--nocapture"])
            .env(CHILD, "1")
            .output()
            .expect("isolated two-device qualification");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        print!("{stdout}{stderr}");
        assert!(
            output.status.success(),
            "two physical GPUs and actual surviving-device dispatch are required"
        );
        assert!(
            stdout.contains("1 passed; 0 failed; 0 ignored"),
            "required control must execute"
        );
        return;
    }
    assert!(
        metal_runtime::device_slots().expect("complete physical inventory") >= 2,
        "two physical Metal devices are required"
    );
    // First establish both qualified identities before destructively quarantining one.
    let first = metal_runtime::with_device_for_qualification(0, || {
        metal_runtime::current_health().unwrap()
    })
    .expect("first physical device");
    let second = metal_runtime::with_device_for_qualification(1, || {
        metal_runtime::current_health().unwrap()
    })
    .expect("second physical device");
    assert_ne!(first.identity(), second.identity());
    let pool = iroha_accel::ProcessResources::get().expect("shared resources");
    let before = pool.usage();
    metal_runtime::with_device_for_qualification(0, || {
        with_metal_state(|state| {
            let buffer =
                MetalBuffer::allocate(&state.device, 17).expect("funded uncertain backing");
            let buffers = [&buffer];
            let mut command = metal_buffers::Command::prepare(&state.queue, &buffers)
                .expect("first device command");
            assert!(command.commit());
            // No completion was observed. Drop preserves original native custody.
            drop(command);
            drop(buffer);
        })
        .expect("first device lease");
    })
    .expect("destructive first-device control executed");
    assert!(!first.usable());
    assert!(second.usable());
    assert_eq!(pool.usage().in_flight[0], before.in_flight[0] + 1);
    assert!(pool.usage().unified_bytes[0] > before.unified_bytes[0]);
    let retained = pool.usage();
    let config = crate::acceleration_config();
    crate::set_acceleration_config(crate::AccelerationConfig {
        max_gpus: Some(0),
        ..config
    });
    assert!(!metal_available());
    crate::set_acceleration_config(crate::AccelerationConfig {
        max_gpus: Some(1),
        ..config
    });
    release_metal_state();
    assert!(
        !first.usable(),
        "policy and discovery cannot revive a quarantined identity"
    );
    // max_gpus=1 excludes the failed owner from the active count, retaining its debt.
    metal_runtime::with_device_for_qualification(1, || {
        require_dispatch(MetalKernel::Add32, Some([5, 5, 5, 5]), || {
            metal_vadd32([1, 2, 3, 4], [4, 3, 2, 1])
        });
    })
    .expect("healthy second device remains eligible at cap one");
    assert_eq!(pool.usage().in_flight[0], retained.in_flight[0]);
    assert_eq!(pool.usage().unified_bytes[0], retained.unified_bytes[0]);
}

/// Enter each direct AES family without a pre-existing physical TLS binding.
#[cfg(target_os = "macos")]
#[test]
fn required_metal_unbound_aes_keeps_exact_owner_through_copy() {
    const CHILD: &str = "IVM_METAL_UNBOUND_AES_CONTROL";
    if std::env::var(CHILD).as_deref() != Ok("1") {
        let path = module_path!().split_once("::").expect("crate module").1;
        let name = format!("{path}::required_metal_unbound_aes_keeps_exact_owner_through_copy");
        let output = std::process::Command::new(std::env::current_exe().expect("test executable"))
            .args(["--exact", &name, "--nocapture"])
            .env(CHILD, "1")
            .output()
            .expect("isolated direct AES qualification");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        print!("{stdout}{stderr}");
        assert!(
            output.status.success(),
            "unbound direct AES APIs must publish native output"
        );
        assert!(
            stdout.contains("1 passed; 0 failed; 0 ignored"),
            "required control must execute"
        );
        return;
    }
    assert!(metal_runtime::current_health().is_none());
    let health =
        with_metal_state(|_| metal_runtime::current_health().expect("qualified physical health"))
            .expect("physical Metal device required");
    assert!(
        metal_runtime::current_health().is_none(),
        "control must not carry an outer TLS pin"
    );
    let states: Vec<[u8; 16]> = (0..128)
        .map(|index| std::array::from_fn(|byte| (index + byte * 17) as u8))
        .collect();
    let keys = [[0x39; 16], [0xA7; 16]];
    for (decrypt, fused, kernel) in [
        (false, false, MetalKernel::AesEncBatch),
        (true, false, MetalKernel::AesDecBatch),
        (false, true, MetalKernel::AesEncRounds),
        (true, true, MetalKernel::AesDecRounds),
    ] {
        let rounds = &keys[..if fused { 2 } else { 1 }];
        let expected: Vec<_> = states
            .iter()
            .copied()
            .map(|state| {
                rounds.iter().copied().fold(state, |block, key| {
                    if decrypt {
                        crate::aes::aesdec(block, key)
                    } else {
                        crate::aes::aesenc(block, key)
                    }
                })
            })
            .collect();
        let before = health.completions(kernel as usize);
        let mut output = vec![[0xEF; 16]; states.len()];
        let accepted = match (decrypt, fused) {
            (false, false) => metal_aesenc_batch_into(&states, keys[0], &mut output),
            (true, false) => metal_aesdec_batch_into(&states, keys[0], &mut output),
            (false, true) => metal_aesenc_rounds_batch_into(&states, rounds, &mut output),
            (true, true) => metal_aesdec_rounds_batch_into(&states, rounds, &mut output),
        };
        assert!(
            accepted,
            "{kernel:?}: completed unbound into call must accept output"
        );
        assert_eq!(output, expected);
        assert!(metal_runtime::current_health().is_none());
        assert!(
            health.completions(kernel as usize) > before,
            "{kernel:?}: exact physical receipt required"
        );
        let before = health.completions(kernel as usize);
        let mut in_place = states.clone();
        assert!(
            metal_aes_batch_in_place(&mut in_place, rounds, decrypt, fused),
            "{kernel:?}: unbound in-place call must accept output"
        );
        assert_eq!(in_place, expected);
        assert!(metal_runtime::current_health().is_none());
        assert!(
            health.completions(kernel as usize) > before,
            "{kernel:?}: in-place physical receipt required"
        );
        println!(
            "IVM_METAL_UNBOUND_AES_RECEIPT device={} kernel={kernel:?} into_and_in_place=true",
            health.identity()
        );
    }
}
