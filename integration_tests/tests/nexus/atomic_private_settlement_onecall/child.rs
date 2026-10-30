//! One fresh SDK process per continuation; only original caller inputs enter on stdin.
//!
//! Child success is a normal process exit, not an injected crash. Journal acknowledgement
//! loss and actual validator restarts are recorded as distinct failure boundaries.
use super::*;
use iroha::client::PrivateSettlementRunOptions;
use iroha::workflow::{
    Digest32, FileWorkflowStorage, WorkflowId, WorkflowJournalV1, WorkflowRunnerLease,
    WorkflowStorage, WorkflowStorageDurability, WorkflowStorageError, WorkflowStorageErrorKind,
};
use iroha_data_model::{
    nexus::{PrivateSettlementProofIntentV1, PrivateSettlementReceiptV1},
    parameter::system::ConsensusMode,
    sumeragi_finality::{
        SignedGenesisPinsV1, SumeragiFinalityVerifier, authenticate_signed_genesis_v1,
    },
};
use iroha_private_settlement::{
    FilePrivateSettlementArtifacts, OneCallPrivateSettlementOperation,
    OneCallPrivateSettlementRequest, PrivateSettlementBusinessTerms,
};
use std::{
    io::Read,
    path::Path,
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

const CHILD_INPUT_MAX: u64 = 32 * 1024 * 1024;
pub(super) const CHILD_REPORT_MAX: usize = 512 * 1024;
pub(super) const CHILD_LOG_MAX: usize = 1024 * 1024;
const ARTIFACT_FILE_MAX: usize = 32 * 1024 * 1024 + 28;
const TOTAL_ARTIFACT_BYTES: usize = 128 * 1024 * 1024;
const FINALITY_FILE_MAX: usize = TOTAL_ARTIFACT_BYTES + 28;

/// Closed fixture controls, never protocol or node configuration switches.
#[derive(Clone, Copy, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "integration_tests::private_onecall::Mode")]
pub(super) enum Mode {
    ReadOnly,
    PrepareOnly,
    PrepareCached,
    Unaudited,
    Finish,
    FinishCached,
    RecoverCached,
    Checkpoint(u8),
}
impl Mode {
    pub(super) fn label(self) -> String {
        match self {
            Self::ReadOnly => "read_only".to_owned(),
            Self::PrepareOnly => "prepare_only".to_owned(),
            Self::PrepareCached => "prepare_cached_zero_reads".to_owned(),
            Self::Unaudited => "unaudited".to_owned(),
            Self::Finish => "finish".to_owned(),
            Self::FinishCached => "finish_cached_zero_reads".to_owned(),
            Self::RecoverCached => "recover_cached_zero_reads".to_owned(),
            Self::Checkpoint(step) => format!("checkpoint_{step}"),
        }
    }
    pub(super) fn finality_read_allowance(self) -> u16 {
        if matches!(
            self,
            Self::PrepareCached | Self::FinishCached | Self::RecoverCached
        ) {
            0
        } else {
            PrivateSettlementRunOptions::default().max_finality_proof_reads
        }
    }
    pub(super) fn requires_unchanged_effects(self) -> bool {
        matches!(
            self,
            Self::ReadOnly | Self::PrepareCached | Self::FinishCached | Self::RecoverCached
        )
    }
    fn validate(self) -> Result<()> {
        ensure!(
            !matches!(self, Self::Checkpoint(step) if step > 3),
            "unknown cold SDK phase"
        );
        Ok(())
    }
}

/// Data-only transport of separately selected pins; decoding cannot confer authority.
#[derive(Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "integration_tests::private_onecall::Pins")]
pub(super) struct Pins {
    chain: String,
    network: iroha_data_model::NetworkId,
    genesis_hash: HashOf<BlockHeader>,
    signed_genesis_sha256: [u8; 32],
    genesis_key: iroha_crypto::PublicKey,
    roster: Vec<PeerId>,
    mode: ConsensusMode,
}
impl Pins {
    pub(super) fn from_network(network: &Network, wire: &[u8]) -> Result<Self> {
        use sha2::{Digest as _, Sha256};
        let mut roster = network
            .validators()
            .iter()
            .map(|peer| peer.id())
            .collect::<Vec<_>>();
        roster.sort();
        Ok(Self {
            chain: network.chain_id().to_string(),
            network: network.network_id(),
            genesis_hash: network.genesis().0.hash(),
            signed_genesis_sha256: Sha256::digest(wire).into(),
            genesis_key: iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_KEYPAIR
                .public_key()
                .clone(),
            roster,
            mode: ConsensusMode::Npos,
        })
    }
    fn selected(&self) -> SignedGenesisPinsV1 {
        SignedGenesisPinsV1 {
            chain_id: self.chain.clone(),
            network_id: self.network,
            genesis_hash: self.genesis_hash,
            signed_genesis_sha256: self.signed_genesis_sha256,
            genesis_public_key: self.genesis_key.clone(),
            roster: self.roster.clone(),
            mode: self.mode,
        }
    }
}
#[derive(Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "integration_tests::private_onecall::ChildInput")]
pub(super) struct ChildInput {
    pub(super) intent: PrivateSettlementProofIntentV1,
    pub(super) original_openings: Vec<wallet::OriginalOpening>,
    pub(super) original_genesis: Vec<u8>,
    pub(super) pins: Pins,
    pub(super) endpoint: String,
    pub(super) endpoints: Vec<Vec<String>>,
    pub(super) account_chain_discriminant: u16,
    pub(super) custody_path: String,
    pub(super) invocation: [u8; 32],
    pub(super) mode: Mode,
    pub(super) sequence: u64,
    pub(super) deadline_unix_ms: u64,
    pub(super) schema: Hash,
    pub(super) minimum_context_time_ms: u64,
    // Runtime-only: original caller retains this key; stdin is its only serialized transport.
    pub(super) artifact_key: [u8; 32],
}
impl Drop for ChildInput {
    fn drop(&mut self) {
        use zeroize::Zeroize as _;
        self.artifact_key.zeroize();
    }
}
impl ChildInput {
    pub(super) fn binding(&self) -> Result<Hash> {
        let mut binding_input = self.clone();
        binding_input.artifact_key.fill(0);
        let bytes = zeroize::Zeroizing::new(norito::encode_canonical(&binding_input)?);
        Ok(Hash::new(&bytes))
    }
    fn report_name(&self) -> String {
        format!(
            "child-{:03}-{}.norito",
            self.sequence,
            hex::encode(self.invocation)
        )
    }
}
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "integration_tests::private_onecall::ChildReport")]
pub(super) struct ChildReport {
    pub(super) pid: u32,
    binding: Hash,
    pub(super) preparation_id: [u8; 32],
    pub(super) settlement_id: Option<[u8; 32]>,
    pub(super) provisional: Option<ProvisionalPublic>,
    pub(super) prepared: Option<PreparedPublic>,
    pub(super) receipt: Option<PrivateSettlementReceiptV1>,
    pub(super) evidence: Option<iroha_data_model::query::CommittedTransaction>,
    pub(super) openings: u64,
    pub(super) stages: u64,
    pub(super) reservations: u64,
    pub(super) reconciliations: u64,
}

pub(super) fn unix_ms() -> Result<u64> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis()
        .try_into()?)
}
fn remaining(input: &ChildInput) -> Result<Duration> {
    let remaining = input
        .deadline_unix_ms
        .checked_sub(unix_ms()?)
        .filter(|v| *v > 0)
        .ok_or_else(|| eyre!("original cold SDK deadline elapsed"))?;
    ensure!(
        remaining <= COLD_CASE_LIMIT.as_millis() as u64,
        "cold SDK deadline exceeds original finite bound"
    );
    Ok(Duration::from_millis(remaining))
}
fn artifact_effect_snapshot(
    root: &iroha_fs::PrivateDirectory,
    original: &WorkflowCustodySnapshot,
) -> Result<Vec<(String, Hash)>> {
    let folder = "encrypted-artifacts";
    let directory = iroha_fs::PrivateDirectory::open_exact(root.path().join(folder))?;
    let mut admitted = std::collections::BTreeMap::new();
    if let Some(round) = original.proof_round {
        let round = hex::encode(round);
        admitted.insert(format!("{round}.contexts"), (ARTIFACT_FILE_MAX, true));
        admitted.insert(
            format!("{round}.preparation-finality"),
            (ARTIFACT_FILE_MAX, false),
        );
        for ordinal in 0..PARTICIPANT_COUNT {
            admitted.insert(
                format!("{round}-{ordinal:03}.prepared"),
                (ARTIFACT_FILE_MAX, true),
            );
        }
    }
    if let Some(workflow) = original.settlement {
        let workflow = hex::encode(workflow.as_bytes());
        admitted.insert(
            format!("{workflow}.settlement-finality"),
            (FINALITY_FILE_MAX, false),
        );
        admitted.insert(format!("{workflow}.settlement-finality.lock"), (0, false));
    }
    let mut names = Vec::new();
    let mut plaintext_bytes = 0_u64;
    // One context, three proof artifacts, one preparation prefix, and one prepared
    // finality record plus its CAS lock. No unrelated file or historical record fits.
    directory.visit_private_files(7, |name, metadata| {
        let name = name
            .to_str()
            .ok_or_else(|| std::io::Error::other("invalid custody name"))?;
        let (maximum, immutable) = admitted.get(name).ok_or_else(|| {
            std::io::Error::other("custody name differs from original workflow/round")
        })?;
        if metadata.len() > *maximum as u64 {
            return Err(std::io::Error::other("original custody file exceeds bound"));
        }
        if *maximum != 0 {
            let length = metadata
                .len()
                .checked_sub(28)
                .ok_or_else(|| std::io::Error::other("original custody lacks AEAD envelope"))?;
            plaintext_bytes = plaintext_bytes
                .checked_add(length)
                .filter(|total| *total <= TOTAL_ARTIFACT_BYTES as u64)
                .ok_or_else(|| {
                    std::io::Error::other("original aggregate custody bound exceeded")
                })?;
        }
        if *immutable {
            names.push(name.to_owned());
        }
        Ok(())
    })?;
    names.sort();
    names
        .into_iter()
        .map(|name| {
            Ok((
                format!("{folder}/{name}"),
                Hash::new(&*directory.read(&name, ARTIFACT_FILE_MAX)?),
            ))
        })
        .collect()
}
pub(super) fn artifact_snapshot(root: &iroha_fs::PrivateDirectory) -> Result<Vec<(String, Hash)>> {
    let original = workflow_snapshot(root)?;
    let files = artifact_effect_snapshot(root, &original)?;
    ensure!(
        files.len() == 4
            && files
                .iter()
                .filter(|(name, _)| name.ends_with(".contexts"))
                .count()
                == 1
            && files
                .iter()
                .filter(|(name, _)| name.ends_with(".prepared"))
                .count()
                == 3,
        "cold N3 must retain one original context and three genuine prepared artifacts"
    );
    Ok(files)
}
struct WorkflowCustodySnapshot {
    effects: Vec<(String, Hash)>,
    proof_round: Option<Digest32>,
    settlement: Option<WorkflowId>,
}
fn workflow_snapshot(root: &iroha_fs::PrivateDirectory) -> Result<WorkflowCustodySnapshot> {
    let folder = "journals";
    let directory = root.open_child(folder)?;
    let mut names = directory.entries(4)?;
    names.sort();
    let mut result = Vec::new();
    let mut worker_round = None;
    let mut phase_binding = None;
    let mut settlement = None;
    let mut lease_rows = Vec::new();
    let mut drivers = std::collections::BTreeSet::new();
    for name in names {
        let text = name
            .to_str()
            .ok_or_else(|| eyre!("invalid workflow custody name"))?;
        ensure!(
            text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit()),
            "unexpected workflow directory"
        );
        let workflow = directory.open_child(&name)?;
        let files = workflow.entries(3)?;
        ensure!(
            files.iter().all(|file| ["journal", "lock", "runner"]
                .iter()
                .any(|expected| file.as_os_str() == *expected)),
            "unexpected workflow custody file"
        );
        let mut id = [0; 32];
        hex::decode_to_slice(text, &mut id)?;
        ensure!(
            text == hex::encode(id),
            "noncanonical workflow directory identity"
        );
        let storage = FileWorkflowStorage::open_or_create(directory.path())?;
        let original_bytes = storage.load(&WorkflowId::from_bytes(id))?;
        if let Some(bytes) = original_bytes {
            let digest = {
                let journal = WorkflowJournalV1::decode_record(&bytes)?;
                ensure!(
                    matches!(
                        journal.identity.driver.id.as_str(),
                        "private_settlement_preparation" | "private_settlement"
                    ) && drivers.insert(journal.identity.driver.id.clone()),
                    "unexpected or duplicate original workflow driver"
                );
                if journal.identity.driver.id == "private_settlement_preparation" {
                    worker_round = journal
                        .steps
                        .first()
                        .and_then(|step| step.round.as_ref())
                        .and_then(|round| round.round_id);
                } else {
                    let network: iroha_data_model::NetworkId = journal.identity.network.parse()?;
                    phase_binding =
                        Some((*network.as_bytes(), journal.identity.request_context_digest));
                    settlement = Some(WorkflowId::from_bytes(id));
                }
                // Recovery may reconcile observations but may not open or dispatch effects.
                journal_effect_digest(&journal)?
            };
            result.push((format!("{folder}/{text}/journal"), digest));
        } else {
            ensure!(
                files.len() == 2
                    && files.iter().all(|file| ["lock", "runner"]
                        .iter()
                        .any(|expected| file.as_os_str() == *expected)),
                "unexpected journal-less custody row"
            );
            lease_rows.push(text.to_owned());
        }
    }
    {
        for (name, role) in
            require_original_worker_leases(worker_round, phase_binding, &lease_rows)?
        {
            // Pin the storage-owned lease identity. Lock contents are not application state.
            result.push((
                format!("{folder}/{name}/{role}"),
                Hash::new(name.as_bytes()),
            ));
        }
        result.sort_by(|left, right| left.0.cmp(&right.0));
    }
    Ok(WorkflowCustodySnapshot {
        effects: result,
        proof_round: worker_round,
        settlement,
    })
}
fn require_original_worker_leases(
    round: Option<Digest32>,
    phase: Option<(Digest32, Digest32)>,
    leases: &[String],
) -> Result<Vec<(String, &'static str)>> {
    let mut expected = std::collections::BTreeMap::new();
    if let Some(round) = round {
        expected.insert(
            hex::encode(iroha::workflow::domain_digest(
                b"iroha:private-settlement:proof-worker:v1\0",
                &[&round],
            )),
            "proof-worker-lease",
        );
    }
    if let Some((network, request)) = phase {
        expected.insert(
            hex::encode(iroha::workflow::domain_digest(
                b"iroha:private-settlement:global-phase-worker:v1\0",
                &[&network, &request],
            )),
            "phase-worker-lease",
        );
    }
    ensure!(
        leases.iter().eq(expected.keys()),
        "worker leases do not bind the original preparation round and prepared request"
    );
    Ok(expected.into_iter().collect())
}
// Snapshots run only before or after a joined child. Native in-flight staging is owned and
// bounded by FilePrivateSettlementWallet; a completed child must leave exactly these roles.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct WalletSnapshot {
    pub(super) envelope: Option<(usize, Hash)>,
    pub(super) lock: bool,
}
pub(super) fn wallet_snapshot(root: &iroha_fs::PrivateDirectory) -> Result<WalletSnapshot> {
    let directory = root.open_child("software-wallet")?;
    let names = directory.entries(2)?;
    ensure!(
        names.iter().all(
            |name| ["authorized-inputs.wallet", "authorized-inputs.wallet.lock"]
                .iter()
                .any(|expected| name.as_os_str() == *expected)
        ),
        "unexpected software wallet custody file"
    );
    let lock = directory
        .read_optional("authorized-inputs.wallet.lock", 0)?
        .is_some();
    let envelope = directory
        .read_optional(
            "authorized-inputs.wallet",
            wallet::MAX_WALLET_INDEX_BYTES + 28,
        )?
        .map(|bytes| (bytes.len(), Hash::new(&bytes)));
    ensure!(
        envelope.is_none_or(|(length, _)| length >= 28) && (envelope.is_none() || lock),
        "incomplete software wallet custody"
    );
    Ok(WalletSnapshot { envelope, lock })
}
pub(super) struct CustodySnapshot {
    pub(super) effects: Vec<(String, Hash)>,
    pub(super) wallet: WalletSnapshot,
}
pub(super) fn custody_snapshot(root: &iroha_fs::PrivateDirectory) -> Result<CustodySnapshot> {
    // Exact transaction identities and immutable artifacts remain independent of the sole
    // encrypted pending-wallet record, whose one success transition is checked separately.
    let original = workflow_snapshot(root)?;
    let mut effects = artifact_effect_snapshot(root, &original)?;
    effects.extend(original.effects);
    Ok(CustodySnapshot {
        effects,
        wallet: wallet_snapshot(root)?,
    })
}

struct CheckpointStorage {
    inner: FileWorkflowStorage,
    selected_step: Option<usize>,
    interrupted: AtomicBool,
}
impl WorkflowStorage for CheckpointStorage {
    fn durability(&self) -> WorkflowStorageDurability {
        self.inner.durability()
    }
    fn load(&self, id: &WorkflowId) -> std::result::Result<Option<Vec<u8>>, WorkflowStorageError> {
        self.inner.load(id)
    }
    fn acquire_runner(
        &self,
        id: &WorkflowId,
    ) -> std::result::Result<Box<dyn WorkflowRunnerLease>, WorkflowStorageError> {
        self.inner.acquire_runner(id)
    }
    fn compare_and_store(
        &self,
        id: &WorkflowId,
        expected: Option<&Digest32>,
        bytes: &[u8],
    ) -> std::result::Result<(), WorkflowStorageError> {
        let checkpoint = self.selected_step.is_some_and(|step| {
            WorkflowJournalV1::decode_record(bytes).is_ok_and(|journal| {
                journal.identity.driver.id == "private_settlement"
                    && journal
                        .steps
                        .get(step)
                        .is_some_and(|record| record.is_done())
            })
        });
        self.inner.compare_and_store(id, expected, bytes)?;
        if checkpoint && !self.interrupted.swap(true, Ordering::SeqCst) {
            return Err(WorkflowStorageError::new(
                WorkflowStorageErrorKind::WriteUncertain,
                "test acknowledgement loss after exact durable settlement checkpoint",
            ));
        }
        Ok(())
    }
}
/// Bind external-effect coordinates while permitting read-only observation reconciliation.
/// The private evidence projection includes both native transaction identity hashes.
fn journal_effect_digest(journal: &WorkflowJournalV1) -> Result<Hash> {
    let effects = journal
        .steps
        .iter()
        .map(|step| {
            (
                step.transaction.as_ref().map(|tx| {
                    tx.attempts
                        .iter()
                        .map(|attempt| {
                            (
                                attempt.ordinal,
                                attempt.identity.map(|identity| {
                                    (identity.signed_transaction_hash, identity.entrypoint_hash)
                                }),
                                attempt.dispatch_ordinal,
                            )
                        })
                        .collect::<Vec<_>>()
                }),
                step.round
                    .as_ref()
                    .map(|round| (round.round_id, round.start_ordinal)),
            )
        })
        .collect::<Vec<_>>();
    Ok(Hash::new(norito::encode_canonical(&(
        journal.identity.clone(),
        effects,
    ))?))
}

fn client(input: &ChildInput) -> Result<iroha::client::AccountClient> {
    use iroha_config_base::{
        read::ConfigReader,
        toml::{TomlSource, Writer as TomlWriter},
    };
    let mut table = toml::Table::new();
    TomlWriter::new(&mut table)
        .write("chain", input.pins.chain.clone())
        .write("network_id", input.intent.network_id.to_string())
        .write(
            ["account", "chain_discriminant"],
            i64::from(input.account_chain_discriminant),
        )
        .write(
            ["account", "public_key"],
            ALICE_KEYPAIR.public_key().to_string(),
        )
        .write(
            ["account", "private_key"],
            iroha_crypto::ExposedPrivateKey(ALICE_KEYPAIR.private_key().clone()).to_string(),
        )
        .write("torii_url", input.endpoint.clone())
        .write("torii_request_timeout_ms", 300_000_i64)
        .write(["transaction", "status_timeout_ms"], 300_000_i64)
        .write(["transaction", "time_to_live_ms"], 600_000_i64);
    let config = ConfigReader::new()
        .without_env()
        .with_toml_source(TomlSource::inline(table))
        .read_and_complete::<iroha::config::UserConfig>()
        .map_err(|error| eyre!("cold SDK client configuration: {error:?}"))?
        .parse()
        .map_err(|error| eyre!("cold SDK client configuration values: {error:?}"))?;
    Ok(SdkClient::builder(config).build()?.account_client()?)
}
pub(super) fn verifier(input: &ChildInput) -> Result<SumeragiFinalityVerifier> {
    ensure!(
        input.intent.network_id == input.pins.network,
        "caller intent substituted original network pin"
    );
    let pins = input.pins.selected();
    let authenticated = authenticate_signed_genesis_v1(&input.original_genesis, &pins)?;
    Ok(SumeragiFinalityVerifier::new(
        authenticated.block(),
        &pins.chain_id,
        authenticated.validators().to_vec(),
    )?)
}
fn options(input: &ChildInput) -> Result<PrivateSettlementRunOptions> {
    Ok(PrivateSettlementRunOptions {
        workflow: iroha::workflow::WorkflowRunOptions {
            observation_timeout: remaining(input)?.min(FINALITY_TIMEOUT),
            poll_interval: POLL_INTERVAL,
            retransmit_identical_bytes: false,
            retry_rejected_step: false,
        },
        max_finality_proof_reads: input.mode.finality_read_allowance(),
    })
}
fn execute(input: ChildInput) -> Result<ChildReport> {
    input.mode.validate()?;
    remaining(&input)?;
    let original_verifier = verifier(&input)?; // Authenticate independently selected pins before opening any wallet.
    let root = Path::new(&input.custody_path);
    iroha_fs::PrivateDirectory::open_exact(root)?;
    let wallet = Arc::new(wallet::OriginalWallet::open(
        input.original_openings.clone(),
        &root.join("software-wallet"),
        &input.artifact_key,
    )?);
    let storage = Arc::new(CheckpointStorage {
        inner: FileWorkflowStorage::open_or_create(root.join("journals"))?,
        selected_step: match input.mode {
            Mode::Checkpoint(step) => Some(usize::from(step)),
            _ => None,
        },
        interrupted: AtomicBool::new(false),
    });
    let artifacts = Arc::new(FilePrivateSettlementArtifacts::open(
        root.join("encrypted-artifacts"),
        &input.artifact_key,
    )?);
    let finality = artifacts.finality().clone();
    let terms = input
        .intent
        .legs
        .iter()
        .enumerate()
        .map(|(ordinal, _)| {
            let data = default_private_settlement_leg_data(ordinal);
            PrivateSettlementBusinessTerms {
                asset: cbdc_asset_definition_id(ordinal),
                payer: AccountId::new(data.payer.public_key().clone()),
                recipient: AccountId::new(data.recipient.public_key().clone()),
                amount: data.amount,
                reimbursement: if ordinal == usize::from(input.intent.reimbursement_leg_ordinal) {
                    5
                } else {
                    0
                },
                memo: data.memo,
            }
        })
        .collect();
    let endpoints = input
        .endpoints
        .iter()
        .map(|row| {
            row.iter()
                .map(|url| Url::parse(url).map_err(eyre::Report::new))
                .collect::<Result<Vec<_>>>()
        })
        .collect::<Result<Vec<_>>>()?;
    let request = OneCallPrivateSettlementRequest::new(
        input.intent.clone(),
        terms,
        input.schema,
        input.minimum_context_time_ms,
        endpoints,
        PrivateSettlementCapsulePaddingV1::KiB16,
        input.intent.expiry_height + SIDECAR_RETENTION_BLOCKS,
        PRIVATE_SETTLEMENT_MAX_RECEIPT_BYTES_V1 as u64,
        32 * 1024 * 1024,
        TOTAL_ARTIFACT_BYTES,
    )?;
    let account = client(&input)?;
    let operation = OneCallPrivateSettlementOperation::new(
        account.clone(),
        request,
        storage.clone(),
        artifacts,
        wallet.clone(),
        original_verifier,
    )?;
    let preparation_id = *operation.id().as_bytes();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let recovered = runtime.block_on(operation.recover(&options(&input)?))?;
    ensure!(
        wallet.openings.load(Ordering::SeqCst) == 0
            && wallet.stages.load(Ordering::SeqCst) == 0
            && wallet.reservations_called.load(Ordering::SeqCst) == 0,
        "cold readonly recovery cannot reopen wallet secrets or acquire notes"
    );
    ensure!(
        wallet.reconciliations.load(Ordering::SeqCst) == usize::from(recovered.outcome.is_some()),
        "cold recovery must reconcile exactly verified successful original custody"
    );
    let verified = if matches!(input.mode, Mode::ReadOnly | Mode::RecoverCached) {
        recovered.outcome
    } else if matches!(input.mode, Mode::PrepareOnly | Mode::PrepareCached) {
        ensure!(
            recovered.outcome.is_none(),
            "preparation unexpectedly settled"
        );
        wallet.enable_explicit_continuation();
        let original = runtime.block_on(operation.prepare(&options(&input)?))?;
        ensure!(original.manifest().proof_intent() == input.intent);
        drop(original);
        None
    } else {
        wallet.enable_explicit_continuation();
        let result = runtime.block_on(operation.run(&options(&input)?));
        if matches!(input.mode, Mode::Checkpoint(_)) {
            ensure!(
                storage.interrupted.load(Ordering::SeqCst) && result.is_err(),
                "explicit continuation did not reach its original durable phase checkpoint"
            );
            None
        } else if input.mode == Mode::Unaudited {
            ensure!(
                matches!(&result, Err(iroha_private_settlement::OneCallPrivateSettlementError::Settlement(iroha::client::PrivateSettlementError::Workflow(error))) if error.step_index == Some(1) && error.code == iroha::workflow::WorkflowErrorCode::LocalResourceUnavailable),
                "unaudited original must stop at Prepare: {result:?}"
            );
            None
        } else {
            ensure!(
                matches!(input.mode, Mode::Finish | Mode::FinishCached),
                "unknown cold SDK fixture mode"
            );
            Some(result?.outcome)
        }
    };
    let verified = verified
        .map(|outcome| match outcome {
            iroha::client::VerifiedPrivateSettlementOutcome::Settled(receipt) => Ok(receipt),
            iroha::client::VerifiedPrivateSettlementOutcome::Aborted(_) => {
                Err(eyre!("original payment aborted instead of settling"))
            }
        })
        .transpose()?;
    // The same completed original preparation must be readable without another native proof.
    let mut cached_options = options(&input)?;
    cached_options.max_finality_proof_reads = 0;
    let prepared = runtime.block_on(operation.prepared_request(&cached_options))?;
    let provisional = prepared
        .as_ref()
        .map(|request| ProvisionalPublic::from_materials(request.materials()))
        .transpose()?;
    let (settlement_id, public) = if let Some(prepared) = &prepared {
        let settlement = account.private_settlement(
            prepared.clone(),
            storage.clone(),
            finality,
            verifier(&input)?,
        )?;
        let id = settlement.id();
        let journal = storage.load(&id)?;
        let public = if let Some(journal) = journal {
            ensure!(
                !matches!(input.mode, Mode::PrepareOnly | Mode::PrepareCached),
                "preparation-only created a settlement journal"
            );
            let journal = WorkflowJournalV1::decode_record(&journal)?;
            let certificates = journal.steps[0]
                .round
                .as_ref()
                .and_then(|round| round.transcript.as_deref())
                .ok_or_else(|| eyre!("original availability transcript absent"))?;
            let certificates: Vec<PrivateSettlementSidecarAvailabilityV1> =
                norito::decode_canonical(certificates)?;
            Some(PreparedPublic::from_materials(
                prepared.materials(),
                &certificates,
            )?)
        } else {
            ensure!(
                matches!(
                    input.mode,
                    Mode::ReadOnly | Mode::PrepareOnly | Mode::PrepareCached
                ),
                "settlement continuation omitted original journal"
            );
            None
        };
        (Some(*id.as_bytes()), public)
    } else {
        (None, None)
    };
    let evidence = verified.as_ref().map(|value| value.evidence().clone());
    let receipt = verified.as_ref().map(|value| value.receipt().clone());
    // Drop every material/operation/verifier owner before the process returns its public
    // report. Parent continuation always spawns a new executable and reconstructs inputs.
    drop(prepared);
    drop(verified);
    drop(operation);
    drop(account);
    drop(runtime);
    Ok(ChildReport {
        pid: std::process::id(),
        binding: input.binding()?,
        preparation_id,
        settlement_id,
        provisional,
        prepared: public,
        receipt,
        evidence,
        openings: wallet.openings.load(Ordering::SeqCst) as u64,
        stages: wallet.stages.load(Ordering::SeqCst) as u64,
        reservations: wallet.reservations_called.load(Ordering::SeqCst) as u64,
        reconciliations: wallet.reconciliations.load(Ordering::SeqCst) as u64,
    })
}

pub(super) fn test_filter() -> Result<String> {
    let (_, module) = module_path!()
        .split_once("::")
        .ok_or_else(|| eyre!("cold child module not nested in test crate"))?;
    Ok(format!("{module}::cold_sdk_child"))
}
fn validate_report(report: &ChildReport, input: &ChildInput, pid: u32) -> Result<()> {
    ensure!(
        report.pid == pid && pid != std::process::id() && report.binding == input.binding()?,
        "report substituted original invocation, phase, pins or actual child process"
    );
    ensure!(
        if report.receipt.is_some() {
            (1..=2).contains(&report.reconciliations)
        } else {
            report.reconciliations == 0
        },
        "local wallet consumption must follow a successful verified receipt"
    );
    if input.mode.requires_unchanged_effects() {
        ensure!(report.openings == 0 && report.stages == 0 && report.reservations == 0);
    }
    if matches!(input.mode, Mode::ReadOnly | Mode::RecoverCached) {
        ensure!(
            report.reconciliations == u64::from(report.receipt.is_some()),
            "readonly wallet reconciliation must follow exactly verified success"
        );
    }
    Ok(())
}
pub(super) struct CompletedChild {
    pub(super) report: ChildReport,
    pub(super) report_bytes: Vec<u8>,
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
}
async fn read_log(reader: impl tokio::io::AsyncRead + Unpin) -> Result<Vec<u8>> {
    use tokio::io::AsyncReadExt as _;
    let mut bytes = Vec::new();
    reader
        .take(CHILD_LOG_MAX as u64 + 1)
        .read_to_end(&mut bytes)
        .await?;
    ensure!(
        bytes.len() <= CHILD_LOG_MAX,
        "cold child output exceeds finite diagnostic bound"
    );
    Ok(bytes)
}
fn require_exact_terminal(stdout: &[u8]) -> Result<()> {
    let text = std::str::from_utf8(stdout)?;
    let expected = format!("test {} ... ok", test_filter()?);
    ensure!(
        text.lines()
            .filter(|line| *line == "running 1 test")
            .count()
            == 1
            && text.lines().filter(|line| *line == expected).count() == 1
            && text
                .lines()
                .filter(|line| line.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;"))
                .count()
                == 1,
        "cold child did not execute exactly its selected internal test"
    );
    Ok(())
}
pub(super) fn spawn(
    runtime: &tokio::runtime::Runtime,
    input: &ChildInput,
    deadline: Instant,
) -> Result<CompletedChild> {
    input.mode.validate()?;
    remaining(input)?;
    let original = zeroize::Zeroizing::new(norito::encode_canonical(input)?);
    ensure!(original.len() as u64 <= CHILD_INPUT_MAX);
    let root = iroha_fs::PrivateDirectory::open_exact(&input.custody_path)?;
    let report_name = input.report_name();
    ensure!(
        root.read_optional(&report_name, CHILD_REPORT_MAX)?
            .is_none(),
        "stale child report already exists"
    );
    let (pid, stdout, stderr) = runtime.block_on(async {
        use tokio::io::AsyncWriteExt as _;
        let mut child = tokio::process::Command::new(std::env::current_exe()?)
            .arg(test_filter()?)
            .arg("--exact")
            .arg("--ignored")
            .arg("--test-threads=1")
            .arg("--nocapture")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
        let pid = child
            .id()
            .ok_or_else(|| eyre!("child process identity absent"))?;
        let outcome = tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), async {
            let stdout = child
                .stdout
                .take()
                .ok_or_else(|| eyre!("child stdout unavailable"))?;
            let stderr = child
                .stderr
                .take()
                .ok_or_else(|| eyre!("child stderr unavailable"))?;
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| eyre!("child stdin unavailable"))?;
            let send_and_wait = async {
                stdin.write_all(&original).await?;
                stdin.shutdown().await?;
                drop(stdin);
                let status = child.wait().await?;
                ensure!(status.success(), "fresh cold SDK child failed: {status}");
                Ok::<_, eyre::Report>(())
            };
            let ((), stdout, stderr) =
                tokio::try_join!(send_and_wait, read_log(stdout), read_log(stderr))?;
            Ok::<_, eyre::Report>((stdout, stderr))
        })
        .await;
        if !matches!(outcome, Ok(Ok(_))) {
            // Only this exact owned child is stopped. No node, Cargo, compiler or agent process
            // is targeted. Cleanup gets five seconds solely to reap, never to continue work.
            let _ = child.start_kill();
            tokio::time::timeout(Duration::from_secs(5), child.wait())
                .await
                .wrap_err("cold child cleanup did not reap original process")??;
        }
        let (stdout, stderr) = outcome.wrap_err("original cold SDK deadline elapsed")??;
        require_exact_terminal(&stdout)?;
        Ok::<_, eyre::Report>((pid, stdout, stderr))
    })?;
    ensure!(
        Instant::now() < deadline,
        "child report missed original deadline"
    );
    let bytes = root.read(&report_name, CHILD_REPORT_MAX)?;
    let report: ChildReport = norito::decode_canonical(&bytes)?;
    validate_report(&report, input, pid)?;
    ensure!(
        Instant::now() < deadline,
        "child report read missed original deadline"
    );
    Ok(CompletedChild {
        report,
        report_bytes: bytes.to_vec(),
        stdout,
        stderr,
    })
}
#[test]
#[ignore = "internal bounded-stdin child of the genuine one-call N3 process test"]
fn cold_sdk_child() -> Result<()> {
    let mut bytes = zeroize::Zeroizing::new(Vec::new());
    std::io::stdin()
        .take(CHILD_INPUT_MAX + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        !bytes.is_empty() && bytes.len() as u64 <= CHILD_INPUT_MAX,
        "child requires exact bounded runtime-only request input"
    );
    let input: ChildInput = norito::decode_canonical(&bytes)?;
    drop(bytes);
    input.mode.validate()?;
    remaining(&input)?;
    let root = iroha_fs::PrivateDirectory::open_exact(&input.custody_path)?;
    let name = input.report_name();
    ensure!(
        root.read_optional(&name, CHILD_REPORT_MAX)?.is_none(),
        "child report collision"
    );
    let report = std::thread::Builder::new()
        .name("private-onecall-cold-child".to_owned())
        .stack_size(TEST_STACK_BYTES)
        .spawn(move || execute(input))?
        .join()
        .map_err(|_| eyre!("cold SDK child worker panicked"))??;
    let bytes = norito::encode_canonical(&report)?;
    ensure!(bytes.len() <= CHILD_REPORT_MAX);
    root.write_atomic(name, &bytes, iroha_fs::PublishMode::CreateNew)?;
    Ok(())
}

#[test]
fn cold_child_control_is_closed_and_filter_is_exact() {
    assert!(Mode::Checkpoint(3).validate().is_ok());
    assert!(Mode::Checkpoint(4).validate().is_err());
    assert!(Mode::Checkpoint(u8::MAX).validate().is_err());
    for mode in [Mode::PrepareCached, Mode::FinishCached, Mode::RecoverCached] {
        assert!(mode.validate().is_ok());
        assert_eq!(mode.finality_read_allowance(), 0);
        assert!(mode.requires_unchanged_effects());
    }
    for mode in [
        Mode::ReadOnly,
        Mode::PrepareOnly,
        Mode::Finish,
        Mode::Checkpoint(0),
    ] {
        assert_eq!(mode.finality_read_allowance(), 64);
    }
    assert!(!Mode::PrepareOnly.requires_unchanged_effects());
    assert!(
        test_filter()
            .unwrap()
            .ends_with("atomic_private_settlement_onecall::child::cold_sdk_child")
    );
}

fn test_input(network: &Network, custody_path: String) -> ChildInput {
    let routes = (0..PARTICIPANT_COUNT)
        .map(|ordinal| PrivateSettlementRouteV1 {
            dataspace_id: DataSpaceId::new(ordinal as u64 + 1),
            lane_id: LaneId::new(ordinal as u32 + 1),
            lane_incarnation: hash(0xE0 + ordinal as u8),
        })
        .collect::<Vec<_>>();
    let governed = governed_legs(&routes, 301, 401).unwrap();
    let wire = network.genesis().0.encode_wire().unwrap();
    ChildInput {
        intent: intent(network.network_id(), 301, 401, &governed).unwrap(),
        original_openings: wallet::original_openings(&governed),
        pins: Pins::from_network(network, &wire).unwrap(),
        original_genesis: wire,
        endpoint: "http://127.0.0.1:1/".to_owned(),
        endpoints: vec![vec!["http://127.0.0.1:1/".to_owned()]; 3],
        account_chain_discriminant: network.client().client().account_chain_discriminant(),
        custody_path,
        invocation: [31; 32],
        mode: Mode::ReadOnly,
        sequence: 0,
        deadline_unix_ms: unix_ms().unwrap() + 10_000,
        schema: hash(7),
        minimum_context_time_ms: 0,
        artifact_key: [19; 32],
    }
}
#[test]
fn cold_child_rejects_substituted_signed_genesis_before_wallet_and_binds_reports() {
    let handle = std::thread::Builder::new()
        .stack_size(TEST_STACK_BYTES)
        .spawn(|| {
            let root = tempfile::tempdir().unwrap();
            // These unstarted networks only supply owned signed inputs. Release
            // each network permit before constructing the next fixture, and make
            // the deadline-bearing input last so genesis preparation cannot age it.
            let replacement_genesis = {
                let replacement = n3_smoke_builder(TopologyShape::new(PARTICIPANT_COUNT))
                    .with_base_seed("cold-replacement-pins")
                    .build();
                replacement.genesis().0.encode_wire().unwrap()
            };
            let mut input = {
                let original = n3_smoke_builder(TopologyShape::new(PARTICIPANT_COUNT))
                    .with_base_seed("cold-original-pins")
                    .build();
                test_input(
                    &original,
                    root.path()
                        .join("never-opened")
                        .to_str()
                        .unwrap()
                        .to_owned(),
                )
            };
            assert!(verifier(&input).is_ok());
            let account = client(&input).expect("explicit cold client configuration");
            assert_eq!(account.authority(), &*ALICE_ID);
            assert_eq!(account.chain_id().to_string(), input.pins.chain);
            assert_eq!(account.network_id(), &input.intent.network_id);
            assert_eq!(account.endpoint().as_str(), input.endpoint);
            let endpoint = input.endpoint.clone();
            input.endpoint = "not a Torii URL".to_owned();
            assert!(client(&input).is_err());
            input.endpoint = endpoint;
            let pid = std::process::id().wrapping_add(1);
            let report = ChildReport {
                pid,
                binding: input.binding().unwrap(),
                preparation_id: [1; 32],
                settlement_id: None,
                provisional: None,
                prepared: None,
                receipt: None,
                evidence: None,
                openings: 0,
                stages: 0,
                reservations: 0,
                reconciliations: 0,
            };
            assert!(validate_report(&report, &input, pid).is_ok());
            assert!(validate_report(&report, &input, pid.wrapping_add(1)).is_err());
            input.sequence += 1;
            assert!(validate_report(&report, &input, pid).is_err());
            input.sequence -= 1;
            input.mode = Mode::Finish;
            assert!(validate_report(&report, &input, pid).is_err());
            input.mode = Mode::ReadOnly;
            input.original_openings.swap(0, 1);
            assert!(validate_report(&report, &input, pid).is_err());
            input.original_openings.swap(0, 1);
            let original_deadline = input.deadline_unix_ms;
            input.deadline_unix_ms = 0;
            assert!(remaining(&input).is_err());
            input.deadline_unix_ms =
                unix_ms().unwrap() + COLD_CASE_LIMIT.as_millis() as u64 + 60_000;
            assert!(remaining(&input).is_err());
            input.deadline_unix_ms = original_deadline;
            input.original_genesis = replacement_genesis;
            assert!(verifier(&input).is_err());
            assert!(execute(input).is_err());
            assert!(
                !root.path().join("never-opened").exists(),
                "untrusted genesis reached wallet/custody opening"
            );
        })
        .unwrap();
    handle.join().unwrap();
}
#[test]
fn cold_report_custody_refuses_overflow_collision_and_path_escape() {
    let temp = tempfile::tempdir().unwrap();
    let root = iroha_fs::PrivateDirectory::open_or_create(temp.path().join("custody")).unwrap();
    root.write_atomic("report", &[1; 16], iroha_fs::PublishMode::CreateNew)
        .unwrap();
    assert!(root.read("report", 15).is_err());
    assert!(
        root.write_atomic("report", &[2], iroha_fs::PublishMode::CreateNew)
            .is_err()
    );
    assert!(root.read("../report", 16).is_err());
    assert!(root.read("report", 16).is_ok());
}

#[test]
fn cold_child_terminal_and_logs_are_exact_and_bounded() {
    let terminal = format!(
        "running 1 test\ntest {} ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 5 filtered out; finished in 0.01s\n",
        test_filter().unwrap()
    );
    assert!(require_exact_terminal(terminal.as_bytes()).is_ok());
    assert!(require_exact_terminal(b"running 0 tests\ntest result: ok. 0 passed;").is_err());
    assert!(require_exact_terminal(format!("{terminal}{terminal}").as_bytes()).is_err());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    assert_eq!(
        runtime.block_on(read_log(&b"original"[..])).unwrap(),
        b"original"
    );
    assert!(
        runtime
            .block_on(read_log(&vec![0; CHILD_LOG_MAX + 1][..]))
            .is_err()
    );
}

#[test]
fn cold_snapshot_accepts_only_the_original_round_and_phase_worker_leases() {
    let original = [7; 32];
    let name = hex::encode(iroha::workflow::domain_digest(
        b"iroha:private-settlement:proof-worker:v1\0",
        &[&original],
    ));
    assert!(require_original_worker_leases(None, None, &[]).is_ok());
    assert!(require_original_worker_leases(Some(original), None, &[name.clone()]).is_ok());
    assert!(require_original_worker_leases(None, None, &[name.clone()]).is_err());
    assert!(require_original_worker_leases(Some(original), None, &[]).is_err());
    assert!(require_original_worker_leases(Some([8; 32]), None, &[name.clone()]).is_err());
    assert!(
        require_original_worker_leases(Some(original), None, &[name.clone(), name.clone()])
            .is_err()
    );
    let phase = ([9; 32], [10; 32]);
    let phase_name = hex::encode(iroha::workflow::domain_digest(
        b"iroha:private-settlement:global-phase-worker:v1\0",
        &[&phase.0, &phase.1],
    ));
    let mut leases = vec![name, phase_name];
    leases.sort();
    assert!(require_original_worker_leases(Some(original), Some(phase), &leases).is_ok());
    assert!(require_original_worker_leases(Some(original), None, &leases).is_err());
    assert!(require_original_worker_leases(None, Some(phase), &leases).is_err());
    assert!(
        require_original_worker_leases(Some(original), Some(([11; 32], phase.1)), &leases).is_err()
    );
    assert!(
        require_original_worker_leases(Some(original), Some((phase.0, [11; 32])), &leases).is_err()
    );
}

#[test]
fn cold_effect_snapshot_excludes_only_original_bounded_pending_prefixes() {
    let temporary = tempfile::tempdir().unwrap();
    let root =
        iroha_fs::PrivateDirectory::open_or_create(temporary.path().join("custody")).unwrap();
    let artifacts = root.create_child("encrypted-artifacts").unwrap();
    let original = WorkflowCustodySnapshot {
        effects: Vec::new(),
        proof_round: Some([7; 32]),
        settlement: Some(WorkflowId::from_bytes([9; 32])),
    };
    let context = format!("{}.contexts", hex::encode([7; 32]));
    artifacts
        .write_atomic(&context, &[1; 64], iroha_fs::PublishMode::CreateNew)
        .unwrap();
    let before = artifact_effect_snapshot(&root, &original).unwrap();
    for (name, bytes) in [
        (
            format!("{}.preparation-finality", hex::encode([7; 32])),
            &[2; 64][..],
        ),
        (
            format!("{}.settlement-finality", hex::encode([9; 32])),
            &[3; 64][..],
        ),
        (
            format!("{}.settlement-finality.lock", hex::encode([9; 32])),
            &b""[..],
        ),
    ] {
        artifacts
            .write_atomic(name, bytes, iroha_fs::PublishMode::CreateNew)
            .unwrap();
    }
    // This helper checks inventory/effect separation; actual child recovery is the
    // only fixture path that decrypts and independently authenticates checkpoint slots.
    assert_eq!(artifact_effect_snapshot(&root, &original).unwrap(), before);
    artifacts
        .write_atomic(&context, &[4; 64], iroha_fs::PublishMode::Replace)
        .unwrap();
    assert_ne!(artifact_effect_snapshot(&root, &original).unwrap(), before);
    let substituted = WorkflowCustodySnapshot {
        proof_round: Some([8; 32]),
        ..original
    };
    assert!(artifact_effect_snapshot(&root, &substituted).is_err());
    let oversized = artifacts
        .path()
        .join(format!("{}.settlement-finality", hex::encode([9; 32])));
    std::fs::OpenOptions::new()
        .write(true)
        .open(oversized)
        .unwrap()
        .set_len(FINALITY_FILE_MAX as u64 + 1)
        .unwrap();
    let restored = WorkflowCustodySnapshot {
        proof_round: Some([7; 32]),
        ..substituted
    };
    assert!(artifact_effect_snapshot(&root, &restored).is_err());
    std::fs::OpenOptions::new()
        .write(true)
        .open(artifacts.path().join(&context))
        .unwrap()
        .set_len(ARTIFACT_FILE_MAX as u64)
        .unwrap();
    std::fs::OpenOptions::new()
        .write(true)
        .open(
            artifacts
                .path()
                .join(format!("{}.settlement-finality", hex::encode([9; 32]))),
        )
        .unwrap()
        .set_len(100 * 1024 * 1024 + 28)
        .unwrap();
    assert!(
        artifact_effect_snapshot(&root, &restored).is_err(),
        "individually bounded files cannot exceed the original aggregate custody cap"
    );
}

#[test]
fn cold_effect_snapshot_binds_every_signed_identity_and_dispatch_coordinate() {
    use iroha::workflow::{
        DriverIdentityV1, StepDescriptorV1, TransactionAttemptV1, TransactionIdentityV1,
        WorkflowBindingV1, WorkflowKindV1, WorkflowPlanV1,
    };
    let plan = WorkflowPlanV1 {
        kind: WorkflowKindV1::PrivateSettlement,
        instance: "cold-effect-binding".to_owned(),
        driver: DriverIdentityV1 {
            id: "private_settlement".to_owned(),
            schema_version: 1,
        },
        request_context_digest: [1; 32],
        steps: vec![StepDescriptorV1::transaction("settle", [2; 32])],
    };
    let binding = WorkflowBindingV1 {
        network: "network".to_owned(),
        authority: "authority".to_owned(),
    };
    let mut original = WorkflowJournalV1::new(&plan, &binding, 7).unwrap();
    original.steps[0]
        .transaction
        .as_mut()
        .unwrap()
        .attempts
        .push(TransactionAttemptV1 {
            ordinal: 1,
            identity: Some(TransactionIdentityV1 {
                signed_transaction_hash: [3; 32],
                entrypoint_hash: [4; 32],
            }),
            dispatch_ordinal: 1,
            possible_admissions: 1,
            evidence: None,
        });
    let expected = journal_effect_digest(&original).unwrap();
    for coordinate in 0..6 {
        let mut changed = original.clone();
        let attempt = &mut changed.steps[0].transaction.as_mut().unwrap().attempts[0];
        match coordinate {
            0 => attempt.identity.as_mut().unwrap().signed_transaction_hash[0] ^= 1,
            1 => attempt.identity.as_mut().unwrap().entrypoint_hash[0] ^= 1,
            2 => attempt.identity = None,
            3 => attempt.ordinal += 1,
            4 => attempt.dispatch_ordinal += 1,
            5 => changed.identity.request_context_digest[0] ^= 1,
            _ => unreachable!(),
        }
        assert_ne!(
            journal_effect_digest(&changed).unwrap(),
            expected,
            "coordinate {coordinate}"
        );
    }
    let mut observed = original.clone();
    observed.sequence += 1;
    observed.updated_at_ms += 1;
    observed.steps[0].transaction.as_mut().unwrap().attempts[0].possible_admissions = 0;
    assert_eq!(
        journal_effect_digest(&observed).unwrap(),
        expected,
        "read-only reconciliation may persist observations without starting another effect"
    );
}

#[test]
fn cold_wallet_snapshot_requires_exact_bounded_production_file_roles() {
    let temporary = tempfile::tempdir().unwrap();
    let root = iroha_fs::PrivateDirectory::open_or_create(temporary.path().join("owner")).unwrap();
    let directory = root.create_child("software-wallet").unwrap();
    assert_eq!(wallet_snapshot(&root).unwrap(), WalletSnapshot::default());
    // Opaque bytes exercise physical inventory only, never AEAD or successful settlement.
    directory
        .write_atomic(
            "authorized-inputs.wallet",
            &[1; 40],
            iroha_fs::PublishMode::CreateNew,
        )
        .unwrap();
    assert!(wallet_snapshot(&root).is_err());
    directory
        .write_atomic(
            "authorized-inputs.wallet.lock",
            &[],
            iroha_fs::PublishMode::CreateNew,
        )
        .unwrap();
    let original = wallet_snapshot(&root).unwrap();
    assert_eq!(original.envelope.unwrap().0, 40);
    directory
        .write_atomic("unexpected", b"unknown", iroha_fs::PublishMode::CreateNew)
        .unwrap();
    assert!(wallet_snapshot(&root).is_err());
    directory.remove_private("unexpected").unwrap();
    directory
        .write_atomic(
            "authorized-inputs.wallet.lock",
            b"nonempty",
            iroha_fs::PublishMode::Replace,
        )
        .unwrap();
    assert!(wallet_snapshot(&root).is_err());
    directory
        .write_atomic(
            "authorized-inputs.wallet.lock",
            &[],
            iroha_fs::PublishMode::Replace,
        )
        .unwrap();
    assert_eq!(wallet_snapshot(&root).unwrap(), original);
    std::fs::OpenOptions::new()
        .write(true)
        .open(directory.path().join("authorized-inputs.wallet"))
        .unwrap()
        .set_len((wallet::MAX_WALLET_INDEX_BYTES + 29) as u64)
        .unwrap();
    assert!(wallet_snapshot(&root).is_err());
}
