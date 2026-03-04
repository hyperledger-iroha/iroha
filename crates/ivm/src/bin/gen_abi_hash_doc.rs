//! Generate or check the generated ABI hash section in `docs/source/ivm_header.md`.
//! Usage:
//!   cargo run -p ivm --bin gen_abi_hash_doc -- --write
//!   cargo run -p ivm --bin gen_abi_hash_doc -- --check

use std::{fs, path::PathBuf};

const BEGIN: &str = "<!-- BEGIN GENERATED ABI HASHES -->";
const END: &str = "<!-- END GENERATED ABI HASHES -->";

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

    if !write && !check {
        eprintln!("usage: --write or --check");
        return;
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

    let table = ivm::syscalls::render_abi_hashes_markdown_table();
    let expected = format!("{BEGIN}\n{table}{END}");

    if check {
        assert_eq!(
            section, expected,
            "docs/source/ivm_header.md ABI hashes out of date; run: cargo run -p ivm --bin gen_abi_hash_doc -- --write"
        );
        return;
    }

    if write {
        text.replace_range(beg..end + END.len(), &expected);
        fs::write(&path, text).expect("write ivm_header.md");
        eprintln!("updated: {}", path.display());
    }
}
