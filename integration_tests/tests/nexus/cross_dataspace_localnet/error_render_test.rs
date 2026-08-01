// Display/debug composition coverage for cross-dataspace probe errors.

#[test]
fn render_error_with_debug_keeps_display_and_debug_context() {
    assert_eq!(
        render_error_with_debug(&DisplayOnlyTxError),
        "route probe failed (DisplayOnlyTxError)"
    );
}
