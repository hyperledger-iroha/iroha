//! Ensure Metal-gated helpers degrade to scalar paths on platforms or builds
//! without `feature = "metal"`. Guards the optional backend for non-macOS and
//! metal-disabled configurations.

#[test]
fn metal_helpers_report_unavailable_when_disabled() {
    // `metal_available` is compile-time false when the feature or platform is missing.
    assert!(
        !ivm::metal_available(),
        "metal must report unavailable when the feature/platform is missing"
    );

    // The policy/runtime helpers must report disabled/unavailable on non-Metal builds.
    assert!(
        !ivm::metal_disabled(),
        "metal_disabled stays false in non-Metal builds (availability governs usage)"
    );
}
