// Empty-input coverage for deterministic log export.
#[test]
fn collect_log_text_rejects_empty() {
    let entries: Vec<(usize, String)> = Vec::new();
    assert!(
        super::collect_log_text(&entries).is_err(),
        "export should fail when no logs are available"
    );
}
