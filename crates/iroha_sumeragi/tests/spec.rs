//! Specification traceability and the §12.6 size gate of `iroha_sumeragi` (spec Appendix E,
//! E39): every `§` reference and every `// SPEC:` marker in the crate resolves in the workspace
//! spec `specs/sumeragi.md`, the spec's quorum table matches the code, and the core stays within
//! the 8 000-line budget (the count per module is printed).

// As in `src/lib.rs`: clippy attributes the workspace member `vendor/concread`'s feature name
// `simd_support` to every crate it checks.
#![allow(clippy::redundant_feature_names)]

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use iroha_sumeragi::types::{fault_threshold, quorum};

/// The §12.6 MUST: the core crate (everything except `sim/` and tests) has at most this many
/// non-blank, non-comment lines.
const CORE_BUDGET: usize = 8_000;

/// The planned §12.6 split: module, budget, files (relative to `src/`).
const MODULES: [(&str, usize, &[&str]); 8] = [
    (
        "types",
        600,
        &["types.rs", "types/control_witness.rs", "bytes.rs"],
    ),
    ("topology", 300, &["topology.rs"]),
    (
        "message",
        800,
        &[
            "message.rs",
            "message/attestation.rs",
            "preimage.rs",
            "crypto.rs",
            "evidence.rs",
        ],
    ),
    ("safety", 700, &["safety.rs", "machine/restart.rs"]),
    ("pacemaker", 400, &["pacemaker.rs"]),
    (
        "core",
        3_000,
        &[
            "machine/mod.rs",
            "machine/intake.rs",
            "machine/control.rs",
            "machine/proposal.rs",
            "machine/propose.rs",
            "machine/round.rs",
            "machine/timeout.rs",
            "machine/timers.rs",
            "machine/votes.rs",
        ],
    ),
    ("sync", 700, &["machine/sync.rs"]),
    ("api", 600, &["api.rs", "lib.rs"]),
];

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The workspace root (`crates/iroha_sumeragi/../..`).
fn workspace_root() -> PathBuf {
    crate_dir()
        .parent()
        .and_then(Path::parent)
        .expect("the crate lives in `crates/` of the workspace")
        .to_path_buf()
}

/// The normative specification, `specs/sumeragi.md`.
fn spec() -> String {
    let path = workspace_root().join("specs/sumeragi.md");
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Every `.rs` file under `dir`, sorted.
fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("read a source directory") {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// `src/`-relative path with `/` separators.
fn relative(path: &Path) -> String {
    let src = crate_dir().join("src");
    let rel = path.strip_prefix(&src).unwrap_or(path);
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// The dotted number at the start of `text` (`"6.9 Catch-up"` → `"6.9"`), if any.
fn leading_number(text: &str) -> Option<String> {
    let number: String = text
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    let number = number.trim_end_matches('.');
    (!number.is_empty() && number.starts_with(|c: char| c.is_ascii_digit()))
        .then(|| number.to_owned())
}

/// What a reference can name: numbered headings, the numbered items of a top-level section
/// without subsections in between (`§1.2` = item 2 of section 1), and the Appendix E rows.
fn anchors(spec: &str) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut sections = BTreeSet::new();
    let mut rows = BTreeSet::new();
    let mut top: Option<String> = None;
    for line in spec.lines() {
        if let Some(rest) = line.strip_prefix("## ") {
            top = leading_number(rest);
            sections.extend(top.clone());
        } else if let Some(rest) = line
            .strip_prefix("### ")
            .or_else(|| line.strip_prefix("#### "))
        {
            top = None;
            sections.extend(leading_number(rest));
        } else if let Some(top) = &top
            && let Some(item) = leading_number(line)
            && !item.contains('.')
            && line[item.len()..].starts_with(". ")
        {
            sections.insert(format!("{top}.{item}"));
        }
        if let Some(rest) = line.strip_prefix("| E") {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            if !digits.is_empty() && rest[digits.len()..].starts_with(" |") {
                rows.insert(format!("E{digits}"));
            }
        }
    }
    (sections, rows)
}

/// The `§` references in `text`.
fn section_refs(text: &str) -> Vec<String> {
    text.split('§').skip(1).filter_map(leading_number).collect()
}

/// The Appendix E rows cited as `Appendix E, E<n>` in `text` (whitespace-normalised).
fn row_refs(text: &str) -> Vec<String> {
    const CITE: &str = "Appendix E, E";
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    text.match_indices(CITE)
        .filter_map(|(at, _)| {
            let rest = &text[at + CITE.len()..];
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            (!digits.is_empty()).then(|| format!("E{digits}"))
        })
        .collect()
}

/// The `// SPEC:` comment blocks of a file: `(line number, text of the block)`.
fn spec_markers(source: &str) -> Vec<(usize, String)> {
    let lines: Vec<&str> = source.lines().collect();
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if !line.trim_start().starts_with("// SPEC:") {
            continue;
        }
        let block: Vec<&str> = lines[i..]
            .iter()
            .take_while(|l| l.trim_start().starts_with("//"))
            .map(|l| l.trim_start().trim_start_matches('/'))
            .collect();
        out.push((i + 1, block.join(" ")));
    }
    out
}

#[test]
fn every_spec_reference_resolves() {
    let spec = spec();
    let title = spec.lines().next().unwrap_or_default();
    assert!(
        title.starts_with("# Sumeragi") && !title.contains("v2"),
        "the spec is the one version of Sumeragi: {title}"
    );
    let (sections, rows) = anchors(&spec);
    assert!(
        sections.contains("7.4") && rows.contains("E1"),
        "spec anchors parsed"
    );
    let mut missing = Vec::new();
    let mut markers = 0;
    for path in rust_files(&crate_dir().join("src")) {
        let source = fs::read_to_string(&path).expect("read a source file");
        let file = relative(&path);
        for reference in section_refs(&source) {
            if !sections.contains(&reference) {
                missing.push(format!("{file}: §{reference}"));
            }
        }
        for row in row_refs(&source) {
            if !rows.contains(&row) {
                missing.push(format!("{file}: Appendix E, {row}"));
            }
        }
        for (line, block) in spec_markers(&source) {
            markers += 1;
            if row_refs(&block).is_empty() {
                missing.push(format!(
                    "{file}:{line}: `// SPEC:` without an Appendix E row"
                ));
            }
        }
    }
    assert!(markers > 0, "the crate has `// SPEC:` markers");
    assert!(
        missing.is_empty(),
        "unresolved references:\n{}",
        missing.join("\n")
    );
}

/// §1.2: `f = floor((n − 1) / 3)` and `q = n − f`; the spec's table of examples matches the code.
#[test]
fn spec_quorum_table_matches_the_code() {
    let spec = spec();
    assert!(
        spec.contains("quorum `q_h = n_h − f_h`"),
        "§1.2 states q = n − f"
    );
    let start = spec.find("n=4→").expect("the §1.2 examples");
    let examples = &spec[start..];
    let examples = &examples[..examples.find("Two quorums").expect("the §1.2 examples end")];
    let examples: String = examples.split_whitespace().collect();
    let mut checked = 0;
    for part in examples.split("n=").skip(1) {
        let n: String = part.chars().take_while(char::is_ascii_digit).collect();
        let Some(pair) = part.split_once("→(").and_then(|(_, p)| p.split_once(')')) else {
            continue;
        };
        let numbers: Vec<usize> = pair
            .0
            .split(',')
            .map(|x| x.trim_start_matches("f=").trim_start_matches("q=").trim())
            .map(|x| x.parse().expect("a number"))
            .collect();
        let n: usize = n.parse().expect("n");
        assert_eq!(
            numbers,
            vec![fault_threshold(n), quorum(n)],
            "n = {n}: (f, q) in the spec vs the code"
        );
        checked += 1;
    }
    assert!(checked >= 5, "the example table was parsed ({checked})");
}

/// Non-blank, non-comment lines of `source`, without an inline `#[cfg(test)] mod tests { … }`
/// at its end.
fn core_lines(source: &str) -> usize {
    let lines: Vec<&str> = source.lines().collect();
    let end = lines
        .windows(2)
        .position(|pair| {
            let next = pair[1].trim();
            pair[0].trim() == "#[cfg(test)]" && next.starts_with("mod tests") && next.ends_with('{')
        })
        .unwrap_or(lines.len());
    lines[..end]
        .iter()
        .filter(|line| {
            let line = line.trim();
            !line.is_empty() && !line.starts_with("//")
        })
        .count()
}

/// §12.6: the core stays within its line budget. The count per module is printed with its
/// planned share (over-plan modules are reported, not failed; see Appendix E, E39).
#[test]
fn core_size_budget() {
    let mut total = 0;
    let mut unassigned = Vec::new();
    let mut per_module = vec![0usize; MODULES.len()];
    for path in rust_files(&crate_dir().join("src")) {
        let file = relative(&path);
        if file.starts_with("sim/")
            || file.starts_with("machine/tests/")
            || matches!(file.as_str(), "testing.rs" | "evidence/tests.rs")
        {
            continue;
        }
        let count = core_lines(&fs::read_to_string(&path).expect("read a source file"));
        total += count;
        match MODULES
            .iter()
            .position(|(_, _, files)| files.contains(&file.as_str()))
        {
            Some(module) => per_module[module] += count,
            None => unassigned.push(file),
        }
    }
    for ((name, plan, _), count) in MODULES.iter().zip(&per_module) {
        let note = if count > plan {
            "  (over the plan)"
        } else {
            ""
        };
        println!("{name:10} {count:5} / {plan:5}{note}");
    }
    println!("{:10} {total:5} / {CORE_BUDGET:5}", "total");
    assert!(
        unassigned.is_empty(),
        "assign these files to a §12.6 module: {unassigned:?}"
    );
    assert!(
        total <= CORE_BUDGET,
        "the core has {total} lines, above the §12.6 budget of {CORE_BUDGET}"
    );
}
