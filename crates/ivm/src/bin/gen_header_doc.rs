//! Generate or check header policy sections in `specs/ivm_header*.md`.
//! Usage:
//!   cargo run -p ivm --bin gen_header_doc -- --write
//!   cargo run -p ivm --bin gen_header_doc -- --check

use std::path::{Path, PathBuf};

mod support;

use support::{
    EXPECTED_DOC_LOCALES, GeneratedOutput, exact_localized_markdown_paths, parse_generation_mode,
    sync_generated_outputs,
};

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
         \x20 - 8..16: `max_cycles: u64` (little‑endian)\n\
         \x20 - 16: `abi_version: u8`\n\
         \x20 - 17..49: `abi_hash: [u8; 32]` (Iroha Hash v1 commitment to the canonical descriptor for `abi_version`)\n",
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

fn header_doc_paths(source_dir: &Path) -> Result<Vec<PathBuf>, String> {
    header_doc_paths_for(source_dir, EXPECTED_DOC_LOCALES)
}

fn header_doc_paths_for(
    source_dir: &Path,
    expected_locales: &[&str],
) -> Result<Vec<PathBuf>, String> {
    exact_localized_markdown_paths(source_dir, "ivm_header", true, expected_locales)
}

fn replace_generated_section(
    text: &str,
    begin_marker: &str,
    end_marker: &str,
    expected: &str,
) -> Result<String, String> {
    let begin_matches = text.match_indices(begin_marker).collect::<Vec<_>>();
    if begin_matches.len() != 1 {
        return Err(format!(
            "expected exactly one begin marker `{begin_marker}`, found {}",
            begin_matches.len()
        ));
    }
    let end_matches = text.match_indices(end_marker).collect::<Vec<_>>();
    if end_matches.len() != 1 {
        return Err(format!(
            "expected exactly one end marker `{end_marker}`, found {}",
            end_matches.len()
        ));
    }
    let begin = begin_matches[0].0;
    let end_start = end_matches[0].0;
    if end_start <= begin {
        return Err(format!(
            "end marker `{end_marker}` precedes begin marker `{begin_marker}`"
        ));
    }
    let end = end_start + end_marker.len();

    let mut rendered = text.to_owned();
    rendered.replace_range(begin..end, expected);
    Ok(rendered)
}

fn render_header_document(
    text: &str,
    include_layout: bool,
    expected_layout: &str,
    expected_policy: &str,
) -> Result<String, String> {
    let rendered = if include_layout {
        replace_generated_section(text, LAYOUT_BEGIN, LAYOUT_END, expected_layout)?
    } else {
        text.to_owned()
    };
    replace_generated_section(&rendered, POLICY_BEGIN, POLICY_END, expected_policy)
}

fn prepare_header_outputs(
    paths: &[PathBuf],
    expected_layout: &str,
    expected_policy: &str,
) -> Result<Vec<GeneratedOutput>, String> {
    paths
        .iter()
        .map(|path| {
            let include_layout =
                path.file_name().and_then(|name| name.to_str()) == Some("ivm_header.md");
            GeneratedOutput::render(path, |text| {
                render_header_document(text, include_layout, expected_layout, expected_policy)
            })
        })
        .collect()
}

fn main() {
    let mode = match parse_generation_mode(std::env::args().skip(1)) {
        Ok(mode) => mode,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let source_dir = PathBuf::from(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join("specs");
    let layout = render_header_layout_markdown();
    let expected_layout = format!("{LAYOUT_BEGIN}\n{layout}{LAYOUT_END}");
    let table = render_header_policy_markdown();
    let expected_policy = format!("{POLICY_BEGIN}\n{table}{POLICY_END}");

    let paths = header_doc_paths(&source_dir)
        .unwrap_or_else(|error| panic!("discover IVM header documents: {error}"));
    let outputs = prepare_header_outputs(&paths, &expected_layout, &expected_policy)
        .unwrap_or_else(|error| panic!("render IVM header documents: {error}"));
    let regenerate_command = "cargo run --locked -p ivm --bin gen_header_doc -- --write";
    let updated = sync_generated_outputs(&outputs, mode, regenerate_command)
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
        LAYOUT_BEGIN, LAYOUT_END, POLICY_BEGIN, POLICY_END, header_doc_paths_for,
        prepare_header_outputs, render_header_document,
    };

    static NEXT_TEMP_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn localized_policy_replacement_preserves_surrounding_prose_and_hashes() {
        let prefix = "---\nlang: ja\n---\n\n翻訳された説明\n\n";
        let suffix = "\n\n<!-- BEGIN GENERATED ABI HASHES -->\nkeep-hash\n<!-- END GENERATED ABI HASHES -->\n\n追記\n";
        let current = format!("{prefix}{POLICY_BEGIN}\nstale\n{POLICY_END}{suffix}");
        let expected_policy = format!("{POLICY_BEGIN}\ncanonical\n{POLICY_END}");
        let expected = format!("{prefix}{expected_policy}{suffix}");

        let rendered = render_header_document(&current, false, "unused", &expected_policy)
            .expect("replace localized policy");
        assert_eq!(rendered, expected);
        assert_eq!(
            render_header_document(&rendered, false, "unused", &expected_policy)
                .expect("idempotent localized replacement"),
            rendered
        );
    }

    #[test]
    fn english_header_replaces_layout_and_policy() {
        let current = format!(
            "intro\n{LAYOUT_BEGIN}\nstale-layout\n{LAYOUT_END}\nmiddle\n{POLICY_BEGIN}\nstale-policy\n{POLICY_END}\ntail\n"
        );
        let expected_layout = format!("{LAYOUT_BEGIN}\nlayout\n{LAYOUT_END}");
        let expected_policy = format!("{POLICY_BEGIN}\npolicy\n{POLICY_END}");
        let expected = format!("intro\n{expected_layout}\nmiddle\n{expected_policy}\ntail\n");

        assert_eq!(
            render_header_document(&current, true, &expected_layout, &expected_policy)
                .expect("replace English generated sections"),
            expected
        );
        assert!(
            render_header_document(
                &format!("{LAYOUT_END}\n{LAYOUT_BEGIN}\n{POLICY_BEGIN}\nstale\n{POLICY_END}"),
                true,
                &expected_layout,
                &expected_policy,
            )
            .is_err()
        );
        assert!(
            render_header_document(
                &format!(
                    "{LAYOUT_BEGIN}\none\n{LAYOUT_END}\n{LAYOUT_BEGIN}\ntwo\n{LAYOUT_END}\n{POLICY_BEGIN}\nstale\n{POLICY_END}"
                ),
                true,
                &expected_layout,
                &expected_policy,
            )
            .is_err()
        );
    }

    #[test]
    fn header_document_discovery_is_sorted() {
        let unique = NEXT_TEMP_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let source_dir = std::env::temp_dir().join(format!(
            "ivm-header-doc-generator-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&source_dir).expect("create source directory");
        for name in ["ivm_header.zh-hant.md", "ivm_header.md", "ivm_header.am.md"] {
            fs::write(source_dir.join(name), "test").expect("write header document");
        }
        fs::write(source_dir.join("ivm_header_notes.md"), "ignore")
            .expect("write unrelated document");

        let paths = header_doc_paths_for(&source_dir, &["am", "zh-hant"])
            .expect("discover header documents");
        assert_eq!(
            paths,
            [
                source_dir.join("ivm_header.am.md"),
                source_dir.join("ivm_header.md"),
                source_dir.join("ivm_header.zh-hant.md"),
            ]
        );

        fs::remove_dir_all(source_dir).expect("remove temporary directory");
    }

    #[test]
    fn late_localized_marker_failure_does_not_publish_earlier_document() {
        let unique = NEXT_TEMP_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let source_dir = std::env::temp_dir().join(format!(
            "ivm-header-doc-late-failure-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&source_dir).expect("create source directory");
        let first = source_dir.join("ivm_header.am.md");
        let second = source_dir.join("ivm_header.zh-hant.md");
        fs::write(&first, format!("{POLICY_BEGIN}\nstale\n{POLICY_END}\n"))
            .expect("write first document");
        fs::write(&second, "missing policy markers\n").expect("write malformed later document");
        let before = fs::read(&first).expect("snapshot first document");
        let expected_policy = format!("{POLICY_BEGIN}\ncurrent\n{POLICY_END}");

        assert!(
            prepare_header_outputs(&[first.clone(), second], "unused", &expected_policy,).is_err()
        );
        assert_eq!(
            fs::read(&first).expect("read first after late failure"),
            before
        );

        fs::remove_dir_all(source_dir).expect("remove temporary directory");
    }
}
