//! Both returned sample trees keep original process credit beyond timing.

use super::*;

#[test]
fn tree_sample_admits_both_backings_before_work_and_retains_credit_until_drop() {
    const CHILD: &str = "IVM_MERKLE_TREE_SAMPLE_CUSTODY";
    if std::env::var(CHILD).as_deref() != Ok("1") {
        let module = module_path!().split_once("::").unwrap().1;
        let test = format!(
            "{module}::tree_sample_admits_both_backings_before_work_and_retains_credit_until_drop"
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
    let geometry = Geometry::new(8_193 * 17 - 7, 17).unwrap();
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
    let context = Sha256Context::production().synthetic();
    let owner = ProcessResources::get().unwrap();
    assert_eq!(
        with_inputs(geometry, Instant::now(), |_, _| -> Result<(), Failure> {
            panic!("unfunded input")
        }),
        Err(Failure::Allocation)
    );
    config.resource_limits.work.host_bytes = geometry.byte_len + tree_bytes * 2 - 1;
    crate::set_acceleration_config(config);
    assert_eq!(
        with_inputs(geometry, Instant::now(), |original, _| {
            assert!(std::ptr::eq(original, owner));
            let _first = SampleTree::reserve(owner, geometry)?;
            SampleTree::reserve(owner, geometry).map(|_| ())
        }),
        Err(Failure::Allocation)
    );
    assert_eq!(owner.usage().host_bytes[0], 0);
    config.resource_limits.work.host_bytes += 1;
    crate::set_acceleration_config(config);
    with_inputs(geometry, Instant::now(), |owner, input| {
        let first = SampleTree::reserve(owner, geometry)?;
        let second = SampleTree::reserve(owner, geometry)?;
        input.fill(0x53);
        let build = || {
            ByteMerkleTree::from_bytes_parallel_in_context(input, geometry.chunk, context)
                .map_err(|_| Failure::Allocation)
        };
        let (left, left_observed, left_ns) = SampleTree::measure(first, build)?;
        let (right, right_observed, right_ns) = SampleTree::measure(second, build)?;
        assert!(left_ns > 0 && right_ns > 0);
        assert_eq!(left.tree.root(), right.tree.root());
        assert_eq!(left_observed, right_observed);
        assert_eq!(
            owner.usage().host_bytes[0],
            geometry.byte_len + 2 * tree_bytes
        );
        config.resource_limits.work.host_bytes = 0;
        crate::set_acceleration_config(config);
        assert_eq!(
            owner.usage().host_bytes[0],
            geometry.byte_len + 2 * tree_bytes
        );
        drop(left);
        assert_eq!(owner.usage().host_bytes[0], geometry.byte_len + tree_bytes);
        drop(right);
        assert_eq!(owner.usage().host_bytes[0], geometry.byte_len);
        Ok(())
    })
    .unwrap();
    assert_eq!(owner.usage().host_bytes[0], 0);
    config.resource_limits.work.host_bytes = geometry.byte_len + tree_bytes;
    crate::set_acceleration_config(config);
    assert!(
        std::panic::catch_unwind(|| {
            let _ = with_inputs(
                geometry,
                Instant::now(),
                |owner, input| -> Result<(), Failure> {
                    let credit = SampleTree::reserve(owner, geometry)?;
                    let (_tree, _, _) = SampleTree::measure(credit, || {
                        ByteMerkleTree::from_bytes_parallel_in_context(
                            input,
                            geometry.chunk,
                            context,
                        )
                        .map_err(|_| Failure::Allocation)
                    })?;
                    panic!("unwind with live output and original credit");
                },
            );
        })
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
    let geometry = Geometry::new(8_192 * 32, 32).unwrap();
    let expired = Instant::now().checked_sub(MAX_CALIBRATION).unwrap();
    let (result, timings) = crate::vector::metal_receipts::timing::observe(|| {
        calibrate(geometry, baseline, context, expired)
    });
    assert!(matches!(result, Err(Failure::Deadline)));
    let timings = timings.expect("refused calibration still publishes original arrays");
    assert_eq!(timings.cpu_ns, [[0; TRIALS]; 2]);
    assert_eq!(timings.metal_ns, [[0; TRIALS]; 2]);
}
