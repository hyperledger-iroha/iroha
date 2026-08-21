#[test]
fn native_amx_signing_guard_capacity_preserves_exact_hard_boundary() {
    let capacity = native_amx_signing_guard_capacity(limits_with_native_capacity(
        MAX_NATIVE_AMX_SIGNING_GUARD_RECORDS_HARD,
    ))
    .expect("exact protocol boundary");
    assert_eq!(capacity.get(), MAX_NATIVE_AMX_SIGNING_GUARD_RECORDS_HARD);
}

#[test]
fn native_amx_signing_guard_capacity_preserves_explicit_runtime_bound() {
    let capacity = native_amx_signing_guard_capacity(limits_with_native_capacity(128))
        .expect("explicit capacity");
    assert_eq!(capacity.get(), 128);
}
