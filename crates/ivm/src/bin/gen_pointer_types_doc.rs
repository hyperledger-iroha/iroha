//! Generate or check the generated pointer-ABI sections in docs.
//! Usage:
//!   cargo run -p ivm --bin gen_pointer_types_doc -- --write
//!   cargo run -p ivm --bin gen_pointer_types_doc -- --check

use std::{
    fs,
    path::{Path, PathBuf},
};

const BEGIN: &str = "<!-- BEGIN GENERATED POINTER TYPES -->";
const END: &str = "<!-- END GENERATED POINTER TYPES -->";

fn localized_pointer_doc_paths(workspace_root: &Path) -> Result<Vec<PathBuf>, String> {
    let localized_root = workspace_root.join("docs/i18n/root");
    let entries = fs::read_dir(&localized_root)
        .map_err(|error| format!("read {}: {error}", localized_root.display()))?;
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry
            .map_err(|error| format!("read entry in {}: {error}", localized_root.display()))?;
        let path = entry.path().join("ivm.md");
        if path.is_file() {
            paths.push(path);
        }
    }
    paths.sort();
    if paths.is_empty() {
        return Err(format!(
            "no localized pointer documents found under {}",
            localized_root.display()
        ));
    }
    Ok(paths)
}

fn render_generated_block(text: &str, expected_block: &str) -> Result<String, String> {
    let begin = text
        .find(BEGIN)
        .ok_or_else(|| format!("begin marker `{BEGIN}` not found"))?;
    if text[begin + BEGIN.len()..].contains(BEGIN) {
        return Err(format!("multiple begin markers `{BEGIN}` found"));
    }

    let end_start = begin
        + text[begin..]
            .find(END)
            .ok_or_else(|| format!("end marker `{END}` not found after begin marker"))?;
    let end = end_start + END.len();
    if text[end..].contains(END) {
        return Err(format!("multiple end markers `{END}` found"));
    }

    let mut rendered = text.to_owned();
    rendered.replace_range(begin..end, expected_block);
    Ok(rendered)
}

fn process(path: &Path, expected_block: &str, write: bool, check: bool) {
    let text =
        fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let rendered = render_generated_block(&text, expected_block)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    if check {
        assert_eq!(
            text,
            rendered,
            "{} out of date; run: cargo run -p ivm --bin gen_pointer_types_doc -- --write",
            path.display()
        );
    }
    if write && text != rendered {
        fs::write(path, rendered)
            .unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
        eprintln!("updated: {}", path.display());
    }
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
    let workspace_root = PathBuf::from(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf();
    let path_ivm_md = workspace_root.join("ivm.md");

    // Render expected table
    let table = ivm::render_pointer_types_markdown_table();
    let expected_block = format!("{BEGIN}\n{table}{END}");

    if !write && !check {
        eprintln!("usage: --write or --check");
        return;
    }

    process(&path_pointer, &expected_block, write, check);
    process(&path_ivm_md, &expected_block, write, check);
    let localized_paths = localized_pointer_doc_paths(&workspace_root)
        .unwrap_or_else(|error| panic!("discover localized pointer documents: {error}"));
    for path in localized_paths {
        process(&path, &expected_block, write, check);
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::{BEGIN, END, localized_pointer_doc_paths, render_generated_block};

    static NEXT_TEMP_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn generated_block_replacement_preserves_localized_surroundings() {
        let prefix = "---\nlang: ar\ndirection: rtl\n---\n\n<div dir=\"rtl\">\n\nمقدمة\n\n";
        let suffix = "\n\nخاتمة\n\n</div>\n";
        let current = format!("{prefix}{BEGIN}\nstale\n{END}{suffix}");
        let expected_block = format!("{BEGIN}\ncanonical\n{END}");
        let expected = format!("{prefix}{expected_block}{suffix}");

        let rendered =
            render_generated_block(&current, &expected_block).expect("replace generated block");
        assert_eq!(rendered, expected);
        assert_eq!(
            render_generated_block(&rendered, &expected_block).expect("idempotent replacement"),
            rendered
        );
    }

    #[test]
    fn localized_pointer_document_discovery_is_sorted() {
        let unique = NEXT_TEMP_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "ivm-pointer-doc-generator-{}-{unique}",
            std::process::id()
        ));
        let localized_root = root.join("docs/i18n/root");
        for locale in ["zh-hant", "am"] {
            let locale_root = localized_root.join(locale);
            fs::create_dir_all(&locale_root).expect("create locale directory");
            fs::write(locale_root.join("ivm.md"), "test").expect("write localized document");
        }
        fs::create_dir_all(localized_root.join("missing")).expect("create unrelated locale");

        let paths = localized_pointer_doc_paths(&root).expect("discover localized documents");
        assert_eq!(
            paths,
            [
                localized_root.join("am/ivm.md"),
                localized_root.join("zh-hant/ivm.md"),
            ]
        );

        fs::remove_dir_all(root).expect("remove temporary directory");
    }
}
