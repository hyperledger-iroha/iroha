//! Generate or check ABI hash sections and runtime samples in the localized docs.
//! Usage:
//!   cargo run -p ivm --bin gen_abi_hash_doc -- --write
//!   cargo run -p ivm --bin gen_abi_hash_doc -- --check

use std::{fs, path::PathBuf};

const BEGIN: &str = "<!-- BEGIN GENERATED ABI HASHES -->";
const END: &str = "<!-- END GENERATED ABI HASHES -->";
const RUNTIME_HASH_PREFIX: &str = "\"abi_hash_hex\": \"";

fn source_dir() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .join("docs/source")
}

fn header_paths() -> Vec<PathBuf> {
    let source_dir = source_dir();
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

fn runtime_sample_paths() -> Vec<PathBuf> {
    let sample_dir = source_dir().join("samples");
    let mut paths = fs::read_dir(&sample_dir)
        .unwrap_or_else(|error| panic!("read {}: {error}", sample_dir.display()))
        .map(|entry| entry.expect("read docs/source/samples entry").path())
        .filter(|path| {
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                return false;
            };
            name == "runtime_abi_hash.md"
                || (name.starts_with("runtime_abi_hash.") && name.ends_with(".md"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    assert!(
        !paths.is_empty(),
        "no docs/source/samples/runtime_abi_hash*.md files"
    );
    paths
}

fn render_runtime_sample(text: &str, hash: &str) -> Result<String, &'static str> {
    let Some(prefix_start) = text.find(RUNTIME_HASH_PREFIX) else {
        return Err("runtime ABI hash field not found");
    };
    if text[prefix_start + RUNTIME_HASH_PREFIX.len()..].contains(RUNTIME_HASH_PREFIX) {
        return Err("multiple runtime ABI hash fields found");
    }
    let value_start = prefix_start + RUNTIME_HASH_PREFIX.len();
    let Some(relative_end) = text[value_start..].find('"') else {
        return Err("unterminated runtime ABI hash field");
    };
    let value_end = value_start + relative_end;
    let current = &text[value_start..value_end];
    if current.len() != 64 || !current.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("runtime ABI hash is not 32-byte hexadecimal");
    }

    let mut rendered = text.to_owned();
    rendered.replace_range(value_start..value_end, hash);
    Ok(rendered)
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

    let runtime_hash = hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1));
    for path in runtime_sample_paths() {
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        let rendered = render_runtime_sample(&text, &runtime_hash)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        if check {
            assert_eq!(
                text,
                rendered,
                "{} runtime ABI hash out of date; run: cargo run -p ivm --bin gen_abi_hash_doc -- --write",
                path.display()
            );
        } else if write && text != rendered {
            fs::write(&path, rendered)
                .unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
            eprintln!("updated: {}", path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::render_runtime_sample;

    #[test]
    fn runtime_sample_replaces_exactly_one_canonical_hash() {
        let old = "{\n  \"abi_hash_hex\": \"1111111111111111111111111111111111111111111111111111111111111111\"\n}\n";
        let new = "2222222222222222222222222222222222222222222222222222222222222222";
        let rendered = render_runtime_sample(old, new).expect("valid runtime sample");

        assert_eq!(rendered, format!("{{\n  \"abi_hash_hex\": \"{new}\"\n}}\n"));
        assert!(render_runtime_sample("{}", new).is_err());
        assert!(render_runtime_sample(&format!("{old}{old}"), new).is_err());
    }
}
