//! Canonical contract and library package scaffolds.
use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
enum PackageTemplate {
    /// A runnable contract and four tests of its public quote entrypoint.
    #[default]
    Contract,
    /// A reusable library with explicitly selected exported declarations.
    Library,
}
#[derive(Args, Clone, Debug)]
struct PackageTemplateArgs {
    /// Package purpose and generated source layout.
    #[arg(long, value_enum, default_value_t)]
    template: PackageTemplate,
    /// Canonical public namespace.
    #[arg(long)]
    namespace: MusubiNamespaceV1,
    /// Override the package name inferred from the directory.
    #[arg(long)]
    name: Option<MusubiPackageNameV1>,
    /// Initial exact version.
    #[arg(long, default_value = "0.1.0")]
    version: MusubiVersionV1,
    /// Source directory for --template library (default: src).
    #[arg(long)]
    source_dir: Option<PortablePath>,
    /// Exported interface name; new source starts with a no-op TODO function.
    /// Repeat as needed, then replace placeholders with the intended functions or types.
    #[arg(long = "export", value_name = "NAME")]
    exports: Vec<Name>,
    /// Bounded package description.
    #[arg(long)]
    description: Option<String>,
    /// Readme file relative to the package root.
    #[arg(long)]
    readme: Option<PortablePath>,
    /// SPDX-like license metadata.
    #[arg(long)]
    license: Option<String>,
    /// License file relative to the package root.
    #[arg(long)]
    license_file: Option<PortablePath>,
    /// Canonical HTTP(S) repository URL.
    #[arg(long)]
    repository: Option<String>,
    /// Canonical lowercase keyword; repeat as needed.
    #[arg(long = "keyword")]
    keywords: Vec<String>,
    /// Positive package include addition; repeat as needed.
    #[arg(long = "include", value_name = "PATH")]
    includes: Vec<PortablePath>,
}
#[derive(Args, Debug)]
pub(super) struct NewArgs {
    /// New package directory. Its parent must already exist.
    #[arg(value_name = "PATH")]
    path: PathBuf,
    #[command(flatten)]
    package: PackageTemplateArgs,
}
#[derive(Args, Debug)]
pub(super) struct InitArgs {
    /// Existing directory to initialize.
    #[arg(default_value = ".", value_name = "PATH")]
    path: PathBuf,
    #[command(flatten)]
    package: PackageTemplateArgs,
    /// Replace an existing regular `Musubi.toml` atomically.
    #[arg(long)]
    force: bool,
}
pub(super) fn run_new(args: &NewArgs) -> CommandResult {
    match fs::symlink_metadata(&args.path) {
        Ok(_) => {
            return Err(
                Diagnostic::new(ErrorCode::Io, "new package destination already exists")
                    .with_context("path", args.path.display().to_string()),
            );
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(io_diagnostic(
                "inspect new package destination",
                &args.path,
                &error,
            ));
        }
    }
    let name = package_name_for_root(&args.path, args.package.name.as_ref())?;
    let manifest = render_package_manifest(&args.package, &name)?;
    let source_dir = library_source_dir(&args.package)?;
    let library_source = (args.package.template == PackageTemplate::Library)
        .then(|| render_package_library(&args.package.exports).map(PackageLibrarySource::Scaffold))
        .transpose()?;
    fs::create_dir(&args.path)
        .map_err(|error| io_diagnostic("create package directory", &args.path, &error))?;
    match library_source {
        Some(source) => initialize_package_files(&args.path, &source_dir, &manifest, &source)?,
        None => initialize_contract_files(&args.path, &name, &manifest)?,
    }
    Ok(Success {
        message: scaffold_message("created", &args.path, args.package.template),
        data: object([
            (
                "manifest",
                Value::from(args.path.join(MANIFEST_FILE_NAME).display().to_string()),
            ),
            (
                "package",
                Value::from(format!("{}/{}", args.package.namespace, name)),
            ),
        ]),
    })
}
pub(super) fn run_init(args: &InitArgs) -> CommandResult {
    let root = canonical_init_root(&args.path)?;
    let manifest_path = root.join(MANIFEST_FILE_NAME);
    if !args.force && fs::symlink_metadata(&manifest_path).is_ok() {
        return Err(Diagnostic::new(
            ErrorCode::ManifestInvalid,
            "package manifest already exists",
        )
        .with_context("path", manifest_path.display().to_string())
        .with_help("pass `--force` to atomically replace a regular manifest"));
    }
    let name = package_name_for_root(&root, args.package.name.as_ref())?;
    let manifest = render_package_manifest(&args.package, &name)?;
    let source_dir = library_source_dir(&args.package)?;
    match args.package.template {
        PackageTemplate::Library => {
            let source =
                prepare_existing_package_library(&root, &source_dir, &args.package.exports)?;
            initialize_package_files(&root, &source_dir, &manifest, &source)?;
        }
        PackageTemplate::Contract => initialize_contract_files(&root, &name, &manifest)?,
    }
    Ok(Success {
        message: scaffold_message("initialized", &args.path, args.package.template),
        data: object([
            ("manifest", Value::from(manifest_path.display().to_string())),
            (
                "package",
                Value::from(format!("{}/{}", args.package.namespace, name)),
            ),
        ]),
    })
}
/// Resolve existing initialization targets before deriving their directory name.
fn canonical_init_root(path: &Path) -> Result<PathBuf, Diagnostic> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| io_diagnostic("inspect package directory", path, &error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(Diagnostic::new(
            ErrorCode::Io,
            "init target must be an existing non-symlink directory",
        )
        .with_context("path", path.display().to_string()));
    }
    fs::canonicalize(path).map_err(|error| io_diagnostic("resolve package directory", path, &error))
}
fn package_name_for_root(
    root: &Path,
    explicit: Option<&MusubiPackageNameV1>,
) -> Result<MusubiPackageNameV1, Diagnostic> {
    if let Some(name) = explicit {
        return Ok(name.clone());
    }
    let raw = root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            Diagnostic::new(
                ErrorCode::Usage,
                "cannot infer a package name from this directory",
            )
            .with_context("path", root.display().to_string())
            .with_help("pass `--name LOWERCASE-KEBAB` explicitly")
        })?;
    raw.parse::<MusubiPackageNameV1>().map_err(|error| {
        Diagnostic::new(ErrorCode::Usage, error.to_string())
            .with_context("inferred_name", raw)
            .with_help("pass `--name LOWERCASE-KEBAB` explicitly")
    })
}
fn render_package_manifest(
    package: &PackageTemplateArgs,
    name: &MusubiPackageNameV1,
) -> Result<String, Diagnostic> {
    let mut output = String::from("manifest-version = 1\n\n[package]\n");
    push_toml_string(&mut output, "namespace", &package.namespace.to_string());
    push_toml_string(&mut output, "name", &name.to_string());
    push_toml_string(&mut output, "version", &package.version.to_string());
    push_toml_string(&mut output, "edition", "1");
    output.push_str("abi-version = 1\n");
    if let Some(value) = &package.description {
        push_toml_string(&mut output, "description", value);
    }
    if let Some(value) = &package.readme {
        push_toml_string(&mut output, "readme", value.as_str());
    }
    if let Some(value) = &package.license {
        push_toml_string(&mut output, "license", value);
    }
    if let Some(value) = &package.license_file {
        push_toml_string(&mut output, "license-file", value.as_str());
    }
    if let Some(value) = &package.repository {
        push_toml_string(&mut output, "repository", value);
    }
    push_toml_array(
        &mut output,
        "keywords",
        package.keywords.iter().map(String::as_str),
    );
    push_toml_array(
        &mut output,
        "include",
        package.includes.iter().map(PortablePath::as_str),
    );
    match package.template {
        PackageTemplate::Library => {
            output.push_str("\n[lib]\n");
            push_toml_string(
                &mut output,
                "source-dir",
                library_source_dir(package)?.as_str(),
            );
            let mut exports = package
                .exports
                .iter()
                .map(AsRef::as_ref)
                .collect::<Vec<_>>();
            exports.sort_unstable();
            exports.dedup();
            push_toml_array(&mut output, "exports", exports);
        }
        PackageTemplate::Contract => {
            library_source_dir(package)?;
            output.push_str("\n[[contract]]\n");
            push_toml_string(&mut output, "name", &name.to_string());
            push_toml_string(&mut output, "path", &format!("contracts/{name}.ko"));
            output.push_str("\n[[test]]\n");
            push_toml_string(&mut output, "name", "quote");
            push_toml_string(&mut output, "path", &format!("tests/{name}.test.ko"));
        }
    }
    parse_manifest(&output)
        .map_err(|error| manifest_diagnostic(Path::new(MANIFEST_FILE_NAME), &error))?;
    Ok(output)
}
fn push_toml_string(output: &mut String, key: &str, value: &str) {
    output.push_str(key);
    output.push_str(" = ");
    output.push_str(&toml_quote(value));
    output.push('\n');
}
fn push_toml_array<I, S>(output: &mut String, key: &str, values: I)
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let values = values
        .into_iter()
        .map(|value| toml_quote(value.as_ref()))
        .collect::<Vec<_>>();
    if values.is_empty() && !matches!(key, "exports") {
        return;
    }
    output.push_str(key);
    output.push_str(" = [");
    output.push_str(&values.join(", "));
    output.push_str("]\n");
}
fn toml_quote(value: &str) -> String {
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    for character in value.chars() {
        match character {
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\\""),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            character => quoted.push(character),
        }
    }
    quoted.push('"');
    quoted
}
pub(super) enum PackageLibrarySource {
    Existing,
    Scaffold(String),
}

fn package_export_names(
    exports: &[Name],
    for_function: bool,
) -> Result<BTreeSet<&str>, Diagnostic> {
    let mut names = BTreeSet::new();
    for name in exports {
        let name = name.as_ref();
        if !iroha_data_model::smart_contract::entrypoint::is_canonical_kotodama_identifier(name)
            || ivm::kotodama::semantic::is_reserved_source_declaration(name, for_function)
        {
            return Err(Diagnostic::new(
                ErrorCode::Usage,
                "export name cannot declare the requested Kotodama interface",
            )
            .with_context("export", name)
            .with_help("use a canonical, non-reserved Kotodama identifier"));
        }
        names.insert(name);
    }
    Ok(names)
}

fn render_package_library(exports: &[Name]) -> Result<String, Diagnostic> {
    let names = package_export_names(exports, true)?;
    let mut source = String::from(
        "// Musubi V1 library source.\n\
         // TODO: Define the library interface and implement its behavior.\n\
         module Library {\n",
    );
    for name in names {
        source.push_str(
            "    // TODO: Replace this no-op function with the intended function or type.\n",
        );
        source.push_str("    fn ");
        source.push_str(name);
        source.push_str("() {}\n");
    }
    source.push_str("}\n");
    Ok(source)
}

pub(super) fn prepare_existing_package_library(
    root: &Path,
    source_dir: &PortablePath,
    exports: &[Name],
) -> Result<PackageLibrarySource, Diagnostic> {
    let library = root.join(source_dir.to_path_buf()).join("lib.ko");
    match fs::symlink_metadata(&library) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
            // Existing exports can be functions or types. Only newly generated function
            // placeholders must avoid names reserved exclusively for functions.
            package_export_names(exports, false)?;
            Ok(PackageLibrarySource::Existing)
        }
        Ok(_) => Err(Diagnostic::new(
            ErrorCode::Io,
            "existing library target is not a regular non-symlink file",
        )
        .with_context("path", library.display().to_string())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            render_package_library(exports).map(PackageLibrarySource::Scaffold)
        }
        Err(error) => Err(io_diagnostic("inspect library source", &library, &error)),
    }
}

pub(super) fn initialize_package_files(
    root: &Path,
    source_dir: &PortablePath,
    manifest: &str,
    library_source: &PackageLibrarySource,
) -> Result<(), Diagnostic> {
    let source_path = root.join(source_dir.to_path_buf());
    fs::create_dir_all(&source_path)
        .map_err(|error| io_diagnostic("create library source directory", &source_path, &error))?;
    let writer = AtomicWriteRoot::new(root).map_err(atomic_diagnostic)?;
    let library = source_dir.to_path_buf().join("lib.ko");
    let library_path = root.join(&library);
    match library_source {
        PackageLibrarySource::Existing => match fs::symlink_metadata(&library_path) {
            Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {}
            Ok(_) => {
                return Err(Diagnostic::new(
                    ErrorCode::Io,
                    "existing library target is not a regular non-symlink file",
                )
                .with_context("path", library_path.display().to_string()));
            }
            Err(error) => {
                return Err(io_diagnostic(
                    "inspect library source",
                    &library_path,
                    &error,
                ));
            }
        },
        PackageLibrarySource::Scaffold(source) => {
            writer
                .install_immutable(&library, source.as_bytes())
                .map_err(atomic_diagnostic)?;
        }
    }
    writer
        .replace(Path::new(MANIFEST_FILE_NAME), manifest.as_bytes())
        .map_err(atomic_diagnostic)
}

fn library_source_dir(package: &PackageTemplateArgs) -> Result<PortablePath, Diagnostic> {
    if package.template == PackageTemplate::Contract
        && (package.source_dir.is_some() || !package.exports.is_empty())
    {
        return Err(Diagnostic::new(
            ErrorCode::Usage,
            "--source-dir and --export require --template library",
        ));
    }
    Ok(package
        .source_dir
        .clone()
        .unwrap_or_else(|| PortablePath::new("src").expect("canonical source directory")))
}
fn scaffold_message(verb: &str, path: &Path, template: PackageTemplate) -> String {
    let kind = match template {
        PackageTemplate::Contract => "contract",
        PackageTemplate::Library => "library",
    };
    let directory = quote_cli_argument(&path.to_string_lossy());
    let mut message = format!(
        "{verb} {kind} package {}\n\nNext:\n  cd {directory}\n  musubi check",
        path.display()
    );
    if template == PackageTemplate::Contract {
        message.push_str("\n  musubi test\n  musubi build");
    }
    message
}
/// The contract template is also the executable coffee example shipped with the repository.
const CONTRACT_SOURCE: &str = include_str!("../templates/contract.ko");
const CONTRACT_TESTS: &str = include_str!("../templates/contract.test.ko");
fn initialize_contract_files(
    root: &Path,
    name: &MusubiPackageNameV1,
    manifest: &str,
) -> Result<(), Diagnostic> {
    let targets = [
        (
            PathBuf::from(format!("contracts/{name}.ko")),
            CONTRACT_SOURCE.to_owned(),
        ),
        (
            PathBuf::from(format!("tests/{name}.test.ko")),
            CONTRACT_TESTS.replace(
                "../contracts/coffee-club.ko",
                &format!("../contracts/{name}.ko"),
            ),
        ),
    ];
    // Capture the creation decision before any file is written. Existing user sources are preserved.
    let prepared = targets
        .into_iter()
        .map(|(path, source)| {
            let physical = root.join(&path);
            let disposition = match fs::symlink_metadata(&physical) {
                Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
                    PackageLibrarySource::Existing
                }
                Ok(_) => {
                    return Err(Diagnostic::new(
                        ErrorCode::Io,
                        "contract scaffold target must be a regular non-symlink file",
                    )
                    .with_context("path", physical.display().to_string()));
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    PackageLibrarySource::Scaffold(source)
                }
                Err(error) => {
                    return Err(io_diagnostic(
                        "inspect contract scaffold source",
                        &physical,
                        &error,
                    ));
                }
            };
            Ok((path, disposition))
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let writer = AtomicWriteRoot::new(root).map_err(atomic_diagnostic)?;
    for (path, disposition) in prepared {
        let parent = root.join(path.parent().expect("scaffold paths have a parent"));
        fs::create_dir_all(&parent)
            .map_err(|error| io_diagnostic("create contract source directory", &parent, &error))?;
        match disposition {
            PackageLibrarySource::Existing => {
                let physical = root.join(&path);
                let metadata = fs::symlink_metadata(&physical).map_err(|error| {
                    io_diagnostic("reinspect contract source", &physical, &error)
                })?;
                if !metadata.is_file() || metadata.file_type().is_symlink() {
                    return Err(Diagnostic::new(
                        ErrorCode::Io,
                        "existing contract source changed file kind",
                    )
                    .with_context("path", physical.display().to_string()));
                }
            }
            PackageLibrarySource::Scaffold(source) => {
                writer
                    .install_immutable(&path, source.as_bytes())
                    .map_err(atomic_diagnostic)?;
            }
        }
    }
    writer
        .replace(Path::new(MANIFEST_FILE_NAME), manifest.as_bytes())
        .map_err(atomic_diagnostic)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn default_init_resolves_the_actual_current_directory() {
        let parsed = Cli::try_parse_from(["musubi", "init", "--namespace", "demo"])
            .expect("default init arguments");
        let Command::Init(args) = parsed.command else {
            panic!("expected init command");
        };
        assert_eq!(args.path, Path::new("."));
        let root = canonical_init_root(&args.path).expect("existing current directory");
        assert_eq!(
            root,
            std::env::current_dir()
                .expect("current directory")
                .canonicalize()
                .expect("canonical current directory")
        );
        assert!(root.file_name().is_some());
    }

    #[test]
    fn init_derives_package_name_from_resolved_directory_without_changing_cwd() {
        let temporary = TempDir::new().expect("init directory");
        let root = temporary.path().join("coffee-club");
        fs::create_dir_all(root.join("child")).expect("existing package directory");
        let path = root.join("child/..");
        let result = invoke([
            OsString::from("musubi"),
            OsString::from("init"),
            path.as_os_str().to_owned(),
            OsString::from("--namespace"),
            OsString::from("demo"),
        ])
        .output
        .render(OutputFormat::Human)
        .expect("init output");
        assert_eq!(result.exit_code(), 0, "{}", result.stderr());
        let manifest = parse_manifest(
            &fs::read_to_string(root.join(MANIFEST_FILE_NAME)).expect("generated manifest"),
        )
        .expect("valid generated manifest");
        assert_eq!(
            manifest.package.expect("package manifest").name.to_string(),
            "coffee-club"
        );
        assert!(root.join("contracts/coffee-club.ko").is_file());
        assert!(root.join("tests/coffee-club.test.ko").is_file());
    }

    #[test]
    fn init_root_resolution_rejects_symlinks_and_non_directories() {
        let temporary = TempDir::new().expect("init directory");
        let root = temporary.path().join("coffee-club");
        fs::create_dir(&root).expect("existing directory");
        let link = temporary.path().join("linked-package");
        std::os::unix::fs::symlink(&root, &link).expect("symlink fixture");
        let file = temporary.path().join("regular-file");
        fs::write(&file, "existing user content").expect("file fixture");
        for path in [&link, &file, &temporary.path().join("missing")] {
            assert!(canonical_init_root(path).is_err(), "{}", path.display());
        }
    }

    #[test]
    fn default_new_and_init_create_runnable_contract_only_packages() {
        for command in ["new", "init"] {
            let temporary = TempDir::new().expect("scaffold directory");
            let root = temporary.path().join("coffee-club");
            if command == "init" {
                fs::create_dir(&root).expect("existing directory");
            }
            let result = invoke([
                OsString::from("musubi"),
                OsString::from(command),
                root.as_os_str().to_owned(),
                OsString::from("--namespace"),
                OsString::from("demo"),
            ])
            .output
            .render(OutputFormat::Human)
            .expect("creation output");
            assert_eq!(result.exit_code(), 0, "{}", result.stderr());
            assert!(
                result
                    .stdout()
                    .contains("musubi check\n  musubi test\n  musubi build")
            );
            assert!(!result.stdout().contains("--frozen"));
            assert!(!root.join("src").exists());
            let manifest_path = root.join(MANIFEST_FILE_NAME);
            let manifest = parse_manifest(&fs::read_to_string(&manifest_path).expect("manifest"))
                .expect("valid manifest");
            assert!(manifest.library.is_none());
            assert_eq!(manifest.contracts.len(), 1);
            assert_eq!(manifest.tests.len(), 1);
            for (operation, mode) in [
                ("check", "--offline"),
                ("test", "--frozen"),
                ("build", "--frozen"),
            ] {
                let result = invoke([
                    OsString::from("musubi"),
                    OsString::from("--manifest-path"),
                    manifest_path.as_os_str().to_owned(),
                    OsString::from(operation),
                    OsString::from(mode),
                ])
                .output
                .render(OutputFormat::Human)
                .expect("workflow output");
                assert_eq!(result.exit_code(), 0, "{operation}: {}", result.stderr());
                if operation == "test" {
                    assert!(result.stdout().contains("4 passed"));
                }
            }
            assert!(
                root.join("target/kotodama/demo/coffee-club/production/coffee-club.to")
                    .is_file()
            );
            let contract_path = root.join("contracts/coffee-club.ko");
            fs::write(
                &contract_path,
                CONTRACT_SOURCE.replace(
                    "return points(coffees: coffees);",
                    "return points(coffees: coffees) + 1;",
                ),
            )
            .expect("mutate public boundary only");
            let result = invoke([
                OsString::from("musubi"),
                OsString::from("--manifest-path"),
                manifest_path.as_os_str().to_owned(),
                OsString::from("test"),
                OsString::from("--frozen"),
            ]);
            assert_ne!(
                result.output.exit_code(),
                0,
                "tests must exercise quote, not only the unchanged helper"
            );
        }
    }

    #[test]
    fn contract_template_rejects_library_options_before_creating_files() {
        for arguments in [["--export", "quote"], ["--source-dir", "library"]] {
            let temporary = TempDir::new().expect("scaffold directory");
            let root = temporary.path().join("app");
            let result = invoke([
                OsString::from("musubi"),
                OsString::from("new"),
                root.as_os_str().to_owned(),
                OsString::from("--namespace"),
                OsString::from("demo"),
                OsString::from(arguments[0]),
                OsString::from(arguments[1]),
            ]);
            assert_eq!(result.output.exit_code(), ErrorCode::Usage.exit_code());
            assert!(!root.exists());
        }
    }

    #[test]
    fn example_sources_match_the_canonical_contract_template() {
        assert_eq!(
            CONTRACT_SOURCE,
            include_str!("../../../examples/coffee-club/contracts/coffee-club.ko")
        );
        assert_eq!(
            CONTRACT_TESTS,
            include_str!("../../../examples/coffee-club/tests/coffee-club.test.ko")
        );
    }
}
