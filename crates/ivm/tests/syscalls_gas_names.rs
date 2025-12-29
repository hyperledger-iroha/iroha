// Check that gas assets referenced in the generated syscalls table resolve to generated GAS_ASSETS.

#[test]
fn gas_assets_in_syscalls_table_are_known() {
    // If gas assets weren't generated for this build, skip the test gracefully.
    if ivm::syscalls::gas_spec::GAS_ASSETS.is_empty() {
        return;
    }
    const BEGIN: &str = "<!-- BEGIN GENERATED SYSCALLS -->";
    const END: &str = "<!-- END GENERATED SYSCALLS -->";
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");
    let text = std::fs::read_to_string(path).expect("read syscalls.md");
    let beg = text.find(BEGIN).expect("begin marker present");
    let end = text.find(END).expect("end marker present");
    let section = &text[beg..end + END.len()];
    let mut unknown = Vec::new();
    let known_ids: std::collections::HashSet<&'static str> = ivm::syscalls::gas_spec::GAS_ASSETS
        .iter()
        .map(|a| a.asset_id)
        .collect();

    for line in section.lines() {
        // Expect rows like: | 0xNN | NAME | Args | Return | Gas |
        if !line.starts_with('|') || line.contains("---") || line.contains("Number") {
            continue;
        }
        let cols: Vec<&str> = line.split('|').map(|s| s.trim()).collect();
        if cols.len() < 6 {
            continue;
        }
        let gas_col = cols[5];
        for p in gas_col
            .split('+')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            let token = p.split_whitespace().next().unwrap_or("");
            if token.starts_with("asset:gas/") && !known_ids.contains(token) {
                unknown.push(format!("{token} in row: {line}"));
            }
        }
    }

    assert!(
        unknown.is_empty(),
        "Unknown gas assets: {}",
        unknown.join("; ")
    );
}
