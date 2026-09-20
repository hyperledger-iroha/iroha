//! Native producer for the reset's typed epoch supervisor plan and observation trust.
//!
//! The topology contains intent only. Shared native context derivation loads held
//! credentials and release artifacts before a plan exists. Only derived public
//! identities and hashes enter the output; final assembly independently rederives them.

use super::super::{MaintenanceAdminIdentityV1, inputs, public_inputs};
use super::epoch_supervisor::{
    EpochSupervisorPlanV1, JOURNAL_DIR, KagamiV1, NativePolicyV1, OngoingIntentV1,
    PriorEpochSupervisorV1, STATE_ROOT, SeedCustodyV1, SeedV1, UNIT_NAME,
};
use super::*;
use iroha_primitives::numeric::Quantity;

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum OngoingAuthorization {
    UntilStopped,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
enum PriorState {
    Absent,
    Running,
    Stopped,
}
impl PriorState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Running => "running",
            Self::Stopped => "stopped",
        }
    }
}

/// Derive a complete typed plan from actual native local inputs and explicit owner intent.
#[derive(clap::Args, Debug)]
pub(in super::super) struct PrepareEpochSupervisorPlan {
    #[arg(long, value_name = "PATH")]
    intent: PathBuf,
    #[command(flatten)]
    local: inputs::ResetContextInputs,
    #[arg(long)]
    host_slug: String,
    /// Required explicit ongoing authority; never inferred from the reset's finite lease.
    #[arg(long, value_enum)]
    authorization: OngoingAuthorization,
    #[arg(long)]
    payment_asset: AssetDefinitionId,
    #[arg(long)]
    transaction_fee_maximum: Quantity,
    #[arg(long, value_parser=clap::value_parser!(u64).range(1..))]
    first_epoch: u64,
    #[arg(long, value_parser=clap::value_parser!(u16).range(2..=256))]
    batch_epochs: u16,
    #[arg(long, value_parser=clap::value_parser!(u64).range(1..))]
    operation_timeout_ms: u64,
    #[arg(long, value_parser=clap::value_parser!(u64).range(1..))]
    provision_timeout_ms: u64,
    #[arg(long, value_parser=clap::value_parser!(u64).range(1..))]
    timeout_ms: u64,
    /// Four original source paths in sorted native PeerId order. No seed file is opened here.
    #[arg(long, value_name = "PATH", num_args = 4, required = true)]
    epoch_seed_source: Vec<PathBuf>,
    #[arg(long, value_enum)]
    prior_state: PriorState,
    /// Exact immediate predecessor plan, required for running/stopped and forbidden for absent.
    #[arg(long, value_name = "PATH")]
    prior_plan: Option<PathBuf>,
    /// Fresh directory under an existing owner-only parent; never overwrite a prior output.
    #[arg(long, value_name = "DIR")]
    output_dir: PathBuf,
}

fn require(ok: bool, message: &'static str) -> Result<()> {
    if ok { Ok(()) } else { Err(eyre!(message)) }
}
fn public_input(path: &Path, label: &str) -> Result<(super::super::PinnedInput, Vec<u8>)> {
    let (file, snapshot) = open_pinned_regular(path, label)?;
    require(
        snapshot.len > 0 && snapshot.len <= super::super::MAX_JSON_BYTES,
        "public supervisor input exceeds its bound",
    )?;
    let bytes = read_pinned_bytes(
        path,
        label,
        file.try_clone()?,
        &snapshot,
        super::super::MAX_JSON_BYTES,
    )?;
    Ok((
        super::super::PinnedInput {
            path: path.into(),
            file,
            snapshot,
        },
        bytes,
    ))
}

fn validate_public_administrator(
    admin: &MaintenanceAdminIdentityV1,
    public: &public_inputs::PublicInputsV1,
) -> Result<()> {
    let key: iroha_crypto::PublicKey = admin.public_key.parse()?;
    let account = AccountId::parse_encoded(&admin.account_id)?;
    require(
        key.to_string() == admin.public_key
            && key != public.canary_public_key
            && key.algorithm() == iroha_crypto::Algorithm::Ed25519
            && AccountId::new(key) == account
            && account.to_string() == admin.account_id
            && admin.network_id == public.network_id.to_string()
            && admin.genesis_hash == public.genesis_hash
            && admin.chain_discriminant == super::super::CHAIN_DISCRIMINANT,
        "public maintenance administrator differs from canonical signer or authenticated candidate genesis",
    )
}

/// Borrowed, non-serializable plan inputs. Production constructs this only from
/// the shared native derivation; no CLI field accepts these computed identities.
struct PlanContext<'a> {
    revision: &'a super::super::RevisionV1,
    validators: &'a [super::super::ValidatorV1],
    validator_clients: &'a [super::super::ValidatorClientV1],
    operator_public_key: &'a str,
    maintenance_admin_identity: &'a MaintenanceAdminIdentityV1,
    maintenance_admin_config_sha256: &'a str,
    http_operator_key_sha256: &'a str,
    public: &'a public_inputs::PublicInputsV1,
    observation_trust_bytes: &'a [u8],
}

#[cfg(unix)]
fn build_plan(
    args: &PrepareEpochSupervisorPlan,
    context: &PlanContext<'_>,
    prior_bytes: Option<Vec<u8>>,
) -> Result<EpochSupervisorPlanV1> {
    let public = context.public;
    let trust_bytes = context.observation_trust_bytes.to_vec();
    validate_public_administrator(context.maintenance_admin_identity, public)?;
    require(
        (args.prior_state == PriorState::Absent) == prior_bytes.is_none(),
        "explicit predecessor state requires exact predecessor bytes, or absence",
    )?;
    require(
        args.epoch_seed_source.len() == 4,
        "exactly four original seed source paths are required",
    )?;
    let selected = context
        .validators
        .iter()
        .find(|row| row.slug == args.host_slug)
        .ok_or_else(|| eyre!("selected supervisor host is not a reset validator"))?;
    let cli = artifact(&selected.artifacts, "iroha_cli")?;
    let kagami = artifact(&selected.artifacts, "kagami")?;
    let release = Path::new(&selected.service_root)
        .join("releases")
        .join(&context.revision.commit);
    require(
        Path::new(&cli.remote_path) == release.join("bin/iroha")
            && Path::new(&kagami.remote_path) == release.join("bin/kagami"),
        "supervisor native paths differ from the selected reset release",
    )?;
    let administrator = AccountId::parse_encoded(&context.maintenance_admin_identity.account_id)?;
    let operator: iroha_crypto::PublicKey = context.operator_public_key.parse()?;
    require(
        operator.to_string() != context.maintenance_admin_identity.public_key,
        "maintenance administrator must be separate from the HTTP operator",
    )?;
    for client in context.validator_clients {
        require(
            AccountId::parse_encoded(&client.account_id)? != administrator,
            "maintenance administrator must be separate from validator clients",
        )?;
    }
    let policy = NativePolicyV1 {
        schema_version: 1,
        intent: OngoingIntentV1 {
            authorization: match args.authorization {
                OngoingAuthorization::UntilStopped => "until_stopped",
            }
            .into(),
            network_id: public.network_id,
            administrator,
            payment_asset: args.payment_asset.clone(),
            transaction_fee_maximum: args.transaction_fee_maximum.clone(),
            first_epoch: args.first_epoch,
            batch_epochs: u64::from(args.batch_epochs),
            operation_timeout_ms: args.operation_timeout_ms,
        },
        release_source_commit: context.revision.commit.clone(),
        iroha_sha256: cli.sha256.clone(),
        kagami: KagamiV1 {
            path: kagami.remote_path.clone(),
            sha256: kagami.sha256.clone(),
        },
        observation_trust_sha256: sha256_hex(&trust_bytes),
        provision_timeout_ms: args.provision_timeout_ms,
    };
    let policy_bytes = json::to_vec(&policy)?;
    let policy_sha256 = sha256_hex(&policy_bytes);
    let generation = Path::new(STATE_ROOT)
        .join("generations")
        .join(&policy_sha256);
    let peers = context
        .validator_clients
        .iter()
        .map(|row| row.peer_id.parse::<PeerId>())
        .collect::<Result<BTreeSet<_>, _>>()?;
    require(
        peers.len() == 4,
        "original seed mapping requires four distinct typed peers",
    )?;
    let mut originals = Vec::with_capacity(4);
    let mut retained = Vec::with_capacity(4);
    for (index, (validator, path)) in peers.into_iter().zip(&args.epoch_seed_source).enumerate() {
        originals.push(SeedV1 {
            validator: validator.clone(),
            path: path
                .to_str()
                .ok_or_else(|| eyre!("original seed path is not UTF8"))?
                .into(),
        });
        retained.push(SeedV1 {
            validator,
            path: epoch_seed_custody::destination(public.network_id, index)?
                .to_string_lossy()
                .into_owned(),
        });
    }
    let custody_bytes = json::to_vec(&SeedCustodyV1 {
        schema_version: 1,
        seeds: retained,
    })?;
    let mut plan = EpochSupervisorPlanV1 {
        schema: "iroha.taira.public-reset.epoch-supervisor-plan.v1".into(),
        host_slug: args.host_slug.clone(),
        unit_name: UNIT_NAME.into(),
        state_root: STATE_ROOT.into(),
        journal_dir: JOURNAL_DIR.into(),
        release_source_commit: context.revision.commit.clone(),
        iroha_sha256: cli.sha256.clone(),
        kagami_sha256: kagami.sha256.clone(),
        policy_sha256,
        policy_bytes,
        observation_trust_sha256: sha256_hex(&trust_bytes),
        observation_trust_bytes: trust_bytes,
        custody_sha256: sha256_hex(&custody_bytes),
        custody_bytes,
        original_seed_sources: originals,
        unit_sha256: String::new(),
        unit_bytes: Vec::new(),
        admin_config_path: generation
            .join("administrator.toml")
            .to_string_lossy()
            .into_owned(),
        admin_config_sha256: context.maintenance_admin_config_sha256.to_owned(),
        http_operator_key_path: generation
            .join("http-operator.key")
            .to_string_lossy()
            .into_owned(),
        http_operator_key_sha256: context.http_operator_key_sha256.to_owned(),
        policy_path: generation
            .join("policy.json")
            .to_string_lossy()
            .into_owned(),
        trust_path: generation.join("trust.json").to_string_lossy().into_owned(),
        custody_path: generation
            .join("custody.json")
            .to_string_lossy()
            .into_owned(),
        timeout_ms: args.timeout_ms,
        prior_state: args.prior_state.as_str().into(),
        prior: prior_bytes.map(|bytes| PriorEpochSupervisorV1 {
            plan_sha256: sha256_hex(&bytes),
            plan_bytes: bytes,
        }),
    };
    plan.unit_bytes = epoch_supervisor::render_unit(&plan, &cli.remote_path)?;
    plan.unit_sha256 = sha256_hex(&plan.unit_bytes);
    epoch_supervisor::validate_plan_context(
        &plan,
        context.revision,
        context.validators,
        context.validator_clients,
        context.maintenance_admin_identity,
        context.maintenance_admin_config_sha256,
        &public.genesis_public_key,
    )?;
    Ok(plan)
}

#[cfg(unix)]
fn publish(directory: &Path, plan: &EpochSupervisorPlanV1) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    super::super::validate_absolute_normal_path(directory, "supervisor plan output")?;
    let parent = directory
        .parent()
        .ok_or_else(|| eyre!("supervisor output parent missing"))?;
    super::super::validate_owner_private_dir(parent, "supervisor output parent")?;
    // Files are fully serialized and validated before the first output mutation.
    let plan_bytes = json::to_vec(plan)?;
    let binding_bytes = json::to_vec(&epoch_generation::binding_from_plan(plan)?)?;
    let temporary = tempfile::Builder::new()
        .prefix(".epoch-reset-inputs-")
        .tempdir_in(parent)?;
    fs::set_permissions(temporary.path(), fs::Permissions::from_mode(0o700))?;
    inputs::write_new_private(&temporary.path().join("supervisor-plan.json"), &plan_bytes)?;
    inputs::write_new_private(
        &temporary.path().join("supervisor-binding.json"),
        &binding_bytes,
    )?;
    inputs::write_new_private(
        &temporary.path().join("observation-trust.json"),
        &plan.observation_trust_bytes,
    )?;
    File::open(temporary.path())?.sync_all()?;
    super::super::validate_owner_private_dir(parent, "supervisor output parent")?;
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        temporary.path(),
        rustix::fs::CWD,
        directory,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(|_| eyre!("cannot atomically publish fresh supervisor input bundle"))?;
    File::open(parent)?.sync_all()?;
    super::super::validate_owner_private_dir(directory, "published supervisor input bundle")?;
    Ok(())
}

/// Derive private/native inputs locally and publish only the unsigned public closure.
pub(in super::super) fn prepare(args: &PrepareEpochSupervisorPlan) -> Result<()> {
    #[cfg(not(unix))]
    {
        let _ = args;
        return Err(eyre!("supervisor plan publication requires Unix custody"));
    }
    #[cfg(unix)]
    {
        let _guard = ChainDiscriminantGuard::enter(super::super::CHAIN_DISCRIMINANT);
        let (intent_pin, intent_bytes) = public_input(&args.intent, "reset topology intent")?;
        let (intent, _intent_guard) = inputs::decode_reset_topology_intent(&intent_bytes)?;
        // No supervisor/beacon plan, trust, administrator identity or caller-computed
        // digest is an input to this first native derivation.
        let context = inputs::derive_reset_context(&intent, &args.local)?;
        let prior = args
            .prior_plan
            .as_ref()
            .map(|path| public_input(path, "prior supervisor plan"))
            .transpose()?;
        let plan = build_plan(
            args,
            &PlanContext {
                revision: &context.revision,
                validators: &context.validators,
                validator_clients: &context.validator_clients,
                operator_public_key: &context.operator_public_key,
                maintenance_admin_identity: &context.maintenance_admin_identity,
                maintenance_admin_config_sha256: &context.maintenance_admin_config_sha256,
                http_operator_key_sha256: &context.http_operator_key_sha256,
                public: &context.public_inputs,
                observation_trust_bytes: &context.observation_trust_bytes,
            },
            prior.as_ref().map(|(_, bytes)| bytes.clone()),
        )?;
        revalidate_pinned(&intent_pin, "reset topology intent")?;
        if let Some((pin, _)) = &prior {
            revalidate_pinned(pin, "prior supervisor plan")?;
        }
        context.revalidate()?;
        publish(&args.output_dir, &plan)
    }
}

#[cfg(all(test, unix))]
#[path = "taira_public_reset_epoch_reset_inputs_tests.rs"]
mod tests;
