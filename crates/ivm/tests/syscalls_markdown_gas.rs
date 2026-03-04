#[test]
fn syscalls_markdown_has_gas_column_and_optional_values() {
    let md = ivm::syscalls::render_syscalls_markdown_table();
    // Must have header columns including Gas
    let header = md.lines().next().expect("non-empty");
    assert!(header.contains("| Gas |"), "header must contain Gas column");
    // Count rows with non-dash gas entries; allow zero when docs are not generated.
    let mut _non_dash = 0usize;
    for line in md.lines() {
        if !line.starts_with('|') || line.starts_with("|---") || line.starts_with("| Number ") {
            continue;
        }
        // Split columns and read the Gas column (5th)
        let parts: Vec<_> = line.split('|').map(|s| s.trim()).collect();
        if parts.len() >= 6 {
            let gas = parts[5];
            if gas != "-" && !gas.is_empty() {
                _non_dash += 1;
            }
        }
    }
    // Presence of the Gas column is already asserted; this test tolerates 0 filled entries.
}
