//! UI tests for norito_derive (proc-macros).

use std::{
    env, fs,
    path::{Component, Path, PathBuf},
    process::Command,
};

#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/pass/*.rs");
    t.compile_fail("tests/ui/fail/*.rs");

    scrub_trybuild_manifest_paths();
}

fn scrub_trybuild_manifest_paths() {
    // trybuild emits absolute paths; rewrite them so target-codex artifacts stay portable.
    let Some(target_dir) = cargo_target_dir() else {
        return;
    };
    let crate_name = env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "norito_derive".to_owned());
    let manifest_dir = target_dir.join("tests").join("trybuild").join(crate_name);
    let manifest_path = manifest_dir.join("Cargo.toml");
    let Ok(contents) = fs::read_to_string(&manifest_path) else {
        return;
    };
    let Some(updated) = rewrite_manifest_paths(&contents, &manifest_dir) else {
        return;
    };
    let _ = fs::write(manifest_path, updated);
}

fn cargo_target_dir() -> Option<PathBuf> {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .args(["metadata", "--no-deps", "--format-version=1", "--offline"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value: norito::json::Value = norito::json::from_slice(&output.stdout).ok()?;
    let target_dir = value.get("target_directory")?.as_str()?;
    Some(PathBuf::from(target_dir))
}

fn rewrite_manifest_paths(contents: &str, manifest_dir: &Path) -> Option<String> {
    let mut changed = false;
    let mut out = String::with_capacity(contents.len());
    for chunk in contents.split_inclusive('\n') {
        let (line, newline) = match chunk.strip_suffix('\n') {
            Some(line) => (line, "\n"),
            None => (chunk, ""),
        };
        let (rewritten, line_changed) = rewrite_path_line(line, manifest_dir);
        if line_changed {
            changed = true;
        }
        out.push_str(&rewritten);
        out.push_str(newline);
    }
    changed.then_some(out)
}

fn rewrite_path_line(line: &str, manifest_dir: &Path) -> (String, bool) {
    let indent_len = line.len() - line.trim_start().len();
    let indent = &line[..indent_len];
    let trimmed = &line[indent_len..];
    let after_key = match trimmed.strip_prefix("path") {
        Some(rest) => rest,
        None => return (line.to_owned(), false),
    };
    let after_eq = match after_key.trim_start().strip_prefix('=') {
        Some(rest) => rest.trim_start(),
        None => return (line.to_owned(), false),
    };
    let (value, rest) = match parse_basic_string(after_eq) {
        Some(parsed) => parsed,
        None => return (line.to_owned(), false),
    };
    let path = PathBuf::from(&value);
    if !path.is_absolute() {
        return (line.to_owned(), false);
    }
    let relative = match make_relative_path(manifest_dir, &path) {
        Some(path) => path,
        None => return (line.to_owned(), false),
    };
    let escaped = escape_basic_string(&relative.to_string_lossy());
    let rewritten = format!("{indent}path = \"{escaped}\"{rest}");
    (rewritten, true)
}

fn parse_basic_string(input: &str) -> Option<(String, &str)> {
    let mut chars = input.char_indices();
    let (_, first) = chars.next()?;
    if first != '"' {
        return None;
    }
    let mut out = String::new();
    let mut escaped = false;
    for (idx, ch) in chars {
        if escaped {
            out.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => return Some((out, &input[idx + ch.len_utf8()..])),
            _ => out.push(ch),
        }
    }
    None
}

fn escape_basic_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            _ => out.push(ch),
        }
    }
    out
}

fn make_relative_path(from: &Path, to: &Path) -> Option<PathBuf> {
    let from_components: Vec<Component<'_>> = from.components().collect();
    let to_components: Vec<Component<'_>> = to.components().collect();
    let first_from = from_components.first()?;
    let first_to = to_components.first()?;
    if first_from != first_to {
        return None;
    }
    let mut common = 0;
    while common < from_components.len()
        && common < to_components.len()
        && from_components[common] == to_components[common]
    {
        common += 1;
    }
    let mut result = PathBuf::new();
    for _ in common..from_components.len() {
        result.push("..");
    }
    for component in &to_components[common..] {
        result.push(component.as_os_str());
    }
    Some(result)
}

#[test]
fn relative_path_shared_prefix() {
    let base = env::temp_dir().join("norito_derive_trybuild_paths");
    let from = base
        .join("target")
        .join("tests")
        .join("trybuild")
        .join("norito_derive");
    let to = base
        .join("crates")
        .join("norito_derive")
        .join("tests")
        .join("ui")
        .join("pass")
        .join("example.rs");
    let relative = make_relative_path(&from, &to).expect("relative path");
    let mut expected = PathBuf::new();
    expected.push("..");
    expected.push("..");
    expected.push("..");
    expected.push("..");
    expected.push("crates");
    expected.push("norito_derive");
    expected.push("tests");
    expected.push("ui");
    expected.push("pass");
    expected.push("example.rs");
    assert_eq!(relative, expected);
}

#[test]
fn rewrite_manifest_paths_scrubs_absolute_paths() {
    let base = env::temp_dir().join("norito_derive_trybuild_manifest");
    let manifest_dir = base
        .join("target")
        .join("tests")
        .join("trybuild")
        .join("norito_derive");
    let dep_path = base.join("crates").join("foo");
    let bin_path = base
        .join("crates")
        .join("norito_derive")
        .join("tests")
        .join("ui")
        .join("pass")
        .join("example.rs");
    let contents = format!(
        "[dependencies.foo]\npath = \"{}\"\n\n[[bin]]\nname = \"trybuild000\"\npath = \"{}\"\n",
        escape_basic_string(&dep_path.to_string_lossy()),
        escape_basic_string(&bin_path.to_string_lossy()),
    );
    let updated = rewrite_manifest_paths(&contents, &manifest_dir).expect("expected rewrite");
    assert!(!updated.contains(dep_path.to_string_lossy().as_ref()));
    assert!(!updated.contains(bin_path.to_string_lossy().as_ref()));
}
