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
const POINTER_TYPE_GOLDEN_BEGIN: &str = "    // BEGIN GENERATED ABI V1 POINTER TYPE IDS";
const POINTER_TYPE_GOLDEN_END: &str = "    // END GENERATED ABI V1 POINTER TYPE IDS";

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

fn render_generated_block(
    text: &str,
    begin_marker: &str,
    end_marker: &str,
    expected_block: &str,
) -> Result<String, String> {
    let begin = text
        .find(begin_marker)
        .ok_or_else(|| format!("begin marker `{begin_marker}` not found"))?;
    if text[begin + begin_marker.len()..].contains(begin_marker) {
        return Err(format!("multiple begin markers `{begin_marker}` found"));
    }

    let end_start = begin
        + text[begin..]
            .find(end_marker)
            .ok_or_else(|| format!("end marker `{end_marker}` not found after begin marker"))?;
    let end = end_start + end_marker.len();
    if text[..begin].contains(end_marker) || text[end..].contains(end_marker) {
        return Err(format!(
            "multiple or misplaced end markers `{end_marker}` found"
        ));
    }

    let mut rendered = text.to_owned();
    rendered.replace_range(begin..end, expected_block);
    Ok(rendered)
}

fn render_pointer_type_golden_block(types: &[ivm::PointerType]) -> Result<String, String> {
    let mut previous_id = None;
    let mut entries = Vec::with_capacity(types.len());
    for pointer_type in types {
        let id = *pointer_type as u16;
        if previous_id.is_some_and(|previous| previous >= id) {
            return Err(format!(
                "pointer types are not strictly increasing: {previous_id:?} then 0x{id:04X}"
            ));
        }
        previous_id = Some(id);
        let name = format!("{pointer_type:?}");
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(format!(
                "pointer type 0x{id:04X} has invalid Rust variant name `{name}`"
            ));
        }
        entries.push((name, id));
    }

    let mut rendered = String::new();
    rendered.push_str(POINTER_TYPE_GOLDEN_BEGIN);
    rendered.push_str("\n    let expected: &[(P, u16)] = &[\n");
    for (name, id) in &entries {
        rendered.push_str("        (P::");
        rendered.push_str(name);
        rendered.push_str(&format!(", 0x{id:04X}),\n"));
    }
    rendered.push_str("    ];\n");
    rendered.push_str(POINTER_TYPE_GOLDEN_END);
    Ok(rendered)
}

fn process(
    path: &Path,
    begin_marker: &str,
    end_marker: &str,
    expected_block: &str,
    write: bool,
    check: bool,
) {
    let text =
        fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let rendered = render_generated_block(&text, begin_marker, end_marker, expected_block)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    if check {
        assert_eq!(
            text,
            rendered,
            "{} out of date; run: cargo run --locked -p ivm --bin gen_pointer_types_doc -- --write",
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
    let path_pointer_type_golden =
        PathBuf::from(manifest_dir).join("tests/pointer_type_ids_golden.rs");

    // Render expected table
    let table = ivm::render_pointer_types_markdown_table();
    let expected_block = format!("{BEGIN}\n{table}{END}");
    let expected_pointer_type_golden = render_pointer_type_golden_block(ivm::PointerType::all())
        .unwrap_or_else(|error| panic!("render pointer type golden: {error}"));

    if !write && !check {
        eprintln!("usage: --write or --check");
        return;
    }

    process(&path_pointer, BEGIN, END, &expected_block, write, check);
    process(&path_ivm_md, BEGIN, END, &expected_block, write, check);
    let localized_paths = localized_pointer_doc_paths(&workspace_root)
        .unwrap_or_else(|error| panic!("discover localized pointer documents: {error}"));
    for path in localized_paths {
        process(&path, BEGIN, END, &expected_block, write, check);
    }
    process(
        &path_pointer_type_golden,
        POINTER_TYPE_GOLDEN_BEGIN,
        POINTER_TYPE_GOLDEN_END,
        &expected_pointer_type_golden,
        write,
        check,
    );
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::{
        BEGIN, END, POINTER_TYPE_GOLDEN_BEGIN, POINTER_TYPE_GOLDEN_END,
        localized_pointer_doc_paths, render_generated_block, render_pointer_type_golden_block,
    };

    static NEXT_TEMP_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn generated_block_replacement_preserves_localized_surroundings() {
        let prefix = "---\nlang: ar\ndirection: rtl\n---\n\n<div dir=\"rtl\">\n\nمقدمة\n\n";
        let suffix = "\n\nخاتمة\n\n</div>\n";
        let current = format!("{prefix}{BEGIN}\nstale\n{END}{suffix}");
        let expected_block = format!("{BEGIN}\ncanonical\n{END}");
        let expected = format!("{prefix}{expected_block}{suffix}");

        let rendered = render_generated_block(&current, BEGIN, END, &expected_block)
            .expect("replace generated block");
        assert_eq!(rendered, expected);
        assert_eq!(
            render_generated_block(&rendered, BEGIN, END, &expected_block)
                .expect("idempotent replacement"),
            rendered
        );
    }

    #[test]
    fn pointer_type_golden_rendering_is_owned_and_idempotent() {
        let expected_block = render_pointer_type_golden_block(&[
            ivm::PointerType::AccountId,
            ivm::PointerType::AssetDefinitionId,
        ])
        .expect("render pointer type golden");
        assert!(expected_block.contains("P::AccountId"));
        assert!(expected_block.contains("0x0002"));

        let prefix = "fn test() {\n";
        let suffix = "\n}\n";
        let current = format!(
            "{prefix}{POINTER_TYPE_GOLDEN_BEGIN}\nstale\n{POINTER_TYPE_GOLDEN_END}{suffix}"
        );
        let rendered = render_generated_block(
            &current,
            POINTER_TYPE_GOLDEN_BEGIN,
            POINTER_TYPE_GOLDEN_END,
            &expected_block,
        )
        .expect("replace pointer type golden");
        assert_eq!(
            render_generated_block(
                &rendered,
                POINTER_TYPE_GOLDEN_BEGIN,
                POINTER_TYPE_GOLDEN_END,
                &expected_block,
            )
            .expect("idempotent pointer type golden replacement"),
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
