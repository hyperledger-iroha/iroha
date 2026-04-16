//! Command implementation for the `musubi` Kotodama package manager.

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};
use eyre::{Result, WrapErr, bail, eyre};
use iroha_data_model::{
    musubi::{
        MusubiArchiveRef, MusubiDappLink, MusubiNamespace, MusubiPackageId, MusubiPackageName,
        MusubiPackageRef, MusubiVersion,
    },
    name::Name,
    smart_contract::ContractAlias,
    sorafs::pin_registry::ManifestDigest,
};
use ivm::{KotodamaCompiler, kotodama::compiler::CompilerOptions};

const DEFAULT_MANIFEST: &str = "Musubi.toml";
const DEFAULT_LOCKFILE: &str = "Musubi.lock";
const LOCKFILE_VERSION: i64 = 1;
const ARCHIVE_DOMAIN_SEPARATOR: &[u8] = b"musubi-source-archive-v1";

/// Run the Musubi command-line interface.
pub(crate) fn run() -> Result<()> {
    Args::parse().run()
}

#[derive(Parser, Debug)]
#[command(
    name = "musubi",
    version = env!("CARGO_PKG_VERSION"),
    author,
    about = "Kotodama source package manager"
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

impl Args {
    fn run(self) -> Result<()> {
        match self.command {
            Command::Init(args) => args.run(),
            Command::Add(args) => args.run(),
            Command::Install(args) => args.run(),
            Command::Build(args) => args.run(),
            Command::Pack(args) => args.run(),
            Command::Publish(args) => args.run(),
            Command::Yank(args) => args.run(),
            Command::Info(args) => args.run(),
        }
    }
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Create a Musubi.toml package manifest
    Init(InitArgs),
    /// Add an exact source-library dependency
    Add(AddArgs),
    /// Validate dependencies and refresh Musubi.lock
    Install(InstallArgs),
    /// Compile a local Kotodama source file
    Build(BuildArgs),
    /// Compute the deterministic source archive hash for this package
    Pack(PackArgs),
    /// Prepare or submit a package release
    Publish(PublishArgs),
    /// Prepare or submit a yank for an existing release
    Yank(YankArgs),
    /// Print package manifest information
    Info(ManifestPathArgs),
}

#[derive(clap::Args, Debug)]
struct ManifestPathArgs {
    /// Path to the Musubi package manifest
    #[arg(long, default_value = DEFAULT_MANIFEST)]
    manifest: PathBuf,
}

#[derive(clap::Args, Debug)]
struct InitArgs {
    /// Package namespace, matching the contract alias suffix shape
    #[arg(long)]
    namespace: MusubiNamespace,
    /// Package name inside the namespace
    #[arg(long)]
    name: MusubiPackageName,
    /// Initial package version
    #[arg(long, default_value = "0.1.0")]
    version: MusubiVersion,
    /// Manifest path to create
    #[arg(long, default_value = DEFAULT_MANIFEST)]
    manifest: PathBuf,
    /// Overwrite an existing manifest
    #[arg(long)]
    force: bool,
    /// Add a dapp link using the package namespace
    #[arg(long)]
    dapp: bool,
    /// Add a dapp link using an explicit namespace
    #[arg(long)]
    dapp_namespace: Option<MusubiNamespace>,
}

impl InitArgs {
    fn run(self) -> Result<()> {
        if self.manifest.exists() && !self.force {
            bail!(
                "`{}` already exists; pass --force to overwrite it",
                self.manifest.display()
            );
        }
        let dapp_namespace = self
            .dapp_namespace
            .or_else(|| self.dapp.then(|| self.namespace.clone()));
        let manifest = MusubiManifest {
            package: ManifestPackage {
                namespace: self.namespace,
                name: self.name,
                version: self.version,
            },
            dependencies: Vec::new(),
            exports: Vec::new(),
            dapp: dapp_namespace.map(|namespace| ManifestDapp {
                namespace,
                contracts: Vec::new(),
            }),
        };
        write_manifest(&self.manifest, &manifest)?;
        println!("created {}", self.manifest.display());
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
struct AddArgs {
    /// Exact package release reference, e.g. dex.universal/swap-core@1.2.3
    package: MusubiPackageRef,
    /// Import alias used by Kotodama source
    #[arg(long)]
    alias: Option<Name>,
    /// Path to the Musubi package manifest
    #[arg(long, default_value = DEFAULT_MANIFEST)]
    manifest: PathBuf,
    /// Replace an existing dependency with the same alias
    #[arg(long)]
    replace: bool,
}

impl AddArgs {
    fn run(self) -> Result<()> {
        let mut manifest = read_manifest(&self.manifest)?;
        let alias = self.alias.unwrap_or_else(|| {
            self.package
                .package
                .name
                .as_str()
                .parse()
                .expect("Musubi package names are valid Iroha names")
        });
        if manifest
            .dependencies
            .iter()
            .any(|dependency| dependency.alias == alias)
            && !self.replace
        {
            bail!("dependency alias `{alias}` already exists; pass --replace to update it");
        }
        manifest
            .dependencies
            .retain(|dependency| dependency.alias != alias);
        manifest.dependencies.push(ManifestDependency {
            alias: alias.clone(),
            package: self.package,
        });
        manifest.normalize();
        write_manifest(&self.manifest, &manifest)?;
        println!("added dependency `{alias}` to {}", self.manifest.display());
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
struct InstallArgs {
    /// Path to the Musubi package manifest
    #[arg(long, default_value = DEFAULT_MANIFEST)]
    manifest: PathBuf,
    /// Lockfile path to refresh
    #[arg(long, default_value = DEFAULT_LOCKFILE)]
    lockfile: PathBuf,
}

impl InstallArgs {
    fn run(self) -> Result<()> {
        let manifest = read_manifest(&self.manifest)?;
        validate_dependency_aliases(&manifest)?;

        // TODO: Replace this exact-version lock seed with on-chain registry
        // resolution once PublishMusubiRelease and release queries are wired.
        let lockfile = MusubiLockfile::from_manifest(&manifest);
        write_lockfile(&self.lockfile, &lockfile)?;
        println!(
            "validated {} dependencies and wrote {}",
            manifest.dependencies.len(),
            self.lockfile.display()
        );
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
struct BuildArgs {
    /// Kotodama source file to compile
    source: PathBuf,
    /// Output .to bytecode path
    #[arg(long)]
    out: Option<PathBuf>,
    /// Optional contract manifest JSON output path
    #[arg(long)]
    manifest_out: Option<PathBuf>,
    /// ABI version. First release supports only ABI 1.
    #[arg(long, default_value_t = 1)]
    abi: u8,
}

impl BuildArgs {
    fn run(self) -> Result<()> {
        if self.abi != 1 {
            bail!("Musubi supports only Kotodama ABI 1 in the first release");
        }
        let source = fs::read_to_string(&self.source)
            .wrap_err_with(|| format!("failed to read `{}`", self.source.display()))?;
        let mut opts = CompilerOptions::default();
        opts.abi_version = self.abi;
        opts.debug_source_name = Some(self.source.display().to_string());
        let compiler = KotodamaCompiler::new_with_options(opts);
        let (bytecode, contract_manifest) = compiler
            .compile_source_with_manifest(&source)
            .map_err(|err| eyre!("Kotodama compile error: {err}"))?;

        let output = self.out.unwrap_or_else(|| {
            let mut output = self.source.clone();
            output.set_extension("to");
            output
        });
        if let Some(parent) = output
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)
                .wrap_err_with(|| format!("failed to create `{}`", parent.display()))?;
        }
        fs::write(&output, &bytecode)
            .wrap_err_with(|| format!("failed to write `{}`", output.display()))?;
        if let Some(manifest_out) = self.manifest_out {
            let rendered = norito::json::to_json_pretty(&contract_manifest)
                .map_err(|err| eyre!("failed to render contract manifest JSON: {err}"))?;
            fs::write(&manifest_out, rendered)
                .wrap_err_with(|| format!("failed to write `{}`", manifest_out.display()))?;
            println!("wrote contract manifest {}", manifest_out.display());
        }
        println!("wrote bytecode {}", output.display());
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
struct PackArgs {
    /// Directory to hash as the package source archive
    #[arg(long, default_value = ".")]
    source_root: PathBuf,
    /// Path to the Musubi package manifest
    #[arg(long, default_value = DEFAULT_MANIFEST)]
    manifest: PathBuf,
}

impl PackArgs {
    fn run(self) -> Result<()> {
        let manifest = read_manifest(&self.manifest)?;
        let archive_hash = hash_source_tree(&self.source_root)?;
        println!("package = {}", manifest.package.package_ref());
        println!("source_root = {}", self.source_root.display());
        println!("archive_hash_blake3_256 = {}", hex::encode(archive_hash));
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
struct PublishArgs {
    /// Path to the Musubi package manifest
    #[arg(long, default_value = DEFAULT_MANIFEST)]
    manifest: PathBuf,
    /// SoraFS manifest digest for the uploaded source archive
    #[arg(long, value_name = "HEX")]
    sorafs_manifest_digest: String,
    /// Precomputed source archive BLAKE3-256 hash; defaults to hashing the manifest directory
    #[arg(long, value_name = "HEX")]
    archive_hash: Option<String>,
    /// Print the release payload without submitting it
    #[arg(long)]
    dry_run: bool,
}

impl PublishArgs {
    fn run(self) -> Result<()> {
        if !self.dry_run {
            // TODO: Submit PublishMusubiRelease after the chain-side registry ISI
            // and namespace authority checks are added.
            bail!(
                "on-chain Musubi publish is not wired yet; use --dry-run for the release payload"
            );
        }
        let manifest = read_manifest(&self.manifest)?;
        let manifest_dir = self.manifest.parent().unwrap_or_else(|| Path::new("."));
        let archive_hash = match self.archive_hash {
            Some(value) => parse_hex_32(&value)?,
            None => hash_source_tree(manifest_dir)?,
        };
        let archive = MusubiArchiveRef::new(
            ManifestDigest::new(parse_hex_32(&self.sorafs_manifest_digest)?),
            archive_hash,
        );
        validate_dapp_link(&manifest)?;

        println!("package = {}", manifest.package.package_ref());
        println!(
            "sorafs_manifest_digest = {}",
            hex::encode(archive.sorafs_manifest.as_bytes())
        );
        println!(
            "archive_hash_blake3_256 = {}",
            hex::encode(archive.archive_hash_blake3_256)
        );
        println!("dependencies = {}", manifest.dependencies.len());
        println!("exports = {}", manifest.exports.len());
        println!("dry_run = true");
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
struct YankArgs {
    /// Exact package release reference to yank
    package: MusubiPackageRef,
    /// Human-readable yank reason
    #[arg(long)]
    reason: String,
    /// Print the yank request without submitting it
    #[arg(long)]
    dry_run: bool,
}

impl YankArgs {
    fn run(self) -> Result<()> {
        if !self.dry_run {
            // TODO: Submit YankMusubiRelease after the chain-side registry ISI
            // and namespace authority checks are added.
            bail!("on-chain Musubi yank is not wired yet; use --dry-run for the yank payload");
        }
        println!("package = {}", self.package);
        println!("reason = {}", self.reason);
        println!("dry_run = true");
        Ok(())
    }
}

impl ManifestPathArgs {
    fn run(self) -> Result<()> {
        let manifest = read_manifest(&self.manifest)?;
        println!("package = {}", manifest.package.package_ref());
        if manifest.dependencies.is_empty() {
            println!("dependencies = 0");
        } else {
            println!("dependencies = {}", manifest.dependencies.len());
            for dependency in &manifest.dependencies {
                println!("  {} = {}", dependency.alias, dependency.package);
            }
        }
        if !manifest.exports.is_empty() {
            println!("exports = {}", join_names(&manifest.exports));
        }
        if let Some(dapp) = &manifest.dapp {
            println!("dapp_namespace = {}", dapp.namespace);
            if !dapp.contracts.is_empty() {
                println!(
                    "dapp_contracts = {}",
                    join_contract_aliases(&dapp.contracts)
                );
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MusubiManifest {
    package: ManifestPackage,
    dependencies: Vec<ManifestDependency>,
    exports: Vec<Name>,
    dapp: Option<ManifestDapp>,
}

impl MusubiManifest {
    fn normalize(&mut self) {
        self.dependencies
            .sort_by(|left, right| left.alias.cmp(&right.alias));
        self.exports.sort();
        if let Some(dapp) = &mut self.dapp {
            dapp.contracts.sort();
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ManifestPackage {
    namespace: MusubiNamespace,
    name: MusubiPackageName,
    version: MusubiVersion,
}

impl ManifestPackage {
    fn package_ref(&self) -> MusubiPackageRef {
        MusubiPackageRef::new(
            MusubiPackageId::new(self.namespace.clone(), self.name.clone()),
            self.version.clone(),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ManifestDependency {
    alias: Name,
    package: MusubiPackageRef,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ManifestDapp {
    namespace: MusubiNamespace,
    contracts: Vec<ContractAlias>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MusubiLockfile {
    packages: Vec<LockedPackage>,
}

impl MusubiLockfile {
    fn from_manifest(manifest: &MusubiManifest) -> Self {
        let mut packages = manifest
            .dependencies
            .iter()
            .map(|dependency| LockedPackage {
                alias: dependency.alias.clone(),
                package: dependency.package.clone(),
            })
            .collect::<Vec<_>>();
        packages.sort_by(|left, right| left.alias.cmp(&right.alias));
        Self { packages }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LockedPackage {
    alias: Name,
    package: MusubiPackageRef,
}

fn read_manifest(path: &Path) -> Result<MusubiManifest> {
    let body = fs::read_to_string(path)
        .wrap_err_with(|| format!("failed to read `{}`", path.display()))?;
    parse_manifest(&body).wrap_err_with(|| format!("failed to parse `{}`", path.display()))
}

fn write_manifest(path: &Path, manifest: &MusubiManifest) -> Result<()> {
    let rendered = render_manifest(manifest)?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .wrap_err_with(|| format!("failed to create `{}`", parent.display()))?;
    }
    fs::write(path, rendered).wrap_err_with(|| format!("failed to write `{}`", path.display()))
}

fn write_lockfile(path: &Path, lockfile: &MusubiLockfile) -> Result<()> {
    let rendered = render_lockfile(lockfile)?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .wrap_err_with(|| format!("failed to create `{}`", parent.display()))?;
    }
    fs::write(path, rendered).wrap_err_with(|| format!("failed to write `{}`", path.display()))
}

fn parse_manifest(body: &str) -> Result<MusubiManifest> {
    let value: toml::Value = toml::from_str(body)?;
    let root = value
        .as_table()
        .ok_or_else(|| eyre!("Musubi.toml must be a TOML table"))?;
    let package_table = required_table(root, "package")?;
    let namespace = required_string(package_table, "namespace")?.parse()?;
    let name = required_string(package_table, "name")?.parse()?;
    let version = required_string(package_table, "version")?.parse()?;

    let mut manifest = MusubiManifest {
        package: ManifestPackage {
            namespace,
            name,
            version,
        },
        dependencies: parse_dependencies(root)?,
        exports: parse_exports(root)?,
        dapp: parse_dapp(root)?,
    };
    validate_dapp_link(&manifest)?;
    manifest.normalize();
    Ok(manifest)
}

fn parse_dependencies(root: &toml::Table) -> Result<Vec<ManifestDependency>> {
    let Some(value) = root.get("dependencies") else {
        return Ok(Vec::new());
    };
    let table = value
        .as_table()
        .ok_or_else(|| eyre!("`dependencies` must be a TOML table"))?;
    let mut dependencies = Vec::with_capacity(table.len());
    for (alias, value) in table {
        let alias = alias.parse::<Name>()?;
        let package = match value {
            toml::Value::String(value) => value.parse()?,
            toml::Value::Table(table) => {
                let package = required_string(table, "package")?;
                let version = required_string(table, "version")?;
                format!("{package}@{version}").parse()?
            }
            _ => {
                bail!(
                    "dependency `{alias}` must be a string reference or a table with package/version"
                );
            }
        };
        dependencies.push(ManifestDependency { alias, package });
    }
    Ok(dependencies)
}

fn parse_exports(root: &toml::Table) -> Result<Vec<Name>> {
    let Some(value) = root.get("exports") else {
        return Ok(Vec::new());
    };
    let table = value
        .as_table()
        .ok_or_else(|| eyre!("`exports` must be a TOML table"))?;
    parse_name_array(table, "functions")
}

fn parse_dapp(root: &toml::Table) -> Result<Option<ManifestDapp>> {
    let Some(value) = root.get("dapp") else {
        return Ok(None);
    };
    let table = value
        .as_table()
        .ok_or_else(|| eyre!("`dapp` must be a TOML table"))?;
    let namespace = required_string(table, "namespace")?.parse()?;
    let contracts = parse_contract_alias_array(table, "contracts")?;
    Ok(Some(ManifestDapp {
        namespace,
        contracts,
    }))
}

fn render_manifest(manifest: &MusubiManifest) -> Result<String> {
    let mut root = toml::Table::new();
    let mut package = toml::Table::new();
    package.insert(
        "namespace".to_owned(),
        toml::Value::String(manifest.package.namespace.to_string()),
    );
    package.insert(
        "name".to_owned(),
        toml::Value::String(manifest.package.name.to_string()),
    );
    package.insert(
        "version".to_owned(),
        toml::Value::String(manifest.package.version.to_string()),
    );
    root.insert("package".to_owned(), toml::Value::Table(package));

    if !manifest.dependencies.is_empty() {
        let mut dependencies = toml::Table::new();
        for dependency in &manifest.dependencies {
            let mut entry = toml::Table::new();
            entry.insert(
                "package".to_owned(),
                toml::Value::String(dependency.package.package.to_string()),
            );
            entry.insert(
                "version".to_owned(),
                toml::Value::String(dependency.package.version.to_string()),
            );
            dependencies.insert(dependency.alias.to_string(), toml::Value::Table(entry));
        }
        root.insert("dependencies".to_owned(), toml::Value::Table(dependencies));
    }

    if !manifest.exports.is_empty() {
        let mut exports = toml::Table::new();
        exports.insert(
            "functions".to_owned(),
            toml::Value::Array(
                manifest
                    .exports
                    .iter()
                    .map(|name| toml::Value::String(name.to_string()))
                    .collect(),
            ),
        );
        root.insert("exports".to_owned(), toml::Value::Table(exports));
    }

    if let Some(dapp) = &manifest.dapp {
        let mut table = toml::Table::new();
        table.insert(
            "namespace".to_owned(),
            toml::Value::String(dapp.namespace.to_string()),
        );
        table.insert(
            "contracts".to_owned(),
            toml::Value::Array(
                dapp.contracts
                    .iter()
                    .map(|alias| toml::Value::String(alias.to_string()))
                    .collect(),
            ),
        );
        root.insert("dapp".to_owned(), toml::Value::Table(table));
    }

    toml::to_string_pretty(&toml::Value::Table(root)).map_err(Into::into)
}

fn render_lockfile(lockfile: &MusubiLockfile) -> Result<String> {
    let mut root = toml::Table::new();
    root.insert("version".to_owned(), toml::Value::Integer(LOCKFILE_VERSION));
    let packages = lockfile
        .packages
        .iter()
        .map(|package| {
            let mut table = toml::Table::new();
            table.insert(
                "alias".to_owned(),
                toml::Value::String(package.alias.to_string()),
            );
            table.insert(
                "name".to_owned(),
                toml::Value::String(package.package.package.to_string()),
            );
            table.insert(
                "version".to_owned(),
                toml::Value::String(package.package.version.to_string()),
            );
            table.insert(
                "source".to_owned(),
                toml::Value::String("registry".to_owned()),
            );
            table.insert("resolved".to_owned(), toml::Value::Boolean(false));
            toml::Value::Table(table)
        })
        .collect::<Vec<_>>();
    root.insert("package".to_owned(), toml::Value::Array(packages));
    toml::to_string_pretty(&toml::Value::Table(root)).map_err(Into::into)
}

fn required_table<'a>(root: &'a toml::Table, key: &str) -> Result<&'a toml::Table> {
    root.get(key)
        .and_then(toml::Value::as_table)
        .ok_or_else(|| eyre!("`{key}` table is required"))
}

fn required_string<'a>(table: &'a toml::Table, key: &str) -> Result<&'a str> {
    table
        .get(key)
        .and_then(toml::Value::as_str)
        .ok_or_else(|| eyre!("`{key}` string is required"))
}

fn parse_name_array(table: &toml::Table, key: &str) -> Result<Vec<Name>> {
    let Some(value) = table.get(key) else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .ok_or_else(|| eyre!("`{key}` must be an array of strings"))?;
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| eyre!("`{key}` entries must be strings"))?
                .parse::<Name>()
                .map_err(Into::into)
        })
        .collect()
}

fn parse_contract_alias_array(table: &toml::Table, key: &str) -> Result<Vec<ContractAlias>> {
    let Some(value) = table.get(key) else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .ok_or_else(|| eyre!("`{key}` must be an array of strings"))?;
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| eyre!("`{key}` entries must be strings"))?
                .parse::<ContractAlias>()
                .map_err(Into::into)
        })
        .collect()
}

fn validate_dependency_aliases(manifest: &MusubiManifest) -> Result<()> {
    let mut seen = BTreeSet::new();
    for dependency in &manifest.dependencies {
        if !seen.insert(dependency.alias.clone()) {
            bail!("duplicate dependency alias `{}`", dependency.alias);
        }
    }
    Ok(())
}

fn validate_dapp_link(manifest: &MusubiManifest) -> Result<()> {
    if let Some(dapp) = &manifest.dapp {
        MusubiDappLink::new(dapp.namespace.clone(), dapp.contracts.clone())?;
    }
    Ok(())
}

fn hash_source_tree(root: &Path) -> Result<[u8; 32]> {
    let root = root
        .canonicalize()
        .wrap_err_with(|| format!("failed to canonicalize `{}`", root.display()))?;
    let mut files = Vec::new();
    collect_source_files(&root, &root, &mut files)?;
    files.sort();

    let mut hasher = blake3::Hasher::new();
    hasher.update(ARCHIVE_DOMAIN_SEPARATOR);
    for relative in files {
        let path = root.join(&relative);
        let bytes =
            fs::read(&path).wrap_err_with(|| format!("failed to read `{}`", path.display()))?;
        let relative = relative.to_string_lossy().replace('\\', "/");
        hasher.update(relative.as_bytes());
        hasher.update(b"\0");
        hasher.update(&(bytes.len() as u64).to_be_bytes());
        hasher.update(b"\0");
        hasher.update(&bytes);
        hasher.update(b"\0");
    }
    Ok(*hasher.finalize().as_bytes())
}

fn collect_source_files(root: &Path, current: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(current)
        .wrap_err_with(|| format!("failed to read directory `{}`", current.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        if should_skip_path(file_name.to_string_lossy().as_ref()) {
            continue;
        }
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_source_files(root, &path, files)?;
        } else if file_type.is_file() {
            files.push(
                path.strip_prefix(root)
                    .expect("walked path under root")
                    .to_path_buf(),
            );
        }
    }
    Ok(())
}

fn should_skip_path(file_name: &str) -> bool {
    matches!(file_name, ".git" | "target" | DEFAULT_LOCKFILE)
}

fn parse_hex_32(raw: &str) -> Result<[u8; 32]> {
    let raw = raw.strip_prefix("0x").unwrap_or(raw);
    let bytes = hex::decode(raw).wrap_err("hex string is invalid")?;
    let array: [u8; 32] = bytes
        .try_into()
        .map_err(|bytes: Vec<u8>| eyre!("expected 32 bytes, got {}", bytes.len()))?;
    Ok(array)
}

fn join_names(names: &[Name]) -> String {
    names
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn join_contract_aliases(aliases: &[ContractAlias]) -> String {
    aliases
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::{Decode, Encode};

    #[test]
    fn manifest_parses_namespace_dependencies_exports_and_dapp_link() {
        let manifest = parse_manifest(
            r#"
            [package]
            namespace = "dex.universal"
            name = "swap-core"
            version = "1.2.3"

            [dependencies]
            math = { package = "std.universal/math", version = "1.0.0" }

            [exports]
            functions = ["quote"]

            [dapp]
            namespace = "dex.universal"
            contracts = ["router::dex.universal"]
            "#,
        )
        .expect("parse manifest");

        assert_eq!(
            manifest.package.package_ref().to_string(),
            "dex.universal/swap-core@1.2.3"
        );
        assert_eq!(manifest.dependencies[0].alias.to_string(), "math");
        assert_eq!(
            manifest.dependencies[0].package.to_string(),
            "std.universal/math@1.0.0"
        );
        assert_eq!(manifest.exports[0].to_string(), "quote");
        assert_eq!(
            manifest.dapp.expect("dapp").contracts[0].as_ref(),
            "router::dex.universal"
        );
    }

    #[test]
    fn manifest_render_roundtrips_added_dependency() {
        let mut manifest = parse_manifest(
            r#"
            [package]
            namespace = "dex.universal"
            name = "swap-core"
            version = "1.2.3"
            "#,
        )
        .expect("parse manifest");
        manifest.dependencies.push(ManifestDependency {
            alias: "math".parse().expect("alias"),
            package: "std.universal/math@1.0.0".parse().expect("package"),
        });
        manifest.normalize();

        let rendered = render_manifest(&manifest).expect("render");
        let reparsed = parse_manifest(&rendered).expect("reparse");

        assert_eq!(reparsed, manifest);
        assert!(rendered.contains("[dependencies.math]"));
    }

    #[test]
    fn lockfile_marks_registry_resolution_as_pending() {
        let manifest = parse_manifest(
            r#"
            [package]
            namespace = "dex.universal"
            name = "swap-core"
            version = "1.2.3"

            [dependencies]
            math = "std.universal/math@1.0.0"
            "#,
        )
        .expect("parse manifest");
        let rendered = render_lockfile(&MusubiLockfile::from_manifest(&manifest)).expect("render");

        assert!(rendered.contains("version = 1"));
        assert!(rendered.contains("name = \"std.universal/math\""));
        assert!(rendered.contains("resolved = false"));
    }

    #[test]
    fn source_tree_hash_is_deterministic_and_ignores_lockfile() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(temp.path().join("Musubi.toml"), "[package]\n").expect("manifest");
        fs::create_dir(temp.path().join("src")).expect("src dir");
        fs::write(temp.path().join("src/lib.ko"), "fn main() {}\n").expect("source");
        fs::write(temp.path().join(DEFAULT_LOCKFILE), "ignored").expect("lockfile");

        let first = hash_source_tree(temp.path()).expect("first hash");
        fs::write(temp.path().join(DEFAULT_LOCKFILE), "changed").expect("lockfile");
        let second = hash_source_tree(temp.path()).expect("second hash");

        assert_eq!(first, second);
    }

    #[test]
    fn hex_parser_requires_32_bytes() {
        let err = parse_hex_32("abcd").expect_err("too short");

        assert!(err.to_string().contains("expected 32 bytes"));
    }

    #[test]
    fn musubi_release_types_are_norito_roundtrip_ready() {
        let archive = MusubiArchiveRef::new(ManifestDigest::new([1; 32]), [2; 32]);
        let bytes = archive.encode();
        let mut cursor = bytes.as_slice();
        let decoded = MusubiArchiveRef::decode(&mut cursor).expect("decode archive");

        assert!(cursor.is_empty());
        assert_eq!(decoded, archive);
    }
}
