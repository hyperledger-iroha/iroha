//! Ensures the GPU Poseidon tables embed the canonical manifest.
use fastpq_isi::poseidon::{PERMUTATION_PROFILE_ID, SBOX_EXPONENT, STATE_WIDTH};
use fastpq_prover::{poseidon_manifest, poseidon_profile_id, poseidon_profile_sha256};

#[test]
fn permutation_profile_identity_binds_sbox_and_constants() {
    assert_eq!(poseidon_profile_id(), PERMUTATION_PROFILE_ID);
    assert!(poseidon_profile_id().contains("x7"));
    assert_eq!(SBOX_EXPONENT, 7);
    assert_eq!(
        poseidon_profile_sha256(),
        "874028de6098d274b42a1dc5034b9d27892a6e3913f7ccdf0060d57fe2db4bbb"
    );
    assert_eq!(
        poseidon_manifest().profile_sha256_hex(),
        poseidon_profile_sha256()
    );
}
#[test]
fn metal_poseidon_tables_match_manifest() {
    let manifest = poseidon_manifest();
    let metal_path = concat!(env!("CARGO_MANIFEST_DIR"), "/metal/kernels/poseidon2.metal");
    let contents = std::fs::read_to_string(metal_path).expect("read poseidon2.metal");
    let round_constants = parse_constant_table(&contents, "ROUND_CONSTANTS");
    assert_eq!(
        round_constants.as_slice(),
        manifest.round_constants().as_slice(),
        "Metal round constants diverged from the manifest"
    );
    let mds = parse_constant_table(&contents, "MDS");
    assert_eq!(
        mds.as_slice(),
        manifest.mds().as_slice(),
        "Metal MDS matrix diverged from the manifest"
    );
}
#[test]
fn cuda_poseidon_tables_match_manifest() {
    let manifest = poseidon_manifest();
    let cuda_path = concat!(env!("CARGO_MANIFEST_DIR"), "/cuda/fastpq_cuda.cu");
    let contents = std::fs::read_to_string(cuda_path).expect("read fastpq_cuda.cu");
    let round_constants = parse_constant_table(&contents, "POSEIDON_ROUND_CONSTANTS");
    assert_eq!(
        round_constants.as_slice(),
        manifest.round_constants().as_slice(),
        "CUDA round constants diverged from the manifest"
    );
    let mds = parse_constant_table(&contents, "POSEIDON_MDS");
    assert_eq!(
        mds.as_slice(),
        manifest.mds().as_slice(),
        "CUDA MDS matrix diverged from the manifest"
    );
}
#[test]
fn accelerator_sources_pin_the_bijective_goldilocks_sbox() {
    assert_eq!(SBOX_EXPONENT, 7);

    let metal_path = concat!(env!("CARGO_MANIFEST_DIR"), "/metal/kernels/poseidon2.metal");
    let metal = std::fs::read_to_string(metal_path).expect("read poseidon2.metal");
    assert!(metal.contains("pow7_vec(add_mod_vec"));
    assert!(metal.contains("chunk[i].x = pow7(chunk[i].x)"));
    assert!(!metal.contains("pow5_vec"));
    let field_path = concat!(env!("CARGO_MANIFEST_DIR"), "/metal/kernels/field.metal");
    let field = std::fs::read_to_string(field_path).expect("read field.metal");
    assert!(field.contains("inline ulong pow7(ulong x)"));
    assert!(!field.contains("inline ulong pow5(ulong x)"));

    let cuda_path = concat!(env!("CARGO_MANIFEST_DIR"), "/cuda/fastpq_cuda.cu");
    let cuda = std::fs::read_to_string(cuda_path).expect("read fastpq_cuda.cu");
    assert!(cuda.contains("poseidon_pow7(f_add"));
    assert!(cuda.contains("state[0] = poseidon_pow7(state[0])"));
    assert!(!cuda.contains("poseidon_pow5"));
}
fn parse_constant_table(contents: &str, marker: &str) -> Vec<[u64; STATE_WIDTH]> {
    let section = extract_braced_section(contents, marker);
    let values = parse_hex_literals(section);
    assert!(
        values.len().is_multiple_of(STATE_WIDTH),
        "section {marker} does not align to STATE_WIDTH={STATE_WIDTH}: {} values",
        values.len()
    );
    values
        .chunks_exact(STATE_WIDTH)
        .map(|chunk| [chunk[0], chunk[1], chunk[2]])
        .collect()
}
fn extract_braced_section<'a>(contents: &'a str, marker: &str) -> &'a str {
    let start = contents
        .find(marker)
        .unwrap_or_else(|| panic!("failed to find marker `{marker}`"));
    let mut brace_start = None;
    for (idx, ch) in contents[start..].char_indices() {
        if ch == '{' {
            brace_start = Some(start + idx);
            break;
        }
    }
    let brace_start = brace_start.unwrap_or_else(|| panic!("marker `{marker}` missing `{{`"));
    let mut depth = 0usize;
    for (offset, ch) in contents[brace_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let begin = brace_start + 1;
                    let end = brace_start + offset;
                    return &contents[begin..end];
                }
            }
            _ => {}
        }
    }
    panic!("marker `{marker}` missing closing `}}`");
}
fn parse_hex_literals(section: &str) -> Vec<u64> {
    let mut values = Vec::new();
    for token in section.split(|c: char| {
        c == ',' || c == '{' || c == '}' || c.is_whitespace() || c == '[' || c == ']'
    }) {
        let token = token.trim();
        if token.starts_with("0x") {
            values.push(parse_hex_token(token));
        }
    }
    values
}
fn parse_hex_token(token: &str) -> u64 {
    let trimmed = token.trim_end_matches(&['u', 'U', 'l', 'L'][..]);
    let digits = trimmed
        .strip_prefix("0x")
        .unwrap_or_else(|| panic!("expected hex literal, found `{token}`"))
        .replace('_', "");
    u64::from_str_radix(&digits, 16).expect("invalid hex literal")
}
