//! Required fixed-memory Rehash on each original physical Metal owner.

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
fn required_metal_rehash_covers_actual_memory_bounds_and_original_fixed_outputs() {
    const CHILD: &str = "IVM_METAL_REHASH_EXACT_GEOMETRY";
    if std::env::var(CHILD).as_deref() != Ok("1") {
        let module = module_path!().split_once("::").unwrap().1;
        let test = format!(
            "{module}::required_metal_rehash_covers_actual_memory_bounds_and_original_fixed_outputs"
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
            "physical Rehash qualification failed"
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
    let minimum_bytes =
        crate::Memory::image_bytes_for_stack_limit(crate::Memory::MIN_STACK_SIZE).unwrap();
    let maximum_bytes =
        crate::Memory::image_bytes_for_stack_limit(crate::Memory::STACK_SIZE).unwrap();
    for slot in 0..slots {
        metal_runtime::with_device_for_qualification(slot, || {
            let health = metal_runtime::current_health().expect("original physical owner");
            for (bytes, chunk, leaves) in [
                (minimum_bytes, 32, minimum_bytes.div_ceil(32)),
                (maximum_bytes, 32, maximum_bytes.div_ceil(32)),
                (8_193 * 17 - 7, 17, 8_195),
            ] {
                let geometry = Geometry::new(bytes, chunk, leaves).unwrap();
                let production = counts(&health, false);
                let synthetic = counts(&health, true);
                let profile = calibrate(geometry, baseline, context, Instant::now())
                    .expect("bounded actual retained-update calibration on required runner");
                assert_eq!(profile.geometry, geometry);
                assert_eq!(profile.baseline, baseline);
                assert_eq!(counts(&health, false), production);
                assert_eq!(counts(&health, true), [synthetic[0] + 2 * TRIALS as u64, synthetic[1]]);
                sample::with_inputs(geometry, Instant::now(), |input, cpu, native| {
                    input.fill(0x63);
                    cpu.rehash_parallel_in_context(input, context);
                    let expected = cpu.root();
                    assert!(metal_merkle::rehash_tree(native, input, baseline, context));
                    assert_eq!(native.root(), expected);
                    for leaf in [0, geometry.leaves / 2, geometry.leaves - 1] {
                        assert_eq!(native.proof(leaf).unwrap(), cpu.proof(leaf).unwrap());
                    }
                    assert_eq!(counts(&health, false), [production[0] + 1, production[1]]);
                    if bytes == minimum_bytes {
                        super::super::with_metal_state_try(|state| {
                            let mut cache = state.merkle_rehash_cost.try_lock().ok()?;
                            assert!(cache.profile.is_none());
                            let now = Instant::now();
                            assert_eq!(cache.qualified_cost(now, geometry, baseline, context, || Some(now), |_| Ok(profile)), profile.cost());
                            Some(())
                        }).expect("first actual profile belongs to original physical owner");
                        // The minimum memory image has more read-only/reserved
                        // space than its writable heap. Use real host code/input
                        // initialization plus a permitted heap write to cross
                        // half the retained leaves without changing dirty bits.
                        input.fill(0);
                        let start = crate::Memory::HEAP_START as usize;
                        let end = start + crate::Memory::HEAP_MAX_SIZE as usize;
                        let code_end = start / 2;
                        let input_end = end + crate::Memory::INPUT_SIZE as usize;
                        let instruction = crate::encoding::wide::encode_ri(
                            crate::instruction::wide::arithmetic::ADDI, 4, 0, 1,
                        ).to_le_bytes();
                        for word in input[..code_end].chunks_exact_mut(4) {
                            word.copy_from_slice(&instruction);
                        }
                        input[start..end].fill(0x53);
                        input[end..input_end].fill(0x27);
                        cpu.rehash_parallel_in_context(input, context);
                        let mut memory = crate::Memory::new_with_stack_limit(crate::Memory::MIN_STACK_SIZE).unwrap();
                        memory.load_code(&input[..code_end]).unwrap();
                        memory.store_bytes(crate::Memory::HEAP_START, &input[start..end]).unwrap();
                        memory.preload_input(0, &input[end..input_end]).unwrap();
                        let dirty = memory.dirty_ranges();
                        assert_eq!(dirty, [(0, code_end), (start, input_end)]);
                        let dirty_leaves: usize = dirty.iter().map(|(start, end)| (end - start) / 32).sum();
                        assert!(dirty_leaves >= geometry.leaves.div_ceil(2));
                        let before = counts(&health, false);
                        let probes = counts(&health, true);
                        assert_eq!(memory.root(), cpu.root_hash());
                        assert_eq!(counts(&health, false), [before[0] + u64::from(profile.cost().is_some()), before[1]]);
                        assert_eq!(counts(&health, true), probes);
                    }
                    let enabled = crate::acceleration_config();
                    let mut refused = enabled;
                    refused.resource_limits.work.host_bytes = 0;
                    crate::set_acceleration_config(refused);
                    let before = counts(&health, false);
                    let unchanged = native.root();
                    assert!(!metal_merkle::rehash_tree(native, input, baseline, context));
                    assert_eq!(native.root(), unchanged);
                    assert!(!native.recompute_all_leaves_accel(input));
                    native.recompute_all_leaves_parallel(input);
                    assert_eq!(native.root(), cpu.root());
                    assert_eq!(counts(&health, false), before);
                    crate::set_acceleration_config(enabled);
                    Ok(())
                }).unwrap();
                println!("IVM_METAL_REHASH_EXACT_RECEIPT device={} bytes={} chunk={} leaves={} production_leaf={} synthetic_leaf={}", health.identity(), bytes, chunk, leaves, counts(&health, false)[0] - production[0], counts(&health, true)[0] - synthetic[0]);
            }
        }).expect("every original physical device must execute");
    }
}
