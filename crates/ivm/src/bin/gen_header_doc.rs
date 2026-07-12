//! Generate or check the generated header policy section in `docs/source/ivm_header.md`.
//! Usage:
//!   cargo run -p ivm --bin gen_header_doc -- --write
//!   cargo run -p ivm --bin gen_header_doc -- --check

use std::{fs, path::PathBuf};

const LAYOUT_BEGIN: &str = "<!-- BEGIN GENERATED HEADER LAYOUT -->";
const LAYOUT_END: &str = "<!-- END GENERATED HEADER LAYOUT -->";
const POLICY_BEGIN: &str = "<!-- BEGIN GENERATED HEADER POLICY -->";
const POLICY_END: &str = "<!-- END GENERATED HEADER POLICY -->";

fn render_header_layout_markdown() -> String {
    format!(
        "- Offsets and sizes ({} bytes total):\n\
         \x20 - 0..4: magic `IVM\\0`\n\
         \x20 - 4: `version_major: u8`\n\
         \x20 - 5: `version_minor: u8`\n\
         \x20 - 6: `mode: u8` (feature bits; see below)\n\
         \x20 - 7: `vector_length: u8`\n\
         \x20 - 8..16: `max_cycles: u64` (little-endian)\n\
         \x20 - 16: `abi_version: u8`\n\
         \x20 - 17..49: `abi_hash: [u8; 32]` (canonical descriptor hash for `abi_version`)\n",
        ivm::HEADER_SIZE
    )
}

fn render_header_policy_markdown() -> String {
    // Known bits from the public ivm_mode re-export
    let zk = ivm::ivm_mode::ZK;
    let vec = ivm::ivm_mode::VECTOR;
    let htm = ivm::ivm_mode::HTM;
    let known_bits = zk | vec | htm;
    let accepted_major = 1u8;
    let vector_len_max = 64u8;

    let mut md = String::new();
    md.push_str("| Field | Policy |\n");
    md.push_str("|---|---|\n");
    md.push_str(&format!("| version_major | {accepted_major} |\n"));
    md.push_str("| version_minor | 0 or 1 (deployable CNTR contracts require 1) |\n");
    md.push_str(&format!(
        "| mode (known bits) | 0x{known_bits:02x} (ZK=0x{zk:02x}, VECTOR=0x{vec:02x}, HTM=0x{htm:02x}) |\n"
    ));
    // First release: only ABI v1 is accepted.
    md.push_str("| abi_version | 1 |\n");
    md.push_str(&format!(
        "| vector_length | 0 or 1..={vector_len_max} (0 selects runtime default; independent of VECTOR bit) |\n"
    ));
    md
}

fn main() {
    let mut write = false;
    let mut check = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--write" => write = true,
            "--check" => check = true,
            _ => {}
        }
    }

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = PathBuf::from(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join("docs/source/ivm_header.md");
    let mut text = fs::read_to_string(&path).expect("read ivm_header.md");

    let layout_beg = text
        .find(LAYOUT_BEGIN)
        .unwrap_or_else(|| panic!("layout begin marker not found in {}", path.display()));
    let layout_end = text
        .find(LAYOUT_END)
        .unwrap_or_else(|| panic!("layout end marker not found in {}", path.display()));
    let layout_section = &text[layout_beg..layout_end + LAYOUT_END.len()];
    let layout = render_header_layout_markdown();
    let expected_layout = format!("{LAYOUT_BEGIN}\n{layout}{LAYOUT_END}");

    let policy_beg = text
        .find(POLICY_BEGIN)
        .unwrap_or_else(|| panic!("policy begin marker not found in {}", path.display()));
    let policy_end = text
        .find(POLICY_END)
        .unwrap_or_else(|| panic!("policy end marker not found in {}", path.display()));
    let policy_section = &text[policy_beg..policy_end + POLICY_END.len()];
    let table = render_header_policy_markdown();
    let expected_policy = format!("{POLICY_BEGIN}\n{table}{POLICY_END}");

    if check {
        assert_eq!(
            layout_section, expected_layout,
            "docs/source/ivm_header.md header layout out of date; run: cargo run -p ivm --bin gen_header_doc -- --write"
        );
        assert_eq!(
            policy_section, expected_policy,
            "docs/source/ivm_header.md header policy out of date; run: cargo run -p ivm --bin gen_header_doc -- --write"
        );
        return;
    }

    if write {
        text.replace_range(layout_beg..layout_end + LAYOUT_END.len(), &expected_layout);
        let policy_beg = text
            .find(POLICY_BEGIN)
            .expect("policy begin marker survives layout replacement");
        let policy_end = text
            .find(POLICY_END)
            .expect("policy end marker survives layout replacement");
        text.replace_range(policy_beg..policy_end + POLICY_END.len(), &expected_policy);
        fs::write(&path, text).expect("write ivm_header.md");
        eprintln!("updated: {}", path.display());
        return;
    }

    eprintln!("usage: --write or --check");
}
