#[test]
fn syscalls_markdown_has_gas_column_and_no_dash_values() {
    let md = ivm::syscalls::render_syscalls_markdown_table();
    // Must have header columns including Gas
    let header = md.lines().next().expect("non-empty");
    assert!(header.contains("| Gas |"), "header must contain Gas column");
    let mut dashed = Vec::new();
    for line in md.lines() {
        if !line.starts_with('|') || line.starts_with("|---") || line.starts_with("| Number ") {
            continue;
        }
        // Read the rightmost table column; argument/return text can contain
        // literal `|` characters such as `ciphertext || tag`.
        let gas = line.trim_end_matches('|').rsplit('|').next().map(str::trim);
        if matches!(gas, Some("-" | "") | None) {
            dashed.push(line.to_string());
        }
    }
    assert!(
        dashed.is_empty(),
        "all generated ABI rows must name a gas asset: {}",
        dashed.join("; ")
    );
}
