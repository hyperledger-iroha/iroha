//! Real synthetic construction preserves original pool admission and refunds.

use super::*;

#[test]
fn synthetic_inputs_are_prepaid_length_only_and_refund_after_failure_or_unwind() {
    const CHILD: &str = "IVM_ED25519_COST_ALLOCATION";
    if std::env::var(CHILD).as_deref() != Ok("1") {
        let path = module_path!().split_once("::").unwrap().1;
        let name = format!(
            "{path}::synthetic_inputs_are_prepaid_length_only_and_refund_after_failure_or_unwind"
        );
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &name, "--nocapture"])
            .env(CHILD, "1")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stdout}\n{stderr}");
        assert!(stdout.contains("1 passed; 0 failed; 0 ignored"), "{stdout}");
        return;
    }
    let bytes = [0xcc; 65_536];
    let lengths = [
        0, 1, 47, 48, 63, 64, 111, 112, 127, 128, 175, 176, 255, 256, 4096, 65_536,
    ];
    let caller: Vec<_> = lengths
        .iter()
        .map(|&len| Ed25519BatchItem {
            message: &bytes[..len],
            signature: [0xe7; 64],
            public_key: [0xe8; 32],
        })
        .collect();
    let geometry = MessageGeometry::new(&caller).unwrap();
    let mut config = crate::AccelerationConfig {
        enable_metal: false,
        enable_cuda: false,
        ..Default::default()
    };
    config.resource_limits.work.host_bytes = 0;
    crate::set_acceleration_config(config);
    let owner = ProcessResources::get().unwrap();
    assert_eq!(
        with_inputs(geometry, Instant::now(), |_, _, _| -> Result<(), Failure> {
            panic!("unfunded construction")
        }),
        Err(Failure::Allocation)
    );
    assert_eq!(owner.usage().host_bytes[0], 0);
    let exact = geometry.total_bytes()
        + geometry.len()
            * (std::mem::size_of::<Ed25519BatchItem<'_>>() + 2 * std::mem::size_of::<bool>());
    // Admit messages and views but refuse a later result allocation; all prior
    // owners must refund without entering the operation callback.
    config.resource_limits.work.host_bytes = exact - 1;
    crate::set_acceleration_config(config);
    assert_eq!(
        with_inputs(geometry, Instant::now(), |_, _, _| -> Result<(), Failure> {
            panic!("partly funded construction")
        }),
        Err(Failure::Allocation)
    );
    assert_eq!(owner.usage().host_bytes[0], 0);
    config.resource_limits.work.host_bytes = exact;
    crate::set_acceleration_config(config);
    assert!(std::ptr::eq(owner, ProcessResources::get().unwrap()));
    with_inputs(geometry, Instant::now(), |inputs, expected, actual| {
        assert_eq!(owner.usage().host_bytes[0], exact);
        assert_eq!(inputs.len(), caller.len());
        for (index, item) in inputs.iter().enumerate() {
            assert_eq!(item.message.len(), lengths[index]);
            assert_ne!(item.signature, caller[index].signature);
            assert_ne!(item.public_key, caller[index].public_key);
            if !item.message.is_empty() {
                assert_ne!(item.message.as_ptr(), caller[index].message.as_ptr());
            }
        }
        crate::signature::cpu_batch_into(inputs, actual);
        assert_eq!(actual, expected);
        Err::<(), _>(Failure::Deadline)
    })
    .unwrap_err();
    assert_eq!(owner.usage().host_bytes[0], 0);
    assert!(
        std::panic::catch_unwind(|| with_inputs(
            geometry,
            Instant::now(),
            |_, _, _| -> Result<(), Failure> { panic!("synthetic operation failed") }
        ))
        .is_err()
    );
    assert_eq!(owner.usage().host_bytes[0], 0);
    assert_eq!(
        with_inputs(
            geometry,
            Instant::now() - MAX_CALIBRATION,
            |_, _, _| -> Result<(), Failure> { panic!("expired construction") }
        ),
        Err(Failure::Deadline)
    );
    assert_eq!(owner.usage().host_bytes[0], 0);
    assert_eq!(bytes, [0xcc; 65_536]);
}
