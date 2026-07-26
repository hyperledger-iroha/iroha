#[test]
fn abi_syscall_list_is_sorted_and_unique() {
    let list = ivm::syscalls::abi_syscall_list();
    assert!(!list.is_empty(), "ABI syscall list must not be empty");
    // Check strict ascending order (implies uniqueness)
    for pair in list.windows(2) {
        assert!(
            pair[0] < pair[1],
            "ABI list not strictly ascending at {:x}, {:x}",
            pair[0],
            pair[1]
        );
    }
    // Check that sorting a cloned copy doesn't change the order
    let mut cloned = list.to_vec();
    cloned.sort_unstable();
    assert_eq!(
        cloned, list,
        "ABI syscall list should be in ascending order"
    );
}

#[test]
fn markdown_render_is_in_ascending_order() {
    // Render the markdown table and parse syscall numbers from the first column.
    let md = ivm::syscalls::render_syscalls_markdown_table();
    let mut parsed: Vec<u32> = Vec::new();
    for line in md.lines() {
        // Skip header and separator lines
        if !line.starts_with('|') {
            continue;
        }
        if line.starts_with("|---") {
            continue;
        }
        // Line format: | 0xNN | NAME | ...
        let parts: Vec<_> = line.split('|').map(|s| s.trim()).collect();
        if parts.len() >= 3
            && parts[1].starts_with("0x")
            && let Ok(n) = u32::from_str_radix(&parts[1][2..], 16)
        {
            parsed.push(n);
        }
    }
    assert!(!parsed.is_empty(), "markdown must render some syscalls");
    // Check ascending order
    for pair in parsed.windows(2) {
        assert!(
            pair[0] < pair[1],
            "markdown rows not ascending at {:x}, {:x}",
            pair[0],
            pair[1]
        );
    }
    // Should match the abi_syscall_list exactly
    assert_eq!(
        parsed,
        ivm::syscalls::abi_syscall_list(),
        "markdown must reflect canonical list"
    );
}

#[test]
fn min_list_length_matches_abi_list() {
    let list = ivm::syscalls::abi_syscall_list();
    let md = ivm::syscalls::render_syscalls_min_list();
    // Count lines that look like entries: "- 0x.."
    let count = md
        .lines()
        .filter(|l| l.trim_start().starts_with("- 0x"))
        .count();
    assert_eq!(
        count,
        list.len(),
        "min list length should match abi_syscall_list length"
    );
}

#[test]
fn min_list_count_matches_markdown_table_row_count() {
    let min_md = ivm::syscalls::render_syscalls_min_list();
    let min_count = min_md
        .lines()
        .filter(|l| l.trim_start().starts_with("- 0x"))
        .count();

    let table_md = ivm::syscalls::render_syscalls_markdown_table();
    let table_count = table_md
        .lines()
        .filter(|l| l.starts_with('|'))
        .filter(|l| !l.starts_with("|---"))
        .filter(|l| !l.starts_with("| Number "))
        .count();

    assert_eq!(
        min_count, table_count,
        "min list count should match markdown table row count"
    );
}

#[test]
fn min_list_numbers_match_markdown_numbers() {
    // Parse numbers from the minimal list
    let min_md = ivm::syscalls::render_syscalls_min_list();
    let mut from_min: Vec<u32> = Vec::new();
    for line in min_md.lines() {
        let s = line.trim_start();
        if !s.starts_with("- 0x") {
            continue;
        }
        // tokens: ["-", "0xNN", ...] or "- 0xNN NAME"
        for tok in s.split_whitespace() {
            if let Some(hex) = tok.strip_prefix("0x") {
                if let Ok(n) = u32::from_str_radix(hex, 16) {
                    from_min.push(n);
                }
                break;
            }
        }
    }
    assert!(!from_min.is_empty());

    // Parse numbers from the markdown table (first column)
    let table_md = ivm::syscalls::render_syscalls_markdown_table();
    let mut from_table: Vec<u32> = Vec::new();
    for line in table_md.lines() {
        if !line.starts_with('|') {
            continue;
        }
        if line.starts_with("|---") || line.starts_with("| Number ") {
            continue;
        }
        let parts: Vec<_> = line.split('|').map(|s| s.trim()).collect();
        if parts.len() >= 3
            && parts[1].starts_with("0x")
            && let Ok(n) = u32::from_str_radix(&parts[1][2..], 16)
        {
            from_table.push(n);
        }
    }
    assert_eq!(
        from_min, from_table,
        "numbers in min list and markdown table must match pairwise"
    );
}

#[test]
fn syscalls_markdown_header_and_row_count_ok() {
    let md = ivm::syscalls::render_syscalls_markdown_table();
    let mut lines = md.lines();
    let header = lines.next().expect("non-empty table");
    assert!(header.contains("| Number |"));
    assert!(header.contains("| Name |"));
    assert!(header.contains("| Args |"));
    assert!(header.contains("| Return |"));
    assert!(header.contains("| Gas |"));

    let row_count = md
        .lines()
        .filter(|l| l.starts_with('|'))
        .filter(|l| !l.starts_with("|---"))
        .filter(|l| !l.starts_with("| Number "))
        .count();
    assert_eq!(row_count, ivm::syscalls::abi_syscall_list().len());
}

#[test]
fn min_list_names_match_syscall_name_lookup() {
    let md = ivm::syscalls::render_syscalls_min_list();
    for line in md.lines() {
        let s = line.trim_start();
        if !s.starts_with("- 0x") {
            continue;
        }
        // Expect format: "- 0xNN NAME"
        let mut iter = s.split_whitespace();
        let dash = iter.next();
        assert_eq!(dash, Some("-"));
        let hex_tok = iter.next().expect("hex token");
        let name_tok = iter.next().unwrap_or("");
        // Parse hex
        assert!(hex_tok.starts_with("0x"));
        let n = u32::from_str_radix(&hex_tok[2..], 16).expect("hex");
        // Lookup name via syscall_name
        let expected = ivm::syscalls::syscall_name(n).unwrap_or("");
        // Some entries may omit name (vendor), in which case expected is "" and name_tok may be ""
        assert_eq!(name_tok, expected, "name mismatch for 0x{n:02X}");
    }
}
