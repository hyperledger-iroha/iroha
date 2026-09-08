//! Local production assembly and owner authorization of the existing reset inventory.

use super::*;
use iroha_crypto::{KeyPair, PrivateKey, Signature};
use norito::codec::Encode as _;
use zeroize::Zeroizing;

/// Actual local inputs whose derived identities are written into the inventory.
#[derive(clap::Args, Debug)]
pub(super) struct LocalInputs {
    #[arg(long, value_name = "PATH")]
    runtime_client_config: PathBuf,
    #[arg(long, value_name = "PATH", num_args = 4)]
    validator_client_config: Vec<PathBuf>,
    #[arg(long, value_name = "PATH")]
    onboarding_token: PathBuf,
    #[arg(long, value_name = "DIR")]
    inrou_stage_dir: PathBuf,
    /// Four exact local systemd units in validator order.
    #[arg(long, value_name = "PATH", num_args = 4)]
    validator_unit: Vec<PathBuf>,
    /// Exact local edge systemd unit.
    #[arg(long, value_name = "PATH")]
    edge_unit: PathBuf,
    /// Independently approved known-hosts; draft host pins must already match.
    #[arg(long, value_name = "PATH")]
    known_hosts: PathBuf,
}

#[derive(clap::Args, Debug)]
pub(super) struct Assemble {
    /// Existing InventoryV1 layout with explicit approved topology, occupancy and intent.
    /// Derived hashes, sizes, modes, source/stage identity and fingerprints are replaced.
    #[arg(long, value_name = "PATH")]
    inventory_draft: PathBuf,
    #[command(flatten)]
    local: LocalInputs,
    /// Fresh file in an existing owner-only directory; never overwritten.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
}

#[derive(clap::Args, Debug)]
pub(super) struct Authorize {
    /// Exact retained assembler output; these file bytes are signed without reformatting.
    #[arg(long, value_name = "PATH")]
    inventory: PathBuf,
    #[command(flatten)]
    local: LocalInputs,
    /// Separately trusted owner Ed25519 public-key file in existing TrustedKeyV1 format.
    #[arg(long, value_name = "PATH")]
    trusted_public_key: PathBuf,
    /// Inherited descriptor of a single owner-private regular file containing the key string.
    /// Key material is never accepted as an argument or printed.
    #[arg(long, value_name = "FD", value_parser = clap::value_parser!(u32).range(3..=65535))]
    signing_key_fd: u32,
    /// Fresh owner-private authorization file; never overwritten.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
}

pub(super) fn assemble(args: &Assemble) -> Result<()> {
    let input = pin_owner_private_file(&args.inventory_draft, "inventory draft")?;
    let bytes = pinned_bytes(&input, MAX_JSON_BYTES)?;
    let (mut inventory, _guard) = decode_inventory(&bytes, "inventory draft")?;
    derive_inventory(&mut inventory, &args.local)?;
    revalidate_pinned(&input, "inventory draft")?;
    write_new_private(&args.output, &assembled_inventory_bytes(&inventory)?)
}

pub(super) fn authorize(args: &Authorize) -> Result<()> {
    let input = pin_owner_private_file(&args.inventory, "retained inventory")?;
    let bytes = pinned_bytes(&input, MAX_JSON_BYTES)?;
    let (inventory, _guard) = decode_inventory(&bytes, "retained inventory")?;
    validate_inventory(&inventory)?;
    let mut derived = inventory.clone();
    derive_inventory(&mut derived, &args.local)?;
    if canonical_inventory_bytes(&derived)? != canonical_inventory_bytes(&inventory)? {
        return Err(eyre!(
            "retained inventory differs from actual local release inputs"
        ));
    }
    let trusted_input =
        pin_owner_private_file(&args.trusted_public_key, "trusted owner public key")?;
    let trusted: TrustedKeyV1 = json::from_slice(&pinned_bytes(&trusted_input, MAX_JSON_BYTES)?)
        .map_err(|_| eyre!("trusted owner public key is not exact V1 JSON"))?;
    let public_key = trusted_key(&trusted)?;
    // Load the authority key only after the complete explicit release input validation.
    let key = inherited_signing_key(args.signing_key_fd, &public_key)?;
    revalidate_pinned(&input, "retained inventory")?;
    revalidate_pinned(&trusted_input, "trusted owner public key")?;
    let envelope = sign_inventory(&inventory, &bytes, &trusted, &key, now_unix_ms()?)?;
    revalidate_pinned(&input, "retained inventory")?;
    revalidate_pinned(&trusted_input, "trusted owner public key")?;
    write_new_private(&args.output, &canonical_bytes(&envelope)?)
}

fn canonical_bytes<T: JsonSerialize>(value: &T) -> Result<Vec<u8>> {
    let mut bytes = json::to_json(value)?.into_bytes();
    bytes.push(b'\n');
    if bytes.len() as u64 > MAX_JSON_BYTES {
        return Err(eyre!("release input exceeds the V1 JSON bound"));
    }
    Ok(bytes)
}

fn pinned_bytes(input: &PinnedInput, maximum: u64) -> Result<Zeroizing<Vec<u8>>> {
    if input.snapshot.len == 0 || input.snapshot.len > maximum {
        return Err(eyre!("release input is empty or exceeds its bound"));
    }
    let mut file = input.file.try_clone()?;
    file.rewind()?;
    Ok(Zeroizing::new(read_pinned_bytes(
        &input.path,
        "release input",
        file,
        &input.snapshot,
        maximum,
    )?))
}

fn derive_inventory(inventory: &mut InventoryV1, inputs: &LocalInputs) -> Result<()> {
    if inventory.validators.len() != 4
        || inventory.validator_clients.len() != 4
        || inputs.validator_client_config.len() != 4
        || inputs.validator_unit.len() != 4
    {
        return Err(eyre!(
            "assembly requires exactly four ordered validator inputs"
        ));
    }
    if inventory.schema != INVENTORY_SCHEMA_V1 {
        return Err(eyre!(
            "inventory draft must use the existing executor inventory V1 schema"
        ));
    }
    // Reject an impossible signed execution plan before source/artifact scans or custody reads.
    validate_timeout_policy(inventory)?;
    let (source, source_bytes) = read_json::<SourceManifestV1>(
        Path::new(&inventory.revision.source_manifest_path),
        "source manifest",
    )?;
    inventory.revision.branch = source.branch;
    inventory.revision.commit = source.head_commit_sha1;
    inventory.revision.tree = source.head_tree_sha1;
    inventory.revision.cargo_lock_sha256 = source.cargo_lock_sha256;
    inventory.revision.source_closure_sha256 = source.closure_sha256;
    inventory.revision.source_manifest_sha256 = sha256_hex(&source_bytes);
    inventory.revision.build_id = inventory.revision.commit.clone();
    inventory.revision.target = BUILD_TARGET.to_owned();
    inventory.revision.profile = BUILD_PROFILE.to_owned();
    validate_revision(&inventory.revision)?;
    validate_source_closure(&inventory.revision)?;
    if iroha_core::release_identity::source_commit() != Some(inventory.revision.commit.as_str()) {
        return Err(eyre!(
            "compiled Core source differs from the release revision"
        ));
    }
    for artifact in inventory
        .validators
        .iter_mut()
        .flat_map(|v| v.artifacts.iter_mut())
        .chain(inventory.edge.artifacts.iter_mut())
    {
        let path = Path::new(&artifact.local_path);
        let (mut file, snapshot) = open_pinned_regular(path, "release artifact")?;
        let (mode, maximum) = artifact_role_policy(&artifact.role)?;
        if snapshot.len == 0 || snapshot.len > maximum {
            return Err(eyre!("release artifact is empty or exceeds its role bound"));
        }
        #[cfg(unix)]
        if snapshot.uid != rustix::process::geteuid().as_raw()
            || snapshot.mode & 0o7777 != u32::from(mode)
        {
            return Err(eyre!(
                "release artifact does not have its required owner and role mode"
            ));
        }
        artifact.sha256 = sha256_reader(&mut file, path)?;
        ensure_pinned_unchanged(path, "release artifact", &file, &snapshot)?;
        artifact.size = snapshot.len;
        artifact.mode = mode;
        artifact.source_commit = inventory.revision.commit.clone();
        artifact.target = BUILD_TARGET.to_owned();
    }
    for (validator, path) in inventory.validators.iter_mut().zip(&inputs.validator_unit) {
        validator.systemd_unit_sha256 = unit_hash(path)?;
    }
    inventory.edge.systemd_unit_sha256 = unit_hash(&inputs.edge_unit)?;
    derive_validator_identities(inventory)?;
    derive_runtime_stage(inventory, inputs)?;
    inventory.artifact_closure_sha256 = artifact_closure_sha256(inventory);
    validate_inventory(inventory)?;
    validate_shared_validator_closure(inventory)?;
    let pinned = validate_artifact_files(inventory)?;
    validate_genesis_hash_files(inventory, &pinned)?;
    validate_known_hosts(inventory, &inputs.known_hosts)?;
    validate_source_closure(&inventory.revision)?;
    for entry in &pinned {
        revalidate_pinned(&entry.input, "release artifact")?;
    }
    Ok(())
}

fn unit_hash(path: &Path) -> Result<String> {
    let (mut file, snapshot) = open_pinned_regular(path, "systemd unit")?;
    if snapshot.len == 0 || snapshot.len > 1024 * 1024 {
        return Err(eyre!("systemd unit is empty or oversized"));
    }
    let hash = sha256_reader(&mut file, path)?;
    ensure_pinned_unchanged(path, "systemd unit", &file, &snapshot)?;
    Ok(hash)
}

#[cfg(unix)]
fn derive_validator_identities(inventory: &mut InventoryV1) -> Result<()> {
    use iroha_config::{
        base::toml::{MAX_TOML_SOURCE_BYTES, TomlSource},
        parameters::actual,
    };
    let genesis_path = &artifact(&inventory.validators[0].artifacts, "genesis")?.local_path;
    let (file, snapshot) = open_pinned_regular(Path::new(genesis_path), "signed genesis")?;
    let genesis = PinnedInput {
        path: PathBuf::from(genesis_path),
        file,
        snapshot,
    };
    let genesis_bytes = pinned_bytes(&genesis, 64 * 1024 * 1024)?;
    for (validator, client) in inventory
        .validators
        .iter_mut()
        .zip(&inventory.validator_clients)
    {
        let path = PathBuf::from(&artifact(&validator.artifacts, "config")?.local_path);
        let (file, snapshot) = open_pinned_regular(&path, "validator config")?;
        let input = PinnedInput {
            path: path.clone(),
            file,
            snapshot,
        };
        let bytes = pinned_bytes(&input, MAX_TOML_SOURCE_BYTES as u64)?;
        validate_validator_genesis_config(
            &bytes,
            Path::new(&artifact(&validator.artifacts, "genesis")?.remote_path),
            &inventory.next_genesis_hash,
        )?;
        let text =
            std::str::from_utf8(&bytes).map_err(|_| eyre!("validator config is not UTF-8"))?;
        let table: toml::Table =
            toml::from_str(text).map_err(|_| eyre!("validator config is not TOML"))?;
        let config = actual::Root::from_toml_source(TomlSource::new_sensitive(
            path,
            table,
            crate::soracloud::zeroize_taira_toml_table,
        ))
        .map_err(|_| eyre!("validator config failed current typed admission"))?;
        revalidate_pinned(&input, "validator config")?;
        if config.common.chain.to_string() != inventory.chain_id
            || config.common.peer.id.to_string() != client.peer_id
        {
            return Err(eyre!(
                "validator config differs from the explicit chain/peer identity"
            ));
        }
        let (hash, metadata) = iroha_core::release_identity::genesis_identity(
            &genesis_bytes,
            &config.genesis.public_key,
        )?;
        if hash.to_string() != inventory.next_genesis_hash
            || Hash::from(config.genesis.expected_hash) != hash
        {
            return Err(eyre!(
                "actual signed genesis differs from the explicit next genesis hash"
            ));
        }
        validate_taira_genesis_mode(metadata.mode)?;
        let shared = config
            .sumeragi
            .v2_config(
                std::time::Duration::from_millis(metadata.block_cadence_ms.get()),
                metadata.mode.into(),
            )
            .map_err(|_| eyre!("validator signed consensus configuration is invalid"))?;
        shared
            .validate_ingress_roster_capacity(4)
            .map_err(|_| eyre!("validator cannot admit the four-member roster"))?;
        validator.node_fingerprint = Hash::new(config.common.peer.id.encode()).to_string();
        validator.build_fingerprint = iroha_core::release_identity::build_fingerprint().to_string();
        validator.config_fingerprint = shared.fingerprint().to_string();
    }
    revalidate_pinned(&genesis, "signed genesis")?;
    Ok(())
}

#[cfg(not(unix))]
fn derive_validator_identities(_: &mut InventoryV1) -> Result<()> {
    Err(eyre!("public reset input assembly requires Unix"))
}

fn derive_runtime_stage(inventory: &mut InventoryV1, inputs: &LocalInputs) -> Result<()> {
    let runtime = pin_owner_private_file(&inputs.runtime_client_config, "runtime client config")?;
    let token = pin_owner_private_file(&inputs.onboarding_token, "onboarding token")?;
    let clients = inputs
        .validator_client_config
        .iter()
        .map(|p| pin_owner_private_file(p, "validator client config"))
        .collect::<Result<Vec<_>>>()?;
    host::validate_validator_client_inputs(&clients, inventory)?;
    let config = host::load_client_config_from_pinned(&runtime, "runtime client config")?;
    if config.torii_api_url.as_str() != format!("{PUBLIC_ROOT}/")
        || config.account.to_string() != inventory.canary_onboarding_request.account_id
    {
        return Err(eyre!(
            "runtime client config does not bind the explicit public Taira canary"
        ));
    }
    let (stage_hash, stage_bytes, files, fixed) =
        host::pin_stage_tree(&inputs.inrou_stage_dir, None)?;
    let identity = crate::soracloud::load_taira_inrou_stage_identity(
        &config,
        &inputs.inrou_stage_dir,
        crate::taira::InrouCanaryMode::Deploy,
    )?;
    if host::revalidate_stage_files(&inputs.inrou_stage_dir, &files, None)?
        != (stage_hash.clone(), stage_bytes)
    {
        return Err(eyre!("Inrou stage changed during inventory assembly"));
    }
    inventory.runtime_client_config_sha256 =
        host::hash_pinned_input(&runtime, "runtime config", None)?;
    inventory.onboarding_token_sha256 = host::hash_pinned_input(&token, "onboarding token", None)?;
    inventory.validator_client_configs_sha256 =
        host::validator_config_closure_sha256(&clients, None)?;
    inventory.inrou_stage_tree_sha256 = stage_hash.clone();
    let file_hash = |path: &str| {
        fixed
            .get(path)
            .cloned()
            .ok_or_else(|| eyre!("Inrou stage omits a mandatory fixed file"))
    };
    inventory.inrou_canary = InrouCanaryV1 {
        public_root: PUBLIC_ROOT.to_owned(),
        replicas: 4,
        service_name: identity.service_name,
        service_version: identity.service_version,
        route_host: identity.route_host,
        route_path_prefix: identity.route_path_prefix,
        healthcheck_path: identity.healthcheck_path,
        bundle_hash: identity.bundle_hash,
        bundle_content_cid: identity.bundle_content_cid,
        bundle_manifest_digest_hex: identity.bundle_manifest_digest_hex,
        guest_content_cid: identity.guest_content_cid,
        guest_manifest_digest_hex: identity.guest_manifest_digest_hex,
        discovery_payload_dir: identity.discovery_payload_dir,
        discovery_manifest_file: "manifests/discovery.to".to_owned(),
        discovery_document_hash: identity.discovery_document_hash,
        discovery_content_cid: identity.discovery_content_cid,
        discovery_manifest_digest_hex: identity.discovery_manifest_digest_hex,
        public_discovery_url: identity.public_discovery_url,
        public_discovery_cid_host_url: identity.public_discovery_cid_host_url,
        deployment_bundle_hash: identity.deployment_bundle_hash,
        container_manifest_hash: identity.container_manifest_hash,
        service_manifest_hash: identity.service_manifest_hash,
        placement_targets: identity.placement_targets,
        stage_tree_sha256: stage_hash,
        stage_bytes,
        receipt_sha256: file_hash("receipt.json")?,
        container_sha256: file_hash("container.json")?,
        service_sha256: file_hash("service.json")?,
        bundle_payload_sha256: file_hash("payloads/bundle.bin")?,
        bundle_manifest_sha256: file_hash("manifests/bundle.to")?,
        guest_manifest_sha256: file_hash("manifests/aarch64.to")?,
        discovery_document_sha256: file_hash("payloads/discovery/index.json")?,
        discovery_manifest_sha256: file_hash("manifests/discovery.to")?,
    };
    Ok(())
}

fn validate_taira_genesis_mode(
    mode: iroha::data_model::parameter::system::SumeragiConsensusMode,
) -> Result<()> {
    if mode != iroha::data_model::parameter::system::SumeragiConsensusMode::Npos {
        return Err(eyre!("canonical Taira requires signed NPoS genesis"));
    }
    Ok(())
}

fn trusted_key(trusted: &TrustedKeyV1) -> Result<PublicKey> {
    if trusted.schema != TRUSTED_KEY_SCHEMA_V1 || trusted.algorithm != "ed25519" {
        return Err(eyre!(
            "owner public key must use the trusted Ed25519 V1 schema"
        ));
    }
    let key = PublicKey::from_str(&trusted.public_key)
        .map_err(|_| eyre!("invalid trusted owner public key"))?;
    if key.try_algorithm()? != Algorithm::Ed25519 || key.to_string() != trusted.public_key {
        return Err(eyre!("trusted owner key must be canonical Ed25519"));
    }
    Ok(key)
}

fn sign_inventory(
    inventory: &InventoryV1,
    bytes: &[u8],
    trusted: &TrustedKeyV1,
    key: &KeyPair,
    now: u64,
) -> Result<AuthorizationEnvelopeV1> {
    if key.public_key() != &trusted_key(trusted)? {
        return Err(eyre!(
            "signing key does not match the independently trusted owner key"
        ));
    }
    let claims = AuthorizationClaimsV1 {
        action: "reset_and_deploy".to_owned(),
        deployment_id: inventory.deployment_id.clone(),
        inventory_sha256: sha256_hex(bytes),
        artifact_closure_sha256: inventory.artifact_closure_sha256.clone(),
        runtime_client_config_sha256: inventory.runtime_client_config_sha256.clone(),
        onboarding_token_sha256: inventory.onboarding_token_sha256.clone(),
        validator_client_configs_sha256: inventory.validator_client_configs_sha256.clone(),
        inrou_stage_tree_sha256: inventory.inrou_stage_tree_sha256.clone(),
        faucet_policy: inventory.faucet_policy.clone(),
        fee_intent: inventory.fee_intent.clone(),
        authorization_nonce: inventory.authorization_nonce.clone(),
        issued_at_unix_ms: now,
        not_before_unix_ms: now,
        expires_at_unix_ms: now
            .checked_add(MAX_AUTHORIZATION_LIFETIME_MS)
            .ok_or_else(|| eyre!("authorization expiry overflow"))?,
        execution_expires_at_unix_ms: now
            .checked_add(execution_lifetime_ms(inventory)?)
            .ok_or_else(|| eyre!("execution expiry overflow"))?,
    };
    let signature = Signature::try_new(key.private_key(), &authorization_message(&claims)?)
        .map_err(|_| eyre!("owner authorization signing failed"))?;
    let envelope = AuthorizationEnvelopeV1 {
        schema: AUTHORIZATION_SCHEMA_V1.to_owned(),
        claims,
        signature_hex: hex::encode(signature.payload()),
    };
    verify_authorization(inventory, &sha256_hex(bytes), &envelope, trusted, now)?;
    Ok(envelope)
}

#[cfg(unix)]
fn inherited_signing_key(fd: u32, public_key: &PublicKey) -> Result<KeyPair> {
    let bytes = crate::client_config::read_inherited_private_file(fd, 512, "owner signing key")?;
    let text =
        std::str::from_utf8(&bytes).map_err(|_| eyre!("invalid owner signing key encoding"))?;
    let private = PrivateKey::from_str(text.strip_suffix('\n').unwrap_or(text))
        .map_err(|_| eyre!("invalid owner signing key encoding"))?;
    KeyPair::new(public_key.clone(), private)
        .map_err(|_| eyre!("signing key differs from the independently trusted owner key"))
}

#[cfg(not(unix))]
fn inherited_signing_key(_: u32, _: &PublicKey) -> Result<KeyPair> {
    Err(eyre!("owner signing requires Unix inherited descriptors"))
}

pub(super) fn write_new_private(path: &Path, bytes: &[u8]) -> Result<()> {
    validate_absolute_normal_path(path, "release output")?;
    let parent = path
        .parent()
        .ok_or_else(|| eyre!("release output has no parent"))?;
    validate_owner_private_dir(parent, "release output directory")?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    validate_owner_private_dir(parent, "release output directory")?;
    temporary
        .persist_noclobber(path)
        .map_err(|_| eyre!("cannot publish fresh release output"))?;
    File::open(parent)?.sync_all()?;
    let pinned = pin_owner_private_file(path, "release output")?;
    if zeroize::Zeroizing::new(pinned_bytes(&pinned, MAX_JSON_BYTES)?).as_slice() != bytes {
        return Err(eyre!(
            "published release output differs from the retained bytes"
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "taira_public_reset_inputs_tests.rs"]
mod tests;
