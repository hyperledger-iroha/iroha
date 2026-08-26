#[test]
fn address_audit_supports_csv_output() {
    use torii_mock_support::TempDir;
    let account = account_id_for_domain("atlas", 0xF7);
    let i105 = encode_account_id_to_i105_for_discriminant(&account, 753).expect("i105");
    let temp_dir = TempDir::new("address_audit_csv").expect("temp dir");
    let path = temp_dir.path().join("addresses.txt");
    fs::write(&path, format!("{i105}\ninvalid-address\n")).expect("write addresses");
    let output = command()
        .current_dir(workspace_root())
        .args([
            "--config",
            "defaults/client.toml",
            "tools",
            "address",
            "audit",
            "--input",
            path.to_str().expect("utf8 path"),
            "--network-prefix",
            "753",
            "--allow-errors",
            "--format",
            "csv",
        ])
        .output()
        .expect("run address audit csv");
    assert!(
        output.status.success(),
        "audit exited with {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let csv_stream = if stdout.contains("input,status,format,i105,canonical_hex") {
        stdout.as_ref()
    } else {
        stderr.as_ref()
    };
    let mut lines = csv_stream
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with("CLI started"));
    assert_eq!(
        lines.next(),
        Some("input,status,format,i105,canonical_hex,error_code,error_message")
    );
    assert!(
        !csv_stream.contains("domain_kind"),
        "first-release CSV must not expose retired selector metadata"
    );
    let rows: Vec<&str> = lines.collect();
    assert_eq!(rows.len(), 2, "expected two CSV rows");
    assert!(
        rows[0].starts_with(&i105),
        "parsed row should contain i105 literal: {}",
        rows[0]
    );
    assert!(
        rows[1].contains(",error,"),
        "error row should include status=error: {}",
        rows[1]
    );
}
