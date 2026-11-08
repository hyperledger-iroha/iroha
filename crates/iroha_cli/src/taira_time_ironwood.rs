//! Offline preparation and all-four-validator signing of the one-time Taira Time and Ironwood
//! activation (`specs/runtime/taira_time_ironwood_v1.md`).
//!
//! The operator independently supplies the chain, authenticated selection, pool, release evidence
//! and ordered scheduled roster. This tool checks those inputs and the exact reviewed draft; it
//! does not discover a committee, authenticate ledger state, submit a transaction or claim
//! activation. Core recomputes the whole change set at the execution boundary.

use std::{
    io::Write,
    num::NonZeroU64,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use eyre::{Result, WrapErr as _, ensure, eyre};
use iroha_core::runtime::{
    ironwood::{published_digest, taira_repaired_chain, taira_time_ironwood_change_set},
    taira_activation::AUTHORIZATION_DIGEST,
};
use iroha_crypto::{PrivateKey, PublicKey, Signature};
use iroha_data_model::{
    NetworkId,
    account::{AccountId, address::ChainDiscriminantGuard},
    asset::{AssetBalanceScope, AssetDefinitionId},
    isi::{InstructionBox, taira_activation::ScheduleTairaTimeIronwoodV1},
    privacy::{PrivacyIronwoodPoolBootstrapV1, PrivacyPoolIdV1},
    runtime::{
        definition::RootSelectionV1,
        lifecycle::{ChangeSetV1, RuntimeNoticeV1},
        taira_activation::{
            TAIRA_ACTIVATION_COMMITTEE_SIZE_V1, TAIRA_ACTIVATION_MAX_CERTIFICATE_BYTES_V1,
            TAIRA_ACTIVATION_VERSION_V1, TairaActivationCertificateV1, TairaActivationPlanV1,
        },
    },
};
use iroha_model_base::{chain::ChainId, peer::PeerId};

use crate::commands::sorafs::toolkit::bootstrap::{
    decode_canonical, parse_digest, read_bounded, read_pinned, read_private_key, validate_roster,
    write_new,
};

const MAX_CERTIFICATE_BYTES: usize = TAIRA_ACTIVATION_MAX_CERTIFICATE_BYTES_V1;
const MAX_POOL_BYTES: usize = 64 * 1024;
const MAX_ROSTER_BYTES: usize = 64 * 1024;
const MAX_SHARE_BYTES: usize = 16 * 1024;

/// Local Time and Ironwood activation artifacts.
#[derive(Debug, clap::Args)]
pub struct TimeIronwood {
    /// Independent I105 account chain discriminant for public JSON (Taira: 369).
    #[arg(long)]
    account_chain_discriminant: u16,
    /// Offline activation operation.
    #[command(subcommand)]
    command: Command,
}

/// Offline activation operations; no command makes network requests.
#[derive(Debug, clap::Subcommand)]
pub enum Command {
    /// Write the canonical JSON of the one pool this activation authorizes.
    Pool(PoolArgs),
    /// Draft the exact compiled change set from a pinned selection.
    DraftChangeSet(DraftChangeSetArgs),
    /// Write the unsigned canonical activation plan.
    Prepare(PrepareArgs),
    /// Sign a pinned draft with one explicitly selected validator credential.
    Sign(SignArgs),
    /// Verify all four validators and write one transaction-stdin instruction.
    Assemble(AssembleArgs),
}

/// The exact Ironwood pool the plan names; its public balance scope is always global.
#[derive(Debug, clap::Args)]
pub struct PoolArgs {
    /// Nonzero identity of the pool.
    #[arg(long, value_parser = parse_digest)]
    pool_id_hex: [u8; 32],
    /// Public asset the pool represents.
    #[arg(long)]
    asset_definition_id: AssetDefinitionId,
    /// Governed reserve account, read under the selected chain discriminant.
    #[arg(long)]
    reserve_account: String,
    /// Fresh JSON `PrivacyIronwoodPoolBootstrapV1` destination.
    #[arg(long)]
    output: PathBuf,
}

/// Pinned State inputs to the release-owned change set constructor.
#[derive(Debug, clap::Args)]
pub struct DraftChangeSetArgs {
    /// Independently selected signed-genesis network identity.
    #[arg(long)]
    network_id: NetworkId,
    /// Height at whose block start the change set activates.
    #[arg(long)]
    effective_height: NonZeroU64,
    /// JSON RootSelectionV1 exported from authenticated runtime State.
    #[arg(long)]
    selection_json: PathBuf,
    /// Independently reviewed BLAKE3 of the selection JSON.
    #[arg(long, value_parser = parse_digest)]
    selection_blake3_hex: [u8; 32],
    /// Digest of the exact successor qualification evidence.
    #[arg(long, value_parser = parse_digest)]
    qualification_hex: [u8; 32],
    /// Fresh JSON `ChangeSetV1` destination.
    #[arg(long)]
    output: PathBuf,
}

/// Independently selected activation context, required again at every operation.
#[derive(Debug, Clone, clap::Args)]
pub struct ContextArgs {
    /// Exact deployment chain label.
    #[arg(long)]
    chain_id: ChainId,
    /// Genesis-derived network identity independently selected by the operator.
    #[arg(long)]
    network_id: NetworkId,
    /// Nonzero identity of this one explicitly approved operation.
    #[arg(long, value_parser = parse_digest)]
    operation_id_hex: [u8; 32],
    /// Fewest blocks between scheduling and the effective height.
    #[arg(long)]
    notice_blocks: u64,
    /// Fewest committed milliseconds between proposal and activation.
    #[arg(long)]
    notice_ms: u64,
    /// Fewest committed milliseconds between scheduling and activation.
    #[arg(long)]
    notice_approval_cushion_ms: u64,
    /// Last block timestamp that may schedule this plan, in Unix milliseconds.
    #[arg(long)]
    expires_at_unix_ms: u64,
    /// Reviewed explicit owner authorization digest; this release pins the exact value.
    #[arg(long, value_parser = parse_digest)]
    authorization_digest_hex: [u8; 32],
    /// Reviewed release evidence digest.
    #[arg(long, value_parser = parse_digest)]
    release_digest_hex: [u8; 32],
    /// Reviewed immutable definition source closure evidence digest.
    #[arg(long, value_parser = parse_digest)]
    source_closure_digest_hex: [u8; 32],
    /// Exact release-owned `ChangeSetV1` JSON produced by draft-change-set.
    #[arg(long)]
    change_set_json: PathBuf,
    /// Independently reviewed BLAKE3 of the exact change set JSON.
    #[arg(long, value_parser = parse_digest)]
    change_set_blake3_hex: [u8; 32],
    /// JSON `PrivacyIronwoodPoolBootstrapV1` naming the one pool this activation authorizes.
    #[arg(long)]
    pool_json: PathBuf,
    /// Independently reviewed BLAKE3 of the exact pool JSON.
    #[arg(long, value_parser = parse_digest)]
    pool_blake3_hex: [u8; 32],
    /// JSON array of four `PeerId` values in exact scheduled committee order.
    #[arg(long, value_name = "PATH")]
    roster: PathBuf,
    /// BLAKE3 of the independently reviewed roster file, lowercase hex.
    #[arg(long, value_parser = parse_digest)]
    roster_blake3_hex: [u8; 32],
}

/// Public share bound to one ordered seat of the exact four-validator activation plan.
#[derive(Debug, Clone, norito::codec::Encode, norito::codec::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha::taira_time_ironwood::ActivationSignatureShare")]
struct ActivationSignatureShare {
    signer_index: u16,
    signature: Signature,
}

/// Expected context and a fresh output path for unsigned canonical bytes.
#[derive(Debug, clap::Args)]
pub struct PrepareArgs {
    /// Explicit expected context.
    #[command(flatten)]
    context: ContextArgs,
    /// New output file for the unsigned canonical plan; never overwritten.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
}

/// A reviewed draft pinned independently of the file being signed.
#[derive(Debug, clap::Args)]
pub struct DraftArgs {
    /// Explicit expected context.
    #[command(flatten)]
    context: ContextArgs,
    /// Canonical unsigned plan produced by prepare.
    #[arg(long, value_name = "PATH")]
    draft: PathBuf,
    /// BLAKE3 of the reviewed unsigned draft, lowercase hex.
    #[arg(long, value_parser = parse_digest)]
    draft_blake3_hex: [u8; 32],
}

/// One validator's runtime-only signing input.
#[derive(Debug, clap::Args)]
pub struct SignArgs {
    /// Reviewed plan and expected context.
    #[command(flatten)]
    draft: DraftArgs,
    /// Zero-based seat of this validator in the independently selected ordered roster.
    #[arg(long)]
    signer_index: u16,
    /// Owner-private, single-link, non-symlink file containing a native private key.
    #[arg(long, value_name = "PATH")]
    private_key_file: PathBuf,
    /// New public canonical signature-share file; never overwritten.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
}

/// Exactly four public signature shares and a transaction-stdin destination.
#[derive(Debug, clap::Args)]
pub struct AssembleArgs {
    /// Reviewed plan and expected context.
    #[command(flatten)]
    draft: DraftArgs,
    /// Canonical signature-share file; repeat for all four distinct validators.
    #[arg(long, value_name = "PATH", required = true)]
    signature: Vec<PathBuf>,
    /// New JSON file containing only the schedule instruction array for iroha tx stdin.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
}

impl TimeIronwood {
    /// Run without reading a client configuration or contacting a node.
    ///
    /// # Errors
    /// Any input differs from its pin or from the expected context, or an output already exists.
    pub(crate) fn run_without_client_config(&self, out: &mut impl Write) -> Result<()> {
        let _account_context = ChainDiscriminantGuard::enter(self.account_chain_discriminant);
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_millis()
            .try_into()
            .map_err(|_| eyre!("clock is out of range"))?;
        self.command
            .execute(now_ms, out)
            .wrap_err("Taira activation")
    }
}

impl Command {
    fn execute(&self, now_ms: u64, out: &mut impl Write) -> Result<()> {
        match self {
            Self::Pool(args) => {
                let reserve_account = AccountId::parse_encoded(&args.reserve_account)
                    .map_err(|_| eyre!("invalid reserve account"))?;
                let pool = PrivacyIronwoodPoolBootstrapV1::new(
                    PrivacyPoolIdV1::new(args.pool_id_hex),
                    args.asset_definition_id.clone(),
                    AssetBalanceScope::Global,
                    reserve_account,
                )?;
                let bytes = norito::json::to_vec(&pool)?;
                write_new(&args.output, &bytes)?;
                writeln!(out, "pool_blake3_hex={}", blake3::hash(&bytes).to_hex())?;
            }
            Self::DraftChangeSet(args) => {
                let change_set = args.change_set()?;
                let bytes = norito::json::to_vec(&change_set)?;
                write_new(&args.output, &bytes)?;
                writeln!(out, "published_definitions={}", change_set.publish.len())?;
                writeln!(out, "published_digest={}", published_digest(&change_set)?)?;
                writeln!(
                    out,
                    "change_set_blake3_hex={}",
                    blake3::hash(&bytes).to_hex()
                )?;
            }
            Self::Prepare(args) => {
                let roster = args.context.load_roster()?;
                let plan = prepare(&args.context, &roster, now_ms)?;
                let bytes = plan.to_bytes()?;
                write_new(&args.output, &bytes)?;
                writeln!(out, "draft_blake3_hex={}", blake3::hash(&bytes).to_hex())?;
                writeln!(out, "plan_id_hex={}", hex::encode(plan.id()?))?;
                writeln!(out, "change_set_id={}", plan.change_set.id()?)?;
            }
            Self::Sign(args) => {
                let (draft, roster) = args.draft.load(now_ms)?;
                let key = read_private_key(&args.private_key_file)?;
                let share = sign(&draft, &roster, args.signer_index, &key)?;
                write_new(&args.output, &norito::encode_canonical(&share)?)?;
            }
            Self::Assemble(args) => {
                let (draft, _) = args.draft.load(now_ms)?;
                let shares = args
                    .signature
                    .iter()
                    .map(|path| decode_canonical(&read_bounded(path, MAX_SHARE_BYTES)?))
                    .collect::<Result<Vec<ActivationSignatureShare>>>()?;
                let certificate = assemble(draft, shares)?;
                let instruction = InstructionBox::from(ScheduleTairaTimeIronwoodV1 { certificate });
                let json =
                    norito::json::to_vec(&vec![STANDARD.encode(norito::to_bytes(&instruction)?)])?;
                write_new(&args.output, &json)?;
            }
        }
        Ok(())
    }
}

impl DraftChangeSetArgs {
    fn change_set(&self) -> Result<ChangeSetV1> {
        let selection: RootSelectionV1 = norito::json::from_slice(&read_pinned(
            &self.selection_json,
            MAX_CERTIFICATE_BYTES,
            self.selection_blake3_hex,
        )?)?;
        // No route exports published definitions, and on the repaired selection they are a function
        // of this release. The signed change set lists them, and the boundary compares it with real
        // State.
        let (_, published) = taira_repaired_chain(self.network_id)?;
        selection.check_closure(&published)?;
        ensure!(
            self.qualification_hex != [0; 32],
            "qualification digest must be nonzero"
        );
        Ok(taira_time_ironwood_change_set(
            self.network_id,
            self.effective_height.get(),
            self.qualification_hex,
            &selection,
            &published,
        )?)
    }
}

impl ContextArgs {
    fn load_roster(&self) -> Result<Vec<PeerId>> {
        let bytes = read_pinned(&self.roster, MAX_ROSTER_BYTES, self.roster_blake3_hex)?;
        let roster: Vec<PeerId> =
            norito::json::from_slice(&bytes).wrap_err("invalid ordered roster JSON")?;
        validate_roster(&roster)?;
        Ok(roster)
    }
}

impl DraftArgs {
    fn load(&self, now_ms: u64) -> Result<(TairaActivationPlanV1, Vec<PeerId>)> {
        let roster = self.context.load_roster()?;
        let bytes = read_pinned(&self.draft, MAX_CERTIFICATE_BYTES, self.draft_blake3_hex)?;
        let draft = TairaActivationPlanV1::from_bytes(&bytes)?;
        ensure!(
            draft == prepare(&self.context, &roster, now_ms)?,
            "draft differs from the independently selected context or pinned inputs"
        );
        Ok((draft, roster))
    }
}

/// The only plan these inputs allow. Core still recomputes the change set at the boundary.
fn prepare(context: &ContextArgs, roster: &[PeerId], now_ms: u64) -> Result<TairaActivationPlanV1> {
    validate_roster(roster)?;
    ensure!(
        context.authorization_digest_hex == AUTHORIZATION_DIGEST,
        "this release authorizes only the owner authorization it pins"
    );
    ensure!(
        context.expires_at_unix_ms > now_ms,
        "activation plan has expired"
    );
    let change_set: ChangeSetV1 = norito::json::from_slice(&read_pinned(
        &context.change_set_json,
        MAX_CERTIFICATE_BYTES,
        context.change_set_blake3_hex,
    )?)?;
    let pool: PrivacyIronwoodPoolBootstrapV1 = norito::json::from_slice(&read_pinned(
        &context.pool_json,
        MAX_POOL_BYTES,
        context.pool_blake3_hex,
    )?)
    .wrap_err("invalid pool JSON")?;
    let committee = roster
        .iter()
        .map(|peer| peer.public_key().clone())
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| eyre!("activation requires four validators"))?;
    let plan = TairaActivationPlanV1 {
        version: TAIRA_ACTIVATION_VERSION_V1,
        chain_id: context.chain_id.clone(),
        network_id: context.network_id,
        operation_id: context.operation_id_hex,
        change_set,
        notice: RuntimeNoticeV1 {
            blocks: context.notice_blocks,
            ms: context.notice_ms,
            approval_cushion_ms: context.notice_approval_cushion_ms,
        },
        committee,
        expires_at_unix_ms: context.expires_at_unix_ms,
        authorization_digest: context.authorization_digest_hex,
        release_digest: context.release_digest_hex,
        source_closure_digest: context.source_closure_digest_hex,
        pool,
    };
    plan.validate()?;
    Ok(plan)
}

fn sign(
    draft: &TairaActivationPlanV1,
    roster: &[PeerId],
    seat: u16,
    key: &PrivateKey,
) -> Result<ActivationSignatureShare> {
    validate_roster(roster)?;
    ensure!(
        roster
            .iter()
            .map(|peer| peer.public_key())
            .eq(draft.committee.iter()),
        "draft committee differs from roster"
    );
    let peer = roster
        .get(usize::from(seat))
        .ok_or_else(|| eyre!("signer seat is outside roster"))?;
    let public = PublicKey::from_private_key(key)
        .map_err(|_| eyre!("cannot derive validator public key"))?;
    ensure!(
        &public == peer.public_key(),
        "validator credential does not match selected roster seat"
    );
    let preimage = draft.signing_bytes()?;
    let signature =
        Signature::try_new(key, &preimage).map_err(|_| eyre!("validator signature failed"))?;
    signature
        .verify(&public, &preimage)
        .map_err(|_| eyre!("validator signature self-check failed"))?;
    Ok(ActivationSignatureShare {
        signer_index: seat,
        signature,
    })
}

fn assemble(
    draft: TairaActivationPlanV1,
    mut shares: Vec<ActivationSignatureShare>,
) -> Result<TairaActivationCertificateV1> {
    ensure!(
        shares.len() == TAIRA_ACTIVATION_COMMITTEE_SIZE_V1,
        "supply all four signature shares"
    );
    shares.sort_by_key(|share| share.signer_index);
    ensure!(
        shares
            .iter()
            .enumerate()
            .all(|(index, share)| usize::from(share.signer_index) == index),
        "signature shares must cover each of the four distinct seats exactly once"
    );
    let signatures = shares
        .into_iter()
        .map(|share| share.signature)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| eyre!("activation requires four signatures"))?;
    let certificate = TairaActivationCertificateV1 {
        plan: draft,
        signatures,
    };
    certificate.verify()?;
    Ok(certificate)
}

#[cfg(test)]
mod tests;
