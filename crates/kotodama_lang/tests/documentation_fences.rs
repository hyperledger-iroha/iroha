//! Compile every canonical documentation fence that claims to contain Kotodama.
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};
use kotodama_lang::lexer::{TokenKind, lex};
use kotodama_lang::session::{CompileRequest, CompilerSession};
fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("kotodama_lang must live under crates/")
        .to_path_buf()
}
fn markdown_files(root: &Path, output: &mut Vec<PathBuf>) {
    let mut entries = fs::read_dir(root)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", root.display()))
        .map(|entry| entry.expect("documentation directory entry").path())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            markdown_files(&path, output);
        } else if path.extension().is_some_and(|extension| extension == "md") {
            output.push(path);
        }
    }
}
fn kotodama_fences(path: &Path) -> Vec<(usize, String)> {
    let source = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    let mut fences = Vec::new();
    let mut open = None;
    let mut body = String::new();
    for (index, line) in source.lines().enumerate() {
        if let Some(start_line) = open {
            if line.trim() == "```" {
                fences.push((start_line, std::mem::take(&mut body)));
                open = None;
            } else {
                body.push_str(line);
                body.push('\n');
            }
        } else if line.trim() == "```kotodama" {
            open = Some(index + 2);
        }
    }
    assert!(
        open.is_none(),
        "unterminated Kotodama fence in {}",
        path.display()
    );
    fences
}
#[test]
fn canonical_documentation_kotodama_fences_compile() {
    let root = repository_root();
    let mut markdown = Vec::new();
    for relative in ["specs", "crates/ivm/docs"] {
        markdown_files(&root.join(relative), &mut markdown);
    }
    let session = CompilerSession::default();
    let mut fence_count = 0usize;
    let mut compiled_sources = BTreeSet::new();
    for path in markdown {
        for (line, source) in kotodama_fences(&path) {
            fence_count += 1;
            if !compiled_sources.insert(source.clone()) {
                continue;
            }
            let relative = path.strip_prefix(&root).unwrap_or(&path);
            let source_name = format!("{}:{line}", relative.display());
            let request = || CompileRequest {
                source: &source,
                source_name: Some(&source_name),
            };
            if let Err(diagnostics) = session.check(request()) {
                panic!(
                    "Kotodama documentation fence failed to check:\n{}",
                    diagnostics.render_human()
                );
            }
            let deployable = lex(&source)
                .expect("a successfully checked documentation fence must lex")
                .first()
                .is_some_and(|token| matches!(token.kind, TokenKind::Seiyaku));
            if deployable && let Err(diagnostics) = session.build(request()) {
                panic!(
                    "Kotodama documentation contract failed to build:\n{}",
                    diagnostics.render_human()
                );
            }
        }
    }
    assert!(
        fence_count > 0,
        "documentation scan found no Kotodama fences"
    );
    assert!(
        !compiled_sources.is_empty(),
        "documentation scan found no unique Kotodama programs"
    );
}
