//! Real synthetic byte and canonical tree allocations use the original process host pool.

use super::*;

#[test]
fn root_samples_reserve_before_construction_and_refund_on_refusal_shrink_and_unwind() {
    const CHILD: &str = "IVM_MERKLE_ROOT_SAMPLE_CUSTODY";
    if std::env::var(CHILD).as_deref() != Ok("1") {
        let module = module_path!().split_once("::").unwrap().1;
        let test = format!(
            "{module}::root_samples_reserve_before_construction_and_refund_on_refusal_shrink_and_unwind"
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
    let node_bytes =
        MerkleTree::<[u8; 32]>::repeated_sha256_node_allocation_bytes(geometry.leaves).unwrap();
    let mut config = crate::AccelerationConfig {
        enable_cuda: false,
        enable_metal: false,
        ..Default::default()
    };
    config.resource_limits.work.host_bytes = 0;
    crate::set_acceleration_config(config);
    let owner = ProcessResources::get().unwrap();
    assert_eq!(
        with_inputs(geometry, Instant::now(), |_, _| -> Result<(), Failure> {
            panic!("no unfunded inputs")
        }),
        Err(Failure::Allocation)
    );
    assert_eq!(owner.usage().host_bytes[0], 0);
    config.resource_limits.work.host_bytes = geometry.byte_len + node_bytes - 1;
    crate::set_acceleration_config(config);
    assert_eq!(
        with_inputs(geometry, Instant::now(), |owner, input| {
            assert!(input.iter().all(|&byte| byte == 0));
            assert_eq!(owner.usage().host_bytes[0], geometry.byte_len);
            CpuTree::reserve(owner, geometry).map(|_| ())
        }),
        Err(Failure::Allocation)
    );
    assert_eq!(owner.usage().host_bytes[0], 0);
    config.resource_limits.work.host_bytes += 1;
    crate::set_acceleration_config(config);
    with_inputs(geometry, Instant::now(), |original, input| {
        assert!(std::ptr::eq(owner, original));
        let credit = CpuTree::reserve(owner, geometry)?;
        let tree = CpuTree::new(input, geometry, credit)?;
        assert_eq!(tree.tree.allocated_bytes(), node_bytes);
        assert_eq!(owner.usage().host_bytes[0], geometry.byte_len + node_bytes);
        config.resource_limits.work.host_bytes = 0;
        crate::set_acceleration_config(config);
        assert_eq!(owner.usage().host_bytes[0], geometry.byte_len + node_bytes);
        let expected = *tree.tree.root().unwrap().as_ref();
        let (actual, _) = tree.finish(Instant::now())?;
        assert_eq!(actual, expected);
        assert_eq!(owner.usage().host_bytes[0], geometry.byte_len);
        Ok(())
    })
    .unwrap();
    assert_eq!(owner.usage().host_bytes[0], 0);
    config.resource_limits.work.host_bytes = geometry.byte_len + node_bytes;
    crate::set_acceleration_config(config);
    assert!(
        std::panic::catch_unwind(|| {
            let _ = with_inputs(
                geometry,
                Instant::now(),
                |owner, input| -> Result<(), Failure> {
                    let credit = CpuTree::reserve(owner, geometry)?;
                    let _tree = CpuTree::new(input, geometry, credit)?;
                    panic!("unwind with both original physical owners");
                },
            );
        })
        .is_err()
    );
    assert_eq!(owner.usage().host_bytes[0], 0);
    assert_eq!(
        with_inputs(
            geometry,
            Instant::now() - MAX_CALIBRATION,
            |_, _| -> Result<(), Failure> { panic!("expired") }
        ),
        Err(Failure::Deadline)
    );
    assert_eq!(owner.usage().host_bytes[0], 0);
}
