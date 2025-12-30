//! Generate or check the generated pointer-ABI sections in docs.
//! Usage:
//!   cargo run -p ivm --bin gen_pointer_types_doc -- --write
//!   cargo run -p ivm --bin gen_pointer_types_doc -- --check

use std::{fs, path::PathBuf};

const BEGIN: &str = "<!-- BEGIN GENERATED POINTER TYPES -->";
const END: &str = "<!-- END GENERATED POINTER TYPES -->";

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
    let path_pointer = PathBuf::from(manifest_dir).join("docs/pointer_abi.md");
    let path_ivm_md = PathBuf::from(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join("ivm.md");

    // Render expected table
    let table = ivm::render_pointer_types_markdown_table();
    let expected_block = format!("{BEGIN}\n{table}{END}\n");

    // Helper: check/replace in a single file
    fn process(path: &PathBuf, expected_block: &str, write: bool, check: bool) {
        let mut text = fs::read_to_string(path).expect("read doc file");
        let beg = text
            .find(BEGIN)
            .unwrap_or_else(|| panic!("begin marker not found in {}", path.display()));
        let end = text
            .find(END)
            .unwrap_or_else(|| panic!("end marker not found in {}", path.display()));
        let section = &text[beg..end + END.len()];
        if check {
            assert_eq!(
                section.trim_end_matches('\n'),
                expected_block.trim_end_matches('\n'),
                "{} out of date; run: cargo run -p ivm --bin gen_pointer_types_doc -- --write",
                path.display()
            );
        }
        if write {
            text.replace_range(beg..end + END.len(), expected_block);
            fs::write(path, text).expect("write doc file");
            eprintln!("updated: {}", path.display());
        }
    }

    if !write && !check {
        eprintln!("usage: --write or --check");
        return;
    }

    process(&path_pointer, &expected_block, write, check);
    process(&path_ivm_md, &expected_block, write, check);
}
