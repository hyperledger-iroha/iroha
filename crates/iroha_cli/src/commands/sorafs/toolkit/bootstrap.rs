//! Offline preparation and all-four-validator signing of the one-time SoraFS repair.
//!
//! The operator independently supplies the chain, authenticated parent, release evidence and ordered scheduled
//! roster. This tool checks those inputs and the exact reviewed draft; it does not discover
//! a committee, authenticate ledger state, submit a transaction or claim activation.

use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{Read as _, Write as _},
    num::NonZeroU64,
    path::{Path, PathBuf},
    process::ExitCode,
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use eyre::{Result, WrapErr as _, ensure, eyre};
use iroha_core::sumeragi::schedule::canonical_committee;
use iroha_crypto::{PrivateKey, PublicKey, Signature};
use iroha_data_model::{
    NetworkId,
    isi::{InstructionBox, sorafs_repair::ApplySorafsRepairV1},
    runtime::{
        definition::{DefinitionDescriptorV1, RootSelectionV1},
        lifecycle::ChangeSetV1,
        sorafs_repair::{
            SORAFS_REPAIR_COMMITTEE_SIZE_V1, SORAFS_REPAIR_MAX_CERTIFICATE_BYTES_V1,
            SORAFS_REPAIR_VERSION_V1, SorafsRepairCertificateV1, SorafsRepairPlanV1,
        },
    },
    sorafs::{
        bootstrap::{SORAFS_BOOTSTRAP_MAX_PAYLOAD_BYTES_V1, SorafsBootstrapPayloadV1},
        provider_admission::{ProviderAdmissionCouncilPolicyV1, governance::decode_frame},
    },
};
use iroha_model_base::{chain::ChainId, peer::PeerId};
use sorafs_manifest::provider_admission::ProviderAdmissionEnvelopeV1;

use super::validation::CliError;

mod payload;

pub use payload::PreparePayloadArgs;

const MAX_CERTIFICATE_BYTES: usize = SORAFS_REPAIR_MAX_CERTIFICATE_BYTES_V1;
const MAX_ROSTER_BYTES: usize = 64 * 1024;

/// Local SoraFS repair artifacts; no command makes network requests.
#[derive(Debug, clap::Subcommand)]
pub enum Command {
    /// Construct and council-sign a pinned public provider/bootstrap request.
    PreparePayload(PreparePayloadArgs),
    /// Draft the exact compiled repair transition from pinned published definitions.
    DraftChangeSet(DraftChangeSetArgs),
    /// Validate a JSON payload and write its unsigned canonical repair plan.
    Prepare(PrepareArgs),
    /// Sign a pinned draft with one explicitly selected validator credential.
    Sign(SignArgs),
    /// Verify all four validators and write one transaction-stdin instruction.
    Assemble(AssembleArgs),
}

/// Pinned State inputs to the release-owned repair transition constructor.
#[derive(Debug, clap::Args)]
pub struct DraftChangeSetArgs {
    /// Independently selected signed-genesis network identity.
    #[arg(long)]
    network_id: NetworkId,
    /// Exact parent height; the change executes at the following height.
    #[arg(long)]
    parent_height: NonZeroU64,
    /// JSON RootSelectionV1 exported from authenticated runtime State.
    #[arg(long)]
    selection_json: PathBuf,
    /// Independently reviewed BLAKE3 of the selection JSON.
    #[arg(long, value_parser = parse_digest)]
    selection_blake3_hex: [u8; 32],
    /// JSON array of every published DefinitionDescriptorV1.
    #[arg(long)]
    definitions_json: PathBuf,
    /// Independently reviewed BLAKE3 of the published definitions JSON.
    #[arg(long, value_parser = parse_digest)]
    definitions_blake3_hex: [u8; 32],
    /// Digest of the exact successor qualification evidence.
    #[arg(long, value_parser = parse_digest)]
    qualification_hex: [u8; 32],
    /// Fresh JSON ChangeSetV1 destination.
    #[arg(long)]
    output: PathBuf,
}

/// Independently selected repair context, required again at every operation.
#[derive(Debug, Clone, clap::Args)]
pub struct ContextArgs {
    /// Exact deployment chain label.
    #[arg(long)]
    chain_id: ChainId,
    /// Genesis-derived network identity independently selected by the operator.
    #[arg(long)]
    network_id: NetworkId,
    /// Authenticated parent height; execution must be at parent + 1.
    #[arg(long)]
    parent_height: NonZeroU64,
    /// Native committed Sumeragi core hash, not the Iroha block header hash.
    #[arg(long, value_parser = parse_digest)]
    parent_core_hash_hex: [u8; 32],
    /// Native authenticated parent execution result hash.
    #[arg(long, value_parser = parse_digest)]
    parent_result_hash_hex: [u8; 32],
    /// Nonzero identity of this one explicitly approved operation.
    #[arg(long, value_parser = parse_digest)]
    operation_id_hex: [u8; 32],
    /// Last permitted executing block timestamp, in Unix milliseconds.
    #[arg(long)]
    expires_at_unix_ms: u64,
    /// Reviewed explicit operator authorization digest.
    #[arg(long, value_parser = parse_digest)]
    authorization_digest_hex: [u8; 32],
    /// Reviewed release evidence digest.
    #[arg(long, value_parser = parse_digest)]
    release_digest_hex: [u8; 32],
    /// Reviewed immutable definition source closure evidence digest.
    #[arg(long, value_parser = parse_digest)]
    source_closure_digest_hex: [u8; 32],
    /// Exact release-owned ChangeSetV1 JSON produced by draft-change-set.
    #[arg(long)]
    change_set_json: PathBuf,
    /// Independently reviewed BLAKE3 of the exact change set JSON.
    #[arg(long, value_parser = parse_digest)]
    change_set_blake3_hex: [u8; 32],
    /// JSON array of four PeerId values in exact scheduled committee order.
    #[arg(long, value_name = "PATH")]
    roster: PathBuf,
    /// BLAKE3 of the independently reviewed roster file, lowercase hex.
    #[arg(long, value_parser = parse_digest)]
    roster_blake3_hex: [u8; 32],
}

/// Public share bound to one ordered seat of the exact four-validator repair plan.
#[derive(Debug, Clone, norito::codec::Encode, norito::codec::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha::commands::sorafs::toolkit::bootstrap::RepairSignatureShare")]
struct RepairSignatureShare {
    signer_index: u16,
    signature: Signature,
}

/// Payload input and a fresh output path for unsigned canonical bytes.
#[derive(Debug, clap::Args)]
pub struct PrepareArgs {
    /// Explicit expected context.
    #[command(flatten)]
    context: ContextArgs,
    /// JSON SorafsBootstrapPayloadV1 containing canonical council/admission frames.
    #[arg(long, value_name = "PATH")]
    payload_json: PathBuf,
    /// New output file for the unsigned canonical certificate; never overwritten.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
}

/// A reviewed draft pinned independently of the file being signed.
#[derive(Debug, clap::Args)]
pub struct DraftArgs {
    /// Explicit expected context.
    #[command(flatten)]
    context: ContextArgs,
    /// Canonical unsigned repair plan produced by prepare.
    #[arg(long, value_name = "PATH")]
    draft: PathBuf,
    /// BLAKE3 of the reviewed unsigned draft, lowercase hex.
    #[arg(long, value_parser = parse_digest)]
    draft_blake3_hex: [u8; 32],
}

/// One validator's runtime-only signing input.
#[derive(Debug, clap::Args)]
pub struct SignArgs {
    /// Reviewed repair plan and expected context.
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
    /// Reviewed repair plan and expected context.
    #[command(flatten)]
    draft: DraftArgs,
    /// Canonical signature-share file; repeat for all four distinct validators.
    #[arg(long, value_name = "PATH", required = true)]
    signature: Vec<PathBuf>,
    /// New JSON file containing only the Apply instruction array for iroha tx stdin.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
}

impl Command {
    /// Run without reading a client configuration or contacting a node.
    pub(crate) fn run(self) -> std::result::Result<ExitCode, CliError> {
        self.execute()
            .map(|()| ExitCode::SUCCESS)
            .map_err(|error| CliError::Validation(format!("SoraFS repair: {error:#}")))
    }

    fn execute(self) -> Result<()> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        match self {
            Self::DraftChangeSet(args) => args.execute()?,
            Self::PreparePayload(args) => payload::run(args, now)?,
            Self::Prepare(args) => {
                let roster = args.context.load_roster()?;
                let bytes = read_bounded(
                    &args.payload_json,
                    SORAFS_BOOTSTRAP_MAX_PAYLOAD_BYTES_V1 * 8,
                )?;
                let payload: SorafsBootstrapPayloadV1 =
                    norito::json::from_slice(&bytes).wrap_err("invalid bootstrap payload JSON")?;
                validate_payload(&payload, &args.context.network_id, now)?;
                let draft = prepare(&payload, &args.context, &roster)?;
                ensure!(
                    args.context.expires_at_unix_ms > now.saturating_mul(1000),
                    "repair plan has expired"
                );
                let bytes = draft.to_bytes()?;
                write_new(&args.output, &bytes)?;
                println!("draft_blake3_hex={}", blake3::hash(&bytes).to_hex());
            }
            Self::Sign(args) => {
                let (draft, roster) = args.draft.load(now)?;
                ensure!(
                    usize::from(args.signer_index) < roster.len(),
                    "signer seat is outside roster"
                );
                let key = read_private_key(&args.private_key_file)?;
                let signature = sign(&draft, &roster, args.signer_index, &key)?;
                write_new(&args.output, &norito::encode_canonical(&signature)?)?;
            }
            Self::Assemble(args) => {
                let (draft, roster) = args.draft.load(now)?;
                ensure!(
                    args.signature.len() == SORAFS_REPAIR_COMMITTEE_SIZE_V1,
                    "supply all four signature shares"
                );
                let shares = args
                    .signature
                    .iter()
                    .map(|path| decode_canonical(&read_bounded(path, 16 * 1024)?))
                    .collect::<Result<Vec<RepairSignatureShare>>>()?;
                let certificate = assemble(draft, shares, &args.draft.context, &roster)?;
                let instruction = InstructionBox::from(ApplySorafsRepairV1 { certificate });
                let json =
                    norito::json::to_vec(&vec![STANDARD.encode(norito::to_bytes(&instruction)?)])?;
                write_new(&args.output, &json)?;
            }
        }
        Ok(())
    }
}

pub(super) fn read_private_key(path: &Path) -> Result<PrivateKey> {
    let bytes = super::super::read_owner_private_handshake_file(
        path,
        16 * 1024,
        None,
        "SoraFS signing credential",
    )?;
    // Never include parser diagnostics or secret input in errors.
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| eyre!("SoraFS credential is not native key text"))?;
    text.trim()
        .parse::<PrivateKey>()
        .map_err(|_| eyre!("SoraFS credential is not a valid native private key"))
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

impl DraftChangeSetArgs {
    fn execute(self) -> Result<()> {
        let selection: RootSelectionV1 = norito::json::from_slice(&read_pinned(
            &self.selection_json,
            MAX_CERTIFICATE_BYTES,
            self.selection_blake3_hex,
        )?)?;
        let descriptors: Vec<DefinitionDescriptorV1> = norito::json::from_slice(&read_pinned(
            &self.definitions_json,
            MAX_CERTIFICATE_BYTES * 8,
            self.definitions_blake3_hex,
        )?)?;
        let mut published = BTreeMap::new();
        for descriptor in descriptors {
            ensure!(
                published.insert(descriptor.id()?, descriptor).is_none(),
                "duplicate published definition"
            );
        }
        selection.check_closure(&published)?;
        ensure!(
            self.qualification_hex != [0; 32],
            "qualification digest must be nonzero"
        );
        let height = self
            .parent_height
            .get()
            .checked_add(1)
            .ok_or_else(|| eyre!("parent height overflow"))?;
        let change_set = iroha_core::runtime::release_catalog()?.sorafs_repair_activation(
            &selection,
            &published,
            self.network_id,
            height,
            self.qualification_hex,
        )?;
        let bytes = norito::json::to_vec(&change_set)?;
        write_new(&self.output, &bytes)?;
        println!("change_set_blake3_hex={}", blake3::hash(&bytes).to_hex());
        Ok(())
    }
}

impl DraftArgs {
    fn load(&self, now: u64) -> Result<(SorafsRepairPlanV1, Vec<PeerId>)> {
        let roster = self.context.load_roster()?;
        let bytes = read_pinned(&self.draft, MAX_CERTIFICATE_BYTES, self.draft_blake3_hex)?;
        let draft = SorafsRepairPlanV1::from_bytes(&bytes)?;
        validate_payload(&draft.payload, &self.context.network_id, now)?;
        ensure!(
            draft.expires_at_unix_ms > now.saturating_mul(1000),
            "repair plan has expired"
        );
        ensure!(
            draft == prepare(&draft.payload, &self.context, &roster)?,
            "draft differs from independently selected repair context or pinned change set"
        );
        Ok((draft, roster))
    }
}

fn validate_roster(roster: &[PeerId]) -> Result<()> {
    ensure!(
        roster.len() == SORAFS_REPAIR_COMMITTEE_SIZE_V1,
        "repair committee must contain exactly four validators"
    );
    ensure!(
        canonical_committee(roster.iter().cloned())? == roster,
        "roster must contain unique BLS-normal validators in native consensus order"
    );
    Ok(())
}

pub(super) fn validate_payload(
    payload: &SorafsBootstrapPayloadV1,
    network: &NetworkId,
    now: u64,
) -> Result<()> {
    payload.validate()?;
    let council: ProviderAdmissionCouncilPolicyV1 = decode_frame(&payload.council)?;
    council.validate()?;
    ensure!(
        council.network_id == *network.as_bytes() && council.revision == 1 && !council.paused,
        "bootstrap council must be active revision one on the selected network"
    );
    let mut previous = None;
    for provider in &payload.providers {
        let envelope: ProviderAdmissionEnvelopeV1 = decode_frame(&provider.envelope)?;
        ensure!(
            envelope.admission_revision == 1 && envelope.expected_current_event_digest.is_none(),
            "bootstrap provider admission must be revision one"
        );
        council.verify_envelope_policy_claim(&envelope, now)?;
        let id = envelope.proposal.provider_id;
        ensure!(
            previous.is_none_or(|prior| prior < id),
            "providers must be strictly ordered by provider id"
        );
        previous = Some(id);
    }
    Ok(())
}

fn prepare(
    payload: &SorafsBootstrapPayloadV1,
    context: &ContextArgs,
    roster: &[PeerId],
) -> Result<SorafsRepairPlanV1> {
    validate_roster(roster)?;
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
        .map_err(|_| eyre!("repair requires four validators"))?;
    let plan = SorafsRepairPlanV1 {
        version: SORAFS_REPAIR_VERSION_V1,
        chain_id: context.chain_id.clone(),
        network_id: context.network_id,
        operation_id: context.operation_id_hex,
        parent_height: context.parent_height.get(),
        parent_block_hash: context.parent_core_hash_hex,
        parent_result_hash: context.parent_result_hash_hex,
        change_set,
        committee,
        expires_at_unix_ms: context.expires_at_unix_ms,
        authorization_digest: context.authorization_digest_hex,
        release_digest: context.release_digest_hex,
        source_closure_digest: context.source_closure_digest_hex,
        payload: payload.clone(),
    };
    plan.validate()?;
    Ok(plan)
}

fn sign(
    draft: &SorafsRepairPlanV1,
    roster: &[PeerId],
    seat: u16,
    key: &PrivateKey,
) -> Result<RepairSignatureShare> {
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
    Ok(RepairSignatureShare {
        signer_index: seat,
        signature,
    })
}

fn assemble(
    draft: SorafsRepairPlanV1,
    mut shares: Vec<RepairSignatureShare>,
    context: &ContextArgs,
    roster: &[PeerId],
) -> Result<SorafsRepairCertificateV1> {
    ensure!(
        draft == prepare(&draft.payload, context, roster)?,
        "draft differs from expected repair context"
    );
    ensure!(
        shares.len() == SORAFS_REPAIR_COMMITTEE_SIZE_V1,
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
        .map_err(|_| eyre!("repair requires four signatures"))?;
    let certificate = SorafsRepairCertificateV1 {
        plan: draft,
        signatures,
    };
    certificate.verify()?;
    Ok(certificate)
}

pub(super) fn parse_digest(text: &str) -> std::result::Result<[u8; 32], String> {
    if text.len() != 64
        || !text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("digest must be exactly 64 lowercase hexadecimal characters".into());
    }
    let mut digest = [0; 32];
    hex::decode_to_slice(text, &mut digest).map_err(|_| "invalid digest".to_owned())?;
    Ok(digest)
}

fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let file = fs::File::open(path).wrap_err("cannot open public SoraFS input")?;
    ensure!(
        file.metadata()?.is_file(),
        "SoraFS input must be a regular file"
    );
    ensure!(
        file.metadata()?.len() <= u64::try_from(limit)?,
        "SoraFS input exceeds byte limit"
    );
    let mut bytes = Vec::new();
    file.take(u64::try_from(limit)? + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        !bytes.is_empty() && bytes.len() <= limit,
        "SoraFS input is empty or exceeds byte limit"
    );
    Ok(bytes)
}

pub(super) fn read_pinned(path: &Path, limit: usize, digest: [u8; 32]) -> Result<Vec<u8>> {
    let bytes = read_bounded(path, limit)?;
    ensure!(
        blake3::hash(&bytes).as_bytes() == &digest,
        "SoraFS input does not match reviewed BLAKE3 pin"
    );
    Ok(bytes)
}

fn decode_canonical<T>(bytes: &[u8]) -> Result<T>
where
    T: norito::core::NoritoSerialize + for<'de> norito::core::NoritoDeserialize<'de>,
{
    norito::decode_canonical_with_limits(
        bytes,
        norito::DecodeLimits::new(
            65536,
            MAX_CERTIFICATE_BYTES,
            65536,
            MAX_CERTIFICATE_BYTES * 4,
            64,
        ),
    )
    .map_err(|_| eyre!("SoraFS artifact is not canonical bounded Norito"))
}

pub(super) fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .wrap_err("SoraFS output must be a new file")?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests;
