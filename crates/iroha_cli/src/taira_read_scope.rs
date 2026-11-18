//! Offline preparation and all-four-validator signing of Taira's certified scoped-read activation
//! (`specs/runtime/taira_read_scope_v1.md`).
//!
//! The operator independently supplies the chain, authenticated selection, owner authorization,
//! release evidence and ordered scheduled roster. This tool checks those inputs and the exact
//! reviewed draft; it does not discover a committee, authenticate ledger state, submit a
//! transaction or claim activation. Core recomputes the whole change set at the execution boundary.

use std::{
    io::Write,
    num::NonZeroU64,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use eyre::{Result, WrapErr as _, ensure, eyre};
use iroha_core::runtime::{ironwood::published_digest, taira_read_scope};
use iroha_crypto::{PrivateKey, PublicKey, Signature};
use iroha_data_model::{
    NetworkId,
    isi::{InstructionBox, taira_read_scope::ScheduleTairaReadScopeV1},
    runtime::{
        definition::RootSelectionV1,
        lifecycle::{ChangeSetV1, RuntimeNoticeV1},
        taira_read_scope::{
            TAIRA_READ_SCOPE_COMMITTEE_SIZE_V1, TAIRA_READ_SCOPE_MAX_CERTIFICATE_BYTES_V1,
            TAIRA_READ_SCOPE_VERSION_V1, TairaReadScopeCertificateV1, TairaReadScopePlanV1,
        },
    },
};
use iroha_model_base::{chain::ChainId, peer::PeerId};

use crate::commands::sorafs::toolkit::bootstrap::{
    decode_canonical, parse_digest, read_bounded, read_pinned, read_private_key, validate_roster,
    write_new,
};

const MAX_CERTIFICATE_BYTES: usize = TAIRA_READ_SCOPE_MAX_CERTIFICATE_BYTES_V1;
const MAX_ROSTER_BYTES: usize = 64 * 1024;
const MAX_SHARE_BYTES: usize = 16 * 1024;

/// Local read-scope certificate artifacts.
#[derive(Debug, clap::Args)]
pub struct ReadScope {
    /// Offline certificate operation.
    #[command(subcommand)]
    command: Command,
}

/// Offline certificate operations; no command makes network requests.
#[derive(Debug, clap::Subcommand)]
pub enum Command {
    /// Draft the exact compiled change set of the permission activation from a pinned selection.
    DraftChangeSet(DraftChangeSetArgs),
    /// Write the unsigned canonical plan for activation.
    Prepare(PrepareArgs),
    /// Sign a pinned draft with one explicitly selected validator credential.
    Sign(SignArgs),
    /// Verify all four validators and write one transaction-stdin instruction.
    Assemble(AssembleArgs),
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

/// Independently selected signing context, required again at every operation.
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
    /// SHA-256 of the retained owner authorization that covers the scoped-read permission activation.
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
    /// JSON array of four `PeerId` values in exact scheduled committee order.
    #[arg(long, value_name = "PATH")]
    roster: PathBuf,
    /// BLAKE3 of the independently reviewed roster file, lowercase hex.
    #[arg(long, value_parser = parse_digest)]
    roster_blake3_hex: [u8; 32],
}

/// Public share bound to one ordered seat of the exact four-validator plan.
#[derive(Debug, Clone, norito::codec::Encode, norito::codec::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha::taira_read_scope::ReadScopeSignatureShare")]
struct ReadScopeSignatureShare {
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

impl ReadScope {
    /// Run without reading a client configuration or contacting a node.
    ///
    /// # Errors
    /// Any input differs from its pin or from the expected context, or an output already exists.
    pub(crate) fn run_without_client_config(&self, out: &mut impl Write) -> Result<()> {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_millis()
            .try_into()
            .map_err(|_| eyre!("clock is out of range"))?;
        self.command
            .execute(now_ms, out)
            .wrap_err("Taira read-scope")
    }
}

impl Command {
    fn execute(&self, now_ms: u64, out: &mut impl Write) -> Result<()> {
        match self {
            Self::DraftChangeSet(args) => {
                let change_set = args.change_set()?;
                let bytes = norito::json::to_vec(&change_set)?;
                write_new(&args.output, &bytes)?;
                writeln!(out, "predecessor={}", change_set.predecessor)?;
                writeln!(out, "successor={}", change_set.successor)?;
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
                    .collect::<Result<Vec<ReadScopeSignatureShare>>>()?;
                let certificate = assemble(draft, shares)?;
                let instruction = InstructionBox::from(ScheduleTairaReadScopeV1 { certificate });
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
        taira_read_scope::draft(
            &selection,
            self.network_id,
            self.effective_height.get(),
            self.qualification_hex,
        )
        .map_err(|error| eyre!(error))
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
    fn load(&self, now_ms: u64) -> Result<(TairaReadScopePlanV1, Vec<PeerId>)> {
        let roster = self.context.load_roster()?;
        let bytes = read_pinned(&self.draft, MAX_CERTIFICATE_BYTES, self.draft_blake3_hex)?;
        let draft = TairaReadScopePlanV1::from_bytes(&bytes)?;
        ensure!(
            draft == prepare(&self.context, &roster, now_ms)?,
            "draft differs from the independently selected context or pinned inputs"
        );
        Ok((draft, roster))
    }
}

/// The only plan these inputs allow. Core still recomputes the change set at the boundary.
fn prepare(context: &ContextArgs, roster: &[PeerId], now_ms: u64) -> Result<TairaReadScopePlanV1> {
    validate_roster(roster)?;
    ensure!(
        context.expires_at_unix_ms > now_ms,
        "read-scope plan has expired"
    );
    let change_set: ChangeSetV1 = norito::json::from_slice(&read_pinned(
        &context.change_set_json,
        MAX_CERTIFICATE_BYTES,
        context.change_set_blake3_hex,
    )?)?;
    let committee = roster
        .iter()
        .map(|peer| peer.public_key().clone())
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| eyre!("the certificate requires four validators"))?;
    let plan = TairaReadScopePlanV1 {
        version: TAIRA_READ_SCOPE_VERSION_V1,
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
    };
    plan.validate()?;
    ensure!(
        plan.change_set.publish.as_slice()
            == [iroha_core::runtime::state_scope_permission::descriptor()],
        "the change set must publish the compiled scoped-read definition"
    );
    Ok(plan)
}

fn sign(
    draft: &TairaReadScopePlanV1,
    roster: &[PeerId],
    seat: u16,
    key: &PrivateKey,
) -> Result<ReadScopeSignatureShare> {
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
    Ok(ReadScopeSignatureShare {
        signer_index: seat,
        signature,
    })
}

fn assemble(
    draft: TairaReadScopePlanV1,
    mut shares: Vec<ReadScopeSignatureShare>,
) -> Result<TairaReadScopeCertificateV1> {
    ensure!(
        shares.len() == TAIRA_READ_SCOPE_COMMITTEE_SIZE_V1,
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
        .map_err(|_| eyre!("the certificate requires four signatures"))?;
    let certificate = TairaReadScopeCertificateV1 {
        plan: draft,
        signatures,
    };
    certificate.verify()?;
    Ok(certificate)
}

#[cfg(test)]
mod tests;
