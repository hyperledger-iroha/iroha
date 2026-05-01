//! Command implementation for the `musubi` Kotodama package manager.
#![allow(
    clippy::match_same_arms,
    clippy::needless_pass_by_value,
    clippy::option_if_let_else,
    clippy::struct_field_names,
    clippy::too_many_lines
)]

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};
use eyre::{Result, WrapErr, bail, eyre};
use iroha::{
    client::{
        Client, SorafsGatewayFetchOptions, SorafsGatewayScoreboardOptions, SorafsStorageFileEntry,
    },
    config::{Config, LoadPath},
};
use iroha_data_model::{
    Decode, Encode,
    isi::{
        musubi::{PublishMusubiRelease, SetMusubiShortAlias, YankMusubiRelease},
        sorafs::RegisterPinManifest,
    },
    musubi::{
        MusubiArchiveRef, MusubiDappLink, MusubiDependency, MusubiNamespace, MusubiPackageId,
        MusubiPackageName, MusubiPackageRef, MusubiRelease, MusubiReleaseStatus, MusubiShortAlias,
        MusubiSourceArchivePlan, MusubiSourceChunkPlan, MusubiSourceFilePlan, MusubiVersion,
        MusubiVersionReq,
    },
    name::Name,
    query::musubi::prelude::{
        FindMusubiPackageReleases, FindMusubiReleaseByRef, FindMusubiShortAliasByName,
        SearchMusubiPackages,
    },
    smart_contract::ContractAlias,
    sorafs::pin_registry::{
        ChunkerProfileHandle, ManifestDigest, PinPolicy as DataModelPinPolicy, StorageClass,
    },
};
use ivm::{
    KotodamaCompiler,
    kotodama::{
        ast::{Block, Expr, Function, Item, Program, Statement},
        compiler::CompilerOptions,
        parser::parse as parse_kotodama,
    },
};
use sorafs_car::gateway::{
    GatewayFetchConfig as SorafsGatewayFetchConfig,
    GatewayProviderInput as SorafsGatewayProviderInput,
};
use sorafs_car::{CarBuildPlan, CarWriter, FileEntry, compute_chunk_plan_digest_sha3};
use sorafs_car::{CarChunk, FilePlan};
use sorafs_manifest::{
    BLAKE3_256_MULTIHASH_CODE, DagCodecId, MANIFEST_DAG_CODEC, ManifestBuilder, chunker_registry,
};

const DEFAULT_MANIFEST: &str = "Musubi.toml";
const DEFAULT_LOCKFILE: &str = "Musubi.lock";
const LOCKFILE_VERSION: i64 = 3;
const DEFAULT_CACHE_DIR: &str = ".musubi/cache";
const DEFAULT_DIST_DIR: &str = ".musubi/dist";
const ARCHIVE_DOMAIN_SEPARATOR: &[u8] = b"musubi-source-archive-v1";

/// Run the Musubi command-line interface.
pub fn run() -> Result<()> {
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
            Command::Update(args) => args.run(),
            Command::Build(args) => args.run(),
            Command::Pack(args) => args.run(),
            Command::Publish(args) => args.run(),
            Command::Yank(args) => args.run(),
            Command::Alias(args) => args.run(),
            Command::Versions(args) => args.run(),
            Command::Search(args) => args.run(),
            Command::Cache(args) => args.run(),
            Command::Info(args) => args.run(),
        }
    }
}

#[derive(clap::Args, Debug)]
struct ClientArgs {
    /// Path to the Iroha client configuration file
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,
    /// Wait for transaction commit or rejection
    #[arg(long)]
    wait: bool,
}

impl ClientArgs {
    fn load(&self) -> Result<(Client, iroha_data_model::account::AccountId)> {
        let load_path = self.config.as_ref().map_or_else(
            || LoadPath::Default(PathBuf::from("client.toml")),
            |path| LoadPath::Explicit(path.clone()),
        );
        let config = Config::load(load_path)
            .map_err(|err| eyre!("failed to load Iroha client config: {err:?}"))?;
        let account = config.account.clone();
        Ok((Client::new(config), account))
    }

    fn submit<I>(&self, instruction: I) -> Result<()>
    where
        I: Into<iroha_data_model::isi::InstructionBox>,
    {
        let (client, _) = self.load()?;
        let hash = if self.wait {
            client.submit_blocking(instruction)?
        } else {
            client.submit(instruction)?
        };
        println!("transaction_hash = {hash}");
        Ok(())
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
    /// Refresh locked dependencies within manifest requirements
    Update(UpdateArgs),
    /// Compile a local Kotodama source file
    Build(BuildArgs),
    /// Compute the deterministic source archive hash for this package
    Pack(PackArgs),
    /// Prepare or submit a package release
    Publish(PublishArgs),
    /// Prepare or submit a yank for an existing release
    Yank(YankArgs),
    /// Resolve or curate a global short alias
    Alias(AliasArgs),
    /// List published versions for a package id
    Versions(VersionsArgs),
    /// Search published Musubi packages
    Search(SearchArgs),
    /// Inspect or prune the local source cache
    Cache(CacheArgs),
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
    #[command(flatten)]
    client: ClientArgs,
    /// Package id, short alias, or exact package ref
    package: String,
    /// Version requirement when PACKAGE has no @version suffix
    #[arg(long)]
    version: Option<MusubiVersionReq>,
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
        let package =
            parse_add_dependency_package(&self.package, self.version, Some(&self.client))?;
        let alias = self.alias.unwrap_or_else(|| {
            package
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
            package: package.package,
            version_req: package.version_req,
        });
        manifest.normalize();
        write_manifest(&self.manifest, &manifest)?;
        println!("added dependency `{alias}` to {}", self.manifest.display());
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
struct InstallArgs {
    #[command(flatten)]
    client: ClientArgs,
    /// Path to the Musubi package manifest
    #[arg(long, default_value = DEFAULT_MANIFEST)]
    manifest: PathBuf,
    /// Lockfile path to refresh
    #[arg(long, default_value = DEFAULT_LOCKFILE)]
    lockfile: PathBuf,
    /// Do not query the on-chain registry; write an unresolved exact-version lockfile
    #[arg(long)]
    offline: bool,
    /// Require the existing lockfile to satisfy the manifest without changes
    #[arg(long)]
    locked: bool,
    /// Local cache directory for verified source archives
    #[arg(long, default_value = DEFAULT_CACHE_DIR)]
    cache_dir: PathBuf,
    /// Fetch missing source archives into the local cache after resolving
    #[arg(long)]
    fetch: bool,
    /// Local provider payload used by --fetch while gateway providers are not configured
    #[arg(long, value_name = "PATH")]
    provider_payload: Vec<PathBuf>,
    #[command(flatten)]
    gateway: GatewayFetchArgs,
}

impl InstallArgs {
    fn run(self) -> Result<()> {
        if !self.fetch && self.gateway.has_any_args() {
            bail!("gateway fetch options require --fetch");
        }
        let manifest = read_manifest(&self.manifest)?;
        validate_dependency_aliases(&manifest)?;

        let lockfile = if self.offline || manifest.dependencies.is_empty() {
            let existing = read_lockfile_optional(&self.lockfile)?;
            if let Some(lockfile) = existing {
                validate_lockfile_satisfies_manifest(&lockfile, &manifest)?;
                lockfile
            } else if self.locked && !manifest.dependencies.is_empty() {
                bail!(
                    "{} is missing; run `musubi install`",
                    self.lockfile.display()
                );
            } else {
                MusubiLockfile::from_manifest(&manifest)
            }
        } else {
            let (client, _) = self.client.load()?;
            let existing = read_lockfile_optional(&self.lockfile)?;
            let lockfile = resolve_manifest_dependencies(
                &client,
                &manifest,
                existing.as_ref(),
                &self.cache_dir,
                ResolveMode::Install,
            )?;
            if self.locked
                && existing
                    .as_ref()
                    .is_some_and(|existing| existing != &lockfile)
            {
                bail!(
                    "{} is out of date; run `musubi install`",
                    self.lockfile.display()
                );
            }
            lockfile
        };
        if self.fetch {
            validate_source_fetch_inputs(&self.provider_payload, &self.gateway)?;
            if self.gateway.has_providers() {
                validate_gateway_scope_for_lockfile(&lockfile, &self.cache_dir, &self.gateway)?;
                let (client, _) = self.client.load()?;
                let runner = ClientGatewayFetchRunner { client: &client };
                fetch_missing_lockfile_sources(
                    &lockfile,
                    &self.cache_dir,
                    SourceFetchMode::Gateway {
                        runner: &runner,
                        args: &self.gateway,
                        allow_unscoped_providers: count_missing_lockfile_sources(
                            &lockfile,
                            &self.cache_dir,
                        ) == 1,
                    },
                )?;
            } else {
                fetch_missing_lockfile_sources(
                    &lockfile,
                    &self.cache_dir,
                    SourceFetchMode::ProviderPayloads(&self.provider_payload),
                )?;
            }
        }
        write_lockfile(&self.lockfile, &lockfile)?;
        println!(
            "validated {} dependencies and wrote {}",
            manifest.dependencies.len(),
            self.lockfile.display()
        );
        Ok(())
    }
}

#[derive(clap::Args, Debug, Default)]
struct GatewayFetchArgs {
    /// Gateway provider descriptor: name=<alias>,provider-id=<64-hex>,base-url=<url>,stream-token=<base64>[,privacy-url=<url>][,package=<alias-or-ref>][,manifest=<64-hex>]
    #[arg(long = "gateway-provider", value_name = "SPEC")]
    gateway_provider: Vec<GatewayProviderSpec>,
    /// Client label sent to `SoraFS` gateway providers for audit and rate limiting.
    #[arg(long)]
    gateway_client_id: Option<String>,
    /// Maximum retry attempts per chunk during gateway fetch
    #[arg(long, value_parser = parse_nonzero_usize)]
    gateway_retry_budget: Option<usize>,
    /// Hard cap on the number of gateway providers used for one fetch
    #[arg(long, value_parser = parse_nonzero_usize)]
    gateway_max_peers: Option<usize>,
    /// Telemetry region label attached to gateway fetch metrics
    #[arg(long)]
    gateway_telemetry_region: Option<String>,
    /// Persist the gateway fetch scoreboard JSON artifact
    #[arg(long, value_name = "PATH")]
    gateway_scoreboard_out: Option<PathBuf>,
    /// Permit `<http://localhost>`, `<http://127.0.0.1>`, or `<http://[::1]>` gateway URLs for local testing.
    #[arg(long = "gateway-allow-insecure-localhost")]
    gateway_allow_insecure_localhost: bool,
}

impl GatewayFetchArgs {
    fn has_providers(&self) -> bool {
        !self.gateway_provider.is_empty()
    }

    fn has_any_args(&self) -> bool {
        self.has_providers()
            || self.gateway_client_id.is_some()
            || self.gateway_retry_budget.is_some()
            || self.gateway_max_peers.is_some()
            || self.gateway_telemetry_region.is_some()
            || self.gateway_scoreboard_out.is_some()
            || self.gateway_allow_insecure_localhost
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct GatewayProviderSpec {
    name: String,
    provider_id_hex: String,
    base_url: String,
    stream_token_b64: String,
    privacy_events_url: Option<String>,
    package: Option<String>,
    manifest_id_hex: Option<String>,
}

impl std::str::FromStr for GatewayProviderSpec {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        parse_gateway_provider_spec(value)
    }
}

#[derive(clap::Args, Debug)]
struct UpdateArgs {
    #[command(flatten)]
    client: ClientArgs,
    /// Path to the Musubi package manifest
    #[arg(long, default_value = DEFAULT_MANIFEST)]
    manifest: PathBuf,
    /// Lockfile path to update
    #[arg(long, default_value = DEFAULT_LOCKFILE)]
    lockfile: PathBuf,
    /// Optional dependency alias or package id to refresh
    #[arg(short = 'p', long)]
    package: Option<String>,
    /// Local cache directory for verified source archives
    #[arg(long, default_value = DEFAULT_CACHE_DIR)]
    cache_dir: PathBuf,
}

impl UpdateArgs {
    fn run(self) -> Result<()> {
        let manifest = read_manifest(&self.manifest)?;
        validate_dependency_aliases(&manifest)?;
        let existing = read_lockfile_optional(&self.lockfile)?;
        let (client, _) = self.client.load()?;
        let lockfile = resolve_manifest_dependencies(
            &client,
            &manifest,
            existing.as_ref(),
            &self.cache_dir,
            ResolveMode::Update {
                package: self.package.as_deref(),
            },
        )?;
        write_lockfile(&self.lockfile, &lockfile)?;
        println!("updated {} dependencies", lockfile.packages.len());
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
    /// Lockfile providing resolved dependency aliases
    #[arg(long, default_value = DEFAULT_LOCKFILE)]
    lockfile: PathBuf,
    /// Local cache directory containing verified dependency sources
    #[arg(long, default_value = DEFAULT_CACHE_DIR)]
    cache_dir: PathBuf,
}

impl BuildArgs {
    fn run(self) -> Result<()> {
        if self.abi != 1 {
            bail!("Musubi supports only Kotodama ABI 1 in the first release");
        }
        let source = fs::read_to_string(&self.source)
            .wrap_err_with(|| format!("failed to read `{}`", self.source.display()))?;
        let lockfile = read_lockfile_optional(&self.lockfile)?;
        let program = if let Some(lockfile) = lockfile.as_ref() {
            link_program_with_lockfile(&source, lockfile, &self.cache_dir)?
        } else {
            parse_kotodama(&source).map_err(|err| eyre!("Kotodama parse error: {err}"))?
        };
        let opts = CompilerOptions {
            abi_version: self.abi,
            debug_source_name: Some(self.source.display().to_string()),
            ..Default::default()
        };
        let compiler = KotodamaCompiler::new_with_options(opts);
        let (bytecode, contract_manifest) = compiler
            .compile_program_with_manifest(&program)
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
    /// Write a deterministic `SoraFS` CAR payload to this path.
    #[arg(long)]
    car_out: Option<PathBuf>,
    /// Write a `SoraFS` manifest to this path.
    #[arg(long)]
    sorafs_manifest_out: Option<PathBuf>,
    /// Write the Musubi source archive plan as Norito bytes to this path
    #[arg(long)]
    source_plan_out: Option<PathBuf>,
}

impl PackArgs {
    fn run(self) -> Result<()> {
        let manifest = read_manifest(&self.manifest)?;
        let archive = hash_source_tree(&self.source_root)?;
        let sorafs = if self.car_out.is_some()
            || self.sorafs_manifest_out.is_some()
            || self.source_plan_out.is_some()
        {
            Some(build_sorafs_source_manifest(
                &manifest,
                &self.source_root,
                self.car_out.as_deref(),
                self.sorafs_manifest_out.as_deref(),
                self.source_plan_out.as_deref(),
                archive,
            )?)
        } else {
            None
        };
        println!("package = {}", manifest.package.package_ref());
        println!("source_root = {}", self.source_root.display());
        println!(
            "archive_hash_blake3_256 = {}",
            hex::encode(archive.archive_hash_blake3_256)
        );
        println!("source_bytes = {}", archive.source_bytes);
        println!("source_file_count = {}", archive.source_file_count);
        if let Some(sorafs) = sorafs {
            println!(
                "sorafs_manifest_digest = {}",
                hex::encode(sorafs.digest.as_bytes())
            );
            println!(
                "car_archive_hash_blake3_256 = {}",
                hex::encode(sorafs.car_hash)
            );
            if let Some(path) = self.car_out {
                println!("car_out = {}", path.display());
            }
            if let Some(path) = self.sorafs_manifest_out {
                println!("sorafs_manifest_out = {}", path.display());
            }
            if let Some(path) = self.source_plan_out {
                println!("source_plan_out = {}", path.display());
            }
        }
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
struct PublishArgs {
    #[command(flatten)]
    client: ClientArgs,
    /// Path to the Musubi package manifest
    #[arg(long, default_value = DEFAULT_MANIFEST)]
    manifest: PathBuf,
    /// `SoraFS` manifest digest for the uploaded source archive.
    #[arg(long, value_name = "HEX")]
    sorafs_manifest_digest: Option<String>,
    /// Precomputed source archive BLAKE3-256 hash; defaults to hashing the manifest directory
    #[arg(long, value_name = "HEX")]
    archive_hash: Option<String>,
    /// Optional CAR output path to prepare before publishing
    #[arg(long)]
    car_out: Option<PathBuf>,
    /// Optional `SoraFS` manifest output path to prepare before publishing.
    #[arg(long)]
    sorafs_manifest_out: Option<PathBuf>,
    /// Optional Musubi source archive plan output path
    #[arg(long)]
    source_plan_out: Option<PathBuf>,
    /// Upload the generated manifest and payload through Torii's `SoraFS` storage pin endpoint.
    #[arg(long)]
    upload: bool,
    /// Lockfile used to pin resolved dependency versions in the release record
    #[arg(long, default_value = DEFAULT_LOCKFILE)]
    lockfile: PathBuf,
    /// Print the release payload without submitting it
    #[arg(long)]
    dry_run: bool,
}

impl PublishArgs {
    fn run(self) -> Result<()> {
        let manifest = read_manifest(&self.manifest)?;
        let manifest_dir = self
            .manifest
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let archive_stats = hash_source_tree(manifest_dir)?;
        let archive_hash = match self.archive_hash {
            Some(value) => {
                let value = parse_hex_32(&value)?;
                if value != archive_stats.archive_hash_blake3_256 {
                    bail!("--archive-hash does not match the canonical source tree hash");
                }
                value
            }
            None => archive_stats.archive_hash_blake3_256,
        };
        if self.sorafs_manifest_digest.is_some() {
            bail!(
                "--sorafs-manifest-digest alone is no longer accepted; publish must build and record a deterministic source archive plan"
            );
        }
        let default_outputs = default_publish_artifact_paths(&manifest);
        let car_out = self
            .car_out
            .as_deref()
            .or(Some(default_outputs.car.as_path()));
        let sorafs_manifest_out = self
            .sorafs_manifest_out
            .as_deref()
            .or(Some(default_outputs.manifest.as_path()));
        let source_plan_out = self
            .source_plan_out
            .as_deref()
            .or(Some(default_outputs.source_plan.as_path()));
        let generated_sorafs = build_sorafs_source_manifest(
            &manifest,
            manifest_dir,
            car_out,
            sorafs_manifest_out,
            source_plan_out,
            archive_stats,
        )?;
        let sorafs_manifest_digest = generated_sorafs.digest;
        let archive = MusubiArchiveRef::new(
            sorafs_manifest_digest,
            archive_hash,
            archive_stats.source_bytes,
            archive_stats.source_file_count,
        );
        validate_dapp_link(&manifest)?;
        if !archive.is_non_empty() {
            bail!("Musubi releases must include a non-empty source archive");
        }
        if manifest.exports.is_empty() {
            bail!("Musubi releases must export at least one Kotodama function");
        }
        validate_exported_functions_exist(manifest_dir, &manifest.exports)?;

        println!("package = {}", manifest.package.package_ref());
        println!(
            "sorafs_manifest_digest = {}",
            hex::encode(archive.sorafs_manifest.as_bytes())
        );
        println!(
            "archive_hash_blake3_256 = {}",
            hex::encode(archive.archive_hash_blake3_256)
        );
        println!("source_bytes = {}", archive.source_bytes);
        println!("source_file_count = {}", archive.source_file_count);
        println!("dependencies = {}", manifest.dependencies.len());
        println!("exports = {}", manifest.exports.len());
        println!("dry_run = {}", self.dry_run);
        if !self.dry_run {
            let (client, account) = self.client.load()?;
            if self.upload {
                let files = generated_sorafs
                    .source_plan
                    .files
                    .iter()
                    .map(|file| SorafsStorageFileEntry {
                        path: file.path.as_slice(),
                        size: file.size,
                    })
                    .collect::<Vec<_>>();
                client
                    .post_sorafs_storage_pin(
                        &generated_sorafs.manifest_bytes,
                        &generated_sorafs.payload,
                        Some(&files),
                    )
                    .wrap_err("failed to upload Musubi source archive through SoraFS storage pin endpoint")?;
                println!("sorafs_storage_pin_uploaded = true");
            }
            let pin_hash = client.submit_blocking(RegisterPinManifest::new(
                generated_sorafs.digest,
                generated_sorafs.chunker.clone(),
                generated_sorafs.chunk_digest_sha3_256,
                generated_sorafs.pin_policy,
                0,
                None,
                None,
            ))?;
            println!("sorafs_pin_transaction_hash = {pin_hash}");
            let lockfile = read_lockfile_optional(&self.lockfile)?;
            let release = release_from_manifest(
                &manifest,
                lockfile.as_ref(),
                archive,
                generated_sorafs.source_plan.clone(),
                account,
                0,
            )?;
            release
                .validate_publishable()
                .map_err(|err| eyre!("{}", err.reason()))?;
            let hash = if self.client.wait {
                client.submit_blocking(PublishMusubiRelease::new(release))?
            } else {
                client.submit(PublishMusubiRelease::new(release))?
            };
            println!("transaction_hash = {hash}");
        }
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
struct YankArgs {
    #[command(flatten)]
    client: ClientArgs,
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
        println!("package = {}", self.package);
        println!("reason = {}", self.reason);
        println!("dry_run = {}", self.dry_run);
        if !self.dry_run {
            let instruction = YankMusubiRelease::new(self.package, self.reason);
            self.client.submit(instruction)?;
        }
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
struct AliasArgs {
    #[command(subcommand)]
    command: AliasCommand,
}

impl AliasArgs {
    fn run(self) -> Result<()> {
        match self.command {
            AliasCommand::Resolve(args) => args.run(),
            AliasCommand::Set(args) => args.run(),
        }
    }
}

#[derive(Subcommand, Debug)]
enum AliasCommand {
    /// Resolve a curated short alias to its canonical package id
    Resolve(AliasResolveArgs),
    /// Bind or update a curated short alias
    Set(AliasSetArgs),
}

#[derive(clap::Args, Debug)]
struct AliasResolveArgs {
    #[command(flatten)]
    client: ClientArgs,
    /// Curated short alias to resolve
    alias: Name,
}

impl AliasResolveArgs {
    fn run(self) -> Result<()> {
        let (client, _) = self.client.load()?;
        let target = client
            .query_single(FindMusubiShortAliasByName::new(self.alias.clone()))
            .wrap_err_with(|| format!("failed to resolve Musubi alias `{}`", self.alias))?;
        println!("alias = {}", self.alias);
        println!("target = {target}");
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
struct AliasSetArgs {
    #[command(flatten)]
    client: ClientArgs,
    /// Curated short alias with no namespace prefix
    alias: Name,
    /// Canonical package id selected by the alias
    target: MusubiPackageId,
    /// Print the alias binding without submitting it
    #[arg(long)]
    dry_run: bool,
}

impl AliasSetArgs {
    fn run(self) -> Result<()> {
        let binding = MusubiShortAlias::new(self.alias, self.target);
        println!("alias = {}", binding.alias);
        println!("target = {}", binding.target);
        println!("dry_run = {}", self.dry_run);
        if !self.dry_run {
            self.client.submit(SetMusubiShortAlias::new(binding))?;
        }
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
struct VersionsArgs {
    #[command(flatten)]
    client: ClientArgs,
    /// Canonical package id to inspect
    package: MusubiPackageId,
    /// Include yanked releases
    #[arg(long)]
    include_yanked: bool,
}

impl VersionsArgs {
    fn run(self) -> Result<()> {
        let (client, _) = self.client.load()?;
        let releases = client
            .query_single(FindMusubiPackageReleases {
                package: self.package.clone(),
                include_yanked: self.include_yanked,
            })
            .wrap_err_with(|| format!("failed to list Musubi releases for `{}`", self.package))?;
        println!("package = {}", self.package);
        println!("versions = {}", releases.len());
        for release in releases {
            println!(
                "  {} {}",
                release.package.version,
                release_status_label(&release.status)
            );
        }
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
struct SearchArgs {
    #[command(flatten)]
    client: ClientArgs,
    /// Text query matched against canonical package ids
    query: String,
    /// Optional namespace filter
    #[arg(long)]
    namespace: Option<MusubiNamespace>,
    /// Include packages that have only yanked releases
    #[arg(long)]
    include_yanked: bool,
    /// Result offset
    #[arg(long, default_value_t = 0)]
    offset: u32,
    /// Result limit
    #[arg(long, default_value_t = 20)]
    limit: u32,
}

impl SearchArgs {
    fn run(self) -> Result<()> {
        let (client, _) = self.client.load()?;
        let packages = client
            .query_single(SearchMusubiPackages {
                namespace: self.namespace,
                query: self.query,
                include_yanked: self.include_yanked,
                offset: self.offset,
                limit: self.limit,
            })
            .wrap_err("failed to search Musubi packages")?;
        println!("packages = {}", packages.len());
        for package in packages {
            let latest = package
                .latest_active
                .map_or_else(|| "none".to_owned(), |version| version.to_string());
            println!(
                "  {} latest={} releases={} yanked={}",
                package.package, latest, package.release_count, package.yanked_count
            );
        }
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
struct CacheArgs {
    #[command(subcommand)]
    command: CacheCommand,
}

impl CacheArgs {
    fn run(self) -> Result<()> {
        match self.command {
            CacheCommand::List(args) => args.run(),
            CacheCommand::Import(args) => args.run(),
            CacheCommand::Fetch(args) => args.run(),
            CacheCommand::Verify(args) => args.verify(),
            CacheCommand::Prune(args) => args.run(),
        }
    }
}

#[derive(Subcommand, Debug)]
enum CacheCommand {
    /// List cache entries referenced by a lockfile
    List(CachePathArgs),
    /// Import a local source tree for one locked dependency
    Import(CacheImportArgs),
    /// Fetch and reconstruct one locked dependency from verified provider payloads
    Fetch(CacheFetchArgs),
    /// Verify cached source hashes referenced by a lockfile
    Verify(CachePathArgs),
    /// Remove unreferenced cache entries
    Prune(CachePruneArgs),
}

#[derive(clap::Args, Debug)]
struct CachePathArgs {
    /// Lockfile path to inspect
    #[arg(long, default_value = DEFAULT_LOCKFILE)]
    lockfile: PathBuf,
    /// Local cache directory
    #[arg(long, default_value = DEFAULT_CACHE_DIR)]
    cache_dir: PathBuf,
}

#[derive(clap::Args, Debug)]
struct CacheImportArgs {
    /// Lockfile path to inspect
    #[arg(long, default_value = DEFAULT_LOCKFILE)]
    lockfile: PathBuf,
    /// Local cache directory
    #[arg(long, default_value = DEFAULT_CACHE_DIR)]
    cache_dir: PathBuf,
    /// Dependency alias or canonical package ref from the lockfile
    package: String,
    /// Source tree to copy into the cache
    #[arg(long)]
    source_root: PathBuf,
    /// Replace an existing cache entry
    #[arg(long)]
    replace: bool,
}

#[derive(clap::Args, Debug)]
struct CacheFetchArgs {
    #[command(flatten)]
    client: ClientArgs,
    /// Lockfile path to inspect
    #[arg(long, default_value = DEFAULT_LOCKFILE)]
    lockfile: PathBuf,
    /// Local cache directory
    #[arg(long, default_value = DEFAULT_CACHE_DIR)]
    cache_dir: PathBuf,
    /// Dependency alias or canonical package ref from the lockfile
    package: String,
    /// Local provider payload containing the canonical concatenated source payload
    #[arg(long = "provider-payload", value_name = "PATH")]
    provider_payload: Vec<PathBuf>,
    #[command(flatten)]
    gateway: GatewayFetchArgs,
    /// Replace an existing cache entry
    #[arg(long)]
    replace: bool,
}

impl CacheFetchArgs {
    fn run(self) -> Result<()> {
        let lockfile = read_lockfile(&self.lockfile)?;
        let package = lockfile
            .find_package(&self.package)
            .ok_or_else(|| eyre!("lockfile does not contain `{}`", self.package))?;
        validate_source_fetch_inputs(&self.provider_payload, &self.gateway)?;
        if self.gateway.has_providers() {
            let (client, _) = self.client.load()?;
            let runner = ClientGatewayFetchRunner { client: &client };
            fetch_locked_package_source_from(
                package,
                &self.cache_dir,
                SourceFetchMode::Gateway {
                    runner: &runner,
                    args: &self.gateway,
                    allow_unscoped_providers: true,
                },
                self.replace,
            )?;
        } else {
            fetch_locked_package_source(
                package,
                &self.cache_dir,
                &self.provider_payload,
                self.replace,
            )?;
        }
        println!(
            "fetched {} to {}",
            package.package,
            cache_source_path(&self.cache_dir, package).display()
        );
        Ok(())
    }
}

impl CacheImportArgs {
    fn run(self) -> Result<()> {
        let lockfile = read_lockfile(&self.lockfile)?;
        let package = lockfile
            .find_package(&self.package)
            .ok_or_else(|| eyre!("lockfile does not contain `{}`", self.package))?;
        let destination = cache_source_path(&self.cache_dir, package);
        if destination.exists() {
            if !self.replace {
                bail!(
                    "cache source path `{}` already exists; pass --replace to overwrite it",
                    destination.display()
                );
            }
            fs::remove_dir_all(&destination)
                .wrap_err_with(|| format!("failed to remove `{}`", destination.display()))?;
        }
        copy_source_tree(&self.source_root, &destination)?;
        verify_cached_package(&self.cache_dir, package)?;
        println!("imported {} to {}", package.package, destination.display());
        Ok(())
    }
}

impl CachePathArgs {
    fn run(self) -> Result<()> {
        let lockfile = read_lockfile(&self.lockfile)?;
        for package in &lockfile.packages {
            let path = cache_source_path(&self.cache_dir, package);
            println!(
                "{} {} cached={}",
                package.alias,
                package.package,
                path.exists()
            );
        }
        Ok(())
    }

    fn verify(self) -> Result<()> {
        let lockfile = read_lockfile(&self.lockfile)?;
        for package in &lockfile.packages {
            verify_cached_package(&self.cache_dir, package)?;
            println!("verified {} {}", package.alias, package.package);
        }
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
struct CachePruneArgs {
    /// Lockfile path whose packages should be retained
    #[arg(long, default_value = DEFAULT_LOCKFILE)]
    lockfile: PathBuf,
    /// Local cache directory
    #[arg(long, default_value = DEFAULT_CACHE_DIR)]
    cache_dir: PathBuf,
    /// Print entries that would be removed
    #[arg(long)]
    dry_run: bool,
}

impl CachePruneArgs {
    fn run(self) -> Result<()> {
        let lockfile = read_lockfile(&self.lockfile)?;
        let keep = lockfile
            .packages
            .iter()
            .map(|package| cache_entry_path(&self.cache_dir, package))
            .collect::<BTreeSet<_>>();
        let registry_dir = self.cache_dir.join("registry");
        if !registry_dir.exists() {
            return Ok(());
        }
        for digest_entry in fs::read_dir(&registry_dir)? {
            let digest_entry = digest_entry?;
            if !digest_entry.file_type()?.is_dir() {
                continue;
            }
            for archive_entry in fs::read_dir(digest_entry.path())? {
                let archive_entry = archive_entry?;
                let path = archive_entry.path();
                if archive_entry.file_type()?.is_dir() && !keep.contains(&path) {
                    println!("prune {}", path.display());
                    if !self.dry_run {
                        fs::remove_dir_all(&path)
                            .wrap_err_with(|| format!("failed to remove `{}`", path.display()))?;
                    }
                }
            }
        }
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
                println!(
                    "  {} = {{ package = \"{}\", version = \"{}\" }}",
                    dependency.alias, dependency.package, dependency.version_req
                );
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
        self.exports.dedup();
        if let Some(dapp) = &mut self.dapp {
            dapp.contracts.sort();
            dapp.contracts.dedup();
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
    package: MusubiPackageId,
    version_req: MusubiVersionReq,
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
    fn new(mut packages: Vec<LockedPackage>) -> Self {
        packages.sort_by(|left, right| {
            left.package
                .cmp(&right.package)
                .then_with(|| left.alias.cmp(&right.alias))
        });
        packages.dedup_by(|left, right| left.package == right.package);
        Self { packages }
    }

    fn from_manifest(manifest: &MusubiManifest) -> Self {
        let packages = manifest
            .dependencies
            .iter()
            .map(|dependency| LockedPackage {
                alias: dependency.alias.clone(),
                package: MusubiPackageRef::new(
                    dependency.package.clone(),
                    dependency
                        .version_req
                        .exact_version()
                        .unwrap_or_else(|| "0.0.0".parse().expect("zero version")),
                ),
                version_req: dependency.version_req.clone(),
                archive: None,
                source_plan: None,
                cache_path: None,
                exports: Vec::new(),
                dependencies: Vec::new(),
                direct: true,
                resolved: false,
            })
            .collect::<Vec<_>>();
        Self::new(packages)
    }

    fn find_package(&self, raw: &str) -> Option<&LockedPackage> {
        self.packages.iter().find(|package| {
            package.alias.as_ref() == raw
                || package.package.to_string() == raw
                || package.package.package.to_string() == raw
        })
    }

    fn package_by_ref(&self, reference: &MusubiPackageRef) -> Option<&LockedPackage> {
        self.packages
            .iter()
            .find(|package| &package.package == reference)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LockedPackage {
    alias: Name,
    package: MusubiPackageRef,
    version_req: MusubiVersionReq,
    archive: Option<MusubiArchiveRef>,
    source_plan: Option<MusubiSourceArchivePlan>,
    cache_path: Option<PathBuf>,
    exports: Vec<Name>,
    dependencies: Vec<LockedDependency>,
    direct: bool,
    resolved: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LockedDependency {
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

fn read_lockfile_optional(path: &Path) -> Result<Option<MusubiLockfile>> {
    if path.exists() {
        read_lockfile(path).map(Some)
    } else {
        Ok(None)
    }
}

fn read_lockfile(path: &Path) -> Result<MusubiLockfile> {
    let body = fs::read_to_string(path)
        .wrap_err_with(|| format!("failed to read `{}`", path.display()))?;
    parse_lockfile(&body).wrap_err_with(|| format!("failed to parse `{}`", path.display()))
}

fn parse_lockfile(body: &str) -> Result<MusubiLockfile> {
    let value: toml::Value = toml::from_str(body)?;
    let root = value
        .as_table()
        .ok_or_else(|| eyre!("Musubi.lock must be a TOML table"))?;
    let version = root
        .get("version")
        .and_then(toml::Value::as_integer)
        .unwrap_or(1);
    let packages = root
        .get("package")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| eyre!("`package` array is required"))?;
    let mut locked = Vec::with_capacity(packages.len());
    for value in packages {
        let table = value
            .as_table()
            .ok_or_else(|| eyre!("lockfile package entries must be tables"))?;
        let alias = required_string(table, "alias")?.parse()?;
        let package_id: MusubiPackageId = required_string(table, "name")?.parse()?;
        let locked_version: MusubiVersion = required_string(table, "version")?.parse()?;
        let version_req = if version >= 2 {
            required_string(table, "requirement")?.parse()?
        } else {
            MusubiVersionReq::new(locked_version.as_str())?
        };
        let archive = parse_lockfile_archive(table)?;
        let source_plan = parse_lockfile_source_plan(table)?;
        let cache_path = table
            .get("cache_path")
            .and_then(toml::Value::as_str)
            .map(PathBuf::from);
        let exports = parse_name_array(table, "exports")?;
        let dependencies = parse_lockfile_dependency_array(table)?;
        let direct = table
            .get("direct")
            .and_then(toml::Value::as_bool)
            .unwrap_or(true);
        let resolved = table
            .get("resolved")
            .and_then(toml::Value::as_bool)
            .unwrap_or_else(|| archive.is_some());
        locked.push(LockedPackage {
            alias,
            package: MusubiPackageRef::new(package_id, locked_version),
            version_req,
            archive,
            source_plan,
            cache_path,
            exports,
            dependencies,
            direct,
            resolved,
        });
    }
    Ok(MusubiLockfile::new(locked))
}

fn parse_lockfile_archive(table: &toml::Table) -> Result<Option<MusubiArchiveRef>> {
    let Some(digest) = table
        .get("sorafs_manifest_digest")
        .and_then(toml::Value::as_str)
    else {
        return Ok(None);
    };
    let archive_hash = required_string(table, "archive_hash_blake3_256")?;
    let source_bytes = table
        .get("source_bytes")
        .and_then(toml::Value::as_integer)
        .ok_or_else(|| eyre!("`source_bytes` integer is required when archive is present"))?;
    let source_file_count = table
        .get("source_file_count")
        .and_then(toml::Value::as_integer)
        .ok_or_else(|| eyre!("`source_file_count` integer is required when archive is present"))?;
    Ok(Some(MusubiArchiveRef::new(
        ManifestDigest::new(parse_hex_32(digest)?),
        parse_hex_32(archive_hash)?,
        u64::try_from(source_bytes).map_err(|_| eyre!("source_bytes must be non-negative"))?,
        u32::try_from(source_file_count).map_err(|_| eyre!("source_file_count must fit u32"))?,
    )))
}

fn parse_lockfile_source_plan(table: &toml::Table) -> Result<Option<MusubiSourceArchivePlan>> {
    let Some(plan_hex) = table
        .get("source_archive_plan_norito")
        .and_then(toml::Value::as_str)
    else {
        return Ok(None);
    };
    let bytes = hex::decode(plan_hex).wrap_err("source_archive_plan_norito is not valid hex")?;
    let mut cursor = bytes.as_slice();
    let plan = MusubiSourceArchivePlan::decode(&mut cursor)
        .map_err(|err| eyre!("failed to decode source archive plan: {err}"))?;
    if !cursor.is_empty() {
        bail!("source_archive_plan_norito contains trailing bytes");
    }
    Ok(Some(plan))
}

fn parse_lockfile_dependency_array(table: &toml::Table) -> Result<Vec<LockedDependency>> {
    let Some(value) = table.get("dependencies") else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .ok_or_else(|| eyre!("`dependencies` must be an array of tables"))?;
    let mut dependencies = Vec::with_capacity(values.len());
    for value in values {
        let table = value
            .as_table()
            .ok_or_else(|| eyre!("lockfile dependency entries must be tables"))?;
        dependencies.push(LockedDependency {
            alias: required_string(table, "alias")?.parse()?,
            package: required_string(table, "package")?.parse()?,
        });
    }
    dependencies.sort_by(|left, right| {
        left.alias
            .cmp(&right.alias)
            .then_with(|| left.package.cmp(&right.package))
    });
    Ok(dependencies)
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
        let (package, version_req) = match value {
            toml::Value::String(value) => {
                let package: MusubiPackageRef = value.parse()?;
                (
                    package.package,
                    MusubiVersionReq::new(package.version.as_str())?,
                )
            }
            toml::Value::Table(table) => {
                let package = required_string(table, "package")?;
                let version = required_string(table, "version")?;
                (package.parse()?, version.parse()?)
            }
            _ => {
                bail!(
                    "dependency `{alias}` must be a string reference or a table with package/version"
                );
            }
        };
        dependencies.push(ManifestDependency {
            alias,
            package,
            version_req,
        });
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
                toml::Value::String(dependency.package.to_string()),
            );
            entry.insert(
                "version".to_owned(),
                toml::Value::String(dependency.version_req.to_string()),
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
    let mut packages = Vec::with_capacity(lockfile.packages.len());
    for package in &lockfile.packages {
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
            "requirement".to_owned(),
            toml::Value::String(package.version_req.to_string()),
        );
        table.insert(
            "source".to_owned(),
            toml::Value::String("registry".to_owned()),
        );
        table.insert(
            "resolved".to_owned(),
            toml::Value::Boolean(package.resolved),
        );
        table.insert("direct".to_owned(), toml::Value::Boolean(package.direct));
        if let Some(archive) = package.archive {
            let source_bytes = i64::try_from(archive.source_bytes)
                .map_err(|_| eyre!("source byte count does not fit TOML integer"))?;
            table.insert(
                "sorafs_manifest_digest".to_owned(),
                toml::Value::String(hex::encode(archive.sorafs_manifest.as_bytes())),
            );
            table.insert(
                "archive_hash_blake3_256".to_owned(),
                toml::Value::String(hex::encode(archive.archive_hash_blake3_256)),
            );
            table.insert(
                "source_bytes".to_owned(),
                toml::Value::Integer(source_bytes),
            );
            table.insert(
                "source_file_count".to_owned(),
                toml::Value::Integer(i64::from(archive.source_file_count)),
            );
        }
        if let Some(source_plan) = &package.source_plan {
            table.insert(
                "source_archive_plan_norito".to_owned(),
                toml::Value::String(hex::encode(source_plan.encode())),
            );
        }
        if let Some(path) = &package.cache_path {
            table.insert(
                "cache_path".to_owned(),
                toml::Value::String(path.display().to_string()),
            );
        }
        if !package.exports.is_empty() {
            table.insert(
                "exports".to_owned(),
                toml::Value::Array(
                    package
                        .exports
                        .iter()
                        .map(|name| toml::Value::String(name.to_string()))
                        .collect(),
                ),
            );
        }
        if !package.dependencies.is_empty() {
            let dependencies = package
                .dependencies
                .iter()
                .map(|dependency| {
                    let mut table = toml::Table::new();
                    table.insert(
                        "alias".to_owned(),
                        toml::Value::String(dependency.alias.to_string()),
                    );
                    table.insert(
                        "package".to_owned(),
                        toml::Value::String(dependency.package.to_string()),
                    );
                    toml::Value::Table(table)
                })
                .collect::<Vec<_>>();
            table.insert("dependencies".to_owned(), toml::Value::Array(dependencies));
        }
        packages.push(toml::Value::Table(table));
    }
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

fn validate_exported_functions_exist(root: &Path, exports: &[Name]) -> Result<()> {
    if exports.is_empty() {
        return Ok(());
    }
    let defined = collect_kotodama_functions(root)?;
    if defined.is_empty() {
        bail!(
            "Musubi release exports functions but no Kotodama `.ko` source files were found under `{}`",
            root.display()
        );
    }
    for export in exports {
        if !defined.contains(export) {
            bail!(
                "Musubi export `{export}` is not defined by any Kotodama source under `{}`",
                root.display()
            );
        }
    }
    Ok(())
}

fn collect_kotodama_functions(root: &Path) -> Result<BTreeSet<Name>> {
    let root = root
        .canonicalize()
        .wrap_err_with(|| format!("failed to canonicalize `{}`", root.display()))?;
    let mut files = Vec::new();
    collect_source_files(&root, &root, &mut files)?;

    let mut functions = BTreeSet::new();
    for relative in files.into_iter().filter(|path| {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension == "ko")
    }) {
        let path = root.join(&relative);
        let source = fs::read_to_string(&path)
            .wrap_err_with(|| format!("failed to read `{}`", path.display()))?;
        let program = parse_kotodama(&source)
            .map_err(|err| eyre!("failed to parse `{}`: {err}", path.display()))?;
        for item in program.items {
            if let Item::Function(function) = item {
                functions.insert(function.name.parse::<Name>().map_err(|err| {
                    eyre!(
                        "Kotodama function `{}` in `{}` is not a valid Musubi export name: {}",
                        function.name,
                        path.display(),
                        err.reason()
                    )
                })?);
            }
        }
    }
    Ok(functions)
}

fn release_from_manifest(
    manifest: &MusubiManifest,
    lockfile: Option<&MusubiLockfile>,
    archive: MusubiArchiveRef,
    source_archive_plan: MusubiSourceArchivePlan,
    published_by: iroha_data_model::account::AccountId,
    published_at_ms: u64,
) -> Result<MusubiRelease> {
    let dependencies = resolved_release_dependencies(manifest, lockfile)?;
    let dapp = manifest
        .dapp
        .as_ref()
        .map(|dapp| MusubiDappLink::new(dapp.namespace.clone(), dapp.contracts.clone()))
        .transpose()?;
    Ok(MusubiRelease::new(
        manifest.package.package_ref(),
        archive,
        dependencies,
        manifest.exports.clone(),
        dapp,
        published_by,
        published_at_ms,
    )
    .with_source_archive_plan(source_archive_plan))
}

fn resolved_release_dependencies(
    manifest: &MusubiManifest,
    lockfile: Option<&MusubiLockfile>,
) -> Result<Vec<MusubiDependency>> {
    let mut dependencies = Vec::with_capacity(manifest.dependencies.len());
    for dependency in &manifest.dependencies {
        let package = if let Some(lockfile) = lockfile {
            let locked = lockfile
                .packages
                .iter()
                .find(|locked| locked.direct && locked.alias == dependency.alias)
                .ok_or_else(|| {
                    eyre!(
                        "dependency `{}` is not present in lockfile; run `musubi install`",
                        dependency.alias
                    )
                })?;
            if locked.package.package != dependency.package
                || !dependency.version_req.matches(&locked.package.version)?
            {
                bail!(
                    "lockfile dependency `{}` does not satisfy manifest requirement {} {}",
                    dependency.alias,
                    dependency.package,
                    dependency.version_req
                );
            }
            locked.package.clone()
        } else {
            let version = dependency.version_req.exact_version().ok_or_else(|| {
                eyre!(
                    "dependency `{}` uses range `{}`; provide a lockfile for publish",
                    dependency.alias,
                    dependency.version_req
                )
            })?;
            MusubiPackageRef::new(dependency.package.clone(), version)
        };
        dependencies.push(MusubiDependency::new(dependency.alias.clone(), package));
    }
    Ok(dependencies)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SourceArchiveStats {
    archive_hash_blake3_256: [u8; 32],
    source_bytes: u64,
    source_file_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SourceFileEntry {
    relative: PathBuf,
    components: Vec<String>,
    bytes: Vec<u8>,
}

fn hash_source_tree(root: &Path) -> Result<SourceArchiveStats> {
    let files = collect_source_file_entries(root)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(ARCHIVE_DOMAIN_SEPARATOR);
    let mut source_bytes = 0_u64;
    let mut source_file_count = 0_u32;
    for entry in files {
        let relative = entry.relative.to_string_lossy().replace('\\', "/");
        hasher.update(relative.as_bytes());
        hasher.update(b"\0");
        hasher.update(&(entry.bytes.len() as u64).to_be_bytes());
        hasher.update(b"\0");
        hasher.update(&entry.bytes);
        hasher.update(b"\0");
        source_bytes = source_bytes
            .checked_add(entry.bytes.len() as u64)
            .ok_or_else(|| eyre!("source archive byte count overflow"))?;
        source_file_count = source_file_count
            .checked_add(1)
            .ok_or_else(|| eyre!("source archive file count overflow"))?;
    }
    Ok(SourceArchiveStats {
        archive_hash_blake3_256: *hasher.finalize().as_bytes(),
        source_bytes,
        source_file_count,
    })
}

fn collect_source_file_entries(root: &Path) -> Result<Vec<SourceFileEntry>> {
    let root = root
        .canonicalize()
        .wrap_err_with(|| format!("failed to canonicalize `{}`", root.display()))?;
    let mut files = Vec::new();
    collect_source_files(&root, &root, &mut files)?;
    files.sort();
    files
        .into_iter()
        .map(|relative| {
            let path = root.join(&relative);
            let bytes =
                fs::read(&path).wrap_err_with(|| format!("failed to read `{}`", path.display()))?;
            let components = relative_path_components(&relative)?;
            Ok(SourceFileEntry {
                relative,
                components,
                bytes,
            })
        })
        .collect()
}

fn relative_path_components(relative: &Path) -> Result<Vec<String>> {
    let mut components = Vec::new();
    for component in relative.components() {
        match component {
            std::path::Component::Normal(os) => {
                let value = os
                    .to_str()
                    .ok_or_else(|| eyre!("source path `{}` is not UTF-8", relative.display()))?;
                if value.is_empty() || value.contains('/') {
                    bail!("source path `{}` is invalid", relative.display());
                }
                components.push(value.to_owned());
            }
            std::path::Component::CurDir => {}
            _ => bail!("source path `{}` is invalid", relative.display()),
        }
    }
    if components.is_empty() {
        bail!("source path `{}` is empty", relative.display());
    }
    Ok(components)
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

fn copy_source_tree(source_root: &Path, destination_root: &Path) -> Result<()> {
    let source_root = source_root
        .canonicalize()
        .wrap_err_with(|| format!("failed to canonicalize `{}`", source_root.display()))?;
    let mut files = Vec::new();
    collect_source_files(&source_root, &source_root, &mut files)?;
    fs::create_dir_all(destination_root)
        .wrap_err_with(|| format!("failed to create `{}`", destination_root.display()))?;
    for relative in files {
        let source = source_root.join(&relative);
        let destination = destination_root.join(&relative);
        ensure_parent_dir(&destination)?;
        fs::copy(&source, &destination).wrap_err_with(|| {
            format!(
                "failed to copy `{}` to `{}`",
                source.display(),
                destination.display()
            )
        })?;
    }
    Ok(())
}

fn should_skip_path(file_name: &str) -> bool {
    matches!(
        file_name,
        ".git" | "target" | DEFAULT_LOCKFILE | ".musubi" | "dist"
    )
}

fn parse_hex_32(raw: &str) -> Result<[u8; 32]> {
    let raw = raw.strip_prefix("0x").unwrap_or(raw);
    let bytes = hex::decode(raw).wrap_err("hex string is invalid")?;
    let array: [u8; 32] = bytes
        .try_into()
        .map_err(|bytes: Vec<u8>| eyre!("expected 32 bytes, got {}", bytes.len()))?;
    Ok(array)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AddDependencyPackage {
    package: MusubiPackageId,
    version_req: MusubiVersionReq,
}

fn parse_add_dependency_package(
    raw: &str,
    version: Option<MusubiVersionReq>,
    client_args: Option<&ClientArgs>,
) -> Result<AddDependencyPackage> {
    if raw.contains('@') {
        if version.is_some() {
            bail!("do not pass --version with an exact package reference");
        }
        if let Ok(package) = raw.parse::<MusubiPackageRef>() {
            return Ok(AddDependencyPackage {
                package: package.package,
                version_req: MusubiVersionReq::new(package.version.as_str())?,
            });
        }
        let (alias, exact_version) = raw
            .split_once('@')
            .ok_or_else(|| eyre!("package reference must contain @version"))?;
        if alias.contains('/') {
            bail!("invalid package reference `{raw}`");
        }
        let package = resolve_short_alias(client_args, alias)?;
        return Ok(AddDependencyPackage {
            package,
            version_req: MusubiVersionReq::new(exact_version)?,
        });
    }
    let package = if raw.contains('/') {
        raw.parse()?
    } else {
        resolve_short_alias(client_args, raw)?
    };
    let version_req = version.ok_or_else(|| {
        eyre!("--version is required when adding a package id without an @version suffix")
    })?;
    Ok(AddDependencyPackage {
        package,
        version_req,
    })
}

fn resolve_short_alias(client_args: Option<&ClientArgs>, raw: &str) -> Result<MusubiPackageId> {
    let alias: Name = raw.parse()?;
    let client_args = client_args
        .ok_or_else(|| eyre!("short alias `{alias}` requires Iroha client configuration"))?;
    let (client, _) = client_args.load()?;
    client
        .query_single(FindMusubiShortAliasByName::new(alias.clone()))
        .wrap_err_with(|| format!("failed to resolve Musubi short alias `{alias}`"))
}

#[derive(Clone, Copy)]
enum ResolveMode<'a> {
    Install,
    Update { package: Option<&'a str> },
}

fn resolve_manifest_dependencies(
    client: &Client,
    manifest: &MusubiManifest,
    _existing: Option<&MusubiLockfile>,
    cache_dir: &Path,
    mode: ResolveMode<'_>,
) -> Result<MusubiLockfile> {
    let _requested_update = match mode {
        ResolveMode::Install => None,
        ResolveMode::Update { package } => package,
    };
    let mut packages = BTreeMap::<MusubiPackageRef, LockedPackage>::new();
    let mut resolving = BTreeSet::<MusubiPackageRef>::new();
    for dependency in &manifest.dependencies {
        let release = resolve_dependency_release(client, dependency)?;
        insert_resolved_release(
            client,
            dependency.alias.clone(),
            dependency.version_req.clone(),
            release,
            true,
            cache_dir,
            &mut packages,
            &mut resolving,
        )?;
    }
    Ok(MusubiLockfile::new(packages.into_values().collect()))
}

fn resolve_dependency_release(
    client: &Client,
    dependency: &ManifestDependency,
) -> Result<MusubiRelease> {
    let releases = client
        .query_single(FindMusubiPackageReleases {
            package: dependency.package.clone(),
            include_yanked: false,
        })
        .wrap_err_with(|| format!("failed to resolve Musubi package `{}`", dependency.package))?;
    let selected = releases
        .into_iter()
        .filter(|release| {
            dependency
                .version_req
                .matches(&release.package.version)
                .unwrap_or(false)
        })
        .max_by(|left, right| {
            left.package
                .version
                .precedence_cmp(&right.package.version)
                .unwrap_or_else(|_| left.package.version.cmp(&right.package.version))
        })
        .ok_or_else(|| {
            eyre!(
                "no active release of `{}` satisfies `{}`",
                dependency.package,
                dependency.version_req
            )
        })?;
    fetch_active_release(client, selected.package)
}

#[allow(clippy::too_many_arguments)]
fn insert_resolved_release(
    client: &Client,
    alias: Name,
    version_req: MusubiVersionReq,
    release: MusubiRelease,
    direct: bool,
    cache_dir: &Path,
    packages: &mut BTreeMap<MusubiPackageRef, LockedPackage>,
    resolving: &mut BTreeSet<MusubiPackageRef>,
) -> Result<()> {
    if !resolving.insert(release.package.clone()) {
        bail!("cyclic Musubi dependency detected at `{}`", release.package);
    }

    let dependency_links = release
        .dependencies
        .iter()
        .map(|dependency| LockedDependency {
            alias: dependency.alias.clone(),
            package: dependency.package.clone(),
        })
        .collect::<Vec<_>>();

    for dependency in &release.dependencies {
        let dependency_release = fetch_active_release(client, dependency.package.clone())?;
        insert_resolved_release(
            client,
            dependency.alias.clone(),
            MusubiVersionReq::new(dependency.package.version.as_str())?,
            dependency_release,
            false,
            cache_dir,
            packages,
            resolving,
        )?;
    }

    let mut locked = locked_package_from_release(
        alias,
        version_req,
        &release,
        dependency_links,
        direct,
        cache_dir,
    );
    if cache_source_path(cache_dir, &locked).exists()
        && let Err(err) = verify_cached_package(cache_dir, &locked)
    {
        eprintln!(
            "warning: cached package `{}` failed verification and will be refetched later: {err}",
            locked.package
        );
    }
    packages
        .entry(locked.package.clone())
        .and_modify(|existing| {
            if direct {
                existing.alias = locked.alias.clone();
                existing.version_req = locked.version_req.clone();
                existing.direct = true;
            }
        })
        .or_insert_with(|| {
            locked.cache_path = Some(cache_source_path(cache_dir, &locked));
            locked
        });
    resolving.remove(&release.package);
    Ok(())
}

fn fetch_active_release(client: &Client, package: MusubiPackageRef) -> Result<MusubiRelease> {
    let release = client
        .query_single(FindMusubiReleaseByRef {
            package: package.clone(),
        })
        .wrap_err_with(|| format!("failed to fetch Musubi release `{package}`"))?;
    if release.status.is_active() {
        Ok(release)
    } else {
        bail!("Musubi dependency `{package}` is yanked and cannot be selected")
    }
}

fn locked_package_from_release(
    alias: Name,
    version_req: MusubiVersionReq,
    release: &MusubiRelease,
    dependencies: Vec<LockedDependency>,
    direct: bool,
    cache_dir: &Path,
) -> LockedPackage {
    let mut locked = LockedPackage {
        alias,
        package: release.package.clone(),
        version_req,
        archive: Some(release.archive),
        source_plan: release.source_archive_plan.clone(),
        cache_path: None,
        exports: release.exports.clone(),
        dependencies,
        direct,
        resolved: true,
    };
    locked.cache_path = Some(cache_source_path(cache_dir, &locked));
    locked
}

fn validate_lockfile_satisfies_manifest(
    lockfile: &MusubiLockfile,
    manifest: &MusubiManifest,
) -> Result<()> {
    for dependency in &manifest.dependencies {
        let locked = lockfile
            .packages
            .iter()
            .find(|locked| locked.direct && locked.alias == dependency.alias)
            .ok_or_else(|| eyre!("lockfile is missing dependency `{}`", dependency.alias))?;
        if locked.package.package != dependency.package
            || !dependency.version_req.matches(&locked.package.version)?
        {
            bail!(
                "lockfile dependency `{}` does not satisfy {} {}",
                dependency.alias,
                dependency.package,
                dependency.version_req
            );
        }
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SorafsBuildSummary {
    digest: ManifestDigest,
    car_hash: [u8; 32],
    chunk_digest_sha3_256: [u8; 32],
    chunker: ChunkerProfileHandle,
    pin_policy: DataModelPinPolicy,
    source_plan: MusubiSourceArchivePlan,
    manifest_bytes: Vec<u8>,
    payload: Vec<u8>,
}

fn build_sorafs_source_manifest(
    manifest: &MusubiManifest,
    source_root: &Path,
    car_out: Option<&Path>,
    manifest_out: Option<&Path>,
    source_plan_out: Option<&Path>,
    archive: SourceArchiveStats,
) -> Result<SorafsBuildSummary> {
    let source_files = collect_source_file_entries(source_root)?;
    let car_files = source_files
        .into_iter()
        .map(|entry| FileEntry {
            path: entry.components,
            data: entry.bytes,
        })
        .collect::<Vec<_>>();
    let (plan, payload) = CarBuildPlan::from_files(car_files)
        .map_err(|err| eyre!("failed to build SoraFS CAR plan: {err}"))?;
    let mut car_bytes = Vec::new();
    let stats = CarWriter::new(&plan, &payload)
        .map_err(|err| eyre!("failed to prepare SoraFS CAR writer: {err}"))?
        .write_to(&mut car_bytes)
        .map_err(|err| eyre!("failed to write SoraFS CAR: {err}"))?;
    if let Some(path) = car_out {
        ensure_parent_dir(path)?;
        fs::write(path, &car_bytes)
            .wrap_err_with(|| format!("failed to write `{}`", path.display()))?;
    }
    let root_cid = stats
        .root_cids
        .first()
        .cloned()
        .ok_or_else(|| eyre!("SoraFS CAR writer produced no root CID"))?;
    let manifest_v1 = ManifestBuilder::new()
        .root_cid(root_cid)
        .dag_codec(DagCodecId(MANIFEST_DAG_CODEC))
        .chunking_from_profile(plan.chunk_profile, BLAKE3_256_MULTIHASH_CODE)
        .content_length(plan.content_length)
        .car_digest(*stats.car_archive_digest.as_bytes())
        .car_size(stats.car_size)
        .pin_policy(sorafs_manifest::PinPolicy::default())
        .extend_metadata([
            (
                "musubi.package".to_owned(),
                manifest.package.package_ref().to_string(),
            ),
            (
                "musubi.archive_hash_blake3_256".to_owned(),
                hex::encode(archive.archive_hash_blake3_256),
            ),
            (
                "musubi.source_bytes".to_owned(),
                archive.source_bytes.to_string(),
            ),
            (
                "musubi.source_file_count".to_owned(),
                archive.source_file_count.to_string(),
            ),
        ])
        .build()
        .map_err(|err| eyre!("failed to build SoraFS manifest: {err}"))?;
    let digest = ManifestDigest::from_manifest(&manifest_v1)
        .map_err(|err| eyre!("failed to digest SoraFS manifest: {err}"))?;
    let chunk_digest_sha3_256 = compute_chunk_plan_digest_sha3(&plan.chunks);
    let chunker = chunker_from_manifest(&manifest_v1);
    let pin_policy = pin_policy_from_manifest(&manifest_v1);
    let source_plan = source_archive_plan_from_car_plan(
        &plan,
        *stats.car_archive_digest.as_bytes(),
        stats.car_size,
    )?;
    let manifest_bytes = manifest_v1
        .encode()
        .map_err(|err| eyre!("failed to encode SoraFS manifest: {err}"))?;
    if let Some(path) = manifest_out {
        ensure_parent_dir(path)?;
        fs::write(path, &manifest_bytes)
            .wrap_err_with(|| format!("failed to write `{}`", path.display()))?;
    }
    if let Some(path) = source_plan_out {
        ensure_parent_dir(path)?;
        fs::write(path, source_plan.encode())
            .wrap_err_with(|| format!("failed to write `{}`", path.display()))?;
    }
    Ok(SorafsBuildSummary {
        digest,
        car_hash: *stats.car_archive_digest.as_bytes(),
        chunk_digest_sha3_256,
        chunker,
        pin_policy,
        source_plan,
        manifest_bytes,
        payload,
    })
}

fn source_archive_plan_from_car_plan(
    plan: &CarBuildPlan,
    car_hash_blake3_256: [u8; 32],
    car_size: u64,
) -> Result<MusubiSourceArchivePlan> {
    let chunks = plan
        .chunks
        .iter()
        .map(|chunk| MusubiSourceChunkPlan::new(chunk.offset, chunk.length, chunk.digest))
        .collect::<Vec<_>>();
    let files = plan
        .files
        .iter()
        .map(|file| {
            Ok(MusubiSourceFilePlan::new(
                file.path.clone(),
                u32::try_from(file.first_chunk)
                    .map_err(|_| eyre!("source archive first_chunk does not fit u32"))?,
                u32::try_from(file.chunk_count)
                    .map_err(|_| eyre!("source archive chunk_count does not fit u32"))?,
                file.size,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(MusubiSourceArchivePlan::new(
        *plan.payload_digest.as_bytes(),
        plan.content_length,
        car_hash_blake3_256,
        car_size,
        chunks,
        files,
    ))
}

fn chunker_from_manifest(manifest: &sorafs_manifest::ManifestV1) -> ChunkerProfileHandle {
    ChunkerProfileHandle {
        profile_id: manifest.chunking.profile_id.0,
        namespace: manifest.chunking.namespace.clone(),
        name: manifest.chunking.name.clone(),
        semver: manifest.chunking.semver.clone(),
        multihash_code: manifest.chunking.multihash_code,
    }
}

fn pin_policy_from_manifest(manifest: &sorafs_manifest::ManifestV1) -> DataModelPinPolicy {
    DataModelPinPolicy {
        min_replicas: manifest.pin_policy.min_replicas,
        storage_class: storage_class_from_manifest(manifest.pin_policy.storage_class),
        retention_epoch: manifest.pin_policy.retention_epoch,
    }
}

fn storage_class_from_manifest(storage_class: sorafs_manifest::StorageClass) -> StorageClass {
    match storage_class {
        sorafs_manifest::StorageClass::Hot => StorageClass::Hot,
        sorafs_manifest::StorageClass::Warm => StorageClass::Warm,
        sorafs_manifest::StorageClass::Cold => StorageClass::Cold,
    }
}

struct PublishArtifactPaths {
    car: PathBuf,
    manifest: PathBuf,
    source_plan: PathBuf,
}

fn default_publish_artifact_paths(manifest: &MusubiManifest) -> PublishArtifactPaths {
    let root = PathBuf::from(DEFAULT_DIST_DIR)
        .join(manifest.package.namespace.to_string())
        .join(manifest.package.name.to_string())
        .join(manifest.package.version.to_string());
    PublishArtifactPaths {
        car: root.join("source.car"),
        manifest: root.join("manifest.norito"),
        source_plan: root.join("source-plan.norito"),
    }
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .wrap_err_with(|| format!("failed to create `{}`", parent.display()))?;
    }
    Ok(())
}

fn cache_entry_path(cache_dir: &Path, package: &LockedPackage) -> PathBuf {
    if let Some(archive) = package.archive {
        cache_dir
            .join("registry")
            .join(hex::encode(archive.sorafs_manifest.as_bytes()))
            .join(hex::encode(archive.archive_hash_blake3_256))
    } else {
        cache_dir
            .join("registry")
            .join("unresolved")
            .join(package.alias.to_string())
    }
}

fn cache_source_path(cache_dir: &Path, package: &LockedPackage) -> PathBuf {
    package
        .cache_path
        .clone()
        .unwrap_or_else(|| cache_entry_path(cache_dir, package).join("src"))
}

fn verify_cached_package(cache_dir: &Path, package: &LockedPackage) -> Result<()> {
    let archive = package
        .archive
        .ok_or_else(|| eyre!("package `{}` has no archive metadata", package.package))?;
    let source_path = cache_source_path(cache_dir, package);
    if !source_path.exists() {
        bail!(
            "cache source path `{}` does not exist",
            source_path.display()
        );
    }
    let stats = hash_source_tree(&source_path)?;
    if stats.archive_hash_blake3_256 != archive.archive_hash_blake3_256
        || stats.source_bytes != archive.source_bytes
        || stats.source_file_count != archive.source_file_count
    {
        bail!(
            "cache source for `{}` does not match lockfile archive",
            package.package
        );
    }
    Ok(())
}

fn fetch_missing_lockfile_sources(
    lockfile: &MusubiLockfile,
    cache_dir: &Path,
    fetch_mode: SourceFetchMode<'_>,
) -> Result<()> {
    for package in &lockfile.packages {
        let source_path = cache_source_path(cache_dir, package);
        if source_path.exists() {
            verify_cached_package(cache_dir, package)?;
            continue;
        }
        fetch_locked_package_source_from(package, cache_dir, fetch_mode, false)?;
    }
    Ok(())
}

fn fetch_locked_package_source(
    package: &LockedPackage,
    cache_dir: &Path,
    provider_payloads: &[PathBuf],
    replace: bool,
) -> Result<()> {
    fetch_locked_package_source_from(
        package,
        cache_dir,
        SourceFetchMode::ProviderPayloads(provider_payloads),
        replace,
    )
}

#[derive(Clone, Copy)]
enum SourceFetchMode<'a> {
    ProviderPayloads(&'a [PathBuf]),
    Gateway {
        runner: &'a dyn GatewayFetchRunner,
        args: &'a GatewayFetchArgs,
        allow_unscoped_providers: bool,
    },
}

trait GatewayFetchRunner {
    fn fetch(&self, request: GatewayFetchRequest) -> Result<Vec<u8>>;
}

struct ClientGatewayFetchRunner<'a> {
    client: &'a Client,
}

impl GatewayFetchRunner for ClientGatewayFetchRunner<'_> {
    fn fetch(&self, request: GatewayFetchRequest) -> Result<Vec<u8>> {
        let runtime =
            tokio::runtime::Runtime::new().wrap_err("failed to initialise Tokio runtime")?;
        let session = runtime
            .block_on(self.client.sorafs_fetch_via_gateway(
                &request.plan,
                request.gateway_config,
                request.providers,
                request.options,
            ))
            .map_err(|err| eyre!("failed to fetch Musubi source through SoraFS gateway: {err}"))?;
        Ok(session.outcome.assemble_payload())
    }
}

#[derive(Debug)]
struct GatewayFetchRequest {
    plan: CarBuildPlan,
    gateway_config: SorafsGatewayFetchConfig,
    providers: Vec<SorafsGatewayProviderInput>,
    options: SorafsGatewayFetchOptions,
}

fn fetch_locked_package_source_from(
    package: &LockedPackage,
    cache_dir: &Path,
    fetch_mode: SourceFetchMode<'_>,
    replace: bool,
) -> Result<()> {
    let destination = cache_source_path(cache_dir, package);
    if destination.exists() {
        if !replace {
            bail!(
                "cache source path `{}` already exists; pass --replace to overwrite it",
                destination.display()
            );
        }
        fs::remove_dir_all(&destination)
            .wrap_err_with(|| format!("failed to remove `{}`", destination.display()))?;
    }

    let source_plan = package
        .source_plan
        .as_ref()
        .ok_or_else(|| eyre!("package `{}` has no source archive plan", package.package))?;
    let payload = match fetch_mode {
        SourceFetchMode::ProviderPayloads(provider_payloads) => {
            read_matching_provider_payload(provider_payloads, source_plan)?
        }
        SourceFetchMode::Gateway {
            runner,
            args,
            allow_unscoped_providers,
        } => {
            let request =
                build_gateway_fetch_request(package, source_plan, args, allow_unscoped_providers)?;
            runner.fetch(request)?
        }
    };
    verify_source_payload(source_plan, &payload)?;
    write_source_payload(source_plan, &payload, &destination)?;
    verify_cached_package(cache_dir, package)?;
    Ok(())
}

fn read_matching_provider_payload(
    provider_payloads: &[PathBuf],
    source_plan: &MusubiSourceArchivePlan,
) -> Result<Vec<u8>> {
    if provider_payloads.is_empty() {
        bail!("cache fetch requires at least one --provider-payload path");
    }
    let mut mismatches = Vec::new();
    for path in provider_payloads {
        let payload =
            fs::read(path).wrap_err_with(|| format!("failed to read `{}`", path.display()))?;
        if payload.len() as u64 != source_plan.content_length {
            mismatches.push(format!(
                "{} length {} != {}",
                path.display(),
                payload.len(),
                source_plan.content_length
            ));
            continue;
        }
        let digest = blake3::hash(&payload);
        if digest.as_bytes() != &source_plan.payload_hash_blake3_256 {
            mismatches.push(format!("{} payload digest mismatch", path.display()));
            continue;
        }
        return Ok(payload);
    }
    bail!(
        "no provider payload matched source archive plan: {}",
        mismatches.join("; ")
    )
}

fn validate_source_fetch_inputs(
    provider_payloads: &[PathBuf],
    gateway: &GatewayFetchArgs,
) -> Result<()> {
    if !provider_payloads.is_empty() && gateway.has_providers() {
        bail!("--provider-payload cannot be combined with --gateway-provider");
    }
    if !gateway.has_providers() && gateway.has_any_args() {
        bail!("gateway fetch options require at least one --gateway-provider");
    }
    if gateway.has_providers() {
        validate_gateway_provider_urls(gateway)?;
    }
    Ok(())
}

fn validate_gateway_provider_urls(gateway: &GatewayFetchArgs) -> Result<()> {
    for provider in &gateway.gateway_provider {
        validate_gateway_provider_url(
            &provider.base_url,
            "--gateway-provider base-url",
            gateway.gateway_allow_insecure_localhost,
        )?;
        if let Some(privacy_events_url) = &provider.privacy_events_url {
            validate_gateway_provider_url(
                privacy_events_url,
                "--gateway-provider privacy-url",
                gateway.gateway_allow_insecure_localhost,
            )?;
        }
    }
    Ok(())
}

fn validate_gateway_provider_url(
    raw: &str,
    label: &str,
    allow_insecure_localhost: bool,
) -> Result<()> {
    let url =
        url::Url::parse(raw.trim()).map_err(|err| eyre!("{label} must be a valid URL: {err}"))?;
    if url.host().is_none() {
        bail!("{label} must include a host");
    }
    match url.scheme() {
        "https" => Ok(()),
        "http" if allow_insecure_localhost && is_loopback_gateway_url(&url) => Ok(()),
        "http" => bail!(
            "{label} must use https; http is only allowed for localhost with --gateway-allow-insecure-localhost"
        ),
        scheme => bail!("{label} must use https, found `{scheme}`"),
    }
}

fn is_loopback_gateway_url(url: &url::Url) -> bool {
    match url.host() {
        Some(url::Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(addr)) => addr.is_loopback(),
        Some(url::Host::Ipv6(addr)) => addr.is_loopback(),
        None => false,
    }
}

fn count_missing_lockfile_sources(lockfile: &MusubiLockfile, cache_dir: &Path) -> usize {
    lockfile
        .packages
        .iter()
        .filter(|package| !cache_source_path(cache_dir, package).exists())
        .count()
}

fn validate_gateway_scope_for_lockfile(
    lockfile: &MusubiLockfile,
    cache_dir: &Path,
    gateway: &GatewayFetchArgs,
) -> Result<()> {
    if count_missing_lockfile_sources(lockfile, cache_dir) > 1
        && gateway
            .gateway_provider
            .iter()
            .any(GatewayProviderSpec::is_unscoped)
    {
        bail!(
            "unscoped --gateway-provider is ambiguous when multiple packages are missing; add package=<alias-or-ref> or manifest=<64-hex>"
        );
    }
    Ok(())
}

fn parse_nonzero_usize(value: &str) -> std::result::Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|err| format!("invalid positive integer `{value}`: {err}"))?;
    if parsed == 0 {
        Err("value must be at least 1".to_owned())
    } else {
        Ok(parsed)
    }
}

fn parse_gateway_provider_spec(value: &str) -> std::result::Result<GatewayProviderSpec, String> {
    let mut name: Option<String> = None;
    let mut provider_id: Option<String> = None;
    let mut base_url: Option<String> = None;
    let mut stream_token: Option<String> = None;
    let mut privacy_events_url: Option<String> = None;
    let mut package: Option<String> = None;
    let mut manifest_id_hex: Option<String> = None;

    for pair in value.split(',') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        let (key, val) = pair.split_once('=').ok_or_else(|| {
            "--gateway-provider expects comma-separated key=value pairs".to_owned()
        })?;
        let val = val.trim();
        match key.trim() {
            "name" => {
                if val.is_empty() {
                    return Err("--gateway-provider name must not be empty".into());
                }
                name = Some(val.to_owned());
            }
            "provider-id" | "provider_id" => {
                provider_id = Some(validate_hex_32_cli(val, "--gateway-provider provider-id")?);
            }
            "base-url" | "base_url" => {
                if val.is_empty() {
                    return Err("--gateway-provider base-url must not be empty".into());
                }
                base_url = Some(val.to_owned());
            }
            "privacy-url" | "privacy_url" => {
                if val.is_empty() {
                    return Err("--gateway-provider privacy-url must not be empty".into());
                }
                privacy_events_url = Some(val.to_owned());
            }
            "stream-token" | "stream_token" => {
                if val.is_empty() {
                    return Err("--gateway-provider stream-token must not be empty".into());
                }
                stream_token = Some(val.to_owned());
            }
            "package" => {
                if val.is_empty() {
                    return Err("--gateway-provider package must not be empty".into());
                }
                package = Some(val.to_owned());
            }
            "manifest" | "manifest-id" | "manifest_id" => {
                manifest_id_hex = Some(validate_hex_32_cli(val, "--gateway-provider manifest")?);
            }
            other => {
                return Err(format!(
                    "unknown --gateway-provider key `{other}`. expected name, provider-id, base-url, stream-token, privacy-url, package, manifest"
                ));
            }
        }
    }

    Ok(GatewayProviderSpec {
        name: name.ok_or_else(|| "--gateway-provider requires name=<alias>".to_owned())?,
        provider_id_hex: provider_id
            .ok_or_else(|| "--gateway-provider requires provider-id=<hex>".to_owned())?,
        base_url: base_url
            .ok_or_else(|| "--gateway-provider requires base-url=<https://...>".to_owned())?,
        stream_token_b64: stream_token
            .ok_or_else(|| "--gateway-provider requires stream-token=<base64>".to_owned())?,
        privacy_events_url,
        package,
        manifest_id_hex,
    })
}

fn validate_hex_32_cli(value: &str, label: &str) -> std::result::Result<String, String> {
    if value.len() != 64 || !value.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("{label} must be 32-byte hex"));
    }
    Ok(value.to_ascii_lowercase())
}

impl GatewayProviderSpec {
    fn is_unscoped(&self) -> bool {
        self.package.is_none() && self.manifest_id_hex.is_none()
    }

    fn matches_package(&self, package: &LockedPackage) -> bool {
        let package_matches = self.package.as_deref().is_none_or(|raw| {
            package.alias.as_ref() == raw
                || package.package.to_string() == raw
                || package.package.package.to_string() == raw
        });
        let manifest_matches = self.manifest_id_hex.as_deref().is_none_or(|manifest| {
            package
                .archive
                .is_some_and(|archive| hex::encode(archive.sorafs_manifest.as_bytes()) == manifest)
        });
        package_matches && manifest_matches
    }

    fn to_input(&self) -> SorafsGatewayProviderInput {
        SorafsGatewayProviderInput {
            name: self.name.clone(),
            provider_id_hex: self.provider_id_hex.clone(),
            base_url: self.base_url.clone(),
            stream_token_b64: self.stream_token_b64.clone(),
            privacy_events_url: self.privacy_events_url.clone(),
        }
    }
}

fn build_gateway_fetch_request(
    package: &LockedPackage,
    source_plan: &MusubiSourceArchivePlan,
    args: &GatewayFetchArgs,
    allow_unscoped_providers: bool,
) -> Result<GatewayFetchRequest> {
    let archive = package
        .archive
        .ok_or_else(|| eyre!("package `{}` has no archive metadata", package.package))?;
    let manifest_id_hex = hex::encode(archive.sorafs_manifest.as_bytes());
    let providers = select_gateway_providers(package, args, allow_unscoped_providers)?;
    if providers.is_empty() {
        bail!(
            "no --gateway-provider matched package `{}` or manifest `{manifest_id_hex}`",
            package.package
        );
    }
    let descriptor = chunker_registry::default_descriptor();
    let gateway_config = SorafsGatewayFetchConfig {
        manifest_id_hex: manifest_id_hex.clone(),
        chunker_handle: descriptor.aliases[0].to_owned(),
        manifest_envelope_b64: None,
        client_id: args.gateway_client_id.clone(),
        expected_manifest_cid_hex: Some(manifest_id_hex),
        blinded_cid_b64: None,
        salt_epoch: None,
        expected_cache_version: None,
        moderation_token_key_b64: None,
    };
    Ok(GatewayFetchRequest {
        plan: car_plan_from_source_archive_plan(source_plan)?,
        gateway_config,
        providers,
        options: gateway_fetch_options(args),
    })
}

fn select_gateway_providers(
    package: &LockedPackage,
    args: &GatewayFetchArgs,
    allow_unscoped_providers: bool,
) -> Result<Vec<SorafsGatewayProviderInput>> {
    args.gateway_provider
        .iter()
        .filter_map(|spec| {
            if spec.is_unscoped() && !allow_unscoped_providers {
                return Some(Err(eyre!(
                    "unscoped --gateway-provider is ambiguous for `{}`; add package=<alias-or-ref> or manifest=<64-hex>",
                    package.package
                )));
            }
            spec.matches_package(package)
                .then(|| Ok(spec.to_input()))
        })
        .collect()
}

fn gateway_fetch_options(args: &GatewayFetchArgs) -> SorafsGatewayFetchOptions {
    SorafsGatewayFetchOptions {
        retry_budget: args.gateway_retry_budget,
        max_peers: args.gateway_max_peers,
        telemetry_region: args.gateway_telemetry_region.clone(),
        scoreboard: args.gateway_scoreboard_out.as_ref().map(|path| {
            SorafsGatewayScoreboardOptions {
                persist_path: Some(path.clone()),
                ..SorafsGatewayScoreboardOptions::default()
            }
        }),
        ..SorafsGatewayFetchOptions::default()
    }
}

fn car_plan_from_source_archive_plan(
    source_plan: &MusubiSourceArchivePlan,
) -> Result<CarBuildPlan> {
    if source_plan.chunks.is_empty() || source_plan.files.is_empty() {
        bail!("source archive plan must contain at least one chunk and one file");
    }
    let descriptor = chunker_registry::default_descriptor();
    let chunks = source_plan
        .chunks
        .iter()
        .map(|chunk| CarChunk {
            offset: chunk.offset,
            length: chunk.length,
            digest: chunk.digest_blake3_256,
            taikai_segment_hint: None,
        })
        .collect();
    let files = source_plan
        .files
        .iter()
        .map(|file| {
            Ok(FilePlan {
                path: file.path.clone(),
                first_chunk: usize::try_from(file.first_chunk)
                    .map_err(|_| eyre!("source file first_chunk does not fit usize"))?,
                chunk_count: usize::try_from(file.chunk_count)
                    .map_err(|_| eyre!("source file chunk_count does not fit usize"))?,
                size: file.size,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(CarBuildPlan {
        chunk_profile: descriptor.profile,
        payload_digest: blake3::Hash::from(source_plan.payload_hash_blake3_256),
        content_length: source_plan.content_length,
        chunks,
        files,
    })
}

fn verify_source_payload(source_plan: &MusubiSourceArchivePlan, payload: &[u8]) -> Result<()> {
    if payload.len() as u64 != source_plan.content_length {
        bail!("source payload length does not match archive plan");
    }
    if blake3::hash(payload).as_bytes() != &source_plan.payload_hash_blake3_256 {
        bail!("source payload digest does not match archive plan");
    }
    for (index, chunk) in source_plan.chunks.iter().enumerate() {
        let start = usize::try_from(chunk.offset)
            .map_err(|_| eyre!("chunk {index} offset does not fit usize"))?;
        let end = start
            .checked_add(chunk.length as usize)
            .ok_or_else(|| eyre!("chunk {index} range overflows"))?;
        let bytes = payload
            .get(start..end)
            .ok_or_else(|| eyre!("chunk {index} range is outside the payload"))?;
        if blake3::hash(bytes).as_bytes() != &chunk.digest_blake3_256 {
            bail!("chunk {index} digest does not match archive plan");
        }
    }
    Ok(())
}

fn write_source_payload(
    source_plan: &MusubiSourceArchivePlan,
    payload: &[u8],
    destination: &Path,
) -> Result<()> {
    fs::create_dir_all(destination)
        .wrap_err_with(|| format!("failed to create `{}`", destination.display()))?;
    for file in &source_plan.files {
        let relative = file
            .path
            .iter()
            .fold(PathBuf::new(), |path, component| path.join(component));
        let output = destination.join(relative);
        ensure_parent_dir(&output)?;
        let bytes = file_payload_bytes(file, &source_plan.chunks, payload)?;
        fs::write(&output, bytes)
            .wrap_err_with(|| format!("failed to write `{}`", output.display()))?;
    }
    Ok(())
}

fn file_payload_bytes<'a>(
    file: &MusubiSourceFilePlan,
    chunks: &[MusubiSourceChunkPlan],
    payload: &'a [u8],
) -> Result<&'a [u8]> {
    if file.size == 0 {
        return Ok(&payload[0..0]);
    }
    let first_chunk = usize::try_from(file.first_chunk)
        .map_err(|_| eyre!("file first_chunk does not fit usize"))?;
    let chunk_count = usize::try_from(file.chunk_count)
        .map_err(|_| eyre!("file chunk_count does not fit usize"))?;
    let first = chunks
        .get(first_chunk)
        .ok_or_else(|| eyre!("file references a missing first chunk"))?;
    let last = chunks
        .get(
            first_chunk
                .checked_add(chunk_count)
                .and_then(|end| end.checked_sub(1))
                .ok_or_else(|| eyre!("file chunk range is empty"))?,
        )
        .ok_or_else(|| eyre!("file references a missing last chunk"))?;
    let start =
        usize::try_from(first.offset).map_err(|_| eyre!("file offset does not fit usize"))?;
    let end_offset = last
        .offset
        .checked_add(u64::from(last.length))
        .ok_or_else(|| eyre!("file end offset overflows"))?;
    let end = usize::try_from(end_offset).map_err(|_| eyre!("file end does not fit usize"))?;
    let expected_end = start
        .checked_add(usize::try_from(file.size).map_err(|_| eyre!("file size too large"))?)
        .ok_or_else(|| eyre!("file size range overflows"))?;
    if end != expected_end {
        bail!("file chunk span does not match file size");
    }
    payload
        .get(start..end)
        .ok_or_else(|| eyre!("file payload range is outside the payload"))
}

fn link_program_with_lockfile(
    source: &str,
    lockfile: &MusubiLockfile,
    cache_dir: &Path,
) -> Result<Program> {
    let mut program = parse_kotodama(source).map_err(|err| eyre!("Kotodama parse error: {err}"))?;
    let dependency_by_alias = lockfile
        .packages
        .iter()
        .filter(|package| package.direct)
        .map(|package| (package.alias.to_string(), package))
        .collect::<BTreeMap<_, _>>();
    rewrite_namespaced_calls_in_program(&mut program, &dependency_by_alias)?;
    for package in &lockfile.packages {
        if !package.resolved {
            bail!(
                "dependency `{}` is unresolved in Musubi.lock; run `musubi install`",
                package.alias
            );
        }
        let source_root = cache_source_path(cache_dir, package);
        if !source_root.exists() {
            bail!(
                "dependency `{}` source cache is missing at `{}`; run `musubi cache import` after fetching it",
                package.alias,
                source_root.display()
            );
        }
        verify_cached_package(cache_dir, package)?;
        let package_dependency_by_alias = package_dependency_map(lockfile, package)?;
        let mut files = Vec::new();
        collect_source_files(&source_root, &source_root, &mut files)?;
        files.sort();
        let package_functions = collect_package_function_names(&source_root)?;
        let package_prefix = package_prefix_key(&package.package);
        for relative in files.into_iter().filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension == "ko")
        }) {
            let path = source_root.join(&relative);
            let source = fs::read_to_string(&path)
                .wrap_err_with(|| format!("failed to read `{}`", path.display()))?;
            let mut dependency_program = parse_kotodama(&source)
                .map_err(|err| eyre!("failed to parse `{}`: {err}", path.display()))?;
            validate_dependency_program_is_function_only(&dependency_program, package, &path)?;
            rewrite_namespaced_calls_in_program(
                &mut dependency_program,
                &package_dependency_by_alias,
            )?;
            prefix_dependency_program(&mut dependency_program, &package_prefix, &package_functions);
            program.items.extend(dependency_program.items);
        }
    }
    Ok(program)
}

fn package_dependency_map<'a>(
    lockfile: &'a MusubiLockfile,
    package: &LockedPackage,
) -> Result<BTreeMap<String, &'a LockedPackage>> {
    let mut dependencies = BTreeMap::new();
    for dependency in &package.dependencies {
        let locked = lockfile
            .package_by_ref(&dependency.package)
            .ok_or_else(|| {
                eyre!(
                    "dependency `{}` of `{}` is missing from Musubi.lock",
                    dependency.package,
                    package.package
                )
            })?;
        dependencies.insert(dependency.alias.to_string(), locked);
    }
    Ok(dependencies)
}

fn validate_dependency_program_is_function_only(
    program: &Program,
    package: &LockedPackage,
    path: &Path,
) -> Result<()> {
    for item in &program.items {
        if !matches!(item, Item::Function(_)) {
            bail!(
                "dependency `{}` contains unsupported non-function item in `{}`; Musubi v1 libraries are function-only",
                package.package,
                path.display()
            );
        }
    }
    Ok(())
}

fn collect_package_function_names(root: &Path) -> Result<BTreeSet<String>> {
    Ok(collect_kotodama_functions(root)?
        .into_iter()
        .map(|name| name.to_string())
        .collect())
}

fn rewrite_namespaced_calls_in_program(
    program: &mut Program,
    dependency_by_alias: &BTreeMap<String, &LockedPackage>,
) -> Result<()> {
    for item in &mut program.items {
        match item {
            Item::Function(function) => {
                rewrite_namespaced_calls_in_block(&mut function.body, dependency_by_alias)?;
            }
            Item::Const(decl) => {
                rewrite_namespaced_calls_in_expr(&mut decl.value, dependency_by_alias)?;
            }
            Item::Trigger(decl) => {
                for metadata in &mut decl.metadata {
                    rewrite_namespaced_calls_in_expr(&mut metadata.value, dependency_by_alias)?;
                }
            }
            Item::Struct(_) | Item::State(_) | Item::Kotoba(_) => {}
        }
    }
    Ok(())
}

fn rewrite_namespaced_calls_in_block(
    block: &mut Block,
    dependency_by_alias: &BTreeMap<String, &LockedPackage>,
) -> Result<()> {
    for statement in &mut block.statements {
        rewrite_namespaced_calls_in_statement(statement, dependency_by_alias)?;
    }
    Ok(())
}

fn rewrite_namespaced_calls_in_statement(
    statement: &mut Statement,
    dependency_by_alias: &BTreeMap<String, &LockedPackage>,
) -> Result<()> {
    match statement {
        Statement::Let { value, .. } | Statement::Assign { value, .. } | Statement::Expr(value) => {
            rewrite_namespaced_calls_in_expr(value, dependency_by_alias)?
        }
        Statement::AssignExpr { target, value, .. } => {
            rewrite_namespaced_calls_in_expr(target, dependency_by_alias)?;
            rewrite_namespaced_calls_in_expr(value, dependency_by_alias)?;
        }
        Statement::Return(Some(value)) => {
            rewrite_namespaced_calls_in_expr(value, dependency_by_alias)?
        }
        Statement::If {
            cond,
            then_branch,
            else_branch,
        } => {
            rewrite_namespaced_calls_in_expr(cond, dependency_by_alias)?;
            rewrite_namespaced_calls_in_block(then_branch, dependency_by_alias)?;
            if let Some(block) = else_branch {
                rewrite_namespaced_calls_in_block(block, dependency_by_alias)?;
            }
        }
        Statement::While { cond, body } => {
            rewrite_namespaced_calls_in_expr(cond, dependency_by_alias)?;
            rewrite_namespaced_calls_in_block(body, dependency_by_alias)?;
        }
        Statement::For {
            init,
            cond,
            step,
            body,
            ..
        } => {
            if let Some(init) = init {
                rewrite_namespaced_calls_in_statement(init, dependency_by_alias)?;
            }
            if let Some(cond) = cond {
                rewrite_namespaced_calls_in_expr(cond, dependency_by_alias)?;
            }
            if let Some(step) = step {
                rewrite_namespaced_calls_in_statement(step, dependency_by_alias)?;
            }
            rewrite_namespaced_calls_in_block(body, dependency_by_alias)?;
        }
        Statement::ForEachMap { map, body, .. } => {
            rewrite_namespaced_calls_in_expr(map, dependency_by_alias)?;
            rewrite_namespaced_calls_in_block(body, dependency_by_alias)?;
        }
        Statement::Return(None) | Statement::Break | Statement::Continue => {}
    }
    Ok(())
}

fn rewrite_namespaced_calls_in_expr(
    expr: &mut Expr,
    dependency_by_alias: &BTreeMap<String, &LockedPackage>,
) -> Result<()> {
    match expr {
        Expr::Call { name, args } => {
            if let Some((alias, function)) = name.split_once("::") {
                let package = dependency_by_alias.get(alias).ok_or_else(|| {
                    eyre!("Kotodama dependency alias `{alias}` is not present in Musubi.lock")
                })?;
                let export = function.parse::<Name>().map_err(|err| {
                    eyre!(
                        "Kotodama function `{function}` is not a valid Musubi export: {}",
                        err.reason()
                    )
                })?;
                if !package.exports.contains(&export) {
                    bail!(
                        "function `{alias}::{function}` is not exported by `{}`",
                        package.package
                    );
                }
                *name = prefixed_function_name(&package_prefix_key(&package.package), function);
            }
            for arg in args {
                rewrite_namespaced_calls_in_expr(arg, dependency_by_alias)?;
            }
        }
        Expr::Binary { left, right, .. } => {
            rewrite_namespaced_calls_in_expr(left, dependency_by_alias)?;
            rewrite_namespaced_calls_in_expr(right, dependency_by_alias)?;
        }
        Expr::Unary { expr, .. } => rewrite_namespaced_calls_in_expr(expr, dependency_by_alias)?,
        Expr::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            rewrite_namespaced_calls_in_expr(cond, dependency_by_alias)?;
            rewrite_namespaced_calls_in_expr(then_expr, dependency_by_alias)?;
            rewrite_namespaced_calls_in_expr(else_expr, dependency_by_alias)?;
        }
        Expr::Member { object, .. } => {
            rewrite_namespaced_calls_in_expr(object, dependency_by_alias)?
        }
        Expr::Index { target, index } => {
            rewrite_namespaced_calls_in_expr(target, dependency_by_alias)?;
            rewrite_namespaced_calls_in_expr(index, dependency_by_alias)?;
        }
        Expr::Tuple(items) => {
            for item in items {
                rewrite_namespaced_calls_in_expr(item, dependency_by_alias)?;
            }
        }
        Expr::Bool(_)
        | Expr::Number(_)
        | Expr::Decimal(_)
        | Expr::String(_)
        | Expr::Bytes(_)
        | Expr::Ident(_) => {}
    }
    Ok(())
}

fn prefix_dependency_program(program: &mut Program, prefix: &str, functions: &BTreeSet<String>) {
    for item in &mut program.items {
        if let Item::Function(function) = item {
            prefix_dependency_function(function, prefix, functions);
        }
    }
}

fn prefix_dependency_function(function: &mut Function, prefix: &str, functions: &BTreeSet<String>) {
    let original_name = function.name.clone();
    function.name = prefixed_function_name(prefix, &original_name);
    prefix_internal_calls_in_block(&mut function.body, prefix, functions);
}

fn prefix_internal_calls_in_block(block: &mut Block, prefix: &str, functions: &BTreeSet<String>) {
    for statement in &mut block.statements {
        prefix_internal_calls_in_statement(statement, prefix, functions);
    }
}

fn prefix_internal_calls_in_statement(
    statement: &mut Statement,
    prefix: &str,
    functions: &BTreeSet<String>,
) {
    match statement {
        Statement::Let { value, .. } | Statement::Assign { value, .. } | Statement::Expr(value) => {
            prefix_internal_calls_in_expr(value, prefix, functions)
        }
        Statement::AssignExpr { target, value, .. } => {
            prefix_internal_calls_in_expr(target, prefix, functions);
            prefix_internal_calls_in_expr(value, prefix, functions);
        }
        Statement::Return(Some(value)) => prefix_internal_calls_in_expr(value, prefix, functions),
        Statement::If {
            cond,
            then_branch,
            else_branch,
        } => {
            prefix_internal_calls_in_expr(cond, prefix, functions);
            prefix_internal_calls_in_block(then_branch, prefix, functions);
            if let Some(block) = else_branch {
                prefix_internal_calls_in_block(block, prefix, functions);
            }
        }
        Statement::While { cond, body } => {
            prefix_internal_calls_in_expr(cond, prefix, functions);
            prefix_internal_calls_in_block(body, prefix, functions);
        }
        Statement::For {
            init,
            cond,
            step,
            body,
            ..
        } => {
            if let Some(init) = init {
                prefix_internal_calls_in_statement(init, prefix, functions);
            }
            if let Some(cond) = cond {
                prefix_internal_calls_in_expr(cond, prefix, functions);
            }
            if let Some(step) = step {
                prefix_internal_calls_in_statement(step, prefix, functions);
            }
            prefix_internal_calls_in_block(body, prefix, functions);
        }
        Statement::ForEachMap { map, body, .. } => {
            prefix_internal_calls_in_expr(map, prefix, functions);
            prefix_internal_calls_in_block(body, prefix, functions);
        }
        Statement::Return(None) | Statement::Break | Statement::Continue => {}
    }
}

fn prefix_internal_calls_in_expr(expr: &mut Expr, prefix: &str, functions: &BTreeSet<String>) {
    match expr {
        Expr::Call { name, args } => {
            if functions.contains(name) {
                *name = prefixed_function_name(prefix, name);
            }
            for arg in args {
                prefix_internal_calls_in_expr(arg, prefix, functions);
            }
        }
        Expr::Binary { left, right, .. } => {
            prefix_internal_calls_in_expr(left, prefix, functions);
            prefix_internal_calls_in_expr(right, prefix, functions);
        }
        Expr::Unary { expr, .. } => prefix_internal_calls_in_expr(expr, prefix, functions),
        Expr::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            prefix_internal_calls_in_expr(cond, prefix, functions);
            prefix_internal_calls_in_expr(then_expr, prefix, functions);
            prefix_internal_calls_in_expr(else_expr, prefix, functions);
        }
        Expr::Member { object, .. } => prefix_internal_calls_in_expr(object, prefix, functions),
        Expr::Index { target, index } => {
            prefix_internal_calls_in_expr(target, prefix, functions);
            prefix_internal_calls_in_expr(index, prefix, functions);
        }
        Expr::Tuple(items) => {
            for item in items {
                prefix_internal_calls_in_expr(item, prefix, functions);
            }
        }
        Expr::Bool(_)
        | Expr::Number(_)
        | Expr::Decimal(_)
        | Expr::String(_)
        | Expr::Bytes(_)
        | Expr::Ident(_) => {}
    }
}

fn package_prefix_key(package: &MusubiPackageRef) -> String {
    let digest = blake3::hash(package.canonical_ref().as_bytes());
    format!("p{}", hex::encode(&digest.as_bytes()[..8]))
}

fn prefixed_function_name(prefix: &str, function: &str) -> String {
    format!("__musubi_{prefix}_{function}")
}

fn release_status_label(status: &MusubiReleaseStatus) -> &'static str {
    match status {
        MusubiReleaseStatus::Active => "active",
        MusubiReleaseStatus::Yanked(_) => "yanked",
    }
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
    use std::{
        collections::HashMap,
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        thread,
        time::Duration,
    };

    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use iroha_data_model::{Decode, Encode};
    use sorafs_car::{gateway::GatewayFetchContext, multi_fetch::FetchOptions};
    use sorafs_manifest::{StreamTokenBodyV1, StreamTokenV1};

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
            "std.universal/math"
        );
        assert_eq!(manifest.dependencies[0].version_req.to_string(), "1.0.0");
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
            package: "std.universal/math".parse().expect("package"),
            version_req: "^1.0.0".parse().expect("version req"),
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

        assert!(rendered.contains("version = 3"));
        assert!(rendered.contains("name = \"std.universal/math\""));
        assert!(rendered.contains("requirement = \"1.0.0\""));
        assert!(rendered.contains("resolved = false"));
    }

    #[test]
    fn add_dependency_accepts_package_id_with_requirement() {
        let dependency = parse_add_dependency_package(
            "std.universal/math",
            Some("^1.2.0".parse().unwrap()),
            None,
        )
        .expect("parse package id");

        assert_eq!(dependency.package.to_string(), "std.universal/math");
        assert_eq!(dependency.version_req.to_string(), "^1.2.0");
    }

    #[test]
    fn add_dependency_requires_client_for_short_aliases() {
        let error = parse_add_dependency_package("math", Some("1.0.0".parse().unwrap()), None)
            .expect_err("short alias without client config must fail");

        assert!(error.to_string().contains("short alias"));
    }

    #[test]
    fn linker_rewrites_namespaced_calls_to_locked_exports() {
        let mut program = Program {
            items: vec![Item::Function(Function {
                name: "main".to_owned(),
                params: Vec::new(),
                ret_ty: None,
                body: Block {
                    statements: vec![Statement::Expr(Expr::Call {
                        name: "math::add".to_owned(),
                        args: Vec::new(),
                    })],
                },
                modifiers: ivm::kotodama::ast::FunctionModifiers::default(),
                location: ivm::kotodama::ast::SourceLocation { line: 1, column: 1 },
            })],
            contract_meta: None,
            test_target: None,
            fixtures: Vec::new(),
        };
        let package = LockedPackage {
            alias: "math".parse().unwrap(),
            package: "std.universal/math@1.2.3".parse().unwrap(),
            version_req: "^1.0.0".parse().unwrap(),
            archive: None,
            source_plan: None,
            cache_path: None,
            exports: vec!["add".parse().unwrap()],
            dependencies: Vec::new(),
            direct: true,
            resolved: true,
        };
        let dependencies = BTreeMap::from([("math".to_owned(), &package)]);

        rewrite_namespaced_calls_in_program(&mut program, &dependencies).expect("rewrite");

        let Item::Function(function) = &program.items[0] else {
            panic!("expected function");
        };
        let Statement::Expr(Expr::Call { name, .. }) = &function.body.statements[0] else {
            panic!("expected call");
        };
        assert_eq!(
            name,
            &prefixed_function_name(&package_prefix_key(&package.package), "add")
        );
    }

    #[test]
    fn cache_imported_source_verifies_against_lockfile_archive() {
        let source = tempfile::tempdir().expect("source tempdir");
        fs::write(
            source.path().join("Musubi.toml"),
            "[package]\nnamespace = \"std.universal\"\nname = \"math\"\nversion = \"1.2.3\"\n",
        )
        .expect("manifest");
        fs::create_dir(source.path().join("src")).expect("src dir");
        fs::write(source.path().join("src/lib.ko"), "fn add() {}\n").expect("source");
        let stats = hash_source_tree(source.path()).expect("source hash");
        let package = LockedPackage {
            alias: "math".parse().unwrap(),
            package: "std.universal/math@1.2.3".parse().unwrap(),
            version_req: "^1.0.0".parse().unwrap(),
            archive: Some(MusubiArchiveRef::new(
                ManifestDigest::new([7; 32]),
                stats.archive_hash_blake3_256,
                stats.source_bytes,
                stats.source_file_count,
            )),
            source_plan: None,
            cache_path: None,
            exports: vec!["add".parse().unwrap()],
            dependencies: Vec::new(),
            direct: true,
            resolved: true,
        };
        let cache = tempfile::tempdir().expect("cache tempdir");
        let destination = cache_source_path(cache.path(), &package);

        copy_source_tree(source.path(), &destination).expect("copy source tree");
        verify_cached_package(cache.path(), &package).expect("verify cache");
    }

    #[test]
    fn sorafs_plan_uses_same_source_file_set_as_archive_hash() {
        let source = tempfile::tempdir().expect("source tempdir");
        fs::write(
            source.path().join("Musubi.toml"),
            "[package]\nnamespace = \"std.universal\"\nname = \"math\"\nversion = \"1.2.3\"\n",
        )
        .expect("manifest");
        fs::create_dir(source.path().join("src")).expect("src dir");
        fs::write(source.path().join("src/lib.ko"), "fn add() {}\n").expect("source");
        fs::write(source.path().join(DEFAULT_LOCKFILE), "ignored").expect("lockfile");
        fs::create_dir_all(source.path().join(".musubi/cache")).expect("musubi cache");
        fs::write(source.path().join(".musubi/cache/generated.ko"), "ignored").expect("generated");
        fs::create_dir(source.path().join("target")).expect("target dir");
        fs::write(source.path().join("target/build.ko"), "ignored").expect("target");

        let manifest = read_manifest(&source.path().join("Musubi.toml")).expect("manifest");
        let archive = hash_source_tree(source.path()).expect("archive hash");
        let sorafs =
            build_sorafs_source_manifest(&manifest, source.path(), None, None, None, archive)
                .expect("sorafs manifest");

        assert_eq!(
            sorafs.source_plan.files.len(),
            archive.source_file_count as usize
        );
        assert_eq!(sorafs.source_plan.content_length, archive.source_bytes);
        assert_eq!(archive.source_file_count, 2);
    }

    #[test]
    fn cache_fetch_reconstructs_source_from_verified_payload() {
        let source = tempfile::tempdir().expect("source tempdir");
        fs::write(
            source.path().join("Musubi.toml"),
            "[package]\nnamespace = \"std.universal\"\nname = \"math\"\nversion = \"1.2.3\"\n",
        )
        .expect("manifest");
        fs::create_dir(source.path().join("src")).expect("src dir");
        fs::write(source.path().join("src/lib.ko"), "fn add() {}\n").expect("source");

        let manifest = read_manifest(&source.path().join("Musubi.toml")).expect("manifest");
        let archive_stats = hash_source_tree(source.path()).expect("archive hash");
        let sorafs =
            build_sorafs_source_manifest(&manifest, source.path(), None, None, None, archive_stats)
                .expect("sorafs manifest");
        let package = LockedPackage {
            alias: "math".parse().unwrap(),
            package: "std.universal/math@1.2.3".parse().unwrap(),
            version_req: "^1.0.0".parse().unwrap(),
            archive: Some(MusubiArchiveRef::new(
                sorafs.digest,
                archive_stats.archive_hash_blake3_256,
                archive_stats.source_bytes,
                archive_stats.source_file_count,
            )),
            source_plan: Some(sorafs.source_plan),
            cache_path: None,
            exports: vec!["add".parse().unwrap()],
            dependencies: Vec::new(),
            direct: true,
            resolved: true,
        };
        let cache = tempfile::tempdir().expect("cache tempdir");
        let provider_payload = source.path().join("provider.payload");
        fs::write(&provider_payload, sorafs.payload).expect("provider payload");

        fetch_locked_package_source(&package, cache.path(), &[provider_payload], false)
            .expect("fetch source");

        let fetched =
            fs::read_to_string(cache_source_path(cache.path(), &package).join("src/lib.ko"))
                .expect("read fetched");
        assert_eq!(fetched, "fn add() {}\n");
        verify_cached_package(cache.path(), &package).expect("verify cache");
    }

    #[test]
    fn gateway_provider_spec_parses_scoped_provider() {
        let provider_id = "11".repeat(32);
        let manifest = "22".repeat(32);
        let spec: GatewayProviderSpec = format!(
            "name=gw,provider-id={provider_id},base-url=https://gw.example,stream-token=token,privacy-url=https://privacy.example,package=math,manifest={manifest}"
        )
        .parse()
        .expect("parse provider");

        assert_eq!(spec.name, "gw");
        assert_eq!(spec.provider_id_hex, provider_id);
        assert_eq!(spec.base_url, "https://gw.example");
        assert_eq!(spec.stream_token_b64, "token");
        assert_eq!(
            spec.privacy_events_url.as_deref(),
            Some("https://privacy.example")
        );
        assert_eq!(spec.package.as_deref(), Some("math"));
        assert_eq!(spec.manifest_id_hex.as_deref(), Some(manifest.as_str()));

        let err = "name=gw,provider-id=abcd,base-url=https://gw.example,stream-token=token"
            .parse::<GatewayProviderSpec>()
            .expect_err("provider id must be full digest");
        assert!(err.contains("32-byte hex"));
    }

    #[test]
    fn gateway_provider_rejects_public_http_url() {
        let gateway = GatewayFetchArgs {
            gateway_provider: vec![GatewayProviderSpec {
                name: "gw".into(),
                provider_id_hex: "11".repeat(32),
                base_url: "http://gw.example".into(),
                stream_token_b64: "token".into(),
                privacy_events_url: None,
                package: None,
                manifest_id_hex: None,
            }],
            ..GatewayFetchArgs::default()
        };

        let err = validate_source_fetch_inputs(&[], &gateway).expect_err("http rejected");

        assert!(err.to_string().contains("must use https"));
    }

    #[test]
    fn gateway_provider_accepts_https_url() {
        let gateway = GatewayFetchArgs {
            gateway_provider: vec![GatewayProviderSpec {
                name: "gw".into(),
                provider_id_hex: "11".repeat(32),
                base_url: "https://gw.example".into(),
                stream_token_b64: "token".into(),
                privacy_events_url: Some("https://privacy.example/events".into()),
                package: None,
                manifest_id_hex: None,
            }],
            ..GatewayFetchArgs::default()
        };

        validate_source_fetch_inputs(&[], &gateway).expect("https accepted");
    }

    #[test]
    fn gateway_provider_accepts_local_http_only_with_explicit_flag() {
        let gateway = GatewayFetchArgs {
            gateway_provider: vec![GatewayProviderSpec {
                name: "gw".into(),
                provider_id_hex: "11".repeat(32),
                base_url: "http://127.0.0.1:8080".into(),
                stream_token_b64: "token".into(),
                privacy_events_url: Some("http://[::1]:8081/privacy/events".into()),
                package: None,
                manifest_id_hex: None,
            }],
            ..GatewayFetchArgs::default()
        };

        let err =
            validate_source_fetch_inputs(&[], &gateway).expect_err("localhost http needs flag");
        assert!(err.to_string().contains("gateway-allow-insecure-localhost"));

        let gateway = GatewayFetchArgs {
            gateway_allow_insecure_localhost: true,
            ..gateway
        };
        validate_source_fetch_inputs(&[], &gateway).expect("localhost http accepted with flag");
    }

    #[test]
    fn source_archive_plan_reconstructs_gateway_car_plan() {
        let source = tempfile::tempdir().expect("source tempdir");
        fs::write(
            source.path().join("Musubi.toml"),
            "[package]\nnamespace = \"std.universal\"\nname = \"math\"\nversion = \"1.2.3\"\n",
        )
        .expect("manifest");
        fs::create_dir(source.path().join("src")).expect("src dir");
        fs::write(source.path().join("src/lib.ko"), "fn add() {}\n").expect("source");

        let manifest = read_manifest(&source.path().join("Musubi.toml")).expect("manifest");
        let archive = hash_source_tree(source.path()).expect("archive hash");
        let sorafs =
            build_sorafs_source_manifest(&manifest, source.path(), None, None, None, archive)
                .expect("sorafs manifest");

        let plan = car_plan_from_source_archive_plan(&sorafs.source_plan).expect("car plan");

        assert_eq!(
            plan.payload_digest.as_bytes(),
            &sorafs.source_plan.payload_hash_blake3_256
        );
        assert_eq!(plan.content_length, sorafs.source_plan.content_length);
        assert_eq!(plan.chunks.len(), sorafs.source_plan.chunks.len());
        assert_eq!(plan.files.len(), sorafs.source_plan.files.len());
        assert_eq!(plan.chunks[0].offset, sorafs.source_plan.chunks[0].offset);
        assert_eq!(
            plan.chunks[0].digest,
            sorafs.source_plan.chunks[0].digest_blake3_256
        );
        assert_eq!(plan.files[0].path, sorafs.source_plan.files[0].path);
    }

    #[test]
    fn cache_fetch_reconstructs_source_from_gateway_payload() {
        struct StaticGatewayRunner {
            manifest_id_hex: String,
            payload: Vec<u8>,
            scoreboard_path: PathBuf,
        }

        impl GatewayFetchRunner for StaticGatewayRunner {
            fn fetch(&self, request: GatewayFetchRequest) -> Result<Vec<u8>> {
                assert_eq!(request.gateway_config.manifest_id_hex, self.manifest_id_hex);
                assert_eq!(
                    request.gateway_config.expected_manifest_cid_hex.as_deref(),
                    Some(self.manifest_id_hex.as_str())
                );
                assert_eq!(request.gateway_config.chunker_handle, "sorafs.sf1@1.0.0");
                assert_eq!(request.providers.len(), 1);
                assert_eq!(request.providers[0].name, "gw");
                assert_eq!(request.options.retry_budget, Some(2));
                assert_eq!(request.options.max_peers, Some(1));
                assert_eq!(
                    request
                        .options
                        .scoreboard
                        .as_ref()
                        .and_then(|scoreboard| scoreboard.persist_path.as_ref()),
                    Some(&self.scoreboard_path)
                );
                assert_eq!(request.plan.content_length, self.payload.len() as u64);
                Ok(self.payload.clone())
            }
        }

        let source = tempfile::tempdir().expect("source tempdir");
        fs::write(
            source.path().join("Musubi.toml"),
            "[package]\nnamespace = \"std.universal\"\nname = \"math\"\nversion = \"1.2.3\"\n",
        )
        .expect("manifest");
        fs::create_dir(source.path().join("src")).expect("src dir");
        fs::write(source.path().join("src/lib.ko"), "fn add() {}\n").expect("source");

        let manifest = read_manifest(&source.path().join("Musubi.toml")).expect("manifest");
        let archive_stats = hash_source_tree(source.path()).expect("archive hash");
        let sorafs =
            build_sorafs_source_manifest(&manifest, source.path(), None, None, None, archive_stats)
                .expect("sorafs manifest");
        let manifest_id_hex = hex::encode(sorafs.digest.as_bytes());
        let package = LockedPackage {
            alias: "math".parse().unwrap(),
            package: "std.universal/math@1.2.3".parse().unwrap(),
            version_req: "^1.0.0".parse().unwrap(),
            archive: Some(MusubiArchiveRef::new(
                sorafs.digest,
                archive_stats.archive_hash_blake3_256,
                archive_stats.source_bytes,
                archive_stats.source_file_count,
            )),
            source_plan: Some(sorafs.source_plan),
            cache_path: None,
            exports: vec!["add".parse().unwrap()],
            dependencies: Vec::new(),
            direct: true,
            resolved: true,
        };
        let scoreboard_path = source.path().join("scoreboard.json");
        let gateway = GatewayFetchArgs {
            gateway_provider: vec![GatewayProviderSpec {
                name: "gw".into(),
                provider_id_hex: "11".repeat(32),
                base_url: "https://gw.example".into(),
                stream_token_b64: "token".into(),
                privacy_events_url: None,
                package: Some("math".into()),
                manifest_id_hex: None,
            }],
            gateway_client_id: Some("musubi-tests".into()),
            gateway_retry_budget: Some(2),
            gateway_max_peers: Some(1),
            gateway_telemetry_region: Some("test-region".into()),
            gateway_scoreboard_out: Some(scoreboard_path.clone()),
            gateway_allow_insecure_localhost: false,
        };
        let runner = StaticGatewayRunner {
            manifest_id_hex,
            payload: sorafs.payload,
            scoreboard_path,
        };
        let cache = tempfile::tempdir().expect("cache tempdir");

        fetch_locked_package_source_from(
            &package,
            cache.path(),
            SourceFetchMode::Gateway {
                runner: &runner,
                args: &gateway,
                allow_unscoped_providers: false,
            },
            false,
        )
        .expect("gateway fetch source");

        let fetched =
            fs::read_to_string(cache_source_path(cache.path(), &package).join("src/lib.ko"))
                .expect("read fetched");
        assert_eq!(fetched, "fn add() {}\n");
        verify_cached_package(cache.path(), &package).expect("verify cache");
    }

    #[test]
    fn cache_fetch_reconstructs_source_from_live_gateway() {
        struct RealGatewayRunner;

        impl GatewayFetchRunner for RealGatewayRunner {
            fn fetch(&self, request: GatewayFetchRequest) -> Result<Vec<u8>> {
                let context =
                    GatewayFetchContext::new(request.gateway_config, request.providers)
                        .map_err(|err| eyre!("failed to build gateway fetch context: {err}"))?;
                let mut options = FetchOptions::default();
                options.per_chunk_retry_limit = request.options.retry_budget;
                options.global_parallel_limit = request.options.max_peers;
                let runtime = tokio::runtime::Runtime::new()
                    .wrap_err("failed to initialise Tokio runtime")?;
                let outcome = runtime
                    .block_on(context.execute_plan(&request.plan, options))
                    .map_err(|err| eyre!("gateway fetch failed: {err}"))?;
                Ok(outcome.assemble_payload())
            }
        }

        let source = tempfile::tempdir().expect("source tempdir");
        fs::write(
            source.path().join("Musubi.toml"),
            "[package]\nnamespace = \"std.universal\"\nname = \"math\"\nversion = \"1.2.3\"\n",
        )
        .expect("manifest");
        fs::create_dir(source.path().join("src")).expect("src dir");
        fs::write(source.path().join("src/lib.ko"), "fn add() {}\n").expect("source");

        let manifest = read_manifest(&source.path().join("Musubi.toml")).expect("manifest");
        let archive_stats = hash_source_tree(source.path()).expect("archive hash");
        let sorafs =
            build_sorafs_source_manifest(&manifest, source.path(), None, None, None, archive_stats)
                .expect("sorafs manifest");
        let manifest_id_hex = hex::encode(sorafs.digest.as_bytes());
        let chunks = sorafs
            .source_plan
            .chunks
            .iter()
            .map(|chunk| {
                let chunk_start = usize::try_from(chunk.offset).expect("chunk offset");
                let chunk_end = chunk_start + usize::try_from(chunk.length).expect("chunk length");
                (
                    format!(
                        "/v1/sorafs/storage/chunk/{}/{}",
                        manifest_id_hex,
                        hex::encode(chunk.digest_blake3_256)
                    ),
                    sorafs.payload[chunk_start..chunk_end].to_vec(),
                )
            })
            .collect::<HashMap<_, _>>();
        let base_url = spawn_chunk_gateway(chunks);
        let provider_id_hex = "11".repeat(32);
        let chunker_handle = chunker_registry::default_descriptor().aliases[0].to_owned();
        let token = stream_token_b64(&manifest_id_hex, &provider_id_hex, &chunker_handle, 4);
        let package = LockedPackage {
            alias: "math".parse().unwrap(),
            package: "std.universal/math@1.2.3".parse().unwrap(),
            version_req: "^1.0.0".parse().unwrap(),
            archive: Some(MusubiArchiveRef::new(
                sorafs.digest,
                archive_stats.archive_hash_blake3_256,
                archive_stats.source_bytes,
                archive_stats.source_file_count,
            )),
            source_plan: Some(sorafs.source_plan),
            cache_path: None,
            exports: vec!["add".parse().unwrap()],
            dependencies: Vec::new(),
            direct: true,
            resolved: true,
        };
        let gateway = GatewayFetchArgs {
            gateway_provider: vec![GatewayProviderSpec {
                name: "gw".into(),
                provider_id_hex,
                base_url,
                stream_token_b64: token,
                privacy_events_url: None,
                package: Some("math".into()),
                manifest_id_hex: None,
            }],
            gateway_retry_budget: Some(1),
            gateway_max_peers: Some(1),
            gateway_allow_insecure_localhost: true,
            ..GatewayFetchArgs::default()
        };
        validate_source_fetch_inputs(&[], &gateway).expect("localhost gateway accepted");

        let cache = tempfile::tempdir().expect("cache tempdir");
        fetch_locked_package_source_from(
            &package,
            cache.path(),
            SourceFetchMode::Gateway {
                runner: &RealGatewayRunner,
                args: &gateway,
                allow_unscoped_providers: false,
            },
            false,
        )
        .expect("live gateway fetch source");

        let fetched =
            fs::read_to_string(cache_source_path(cache.path(), &package).join("src/lib.ko"))
                .expect("read fetched");
        assert_eq!(fetched, "fn add() {}\n");
        verify_cached_package(cache.path(), &package).expect("verify cache");
    }

    fn spawn_chunk_gateway(chunks: HashMap<String, Vec<u8>>) -> String {
        let request_count = chunks.len();
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind gateway");
        let addr = listener.local_addr().expect("gateway addr");
        thread::spawn(move || {
            for _ in 0..request_count {
                if let Ok((mut stream, _)) = listener.accept() {
                    serve_chunk_gateway(&mut stream, &chunks);
                }
            }
        });
        format!("http://{addr}")
    }

    fn serve_chunk_gateway(stream: &mut TcpStream, chunks: &HashMap<String, Vec<u8>>) {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let mut request = Vec::new();
        let mut buffer = [0_u8; 1024];
        loop {
            let bytes_read = stream.read(&mut buffer).expect("read request");
            if bytes_read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..bytes_read]);
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        let request = String::from_utf8_lossy(&request);
        let path = request
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or_default();
        if let Some(body) = chunks.get(path) {
            write_http_response(stream, 200, "OK", body);
        } else {
            write_http_response(stream, 404, "Not Found", b"missing chunk");
        }
    }

    fn write_http_response(stream: &mut TcpStream, status: u16, reason: &str, body: &[u8]) {
        let header = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(header.as_bytes()).expect("write header");
        stream.write_all(body).expect("write body");
    }

    fn stream_token_b64(
        manifest_cid_hex: &str,
        provider_id_hex: &str,
        profile_handle: &str,
        max_streams: u16,
    ) -> String {
        let mut provider_id = [0_u8; 32];
        provider_id.copy_from_slice(&hex::decode(provider_id_hex).expect("provider id hex"));
        let token = StreamTokenV1 {
            body: StreamTokenBodyV1 {
                token_id: "01J9TK3GR0XM6YQF7WQXA9Z2SF".to_owned(),
                manifest_cid: hex::decode(manifest_cid_hex).expect("manifest cid hex"),
                provider_id,
                profile_handle: profile_handle.to_owned(),
                max_streams,
                ttl_epoch: 9_999_999_999,
                rate_limit_bytes: 8 * 1024 * 1024,
                issued_at: 1_735_000_000,
                requests_per_minute: 120,
                token_pk_version: 1,
            },
            signature: vec![0; 64],
        };
        STANDARD.encode(norito::to_bytes(&token).expect("encode stream token"))
    }

    #[test]
    fn provider_payload_and_gateway_provider_cannot_be_mixed() {
        let gateway = GatewayFetchArgs {
            gateway_provider: vec![GatewayProviderSpec {
                name: "gw".into(),
                provider_id_hex: "11".repeat(32),
                base_url: "https://gw.example".into(),
                stream_token_b64: "token".into(),
                privacy_events_url: None,
                package: None,
                manifest_id_hex: None,
            }],
            ..GatewayFetchArgs::default()
        };
        let payloads = vec![PathBuf::from("provider.payload")];

        let err = validate_source_fetch_inputs(&payloads, &gateway).expect_err("mix rejected");

        assert!(err.to_string().contains("cannot be combined"));
    }

    #[test]
    fn install_fetch_rejects_unscoped_gateway_for_multiple_missing_packages() {
        let cache = tempfile::tempdir().expect("cache tempdir");
        let package_a = LockedPackage {
            alias: "math".parse().unwrap(),
            package: "std.universal/math@1.2.3".parse().unwrap(),
            version_req: "^1.0.0".parse().unwrap(),
            archive: None,
            source_plan: None,
            cache_path: None,
            exports: vec!["add".parse().unwrap()],
            dependencies: Vec::new(),
            direct: true,
            resolved: true,
        };
        let package_b = LockedPackage {
            alias: "util".parse().unwrap(),
            package: "std.universal/util@1.2.3".parse().unwrap(),
            version_req: "^1.0.0".parse().unwrap(),
            archive: None,
            source_plan: None,
            cache_path: None,
            exports: vec!["id".parse().unwrap()],
            dependencies: Vec::new(),
            direct: true,
            resolved: true,
        };
        let lockfile = MusubiLockfile::new(vec![package_a, package_b]);
        let gateway = GatewayFetchArgs {
            gateway_provider: vec![GatewayProviderSpec {
                name: "gw".into(),
                provider_id_hex: "11".repeat(32),
                base_url: "https://gw.example".into(),
                stream_token_b64: "token".into(),
                privacy_events_url: None,
                package: None,
                manifest_id_hex: None,
            }],
            ..GatewayFetchArgs::default()
        };

        let err = validate_gateway_scope_for_lockfile(&lockfile, cache.path(), &gateway)
            .expect_err("ambiguous provider rejected");

        assert!(err.to_string().contains("ambiguous"));
    }

    #[test]
    fn dependency_programs_reject_non_function_items() {
        let program = parse_kotodama("const answer = 1;").expect("parse const");
        let package = LockedPackage {
            alias: "math".parse().unwrap(),
            package: "std.universal/math@1.2.3".parse().unwrap(),
            version_req: "^1.0.0".parse().unwrap(),
            archive: None,
            source_plan: None,
            cache_path: None,
            exports: vec!["add".parse().unwrap()],
            dependencies: Vec::new(),
            direct: true,
            resolved: true,
        };

        let err =
            validate_dependency_program_is_function_only(&program, &package, Path::new("lib.ko"))
                .expect_err("const rejected");

        assert!(err.to_string().contains("function-only"));
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
        assert!(first.source_bytes > 0);
        assert_eq!(first.source_file_count, 2);
    }

    #[test]
    fn export_validation_requires_defined_kotodama_function() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::create_dir(temp.path().join("src")).expect("src dir");
        fs::write(temp.path().join("src/lib.ko"), "fn quote() {}\n").expect("source");

        validate_exported_functions_exist(temp.path(), &["quote".parse().expect("export")])
            .expect("export exists");
        let err =
            validate_exported_functions_exist(temp.path(), &["swap".parse().expect("export")])
                .expect_err("missing export");

        assert!(err.to_string().contains("not defined"));
    }

    #[test]
    fn hex_parser_requires_32_bytes() {
        let err = parse_hex_32("abcd").expect_err("too short");

        assert!(err.to_string().contains("expected 32 bytes"));
    }

    #[test]
    fn musubi_release_types_are_norito_roundtrip_ready() {
        let archive = MusubiArchiveRef::new(ManifestDigest::new([1; 32]), [2; 32], 10, 1);
        let bytes = archive.encode();
        let mut cursor = bytes.as_slice();
        let decoded = MusubiArchiveRef::decode(&mut cursor).expect("decode archive");

        assert!(cursor.is_empty());
        assert_eq!(decoded, archive);
    }
}
