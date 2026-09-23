//! Read-only qualification of automatic KAGEMUSHA authority retention across real epochs.
//!
//! All progress comes from the existing authenticated four-peer finality observer. This command
//! never produces transactions or empty blocks. It verifies complete consecutive authorizations,
//! unchanged mint and beacon custody, and the exact parent-QC boundary before retaining evidence.

use super::*;
use iroha_data_model::{
    block::consensus_v2::HeightContext,
    bridge::BridgeFinalityProof,
    isi::kagemusha_v1::{
        BeaconEpochBindingV1, KagemushaMintFinalityEpochAuthorizationV1,
        KagemushaMintFinalityEpochDecisionV1,
    },
};
use std::num::NonZeroU64;

#[path = "taira_epoch_supervisor.rs"]
mod supervisor;

#[derive(Debug, clap::Subcommand)]
pub(crate) enum Command {
    /// Observe real committed epoch retention and durably retain its verified evidence.
    Maintain(MaintainArgs),
    /// Reauthenticate existing retention evidence without modifying its journal.
    Status(MaintainArgs),
    /// Continuously observe retention under an explicit public release policy and one worker lock.
    Supervise(supervisor::Args),
    /// Reauthenticate the live worker incarnation and its current retained epoch.
    SupervisorStatus(supervisor::StatusArgs),
}

#[derive(Debug, clap::Args)]
pub(crate) struct MaintainArgs {
    /// Independently selected signed genesis and exact four observation peers.
    #[arg(long)]
    trust: PathBuf,
    /// Existing owner-private parent of the network's retention journal.
    #[arg(long)]
    journal_dir: PathBuf,
    /// Wait until this actual scheduling epoch is finalized under unchanged custody.
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
    stop_after_epoch: u64,
    /// Finite read-side budget; it never authorizes a write to the ledger.
    #[arg(long, default_value_t = 180_000, value_parser = clap::value_parser!(u64).range(1..))]
    timeout_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct RetentionCompletionV1 {
    schema_version: u8,
    network_id: NetworkId,
    anchor_height: u64,
    observed_height: u64,
    completed_epoch: u64,
    authority_generation: u64,
    authority_id: [u8; 32],
    authorization_id: [u8; 32],
    beacon_binding: BeaconEpochBindingV1,
    authorization_chain: Vec<KagemushaMintFinalityEpochAuthorizationV1>,
    boundary_context_id: iroha_data_model::block::consensus_v2::HeightContextId,
    successor_context_id: iroha_data_model::block::consensus_v2::HeightContextId,
    parent_decision: RetentionParentDecisionV1,
    authority: iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalityAuthorityGenerationV1,
    #[norito(required)]
    previous_cursor_id: Option<String>,
    cursor_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct RetentionParentDecisionV1 {
    context_id: iroha_data_model::block::consensus_v2::HeightContextId,
    height: u64,
    phase: iroha_data_model::block::consensus_v2::GlobalPhase,
    subject: iroha_data_model::block::consensus_v2::BlockSubject,
    execution_commitment: iroha_data_model::block::consensus_v2::ExecutionCommitment,
}

impl RetentionCompletionV1 {
    fn computed_id(&self) -> Result<String> {
        let mut body = self.clone();
        body.cursor_id.clear();
        let mut bytes = b"iroha.kagemusha.retention-cursor.v1\0".to_vec();
        bytes.extend(json::to_vec(&body)?);
        Ok(digest(&bytes))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct RetentionCursorV1 {
    schema_version: u8,
    network_id: NetworkId,
    completed_epoch: u64,
    cursor_id: String,
}

/// Pure public admission shared by deployment and the read-only native worker.
pub(crate) fn supervisor_generation_admission(
    policy_bytes: &[u8],
    trust_bytes: &[u8],
    config: &iroha::config::Config,
    http_operator: &iroha_crypto::KeyPair,
) -> Result<()> {
    supervisor::generation_admission(policy_bytes, trust_bytes, config, http_operator)
}

fn require_epoch_budget(deadline: Instant, stage: &str) -> Result<()> {
    require(
        Instant::now() < deadline,
        &format!("epoch retention deadline exhausted while {stage}"),
    )
}

/// Preserve independently selected trust across release changes, never across a network change.
fn validate_observation_trust(
    original: &DeploymentTrustV1,
    current: &DeploymentTrustV1,
    network: NetworkId,
) -> Result<()> {
    original.validate(network)?;
    current.validate(network)?;
    let mut identity = current.clone();
    for (peer, retained) in identity.peers.iter_mut().zip(&original.peers) {
        peer.build_fingerprint = retained.build_fingerprint;
        peer.config_fingerprint = retained.config_fingerprint;
    }
    require(
        &identity == original,
        "current observation trust changed original network, genesis, roster, identity or endpoint",
    )
}

/// Read-only custody of a supervisor journal, or its pre-installation absence.
///
/// The caller must retain its distinct deployment lock through quiescence and
/// drop this guard immediately before starting the supervised worker.
pub(crate) struct SupervisorJournalGuard {
    journal: Option<Journal>,
    parent_path: PathBuf,
    parent: File,
    worker_name: String,
}

impl SupervisorJournalGuard {
    #[cfg(unix)]
    fn revalidate_parent(&self) -> Result<()> {
        use std::os::unix::fs::MetadataExt as _;
        require(
            self.parent_path.canonicalize()? == self.parent_path,
            "supervisor journal parent must remain a direct canonical path",
        )?;
        let named = fs::symlink_metadata(&self.parent_path)?;
        let held = self.parent.metadata()?;
        private_metadata(&named, true)?;
        private_metadata(&held, true)?;
        require(
            named.dev() == held.dev() && named.ino() == held.ino(),
            "supervisor journal parent was replaced during quiescence",
        )
    }

    /// Recheck the held journal lock, or the unchanged parent and absent child.
    #[cfg(unix)]
    pub(crate) fn revalidate(&self) -> Result<()> {
        self.revalidate_parent()?;
        if let Some(journal) = &self.journal {
            journal.revalidate()?;
        } else {
            match rustix::fs::statat(
                &self.parent,
                self.worker_name.as_str(),
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            ) {
                Err(rustix::io::Errno::NOENT) => {}
                Ok(_) => {
                    eyre::bail!("supervisor journal appeared after the pre-installation check")
                }
                Err(error) => return Err(error.into()),
            }
        }
        self.revalidate_parent()
    }

    /// Supervisor journal custody is available only on Unix.
    #[cfg(not(unix))]
    pub(crate) fn revalidate(&self) -> Result<()> {
        eyre::bail!("supervisor journal quiescence requires Unix descriptor custody")
    }
}

/// Hold the existing native worker lock without creating or repairing a journal.
///
/// An absent child is accepted only as a pre-first-installation observation;
/// callers retain their deployment lock and must revalidate before worker start.
#[cfg(unix)]
pub(crate) fn supervisor_journal_guard(
    journal_dir: &Path,
    network: NetworkId,
) -> Result<SupervisorJournalGuard> {
    use rustix::fs::{Mode, OFlags};
    require(
        journal_dir.is_absolute()
            && journal_dir.components().all(|part| {
                matches!(
                    part,
                    std::path::Component::RootDir | std::path::Component::Normal(_)
                )
            })
            && journal_dir.canonicalize()? == journal_dir,
        "supervisor journal parent must be an absolute direct canonical directory",
    )?;
    let parent = File::from(rustix::fs::open(
        journal_dir,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )?);
    private_metadata(&parent.metadata()?, true)?;
    let mut guard = SupervisorJournalGuard {
        journal: None,
        parent_path: journal_dir.into(),
        parent,
        worker_name: format!("epoch-worker-{network}"),
    };
    guard.revalidate_parent()?;
    match rustix::fs::statat(
        &guard.parent,
        guard.worker_name.as_str(),
        rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
    ) {
        Ok(_) => {
            guard.journal = Some(Journal::open(
                &guard.parent_path.join(&guard.worker_name),
                false,
            )?);
        }
        Err(rustix::io::Errno::NOENT) => {}
        Err(error) => return Err(error.into()),
    }
    guard.revalidate()?;
    Ok(guard)
}

/// Supervisor journal custody is available only on Unix.
#[cfg(not(unix))]
pub(crate) fn supervisor_journal_guard(_: &Path, _: NetworkId) -> Result<SupervisorJournalGuard> {
    eyre::bail!("supervisor journal quiescence requires Unix descriptor custody")
}

/// Repair only the mkdir-before-lock cut, never any prepared operation state.
#[cfg(unix)]
fn open_initializing_journal(path: &Path, create: bool) -> Result<Journal> {
    use rustix::fs::{Mode, OFlags};
    use std::os::unix::fs::MetadataExt as _;
    if !create
        && fs::symlink_metadata(path.join("lock"))
            .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
    {
        let name = path
            .file_name()
            .ok_or_else(|| eyre!("initial journal has no filename"))?;
        let parent_path = path
            .parent()
            .ok_or_else(|| eyre!("initial journal has no parent"))?
            .canonicalize()?;
        let parent = File::from(rustix::fs::open(
            &parent_path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
        )?);
        private_metadata(&parent.metadata()?, true)?;
        let directory = File::from(rustix::fs::openat(
            &parent,
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
        )?);
        let held = directory.metadata()?;
        private_metadata(&held, true)?;
        let named_path = parent_path.join(name);
        require(
            fs::read_dir(&named_path)?.next().is_none(),
            "missing journal lock with retained evidence cannot be recreated",
        )?;
        let named = fs::symlink_metadata(&named_path)?;
        require(
            named.is_dir() && named.dev() == held.dev() && named.ino() == held.ino(),
            "initial journal directory changed before lock recovery",
        )?;
        match rustix::fs::openat(
            &directory,
            "lock",
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::from_raw_mode(0o600),
        ) {
            Ok(fd) => {
                File::from(fd).sync_all()?;
                directory.sync_all()?;
            }
            Err(rustix::io::Errno::EXIST) => {}
            Err(error) => return Err(error.into()),
        }
    }
    Journal::open(path, create)
}

#[cfg(not(unix))]
fn open_initializing_journal(_: &Path, _: bool) -> Result<Journal> {
    eyre::bail!("epoch journal initialization requires Unix")
}

/// Only a provably unprepared initialization may resume after a publication crash.
fn require_uninitialized_journal(journal: &Journal, allowed: &[&str]) -> Result<()> {
    journal.revalidate()?;
    for (index, entry) in fs::read_dir(&journal.path)?.enumerate() {
        require(
            index < 64,
            "incomplete journal has excessive initialization debris",
        )?;
        let entry = entry?;
        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or_else(|| eyre!("invalid initialization evidence name"))?;
        let unreferenced_stage = name.strip_prefix(".staging-").is_some_and(|suffix| {
            suffix.len() == 32
                && suffix
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        });
        require(
            name == "lock" || allowed.contains(&name) || unreferenced_stage,
            "incomplete journal contains retained operation evidence; initialization cannot replace it",
        )?;
    }
    journal.revalidate()
}

fn certificate_decision(
    certificate: &iroha_data_model::block::consensus_v2::QuorumCertificate,
) -> RetentionParentDecisionV1 {
    RetentionParentDecisionV1 {
        context_id: certificate.round.context_id,
        height: certificate.round.height,
        phase: certificate.phase,
        subject: certificate.subject,
        execution_commitment: certificate.execution_commitment,
    }
}

fn verify_retained_boundary(
    previous: &BridgeFinalityProof,
    next: &BridgeFinalityProof,
) -> Result<()> {
    let before = &previous.finality_artifact.height_context;
    let after = &next.finality_artifact.height_context;
    let snapshot = before
        .next_epoch_snapshot
        .as_ref()
        .ok_or_else(|| eyre!("retention boundary omits the certified successor snapshot"))?;
    let parent = after
        .parent_commit_qc
        .as_ref()
        .ok_or_else(|| eyre!("retained epoch has no authenticated parent CommitQC"))?;
    after
        .kagemusha_mint_finality_authorization
        .validate_successor(&before.kagemusha_mint_finality_authorization)
        .map_err(|error| {
            eyre!("retained authorization is not a complete linked successor: {error}")
        })?;
    require(
        before.height == before.epoch_end_height
            && before.height.checked_add(1) == Some(after.height)
            && before.epoch.checked_add(1) == Some(after.epoch)
            && after.kagemusha_mint_finality_authorization.decision
                == KagemushaMintFinalityEpochDecisionV1::Retain
            && after.kagemusha_mint_finality_authority == before.kagemusha_mint_finality_authority
            && after.roster == before.roster
            && after.mode == before.mode
            && after.quorum == before.quorum
            && snapshot.epoch == after.epoch
            && snapshot.epoch_end_height == after.epoch_end_height
            && snapshot.kagemusha_mint_finality_authorization
                == after.kagemusha_mint_finality_authorization
            && snapshot.kagemusha_mint_finality_authority
                == after.kagemusha_mint_finality_authority
            && snapshot.roster == after.roster
            && snapshot.mode == after.mode
            && snapshot.quorum == after.quorum
            && snapshot.leader_seed == after.leader_seed
            && certificate_decision(parent)
                == certificate_decision(&previous.finality_artifact.commit_qc),
        "epoch advancement changed retained custody or its exact certified parent boundary",
    )
}

fn make_retention_receipt(
    previous: &BridgeFinalityProof,
    next: &BridgeFinalityProof,
    chain: &[KagemushaMintFinalityEpochAuthorizationV1],
    previous_cursor_id: Option<String>,
) -> Result<RetentionCompletionV1> {
    verify_retained_boundary(previous, next)?;
    let before = &previous.finality_artifact.height_context;
    let current = &next.finality_artifact.height_context;
    require(
        chain.last() == Some(&current.kagemusha_mint_finality_authorization)
            && chain
                .first()
                .is_some_and(|authorization| authorization.epoch == 0)
            && chain.len()
                == usize::try_from(current.epoch)
                    .ok()
                    .and_then(|epoch| epoch.checked_add(1))
                    .ok_or_else(|| eyre!("retention chain length overflows"))?,
        "retention receipt does not carry its complete authorization chain",
    )?;
    for pair in chain.windows(2) {
        pair[1]
            .validate_successor(&pair[0])
            .map_err(|error| eyre!("retention chain linkage failed: {error}"))?;
        require(
            pair[1].decision == KagemushaMintFinalityEpochDecisionV1::Retain,
            "retention chain contains a prepared activation or cancellation",
        )?;
    }
    let authorization = &current.kagemusha_mint_finality_authorization;
    let mut receipt = RetentionCompletionV1 {
        schema_version: 1,
        network_id: current.network_id,
        anchor_height: 1,
        observed_height: current.height,
        completed_epoch: current.epoch,
        authority_generation: authorization.authority_generation,
        authority_id: authorization.authority_id,
        authorization_id: authorization
            .authorization_id()
            .map_err(|error| eyre!(error))?,
        beacon_binding: authorization.beacon,
        authorization_chain: chain.to_vec(),
        boundary_context_id: before.id(),
        successor_context_id: current.id(),
        parent_decision: certificate_decision(&previous.finality_artifact.commit_qc),
        authority: current.kagemusha_mint_finality_authority.clone(),
        previous_cursor_id,
        cursor_id: String::new(),
    };
    receipt.cursor_id = receipt.computed_id()?;
    Ok(receipt)
}

/// Only verified observer prefixes enter this function; raw contexts cannot grant retention.
fn retention_receipts(
    height: &VerifiedCommittedHeightV1,
    stop: u64,
) -> Result<Vec<RetentionCompletionV1>> {
    let first = height
        .proof_at(NonZeroU64::new(1).expect("nonzero genesis height"))
        .ok_or_else(|| eyre!("authenticated retention prefix omits genesis"))?;
    let genesis = &first.finality_artifact.height_context;
    require(
        genesis.epoch == 0
            && genesis.height == 1
            && genesis.kagemusha_mint_finality_authorization.decision
                == KagemushaMintFinalityEpochDecisionV1::Genesis,
        "retention prefix has no canonical genesis authorization",
    )?;
    let mut chain = vec![genesis.kagemusha_mint_finality_authorization];
    let mut previous = first;
    let mut receipts: Vec<RetentionCompletionV1> = Vec::new();
    for at in 2..=height.committed_height().get() {
        let proof = height
            .proof_at(NonZeroU64::new(at).ok_or_else(|| eyre!("zero retention height"))?)
            .ok_or_else(|| eyre!("authenticated retention prefix is not contiguous"))?;
        let before = &previous.finality_artifact.height_context;
        let current = &proof.finality_artifact.height_context;
        if current.epoch == before.epoch {
            require(
                current.kagemusha_mint_finality_authorization
                    == before.kagemusha_mint_finality_authorization
                    && current.kagemusha_mint_finality_authority
                        == before.kagemusha_mint_finality_authority
                    && current.roster == before.roster,
                "authority changed inside a retained scheduling epoch",
            )?;
        } else {
            verify_retained_boundary(previous, proof)?;
            chain.push(current.kagemusha_mint_finality_authorization);
            let receipt = make_retention_receipt(
                previous,
                proof,
                &chain,
                receipts.last().map(|receipt| receipt.cursor_id.clone()),
            )?;
            receipts.push(receipt);
            if current.epoch >= stop {
                break;
            }
        }
        previous = proof;
    }
    Ok(receipts)
}

struct RetentionRuntime {
    trust: DeploymentTrustV1,
    clients: [Client; 4],
    observer: AuthenticatedHeightObserverV1,
    network_id: NetworkId,
}

impl RetentionRuntime {
    fn new<C: RunContext>(context: &C, trust: DeploymentTrustV1) -> Result<Self> {
        require(
            !context.input_instructions()
                && !context.output_instructions()
                && context.transaction_metadata().is_none(),
            "read-only retention cannot combine transaction instructions or metadata",
        )?;
        require(
            context.config().chain.to_string() == "fc56984b-2be7-431d-840e-21514d1883f0"
                && context.config().account_chain_discriminant == 369,
            "retention requires the canonical Taira account profile",
        )?;
        let network_id = context.config().network_id;
        trust.validate(network_id)?;
        let clients = trust
            .peers
            .iter()
            .map(|peer| {
                let mut config = context.config().clone();
                config.torii_api_url = peer.torii_origin.parse()?;
                let mut builder = Client::builder(config);
                builder.operator_key_pair = context.operator_key_pair().cloned();
                builder.build().map_err(Into::into)
            })
            .collect::<Result<Vec<_>>>()?
            .try_into()
            .map_err(|_| eyre!("retention requires exactly four admitted observation peers"))?;
        let observer = AuthenticatedHeightObserverV1::from_trust(&trust, network_id)?;
        Ok(Self {
            trust,
            clients,
            observer,
            network_id,
        })
    }

    fn checkpoint(&mut self, deadline: Instant) -> Result<VerifiedCommittedHeightV1> {
        loop {
            require_epoch_budget(deadline, "authenticating committed retention")?;
            match self
                .observer
                .observe_current(&self.clients, 369, deadline)?
            {
                HeightObservationV1::Verified(value) => return Ok(value),
                HeightObservationV1::Pending => {
                    std::thread::sleep(operation_poll_delay(deadline, Instant::now()))
                }
            }
        }
    }
}

fn retention_path(parent: &Path, network: NetworkId) -> PathBuf {
    parent.join(format!("retention-{network}"))
}

fn retain_receipts(
    journal: &Journal,
    receipts: &[RetentionCompletionV1],
    trust: &DeploymentTrustV1,
    network: NetworkId,
) -> Result<()> {
    let original = match journal.optional_json::<DeploymentTrustV1>("trust.json")? {
        Some(original) => original,
        None => {
            require_uninitialized_journal(journal, &[])?;
            journal.install_json("trust.json", trust)?;
            trust.clone()
        }
    };
    validate_observation_trust(&original, trust, network)?;
    if let Some(cursor) = journal.optional_json::<RetentionCursorV1>("cursor.json")? {
        require(
            cursor.schema_version == 1
                && cursor.network_id == network
                && receipts.iter().any(|receipt| {
                    receipt.completed_epoch == cursor.completed_epoch
                        && receipt.cursor_id == cursor.cursor_id
                }),
            "retained cursor differs from freshly authenticated authorization history",
        )?;
    }
    for receipt in receipts {
        let name = format!("epoch-{}.json", receipt.completed_epoch);
        match journal.optional_json::<RetentionCompletionV1>(&name)? {
            Some(retained) => require(
                retained == *receipt,
                "immutable retained epoch evidence changed",
            )?,
            None => journal.install_json(&name, receipt)?,
        }
    }
    if let Some(receipt) = receipts.last() {
        let next_cursor = RetentionCursorV1 {
            schema_version: 1,
            network_id: network,
            completed_epoch: receipt.completed_epoch,
            cursor_id: receipt.cursor_id.clone(),
        };
        if journal
            .optional_json::<RetentionCursorV1>("cursor.json")?
            .as_ref()
            != Some(&next_cursor)
        {
            supervisor::replace_retention_cursor(journal, &next_cursor)?;
        }
    }
    Ok(())
}

fn read_retained_completion(
    parent: &Path,
    network: NetworkId,
    receipt: &RetentionCompletionV1,
    trust: &DeploymentTrustV1,
) -> Result<()> {
    let root = retention_path(parent, network);
    let original: DeploymentTrustV1 = supervisor::read_pinned_json(&root.join("trust.json"))?;
    validate_observation_trust(&original, trust, network)?;
    let retained: RetentionCompletionV1 = supervisor::read_pinned_json(
        &root.join(format!("epoch-{}.json", receipt.completed_epoch)),
    )?;
    require(
        retained == *receipt && retained.cursor_id == retained.computed_id()?,
        "retained epoch receipt differs from freshly authenticated retention",
    )
}

fn maintain<C: RunContext>(
    context: &mut C,
    args: MaintainArgs,
    persist: bool,
) -> Result<RetentionCompletionV1> {
    let deadline = operation_deadline(args.timeout_ms)?;
    let trust: DeploymentTrustV1 = json::from_slice(&read_public_input(&args.trust)?)?;
    let mut runtime = RetentionRuntime::new(context, trust)?;
    let journal = if persist {
        let path = retention_path(&args.journal_dir, runtime.network_id);
        let create = match fs::symlink_metadata(&path) {
            Ok(_) => false,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
            Err(error) => return Err(error.into()),
        };
        Some(open_initializing_journal(&path, create)?)
    } else {
        None
    };
    loop {
        let height = runtime.checkpoint(deadline)?;
        // Rebuild the complete deterministic prefix even on restart; journal state
        // is never an authority for skipping a signature, epoch, or parent link.
        let receipts = retention_receipts(&height, u64::MAX)?;
        if let Some(journal) = &journal {
            retain_receipts(journal, &receipts, &runtime.trust, runtime.network_id)?;
        }
        if let Some(receipt) = receipts
            .iter()
            .find(|receipt| receipt.completed_epoch == args.stop_after_epoch)
        {
            if !persist {
                read_retained_completion(
                    &args.journal_dir,
                    runtime.network_id,
                    receipt,
                    &runtime.trust,
                )?;
            }
            context.print_data(receipt)?;
            return Ok(receipt.clone());
        }
        std::thread::sleep(operation_poll_delay(deadline, Instant::now()));
    }
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
        match self {
            Self::Maintain(args) => maintain(context, args, true).map(|_| ()),
            Self::Status(args) => maintain(context, args, false).map(|_| ()),
            Self::Supervise(args) => supervisor::run(context, args),
            Self::SupervisorStatus(args) => supervisor::status(context, args),
        }
    }
}

#[cfg(all(test, unix))]
#[path = "taira_epoch_retention_tests.rs"]
mod tests;
