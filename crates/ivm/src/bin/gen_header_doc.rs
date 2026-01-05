//! Generate or check the generated header policy section in `docs/source/ivm_header.md`.
//! Usage:
//!   cargo run -p ivm --bin gen_header_doc -- --write
//!   cargo run -p ivm --bin gen_header_doc -- --check

use std::{fs, path::PathBuf};

const BEGIN: &str = "<!-- BEGIN GENERATED HEADER POLICY -->";
const END: &str = "<!-- END GENERATED HEADER POLICY -->";

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
    md.push_str("| version_minor | 0 |\n");
    md.push_str(&format!(
        "| mode (known bits) | 0x{known_bits:02x} (ZK=0x{zk:02x}, VECTOR=0x{vec:02x}, HTM=0x{htm:02x}) |\n"
    ));
    // First release: only ABI v1 is accepted.
    md.push_str("| abi_version | 1 |\n");
    md.push_str(&format!(
        "| vector_length | 0 or 1..={vector_len_max} (advisory; independent of VECTOR bit) |\n"
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

    let beg = text
        .find(BEGIN)
        .unwrap_or_else(|| panic!("begin marker not found in {}", path.display()));
    let end = text
        .find(END)
        .unwrap_or_else(|| panic!("end marker not found in {}", path.display()));
    let section = &text[beg..end + END.len()];

    let table = render_header_policy_markdown();
    let expected = format!("{BEGIN}\n{table}{END}");

    if check {
        assert_eq!(
            section, expected,
            "docs/source/ivm_header.md header policy out of date; run: cargo run -p ivm --bin gen_header_doc -- --write"
        );
        return;
    }

    if write {
        text.replace_range(beg..end + END.len(), &expected);
        fs::write(&path, text).expect("write ivm_header.md");
        eprintln!("updated: {}", path.display());
        return;
    }

    eprintln!("usage: --write or --check");
}
