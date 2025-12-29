//! Test support helpers for parsing `#[model]` modules and collecting model items.
//!
//! These helpers live in tests to avoid pulling heavy dependencies (like `syn`)
//! into the library or build script. They provide deterministic and safe
//! behavior, especially around filesystem traversal where symlinks are
//! explicitly ignored to prevent infinite recursion.

use std::{
    collections::{BTreeMap, VecDeque},
    fs,
    path::Path,
};

/// Collect modules annotated with `#[model]` under `dir` without following symlinks.
#[allow(dead_code)]
pub fn collect_model_items(dir: &Path) -> std::io::Result<BTreeMap<String, Vec<String>>> {
    let mut map = BTreeMap::new();
    let mut queue = VecDeque::from([dir.to_path_buf()]);

    while let Some(current) = queue.pop_front() {
        for entry in fs::read_dir(&current)? {
            let entry = entry?;
            let path = entry.path();
            let meta = fs::symlink_metadata(&path)?;
            let ft = meta.file_type();

            if ft.is_symlink() {
                // Avoid following symlinks to prevent cycles and hangs.
                continue;
            }
            if ft.is_dir() {
                queue.push_back(path);
                continue;
            }
            if !ft.is_file() {
                continue;
            }

            let Ok(src) = fs::read_to_string(&path) else {
                continue;
            };
            if src.contains("#[model]")
                && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
            {
                map.entry(stem.to_string()).or_default();
            }
        }
    }

    Ok(map)
}

/// Parse Rust-like source to find a `#[model] mod` and collect public item names.
///
/// This is a lightweight, best-effort parser sufficient for tests. It avoids
/// pulling `syn` to keep build times short.
#[allow(dead_code)]
pub fn parse_model_module(src: &str) -> Vec<String> {
    // Find the start of a `#[model]` attribute and the following `mod` block.
    let attr_pos = match src.find("#[model]") {
        Some(p) => p,
        None => return vec![],
    };
    let rest = &src[attr_pos..];
    let mod_pos = match rest.find("mod") {
        Some(p) => attr_pos + p,
        None => return vec![],
    };
    // Find the first '{' after `mod` and parse until its matching '}'.
    let brace_start = match src[mod_pos..].find('{') {
        Some(p) => mod_pos + p,
        None => return vec![],
    };
    let mut depth = 0usize;
    let mut end = brace_start;
    for (i, ch) in src[brace_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = brace_start + i;
                    break;
                }
            }
            _ => {}
        }
    }
    let body = &src[brace_start + 1..end];

    // Naively collect identifiers following `pub struct|enum|union|type|const|fn|trait`.
    let mut names = Vec::new();
    for line in body.lines() {
        let l = line.trim_start();
        if !l.starts_with("pub ") {
            continue;
        }
        let after_pub = &l[4..];
        let kinds = [
            ("struct", "struct"),
            ("enum", "enum"),
            ("union", "union"),
            ("type", "type"),
            ("const", "const"),
            ("fn", "fn"),
            ("trait", "trait"),
        ];
        for (kw, _) in kinds {
            if let Some(rest) = after_pub.strip_prefix(kw) {
                // Extract identifier: skip whitespace then take while valid ident char.
                let ident = rest
                    .trim_start()
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect::<String>();
                if !ident.is_empty() {
                    names.push(ident);
                }
                break;
            }
        }
    }
    names
}
