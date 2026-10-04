//! Required complete tree construction on each original physical Metal owner.

use super::*;
use crate::vector::{MetalKernel, metal_merkle, metal_owner::HealthLease, metal_runtime};

fn counts(health: &HealthLease, synthetic: bool) -> [u64; 2] {
    [MetalKernel::Sha256Leaves, MetalKernel::Sha256Pairs].map(|kernel| {
        if synthetic {
            health.synthetic_completions(kernel as usize)
        } else {
            health.completions(kernel as usize)
        }
    })
}

#[test]
fn required_metal_tree_exact_geometry_retains_outputs_and_separates_synthetic_receipts() {
    const CHILD: &str = "IVM_METAL_TREE_EXACT_GEOMETRY";
    if std::env::var(CHILD).as_deref() != Ok("1") {
        let module = module_path!().split_once("::").unwrap().1;
        let test = format!(
            "{module}::required_metal_tree_exact_geometry_retains_outputs_and_separates_synthetic_receipts"
        );
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &test, "--nocapture"])
            .env(CHILD, "1")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        print!("{stdout}{stderr}");
        assert!(
            output.status.success(),
            "physical whole-tree qualification failed"
        );
        assert!(stdout.contains("1 passed; 0 failed; 0 ignored"));
        return;
    }
    crate::set_acceleration_config(crate::AccelerationConfig {
        enable_simd: true,
        enable_metal: true,
        enable_cuda: false,
        merkle_min_leaves_gpu: Some(0),
        merkle_min_leaves_metal: Some(0),
        ..Default::default()
    });
    let context = Sha256Context::production();
    let baseline = Sha256Baseline::capture(context).expect("original qualified CPU baseline");
    let slots = metal_runtime::device_slots().expect("complete physical Metal inventory");
    assert!(slots > 0, "physical Metal device is required");
    for slot in 0..slots {
        metal_runtime::with_device_for_qualification(slot, || {
            let health = metal_runtime::current_health().expect("original physical owner");
            for (bytes, chunk) in [(8_193 * 17 - 7, 17), (8_193 * 32 - 1, 32)] {
                let geometry = Geometry::new(bytes, chunk).unwrap();
                let production = counts(&health, false);
                let synthetic = counts(&health, true);
                let profile = super::super::metal_receipts::timing::report(
                    "BuildTree",
                    health.identity(),
                    geometry,
                    baseline,
                    || calibrate(geometry, baseline, context, Instant::now()),
                )
                .expect("bounded actual whole-tree calibration on required runner");
                assert_eq!(profile.geometry, geometry);
                assert_eq!(profile.baseline, baseline);
                assert_eq!(counts(&health, false), production);
                assert_eq!(counts(&health, true), [synthetic[0] + 2 * TRIALS as u64, synthetic[1]]);
                sample::with_inputs(geometry, Instant::now(), |_, input| {
                    input.fill(0x63);
                    let expected = crate::ByteMerkleTree::from_bytes_parallel(input, chunk).unwrap();
                    let tree = metal_merkle::tree_from_bytes(input, chunk).expect("required actual complete Metal tree");
                    assert_eq!(tree.root(), expected.root());
                    for leaf in [0, geometry.leaves / 2, geometry.leaves - 1] {
                        assert_eq!(tree.proof(leaf).unwrap(), expected.proof(leaf).unwrap());
                    }
                    assert_eq!(counts(&health, false), [production[0] + 1, production[1]]);
                    if chunk == 17 {
                        super::super::with_metal_state_try(|state| {
                            let mut cache = state.merkle_tree_cost.try_lock().ok()?;
                            assert!(cache.profile.is_none());
                            let now = Instant::now();
                            assert_eq!(cache.qualified_cost(now, geometry, baseline, context, || Some(now), |_| Ok(profile)), profile.cost());
                            Some(())
                        }).expect("measured profile belongs to original physical owner");
                        let before = counts(&health, false);
                        let probes = counts(&health, true);
                        assert_eq!(crate::ByteMerkleTree::from_bytes_accel(input, chunk).unwrap().root(), expected.root());
                        assert_eq!(counts(&health, false), [before[0] + u64::from(profile.cost().is_some()), before[1]]);
                        assert_eq!(counts(&health, true), probes);
                    }
                    let enabled = crate::acceleration_config();
                    let mut refused = enabled;
                    refused.resource_limits.work.host_bytes = 0;
                    crate::set_acceleration_config(refused);
                    let before = counts(&health, false);
                    assert!(metal_merkle::tree_from_bytes(input, chunk).is_none());
                    assert_eq!(crate::ByteMerkleTree::from_bytes_accel(input, chunk).unwrap().root(), expected.root());
                    assert_eq!(counts(&health, false), before);
                    crate::set_acceleration_config(enabled);
                    assert!(input.iter().all(|&byte| byte == 0x63));
                    Ok(())
                }).unwrap();
                println!("IVM_METAL_TREE_EXACT_RECEIPT device={} bytes={} chunk={} production_leaf={} synthetic_leaf={}", health.identity(), bytes, chunk, counts(&health, false)[0] - production[0], counts(&health, true)[0] - synthetic[0]);
            }
        }).expect("every original physical device must execute");
    }
}
