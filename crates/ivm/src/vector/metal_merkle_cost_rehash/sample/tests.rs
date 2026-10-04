//! Original process admission precedes both fixed retained sample allocations.

use super::*;

#[test]
fn retained_samples_admit_both_trees_and_keep_credit_through_updates_and_unwind() {
    const CHILD: &str = "IVM_MERKLE_REHASH_SAMPLE_CUSTODY";
    if std::env::var(CHILD).as_deref() != Ok("1") {
        let module = module_path!().split_once("::").unwrap().1;
        let test = format!(
            "{module}::retained_samples_admit_both_trees_and_keep_credit_through_updates_and_unwind"
        );
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &test, "--nocapture"])
            .env(CHILD, "1")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stdout}\n{stderr}");
        assert!(stdout.contains("1 passed; 0 failed; 0 ignored"));
        return;
    }
    let geometry = Geometry::new(8_193 * 17 - 7, 17, 8_193).unwrap();
    let tree_bytes = ByteMerkleTree::memory_plan(geometry.leaves)
        .unwrap()
        .requested_bytes();
    let mut config = crate::AccelerationConfig {
        enable_simd: false,
        enable_cuda: false,
        enable_metal: false,
        ..Default::default()
    };
    config.resource_limits.work.host_bytes = 0;
    crate::set_acceleration_config(config);
    let owner = ProcessResources::get().unwrap();
    let context = Sha256Context::production().synthetic();
    assert_eq!(
        with_inputs(geometry, Instant::now(), |_, _, _| -> Result<(), Failure> {
            panic!("unfunded callback")
        }),
        Err(Failure::Allocation)
    );
    config.resource_limits.work.host_bytes = geometry.byte_len + 2 * tree_bytes - 1;
    crate::set_acceleration_config(config);
    assert_eq!(
        with_inputs(geometry, Instant::now(), |_, _, _| -> Result<(), Failure> {
            panic!("both outputs must be funded before allocation")
        }),
        Err(Failure::Allocation)
    );
    assert_eq!(owner.usage().host_bytes[0], 0);
    config.resource_limits.work.host_bytes += 1;
    crate::set_acceleration_config(config);
    with_inputs(geometry, Instant::now(), |input, cpu, metal| {
        assert_eq!(
            owner.usage().host_bytes[0],
            geometry.byte_len + 2 * tree_bytes
        );
        for byte in [0, 0x63] {
            input.fill(byte);
            assert_eq!(
                cpu.rehash_parallel_in_context(input, context),
                metal.rehash_parallel_in_context(input, context)
            );
            assert_eq!(cpu.root(), metal.root());
        }
        config.resource_limits.work.host_bytes = 0;
        crate::set_acceleration_config(config);
        assert_eq!(
            owner.usage().host_bytes[0],
            geometry.byte_len + 2 * tree_bytes
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(owner.usage().host_bytes[0], 0);
    config.resource_limits.work.host_bytes = geometry.byte_len + 2 * tree_bytes;
    crate::set_acceleration_config(config);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = with_inputs(
                geometry,
                Instant::now(),
                |input, cpu, _| -> Result<(), Failure> {
                    cpu.rehash_parallel_in_context(input, context);
                    assert_eq!(
                        owner.usage().host_bytes[0],
                        geometry.byte_len + 2 * tree_bytes
                    );
                    panic!("unwind retains original owners until physical sample drop");
                },
            );
        }))
        .is_err()
    );
    assert_eq!(owner.usage().host_bytes[0], 0);
}

#[cfg(feature = "metal-hardware-tests")]
#[test]
fn expired_calibration_reports_original_empty_arrays_without_starting_sample_work() {
    let previous = crate::vector::set_thread_forced_simd(Some(crate::vector::SimdChoice::Scalar));
    let context = Sha256Context::production();
    let baseline = Sha256Baseline::capture(context).unwrap();
    crate::vector::set_thread_forced_simd(previous);
    let geometry = Geometry::new(8_192 * 32, 32, 8_192).unwrap();
    let expired = Instant::now().checked_sub(MAX_CALIBRATION).unwrap();
    let (result, timings) = crate::vector::metal_receipts::timing::observe(|| {
        calibrate(geometry, baseline, context, expired)
    });
    assert!(matches!(result, Err(Failure::Deadline)));
    let timings = timings.expect("refused calibration still publishes original arrays");
    assert_eq!(timings.cpu_ns, [[0; TRIALS]; 2]);
    assert_eq!(timings.metal_ns, [[0; TRIALS]; 2]);
}
