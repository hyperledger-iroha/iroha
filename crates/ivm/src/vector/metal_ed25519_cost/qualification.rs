//! Required physical Items-path parity and original-owner synthetic receipts.

use super::*;
use crate::{
    signature::Ed25519BatchItem,
    vector::{MetalKernel, metal_receipts, metal_runtime, metal_signature},
};

const KERNEL: usize = MetalKernel::Ed25519 as usize;
const MIXED: [usize; 16] = [
    0, 1, 47, 48, 63, 64, 111, 112, 127, 128, 175, 176, 255, 256, 4096, 65_536,
];

fn measure_geometry(lengths: [usize; 16]) {
    let health = metal_runtime::current_health().expect("original physical owner");
    let source = [0xcc; 65_536];
    let caller: Vec<_> = lengths
        .iter()
        .map(|&len| Ed25519BatchItem {
            message: &source[..len],
            ..Default::default()
        })
        .collect();
    let geometry = MessageGeometry::new(&caller).unwrap();
    let production = health.completions(KERNEL);
    let synthetic = health.synthetic_completions(KERNEL);
    let profile = calibrate(geometry, Instant::now())
        .expect("bounded actual Items calibration must complete on the required physical runner");
    assert!(profile.geometry.matches(geometry));
    assert_eq!(health.completions(KERNEL), production);
    assert_eq!(
        health.synthetic_completions(KERNEL),
        synthetic + TRIALS as u64
    );
    sample::with_inputs(geometry, Instant::now(), |inputs, expected, actual| {
        assert!(metal_signature::metal_ed25519_items_into(inputs, actual));
        assert_eq!(actual, expected);
        assert_eq!(health.completions(KERNEL), production + 1);
        let before = health.synthetic_completions(KERNEL);
        assert!(metal_receipts::with_synthetic(|| {
            metal_signature::metal_ed25519_items_into(inputs, actual)
        }));
        assert_eq!(actual, expected);
        assert_eq!(health.completions(KERNEL), production + 1);
        assert_eq!(health.synthetic_completions(KERNEL), before + 1);
        Ok(())
    })
    .unwrap();
    // Put the genuinely measured mixed-length profile in its original physical
    // state. A measured CPU winner remains an honest automatic CPU selection.
    if lengths == MIXED {
        let use_gpu = profile.cost().is_some();
        super::super::with_metal_state_try(|state| {
            let mut cache = state.ed25519_cost.try_lock().ok()?;
            let now = Instant::now();
            assert_eq!(
                cache
                    .qualified_cost(now, geometry, || Some(now), |_| Ok(profile))
                    .is_some(),
                use_gpu
            );
            Some(())
        })
        .expect("measured profile retains its original physical owner");
        sample::with_inputs(geometry, Instant::now(), |inputs, expected, actual| {
            let before = health.completions(KERNEL);
            let probes = health.synthetic_completions(KERNEL);
            let enabled = crate::acceleration_config();
            for opt_out in [true, false] {
                let selected = metal_runtime::current_selection().expect("original owner");
                let mut disabled = enabled;
                if opt_out {
                    disabled.enable_metal = false;
                } else {
                    disabled.max_gpus = Some(0);
                }
                crate::set_acceleration_config(disabled);
                assert!(
                    selected
                        .run(|| panic!("policy changed after selection"))
                        .is_none()
                );
                actual.fill(true);
                assert!(!metal_runtime::metal_ed25519_auto_into(inputs, actual));
                assert!(actual.iter().all(|&value| value));
                assert_eq!(health.completions(KERNEL), before);
                assert_eq!(health.synthetic_completions(KERNEL), probes);
                crate::set_acceleration_config(enabled);
                let same = metal_runtime::current_health().expect("restored original owner");
                assert!(iroha_allocation::ChargedShared::ptr_eq(&health, &same));
            }
            assert!(crate::signature::verify_ed25519_batch_items_into(
                inputs, actual
            ));
            assert_eq!(actual, expected);
            assert_eq!(health.completions(KERNEL), before + u64::from(use_gpu));
            assert_eq!(health.synthetic_completions(KERNEL), probes);
            Ok(())
        })
        .unwrap();
    }
    println!(
        "IVM_METAL_ED25519_COST_RECEIPT device={} items={} message_bytes={} production_batches={} synthetic_batches={}",
        health.identity(),
        geometry.len(),
        geometry.total_bytes(),
        health.completions(KERNEL) - production,
        health.synthetic_completions(KERNEL) - synthetic,
    );
}

#[test]
fn required_metal_ed25519_exact_geometry_measures_real_items_without_production_credit() {
    const CHILD: &str = "IVM_METAL_ED25519_EXACT_GEOMETRY";
    if std::env::var(CHILD).as_deref() != Ok("1") {
        let path = module_path!().split_once("::").unwrap().1;
        let name = format!(
            "{path}::required_metal_ed25519_exact_geometry_measures_real_items_without_production_credit"
        );
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &name, "--nocapture"])
            .env(CHILD, "1")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        print!("{stdout}{stderr}");
        assert!(
            output.status.success(),
            "exact-geometry physical Ed25519 qualification failed"
        );
        assert!(
            stdout.contains("1 passed; 0 failed; 0 ignored"),
            "physical test must execute"
        );
        return;
    }
    crate::set_acceleration_config(crate::AccelerationConfig {
        enable_metal: true,
        enable_cuda: false,
        ..Default::default()
    });
    let slots = metal_runtime::device_slots().expect("complete physical Metal inventory");
    assert!(slots > 0, "physical Metal device is required");
    for slot in 0..slots {
        metal_runtime::with_device_for_qualification(slot, || {
            for lengths in [[0; 16], [32; 16], MIXED] {
                measure_geometry(lengths);
            }
        })
        .expect("every observed physical device must qualify");
    }
}
