//! Generate or check the generated pointer-ABI sections in docs.
//! Usage:
//!   cargo run -p ivm --bin gen_pointer_types_doc -- --write
//!   cargo run -p ivm --bin gen_pointer_types_doc -- --check
//!   cargo run -p ivm --bin gen_pointer_types_doc -- --write --root /tmp/ivm-doc-stage

use std::path::PathBuf;

mod support;

use support::{GeneratedOutput, parse_generation_options, sync_generated_outputs};

const BEGIN: &str = "<!-- BEGIN GENERATED POINTER TYPES -->";
const END: &str = "<!-- END GENERATED POINTER TYPES -->";
const POINTER_TYPE_GOLDEN_BEGIN: &str = "    // BEGIN GENERATED ABI V1 POINTER TYPE IDS";
const POINTER_TYPE_GOLDEN_END: &str = "    // END GENERATED ABI V1 POINTER TYPE IDS";

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

fn prepare_generated_block_outputs(
    paths: &[PathBuf],
    begin_marker: &str,
    end_marker: &str,
    expected_block: &str,
) -> Result<Vec<GeneratedOutput>, String> {
    paths
        .iter()
        .map(|path| {
            GeneratedOutput::render(path, |text| {
                render_generated_block(text, begin_marker, end_marker, expected_block)
            })
        })
        .collect()
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn main() {
    let options = match parse_generation_options(std::env::args().skip(1), workspace_root()) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let manifest_dir = options.root.join("crates/ivm");
    let path_pointer = manifest_dir.join("docs/pointer_abi.md");
    let path_ivm_md = options.root.join("ivm.md");
    let path_pointer_type_golden = manifest_dir.join("tests/pointer_type_ids_golden.rs");

    // Render expected table
    let table = ivm::render_pointer_types_markdown_table();
    let expected_block = format!("{BEGIN}\n{table}{END}");
    let expected_pointer_type_golden = render_pointer_type_golden_block(ivm::PointerType::all())
        .unwrap_or_else(|error| panic!("render pointer type golden: {error}"));

    let document_paths = vec![path_pointer, path_ivm_md];
    let mut outputs = prepare_generated_block_outputs(&document_paths, BEGIN, END, &expected_block)
        .unwrap_or_else(|error| panic!("render pointer documents: {error}"));
    outputs.extend(
        prepare_generated_block_outputs(
            &[path_pointer_type_golden],
            POINTER_TYPE_GOLDEN_BEGIN,
            POINTER_TYPE_GOLDEN_END,
            &expected_pointer_type_golden,
        )
        .unwrap_or_else(|error| panic!("render pointer type golden: {error}")),
    );
    let regenerate_command = "cargo run --locked -p ivm --bin gen_pointer_types_doc -- --write";
    let updated = sync_generated_outputs(&outputs, options.mode, regenerate_command)
        .unwrap_or_else(|error| panic!("{error}"));
    for path in updated {
        eprintln!("updated: {}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::{
        BEGIN, END, POINTER_TYPE_GOLDEN_BEGIN, POINTER_TYPE_GOLDEN_END,
        prepare_generated_block_outputs, render_generated_block, render_pointer_type_golden_block,
    };

    static NEXT_TEMP_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn generated_block_replacement_preserves_surrounding_prose() {
        let prefix = "# Pointer ABI\n\nIntroduction.\n\n";
        let suffix = "\n\nAdditional notes.\n";
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
        assert!(render_generated_block("no markers", BEGIN, END, &expected_block).is_err());
        assert!(
            render_generated_block(&format!("{END}\n{BEGIN}"), BEGIN, END, &expected_block)
                .is_err()
        );
        assert!(
            render_generated_block(
                &format!("{BEGIN}\none\n{END}\n{BEGIN}\ntwo\n{END}"),
                BEGIN,
                END,
                &expected_block,
            )
            .is_err()
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
    fn late_marker_failure_does_not_publish_earlier_document() {
        let unique = NEXT_TEMP_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "ivm-pointer-doc-late-failure-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create test directory");
        let first = root.join("first.md");
        let second = root.join("second.md");
        fs::write(&first, format!("{BEGIN}\nstale\n{END}\n")).expect("write first document");
        fs::write(&second, "missing markers\n").expect("write malformed later document");
        let before = fs::read(&first).expect("snapshot first document");
        let expected = format!("{BEGIN}\ncurrent\n{END}");

        assert!(
            prepare_generated_block_outputs(&[first.clone(), second], BEGIN, END, &expected)
                .is_err()
        );
        assert_eq!(
            fs::read(&first).expect("read first after late failure"),
            before
        );

        fs::remove_dir_all(root).expect("remove temporary directory");
    }
}
