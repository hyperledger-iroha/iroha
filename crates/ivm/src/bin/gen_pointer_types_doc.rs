//! Generate or check the generated pointer-ABI sections in docs.
//! Usage:
//!   cargo run -p ivm --bin gen_pointer_types_doc -- --write
//!   cargo run -p ivm --bin gen_pointer_types_doc -- --check

use std::{fs, path::PathBuf};

const BEGIN: &str = "<!-- BEGIN GENERATED POINTER TYPES -->";
const END: &str = "<!-- END GENERATED POINTER TYPES -->";

fn normalized_generated_block(
    text: &str,
    marker_end: usize,
    expected_block: &str,
) -> (usize, String) {
    let mut replace_end = marker_end;
    while text.as_bytes().get(replace_end) == Some(&b'\n') {
        replace_end += 1;
    }
    let separator = if replace_end == text.len() { "\n" } else { "\n\n" };
    (replace_end, format!("{expected_block}{separator}"))
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
    let path_pointer = PathBuf::from(manifest_dir).join("docs/pointer_abi.md");
    let path_ivm_md = PathBuf::from(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join("ivm.md");

    // Render expected table
    let table = ivm::render_pointer_types_markdown_table();
    let expected_block = format!("{BEGIN}\n{table}{END}");

    // Helper: check/replace in a single file
    fn process(path: &PathBuf, expected_block: &str, write: bool, check: bool) {
        let mut text = fs::read_to_string(path).expect("read doc file");
        let beg = text
            .find(BEGIN)
            .unwrap_or_else(|| panic!("begin marker not found in {}", path.display()));
        let end = text
            .find(END)
            .unwrap_or_else(|| panic!("end marker not found in {}", path.display()));
        let marker_end = end + END.len();
        let (replace_end, replacement) =
            normalized_generated_block(&text, marker_end, expected_block);
        let section = &text[beg..replace_end];
        if check {
            assert_eq!(
                section,
                replacement,
                "{} out of date; run: cargo run -p ivm --bin gen_pointer_types_doc -- --write",
                path.display()
            );
        }
        if write {
            text.replace_range(beg..replace_end, &replacement);
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

#[cfg(test)]
mod tests {
    use super::normalized_generated_block;

    #[test]
    fn generated_block_spacing_is_canonical_and_idempotent() {
        let marker = "<!-- END -->";
        let expected = "<!-- BEGIN -->\nbody\n<!-- END -->";

        let with_following_text = format!("{expected}\n\n\nNotes\n");
        let marker_end = with_following_text.find(marker).expect("end marker") + marker.len();
        let (replace_end, replacement) =
            normalized_generated_block(&with_following_text, marker_end, expected);
        assert_eq!(replacement, format!("{expected}\n\n"));
        let mut normalized = with_following_text;
        normalized.replace_range(0..replace_end, &replacement);
        assert_eq!(normalized, format!("{expected}\n\nNotes\n"));

        let marker_end = normalized.find(marker).expect("end marker") + marker.len();
        let (replace_end, second) =
            normalized_generated_block(&normalized, marker_end, expected);
        assert_eq!(&normalized[..replace_end], second);

        let at_eof = format!("{expected}\n\n");
        let marker_end = at_eof.find(marker).expect("end marker") + marker.len();
        let (replace_end, replacement) =
            normalized_generated_block(&at_eof, marker_end, expected);
        assert_eq!(replace_end, at_eof.len());
        assert_eq!(replacement, format!("{expected}\n"));
    }
}
