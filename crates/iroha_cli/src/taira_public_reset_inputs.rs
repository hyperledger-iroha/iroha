//! Local production assembly and owner authorization of the existing reset inventory.

use super::*;
use iroha_crypto::{KeyPair, PrivateKey, Signature};
use norito::codec::Encode as _;
use zeroize::Zeroizing;

#[path = "taira_public_reset_context_release.rs"]
mod context_release;
#[path = "taira_public_reset_context.rs"]
mod reset_context;
pub(super) use reset_context::{
    DerivedResetContext, ResetContextInputs, ResetTopologyIntentV1, decode_reset_topology_intent,
    derive_reset_context,
};

pub(super) fn topology_canary_request(bytes: &[u8]) -> Result<AccountOnboardingPlanRequestV1> {
    let (intent, _guard) = decode_reset_topology_intent(bytes)?;
    Ok(intent.canary_onboarding_request)
}

/// Actual local inputs whose derived identities are written into the inventory.
#[derive(clap::Args, Debug)]
pub(super) struct LocalInputs {
    /// Complete native bundle produced by prepare-public-inputs; no handwritten identity fields.
    #[arg(long, value_name = "DIR")]
    public_inputs: PathBuf,
    #[arg(long, value_name = "PATH")]
    runtime_client_config: PathBuf,
    #[arg(long, value_name = "PATH", num_args = 4)]
    validator_client_config: Vec<PathBuf>,
    #[arg(long, value_name = "PATH")]
    onboarding_token: PathBuf,
    /// Dedicated runtime key whose public identity every validator explicitly allows.
    #[arg(long, value_name = "PATH")]
    validator_operator_key: PathBuf,
    #[arg(long, value_name = "DIR")]
    inrou_stage_dir: Option<PathBuf>,
    /// Four exact local systemd units in validator order.
    #[arg(long, value_name = "PATH", num_args = 4)]
    validator_unit: Vec<PathBuf>,
    /// Native public request and seat map produced by prepare-beacon-inputs.
    #[arg(long, value_name = "PATH")]
    beacon_inputs: PathBuf,
    /// Four pre-rendered FD200 units using --config-file beacon.toml, in validator order.
    #[arg(long, value_name = "PATH", num_args = 4)]
    beacon_validator_unit: Vec<PathBuf>,
    /// Exact local edge systemd unit.
    #[arg(long, value_name = "PATH")]
    edge_unit: PathBuf,
    /// Independently approved known-hosts; draft host pins must already match.
    #[arg(long, value_name = "PATH")]
    known_hosts: PathBuf,
}

#[derive(clap::Args, Debug)]
pub(super) struct Assemble {
    /// Closed topology and authority intent; native-derived pins and generated plans are forbidden.
    #[arg(long, value_name = "PATH")]
    intent: PathBuf,
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

/// Derive public beacon request and renderer seat paths from authenticated genesis.
#[derive(clap::Args, Debug)]
pub(super) struct PrepareBeaconInputs {
    #[arg(long, value_name = "PATH")]
    intent: PathBuf,
    #[arg(long, value_name = "DIR")]
    public_inputs: PathBuf,
    /// New owner-private public result; never replaced.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
}

pub(super) fn prepare_beacon_inputs(args: &PrepareBeaconInputs) -> Result<()> {
    let input = pin_owner_private_file(&args.intent, "reset topology intent")?;
    let (intent, _guard) = decode_reset_topology_intent(&pinned_bytes(&input, MAX_JSON_BYTES)?)?;
    let public = public_inputs::load(&args.public_inputs)?;
    let read = |path: &Path, label| -> Result<Vec<u8>> {
        let (file, snapshot) = open_pinned_regular(path, label)?;
        read_pinned_bytes(path, label, file, &snapshot, MAX_JSON_BYTES)
    };
    let wire = read(
        &args.public_inputs.join("genesis.signed.nrt"),
        "prepared signed genesis",
    )?;
    if sha256_hex(&wire) != public.signed_genesis_sha256 {
        return Err(eyre!(
            "prepared beacon genesis changed after native public-input validation"
        ));
    }
    let manifest = read(
        &args.public_inputs.join("genesis.json"),
        "public raw genesis manifest",
    )?;
    if sha256_hex(&manifest) != public.raw_manifest_sha256 {
        return Err(eyre!(
            "public raw genesis manifest changed after bundle validation"
        ));
    }
    if public.canary_onboarding_request != intent.canary_onboarding_request {
        return Err(eyre!("topology canary differs from native public bundle"));
    }
    let slots = intent
        .validators
        .iter()
        .map(|v| host::beacon::BeaconValidatorSlot { slug: &v.slug })
        .collect::<Vec<_>>();
    let prepared = host::beacon::prepare_public_beacon_inputs_from_slots(
        &wire,
        &manifest,
        &public.genesis_public_key,
        public.genesis_hash.parse()?,
        &intent.authorization_nonce,
        &slots,
        &intent.validator_clients,
    )?;
    revalidate_pinned(&input, "reset topology intent")?;
    write_new_private(&args.output, &canonical_bytes(&prepared)?)
}

pub(super) fn assemble(args: &Assemble) -> Result<()> {
    let input = pin_owner_private_file(&args.intent, "reset topology intent")?;
    let (intent, _guard) = decode_reset_topology_intent(&pinned_bytes(&input, MAX_JSON_BYTES)?)?;
    let (inventory, context) = derive_inventory_from_intent(&intent, &args.local)?;
    context.revalidate()?;
    revalidate_pinned(&input, "reset topology intent")?;
    write_new_private(&args.output, &assembled_inventory_bytes(&inventory)?)
}

pub(super) fn authorize(args: &Authorize) -> Result<()> {
    let input = pin_owner_private_file(&args.inventory, "retained inventory")?;
    let bytes = pinned_bytes(&input, MAX_JSON_BYTES)?;
    let (inventory, _guard) = decode_inventory(&bytes, "retained inventory")?;
    validate_inventory(&inventory)?;
    let intent = ResetTopologyIntentV1::from(&inventory);
    let (derived, context) = derive_inventory_from_intent(&intent, &args.local)?;
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
    context.revalidate()?;
    let key = inherited_signing_key(args.signing_key_fd, &public_key)?;
    revalidate_pinned(&input, "retained inventory")?;
    revalidate_pinned(&trusted_input, "trusted owner public key")?;
    let envelope = sign_inventory(&inventory, &bytes, &trusted, &key, now_unix_ms()?)?;
    revalidate_pinned(&input, "retained inventory")?;
    revalidate_pinned(&trusted_input, "trusted owner public key")?;
    context.revalidate()?;
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

#[cfg(test)]
fn derive_inventory(inventory: &mut InventoryV1, inputs: &LocalInputs) -> Result<()> {
    validate_timeout_policy(inventory)?;
    let intent = ResetTopologyIntentV1::from(&*inventory);
    let (derived, context) = derive_inventory_from_intent(&intent, inputs)?;
    context.revalidate()?;
    *inventory = derived;
    Ok(())
}

fn derive_inventory_from_intent(
    intent: &ResetTopologyIntentV1,
    inputs: &LocalInputs,
) -> Result<(InventoryV1, DerivedResetContext)> {
    if inputs.beacon_validator_unit.len() != 4 {
        return Err(eyre!("assembly requires four ordered beacon units"));
    }
    let context = derive_reset_context(intent, &ResetContextInputs::from(inputs))?;
    let beacon = host::beacon::load_plan(
        &context.validators,
        &inputs.beacon_inputs,
        &inputs.public_inputs.join("genesis.json"),
        &inputs.beacon_validator_unit,
        &context.public_inputs.genesis_public_key,
    )?;
    let mut inventory = context.build_inventory(beacon)?;
    // Retain the existing signed-genesis/nonce/seat rederivation gate, not merely plan decoding.
    host::beacon::derive_plan(
        &mut inventory,
        &inputs.beacon_inputs,
        &inputs.public_inputs.join("genesis.json"),
        &inputs.beacon_validator_unit,
        &context.public_inputs.genesis_public_key,
    )?;
    let operator_key =
        host::pin_validator_operator_key(&inputs.validator_operator_key, &inventory)?;
    validate_inventory(&inventory)?;
    validate_shared_validator_closure(&inventory)?;
    let pinned = validate_artifact_files(&inventory)?;
    validate_genesis_hash_files(&inventory, &pinned)?;
    let known_hosts = validate_known_hosts(&inventory, &inputs.known_hosts)?;
    validate_source_closure(&inventory.revision)?;
    for entry in &pinned {
        revalidate_pinned(&entry.input, "release artifact")?;
    }
    revalidate_pinned(&operator_key, "validator operator key")?;
    revalidate_pinned(&known_hosts, "OpenSSH known-hosts")?;
    context.revalidate()?;
    Ok((inventory, context))
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

fn validate_candidate_inrou_scope(
    scope: QualificationScopeV1,
    slug: &str,
    inrou: &iroha_config::parameters::actual::SoracloudRuntimeInrou,
) -> Result<()> {
    match scope {
        QualificationScopeV1::CoreTestnet if !inrou.enabled => Ok(()),
        QualificationScopeV1::CoreTestnet => Err(eyre!(
            "core_testnet requires the candidate Inrou runtime to be disabled"
        )),
        QualificationScopeV1::FullInrou => host::stopped_runtime::validate_config_slot(slug, inrou),
    }
}

fn validate_validator_pin_fee_asset(
    configured: &iroha::data_model::asset::AssetDefinitionId,
    faucet_asset: &str,
) -> Result<()> {
    let expected = faucet_asset
        .parse::<iroha::data_model::asset::AssetDefinitionId>()
        .map_err(|_| eyre!("inventory faucet asset is not a valid asset definition identity"))?;
    if configured != &expected {
        return Err(eyre!(
            "validator SoraFS pin fee asset differs from the inventory faucet asset"
        ));
    }
    Ok(())
}

pub(super) fn validate_taira_genesis_mode(
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
        qualification_scope: inventory.qualification_scope,
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
