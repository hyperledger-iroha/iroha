//! Space Directory operator helpers.

use std::{
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{Run, RunContext};
use eyre::{Result, WrapErr, eyre};
use iroha::data_model::{
    isi::{
        InstructionBox,
        space_directory::{
            ExpireSpaceDirectoryManifest, PublishSpaceDirectoryManifest,
            RevokeSpaceDirectoryManifest,
        },
    },
    nexus::{AssetPermissionManifest, DataSpaceId},
};
use iroha::{
    config::Config,
    data_model::{Decode, Encode},
};
use iroha_data_model::{
    asset::AssetDefinitionId,
    name::Name,
    nexus::{
        Allowance, AllowanceWindow, AmxRole, CapabilityScope, DenyDirective, ManifestEffect,
        ManifestEntry, ManifestVersion, SmartContractId, UniversalAccountId,
    },
};
use iroha_primitives::numeric::Numeric;
use norito::json::{self, JsonDeserialize, JsonSerialize, Value as JsonValue};
use reqwest::{
    blocking::Client as BlockingHttpClient,
    header::{ACCEPT, HeaderValue},
};
use url::Url;

#[allow(clippy::large_enum_variant)]
#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Manage UAID capability manifests.
    #[command(subcommand)]
    Manifest(ManifestCommand),
    /// Inspect UAID bindings surfaced by Torii.
    #[command(subcommand)]
    Bindings(BindingsCommand),
}

#[allow(clippy::large_enum_variant)]
#[derive(clap::Subcommand, Debug)]
pub enum ManifestCommand {
    /// Publish or replace a capability manifest (.to payload).
    Publish(ManifestPublishArgs),
    /// Encode manifest JSON into Norito bytes and record its hash.
    Encode(ManifestEncodeArgs),
    /// Revoke a manifest for a UAID/dataspace pair.
    Revoke(ManifestRevokeArgs),
    /// Expire a manifest that reached its scheduled end-of-life.
    Expire(ManifestExpireArgs),
    /// Produce an audit bundle for an existing capability manifest + dataspace profile.
    AuditBundle(ManifestAuditBundleArgs),
    /// Fetch manifests for a UAID via Torii.
    Fetch(ManifestFetchArgs),
    /// Scaffold manifest/profile templates for a UAID + dataspace pair.
    Scaffold(ManifestScaffoldArgs),
}

#[derive(clap::Args, Debug)]
pub struct ManifestAuditBundleArgs {
    /// Path to the Norito-encoded `AssetPermissionManifest` (.to).
    #[arg(long, value_name = "PATH", conflicts_with = "manifest_json")]
    pub manifest: Option<PathBuf>,
    /// Path to the JSON `AssetPermissionManifest` (encoded on export).
    #[arg(long, value_name = "PATH", conflicts_with = "manifest")]
    pub manifest_json: Option<PathBuf>,
    /// Dataspace profile JSON used to capture governance/audit hooks.
    #[arg(long, value_name = "PATH")]
    pub profile: PathBuf,
    /// Directory where the bundle (manifest/profile/hash/audit metadata) will be written.
    #[arg(long = "out-dir", value_name = "DIR")]
    pub out_dir: PathBuf,
    /// Optional operator note recorded inside the bundle metadata.
    #[arg(long, value_name = "TEXT")]
    pub notes: Option<String>,
}

#[derive(clap::Args, Debug)]
pub struct ManifestPublishArgs {
    /// Path to the Norito-encoded `AssetPermissionManifest` (.to).
    #[arg(long, value_name = "PATH", conflicts_with = "manifest_json")]
    pub manifest: Option<PathBuf>,
    /// Path to the JSON `AssetPermissionManifest` (encoded on submit).
    #[arg(long, value_name = "PATH", conflicts_with = "manifest")]
    pub manifest_json: Option<PathBuf>,
    /// Optional CLI-level reason used when publishing a new manifest (added to metadata).
    #[arg(long, value_name = "TEXT")]
    pub reason: Option<String>,
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::Manifest(command) => command.run(context),
            Command::Bindings(command) => command.run(context),
        }
    }
}

impl Run for ManifestCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            ManifestCommand::Publish(args) => args.run(context),
            ManifestCommand::Encode(args) => args.run(context),
            ManifestCommand::Revoke(args) => args.run(context),
            ManifestCommand::Expire(args) => args.run(context),
            ManifestCommand::AuditBundle(args) => args.run(context),
            ManifestCommand::Fetch(args) => args.run(context),
            ManifestCommand::Scaffold(args) => args.run(context),
        }
    }
}

impl Run for ManifestPublishArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let manifest = self.load_manifest()?;
        let mut instruction = PublishSpaceDirectoryManifest { manifest };
        if let Some(reason) = &self.reason {
            instruction.manifest.entries.iter_mut().for_each(|entry| {
                if entry.notes.is_none() {
                    entry.notes = Some(reason.clone());
                }
            });
        }
        let isi: InstructionBox = instruction.into();
        context.finish(vec![isi])
    }
}

impl ManifestPublishArgs {
    fn load_manifest(&self) -> Result<AssetPermissionManifest> {
        load_manifest_from_sources(self.manifest.as_deref(), self.manifest_json.as_deref())
    }
}

#[derive(clap::Args, Debug)]
pub struct ManifestEncodeArgs {
    /// Path to the JSON `AssetPermissionManifest`.
    #[arg(long, value_name = "PATH")]
    pub json: PathBuf,
    /// Target path for the Norito `.to` payload (defaults to `<json>.manifest.to`).
    #[arg(long, value_name = "PATH")]
    pub out: Option<PathBuf>,
    /// Optional file for the manifest hash (defaults to `<out>.hash`).
    #[arg(long, value_name = "PATH")]
    pub hash_out: Option<PathBuf>,
}

impl ManifestEncodeArgs {
    fn resolve_out(&self) -> PathBuf {
        self.out
            .clone()
            .unwrap_or_else(|| self.json.with_extension("manifest.to"))
    }

    fn resolve_hash_out(&self, to_path: &Path) -> PathBuf {
        self.hash_out
            .clone()
            .unwrap_or_else(|| to_path.with_extension("hash"))
    }
}

impl Run for ManifestEncodeArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let json_bytes = std::fs::read(&self.json).wrap_err_with(|| {
            format!("failed to read manifest JSON from {}", self.json.display())
        })?;
        let manifest: AssetPermissionManifest = norito::json::from_slice(&json_bytes)
            .wrap_err("manifest JSON could not be parsed as AssetPermissionManifest")?;

        let encoded = manifest.encode();

        let out_path = self.resolve_out();
        if let Some(parent) = out_path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)
                .wrap_err("failed to create manifest output directory")?;
        }
        std::fs::write(&out_path, &encoded).wrap_err_with(|| {
            format!("failed to write Norito payload to {}", out_path.display())
        })?;

        let hash = iroha_crypto::Hash::new(&encoded);
        let hash_hex = hex::encode(hash.as_ref());
        let hash_path = self.resolve_hash_out(out_path.as_path());
        if let Some(parent) = hash_path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).wrap_err("failed to create hash output directory")?;
        }
        std::fs::write(&hash_path, format!("{hash_hex}\n")).wrap_err_with(|| {
            format!("failed to write manifest hash to {}", hash_path.display())
        })?;

        context.println(format!(
            "Wrote Norito payload ({:?} bytes) to {}",
            encoded.len(),
            out_path.display()
        ))?;
        context.println(format!(
            "Manifest hash (BLAKE3-256): {} (written to {})",
            hash_hex,
            hash_path.display()
        ))?;
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct ManifestRevokeArgs {
    /// UAID whose manifest should be revoked.
    #[arg(long, value_name = "UAID")]
    pub uaid: String,
    /// Dataspace identifier hosting the manifest.
    #[arg(long, value_name = "ID")]
    pub dataspace: u64,
    /// Epoch (inclusive) when the revocation takes effect.
    #[arg(long, value_name = "EPOCH")]
    pub revoked_epoch: u64,
    /// Optional reason recorded with the revocation.
    #[arg(long, value_name = "TEXT")]
    pub reason: Option<String>,
}

impl Run for ManifestRevokeArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let uaid = self
            .uaid
            .parse()
            .wrap_err("failed to parse UAID (expecting `uaid:<hex>` or a 64-hex digest)")?;
        let dataspace = DataSpaceId::new(self.dataspace);
        let instruction = RevokeSpaceDirectoryManifest {
            uaid,
            dataspace,
            revoked_epoch: self.revoked_epoch,
            reason: self.reason.clone(),
        };
        context.finish(vec![InstructionBox::from(instruction)])
    }
}

#[derive(clap::Args, Debug)]
pub struct ManifestExpireArgs {
    /// UAID whose manifest should be expired.
    #[arg(long, value_name = "UAID")]
    pub uaid: String,
    /// Dataspace identifier hosting the manifest.
    #[arg(long, value_name = "ID")]
    pub dataspace: u64,
    /// Epoch (inclusive) when the expiry occurred.
    #[arg(long, value_name = "EPOCH")]
    pub expired_epoch: u64,
}

impl Run for ManifestExpireArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let uaid = self
            .uaid
            .parse()
            .wrap_err("failed to parse UAID (expecting `uaid:<hex>` or a 64-hex digest)")?;
        let dataspace = DataSpaceId::new(self.dataspace);
        let instruction = ExpireSpaceDirectoryManifest {
            uaid,
            dataspace,
            expired_epoch: self.expired_epoch,
        };
        context.finish(vec![InstructionBox::from(instruction)])
    }
}

#[derive(clap::Args, Debug)]
pub struct ManifestFetchArgs {
    /// UAID literal whose manifests should be fetched.
    #[arg(long, value_name = "UAID")]
    pub uaid: String,
    /// Optional dataspace id filter.
    #[arg(long, value_name = "ID")]
    pub dataspace: Option<u64>,
    /// Manifest lifecycle status filter (active, inactive, all).
    #[arg(long, value_enum, default_value_t = ManifestStatusArg::All)]
    pub status: ManifestStatusArg,
    /// Maximum number of manifests to return.
    #[arg(long, value_name = "N")]
    pub limit: Option<u64>,
    /// Offset for pagination.
    #[arg(long, value_name = "N")]
    pub offset: Option<u64>,
    /// Preferred account literal encoding (`ih58` or `compressed`).
    #[arg(
        long = "address-format",
        value_enum,
        default_value_t = AddressFormatArg::Ih58
    )]
    pub address_format: AddressFormatArg,
    /// Optional path where the JSON response will be stored.
    #[arg(long = "json-out", value_name = "PATH")]
    pub json_out: Option<PathBuf>,
}

impl Run for ManifestFetchArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client = SpaceDirectoryRestClient::new(context.config())?;
        let payload = client.fetch_manifests(&self)?;
        if let Some(path) = &self.json_out {
            write_json_response(path, &payload)?;
        }
        context.print_data(&payload)
    }
}

#[derive(clap::Args, Debug)]
pub struct ManifestScaffoldArgs {
    /// Universal account identifier (`uaid:<hex>` or raw 64-hex digest, LSB=1).
    #[arg(long = "uaid", value_name = "UAID", required = true)]
    pub uaid: String,
    /// Dataspace identifier the manifest targets.
    #[arg(long = "dataspace", value_name = "ID", required = true)]
    pub dataspace: u64,
    /// Activation epoch recorded in the manifest.
    #[arg(long = "activation-epoch", value_name = "EPOCH", required = true)]
    pub activation_epoch: u64,
    /// Optional expiry epoch recorded in the manifest.
    #[arg(long = "expiry-epoch", value_name = "EPOCH")]
    pub expiry_epoch: Option<u64>,
    /// Override the issued timestamp (milliseconds since UNIX epoch).
    #[arg(long = "issued-ms", value_name = "MS")]
    pub issued_ms: Option<u64>,
    /// Optional notes propagated to scaffolded entries.
    #[arg(long = "notes", value_name = "TEXT")]
    pub notes: Option<String>,
    /// Output path for the manifest JSON (defaults to `artifacts/space_directory/scaffold/<timestamp>/manifest.json`).
    #[arg(long = "manifest-out", value_name = "PATH")]
    pub manifest_out: Option<PathBuf>,
    /// Optional output path for the dataspace profile skeleton (defaults beside the manifest).
    #[arg(long = "profile-out", value_name = "PATH")]
    pub profile_out: Option<PathBuf>,
    /// Optional allow-entry template.
    #[command(flatten)]
    pub allow: ManifestScaffoldAllowArgs,
    /// Optional deny-entry template.
    #[command(flatten)]
    pub deny: ManifestScaffoldDenyArgs,
    /// Dataspace profile scaffolding parameters.
    #[command(flatten)]
    pub profile: ManifestScaffoldProfileArgs,
}

#[derive(clap::Args, Debug, Default)]
pub struct ManifestScaffoldAllowArgs {
    /// Optional dataspace override for the allow entry scope.
    #[arg(long = "allow-dataspace", value_name = "ID")]
    pub dataspace: Option<u64>,
    /// Program identifier (`contract.name`) for the allow entry.
    #[arg(long = "allow-program", value_name = "PROGRAM")]
    pub program: Option<String>,
    /// Method/entry-point for the allow entry.
    #[arg(long = "allow-method", value_name = "NAME")]
    pub method: Option<String>,
    /// Asset identifier (e.g. `xor#sora`) for the allow entry.
    #[arg(long = "allow-asset", value_name = "DEF#DOMAIN")]
    pub asset: Option<String>,
    /// AMX role enforced by the allow entry (`initiator` or `participant`).
    #[arg(long = "allow-role", value_name = "ROLE")]
    pub role: Option<String>,
    /// Deterministic allowance cap (decimal string).
    #[arg(long = "allow-max-amount", value_name = "DECIMAL")]
    pub max_amount: Option<String>,
    /// Allowance window (`per-slot`, `per-minute`, or `per-day`).
    #[arg(long = "allow-window", value_name = "WINDOW")]
    pub window: Option<String>,
    /// Optional operator note stored alongside the entry.
    #[arg(long = "allow-notes", value_name = "TEXT")]
    pub notes: Option<String>,
}

#[derive(clap::Args, Debug, Default)]
pub struct ManifestScaffoldDenyArgs {
    /// Optional dataspace override for the deny entry scope.
    #[arg(long = "deny-dataspace", value_name = "ID")]
    pub dataspace: Option<u64>,
    /// Program identifier (`contract.name`) for the deny entry.
    #[arg(long = "deny-program", value_name = "PROGRAM")]
    pub program: Option<String>,
    /// Method/entry-point for the deny entry.
    #[arg(long = "deny-method", value_name = "NAME")]
    pub method: Option<String>,
    /// Asset identifier (e.g. `xor#sora`) for the deny entry.
    #[arg(long = "deny-asset", value_name = "DEF#DOMAIN")]
    pub asset: Option<String>,
    /// AMX role enforced by the deny entry.
    #[arg(long = "deny-role", value_name = "ROLE")]
    pub role: Option<String>,
    /// Optional reason recorded for the deny directive.
    #[arg(long = "deny-reason", value_name = "TEXT")]
    pub reason: Option<String>,
    /// Optional operator note stored alongside the entry.
    #[arg(long = "deny-notes", value_name = "TEXT")]
    pub notes: Option<String>,
}

#[derive(clap::Args, Debug, Default)]
pub struct ManifestScaffoldProfileArgs {
    /// Dataspace profile identifier (default `profile.<dataspace>.v1`).
    #[arg(long = "profile-id", value_name = "ID")]
    pub profile_id: Option<String>,
    /// Epoch recorded in the profile metadata.
    #[arg(long = "profile-activation-epoch", value_name = "EPOCH")]
    pub activation_epoch: Option<u64>,
    /// Dataspace governance issuer account.
    #[arg(long = "profile-governance-issuer", value_name = "ACCOUNT@DOMAIN")]
    pub governance_issuer: Option<String>,
    /// Governance ticket/evidence label.
    #[arg(long = "profile-governance-ticket", value_name = "TEXT")]
    pub governance_ticket: Option<String>,
    /// Governance quorum threshold.
    #[arg(long = "profile-governance-quorum", value_name = "N")]
    pub governance_quorum: Option<u32>,
    /// Validator account identifiers.
    #[arg(long = "profile-validator", value_name = "ACCOUNT@DOMAIN")]
    pub validators: Vec<String>,
    /// Validator quorum threshold.
    #[arg(long = "profile-validator-quorum", value_name = "N")]
    pub validator_quorum: Option<u32>,
    /// Protected namespace entries.
    #[arg(long = "profile-protected-namespace", value_name = "NAME")]
    pub protected_namespaces: Vec<String>,
    /// DA class label (default `A`).
    #[arg(long = "profile-da-class", value_name = "TEXT")]
    pub da_class: Option<String>,
    /// DA attester quorum.
    #[arg(long = "profile-da-quorum", value_name = "N")]
    pub da_quorum: Option<u32>,
    /// DA attester identifiers.
    #[arg(long = "profile-da-attester", value_name = "ACCOUNT@DOMAIN")]
    pub da_attesters: Vec<String>,
    /// DA rotation cadence in epochs.
    #[arg(long = "profile-da-rotation-epochs", value_name = "EPOCHS")]
    pub da_rotation_epochs: Option<u64>,
    /// Composability group identifier (hex string).
    #[arg(long = "profile-composability-group", value_name = "HEX")]
    pub composability_group: Option<String>,
    /// Optional audit log schema hint.
    #[arg(long = "profile-audit-log-schema", value_name = "TEXT")]
    pub audit_log_schema: Option<String>,
    /// Optional `PagerDuty` service label.
    #[arg(long = "profile-pagerduty-service", value_name = "TEXT")]
    pub pagerduty_service: Option<String>,
}

impl Run for ManifestScaffoldArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let uaid = parse_uaid_literal(&self.uaid)?;
        let dataspace = DataSpaceId::new(self.dataspace);
        let issued_ms = self.issued_ms.unwrap_or(current_unix_time_ms()?);
        let entries = self.build_entries(dataspace)?;
        let manifest = AssetPermissionManifest {
            version: ManifestVersion::V1,
            uaid,
            dataspace,
            issued_ms,
            activation_epoch: self.activation_epoch,
            expiry_epoch: self.expiry_epoch,
            entries,
        };

        let manifest_path = self.resolve_manifest_out(&manifest)?;
        write_json(&manifest_path, &manifest)?;
        context.println(format!(
            "Wrote Space Directory manifest scaffold to {}",
            manifest_path.display()
        ))?;

        if let Some(profile_path) = self.resolve_profile_out(&manifest_path)? {
            let profile_json = self
                .profile
                .build_profile_value(&manifest, self.notes.as_deref());
            write_json(&profile_path, &profile_json)?;
            context.println(format!(
                "Wrote dataspace profile scaffold to {}",
                profile_path.display()
            ))?;
        }

        Ok(())
    }
}

impl ManifestScaffoldArgs {
    fn build_entries(&self, manifest_dataspace: DataSpaceId) -> Result<Vec<ManifestEntry>> {
        let mut entries = Vec::new();
        if let Some(entry) = self
            .allow
            .build_entry(manifest_dataspace, self.notes.as_deref())?
        {
            entries.push(entry);
        }
        if let Some(entry) = self
            .deny
            .build_entry(manifest_dataspace, self.notes.as_deref())?
        {
            entries.push(entry);
        }
        Ok(entries)
    }

    fn resolve_manifest_out(&self, manifest: &AssetPermissionManifest) -> Result<PathBuf> {
        let path = self.manifest_out.as_ref().map_or_else(
            || {
                let dir = default_scaffold_dir().join(format!(
                    "{}_{}",
                    manifest.dataspace.as_u64(),
                    manifest.activation_epoch
                ));
                dir.join("manifest.json")
            },
            PathBuf::clone,
        );
        ensure_parent_dir(&path)?;
        Ok(path)
    }

    fn resolve_profile_out(&self, manifest_path: &Path) -> Result<Option<PathBuf>> {
        let path = self.profile_out.as_ref().map_or_else(
            || {
                manifest_path.parent().map_or_else(
                    || PathBuf::from("profile.json"),
                    |dir| dir.join("profile.json"),
                )
            },
            PathBuf::clone,
        );
        ensure_parent_dir(&path)?;
        Ok(Some(path))
    }
}

impl ManifestScaffoldAllowArgs {
    fn build_entry(
        &self,
        manifest_dataspace: DataSpaceId,
        default_notes: Option<&str>,
    ) -> Result<Option<ManifestEntry>> {
        if self.is_empty() {
            return Ok(None);
        }
        let scope = CapabilityScope {
            dataspace: self.dataspace.map(DataSpaceId::new),
            program: opt_program(self.program.as_deref(), "--allow-program")?,
            method: opt_name(self.method.as_deref(), "--allow-method")?,
            asset: opt_asset(self.asset.as_deref(), "--allow-asset")?,
            role: opt_amx_role(self.role.as_deref(), "--allow-role")?,
        };
        let allowance = Allowance {
            max_amount: match self.max_amount.as_deref() {
                Some(value) => Some(parse_numeric_decimal(value, "--allow-max-amount")?),
                None => None,
            },
            window: parse_allowance_window(self.window.as_deref())?,
        };
        let notes = self
            .notes
            .clone()
            .or_else(|| default_notes.map(ToOwned::to_owned));
        Ok(Some(ManifestEntry {
            scope: update_scope_defaults(scope, manifest_dataspace),
            effect: ManifestEffect::Allow(allowance),
            notes,
        }))
    }

    fn is_empty(&self) -> bool {
        self.dataspace.is_none()
            && self.program.is_none()
            && self.method.is_none()
            && self.asset.is_none()
            && self.role.is_none()
            && self.max_amount.is_none()
    }
}

impl ManifestScaffoldDenyArgs {
    fn build_entry(
        &self,
        manifest_dataspace: DataSpaceId,
        default_notes: Option<&str>,
    ) -> Result<Option<ManifestEntry>> {
        if self.is_empty() {
            return Ok(None);
        }
        let scope = CapabilityScope {
            dataspace: self.dataspace.map(DataSpaceId::new),
            program: opt_program(self.program.as_deref(), "--deny-program")?,
            method: opt_name(self.method.as_deref(), "--deny-method")?,
            asset: opt_asset(self.asset.as_deref(), "--deny-asset")?,
            role: opt_amx_role(self.role.as_deref(), "--deny-role")?,
        };
        let notes = self
            .notes
            .clone()
            .or_else(|| default_notes.map(ToOwned::to_owned));
        let effect = ManifestEffect::Deny(DenyDirective {
            reason: self.reason.clone(),
        });
        Ok(Some(ManifestEntry {
            scope: update_scope_defaults(scope, manifest_dataspace),
            effect,
            notes,
        }))
    }

    fn is_empty(&self) -> bool {
        self.dataspace.is_none()
            && self.program.is_none()
            && self.method.is_none()
            && self.asset.is_none()
            && self.role.is_none()
            && self.reason.is_none()
    }
}

impl ManifestScaffoldProfileArgs {
    #[allow(clippy::too_many_lines)]
    fn build_profile_value(
        &self,
        manifest: &AssetPermissionManifest,
        default_notes: Option<&str>,
    ) -> JsonValue {
        let dataspace = manifest.dataspace.as_u64();
        let profile_id = self
            .profile_id
            .clone()
            .unwrap_or_else(|| format!("profile.{dataspace}.v1"));
        let activation_epoch = self.activation_epoch.unwrap_or(manifest.activation_epoch);
        let governance_issuer = self
            .governance_issuer
            .clone()
            .unwrap_or_else(|| "governance@example".to_owned());
        let governance_ticket = self
            .governance_ticket
            .clone()
            .unwrap_or_else(|| "gov-ticket-placeholder".to_owned());
        let governance_quorum = self.governance_quorum.unwrap_or(1);
        let validators = if self.validators.is_empty() {
            vec![
                "validator-1@example".to_owned(),
                "validator-2@example".to_owned(),
            ]
        } else {
            self.validators.clone()
        };
        let validator_quorum = self
            .validator_quorum
            .unwrap_or_else(|| u32::try_from(validators.len().max(1)).unwrap_or(u32::MAX));
        let protected_namespaces = if self.protected_namespaces.is_empty() {
            vec!["default".to_owned()]
        } else {
            self.protected_namespaces.clone()
        };
        let da_class = self.da_class.clone().unwrap_or_else(|| "A".to_owned());
        let da_quorum = self.da_quorum.unwrap_or(4);
        let da_attesters = if self.da_attesters.is_empty() {
            vec![
                "attester-1@example".to_owned(),
                "attester-2@example".to_owned(),
            ]
        } else {
            self.da_attesters.clone()
        };
        let da_rotation_epochs = self.da_rotation_epochs.unwrap_or(96);
        let composability_group = self.composability_group.clone().unwrap_or_else(|| {
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_owned()
        });
        let audit_log_schema = self
            .audit_log_schema
            .clone()
            .unwrap_or_else(|| "metrics/space_directory_audit.prom".to_owned());
        let pagerduty_service = self
            .pagerduty_service
            .clone()
            .unwrap_or_else(|| "space-directory-oncall".to_owned());

        let mut governance = json::Map::new();
        governance.insert("issuer".into(), JsonValue::from(governance_issuer));
        governance.insert("evidence_ticket".into(), JsonValue::from(governance_ticket));
        governance.insert(
            "quorum".into(),
            JsonValue::from(u64::from(governance_quorum)),
        );

        let mut da_profile = json::Map::new();
        da_profile.insert("class".into(), JsonValue::from(da_class));
        da_profile.insert("quorum".into(), JsonValue::from(u64::from(da_quorum)));
        da_profile.insert(
            "attesters".into(),
            JsonValue::Array(
                da_attesters
                    .into_iter()
                    .map(JsonValue::from)
                    .collect::<Vec<_>>(),
            ),
        );
        da_profile.insert(
            "rotation_epochs".into(),
            JsonValue::from(da_rotation_epochs),
        );

        let mut composability = json::Map::new();
        composability.insert("group_id_hex".into(), JsonValue::from(composability_group));
        composability.insert(
            "activation_epoch".into(),
            JsonValue::from(manifest.activation_epoch),
        );
        composability.insert("whitelist".into(), JsonValue::Array(Vec::new()));
        composability.insert(
            "deny_wins_policy".into(),
            JsonValue::from(default_notes.unwrap_or("update with governance guidance")),
        );

        let mut audit_hooks = json::Map::new();
        audit_hooks.insert(
            "events".into(),
            JsonValue::Array(vec![
                JsonValue::from("SpaceDirectoryEvent.ManifestActivated"),
                JsonValue::from("SpaceDirectoryEvent.ManifestRevoked"),
            ]),
        );
        audit_hooks.insert("log_schema".into(), JsonValue::from(audit_log_schema));
        audit_hooks.insert(
            "pagerduty_service".into(),
            JsonValue::from(pagerduty_service),
        );

        let mut profile = json::Map::new();
        profile.insert("profile_id".into(), JsonValue::from(profile_id));
        profile.insert("dataspace".into(), JsonValue::from(dataspace));
        profile.insert("activation_epoch".into(), JsonValue::from(activation_epoch));
        profile.insert("governance".into(), JsonValue::Object(governance));
        profile.insert(
            "validators".into(),
            JsonValue::Array(validators.into_iter().map(JsonValue::from).collect()),
        );
        profile.insert(
            "validator_quorum".into(),
            JsonValue::from(u64::from(validator_quorum)),
        );
        profile.insert(
            "protected_namespaces".into(),
            JsonValue::Array(
                protected_namespaces
                    .into_iter()
                    .map(JsonValue::from)
                    .collect(),
            ),
        );
        profile.insert("da_profile".into(), JsonValue::Object(da_profile));
        profile.insert(
            "composability_group".into(),
            JsonValue::Object(composability),
        );
        profile.insert("audit_hooks".into(), JsonValue::Object(audit_hooks));

        JsonValue::Object(profile)
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum BindingsCommand {
    /// Fetch UAID dataspace bindings via Torii.
    Fetch(BindingsFetchArgs),
}

impl Run for BindingsCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            BindingsCommand::Fetch(args) => args.run(context),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct BindingsFetchArgs {
    /// UAID literal whose bindings should be fetched.
    #[arg(long, value_name = "UAID")]
    pub uaid: String,
    /// Preferred account literal encoding (`ih58` or `compressed`).
    #[arg(
        long = "address-format",
        value_enum,
        default_value_t = AddressFormatArg::Ih58
    )]
    pub address_format: AddressFormatArg,
    /// Optional path where the JSON response will be stored.
    #[arg(long = "json-out", value_name = "PATH")]
    pub json_out: Option<PathBuf>,
}

impl Run for BindingsFetchArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client = SpaceDirectoryRestClient::new(context.config())?;
        let payload = client.fetch_bindings(&self)?;
        if let Some(path) = &self.json_out {
            write_json_response(path, &payload)?;
        }
        context.print_data(&payload)
    }
}

#[derive(Default, clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddressFormatArg {
    #[default]
    #[value(name = "ih58", alias = "ih-b32", alias = "canonical")]
    Ih58,
    #[value(name = "compressed", alias = "snx1")]
    Compressed,
}

impl AddressFormatArg {
    fn as_query_value(self) -> &'static str {
        match self {
            Self::Ih58 => "ih58",
            Self::Compressed => "compressed",
        }
    }
}

#[derive(Default, clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManifestStatusArg {
    Active,
    Inactive,
    #[default]
    All,
}

impl ManifestStatusArg {
    fn as_query_value(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
            Self::All => "all",
        }
    }
}

impl Run for ManifestAuditBundleArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let manifest =
            load_manifest_from_sources(self.manifest.as_deref(), self.manifest_json.as_deref())?;
        let encoded = manifest.encode();
        fs::create_dir_all(&self.out_dir).wrap_err_with(|| {
            format!(
                "failed to create audit bundle directory {}",
                self.out_dir.display()
            )
        })?;

        let manifest_json_name = "manifest.json";
        let manifest_to_name = "manifest.to";
        let manifest_hash_name = "manifest.hash";
        let profile_json_name = "profile.json";
        let bundle_name = "audit_bundle.json";

        let manifest_json_path = self.out_dir.join(manifest_json_name);
        let manifest_to_path = self.out_dir.join(manifest_to_name);
        let manifest_hash_path = self.out_dir.join(manifest_hash_name);
        let profile_out_path = self.out_dir.join(profile_json_name);
        let bundle_path = self.out_dir.join(bundle_name);

        let manifest_json_bytes = norito::json::to_vec_pretty(&manifest)
            .wrap_err("failed to serialize manifest JSON for audit bundle")?;
        fs::write(&manifest_json_path, manifest_json_bytes).wrap_err_with(|| {
            format!(
                "failed to write manifest JSON to {}",
                manifest_json_path.display()
            )
        })?;

        fs::write(&manifest_to_path, &encoded).wrap_err_with(|| {
            format!(
                "failed to write manifest Norito payload to {}",
                manifest_to_path.display()
            )
        })?;

        let manifest_hash = iroha_crypto::Hash::new(&encoded);
        let manifest_hash_hex = hex::encode(manifest_hash.as_ref());
        fs::write(&manifest_hash_path, format!("{manifest_hash_hex}\n")).wrap_err_with(|| {
            format!(
                "failed to write manifest hash to {}",
                manifest_hash_path.display()
            )
        })?;

        let profile_bytes = fs::read(&self.profile).wrap_err_with(|| {
            format!(
                "failed to read dataspace profile {}",
                self.profile.display()
            )
        })?;
        let profile_json: JsonValue = norito::json::from_slice(&profile_bytes)
            .wrap_err("profile JSON could not be parsed")?;
        let profile_serialized = norito::json::to_vec_pretty(&profile_json)
            .wrap_err("failed to serialize profile JSON for audit bundle")?;
        fs::write(&profile_out_path, profile_serialized).wrap_err_with(|| {
            format!(
                "failed to write profile JSON to {}",
                profile_out_path.display()
            )
        })?;

        let audit_hooks = extract_audit_hooks(&profile_json)?;
        let generated_at_ms = current_unix_time_ms()?;
        let dataspace_id: u64 = manifest.dataspace.into();
        let bundle = ManifestAuditBundle {
            generated_at_ms,
            uaid: manifest.uaid.to_string(),
            dataspace_id,
            manifest_hash: manifest_hash_hex,
            activation_epoch: manifest.activation_epoch,
            expiry_epoch: manifest.expiry_epoch,
            audit_hooks,
            notes: self.notes.clone(),
            artifacts: BundleArtifacts {
                manifest_json: manifest_json_name.to_owned(),
                manifest_norito: manifest_to_name.to_owned(),
                manifest_hash: manifest_hash_name.to_owned(),
                profile_json: profile_json_name.to_owned(),
            },
            profile: profile_json,
        };

        let bundle_bytes = norito::json::to_vec_pretty(&bundle)
            .wrap_err("failed to serialize audit bundle metadata")?;
        fs::write(&bundle_path, bundle_bytes).wrap_err_with(|| {
            format!("failed to write audit bundle to {}", bundle_path.display())
        })?;

        context.println(format!(
            "Wrote Space Directory audit bundle to {}",
            bundle_path.display()
        ))?;
        Ok(())
    }
}

struct SpaceDirectoryRestClient {
    client: BlockingHttpClient,
    base_url: Url,
    basic_auth: Option<(String, String)>,
}

impl SpaceDirectoryRestClient {
    fn new(config: &Config) -> Result<Self> {
        let client = BlockingHttpClient::builder()
            .user_agent("iroha-cli space-directory")
            .build()
            .wrap_err("failed to construct HTTP client for space-directory queries")?;
        let basic_auth = config.basic_auth.as_ref().map(|auth| {
            (
                auth.web_login.as_str().to_owned(),
                auth.password.expose_secret().to_owned(),
            )
        });
        Ok(Self {
            client,
            base_url: config.torii_api_url.clone(),
            basic_auth,
        })
    }

    fn fetch_manifests(&self, args: &ManifestFetchArgs) -> Result<JsonValue> {
        let mut url = build_space_directory_url(
            &self.base_url,
            &[
                "v1",
                "space-directory",
                "uaids",
                args.uaid.as_str(),
                "manifests",
            ],
        )?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("address_format", args.address_format.as_query_value());
            if let Some(dataspace) = args.dataspace {
                pairs.append_pair("dataspace", &dataspace.to_string());
            }
            if args.status != ManifestStatusArg::All {
                pairs.append_pair("status", args.status.as_query_value());
            }
            if let Some(limit) = args.limit {
                pairs.append_pair("limit", &limit.to_string());
            }
            if let Some(offset) = args.offset {
                pairs.append_pair("offset", &offset.to_string());
            }
        }
        self.get_json(&url)
    }

    fn fetch_bindings(&self, args: &BindingsFetchArgs) -> Result<JsonValue> {
        let mut url = build_space_directory_url(
            &self.base_url,
            &["v1", "space-directory", "uaids", args.uaid.as_str()],
        )?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("address_format", args.address_format.as_query_value());
        }
        self.get_json(&url)
    }

    fn get_json(&self, url: &Url) -> Result<JsonValue> {
        let mut request = self
            .client
            .get(url.clone())
            .header(ACCEPT, HeaderValue::from_static("application/json"));
        if let Some((ref login, ref password)) = self.basic_auth {
            request = request.basic_auth(login, Some(password));
        }
        let response = request
            .send()
            .wrap_err_with(|| format!("failed to query Torii at {url}"))?;
        let status = response.status();
        let bytes = response
            .bytes()
            .wrap_err("failed to read Torii response body")?
            .to_vec();
        if !status.is_success() {
            let preview = String::from_utf8_lossy(&bytes);
            return Err(eyre!(
                "Torii {url} responded with {}: {}",
                status,
                preview.trim()
            ));
        }
        let value: JsonValue = json::from_slice(&bytes)
            .map_err(|err| eyre!("failed to parse Torii response JSON: {err}"))?;
        Ok(value)
    }
}

fn write_json_response(path: &Path, value: &JsonValue) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .wrap_err_with(|| format!("failed to create {}", parent.display()))?;
    }
    let bytes = json::to_vec_pretty(value)
        .map_err(|err| eyre!("failed to serialize JSON payload: {err}"))?;
    fs::write(path, bytes).wrap_err_with(|| format!("failed to write {}", path.display()))
}

fn build_space_directory_url(base: &Url, segments: &[&str]) -> Result<Url> {
    let mut url = base.clone();
    {
        let mut path = url
            .path_segments_mut()
            .map_err(|()| eyre!("torii_api_url must include a path segment-ready base"))?;
        path.pop_if_empty();
        path.extend(segments);
    }
    Ok(url)
}

fn load_manifest_from_sources(
    manifest_path: Option<&Path>,
    manifest_json_path: Option<&Path>,
) -> Result<AssetPermissionManifest> {
    match (manifest_path, manifest_json_path) {
        (Some(path), None) => {
            let bytes = fs::read(path).wrap_err_with(|| {
                format!("failed to read manifest payload from {}", path.display())
            })?;
            AssetPermissionManifest::decode(&mut &*bytes).wrap_err("manifest is not valid Norito")
        }
        (None, Some(path)) => {
            let bytes = fs::read(path).wrap_err_with(|| {
                format!("failed to read manifest JSON from {}", path.display())
            })?;
            norito::json::from_slice(&bytes).wrap_err_with(|| {
                format!(
                    "manifest JSON could not be parsed as AssetPermissionManifest ({})",
                    path.display()
                )
            })
        }
        (None, None) => Err(eyre::eyre!(
            "either --manifest (Norito .to) or --manifest-json (raw JSON) must be provided"
        )),
        (Some(_), Some(_)) => unreachable!("clap enforces conflicts"),
    }
}

fn extract_audit_hooks(profile: &JsonValue) -> Result<Option<AuditHookSummary>> {
    let Some(raw_hooks) = profile.get("audit_hooks") else {
        return Ok(None);
    };
    if raw_hooks.is_null() {
        return Ok(None);
    }
    let hooks: AuditHookSummary = norito::json::from_value(raw_hooks.clone())
        .wrap_err("profile audit_hooks could not be parsed")?;
    eyre::ensure!(
        hooks
            .events
            .iter()
            .any(|event| event == "SpaceDirectoryEvent.ManifestActivated"),
        "profile audit_hooks.events must include SpaceDirectoryEvent.ManifestActivated"
    );
    eyre::ensure!(
        hooks
            .events
            .iter()
            .any(|event| event == "SpaceDirectoryEvent.ManifestRevoked"),
        "profile audit_hooks.events must include SpaceDirectoryEvent.ManifestRevoked"
    );
    Ok(Some(hooks))
}

fn current_unix_time_ms() -> Result<u64> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .wrap_err("system clock drifted before UNIX_EPOCH")?;
    u64::try_from(duration.as_millis())
        .map_err(|_| eyre!("system clock exceeded u64::MAX milliseconds since UNIX_EPOCH"))
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .wrap_err_with(|| format!("failed to create {}", parent.display()))?;
    }
    Ok(())
}

fn default_scaffold_dir() -> PathBuf {
    PathBuf::from("artifacts")
        .join("space_directory")
        .join("scaffold")
}

fn write_json<T: JsonSerialize>(path: &Path, value: &T) -> Result<()> {
    ensure_parent_dir(path)?;
    let bytes = norito::json::to_vec_pretty(value)
        .map_err(|err| eyre!("failed to serialize JSON payload: {err}"))?;
    fs::write(path, bytes).wrap_err_with(|| format!("failed to write {}", path.display()))
}

fn parse_numeric_decimal(value: &str, flag: &str) -> Result<Numeric> {
    value
        .parse::<Numeric>()
        .wrap_err_with(|| format!("{flag} must be a decimal number"))
}

fn parse_allowance_window(value: Option<&str>) -> Result<AllowanceWindow> {
    value
        .map(|raw| raw.trim().to_ascii_lowercase())
        .map_or_else(
            || Ok(AllowanceWindow::PerDay),
            |raw| {
                if raw.is_empty() || raw == "per-day" || raw == "perday" {
                    Ok(AllowanceWindow::PerDay)
                } else if raw == "per-minute" || raw == "perminute" {
                    Ok(AllowanceWindow::PerMinute)
                } else if raw == "per-slot" || raw == "perslot" {
                    Ok(AllowanceWindow::PerSlot)
                } else {
                    Err(eyre!("invalid allowance window `{raw}`"))
                }
            },
        )
}

fn opt_program(value: Option<&str>, flag: &str) -> Result<Option<SmartContractId>> {
    value
        .map(|raw| {
            raw.parse()
                .wrap_err_with(|| format!("failed to parse {flag}"))
        })
        .transpose()
}

fn opt_name(value: Option<&str>, flag: &str) -> Result<Option<Name>> {
    value
        .map(|raw| {
            raw.parse()
                .wrap_err_with(|| format!("failed to parse {flag}"))
        })
        .transpose()
}

fn opt_asset(value: Option<&str>, flag: &str) -> Result<Option<AssetDefinitionId>> {
    value
        .map(|raw| {
            raw.parse::<AssetDefinitionId>()
                .wrap_err_with(|| format!("failed to parse {flag}"))
        })
        .transpose()
}

fn opt_amx_role(value: Option<&str>, flag: &str) -> Result<Option<AmxRole>> {
    value
        .map(|raw| match raw.trim().to_ascii_lowercase().as_str() {
            "initiator" => Ok(AmxRole::Initiator),
            "participant" => Ok(AmxRole::Participant),
            other => Err(eyre!("invalid {} role `{other}`", flag)),
        })
        .transpose()
}

fn update_scope_defaults(
    mut scope: CapabilityScope,
    manifest_dataspace: DataSpaceId,
) -> CapabilityScope {
    if scope.dataspace.is_none() {
        scope.dataspace = Some(manifest_dataspace);
    }
    scope
}

fn parse_uaid_literal(raw: &str) -> Result<UniversalAccountId> {
    UniversalAccountId::from_str(raw.trim())
        .wrap_err("UAID literal must be `uaid:<hex>` or a 64-hex digest")
}

#[derive(Debug, JsonSerialize)]
struct ManifestAuditBundle {
    generated_at_ms: u64,
    uaid: String,
    dataspace_id: u64,
    manifest_hash: String,
    activation_epoch: u64,
    expiry_epoch: Option<u64>,
    #[norito(skip_serializing_if = "Option::is_none")]
    audit_hooks: Option<AuditHookSummary>,
    #[norito(skip_serializing_if = "Option::is_none")]
    notes: Option<String>,
    artifacts: BundleArtifacts,
    profile: JsonValue,
}

#[derive(Debug, JsonSerialize)]
struct BundleArtifacts {
    manifest_json: String,
    manifest_norito: String,
    manifest_hash: String,
    profile_json: String,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
struct AuditHookSummary {
    events: Vec<String>,
    #[norito(default)]
    log_schema: Option<String>,
    #[norito(default)]
    pagerduty_service: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RunContext;
    use eyre::Result;
    use iroha::{
        config::Config,
        crypto::{Algorithm, Hash as CryptoHash, KeyPair},
        data_model::{
            metadata::Metadata,
            nexus::{
                Allowance, AllowanceWindow, AmxRole, CapabilityScope, DataSpaceId, ManifestEffect,
                ManifestEntry, ManifestVersion, UniversalAccountId,
            },
            prelude::*,
        },
    };
    use iroha_i18n::{Bundle, Language, Localizer};
    use norito::json::JsonSerialize;
    use std::path::Path;
    use tempfile::tempdir;
    use url::Url;

    #[test]
    fn parse_uaid_literal_accepts_prefixed_or_raw() {
        let uaid = UniversalAccountId::from_hash(CryptoHash::new(b"cli-uaid"));
        let hex = uaid.as_hash().to_string();
        let variants = [
            hex.clone(),
            format!("uaid:{hex}"),
            format!("UAID:{}", hex.to_uppercase()),
        ];

        for literal in variants {
            let parsed = parse_uaid_literal(&literal).expect("parse UAID literal");
            assert_eq!(parsed, uaid);
        }
    }

    #[test]
    fn manifest_encode_writes_payload_and_hash_with_defaults() {
        let manifest = sample_manifest();
        let dir = tempdir().expect("tmpdir");
        let json_path = dir.path().join("manifest.json");
        write_manifest_json(&json_path, &manifest);

        let mut ctx = TestContext::new();
        ManifestEncodeArgs {
            json: json_path.clone(),
            out: None,
            hash_out: None,
        }
        .run(&mut ctx)
        .expect("encode manifest");

        let to_path = dir.path().join("manifest.manifest.to");
        let hash_path = dir.path().join("manifest.manifest.hash");
        assert!(to_path.exists(), "missing encoded payload");
        assert!(hash_path.exists(), "missing hash payload");

        let encoded = std::fs::read(&to_path).expect("read encoded manifest");
        let mut reader: &[u8] = &encoded;
        let decoded = AssetPermissionManifest::decode(&mut reader).expect("decode Norito manifest");
        assert_eq!(decoded, manifest, "encoded manifest differs");

        let expected_hash = iroha_crypto::Hash::new(&encoded);
        let hash_body = std::fs::read_to_string(&hash_path).expect("read manifest hash file");
        assert_eq!(
            hash_body.trim(),
            hex::encode(expected_hash.as_ref()),
            "hash file did not match encoded payload"
        );

        assert_eq!(
            ctx.lines.len(),
            2,
            "encode command should print two status lines"
        );
        assert!(
            ctx.lines[0].contains("Wrote Norito payload"),
            "missing payload log line"
        );
        assert!(
            ctx.lines[1].contains("Manifest hash"),
            "missing hash log line"
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn manifest_scaffold_writes_manifest_and_profile_templates() {
        let dir = tempdir().expect("tmpdir");
        let manifest_path = dir.path().join("fixture").join("manifest.json");
        let profile_path = dir.path().join("fixture").join("profile.json");

        let allow_args = ManifestScaffoldAllowArgs {
            program: Some("cbdc.transfer".to_owned()),
            method: Some("transfer".to_owned()),
            asset: Some("cbdc#centralbank".to_owned()),
            role: Some("initiator".to_owned()),
            max_amount: Some("500000000".to_owned()),
            window: Some("per-day".to_owned()),
            ..Default::default()
        };
        let deny_args = ManifestScaffoldDenyArgs {
            program: Some("cbdc.kit".to_owned()),
            method: Some("withdraw".to_owned()),
            reason: Some("Withdrawals disabled for dApp".to_owned()),
            ..Default::default()
        };
        let profile_args = ManifestScaffoldProfileArgs {
            profile_id: Some("profile.cbdc.preview".to_owned()),
            governance_issuer: Some("parliament@cbdc".to_owned()),
            governance_ticket: Some("gov-ticket".to_owned()),
            governance_quorum: Some(4),
            validators: vec![
                "cbdc-validator-1@cbdc".to_owned(),
                "cbdc-validator-2@cbdc".to_owned(),
            ],
            validator_quorum: Some(2),
            protected_namespaces: vec!["cbdc".to_owned(), "gov".to_owned()],
            da_class: Some("A".to_owned()),
            da_quorum: Some(8),
            da_attesters: vec![
                "da-attester-1@cbdc".to_owned(),
                "da-attester-2@cbdc".to_owned(),
            ],
            da_rotation_epochs: Some(96),
            composability_group: Some(
                "3b2a4e8b7047e36de62a1b4a64f48870664f581e06e4b1340f9e963835d02c8b".to_owned(),
            ),
            audit_log_schema: Some("metrics/nexus_space_directory_audit.prom".to_owned()),
            pagerduty_service: Some("Nexus-SpaceDirectory".to_owned()),
            ..Default::default()
        };

        let mut ctx = TestContext::new();
        ManifestScaffoldArgs {
            uaid: sample_manifest().uaid.to_string(),
            dataspace: 11,
            activation_epoch: 4097,
            expiry_epoch: Some(8192),
            issued_ms: Some(1_762_723_200_000),
            notes: Some("scaffold smoke".to_owned()),
            manifest_out: Some(manifest_path.clone()),
            profile_out: Some(profile_path.clone()),
            allow: allow_args,
            deny: deny_args,
            profile: profile_args,
        }
        .run(&mut ctx)
        .expect("scaffold manifest/profile");

        assert!(manifest_path.exists(), "manifest scaffold missing");
        assert!(profile_path.exists(), "profile scaffold missing");

        let manifest_json: JsonValue =
            json::from_slice(&fs::read(&manifest_path).expect("read manifest scaffold"))
                .expect("parse manifest scaffold");
        assert_eq!(
            manifest_json.get("dataspace").and_then(JsonValue::as_u64),
            Some(11),
            "manifest dataspace mismatch"
        );
        assert_eq!(
            manifest_json
                .get("entries")
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(2),
            "manifest scaffold should include two entries"
        );

        let profile_json: JsonValue =
            json::from_slice(&fs::read(&profile_path).expect("read profile scaffold"))
                .expect("parse profile scaffold");
        let events = profile_json
            .get("audit_hooks")
            .and_then(|hooks| hooks.get("events"))
            .and_then(JsonValue::as_array)
            .cloned()
            .unwrap_or_default();
        assert!(
            events
                .iter()
                .any(|value| value.as_str() == Some("SpaceDirectoryEvent.ManifestActivated")),
            "profile scaffold must subscribe to ManifestActivated"
        );
        assert!(
            events
                .iter()
                .any(|value| value.as_str() == Some("SpaceDirectoryEvent.ManifestRevoked")),
            "profile scaffold must subscribe to ManifestRevoked"
        );
        assert!(
            ctx.lines
                .iter()
                .any(|line| line.contains("manifest scaffold")),
            "scaffold command should log manifest path"
        );
    }

    #[test]
    fn manifest_encode_respects_custom_output_paths() {
        let manifest = sample_manifest();
        let dir = tempdir().expect("tmpdir");
        let json_path = dir.path().join("fixtures").join("manifest.json");
        std::fs::create_dir_all(json_path.parent().unwrap()).expect("json dir");
        write_manifest_json(&json_path, &manifest);

        let out_path = dir
            .path()
            .join("artifacts")
            .join("nexus")
            .join("uaid")
            .join("wholesale.to");
        let hash_path = dir
            .path()
            .join("metadata")
            .join("nexus")
            .join("uaid")
            .join("wholesale.hash");

        let mut ctx = TestContext::new();
        ManifestEncodeArgs {
            json: json_path,
            out: Some(out_path.clone()),
            hash_out: Some(hash_path.clone()),
        }
        .run(&mut ctx)
        .expect("encode manifest with overrides");

        assert!(out_path.exists(), "custom output path was not created");
        assert!(hash_path.exists(), "custom hash path was not created");

        let encoded = std::fs::read(&out_path).expect("read encoded manifest");
        let expected_hash = iroha_crypto::Hash::new(&encoded);
        let hash_body = std::fs::read_to_string(&hash_path).expect("read manifest hash file");
        assert_eq!(
            hash_body.trim(),
            hex::encode(expected_hash.as_ref()),
            "hash file did not match encoded payload"
        );
    }

    fn write_manifest_json(path: &Path, manifest: &AssetPermissionManifest) {
        let json_bytes = norito::json::to_vec(manifest).expect("serialize manifest to JSON");
        std::fs::write(path, json_bytes).expect("write JSON manifest");
    }

    fn sample_manifest() -> AssetPermissionManifest {
        AssetPermissionManifest {
            version: ManifestVersion::V1,
            uaid: UniversalAccountId::from_hash(CryptoHash::new(b"cli-manifest")),
            dataspace: DataSpaceId::new(11),
            issued_ms: 1_736_668_800_000,
            activation_epoch: 4_096,
            expiry_epoch: Some(4_600),
            entries: vec![ManifestEntry {
                scope: CapabilityScope {
                    dataspace: Some(DataSpaceId::new(11)),
                    program: None,
                    method: None,
                    asset: None,
                    role: Some(AmxRole::Initiator),
                },
                effect: ManifestEffect::Allow(Allowance {
                    max_amount: None,
                    window: AllowanceWindow::PerDay,
                }),
                notes: Some("CLI test manifest".to_string()),
            }],
        }
    }

    #[test]
    fn build_space_directory_url_appends_segments() {
        let base = Url::parse("https://example.test/torii/").expect("base url");
        let url = build_space_directory_url(
            &base,
            &["v1", "space-directory", "uaids", "uaid:abc", "manifests"],
        )
        .expect("build url");
        assert_eq!(
            url.as_str(),
            "https://example.test/torii/v1/space-directory/uaids/uaid:abc/manifests"
        );
    }

    #[test]
    fn address_format_arg_serializes_expected_labels() {
        assert_eq!(AddressFormatArg::Ih58.as_query_value(), "ih58");
        assert_eq!(AddressFormatArg::Compressed.as_query_value(), "compressed");
    }

    #[test]
    fn manifest_status_arg_serializes_expected_labels() {
        assert_eq!(ManifestStatusArg::Active.as_query_value(), "active");
        assert_eq!(ManifestStatusArg::Inactive.as_query_value(), "inactive");
        assert_eq!(ManifestStatusArg::All.as_query_value(), "all");
    }

    #[test]
    fn write_json_response_creates_parent_directories() {
        let dir = tempdir().expect("tmpdir");
        let path = dir.path().join("nested").join("bindings.json");
        let payload = norito::json!({"uaid": "uaid:test", "dataspaces": []});
        write_json_response(&path, &payload).expect("write json");
        let written = std::fs::read_to_string(&path).expect("read json");
        assert!(
            written.contains("\"dataspaces\""),
            "json output should include the dataspaces array"
        );
    }

    struct TestContext {
        cfg: Config,
        lines: Vec<String>,
        i18n: Localizer,
    }

    impl TestContext {
        fn new() -> Self {
            let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
            let account_id =
                AccountId::new("wonderland".parse().unwrap(), key_pair.public_key().clone());
            let cfg = Config {
                chain: ChainId::from("00000000-0000-0000-0000-000000000000"),
                account: account_id,
                key_pair,
                basic_auth: None,
                torii_api_url: Url::parse("http://127.0.0.1/").unwrap(),
                torii_api_version: iroha::config::default_torii_api_version(),
                torii_api_min_proof_version: iroha::config::DEFAULT_TORII_API_MIN_PROOF_VERSION
                    .to_string(),
                torii_request_timeout: iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT,
                transaction_ttl: iroha::config::DEFAULT_TRANSACTION_TIME_TO_LIVE,
                transaction_status_timeout: iroha::config::DEFAULT_TRANSACTION_STATUS_TIMEOUT,
                transaction_add_nonce: iroha::config::DEFAULT_TRANSACTION_NONCE,
                connect_queue_root: iroha::config::default_connect_queue_root(),
                sorafs_alias_cache: crate::config_utils::default_alias_cache_policy(),
                sorafs_anonymity_policy: crate::config_utils::default_anonymity_policy(),
                sorafs_rollout_phase: crate::config_utils::default_rollout_phase(),
            };
            Self {
                cfg,
                lines: Vec::new(),
                i18n: Localizer::new(Bundle::Cli, Language::English),
            }
        }
    }

    impl RunContext for TestContext {
        fn config(&self) -> &Config {
            &self.cfg
        }

        fn transaction_metadata(&self) -> Option<&Metadata> {
            None
        }

        fn input_instructions(&self) -> bool {
            false
        }

        fn output_instructions(&self) -> bool {
            false
        }

        fn i18n(&self) -> &Localizer {
            &self.i18n
        }

        fn print_data<T>(&mut self, _data: &T) -> Result<()>
        where
            T: JsonSerialize + ?Sized,
        {
            Ok(())
        }

        fn println(&mut self, data: impl std::fmt::Display) -> Result<()> {
            self.lines.push(data.to_string());
            Ok(())
        }
    }
}
