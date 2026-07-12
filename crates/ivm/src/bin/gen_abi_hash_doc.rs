//! Generate or check ABI hash sections in every localized `docs/source/ivm_header*.md`.
//! Usage:
//!   cargo run -p ivm --bin gen_abi_hash_doc -- --write
//!   cargo run -p ivm --bin gen_abi_hash_doc -- --check

use std::{fs, path::PathBuf};

const BEGIN: &str = "<!-- BEGIN GENERATED ABI HASHES -->";
const END: &str = "<!-- END GENERATED ABI HASHES -->";

fn header_paths() -> Vec<PathBuf> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let source_dir = PathBuf::from(manifest_dir)
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .join("docs/source");
    let mut paths = fs::read_dir(&source_dir)
        .unwrap_or_else(|error| panic!("read {}: {error}", source_dir.display()))
        .map(|entry| entry.expect("read docs/source entry").path())
        .filter(|path| {
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                return false;
            };
            name == "ivm_header.md" || (name.starts_with("ivm_header.") && name.ends_with(".md"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    assert!(!paths.is_empty(), "no docs/source/ivm_header*.md files");
    paths
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

    if !write && !check {
        eprintln!("usage: --write or --check");
        return;
    }

    let table = ivm::syscalls::render_abi_hashes_markdown_table();
    let expected = format!("{BEGIN}\n{table}{END}");

    for path in header_paths() {
        let mut text = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        let beg = text
            .find(BEGIN)
            .unwrap_or_else(|| panic!("begin marker not found in {}", path.display()));
        let end = text
            .find(END)
            .unwrap_or_else(|| panic!("end marker not found in {}", path.display()));
        let section_end = end + END.len();

        if check {
            assert_eq!(
                &text[beg..section_end],
                expected,
                "{} ABI hashes out of date; run: cargo run -p ivm --bin gen_abi_hash_doc -- --write",
                path.display()
            );
        } else if write && text[beg..section_end] != expected {
            text.replace_range(beg..section_end, &expected);
            fs::write(&path, text)
                .unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
            eprintln!("updated: {}", path.display());
        }
    }
}
