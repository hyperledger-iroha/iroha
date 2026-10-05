//! Source checks for the current-path inventory in `specs/ram_lfe_execution_proof.md`.
//!
//! The inventory names the symbols that refuse, sign or attest today. These
//! tests fail when a named symbol leaves its cited file or when a stated call
//! order changes, so the table cannot drift from the source silently.

use super::{RamLfeBackend, RamLfeError};
use std::path::PathBuf;

const MARKER: &str = "<!-- ram-lfe-current-paths -->";
/// Stable refusal code of the SDK input-encryption helpers.
const REFUSAL_CODE: &str = "ram_lfe_encryption_unavailable";

fn repository_file(path: &str) -> String {
    let full = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path);
    std::fs::read_to_string(&full)
        .unwrap_or_else(|error| panic!("inventory cites {path}, which cannot be read: {error}"))
}

fn backticked(cell: &str) -> Vec<&str> {
    cell.split('`').skip(1).step_by(2).collect()
}

/// One inventory row: the symbols it names, the file it cites and its statement.
struct Row<'a> {
    symbols: Vec<&'a str>,
    file: &'a str,
    statement: &'a str,
}

fn inventory(document: &str) -> Vec<Row<'_>> {
    let start = document
        .find(MARKER)
        .expect("specification carries the current-path inventory")
        + MARKER.len();
    let rows: Vec<Row<'_>> = document[start..]
        .lines()
        .skip_while(|line| !line.starts_with('|'))
        .take_while(|line| line.starts_with('|'))
        // Header and separator.
        .skip(2)
        .map(|line| {
            let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
            assert_eq!(cells.len(), 3, "inventory row has three cells: {line}");
            let files = backticked(cells[1]);
            assert_eq!(files.len(), 1, "inventory row cites one file: {line}");
            Row {
                symbols: backticked(cells[0]),
                file: files[0],
                statement: cells[2],
            }
        })
        .collect();
    assert!(!rows.is_empty(), "inventory has rows");
    rows
}

fn specification() -> String {
    repository_file("specs/ram_lfe_execution_proof.md")
}

#[test]
fn every_inventory_symbol_exists_in_its_cited_file() {
    let document = specification();
    let rows = inventory(&document);
    let mut checked = 0;
    for row in &rows {
        assert!(
            !row.symbols.is_empty(),
            "row for {} names a symbol",
            row.file
        );
        assert!(!row.statement.is_empty());
        let source = repository_file(row.file);
        for symbol in &row.symbols {
            for segment in symbol.split("::") {
                assert!(
                    source.contains(segment),
                    "{} no longer contains `{segment}` of `{symbol}`",
                    row.file
                );
            }
            checked += 1;
        }
    }
    // Every component the inventory covers has at least one row.
    for prefix in [
        "crates/iroha_crypto/",
        "crates/iroha_data_model/",
        "crates/iroha_core/",
        "crates/iroha_torii/",
        "IrohaSwift/",
        "kotlin/",
        "java/",
        "javascript/",
        "python/",
        "csharp/",
    ] {
        assert!(
            rows.iter().any(|row| row.file.starts_with(prefix)),
            "inventory has no row under {prefix}"
        );
    }
    assert!(checked >= rows.len());
}

#[test]
fn inventory_links_the_machine_readable_consumer_inventory() {
    // The complete consumer inventory belongs to another task. Its checker
    // names the file it validates; the specification must link that file.
    let checker = repository_file("scripts/check_fhe_ownership_map.py");
    let path = checker
        .lines()
        .find_map(|line| line.strip_prefix("DEFAULT_MAP = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .expect("the ownership checker declares its default inventory");
    let link = path
        .strip_prefix("specs/")
        .expect("the inventory is a tracked specification file");
    assert!(
        specification().contains(&format!("]({link})")),
        "specification must link the consumer inventory {path}"
    );
    assert!(!repository_file(path).is_empty());
}

#[test]
fn backend_refusal_is_what_the_inventory_states() {
    assert_eq!(
        RamLfeBackend::BfvAffineV1.require_production_support(),
        Err(RamLfeError::InsecureBfvProfile)
    );
    assert_eq!(
        RamLfeBackend::BfvProgrammedV1.require_production_support(),
        Err(RamLfeError::InsecureBfvProfile)
    );
    assert_eq!(
        RamLfeBackend::HkdfSha3_512PrfV1.require_production_support(),
        Ok(())
    );
}

/// Text of one function of the Torii resolver, from its name to the closing
/// brace at the indentation of its `fn` line.
fn method_body<'a>(source: &'a str, name: &str) -> &'a str {
    let start = source
        .find(&format!("fn {name}("))
        .unwrap_or_else(|| panic!("Torii resolver has no `{name}`"));
    let line_start = source[..start].rfind('\n').map_or(0, |newline| newline + 1);
    let indentation: String = source[line_start..]
        .chars()
        .take_while(|character| *character == ' ')
        .collect();
    let body = &source[start..];
    let end = body
        .find(&format!("\n{indentation}}}\n"))
        .unwrap_or_else(|| panic!("`{name}` has no closing brace at its indentation"));
    &body[..end]
}

fn position(body: &str, needle: &str) -> usize {
    body.find(needle)
        .unwrap_or_else(|| panic!("expected `{needle}` in the method body"))
}

#[test]
fn torii_call_order_is_what_the_inventory_states() {
    let source = repository_file("crates/iroha_torii/src/identifier_resolution.rs");

    // The encrypted execution refuses the insecure backends first and then
    // every backend except the programmed one.
    let execute = method_body(&source, "execute_encrypted");
    assert!(
        position(execute, "require_supported_program_policy(")
            < position(execute, "UnsupportedBackend(")
    );
    let supported = method_body(&source, "require_supported_program_policy");
    assert_eq!(supported.matches("require_production_support()").count(), 2);

    // The general identifier path runs the encrypted execution before it
    // looks at the opening.
    let derive = method_body(&source, "derive_encrypted");
    assert!(
        position(derive, "self.execute_encrypted(") < position(derive, "validate_output_opening(")
    );

    // The phone path checks the pinned contract shape first and only then
    // runs the encrypted execution.
    let phone = method_body(&source, "derive_phone_retail_encrypted");
    assert!(
        position(phone, "InvalidPhoneCanonicality(") < position(phone, "self.execute_encrypted(")
    );
    assert!(position(phone, "is_phone_retail()") < position(phone, "self.execute_encrypted("));
}

#[test]
fn sdk_refusals_are_what_the_inventory_states() {
    let document = specification();
    let rows = inventory(&document);
    let sdk_row = |prefix: &str| {
        rows.iter()
            .find(|row| row.file.starts_with(prefix))
            .unwrap_or_else(|| panic!("inventory has no row under {prefix}"))
    };
    // Swift, Kotlin, Java and JavaScript refuse input encryption.
    for prefix in ["kotlin/", "java/", "javascript/"] {
        let row = sdk_row(prefix);
        assert!(
            repository_file(row.file).contains(REFUSAL_CODE),
            "{} no longer refuses with {REFUSAL_CODE}",
            row.file
        );
    }
    assert!(repository_file(sdk_row("IrohaSwift/").file).contains("ramLfeEncryptionUnavailable"));
    // Python and C# carry receipt and policy models and no encryption helper,
    // so the files the inventory cites carry no refusal either.
    for prefix in ["python/", "csharp/"] {
        let row = sdk_row(prefix);
        assert!(
            !repository_file(row.file).contains(REFUSAL_CODE),
            "{} gained an input-encryption refusal; update the inventory",
            row.file
        );
    }
}
