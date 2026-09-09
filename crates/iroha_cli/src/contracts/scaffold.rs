//! Offline, atomic creation of a first Kotodama project.
use super::{DEFAULT_CONTRACT_GAS_LIMIT, RunContext};
use eyre::{Result, WrapErr as _, eyre};
use ivm::kotodama::{
    compiler::CompilerOptions,
    formatter::format_source,
    lexer::{TokenKind, lex},
    semantic::is_reserved_source_declaration,
    session::{CompileRequest, CompilerSession},
    source::{FrontendBudget, SourceFile, SourceId},
};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

/// Arguments for creating a complete local contract project.
#[derive(clap::Args, Debug)]
pub struct DevNewArgs {
    /// New or empty directory that will contain the project.
    pub directory: PathBuf,
    /// Source-level seiyaku name using the canonical identifier grammar.
    #[arg(long, default_value = "Counter")]
    pub name: String,
}
impl DevNewArgs {
    pub(super) fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        create_project(&self.directory, &self.name)?;
        context.print_data(&norito::json!({
            "directory": (self.directory.display().to_string()),
            "name": (self.name),
            "next": "iroha contract dev check",
        }))
    }
}

fn validate_name(name: &str) -> Result<()> {
    let tokens = lex(name).map_err(|error| eyre!(error))?;
    if !matches!(tokens.as_slice(), [token, end]
        if matches!(&token.kind, TokenKind::Ident(value) if value == name)
            && end.kind == TokenKind::EOF)
        || is_reserved_source_declaration(name, false)
    {
        return Err(eyre!(
            "`{name}` is not an available Kotodama declaration name"
        ));
    }
    Ok(())
}

fn create_project(directory: &Path, name: &str) -> Result<()> {
    publish_project_files(directory, project_files(name)?)
}

fn publish_project_files(directory: &Path, files: BTreeMap<PathBuf, String>) -> Result<()> {
    publish_project_files_with(directory, files, |from, to| fs::rename(from, to))
}

fn publish_project_files_with(
    directory: &Path,
    files: BTreeMap<PathBuf, String>,
    publish: impl FnOnce(&Path, &Path) -> std::io::Result<()>,
) -> Result<()> {
    match fs::symlink_metadata(directory) {
        Ok(metadata) => {
            if !metadata.is_dir()
                || metadata.file_type().is_symlink()
                || fs::read_dir(directory)?.next().is_some()
            {
                return Err(eyre!(
                    "project destination must be a new or empty directory: {}",
                    directory.display()
                ));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).wrap_err("inspect the Kotodama project destination"),
    }
    let parent = directory
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let staging = tempfile::Builder::new()
        .prefix(".kotodama-new-")
        .tempdir_in(parent)
        .wrap_err("create staged Kotodama project beside the destination")?;
    for (relative, contents) in &files {
        let path = staging.path().join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, contents)?;
    }
    validate_project(staging.path())?;
    // Rename replaces an empty directory atomically where the filesystem supports it.
    // Never remove the destination first: a failed rename must leave it intact.
    publish(staging.path(), directory).wrap_err(
        "publish the complete Kotodama project atomically; if the filesystem cannot replace an empty directory, choose a destination that does not exist",
    )?;
    Ok(())
}

fn validate_project(directory: &Path) -> Result<()> {
    // Check the exact staged graph and execute every declared standalone suite before
    // publishing any project file. The fixture runner writes no artifacts or credentials.
    let graph = ivm::kotodama::driver::load_source_project_manifest(
        &directory.join("kotodama.project.json"),
    )?;
    ivm::kotodama::driver::BuildDriver::new(CompilerSession::default(), "kotodama-new")
        .check_project(graph.graph)?;
    let manifest = super::load_contract_app_manifest(&directory.join("iroha.contracts.toml"))?;
    if manifest.tests.is_empty() {
        return Err(eyre!("generated project has no declared standalone tests"));
    }
    for test in &manifest.tests {
        let request = ivm::koto_test_driver::KotoTestRunRequestV1::new(
            directory.join(&test.path),
            CompilerOptions::default().chain_discriminant,
        );
        // A standalone koto_test input discovers only its explicitly named contract,
        // never additional suites from ambient sibling directories.
        let report = ivm::koto_test_driver::run_tests_structured_v1(&request)
            .wrap_err_with(|| format!("validate generated tests `{}`", test.path.display()))?;
        if report.cases.is_empty() {
            return Err(eyre!(
                "generated tests `{}` executed no test cases",
                test.path.display()
            ));
        }
        if !report.is_success() {
            let failures = report
                .cases
                .iter()
                .filter(|case| !case.passed)
                .map(|case| {
                    format!(
                        "{}: {}",
                        case.name,
                        case.failure.as_deref().unwrap_or("test failed")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            return Err(eyre!(
                "generated tests `{}` failed:\n{failures}",
                test.path.display()
            ));
        }
    }
    Ok(())
}

fn project_files(name: &str) -> Result<BTreeMap<PathBuf, String>> {
    validate_name(name)?;
    let source = format!(
        r#"seiyaku {name} {{
    error enum {name}Error {{ ZeroAmount = 1, }}
    state int total;
    hajimari() {{ total = 0; }}
    kotoage fn add(int amount) authorize("CanInvokeContractEntrypoint") {{
        require(amount > 0, {name}Error::ZeroAmount);
        total = total + amount;
    }}
    view fn value() -> int {{ return total; }}
}}
"#
    );
    CompilerSession::new(CompilerOptions::default()).check(CompileRequest {
        source: &source,
        source_name: Some("contracts/seiyaku.ko"),
    })?;
    // This public identity belongs solely to deterministic in-process fixtures. No signing
    // material or runtime client configuration is written into the generated project.
    let fixture_account = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";
    let tests = format!(
        r#"module {name}Tests {{
    koto_test {{ target: "../contracts/seiyaku.ko" }}
    fixture permitted {{
        actor("app", AccountId::parse("{fixture_account}"));
        grant_seiyaku_kotoage_permission("app", "add");
    }}
    fixture unpermitted {{
        actor("app", AccountId::parse("{fixture_account}"));
    }}
    #[test(fixture = "permitted")]
    fn adds_to_total() {{
        test::invoke_kotoage_as(actor: "app", kotoage: "hajimari", arguments: json {{}});
        test::invoke_kotoage_as(actor: "app", kotoage: "add", arguments: json {{ amount: 3 }});
        test::assert_eq(actual: total, expected: 3);
    }}
    #[test(fixture = "permitted")]
    fn rejects_zero_amount() {{
        test::expect_reject_as(actor: "app", kotoage: "add", arguments: json {{ amount: 0 }}, expected: {name}Error::ZeroAmount);
    }}
    #[test(fixture = "unpermitted")]
    fn requires_invocation_permission() {{
        test::expect_reject_as(actor: "app", kotoage: "add", arguments: json {{ amount: 1 }}, expected: test::Rejection::PermissionDenied);
    }}
}}
"#
    );
    let source = format_source(
        &SourceFile::new(SourceId(0), "contracts/seiyaku.ko", source),
        FrontendBudget::v1(),
    )?;
    let tests = format_source(
        &SourceFile::new(SourceId(1), "tests/seiyaku.test.ko", tests),
        FrontendBudget::v1(),
    )?;
    let project = norito::json::to_string_pretty(&norito::json!({
        "version": 1, "root": "contracts/seiyaku.ko", "imports": [], "packages": [],
    }))?;
    let app = format!(
        r#"bundle_name = "app"
default_dataspace = "universal"

[profiles.local]
default_gas_limit = {DEFAULT_CONTRACT_GAS_LIMIT}

[[contracts]]
name = "app"
alias = "app"
kotodama_project = "kotodama.project.json"
artifact = "target/kotodama/local/seiyaku.to"

[[tests]]
path = "tests/seiyaku.test.ko"
"#
    );
    let readme = format!(
        r#"# {name}

This project contains a Kotodama `seiyaku` (誓約), its `hajimari` (始まり)
initializer, an authorized `kotoage` (言挙げ), a view, and local tests.

From this directory, run:

```sh
iroha contract dev check
iroha contract dev schema
koto fmt --check contracts/seiyaku.ko tests/seiyaku.test.ko
```

The tests use an in-process fixture account and require no network or client keys.
Generated artifacts are under `target/`. Edit `kotodama.project.json` to declare
module dependencies explicitly. The Kotodama editor extension uses this same graph.

For local execution inspection use the existing `iroha contract debug-call` and
`iroha contract debug-view` commands. Follow the
[first contract guide](https://docs.iroha.tech/blockchain/smart-contracts#first-project)
for editor setup, language explanations, and deployment using a separately
configured runtime account.
"#
    );
    Ok(BTreeMap::from([
        ("contracts/seiyaku.ko".into(), source),
        ("tests/seiyaku.test.ko".into(), tests),
        ("kotodama.project.json".into(), format!("{project}\n")),
        ("iroha.contracts.toml".into(), app),
        ("README.md".into(), readme),
        (".gitignore".into(), "/target/\n/client.toml\n/.runtime/\n".to_owned()),
        (".vscode/settings.json".into(), "{\n  \"kotodama.project\": \"${workspaceFolder}/kotodama.project.json\",\n  \"[kotodama]\": { \"editor.defaultFormatter\": \"hyperledger-iroha.kotodama\", \"editor.formatOnSave\": true }\n}\n".to_owned()),
        (".vscode/extensions.json".into(), "{\n  \"recommendations\": [\"hyperledger-iroha.kotodama\"]\n}\n".to_owned()),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_use_the_canonical_lexer_and_reject_injection_and_keywords() {
        for name in ["Counter", "Treasury_2"] {
            validate_name(name).expect("declaration identifier");
        }
        for name in ["seiyaku", "金庫", "x {}", "../escape", "", "int"] {
            assert!(validate_name(name).is_err(), "accepted {name:?}");
        }
    }

    #[test]
    fn scaffold_is_complete_offline_and_never_overwrites_existing_content() {
        let parent = tempfile::tempdir().expect("temporary parent");
        let target = parent.path().join("app");
        create_project(&target, "Counter").expect("create project");
        for path in [
            "contracts/seiyaku.ko",
            "tests/seiyaku.test.ko",
            "kotodama.project.json",
            "iroha.contracts.toml",
            ".vscode/settings.json",
            "README.md",
        ] {
            assert!(target.join(path).is_file(), "missing {path}");
        }
        let manifest =
            super::super::load_contract_app_manifest(&target.join("iroha.contracts.toml"))
                .expect("app manifest");
        assert_eq!(manifest.tests.len(), 1);
        assert!(create_project(&target, "Different").is_err());
        assert!(
            fs::read_to_string(target.join("contracts/seiyaku.ko"))
                .expect("source")
                .contains("seiyaku Counter")
        );
        assert!(!target.join("client.toml").exists());
    }

    #[test]
    fn scaffold_tests_execute_the_generated_contract_and_exact_rejections() {
        let parent = tempfile::tempdir().expect("temporary parent");
        let target = parent.path().join("app");
        create_project(&target, "Counter").expect("create project");
        let report = ivm::koto_test_driver::run_tests_structured_v1(
            &ivm::koto_test_driver::KotoTestRunRequestV1::new(
                target.join("tests/seiyaku.test.ko"),
                CompilerOptions::default().chain_discriminant,
            ),
        )
        .expect("generated tests execute");
        assert_eq!(report.cases.len(), 3);
        assert!(report.is_success(), "{report:?}");
    }

    #[test]
    fn final_publication_failures_preserve_new_and_empty_destinations() {
        for existing in [false, true] {
            let parent = tempfile::tempdir().expect("temporary parent");
            let target = parent.path().join("app");
            if existing {
                fs::create_dir(&target).expect("empty destination");
            }
            let original_permissions = fs::metadata(&target).ok().map(|value| value.permissions());
            let publication_reached = std::cell::Cell::new(false);
            let error = publish_project_files_with(
                &target,
                project_files("Counter").expect("generated files"),
                |staging, destination| {
                    publication_reached.set(true);
                    assert_eq!(destination, target);
                    assert_eq!(destination.exists(), existing);
                    assert!(staging.join("contracts/seiyaku.ko").is_file());
                    assert!(staging.join("tests/seiyaku.test.ko").is_file());
                    Err(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        "injected final publication failure",
                    ))
                },
            )
            .expect_err("a failed final rename must preserve the destination");
            assert!(
                publication_reached.get(),
                "complete validation must precede publication"
            );
            assert!(format!("{error:#}").contains("injected final publication failure"));
            assert_eq!(target.exists(), existing);
            assert_eq!(
                fs::metadata(&target).ok().map(|value| value.permissions()),
                original_permissions
            );
            if existing {
                assert!(
                    fs::read_dir(&target)
                        .expect("empty destination")
                        .next()
                        .is_none()
                );
            }
            assert_eq!(
                fs::read_dir(parent.path()).expect("parent").count(),
                usize::from(existing)
            );
        }
    }

    #[test]
    fn final_publication_preserves_a_destination_populated_after_validation() {
        for existing in [false, true] {
            let parent = tempfile::tempdir().expect("temporary parent");
            let target = parent.path().join("app");
            if existing {
                fs::create_dir(&target).expect("empty destination");
            }
            let error = publish_project_files_with(
                &target,
                project_files("Counter").expect("generated files"),
                |staging, destination| {
                    if !existing {
                        fs::create_dir(destination)?;
                    }
                    fs::write(destination.join("user-work.txt"), "preserve this work")?;
                    fs::rename(staging, destination)
                },
            )
            .expect_err("a competing nonempty destination must prevent publication");
            assert!(format!("{error:#}").contains("publish the complete Kotodama project"));
            assert_eq!(
                fs::read_to_string(target.join("user-work.txt")).expect("competing work"),
                "preserve this work"
            );
            assert_eq!(fs::read_dir(&target).expect("destination").count(), 1);
            assert_eq!(fs::read_dir(parent.path()).expect("parent").count(), 1);
        }
    }

    #[cfg(unix)]
    #[test]
    fn scaffold_atomically_replaces_an_existing_empty_directory() {
        let parent = tempfile::tempdir().expect("temporary parent");
        let target = parent.path().join("app");
        fs::create_dir(&target).expect("empty destination");
        create_project(&target, "Counter").expect("publish into the empty destination");
        assert!(target.join("contracts/seiyaku.ko").is_file());
        assert!(target.join("tests/seiyaku.test.ko").is_file());
        assert_eq!(fs::read_dir(parent.path()).expect("parent").count(), 1);
    }

    #[test]
    fn generated_test_compilation_failures_preserve_new_and_empty_destinations() {
        for existing in [false, true] {
            let parent = tempfile::tempdir().expect("temporary parent");
            let target = parent.path().join("app");
            if existing {
                fs::create_dir(&target).expect("empty destination");
            }
            let mut files = project_files("Counter").expect("generated files");
            let tests = files
                .get_mut(Path::new("tests/seiyaku.test.ko"))
                .expect("standalone tests");
            *tests = tests.replace("test::assert_eq", "test::missing_assertion");
            let error = publish_project_files(&target, files)
                .expect_err("invalid standalone tests must prevent publication");
            let diagnostic = format!("{error:#}");
            assert!(diagnostic.contains("missing_assertion"), "{diagnostic}");
            assert_eq!(target.exists(), existing);
            if existing {
                assert!(
                    fs::read_dir(&target)
                        .expect("empty destination")
                        .next()
                        .is_none()
                );
            }
            assert_eq!(
                fs::read_dir(parent.path()).expect("parent").count(),
                usize::from(existing)
            );
        }
    }

    #[test]
    fn generated_suites_without_test_cases_preserve_new_and_empty_destinations() {
        for existing in [false, true] {
            let parent = tempfile::tempdir().expect("temporary parent");
            let target = parent.path().join("app");
            if existing {
                fs::create_dir(&target).expect("empty destination");
            }
            let mut files = project_files("Counter").expect("generated files");
            let tests = files
                .get_mut(Path::new("tests/seiyaku.test.ko"))
                .expect("standalone tests");
            *tests = tests
                .lines()
                .filter(|line| !line.trim_start().starts_with("#[test("))
                .collect::<Vec<_>>()
                .join("\n");
            let error = publish_project_files(&target, files)
                .expect_err("a declared suite without cases must prevent publication");
            let diagnostic = format!("{error:#}");
            assert!(diagnostic.contains("no #[test]"), "{diagnostic}");
            assert_eq!(target.exists(), existing);
            if existing {
                assert!(
                    fs::read_dir(&target)
                        .expect("empty destination")
                        .next()
                        .is_none()
                );
            }
            assert_eq!(
                fs::read_dir(parent.path()).expect("parent").count(),
                usize::from(existing)
            );
        }
    }

    #[test]
    fn generated_exact_rejection_mismatches_preserve_new_and_empty_destinations() {
        for existing in [false, true] {
            let parent = tempfile::tempdir().expect("temporary parent");
            let target = parent.path().join("app");
            if existing {
                fs::create_dir(&target).expect("empty destination");
            }
            let mut files = project_files("Counter").expect("generated files");
            let tests = files
                .get_mut(Path::new("tests/seiyaku.test.ko"))
                .expect("standalone tests");
            *tests = tests.replace(
                "expected: CounterError::ZeroAmount",
                "expected: test::Rejection::PermissionDenied",
            );
            let error = publish_project_files(&target, files)
                .expect_err("an exact rejection mismatch must prevent publication");
            let diagnostic = format!("{error:#}");
            assert!(diagnostic.contains("rejects_zero_amount"), "{diagnostic}");
            assert!(diagnostic.contains("PermissionDenied"), "{diagnostic}");
            assert!(diagnostic.contains("ZeroAmount"), "{diagnostic}");
            assert_eq!(target.exists(), existing);
            if existing {
                assert!(
                    fs::read_dir(&target)
                        .expect("empty destination")
                        .next()
                        .is_none()
                );
            }
            assert_eq!(
                fs::read_dir(parent.path()).expect("parent").count(),
                usize::from(existing)
            );
        }
    }

    #[test]
    fn scaffold_build_schema_and_format_share_the_offline_project_graph() {
        let parent = tempfile::tempdir().expect("temporary parent");
        let target = parent.path().join("app");
        create_project(&target, "Counter").expect("create project");
        let manifest = target.join("iroha.contracts.toml");
        let lint = super::super::dev_run_lints(&manifest, false).expect("offline check");
        assert_eq!(
            lint.get("ok").and_then(norito::json::Value::as_bool),
            Some(true),
            "{lint:?}"
        );
        let build = super::super::dev_build_manifest(&manifest, "local", false, false)
            .expect("offline build");
        let schema =
            super::super::render_dev_schema_markdown(&manifest, &build).expect("offline schema");
        assert!(schema.contains("add"));
        assert!(schema.contains("amount"));
        assert!(target.join("target/kotodama/local/seiyaku.to").is_file());
        super::super::dev_build_manifest(&manifest, "local", true, false)
            .expect("locked graph and artifact verification");
        for (index, relative) in ["contracts/seiyaku.ko", "tests/seiyaku.test.ko"]
            .iter()
            .enumerate()
        {
            let text = fs::read_to_string(target.join(relative)).expect("generated source");
            let source = SourceFile::new(SourceId(index as u32), *relative, text.clone());
            assert_eq!(
                format_source(&source, FrontendBudget::v1()).expect("canonical format"),
                text
            );
        }
        assert!(!target.join("client.toml").exists());
    }
}
