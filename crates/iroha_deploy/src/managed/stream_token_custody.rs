//! Initial managed signer custody: original intent, once-only wallet dispatch and native evidence.
//!
//! Configure and Enroll are the only writable purposes. Exact transaction finality and fresh
//! custody state are separate observations; neither enables services or establishes admission,
//! capacity, current signing eligibility, or hardware custody guarantees.

use super::{Error, PreparedLocalnet, Result};
use crate::{
    localnet::service_authorities::{StreamTokenAuthorityManifest, StreamTokenAuthorityRole},
    verify::{
        finality::{
            AttestationQuorum, FinalityError, FinalitySource, FinalityVerifier, GenesisAnchor,
        },
        http::HttpFinalitySource,
    },
};
use iroha::{client::Client, config::Config};
use iroha_crypto::{Hash, HashOf, Signature};
use iroha_data_model::{
    account::AccountId,
    block::BlockHeader,
    sorafs::stream_token_custody::proof::VerifiedStreamTokenCustodyStateV1,
    sumeragi_finality::SumeragiFinalityCheckpoint,
    transaction::{SignedTransaction, TransactionEntrypoint},
};
use iroha_fs::{PrivateDirectory, PublishMode};
use iroha_model_base::peer::PeerId;
use iroha_wallet::operations::{
    AccountService, BoundedTransactionOptions, OperationReport, OperationStatus,
    StreamTokenCustodyConfigureRequest, StreamTokenCustodyEnrollRequest,
    StreamTokenCustodySelection,
};
use sorafs_manifest::signer::{
    custody::{
        SIGNER_CUSTODY_MAGIC_V1, SIGNER_CUSTODY_VERSION_V1, SignerCustodyBindingV1,
        SignerCustodyRecordV1, SignerCustodyStatementV1,
    },
    custody_control::SignerCustodyPolicyV1,
};
use std::{
    fs::File,
    num::NonZeroU64,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[path = "stream_token_custody/identity.rs"]
mod identity;
#[path = "stream_token_custody/journal.rs"]
mod journal;
use journal::{Action, Original, Terms};

const MAX_CHECKPOINT_BYTES: usize = 32 * 1024 * 1024;
const MAX_REPLAY_SUCCESSORS: u64 = 16;

/// Independent successful inclusion of the exact original signed wallet transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ManagedCustodyFinality {
    /// Original signed transaction identity, whose exact wire was independently compared.
    pub transaction_hash: HashOf<SignedTransaction>,
    /// Original certified execution carrier height.
    pub height: u64,
    /// Original authenticated Iroha carrier header hash.
    pub block_hash: HashOf<BlockHeader>,
}

/// Separate node observation, exact original inclusion and freshly proved current custody.
#[derive(Debug)]
pub struct ManagedCustodyProgress {
    /// Exact wallet/node observation; `Applied` alone is not independent finality.
    pub transaction_status: OperationStatus,
    /// Independently authenticated original transaction carrier, when replay has reached it.
    pub finalized: Option<ManagedCustodyFinality>,
    /// Fresh native state at the separately observed quorum tip, not a signing eligibility claim.
    /// `None` means unavailable, including a changed binding; only a verified state's absent
    /// current record is authenticated absence. Historical inclusion remains independently valid.
    pub current: Option<VerifiedStreamTokenCustodyStateV1>,
}

/// An original caller-selected enrollment interval and exclusive submission deadline.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_deploy::managed::ManagedCustodyEnrollmentInterval")]
pub struct ManagedCustodyEnrollmentInterval {
    /// Inclusive original beginning of the attester's authorization, in Unix milliseconds.
    pub issued_at_unix_ms: u64,
    /// Exclusive original end of that authorization, in Unix milliseconds.
    pub expires_at_unix_ms: u64,
    /// Exclusive original transaction deadline; cannot exceed the enrollment expiry.
    pub deadline_unix_ms: u64,
}

/// Held native private custody for one generated Global network's initial Configure and Enroll.
///
/// The generated manager, role signer and attester remain distinct. Opening authenticates the
/// original signed genesis and retained authority profile. Fixed journals support initial
/// provisioning only; expired or conflicting originals are never reset or silently renewed.
pub struct ManagedStreamTokenCustody {
    prepared: PreparedLocalnet,
    directory: PrivateDirectory,
    _lock: File,
    manifest: StreamTokenAuthorityManifest,
    config: Config,
    genesis: GenesisAnchor,
    peers: Vec<(PeerId, Client)>,
}

impl ManagedStreamTokenCustody {
    /// Authenticate a retained generated authority profile and exclusively open its coordinator.
    /// # Errors
    /// Rejects a Standard/private root, substituted genesis/roles/configuration, unsafe custody
    /// or another active coordinator. No network operation or transaction occurs here.
    pub fn open(prepared: &PreparedLocalnet) -> Result<Self> {
        identity::open(prepared)
    }

    /// Retain the original first configuration, prepare its wallet journal, and advance once.
    /// # Errors
    /// Rejects changed original policy/fees/deadline, nonempty current custody, wrong generated
    /// keys or provider, failed native proof, original expiry, or unsafe private publication.
    pub fn configure(
        &mut self,
        policy: &SignerCustodyPolicyV1,
        deadline_unix_ms: u64,
        options: &BoundedTransactionOptions,
    ) -> Result<ManagedCustodyProgress> {
        self.validate_profile()?;
        self.validate_policy(policy)?;
        let directory = self.directory.ensure_child("configure")?;
        match journal::read_original(&directory)? {
            Some(original) => original.matches_configuration(policy, deadline_unix_ms, options)?,
            None => {
                journal::require_empty(&directory)?;
                let terms = Terms::new(deadline_unix_ms, options)?;
                let (verifier, current) = self.observe(&policy.binding, options.deadline)?;
                if current.current().is_some() {
                    return Err(invalid("initial custody configuration already exists"));
                }
                let original = Original {
                    selection: self.selection(&policy.binding, &current)?,
                    action: Action::Configure(policy.clone()),
                    terms,
                    checkpoint: checkpoint_bytes(&verifier)?,
                };
                journal::publish_original(&directory, &original)?;
            }
        }
        self.advance_configure(options.deadline)
    }

    /// Retain an independently attested initial enrollment after proving Configure inclusion.
    ///
    /// Evidence commits the authenticated original genesis/profile, selected policy and exact
    /// fresh native record/checkpoint. It does not assert physical or hardware key isolation.
    /// # Errors
    /// Rejects unfinalized Configure, changed current policy/CAS, revoked roles, changed original
    /// interval/fees, invalid attester custody, expiry or failed independent native evidence.
    pub fn enroll(
        &mut self,
        interval: ManagedCustodyEnrollmentInterval,
        options: &BoundedTransactionOptions,
    ) -> Result<ManagedCustodyProgress> {
        self.validate_profile()?;
        let configured = self.directory.open_child("configure")?;
        let configuration = journal::required_original(&configured)?;
        let Action::Configure(policy) = &configuration.action else {
            return Err(invalid("configuration journal has another purpose"));
        };
        self.validate_policy(policy)?;
        let transaction = self.verify_wallet(&configured, &configuration, options.deadline)?;
        let finalized = self
            .retained_finality(&configured, &transaction)?
            .ok_or_else(|| invalid("configuration requires independent original inclusion"))?;
        let directory = self.directory.ensure_child("enroll")?;
        match journal::read_original(&directory)? {
            Some(original) => original.matches_enrollment(interval, options)?,
            None => {
                journal::require_empty(&directory)?;
                let terms = Terms::new(interval.deadline_unix_ms, options)?;
                let (verifier, current) = self.observe(&policy.binding, options.deadline)?;
                let selected = current
                    .current()
                    .ok_or_else(|| invalid("custody policy absent"))?;
                if selected.control().policy != *policy
                    || selected.control().signer_revoked
                    || selected.control().attester_revoked
                    || selected.control().active_head.is_some()
                    || selected.record().revision != 1
                    || selected.record().execution_height != finalized.height
                    || selected.record().authority != self.config.account
                {
                    return Err(invalid(
                        "initial configured custody changed before enrollment",
                    ));
                }
                let observed = now_ms()?;
                validate_interval(interval, observed)?;
                let checkpoint = checkpoint_bytes(&verifier)?;
                let selection = self.selection(&policy.binding, &current)?;
                let evidence_digest = self.evidence_digest(policy, &selection, &checkpoint)?;
                let statement = SignerCustodyStatementV1 {
                    magic: SIGNER_CUSTODY_MAGIC_V1,
                    version: SIGNER_CUSTODY_VERSION_V1,
                    binding: policy.binding.clone(),
                    authority: policy.attester_authority.clone(),
                    anchor: selected.anchor(),
                    sequence: selected.control().next_sequence,
                    predecessor_digest: selected.control().predecessor_digest,
                    issued_at_unix_ms: interval.issued_at_unix_ms,
                    expires_at_unix_ms: interval.expires_at_unix_ms,
                    evidence_digest,
                    revoked: false,
                };
                let key = self.attester()?;
                let payload = statement
                    .signing_payload()
                    .map_err(|_| invalid("invalid enrollment statement"))?;
                let signature = Signature::new(key.private_key(), &payload);
                let enrollment = SignerCustodyRecordV1 {
                    statement,
                    attestation: signature
                        .payload()
                        .try_into()
                        .map_err(|_| invalid("invalid attester signature"))?,
                };
                let original = Original {
                    selection,
                    action: Action::Enroll {
                        anchor: selected.anchor(),
                        observed_at_unix_ms: observed,
                        interval,
                        enrollment: journal::encode(&enrollment, 16 * 1024)?,
                    },
                    terms,
                    checkpoint,
                };
                journal::publish_original(&directory, &original)?;
            }
        }
        self.advance_enroll(options.deadline)
    }

    /// Recover and advance only the original Configure, with a fresh finite I/O deadline.
    /// # Errors
    /// Rejects changed custody/context, wallet evidence or native finality. This cannot renew UTC
    /// authorization; the wallet's retained pre-dispatch marker permits at most one send.
    pub fn advance_configure(&mut self, deadline: Instant) -> Result<ManagedCustodyProgress> {
        self.advance("configure", true, deadline)
    }

    /// Recover and advance only the original Enroll without replacing its signed interval.
    /// # Errors
    /// Rejects changed custody/context, wallet evidence or native finality. Current proof is
    /// separate from original inclusion and can show custody changed after that transaction.
    pub fn advance_enroll(&mut self, deadline: Instant) -> Result<ManagedCustodyProgress> {
        self.advance("enroll", false, deadline)
    }

    fn advance(
        &mut self,
        name: &str,
        configure: bool,
        deadline: Instant,
    ) -> Result<ManagedCustodyProgress> {
        require_deadline(deadline)?;
        self.validate_profile()?;
        let directory = self.directory.open_child(name)?;
        let original = journal::required_original(&directory)?;
        if matches!(original.action, Action::Configure(_)) != configure {
            return Err(invalid("retained custody purpose differs"));
        }
        self.validate_original(&original)?;
        let needs_prepare = match directory.open_child("transaction") {
            Ok(_) => false,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
            Err(error) => return Err(error.into()),
        };
        let retained_transaction = if needs_prepare {
            None
        } else {
            Some(self.verify_wallet(&directory, &original, deadline)?)
        };
        if let Some(transaction) = &retained_transaction
            && let Some(finalized) = self.retained_finality(&directory, transaction)?
        {
            // Immutable original inclusion survives a current peer/quorum outage. Freshness
            // remains a separate best-effort observation and cannot renew that historical fact.
            let current = self
                .observe(&original.selection.binding, deadline)
                .ok()
                .map(|(_, current)| current);
            return Ok(ManagedCustodyProgress {
                transaction_status: OperationStatus::Applied,
                finalized: Some(finalized),
                current,
            });
        }
        if needs_prepare && now_ms()? >= original.terms.signing_deadline_unix_ms {
            let current = self
                .observe(&original.selection.binding, deadline)
                .ok()
                .map(|(_, current)| current);
            return Ok(ManagedCustodyProgress {
                transaction_status: OperationStatus::Expired,
                finalized: None,
                current,
            });
        }
        let observed = self.observe_finality(deadline)?;
        let current = self
            .read_current(&original.selection.binding, &observed, deadline)
            .ok();
        let unchanged = current.as_ref().is_some_and(|state| {
            matches_predecessor(
                &original.selection,
                state.current().map(|current| current.record()),
            )
        });
        let journal_path = directory.path().join("transaction");
        let account = AccountService::new(self.config.clone())
            .map_err(|_| invalid("cannot open custody wallet"))?;
        if needs_prepare {
            if now_ms()? >= original.terms.signing_deadline_unix_ms {
                return Ok(ManagedCustodyProgress {
                    transaction_status: OperationStatus::Expired,
                    finalized: None,
                    current,
                });
            }
            if !unchanged {
                return Err(invalid(
                    "fresh custody predecessor differs; original request cannot be re-signed",
                ));
            }
            let signing_deadline = original.terms.signing_deadline(deadline)?;
            match original.request(signing_deadline) {
                journal::Request::Configure(request) => {
                    account.prepare_stream_token_custody_configure(&request, &journal_path)
                }
                journal::Request::Enroll(request) => {
                    account.prepare_stream_token_custody_enroll(&request, &journal_path)
                }
            }
            .map_err(|_| {
                invalid("custody preparation failed; retain original request and journal")
            })?;
        }
        let transaction = match retained_transaction {
            Some(transaction) => transaction,
            None => self.verify_wallet(&directory, &original, deadline)?,
        };
        let mut report = match original.request(deadline) {
            journal::Request::Configure(request) => {
                account.resume_stream_token_custody_configure(&journal_path, &request)
            }
            journal::Request::Enroll(request) => {
                account.resume_stream_token_custody_enroll(&journal_path, &request)
            }
        }
        .map_err(|_| invalid("custody transaction unresolved; recover its original journal"))?;
        if report.status == OperationStatus::Absent {
            if now_ms()? >= original.terms.signing_deadline_unix_ms {
                report.status = OperationStatus::Expired;
            } else if unchanged {
                report = match original.request(deadline) {
                    journal::Request::Configure(request) => {
                        account.submit_stream_token_custody_configure(&journal_path, &request)
                    }
                    journal::Request::Enroll(request) => {
                        account.submit_stream_token_custody_enroll(&journal_path, &request)
                    }
                }
                .map_err(|_| {
                    invalid("custody transaction unresolved; recover its original journal")
                })?;
            }
        }
        let finalized = if let Some(retained) = self.retained_finality(&directory, &transaction)? {
            Some(retained)
        } else if report.status == OperationStatus::Applied {
            self.advance_carrier(
                &directory,
                &original,
                &transaction,
                &report,
                observed.checkpoint().height(),
                deadline,
            )?
        } else {
            None
        };
        // Refresh separately after any dispatch/replay; an old original inclusion never becomes
        // a claim that today's policy, revocation or enrollment head still agrees.
        let observed = self.observe_finality(deadline)?;
        let current = self
            .read_current(&original.selection.binding, &observed, deadline)
            .ok();
        Ok(ManagedCustodyProgress {
            transaction_status: report.status,
            finalized,
            current,
        })
    }

    fn observe(
        &mut self,
        binding: &SignerCustodyBindingV1,
        deadline: Instant,
    ) -> Result<(FinalityVerifier, VerifiedStreamTokenCustodyStateV1)> {
        let verifier = self.observe_finality(deadline)?;
        let current = self.read_current(binding, &verifier, deadline)?;
        Ok((verifier, current))
    }

    fn observe_finality(&mut self, deadline: Instant) -> Result<FinalityVerifier> {
        require_deadline(deadline)?;
        let retained = journal::read_optional(
            &self.directory,
            "current-checkpoint.nrt",
            MAX_CHECKPOINT_BYTES,
        )?;
        let mut verifier = if let Some(bytes) = retained {
            self.decode_checkpoint(&bytes)?
        } else {
            let source = self.source(1, deadline)?;
            let proof = source
                .finality_proof(NonZeroU64::new(1).expect("positive genesis"))
                .map_err(|_| invalid("cannot read original genesis result"))?;
            FinalityVerifier::from_genesis(&self.genesis, &proof)
                .map_err(|_| invalid("original genesis finality differs"))?
        };
        let source = self.source(verifier.checkpoint().height(), deadline)?;
        let observation = verifier.observe(&source, &rand::random());
        retain_observation(&self.directory, &mut verifier, observation)?;
        Ok(verifier)
    }

    fn read_current(
        &self,
        binding: &SignerCustodyBindingV1,
        verifier: &FinalityVerifier,
        deadline: Instant,
    ) -> Result<VerifiedStreamTokenCustodyStateV1> {
        let block = verifier
            .verified_tip()
            .map_err(|_| invalid("invalid certified custody tip"))?;
        let owner = self.role(StreamTokenAuthorityRole::IssuerOperator)?;
        let schema = iroha_core::state::State::native_world_schema_hash_v1()
            .map_err(|_| invalid("native custody schema is unavailable"))?;
        read_selected_peers(&self.peers, deadline, |client, deadline| {
            client
                .with_request_deadline(deadline)
                .get_stream_token_custody_state(
                    self.manifest.provider_id,
                    owner,
                    binding,
                    schema,
                    &block,
                )
                .map_err(|_| invalid("native custody candidate is unavailable or invalid"))
        })
    }

    fn source(&self, height: u64, deadline: Instant) -> Result<HttpFinalitySource> {
        HttpFinalitySource::new(
            self.config.network_id,
            NonZeroU64::new(height).ok_or_else(|| invalid("zero custody checkpoint"))?,
            self.peers
                .iter()
                .map(|(_, client)| client.clone())
                .collect(),
            self.peers.clone(),
            deadline,
        )
        .map_err(|_| invalid("invalid custody finality source"))
    }

    fn decode_checkpoint(&self, bytes: &[u8]) -> Result<FinalityVerifier> {
        decode_checkpoint(
            bytes,
            self.config.network_id,
            &self.config.chain.to_string(),
        )
    }

    fn verify_wallet(
        &self,
        directory: &PrivateDirectory,
        original: &Original,
        deadline: Instant,
    ) -> Result<SignedTransaction> {
        let account = AccountService::new(self.config.clone())
            .map_err(|_| invalid("cannot open custody wallet"))?;
        let path = directory.path().join("transaction");
        match original.request(deadline) {
            journal::Request::Configure(request) => {
                account.verify_stream_token_custody_configure_journal(&path, &request)
            }
            journal::Request::Enroll(request) => {
                account.verify_stream_token_custody_enroll_journal(&path, &request)
            }
        }
        .map_err(|_| invalid("custody wallet differs from original request"))
    }

    fn retained_finality(
        &self,
        directory: &PrivateDirectory,
        transaction: &SignedTransaction,
    ) -> Result<Option<ManagedCustodyFinality>> {
        retained_carrier(
            directory,
            self.config.network_id,
            &self.config.chain.to_string(),
            transaction,
        )
    }

    fn advance_carrier(
        &self,
        directory: &PrivateDirectory,
        original: &Original,
        transaction: &SignedTransaction,
        report: &OperationReport,
        observed_height: u64,
        deadline: Instant,
    ) -> Result<Option<ManagedCustodyFinality>> {
        let height = report
            .data
            .get("evidence")
            .and_then(|e| e.get("block_height"))
            .and_then(norito::json::Value::as_u64)
            .ok_or_else(|| invalid("Applied custody observation has no carrier hint"))?;
        let original_verifier = self.decode_checkpoint(&original.checkpoint)?;
        if height <= original_verifier.checkpoint().height() {
            return Err(invalid("custody carrier predates its original request"));
        }
        if height > observed_height {
            return Ok(None);
        }
        // Applied is only a replaceable lookup hint. A false earlier hint cannot irreversibly
        // pin a carrier or prevent replaying the original transaction at its actual height.
        let progress = journal::read_optional(directory, "replay.nrt", MAX_CHECKPOINT_BYTES)?
            .map(|bytes| self.decode_checkpoint(&bytes))
            .transpose()?;
        let mut verifier = replay_start(original_verifier, progress, height)?;
        let source = self.source(verifier.checkpoint().height(), deadline)?;
        let target = height.min(
            verifier
                .checkpoint()
                .height()
                .saturating_add(MAX_REPLAY_SUCCESSORS),
        );
        verifier
            .catch_up(
                &source,
                NonZeroU64::new(target).ok_or_else(|| invalid("zero custody carrier"))?,
            )
            .map_err(|_| invalid("original custody carrier replay unavailable"))?;
        let bytes = checkpoint_bytes(&verifier)?;
        directory.write_atomic("replay.nrt", &bytes, PublishMode::Replace)?;
        if verifier.checkpoint().height() != height {
            return Ok(None);
        }
        let finalized = verify_carrier(&verifier, transaction)?;
        directory.write_atomic("carrier.nrt", &bytes, PublishMode::CreateNew)?;
        Ok(Some(finalized))
    }
}

// Every candidate is checked by the supplied SDK operation against the same independently
// authenticated block. This helper owns only bounded endpoint iteration, never proof authority.
fn read_selected_peers<T>(
    peers: &[(PeerId, Client)],
    deadline: Instant,
    mut read: impl FnMut(&Client, Instant) -> Result<T>,
) -> Result<T> {
    for (_, client) in peers {
        require_deadline(deadline)?;
        if let Ok(value) = read(
            client,
            deadline.min(Instant::now() + Duration::from_secs(5)),
        ) {
            require_deadline(deadline)?;
            return Ok(value);
        }
    }
    Err(invalid(
        "fresh native custody presence or absence proof unavailable",
    ))
}

fn decode_checkpoint(
    bytes: &[u8],
    network: iroha_data_model::NetworkId,
    chain: &str,
) -> Result<FinalityVerifier> {
    if bytes.len() > MAX_CHECKPOINT_BYTES {
        return Err(invalid("custody checkpoint exceeds bound"));
    }
    let checkpoint = SumeragiFinalityCheckpoint::decode_canonical(bytes)
        .map_err(|_| invalid("invalid retained custody checkpoint"))?;
    FinalityVerifier::from_checkpoint(checkpoint, network, chain)
        .map_err(|_| invalid("retained custody checkpoint changed network or chain"))
}

pub(crate) fn retained_carrier(
    directory: &PrivateDirectory,
    network: iroha_data_model::NetworkId,
    chain: &str,
    transaction: &SignedTransaction,
) -> Result<Option<ManagedCustodyFinality>> {
    journal::read_optional(directory, "carrier.nrt", MAX_CHECKPOINT_BYTES)?
        .map(|bytes| verify_carrier(&decode_checkpoint(&bytes, network, chain)?, transaction))
        .transpose()
}

fn matches_predecessor(
    selection: &StreamTokenCustodySelection,
    current: Option<
        &iroha_data_model::sorafs::stream_token_custody::StreamTokenCustodyControlRecordV1,
    >,
) -> bool {
    match current {
        None => {
            selection.current.is_none()
                && selection.expected_revision == 0
                && selection.expected_digest == [0; 32]
        }
        Some(current) => {
            selection.current.as_ref() == Some(current)
                && selection.expected_revision == current.revision
                && current
                    .canonical_digest()
                    .is_ok_and(|digest| digest == selection.expected_digest)
        }
    }
}

pub(crate) fn retain_observation(
    directory: &PrivateDirectory,
    verifier: &mut FinalityVerifier,
    observation: std::result::Result<AttestationQuorum, FinalityError>,
) -> Result<()> {
    match &observation {
        Ok(_) => {}
        Err(FinalityError::CatchingUp { .. }) => {
            if !verifier.promote_verified_progress() {
                return Err(invalid("custody catch-up omitted its verified prefix"));
            }
        }
        Err(_) => return Err(invalid("fresh native custody quorum unavailable")),
    }
    directory.write_atomic(
        "current-checkpoint.nrt",
        &checkpoint_bytes(verifier)?,
        PublishMode::Replace,
    )?;
    observation
        .map(|_| ())
        .map_err(|_| invalid("custody finality is catching up; a fresh quorum is still required"))
}

pub(crate) fn replay_start(
    original: FinalityVerifier,
    progress: Option<FinalityVerifier>,
    hint: u64,
) -> Result<FinalityVerifier> {
    if hint <= original.checkpoint().height() {
        return Err(invalid("custody carrier predates original authorization"));
    }
    if let Some(progress) = progress {
        if progress.checkpoint().network_id() != original.checkpoint().network_id()
            || progress.checkpoint().chain_id() != original.checkpoint().chain_id()
            || progress.checkpoint().height() < original.checkpoint().height()
        {
            return Err(invalid("custody replay left its original certified prefix"));
        }
        if progress.checkpoint().height() <= hint {
            return Ok(progress);
        }
    }
    Ok(original)
}

pub(crate) fn verify_carrier(
    verifier: &FinalityVerifier,
    transaction: &SignedTransaction,
) -> Result<ManagedCustodyFinality> {
    let verified = verifier
        .verified_tip()
        .map_err(|_| invalid("invalid original custody carrier"))?;
    verified
        .verify_global_scope(
            verifier.checkpoint().network_id(),
            verifier.checkpoint().chain_id(),
        )
        .map_err(|_| invalid("custody carrier is not the selected Global root"))?;
    transaction
        .verify_signature()
        .map_err(|_| invalid("invalid original custody signature"))?;
    if transaction.network_id() != Some(&verifier.checkpoint().network_id()) {
        return Err(invalid("custody transaction network differs from carrier"));
    }
    let wire = transaction
        .encode_wire_v1()
        .map_err(|_| invalid("invalid original custody wire"))?;
    let mut found = false;
    for (index, entrypoint) in verified.block().network_entrypoints().enumerate() {
        let TransactionEntrypoint::External(candidate) = entrypoint else {
            continue;
        };
        if candidate.hash() != transaction.hash() {
            continue;
        }
        let input_index =
            u32::try_from(index).map_err(|_| invalid("custody carrier index exceeds bound"))?;
        if found
            || candidate
                .encode_wire_v1()
                .map_err(|_| invalid("invalid carrier transaction wire"))?
                != wire
            || !verified
                .block()
                .network_output_at(input_index)
                .is_some_and(|(_, output)| output.result.as_ref().is_ok())
        {
            return Err(invalid(
                "custody carrier lacks exact successful original execution",
            ));
        }
        found = true;
    }
    if !found {
        return Err(invalid(
            "original custody transaction absent from certified carrier",
        ));
    }
    Ok(ManagedCustodyFinality {
        transaction_hash: transaction.hash(),
        height: verified.height(),
        block_hash: verified.header().hash(),
    })
}

fn checkpoint_bytes(verifier: &FinalityVerifier) -> Result<Vec<u8>> {
    let bytes = verifier
        .checkpoint()
        .encode_canonical()
        .map_err(|_| invalid("cannot encode custody checkpoint"))?;
    if bytes.len() > MAX_CHECKPOINT_BYTES {
        return Err(invalid("custody checkpoint exceeds bound"));
    }
    Ok(bytes)
}
fn invalid(message: &'static str) -> Error {
    Error::Invalid(message.into())
}
fn require_deadline(deadline: Instant) -> Result<()> {
    if deadline <= Instant::now() {
        return Err(invalid(
            "custody I/O deadline elapsed; retain original journals",
        ));
    }
    Ok(())
}
fn now_ms() -> Result<u64> {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| invalid("invalid UTC clock"))?
            .as_millis(),
    )
    .map_err(|_| invalid("UTC clock exceeds custody bounds"))
}
fn validate_interval(interval: ManagedCustodyEnrollmentInterval, now: u64) -> Result<()> {
    if interval.issued_at_unix_ms == 0
        || interval.issued_at_unix_ms > now
        || interval.expires_at_unix_ms <= now
        || interval.expires_at_unix_ms == u64::MAX
        || interval.deadline_unix_ms <= now
        || interval.deadline_unix_ms > interval.expires_at_unix_ms
    {
        return Err(invalid(
            "enrollment requires its original finite UTC interval",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "stream_token_custody/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "stream_token_custody/transport_tests.rs"]
mod transport_tests;
