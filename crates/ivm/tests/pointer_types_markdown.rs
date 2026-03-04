use ivm::{
    self, PointerType, SyscallPolicy, is_type_allowed_for_policy,
    render_pointer_types_markdown_table,
};

fn all_pointer_types() -> Vec<PointerType> {
    use PointerType::*;
    vec![
        AccountId,
        AssetDefinitionId,
        Name,
        Json,
        NftId,
        Blob,
        AssetId,
        DomainId,
        NoritoBytes,
        DataSpaceId,
        AxtDescriptor,
        AssetHandle,
        ProofBlob,
    ]
}

#[test]
fn pointer_types_markdown_is_consistent_with_policy() {
    let md = render_pointer_types_markdown_table();
    // Parse rows and collect (id, name, abi_v1_allowed)
    let mut rows = Vec::new();
    for line in md.lines() {
        if line.starts_with("|---") || line.starts_with("| ID ") || line.trim().is_empty() {
            continue;
        }
        let parts: Vec<_> = line
            .trim_matches('|')
            .split('|')
            .map(|s| s.trim())
            .collect();
        assert!(parts.len() == 3, "bad row: {line}");
        let id_str = parts[0];
        let name = parts[1];
        let abi_v1 = parts[2];
        assert!(id_str.starts_with("0x") && id_str.len() == 6);
        let id = u16::from_str_radix(&id_str[2..], 16).expect("hex id");
        rows.push((id, name.to_string(), abi_v1.to_string()));
    }
    assert!(!rows.is_empty());
    // Check ascending by id and uniqueness
    for w in rows.windows(2) {
        assert!(w[0].0 < w[1].0, "ids must be ascending");
    }

    // For each row, cross-check policy with actual implementation
    let variants = all_pointer_types();
    for (id, name, abi_v1) in rows {
        // Find the variant by Debug name and check id
        let ty = variants
            .iter()
            .copied()
            .find(|t| format!("{t:?}") == name)
            .expect("variant present");
        assert_eq!(ty as u16, id, "id must match enum discriminant");
        let expected_v1 = if is_type_allowed_for_policy(SyscallPolicy::AbiV1, ty) {
            "OK"
        } else {
            "-"
        };
        assert_eq!(abi_v1, expected_v1, "policy mismatch for {name}");
    }
}

#[test]
fn pointer_types_markdown_pairs_match_enum_variants() {
    // Build expected (id, name) pairs from enum variants, sorted by id
    let mut expected: Vec<(u16, String)> = all_pointer_types()
        .into_iter()
        .map(|t| (t as u16, format!("{t:?}")))
        .collect();
    expected.sort_by_key(|(id, _)| *id);

    // Parse actual (id, name) pairs from rendered markdown, in order
    let md = render_pointer_types_markdown_table();
    let mut actual: Vec<(u16, String)> = Vec::new();
    for line in md.lines() {
        if line.starts_with("|---") || line.starts_with("| ID ") || line.trim().is_empty() {
            continue;
        }
        let parts: Vec<_> = line
            .trim_matches('|')
            .split('|')
            .map(|s| s.trim())
            .collect();
        assert!(parts.len() == 3, "bad row: {line}");
        let id = u16::from_str_radix(&parts[0][2..], 16).expect("hex id");
        let name = parts[1].to_string();
        actual.push((id, name));
    }

    assert_eq!(
        actual, expected,
        "pointer types table must match enum variants pairwise"
    );
}

#[test]
fn pointer_types_markdown_header_and_row_count_ok() {
    let md = render_pointer_types_markdown_table();
    let mut lines = md.lines();
    let header = lines.next().expect("non-empty table");
    assert!(header.contains("| ID |"));
    assert!(header.contains("| Name |"));
    assert!(header.contains("| ABI v1 |"));

    // Count data rows (exclude header and separator)
    let rows = md
        .lines()
        .filter(|l| l.starts_with('|'))
        .filter(|l| !l.starts_with("|---"))
        .filter(|l| !l.starts_with("| ID "))
        .count();
    assert_eq!(
        rows,
        all_pointer_types().len(),
        "row count must match enum variants count"
    );
}
