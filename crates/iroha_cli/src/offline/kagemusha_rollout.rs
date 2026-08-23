//! Exact-byte, phase-separated Kagemusha V4 activation rollout.

mod liveness;

use crate::{Run, RunContext, operator_key::load_operator_key_pair};
use clap::{Args as ClapArgs, Subcommand};
use eyre::{Result, WrapErr as _, bail, eyre};
use iroha::{
    client::{
        Client, TransactionWaitOptions, TransactionWaitOutcome, TransactionWaitTerminalStatus,
    },
    data_model::{
        block::{
            consensus_v2::ConsensusMode, decode_framed_signed_block,
            proofs::TrustedBlockProofAnchor,
        },
        bridge::{BridgeFinalityProof, BridgeFinalityVerifier},
        isi::{
            InstructionBox,
            offline::{AuthorizeKagemushaTairaCanaryV4, RecordKagemushaTairaCanaryV4},
        },
        offline::{
            KAGEMUSHA_V4_ACTIVATION_EXPECTATIONS_BODY_SCHEMA,
            KAGEMUSHA_V4_ACTIVATION_EXPECTATIONS_MAX_BYTES,
            KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1,
            KAGEMUSHA_V4_ACTIVATION_FINALITY_RECEIPT_BODY_SCHEMA,
            KAGEMUSHA_V4_ACTIVATION_RECEIPT_MAX_BYTES, KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT,
            KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION, KAGEMUSHA_V4_PROMOTION_RESERVATION_MAX_BYTES,
            KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_BODY_SCHEMA,
            KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_MAX_BYTES,
            KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_MAX_INTERVAL_MS,
            KAGEMUSHA_V4_TAIRA_CANARY_EVIDENCE_BODY_SCHEMA, KagemushaExactBytesDigestV1,
            KagemushaFinalizedBlockWireV1, KagemushaV4ActivationFinalityProofChainV1,
            KagemushaV4ActivationFinalityReceiptBodyV1, KagemushaV4ActivationFinalityReceiptV1,
            KagemushaV4ActivationReceiptExpectationsArtifactV1,
            KagemushaV4ActivationReceiptExpectationsBodyV1,
            KagemushaV4ActivationReceiptExpectationsV1, KagemushaV4PromotionReservationV1,
            KagemushaV4TairaCanaryAuthorizationBodyV1, KagemushaV4TairaCanaryAuthorizationV1,
            KagemushaV4TairaCanaryEvidenceBodyV1, KagemushaV4TairaCanaryEvidenceV1,
            KagemushaV4TairaCanaryPermitV1, KagemushaV4TairaCanaryQueryObservationV1,
            KagemushaV4ValidatorQualificationSealV1, KagemushaV4VerifiedTairaCanaryAuthorizationV1,
            kagemusha_v4_taira_canary_transaction_metadata,
            validate_kagemusha_v4_taira_canary_torii_origin,
            verify_kagemusha_v4_validator_qualification_seals,
        },
        query::CommittedTransaction,
        transaction::{
            Executable, SignedTransaction, TransactionAdmissionIntent, TransactionEntrypoint,
        },
    },
};
use iroha_crypto::{Hash, HashOf, KeyPair, PublicKey};
use iroha_torii_shared::PipelineTransactionStatusResponse;
use iroha_version::codec::DecodeVersioned as _;
use std::{
    ffi::OsString,
    fs::{self, File},
    io::{Read as _, Seek as _, SeekFrom, Write as _},
    num::NonZeroU64,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const SEAL_MAX_BYTES: usize = 1024 * 1024;
const FINALITY_PROOF_MAX_BYTES: usize = 8 * 1024 * 1024;
const TRANSACTION_MAX_BYTES: usize = 64 * 1024 * 1024;
const EXPECTATIONS_FILE_NAME: &str = "activation-expectations-v1.norito";
const SUBMISSION_JOURNAL_FILE_NAME: &str = "activation-submission-journal-v1.norito";
const RECEIPT_FILE_NAME: &str = "activation-finality-receipt-v1.norito";
const CANARY_AUTHORIZATION_FILE_NAME: &str = "canary-authorization-v1.norito";
const CANARY_AUTHORIZATION_SUBMISSION_JOURNAL_FILE_NAME: &str =
    "canary-authorization-submission-journal-v1.norito";
const CANARY_SUBMISSION_JOURNAL_FILE_NAME: &str = "canary-submission-journal-v1.norito";
const CANARY_EVIDENCE_FILE_NAME: &str = "canary-evidence-v1.norito";
const CANARY_VALIDATOR_LIVENESS_CHALLENGE_FILE_NAME: &str =
    "post-canary-validator-liveness-challenge-v1.norito";
const CANARY_VALIDATOR_LIVENESS_EVIDENCE_FILE_NAME: &str =
    "post-canary-validator-liveness-evidence-v1.norito";
const CANARY_CONSTRUCTION_HEADROOM_MS: u64 = 30_000;
const CANARY_MIN_AUTHORIZATION_INTERVAL_MS: u64 = 3 * CANARY_CONSTRUCTION_HEADROOM_MS;
#[cfg(target_os = "linux")]
const ROLLOUT_STATE_ROOT: &str = "/var/lib/iroha/kagemusha-rollout-v1";
#[cfg(target_os = "macos")]
const ROLLOUT_STATE_ROOT: &str = "/private/var/db/iroha-kagemusha-rollout-v1";

#[cfg(target_os = "linux")]
#[allow(
    unsafe_code,
    reason = "Linux exposes descriptor-bound xattr inspection through libc"
)]
unsafe extern "C" {
    fn flistxattr(fd: std::os::raw::c_int, list: *mut std::os::raw::c_char, size: usize) -> isize;
}
#[cfg(target_os = "macos")]
#[allow(
    unsafe_code,
    reason = "macOS exposes descriptor-bound ACL and xattr inspection through libc"
)]
unsafe extern "C" {
    fn acl_get_fd_np(
        fd: std::os::raw::c_int,
        acl_type: std::os::raw::c_int,
    ) -> *mut std::ffi::c_void;
    fn acl_get_entry(
        acl: *mut std::ffi::c_void,
        entry_id: std::os::raw::c_int,
        entry: *mut *mut std::ffi::c_void,
    ) -> std::os::raw::c_int;
    fn acl_free(acl: *mut std::ffi::c_void) -> std::os::raw::c_int;
    fn flistxattr(
        fd: std::os::raw::c_int,
        list: *mut std::os::raw::c_char,
        size: usize,
        options: std::os::raw::c_int,
    ) -> isize;
}

/// Phase-separated rollout command.
#[derive(ClapArgs, Debug)]
pub(crate) struct Args {
    #[command(subcommand)]
    command: Command,
}

impl Args {
    pub(super) const fn allows_fallback_config(&self) -> bool {
        matches!(&self.command, Command::CreateExpectations(_))
    }
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Root-sign and immutably publish pre-submission expectations.
    CreateExpectations(CreateExpectations),
    /// Submit the exact transaction embedded in authenticated expectations.
    Submit(Submit),
    /// Collect finalized evidence and immutably publish the issuer-signed receipt.
    FinalizeReceipt(FinalizeReceipt),
    /// Construct and controller-authorize one promotion-bound canary transaction.
    CreateCanaryAuthorization(CreateCanaryAuthorization),
    /// Finalize the on-chain reservation for the exact authorized canary transaction.
    SubmitCanaryAuthorization(SubmitCanaryAuthorization),
    /// Journal and submit the exact controller-authorized canary transaction.
    SubmitCanary(SubmitCanary),
    /// Collect the canary's post-activation finality extension and publish full evidence.
    FinalizeCanaryEvidence(FinalizeCanaryEvidence),
    /// Challenge all four qualified validators and publish issuer-signed liveness evidence.
    FinalizeValidatorLiveness(liveness::FinalizeValidatorLiveness),
}

impl Run for Args {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self.command {
            Command::CreateExpectations(args) => args.run(context),
            Command::Submit(args) => args.run(context),
            Command::FinalizeReceipt(args) => args.run(context),
            Command::CreateCanaryAuthorization(args) => args.run(context),
            Command::SubmitCanaryAuthorization(args) => args.run(context),
            Command::SubmitCanary(args) => args.run(context),
            Command::FinalizeCanaryEvidence(args) => args.run(context),
            Command::FinalizeValidatorLiveness(args) => args.run(context),
        }
    }
}

#[derive(ClapArgs, Debug)]
struct TrustedInputs {
    /// Independently pinned promotion-controller public key.
    #[arg(long, value_name = "PUBLIC_KEY")]
    promotion_controller: String,
    /// Root-owned canonical promotion reservation.
    #[arg(long, value_name = "ABSOLUTE_PATH")]
    promotion_reservation: PathBuf,
    /// Root-owned canonical activation expectations.
    #[arg(long, value_name = "ABSOLUTE_PATH")]
    expectations: PathBuf,
}

#[derive(ClapArgs, Debug)]
struct CreateExpectations {
    /// Independently pinned promotion-controller public key.
    #[arg(long, value_name = "PUBLIC_KEY")]
    promotion_controller: String,
    /// Root-owned canonical promotion reservation.
    #[arg(long, value_name = "ABSOLUTE_PATH")]
    promotion_reservation: PathBuf,
    /// Exactly four root-owned validator qualification seals.
    #[arg(long = "validator-seal", value_name = "ABSOLUTE_PATH", num_args = 4)]
    validator_seals: Vec<PathBuf>,
    /// Complete, already-authorized versioned SignedTransaction wire.
    #[arg(long, value_name = "ABSOLUTE_PATH")]
    activation_transaction: PathBuf,
    /// Independently governed, already-finalized anchor proof.
    #[arg(long, value_name = "ABSOLUTE_PATH")]
    trusted_finality_anchor: PathBuf,
    /// Independent durable-receipt issuer public key.
    #[arg(long, value_name = "PUBLIC_KEY")]
    receipt_issuer: String,
    /// Runtime-only owner-private promotion-controller key file.
    #[arg(long, value_name = "ABSOLUTE_PATH")]
    controller_private_key_file: PathBuf,
    /// Exact absent promotion-keyed expectations destination; it is never replaced.
    #[arg(long, value_name = "ABSOLUTE_PATH")]
    output: PathBuf,
}

impl CreateExpectations {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        require_root()?;
        let controller = parse_public_key(&self.promotion_controller, "promotion controller")?;
        let issuer = parse_public_key(&self.receipt_issuer, "receipt issuer")?;
        let reservation_bytes = read_root_owned(
            &self.promotion_reservation,
            KAGEMUSHA_V4_PROMOTION_RESERVATION_MAX_BYTES,
            "promotion reservation",
        )?;
        let reservation = KagemushaV4PromotionReservationV1::decode_and_verify_canonical(
            &reservation_bytes,
            &controller,
        )
        .wrap_err("invalid promotion reservation")?;
        require_rollout_state_path(
            &self.output,
            reservation.body.promotion_id,
            EXPECTATIONS_FILE_NAME,
        )?;
        preflight_root_owned_output(&self.output)?;
        let mut seals = self
            .validator_seals
            .iter()
            .map(|path| {
                let bytes = read_root_owned(path, SEAL_MAX_BYTES, "validator seal")?;
                let seal: KagemushaV4ValidatorQualificationSealV1 =
                    norito::decode_canonical_with_limits(
                        &bytes,
                        norito::canonical_decode_limits(bytes.len()),
                    )
                    .wrap_err("invalid validator seal")?;
                if norito::encode_canonical(&seal).ok().as_deref() != Some(bytes.as_slice()) {
                    bail!("validator seal is not canonical");
                }
                seal.verify().wrap_err("invalid validator seal signature")?;
                Ok(seal)
            })
            .collect::<Result<Vec<_>>>()?;
        if seals.len() != KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT {
            bail!("exactly four validator seals are required");
        }
        seals.sort_by(|left, right| left.body.validator_id.cmp(&right.body.validator_id));
        let seals: [KagemushaV4ValidatorQualificationSealV1;
            KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT] = seals
            .try_into()
            .map_err(|_| eyre!("exactly four validator seals are required"))?;
        let binding = seals[0].body.binding.clone();
        let bodies = core::array::from_fn(|index| seals[index].body.clone());
        verify_kagemusha_v4_validator_qualification_seals(&seals, &bodies, &binding)
            .wrap_err("validator seal collection is not exact")?;

        let transaction_bytes = read_root_private_artifact(
            &self.activation_transaction,
            TRANSACTION_MAX_BYTES,
            "activation transaction",
        )?;
        let transaction = norito::core::with_decode_limits_scope(
            norito::canonical_decode_limits(transaction_bytes.len()),
            || SignedTransaction::decode_all_versioned(&transaction_bytes),
        )
        .map_err(|error| eyre!("invalid versioned activation transaction: {error}"))?;
        if transaction
            .encode_wire_v1()
            .map_err(|error| eyre!("failed to re-encode activation transaction: {error}"))?
            != transaction_bytes
        {
            bail!("activation transaction is not the exact canonical V1 wire");
        }
        let anchor_bytes = read_root_owned(
            &self.trusted_finality_anchor,
            FINALITY_PROOF_MAX_BYTES,
            "trusted finality anchor",
        )?;
        let anchor: BridgeFinalityProof = norito::decode_canonical_with_limits(
            &anchor_bytes,
            norito::canonical_decode_limits(anchor_bytes.len()),
        )
        .wrap_err("invalid trusted finality anchor")?;
        if norito::encode_canonical(&anchor).ok().as_deref() != Some(anchor_bytes.as_slice()) {
            bail!("trusted finality anchor is not canonical");
        }
        let policy = transaction
            .authority()
            .multisig_policy()
            .cloned()
            .ok_or_else(|| eyre!("activation authority is not a multisignature account"))?;
        let body = KagemushaV4ActivationReceiptExpectationsBodyV1 {
            schema: KAGEMUSHA_V4_ACTIVATION_EXPECTATIONS_BODY_SCHEMA.to_owned(),
            version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            promotion_controller: controller.clone(),
            promotion_reservation: KagemushaExactBytesDigestV1::from_bytes(&reservation_bytes)?,
            binding,
            receipt_issuer: issuer,
            governance_authority: transaction.authority().clone(),
            governance_multisig_policy: policy,
            validator_seals: seals,
            activation_transaction: transaction,
            trusted_finality_anchor: anchor,
        };
        let artifact = {
            let controller_key = load_root_custodied_key(
                &self.controller_private_key_file,
                "promotion-controller key",
            )?;
            if controller_key.public_key() != &controller {
                bail!("promotion-controller key file does not match the pinned public key");
            }
            KagemushaV4ActivationReceiptExpectationsArtifactV1::try_sign(body, &controller_key)
                .wrap_err("failed to sign activation expectations")?
        };
        let bytes = norito::encode_canonical(&artifact)
            .wrap_err("failed to encode activation expectations")?;
        artifact
            .verify_exact(&bytes, &controller, &reservation_bytes)
            .wrap_err("signed activation expectations failed exact verification")?;
        publish_root_owned(&self.output, &bytes, |published| {
            KagemushaV4ActivationReceiptExpectationsArtifactV1::decode_and_verify_canonical(
                published,
                &controller,
                &reservation_bytes,
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
        })?;
        let report = norito::json!({
            "status": "created",
            "output": (self.output.display().to_string()),
            "byte_len": (u64::try_from(bytes.len()).unwrap_or(u64::MAX)),
            "sha256": (hex::encode(KagemushaExactBytesDigestV1::from_bytes(&bytes)?.sha256)),
            "validator_count": (u64::try_from(KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT).unwrap_or(u64::MAX)),
            "promotion_id": (hex::encode(reservation.body.promotion_id)),
        });
        context.print_data(&report).map_err(|error| {
            eyre!(PublicationError::CommitUncertain {
                path: self.output,
                detail: format!("published expectations report failed: {error}"),
            })
        })
    }
}

#[derive(ClapArgs, Debug)]
struct Submit {
    #[command(flatten)]
    trusted: TrustedInputs,
    /// Explicit authorization for this network write.
    #[arg(long, required = true, action = clap::ArgAction::SetTrue)]
    write_authorized: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SubmissionJournalObservation {
    Absent,
    Matching,
    Mismatched,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SubmissionJournalAction {
    Publish,
    Resume,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
enum SubmissionJournalDecisionError {
    #[error("submission journal does not byte-match the authenticated expectations")]
    Mismatch,
    #[error("retrospective submission refused because Torii already reports transaction status")]
    Retrospective,
}

fn decide_submission_journal(
    observation: SubmissionJournalObservation,
    preexisting_status: bool,
) -> std::result::Result<SubmissionJournalAction, SubmissionJournalDecisionError> {
    match (observation, preexisting_status) {
        (SubmissionJournalObservation::Mismatched, _) => {
            Err(SubmissionJournalDecisionError::Mismatch)
        }
        (SubmissionJournalObservation::Absent, true) => {
            Err(SubmissionJournalDecisionError::Retrospective)
        }
        (SubmissionJournalObservation::Absent, false) => Ok(SubmissionJournalAction::Publish),
        (SubmissionJournalObservation::Matching, _) => Ok(SubmissionJournalAction::Resume),
    }
}

fn require_matching_submission_journal(observation: SubmissionJournalObservation) -> Result<()> {
    match observation {
        SubmissionJournalObservation::Matching => Ok(()),
        SubmissionJournalObservation::Absent => {
            bail!("issuer receipt refused because the exact submission journal is absent")
        }
        SubmissionJournalObservation::Mismatched => {
            bail!("issuer receipt refused because the submission journal does not byte-match")
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error(
    "submission-uncertain for activation {transaction_hash} after {stage}; reconcile through the durable journal at `{journal}` before retry: {detail}"
)]
struct SubmissionUncertain {
    transaction_hash: String,
    stage: &'static str,
    journal: PathBuf,
    detail: String,
}

#[derive(Debug, thiserror::Error)]
#[error(
    "submission-uncertain for canary {transaction_hash} after {stage}; reconcile through the durable journal at `{journal}` before retry: {detail}"
)]
struct CanarySubmissionUncertain {
    transaction_hash: String,
    stage: &'static str,
    journal: PathBuf,
    detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReconciledSubmissionStatus {
    Applied,
    Rejected,
    Expired,
    Unresolved,
}

fn classify_reconciled_submission_status(kind: Option<&str>) -> ReconciledSubmissionStatus {
    match kind {
        Some("Applied") => ReconciledSubmissionStatus::Applied,
        Some("Rejected") => ReconciledSubmissionStatus::Rejected,
        Some("Expired") => ReconciledSubmissionStatus::Expired,
        Some(_) | None => ReconciledSubmissionStatus::Unresolved,
    }
}

impl Submit {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        if !self.write_authorized {
            bail!("--write-authorized is required for governed activation submission");
        }
        require_root()?;
        let loaded = load_verified_expectations(&self.trusted)?;
        let transaction = &loaded.artifact.body.activation_transaction;
        let client = context.client_from_config();
        if transaction.network_id() != Some(&client.network_id) {
            bail!("activation transaction network does not match the configured Torii client");
        }
        let prepared = Client::prepare_transaction_payload(transaction);
        let exact = transaction.encode_wire_v1().map_err(|error| {
            eyre!("failed to encode authenticated activation transaction: {error}")
        })?;
        if prepared.as_bytes() != exact.as_slice() {
            bail!("prepared submission changed the authenticated transaction wire");
        }
        let hash = prepared.hash();
        let journal_path = rollout_state_path(
            loaded.verified.binding().promotion_id,
            SUBMISSION_JOURNAL_FILE_NAME,
        )?;
        let journal_observation = inspect_submission_journal(&journal_path, &loaded)?;
        if journal_observation == SubmissionJournalObservation::Mismatched {
            return Err(
                eyre!(SubmissionJournalDecisionError::Mismatch).wrap_err(format!(
                    "unsafe submission journal at `{}`",
                    journal_path.display()
                )),
            );
        }
        let mut initial_status = match client.get_transaction_status_response_auto(hash) {
            Ok(status) => status,
            Err(error) if journal_observation == SubmissionJournalObservation::Matching => {
                return Err(eyre!(SubmissionUncertain {
                    transaction_hash: hash.to_string(),
                    stage: "pre-submit status reconciliation",
                    journal: journal_path,
                    detail: error.to_string(),
                }));
            }
            Err(error) => {
                return Err(error).wrap_err(
                    "could not establish absence of a retrospective transaction before journal publication; no POST was attempted",
                );
            }
        };
        let journal_action =
            decide_submission_journal(journal_observation, initial_status.is_some()).map_err(
                |error| {
                    eyre!(error).wrap_err(format!(
                        "activation {} and journal `{}`",
                        hash,
                        journal_path.display()
                    ))
                },
            )?;
        if let Some(status) = initial_status.as_ref() {
            require_journal_bound_status_response(
                status,
                transaction,
                &journal_path,
                "pre-submit status identity reconciliation",
            )?;
        }
        if journal_action == SubmissionJournalAction::Publish {
            publish_submission_journal(&journal_path, &loaded)?;
            if inspect_submission_journal(&journal_path, &loaded)?
                != SubmissionJournalObservation::Matching
            {
                bail!("durably published submission journal could not be reverified");
            }
            initial_status =
                client
                    .get_transaction_status_response_auto(hash)
                    .map_err(|error| {
                        eyre!(SubmissionUncertain {
                            transaction_hash: hash.to_string(),
                            stage: "post-journal pre-POST reconciliation",
                            journal: journal_path.clone(),
                            detail: error.to_string(),
                        })
                    })?;
            if let Some(status) = initial_status.as_ref() {
                require_journal_bound_status_response(
                    status,
                    transaction,
                    &journal_path,
                    "post-journal pre-POST status identity reconciliation",
                )?;
            }
        }

        if let Some(status) = initial_status.as_ref() {
            match classify_reconciled_submission_status(Some(&status.status.kind)) {
                ReconciledSubmissionStatus::Rejected | ReconciledSubmissionStatus::Expired => {
                    bail!(
                        "governed activation reached terminal status {} instead of Applied",
                        status.status.kind
                    );
                }
                ReconciledSubmissionStatus::Applied | ReconciledSubmissionStatus::Unresolved => {}
            }
        } else if let Err(post_error) = client.submit_prepared_transaction_payload(&prepared) {
            let mut detail = format!("POST failed or its result was ambiguous: {post_error}");
            match client.get_transaction_status_response_auto(hash) {
                Ok(Some(status)) => {
                    require_journal_bound_status_response(
                        &status,
                        transaction,
                        &journal_path,
                        "ambiguous POST immediate status identity reconciliation",
                    )?;
                    match classify_reconciled_submission_status(Some(&status.status.kind)) {
                        ReconciledSubmissionStatus::Applied => {
                            let evidence = collect_finalized_activation_evidence(
                                &client,
                                transaction,
                                &exact,
                                &loaded.verified,
                                applied_carrier_height_for_submission(
                                    &status,
                                    transaction,
                                    &journal_path,
                                    "ambiguous POST status reconciliation",
                                )?,
                            )
                            .map_err(|error| {
                                eyre!(SubmissionUncertain {
                                    transaction_hash: hash.to_string(),
                                    stage: "ambiguous POST proof reconciliation",
                                    journal: journal_path.clone(),
                                    detail: error.to_string(),
                                })
                            })?;
                            let report = norito::json!({
                                "status": "Applied",
                                "transaction_hash": (hash.to_string()),
                                "carrier_height": (evidence.carrier_height.get()),
                                "reconciliation": "proof-anchored after ambiguous POST",
                                "submission_journal": (journal_path.display().to_string()),
                            });
                            return context.print_data(&report).map_err(|error| {
                                eyre!(SubmissionUncertain {
                                    transaction_hash: hash.to_string(),
                                    stage: "proof-anchored Applied result reporting",
                                    journal: journal_path.clone(),
                                    detail: format!(
                                        "activation is proof-anchored Applied, but reporting failed: {error}"
                                    ),
                                })
                            });
                        }
                        ReconciledSubmissionStatus::Rejected
                        | ReconciledSubmissionStatus::Expired => {
                            bail!(
                                "governed activation reached terminal status {} after ambiguous POST",
                                status.status.kind
                            );
                        }
                        ReconciledSubmissionStatus::Unresolved => {
                            detail.push_str(&format!(
                                "; immediate reconciliation observed {}",
                                status.status.kind
                            ));
                        }
                    }
                }
                Ok(None) => detail.push_str("; immediate reconciliation found no status"),
                Err(error) => detail.push_str(&format!(
                    "; immediate status reconciliation also failed: {error}"
                )),
            }

            let outcome = match wait_for_activation_terminal_status(&client, hash) {
                Ok(outcome) => outcome,
                Err(error) => {
                    return reconcile_after_failed_wait(
                        context,
                        &client,
                        transaction,
                        &exact,
                        &loaded.verified,
                        &journal_path,
                        eyre!("{detail}; terminal wait failed: {error}"),
                    );
                }
            };
            return finish_waited_submission(
                context,
                &client,
                transaction,
                &exact,
                &loaded.verified,
                &journal_path,
                outcome,
            );
        }

        let outcome = match wait_for_activation_terminal_status(&client, hash) {
            Ok(outcome) => outcome,
            Err(wait_error) => {
                return reconcile_after_failed_wait(
                    context,
                    &client,
                    transaction,
                    &exact,
                    &loaded.verified,
                    &journal_path,
                    wait_error,
                );
            }
        };
        finish_waited_submission(
            context,
            &client,
            transaction,
            &exact,
            &loaded.verified,
            &journal_path,
            outcome,
        )
    }
}

#[derive(ClapArgs, Debug)]
struct FinalizeReceipt {
    #[command(flatten)]
    trusted: TrustedInputs,
    /// Runtime-only owner-private receipt-issuer key file.
    #[arg(long, value_name = "ABSOLUTE_PATH")]
    issuer_private_key_file: PathBuf,
    /// Exact absent promotion-keyed receipt destination; it is never replaced.
    #[arg(long, value_name = "ABSOLUTE_PATH")]
    output: PathBuf,
}

impl FinalizeReceipt {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        require_root()?;
        let loaded = load_verified_expectations(&self.trusted)?;
        let expectations = &loaded.verified;
        require_rollout_state_path(
            &self.output,
            expectations.binding().promotion_id,
            RECEIPT_FILE_NAME,
        )?;
        preflight_root_owned_output(&self.output)?;
        let journal_path = rollout_state_path(
            expectations.binding().promotion_id,
            SUBMISSION_JOURNAL_FILE_NAME,
        )?;
        require_matching_submission_journal(inspect_submission_journal(&journal_path, &loaded)?)?;
        let artifact = &loaded.artifact;
        let transaction = &artifact.body.activation_transaction;
        let client = context.client_from_config();
        let status = client
            .get_transaction_status_response_auto(transaction.hash())
            .map_err(|error| {
                eyre!(SubmissionUncertain {
                    transaction_hash: transaction.hash().to_string(),
                    stage: "finalize status transport reconciliation",
                    journal: journal_path.clone(),
                    detail: error.to_string(),
                })
            })?
            .ok_or_else(|| {
                eyre!(SubmissionUncertain {
                    transaction_hash: transaction.hash().to_string(),
                    stage: "finalize status reconciliation",
                    journal: journal_path.clone(),
                    detail: "activation transaction is not visible in pipeline status".to_owned(),
                })
            })?;
        require_journal_bound_status_response(
            &status,
            transaction,
            &journal_path,
            "finalize status identity reconciliation",
        )?;
        match classify_reconciled_submission_status(Some(&status.status.kind)) {
            ReconciledSubmissionStatus::Applied => {}
            ReconciledSubmissionStatus::Rejected | ReconciledSubmissionStatus::Expired => {
                bail!(
                    "activation transaction is not Applied: {}",
                    status.status.kind
                );
            }
            ReconciledSubmissionStatus::Unresolved => {
                return Err(eyre!(SubmissionUncertain {
                    transaction_hash: transaction.hash().to_string(),
                    stage: "finalize status reconciliation",
                    journal: journal_path.clone(),
                    detail: format!(
                        "activation transaction has nonterminal or malformed status {}",
                        status.status.kind
                    ),
                }));
            }
        }
        let carrier_height = applied_carrier_height_for_submission(
            &status,
            transaction,
            &journal_path,
            "finalize Applied carrier-height reconciliation",
        )?;
        let exact_transaction_wire = transaction
            .encode_wire_v1()
            .map_err(|error| eyre!("failed to encode authenticated activation: {error}"))?;
        let evidence = collect_finalized_activation_evidence(
            &client,
            transaction,
            &exact_transaction_wire,
            expectations,
            carrier_height,
        )?;
        let FinalizedActivationEvidence {
            committed,
            block_bytes,
            proofs,
            carrier_height: _,
            ..
        } = evidence;
        let block_digest = KagemushaExactBytesDigestV1::from_bytes(&block_bytes)?;
        let body = KagemushaV4ActivationFinalityReceiptBodyV1 {
            schema: KAGEMUSHA_V4_ACTIVATION_FINALITY_RECEIPT_BODY_SCHEMA.to_owned(),
            version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            promotion_reservation: expectations.promotion_reservation(),
            activation_expectations_artifact: expectations.activation_expectations_artifact(),
            binding: expectations.binding().clone(),
            issuer: expectations.receipt_issuer().clone(),
            governance_authority: expectations.governance_authority().clone(),
            validator_seals: expectations.validator_seals().clone(),
            activation_transaction_intent: expectations.activation_transaction_intent(),
            activation_transaction_wire: expectations.activation_transaction_wire(),
            committed_transaction: committed,
            finalized_block_wire: KagemushaFinalizedBlockWireV1::try_from_bytes(block_bytes)?,
            finalized_block_wire_digest: block_digest,
            finality_proof_chain: KagemushaV4ActivationFinalityProofChainV1::try_from(proofs)?,
        };
        let issuer_key =
            load_root_custodied_key(&self.issuer_private_key_file, "receipt-issuer key")?;
        if issuer_key.public_key() != expectations.receipt_issuer() {
            bail!("receipt-issuer key file does not match authenticated expectations");
        }
        let receipt = KagemushaV4ActivationFinalityReceiptV1::try_sign(body, &issuer_key)
            .wrap_err("failed to sign activation receipt")?;
        let verified = receipt
            .verify(expectations)
            .wrap_err("issuer-signed activation receipt failed exact verification")?;
        let bytes =
            norito::encode_canonical(&receipt).wrap_err("failed to encode activation receipt")?;
        publish_root_owned(&self.output, &bytes, |published| {
            let receipt = KagemushaV4ActivationFinalityReceiptV1::decode_canonical(published)
                .map_err(|error| error.to_string())?;
            receipt
                .verify(expectations)
                .map(|_| ())
                .map_err(|error| error.to_string())
        })?;
        let report = norito::json!({
            "status": "finalized",
            "output": (self.output.display().to_string()),
            "byte_len": (u64::try_from(bytes.len()).unwrap_or(u64::MAX)),
            "sha256": (hex::encode(KagemushaExactBytesDigestV1::from_bytes(&bytes)?.sha256)),
            "finalized_height": (verified.finalized_height()),
            "finalized_block_hash": (verified.finalized_block_hash().to_string()),
            "activation_transaction_intent": (verified.activation_transaction_intent().to_string()),
        });
        context.print_data(&report).map_err(|error| {
            eyre!(PublicationError::CommitUncertain {
                path: self.output,
                detail: format!("published receipt report failed: {error}"),
            })
        })
    }
}

#[derive(ClapArgs, Debug)]
struct CreateCanaryAuthorization {
    #[command(flatten)]
    trusted: TrustedInputs,
    /// Exact immutable issuer-signed activation-finality receipt.
    #[arg(long, value_name = "ABSOLUTE_PATH")]
    activation_receipt: PathBuf,
    /// Short authorization lifetime; the transaction TTL reserves 30 seconds for construction.
    #[arg(long, value_name = "MILLISECONDS")]
    canary_ttl_ms: NonZeroU64,
    /// Exact exclusive consensus-height expiry embedded in the canary transaction.
    #[arg(long, value_name = "HEIGHT")]
    canary_expires_at_height: NonZeroU64,
    /// Runtime-only owner-private promotion-controller key file.
    #[arg(long, value_name = "ABSOLUTE_PATH")]
    controller_private_key_file: PathBuf,
    /// Exact absent promotion-keyed authorization destination; it is never replaced.
    #[arg(long, value_name = "ABSOLUTE_PATH")]
    output: PathBuf,
}

impl CreateCanaryAuthorization {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        require_root()?;
        let loaded = load_verified_expectations(&self.trusted)?;
        let receipt = load_verified_receipt(&self.activation_receipt, &loaded)?;
        require_rollout_state_path(
            &self.output,
            loaded.verified.binding().promotion_id,
            CANARY_AUTHORIZATION_FILE_NAME,
        )?;
        preflight_root_owned_output(&self.output)?;

        let ttl_ms = self.canary_ttl_ms.get();
        if ttl_ms > KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_MAX_INTERVAL_MS {
            bail!(
                "canary TTL exceeds the qualified {} millisecond maximum",
                KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_MAX_INTERVAL_MS
            );
        }
        if ttl_ms < CANARY_MIN_AUTHORIZATION_INTERVAL_MS {
            bail!(
                "canary authorization lifetime must be at least {} milliseconds",
                CANARY_MIN_AUTHORIZATION_INTERVAL_MS
            );
        }
        let transaction_ttl_ms = ttl_ms
            .checked_sub(CANARY_CONSTRUCTION_HEADROOM_MS)
            .filter(|ttl| *ttl != 0)
            .ok_or_else(|| {
                eyre!(
                    "canary authorization lifetime must exceed the fixed {} millisecond construction reserve",
                    CANARY_CONSTRUCTION_HEADROOM_MS
                )
            })?;
        let maximum_expiry_height = receipt
            .finalized_height
            .checked_add(
                u64::try_from(KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1)
                    .expect("proof bound fits u64")
                    .saturating_add(1),
            )
            .ok_or_else(|| eyre!("canary expiry-height corridor overflow"))?;
        let minimum_usable_expiry_height = receipt
            .finalized_height
            .checked_add(2)
            .ok_or_else(|| eyre!("canary minimum expiry-height overflow"))?;
        if self.canary_expires_at_height.get() < minimum_usable_expiry_height
            || self.canary_expires_at_height.get() > maximum_expiry_height
        {
            bail!(
                "canary expiry height must strictly follow activation and remain within the bounded proof corridor"
            );
        }

        let mut client = context.client_from_config();
        if client.network_id != loaded.verified.binding().network_id {
            bail!("canary client network differs from authenticated activation");
        }
        let canonical_torii_origin = canonical_torii_origin(&client.torii_url)?;
        let mut authenticated_head =
            AuthenticatedCanaryHead::new(&loaded.verified, &receipt.artifact)?;
        let live_head = authenticated_head.refresh(&client, &loaded.verified)?;
        require_canary_expiry_margin(live_head, self.canary_expires_at_height)?;
        let receipt_digest = KagemushaExactBytesDigestV1::from_bytes(&receipt.exact_bytes)?;
        let authorized_at_unix_ms = current_unix_ms()?;
        let expires_at_unix_ms = authorized_at_unix_ms
            .checked_add(ttl_ms)
            .ok_or_else(|| eyre!("canary authorization wall-clock expiry overflow"))?;
        let controller_key = load_root_custodied_key(
            &self.controller_private_key_file,
            "promotion-controller key",
        )?;
        if controller_key.public_key() != &loaded.controller {
            bail!("promotion-controller key file does not match the pinned public key");
        }
        let body = KagemushaV4TairaCanaryAuthorizationBodyV1 {
            schema: KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_BODY_SCHEMA.to_owned(),
            version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            binding: loaded.verified.binding().clone(),
            activation_expectations_artifact: loaded.verified.activation_expectations_artifact(),
            activation_finality_receipt: receipt_digest,
            canary_authority: client.account.clone(),
            canonical_torii_origin: canonical_torii_origin.clone(),
            authorized_at_unix_ms,
            expires_at_unix_ms,
            expires_at_height: self.canary_expires_at_height,
        };
        let permit = KagemushaV4TairaCanaryPermitV1::try_sign(
            body,
            &controller_key,
            &loaded.verified,
            &receipt.artifact,
            &receipt.exact_bytes,
        )
        .wrap_err("failed to sign the pre-commit canary permit")?;
        let metadata = kagemusha_v4_taira_canary_transaction_metadata(
            loaded.verified.binding().promotion_id,
            receipt_digest,
            &canonical_torii_origin,
            self.canary_expires_at_height,
        );
        client.add_transaction_nonce = true;
        client.transaction_ttl = Some(Duration::from_millis(transaction_ttl_ms));
        let executable = Executable::Instructions(
            vec![InstructionBox::from(RecordKagemushaTairaCanaryV4::new(
                permit.clone(),
            ))]
            .into(),
        );
        let fee_payment = context.transaction_fee_payment()?;
        let mut payload = client
            .try_build_transaction_payload(executable, fee_payment, metadata)
            .wrap_err("failed to build exact unsigned canary transaction")?;
        payload.admission_intent = TransactionAdmissionIntent::Ordinary;
        let transaction = client
            .quote_and_sign_transaction_payload(payload)
            .wrap_err("failed to fee-quote and sign exact ordinary canary transaction")?;
        if transaction.nonce().is_none()
            || transaction.time_to_live() != Some(Duration::from_millis(transaction_ttl_ms))
            || transaction.admission_intent() != TransactionAdmissionIntent::Ordinary
            || transaction
                .expires_at_height()
                .wrap_err("invalid canary height-expiry metadata")?
                != Some(self.canary_expires_at_height.get())
        {
            bail!("signed canary transaction lost its nonce, TTL, or height expiry");
        }
        let transaction_created_at_unix_ms = u64::try_from(transaction.creation_time().as_millis())
            .wrap_err("canary creation time does not fit u64 milliseconds")?;
        let transaction_expires_at_unix_ms = transaction_created_at_unix_ms
            .checked_add(transaction_ttl_ms)
            .ok_or_else(|| eyre!("canary transaction wall-clock expiry overflow"))?;
        if transaction_created_at_unix_ms < authorized_at_unix_ms
            || transaction_expires_at_unix_ms > expires_at_unix_ms
        {
            bail!("canary construction escaped the controller-signed wall-clock interval");
        }
        let authorization = KagemushaV4TairaCanaryAuthorizationV1::try_sign(
            permit,
            transaction,
            &controller_key,
            &loaded.verified,
            &receipt.artifact,
            &receipt.exact_bytes,
        )
        .wrap_err("failed to sign exact canary authorization")?;
        let bytes = norito::encode_canonical(&authorization)
            .wrap_err("failed to encode canary authorization")?;
        let live_head = authenticated_head.refresh(&client, &loaded.verified)?;
        require_canary_expiry_margin(live_head, self.canary_expires_at_height)?;
        let verification_time_unix_ms = current_unix_ms()?;
        let verified = authorization
            .verify_exact(
                &bytes,
                &loaded.verified,
                &receipt.artifact,
                &receipt.exact_bytes,
                verification_time_unix_ms,
            )
            .wrap_err("new canary authorization failed exact verification")?;
        require_canary_authorization_wall_margin(&verified, verification_time_unix_ms)?;
        publish_root_owned(&self.output, &bytes, |published| {
            let mut fresh_head = AuthenticatedCanaryHead::new(&loaded.verified, &receipt.artifact)
                .map_err(|error| error.to_string())?;
            let head = fresh_head
                .refresh(&client, &loaded.verified)
                .map_err(|error| error.to_string())?;
            require_canary_expiry_margin(head, self.canary_expires_at_height)
                .map_err(|error| error.to_string())?;
            let fresh_time = current_unix_ms().map_err(|error| error.to_string())?;
            let authorization = KagemushaV4TairaCanaryAuthorizationV1::decode_canonical(published)
                .map_err(|error| error.to_string())?;
            let verified = authorization
                .verify_exact(
                    published,
                    &loaded.verified,
                    &receipt.artifact,
                    &receipt.exact_bytes,
                    fresh_time,
                )
                .map_err(|error| error.to_string())?;
            require_canary_authorization_wall_margin(&verified, fresh_time)
                .map_err(|error| error.to_string())
        })?;
        let report = norito::json!({
            "status": "authorized",
            "output": (self.output.display().to_string()),
            "byte_len": (u64::try_from(bytes.len()).unwrap_or(u64::MAX)),
            "sha256": (hex::encode(KagemushaExactBytesDigestV1::from_bytes(&bytes)?.sha256)),
            "promotion_id": (hex::encode(verified.promotion_id())),
            "canary_transaction_intent": (verified.canary_transaction_intent().to_string()),
            "expires_at_unix_ms": (verified.expires_at_unix_ms()),
            "expires_at_height": (verified.expires_at_height().get()),
        });
        context.print_data(&report).map_err(|error| {
            eyre!(PublicationError::CommitUncertain {
                path: self.output,
                detail: format!("published canary-authorization report failed: {error}"),
            })
        })
    }
}

#[derive(ClapArgs, Debug)]
struct SubmitCanaryAuthorization {
    #[command(flatten)]
    trusted: TrustedInputs,
    /// Exact immutable issuer-signed activation-finality receipt.
    #[arg(long, value_name = "ABSOLUTE_PATH")]
    activation_receipt: PathBuf,
    /// Exact controller-signed canary authorization to reserve on-chain.
    #[arg(long, value_name = "ABSOLUTE_PATH")]
    canary_authorization: PathBuf,
    /// Explicit authorization for this production reservation write.
    #[arg(long, required = true, action = clap::ArgAction::SetTrue)]
    write_authorized: bool,
}

impl SubmitCanaryAuthorization {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        if !self.write_authorized {
            bail!("--write-authorized is required for production canary authorization");
        }
        require_root()?;
        let loaded = load_verified_expectations(&self.trusted)?;
        let receipt = load_verified_receipt(&self.activation_receipt, &loaded)?;
        let authorization =
            load_verified_canary_authorization(&self.canary_authorization, &loaded, &receipt)?;
        let mut client = context.client_from_config();
        require_canary_client_binding(&client, &authorization)?;
        let journal_path = rollout_state_path(
            authorization.verified.promotion_id(),
            CANARY_AUTHORIZATION_SUBMISSION_JOURNAL_FILE_NAME,
        )?;
        let transaction = match load_canary_authorization_submission_journal(
            &journal_path,
            &authorization,
            &client,
        )? {
            Some(transaction) => transaction,
            None => {
                let mut authenticated_head =
                    AuthenticatedCanaryHead::new(&loaded.verified, &receipt.artifact)?;
                let live_head = authenticated_head.refresh(&client, &loaded.verified)?;
                require_canary_expiry_margin(
                    live_head,
                    authorization.verified.expires_at_height(),
                )?;
                let now = current_unix_ms()?;
                authorization
                    .artifact
                    .verify_exact(
                        &authorization.exact_bytes,
                        &loaded.verified,
                        &receipt.artifact,
                        &receipt.exact_bytes,
                        now,
                    )
                    .wrap_err("canary authorization expired before reservation construction")?;
                require_canary_authorization_wall_margin(&authorization.verified, now)?;
                let remaining = authorization
                    .verified
                    .expires_at_unix_ms()
                    .checked_sub(now)
                    .and_then(|remaining| remaining.checked_sub(CANARY_CONSTRUCTION_HEADROOM_MS))
                    .filter(|remaining| *remaining != 0)
                    .ok_or_else(|| eyre!("canary authorization has no safe reservation TTL"))?;
                client.add_transaction_nonce = true;
                client.transaction_ttl = Some(Duration::from_millis(remaining));
                let executable = Executable::Instructions(
                    vec![InstructionBox::from(AuthorizeKagemushaTairaCanaryV4::new(
                        authorization.artifact.reservation().clone(),
                    ))]
                    .into(),
                );
                let fee_payment = context.transaction_fee_payment()?;
                let mut payload = client
                    .try_build_transaction_payload(executable, fee_payment, Default::default())
                    .wrap_err("failed to build exact unsigned canary reservation")?;
                payload.admission_intent = TransactionAdmissionIntent::Ordinary;
                let transaction = client
                    .quote_and_sign_transaction_payload(payload)
                    .wrap_err("failed to fee-quote and sign exact canary reservation")?;
                verify_canary_authorization_submission_transaction(
                    &transaction,
                    &authorization,
                    &client,
                )?;
                let exact = transaction.encode_wire_v1().map_err(|error| {
                    eyre!("failed to encode exact canary reservation transaction: {error}")
                })?;
                publish_root_owned(&journal_path, &exact, |published| {
                    let transaction = verify_canary_authorization_submission_journal_bytes(
                        published,
                        &authorization,
                        &client,
                    )
                    .map_err(|error| error.to_string())?;
                    let mut fresh_head =
                        AuthenticatedCanaryHead::new(&loaded.verified, &receipt.artifact)
                            .map_err(|error| error.to_string())?;
                    let head = fresh_head
                        .refresh(&client, &loaded.verified)
                        .map_err(|error| error.to_string())?;
                    require_canary_expiry_margin(head, authorization.verified.expires_at_height())
                        .map_err(|error| error.to_string())?;
                    let now = current_unix_ms().map_err(|error| error.to_string())?;
                    authorization
                        .artifact
                        .verify_for_authorization_execution(
                            &client.network_id,
                            &client.account,
                            now,
                            head.checked_add(1)
                                .ok_or_else(|| "authenticated canary head overflow".to_owned())?,
                        )
                        .map_err(|error| error.to_string())?;
                    require_canary_authorization_wall_margin(&authorization.verified, now)
                        .map_err(|error| error.to_string())?;
                    if client
                        .get_transaction_status_response_auto(transaction.hash())
                        .map_err(|error| error.to_string())?
                        .is_some()
                    {
                        return Err(
                            "canary reservation already has status before journal commit"
                                .to_owned(),
                        );
                    }
                    Ok(())
                })?;
                transaction
            }
        };

        let canary_journal_path = rollout_state_path(
            authorization.verified.promotion_id(),
            CANARY_SUBMISSION_JOURNAL_FILE_NAME,
        )?;
        match inspect_canary_submission_journal(
            &canary_journal_path,
            &authorization,
            &loaded,
            &receipt,
            authorization.verified.authorized_at_unix_ms(),
        )? {
            SubmissionJournalObservation::Mismatched => {
                return Err(
                    eyre!(SubmissionJournalDecisionError::Mismatch).wrap_err(format!(
                        "unsafe canary submission journal at `{}`",
                        canary_journal_path.display()
                    )),
                );
            }
            SubmissionJournalObservation::Matching => {}
            SubmissionJournalObservation::Absent => {
                if client
                    .get_transaction_status_response_auto(transaction.hash())
                    .wrap_err(
                        "failed to prove the reservation transaction absent before canary journal publication",
                    )?
                    .is_some()
                {
                    return Err(eyre!(SubmissionJournalDecisionError::Retrospective).wrap_err(
                        "canary journal cannot be published after its exact transaction was revealed on-chain",
                    ));
                }
                if client
                    .get_transaction_status_response_auto(
                        authorization.verified.canary_transaction().hash(),
                    )
                    .wrap_err(
                        "failed to prove the canary absent before canary journal publication",
                    )?
                    .is_some()
                {
                    return Err(eyre!(SubmissionJournalDecisionError::Retrospective).wrap_err(
                        "canary journal cannot be published after the exact canary was submitted",
                    ));
                }
                publish_canary_submission_journal(
                    &canary_journal_path,
                    &authorization,
                    &loaded,
                    &receipt,
                    &client,
                )?;
                if inspect_canary_submission_journal(
                    &canary_journal_path,
                    &authorization,
                    &loaded,
                    &receipt,
                    authorization.verified.authorized_at_unix_ms(),
                )? != SubmissionJournalObservation::Matching
                {
                    bail!("durable canary submission journal could not be reverified");
                }
            }
        }

        let exact = transaction.encode_wire_v1().map_err(|error| {
            eyre!("failed to encode journaled canary reservation transaction: {error}")
        })?;
        let prepared = Client::prepare_transaction_payload(&transaction);
        if prepared.as_bytes() != exact.as_slice() {
            bail!("prepared canary reservation changed the journaled transaction wire");
        }
        let hash = prepared.hash();
        let mut status = client
            .get_transaction_status_response_auto(hash)
            .map_err(|error| {
                eyre!(CanarySubmissionUncertain {
                    transaction_hash: hash.to_string(),
                    stage: "canary reservation pre-POST reconciliation",
                    journal: journal_path.clone(),
                    detail: error.to_string(),
                })
            })?;
        if status.is_none() {
            let mut authenticated_head =
                AuthenticatedCanaryHead::new(&loaded.verified, &receipt.artifact)?;
            let live_head = authenticated_head.refresh(&client, &loaded.verified)?;
            require_canary_expiry_margin(live_head, authorization.verified.expires_at_height())?;
            let now = current_unix_ms()?;
            authorization
                .artifact
                .verify_for_authorization_execution(
                    &client.network_id,
                    &client.account,
                    now,
                    live_head
                        .checked_add(1)
                        .ok_or_else(|| eyre!("authenticated canary head overflow"))?,
                )
                .wrap_err("canary reservation authorization expired before POST")?;
            require_canary_authorization_wall_margin(&authorization.verified, now)?;
            if let Err(post_error) = client.submit_prepared_transaction_payload(&prepared) {
                status = client
                    .get_transaction_status_response_auto(hash)
                    .map_err(|error| {
                        eyre!(CanarySubmissionUncertain {
                            transaction_hash: hash.to_string(),
                            stage: "ambiguous canary reservation POST reconciliation",
                            journal: journal_path.clone(),
                            detail: format!("POST failed: {post_error}; status failed: {error}"),
                        })
                    })?;
                if status.is_none() {
                    return Err(eyre!(CanarySubmissionUncertain {
                        transaction_hash: hash.to_string(),
                        stage: "ambiguous canary reservation POST reconciliation",
                        journal: journal_path,
                        detail: format!("POST failed and no status is visible: {post_error}"),
                    }));
                }
            }
        }
        if let Some(observed) = status.as_ref() {
            require_journal_bound_status_response(
                observed,
                &transaction,
                &journal_path,
                "canary reservation status identity reconciliation",
            )?;
            match classify_reconciled_submission_status(Some(&observed.status.kind)) {
                ReconciledSubmissionStatus::Rejected | ReconciledSubmissionStatus::Expired => {
                    bail!(
                        "exact canary reservation reached terminal status {} instead of Applied",
                        observed.status.kind
                    );
                }
                ReconciledSubmissionStatus::Applied | ReconciledSubmissionStatus::Unresolved => {}
            }
        }
        let outcome = wait_for_activation_terminal_status(&client, hash).map_err(|error| {
            eyre!(CanarySubmissionUncertain {
                transaction_hash: hash.to_string(),
                stage: "canary reservation terminal wait",
                journal: journal_path.clone(),
                detail: error.to_string(),
            })
        })?;
        require_journal_bound_wait_outcome(&outcome, &transaction, &journal_path)?;
        if classify_reconciled_submission_status(Some(&outcome.terminal_kind))
            != ReconciledSubmissionStatus::Applied
        {
            bail!(
                "exact canary reservation reached terminal status {} instead of Applied",
                outcome.terminal_kind
            );
        }
        let carrier_height = applied_carrier_height_for_submission(
            &outcome.r#final,
            &transaction,
            &journal_path,
            "canary reservation Applied carrier-height reconciliation",
        )?;
        require_canary_expiry_margin(
            carrier_height.get(),
            authorization.verified.expires_at_height(),
        )?;
        collect_finalized_activation_evidence(
            &client,
            &transaction,
            &exact,
            &loaded.verified,
            carrier_height,
        )
        .wrap_err("exact canary reservation failed four-validator finality verification")?;
        context.print_data(&norito::json!({
            "status": "Applied",
            "transaction_hash": (hash.to_string()),
            "carrier_height": (carrier_height.get()),
            "submission_journal": (journal_path.display().to_string()),
            "reserved_canary_entrypoint": (authorization.verified.canary_transaction().hash_as_entrypoint().to_string()),
        }))
    }
}

#[derive(ClapArgs, Debug)]
struct SubmitCanary {
    #[command(flatten)]
    trusted: TrustedInputs,
    /// Exact immutable issuer-signed activation-finality receipt.
    #[arg(long, value_name = "ABSOLUTE_PATH")]
    activation_receipt: PathBuf,
    /// Exact controller-signed canary authorization.
    #[arg(long, value_name = "ABSOLUTE_PATH")]
    canary_authorization: PathBuf,
    /// Explicit authorization for this production canary network write.
    #[arg(long, required = true, action = clap::ArgAction::SetTrue)]
    write_authorized: bool,
}

impl SubmitCanary {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        if !self.write_authorized {
            bail!("--write-authorized is required for production canary submission");
        }
        require_root()?;
        let loaded = load_verified_expectations(&self.trusted)?;
        let receipt = load_verified_receipt(&self.activation_receipt, &loaded)?;
        let authorization =
            load_verified_canary_authorization(&self.canary_authorization, &loaded, &receipt)?;
        let client = context.client_from_config();
        require_canary_client_binding(&client, &authorization)?;
        require_finalized_canary_authorization(&client, &authorization, &loaded)?;
        let transaction = authorization.verified.canary_transaction();
        let exact_wire = transaction
            .encode_wire_v1()
            .map_err(|error| eyre!("failed to encode authorized canary transaction: {error}"))?;
        if !authorization
            .verified
            .canary_transaction_wire()
            .matches_bytes(&exact_wire)
        {
            bail!("authorized canary transaction wire is not exact");
        }
        let prepared = Client::prepare_transaction_payload(transaction);
        if prepared.as_bytes() != exact_wire.as_slice() {
            bail!("prepared submission changed the authorized canary transaction wire");
        }
        let hash = prepared.hash();
        let journal_path = rollout_state_path(
            authorization.verified.promotion_id(),
            CANARY_SUBMISSION_JOURNAL_FILE_NAME,
        )?;
        if inspect_canary_submission_journal(
            &journal_path,
            &authorization,
            &loaded,
            &receipt,
            authorization.verified.authorized_at_unix_ms(),
        )? != SubmissionJournalObservation::Matching
        {
            bail!(
                "canary submission refused: the exact journal must be committed before the on-chain authorization reveals the signed transaction"
            );
        }
        let mut authenticated_head = None;

        let initial_status =
            client
                .get_transaction_status_response_auto(hash)
                .map_err(|error| {
                    eyre!(CanarySubmissionUncertain {
                        transaction_hash: hash.to_string(),
                        stage: "post-journal pre-POST status reconciliation",
                        journal: journal_path.clone(),
                        detail: error.to_string(),
                    })
                })?;
        if let Some(status) = initial_status.as_ref()
            && finish_observed_canary_status(
                context,
                &client,
                &loaded,
                &receipt,
                &authorization,
                &journal_path,
                &exact_wire,
                status,
                "resumed from durable journal",
            )?
        {
            return Ok(());
        }

        let mut post_error = None;
        if initial_status.is_none() {
            let tracker = if let Some(tracker) = authenticated_head.as_mut() {
                tracker
            } else {
                authenticated_head.insert(AuthenticatedCanaryHead::new(
                    &loaded.verified,
                    &receipt.artifact,
                )?)
            };
            let live_head = tracker.refresh(&client, &loaded.verified)?;
            require_canary_expiry_margin(live_head, authorization.verified.expires_at_height())?;
            let fresh_time = current_unix_ms()?;
            authorization
                .artifact
                .verify_exact(
                    &authorization.exact_bytes,
                    &loaded.verified,
                    &receipt.artifact,
                    &receipt.exact_bytes,
                    fresh_time,
                )
                .wrap_err("canary authorization expired before POST")?;
            if let Err(error) = client.submit_prepared_transaction_payload(&prepared) {
                post_error = Some(error.to_string());
                match client.get_transaction_status_response_auto(hash) {
                    Ok(Some(status)) => {
                        if finish_observed_canary_status(
                            context,
                            &client,
                            &loaded,
                            &receipt,
                            &authorization,
                            &journal_path,
                            &exact_wire,
                            &status,
                            "proof-anchored after ambiguous POST",
                        )? {
                            return Ok(());
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        post_error = Some(format!(
                            "{}; immediate status reconciliation also failed: {error}",
                            post_error.as_deref().unwrap_or("POST result was ambiguous")
                        ));
                    }
                }
            }
        }

        let outcome = match wait_for_activation_terminal_status(&client, hash) {
            Ok(outcome) => outcome,
            Err(wait_error) => {
                match client.get_transaction_status_response_auto(hash) {
                    Ok(Some(status)) => {
                        if finish_observed_canary_status(
                            context,
                            &client,
                            &loaded,
                            &receipt,
                            &authorization,
                            &journal_path,
                            &exact_wire,
                            &status,
                            "proof-anchored after failed terminal wait",
                        )? {
                            return Ok(());
                        }
                    }
                    Ok(None) => {}
                    Err(status_error) => {
                        return Err(eyre!(CanarySubmissionUncertain {
                            transaction_hash: hash.to_string(),
                            stage: "failed wait transport reconciliation",
                            journal: journal_path,
                            detail: format!(
                                "terminal wait failed: {wait_error}; final status query failed: {status_error}"
                            ),
                        }));
                    }
                }
                return Err(eyre!(CanarySubmissionUncertain {
                    transaction_hash: hash.to_string(),
                    stage: "failed wait status reconciliation",
                    journal: journal_path,
                    detail: format!(
                        "{}terminal wait failed without a provable terminal result: {wait_error}",
                        post_error
                            .as_deref()
                            .map(|error| format!("ambiguous POST: {error}; "))
                            .unwrap_or_default()
                    ),
                }));
            }
        };
        require_canary_wait_outcome(&outcome, transaction, &journal_path)?;
        if finish_observed_canary_status(
            context,
            &client,
            &loaded,
            &receipt,
            &authorization,
            &journal_path,
            &exact_wire,
            &outcome.r#final,
            "terminal wait",
        )? {
            return Ok(());
        }
        Err(eyre!(CanarySubmissionUncertain {
            transaction_hash: hash.to_string(),
            stage: "configured terminal wait",
            journal: journal_path,
            detail: format!(
                "wait stopped without a supported terminal status: {}",
                outcome.terminal_kind
            ),
        }))
    }
}

fn require_canary_wait_outcome(
    outcome: &TransactionWaitOutcome,
    transaction: &SignedTransaction,
    journal_path: &Path,
) -> Result<()> {
    let expected = transaction.hash().to_string();
    if outcome.hash != expected
        || outcome.r#final.hash != expected
        || outcome.terminal_kind != outcome.r#final.status.kind
        || outcome.block_height != outcome.r#final.status.block_height
        || outcome.scope != outcome.r#final.scope
        || outcome.resolved_from != outcome.r#final.resolved_from
    {
        return Err(eyre!(CanarySubmissionUncertain {
            transaction_hash: expected,
            stage: "terminal wait identity reconciliation",
            journal: journal_path.to_path_buf(),
            detail: "terminal wait summary differs from the authorized canary or final status"
                .to_owned(),
        }));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn finish_observed_canary_status<C: RunContext>(
    context: &mut C,
    client: &Client,
    loaded: &LoadedVerifiedExpectations,
    receipt: &LoadedVerifiedReceipt,
    authorization: &LoadedVerifiedCanaryAuthorization,
    journal_path: &Path,
    exact_wire: &[u8],
    status: &PipelineTransactionStatusResponse,
    reconciliation: &'static str,
) -> Result<bool> {
    let transaction = authorization.verified.canary_transaction();
    if let Err(error) = require_status_response_hash(status, transaction) {
        return Err(eyre!(CanarySubmissionUncertain {
            transaction_hash: transaction.hash().to_string(),
            stage: "status identity reconciliation",
            journal: journal_path.to_path_buf(),
            detail: error.to_string(),
        }));
    }
    match classify_reconciled_submission_status(Some(&status.status.kind)) {
        ReconciledSubmissionStatus::Applied => {
            let carrier_height = applied_carrier_height(status).map_err(|error| {
                eyre!(CanarySubmissionUncertain {
                    transaction_hash: transaction.hash().to_string(),
                    stage: "Applied carrier-height reconciliation",
                    journal: journal_path.to_path_buf(),
                    detail: error.to_string(),
                })
            })?;
            if carrier_height.get() >= authorization.verified.expires_at_height().get() {
                bail!("canary was applied at or after its exclusive height expiry");
            }
            let evidence = collect_finalized_canary_evidence(
                client,
                transaction,
                exact_wire,
                &loaded.verified,
                &receipt.artifact,
                carrier_height,
            )
            .map_err(|error| {
                eyre!(CanarySubmissionUncertain {
                    transaction_hash: transaction.hash().to_string(),
                    stage: "Applied proof reconciliation",
                    journal: journal_path.to_path_buf(),
                    detail: error.to_string(),
                })
            })?;
            require_canary_block_within_authorization(
                &evidence.block_bytes,
                &authorization.verified,
            )
            .map_err(|error| {
                eyre!(CanarySubmissionUncertain {
                    transaction_hash: transaction.hash().to_string(),
                    stage: "Applied authorization-window reconciliation",
                    journal: journal_path.to_path_buf(),
                    detail: error.to_string(),
                })
            })?;
            let report = norito::json!({
                "status": "Applied",
                "transaction_hash": (transaction.hash().to_string()),
                "carrier_height": (evidence.carrier_height.get()),
                "reconciliation": (reconciliation),
                "submission_journal": (journal_path.display().to_string()),
            });
            context.print_data(&report).map_err(|error| {
                eyre!(CanarySubmissionUncertain {
                    transaction_hash: transaction.hash().to_string(),
                    stage: "proof-anchored Applied result reporting",
                    journal: journal_path.to_path_buf(),
                    detail: format!(
                        "canary is proof-anchored Applied, but reporting failed: {error}"
                    ),
                })
            })?;
            Ok(true)
        }
        ReconciledSubmissionStatus::Rejected | ReconciledSubmissionStatus::Expired => {
            bail!(
                "production canary reached terminal status {} instead of Applied",
                status.status.kind
            )
        }
        ReconciledSubmissionStatus::Unresolved => Ok(false),
    }
}

#[derive(ClapArgs, Debug)]
struct FinalizeCanaryEvidence {
    #[command(flatten)]
    trusted: TrustedInputs,
    /// Exact immutable issuer-signed activation-finality receipt.
    #[arg(long, value_name = "ABSOLUTE_PATH")]
    activation_receipt: PathBuf,
    /// Exact controller-signed canary authorization.
    #[arg(long, value_name = "ABSOLUTE_PATH")]
    canary_authorization: PathBuf,
    /// Runtime-only owner-private receipt-issuer key file.
    #[arg(long, value_name = "ABSOLUTE_PATH")]
    issuer_private_key_file: PathBuf,
    /// Exact absent promotion-keyed canary-evidence destination; it is never replaced.
    #[arg(long, value_name = "ABSOLUTE_PATH")]
    output: PathBuf,
}

impl FinalizeCanaryEvidence {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        require_root()?;
        let loaded = load_verified_expectations(&self.trusted)?;
        let expectations = &loaded.verified;
        let receipt = load_verified_receipt(&self.activation_receipt, &loaded)?;
        let authorization =
            load_verified_canary_authorization(&self.canary_authorization, &loaded, &receipt)?;
        require_rollout_state_path(
            &self.output,
            expectations.binding().promotion_id,
            CANARY_EVIDENCE_FILE_NAME,
        )?;
        preflight_root_owned_output(&self.output)?;
        let journal_path = rollout_state_path(
            expectations.binding().promotion_id,
            CANARY_SUBMISSION_JOURNAL_FILE_NAME,
        )?;
        if inspect_canary_submission_journal(
            &journal_path,
            &authorization,
            &loaded,
            &receipt,
            authorization.verified.authorized_at_unix_ms(),
        )? != SubmissionJournalObservation::Matching
        {
            bail!("canary evidence refused without the exact durable submission journal");
        }
        let transaction = authorization.verified.canary_transaction();
        let exact_transaction_wire = transaction
            .encode_wire_v1()
            .map_err(|error| eyre!("failed to encode authorized canary: {error}"))?;
        let client = context.client_from_config();
        require_canary_client_binding(&client, &authorization)?;
        require_finalized_canary_authorization(&client, &authorization, &loaded)?;

        let query_started_at_unix_ms = current_unix_ms()?;
        let status_before = client
            .get_status()
            .wrap_err("failed to capture pre-query Taira node status")?;
        let pipeline_status = client
            .get_transaction_status_response_auto(transaction.hash())
            .wrap_err("failed to query global canary pipeline status")?
            .ok_or_else(|| eyre!("canary transaction is absent from global pipeline status"))?;
        require_status_response_hash(&pipeline_status, transaction)?;
        let pipeline_height = pipeline_status.status.block_height.ok_or_else(|| {
            eyre!("Applied canary status does not carry a finalized block height")
        })?;
        if pipeline_status.status.kind != "Applied"
            || pipeline_status.scope != "global"
            || pipeline_status.resolved_from != "state"
            || pipeline_height <= receipt.finalized_height
            || pipeline_height >= authorization.verified.expires_at_height().get()
        {
            bail!(
                "canary status must be global/state Applied strictly after activation and before its exclusive height expiry"
            );
        }
        let carrier_height = NonZeroU64::new(pipeline_height)
            .ok_or_else(|| eyre!("canary carrier height is zero"))?;
        let fresh = collect_finalized_canary_evidence(
            &client,
            transaction,
            &exact_transaction_wire,
            expectations,
            &receipt.artifact,
            carrier_height,
        )
        .wrap_err("canary transaction/block/finality query failed")?;
        require_canary_block_within_authorization(&fresh.block_bytes, &authorization.verified)?;
        if fresh.transaction_details_trigger_completion_count != 0 {
            bail!("canary Log transaction unexpectedly completed triggers");
        }
        let status_after = client
            .get_status()
            .wrap_err("failed to capture post-query Taira node status")?;
        let query_completed_at_unix_ms = current_unix_ms()?;

        let finality_proof_chain =
            KagemushaV4ActivationFinalityProofChainV1::try_from(fresh.proofs.clone())
                .wrap_err("canary finality extension is invalid")?;
        let finalized_block_wire =
            KagemushaFinalizedBlockWireV1::try_from_bytes(fresh.block_bytes.clone())?;
        let finalized_block_wire_digest =
            KagemushaExactBytesDigestV1::from_bytes(&fresh.block_bytes)?;
        let body = KagemushaV4TairaCanaryEvidenceBodyV1 {
            schema: KAGEMUSHA_V4_TAIRA_CANARY_EVIDENCE_BODY_SCHEMA.to_owned(),
            version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            promotion_controller: expectations.promotion_controller().clone(),
            promotion_id: expectations.binding().promotion_id,
            network_id: expectations.binding().network_id.clone(),
            promotion_reservation: expectations.promotion_reservation(),
            activation_expectations_artifact: expectations.activation_expectations_artifact(),
            activation_finality_receipt: KagemushaExactBytesDigestV1::from_bytes(
                &receipt.exact_bytes,
            )?,
            canary_authorization: authorization.verified.authorization_identity(),
            issuer: expectations.receipt_issuer().clone(),
            activation_transaction_intent: expectations.activation_transaction_intent(),
            activation_finalized_height: receipt.finalized_height,
            activation_finalized_block_hash: receipt.finalized_block_hash,
            canary_transaction_intent: authorization.verified.canary_transaction_intent(),
            canary_transaction_wire: authorization.verified.canary_transaction_wire(),
            committed_transaction: fresh.committed.clone(),
            finalized_block_wire,
            finalized_block_wire_digest,
            finality_proof_chain: finality_proof_chain.clone(),
            finalized_height: fresh.carrier_height.get(),
            finalized_block_hash: fresh.committed.block_hash().clone(),
            query: KagemushaV4TairaCanaryQueryObservationV1 {
                query_started_at_unix_ms,
                query_completed_at_unix_ms,
                pipeline_status_response_norito: exact_canonical_digest(&pipeline_status)?,
                pipeline_status_scope: pipeline_status.scope,
                pipeline_status_resolved_from: pipeline_status.resolved_from,
                pipeline_transaction_intent: transaction.hash(),
                pipeline_status_kind: pipeline_status.status.kind,
                pipeline_status_block_height: pipeline_height,
                transaction_details_response_norito: fresh.transaction_details_response_norito,
                transaction_details_trigger_completion_count: fresh
                    .transaction_details_trigger_completion_count,
                node_status_before_norito: exact_canonical_digest(&status_before)?,
                node_status_before_observed_at_ms: status_before.observed_at_ms,
                node_status_before_height: status_before.blocks,
                node_status_after_norito: exact_canonical_digest(&status_after)?,
                node_status_after_observed_at_ms: status_after.observed_at_ms,
                node_status_after_height: status_after.blocks,
                committed_transaction_norito: exact_canonical_digest(&fresh.committed)?,
                finalized_block_wire: KagemushaExactBytesDigestV1::from_bytes(&fresh.block_bytes)?,
                finality_proof_chain_norito: exact_canonical_digest(&finality_proof_chain)?,
                finality_proof_count: u32::try_from(fresh.proofs.len())
                    .wrap_err("fresh canary proof count does not fit u32")?,
            },
        };
        let issuer_key =
            load_root_custodied_key(&self.issuer_private_key_file, "receipt-issuer key")?;
        if issuer_key.public_key() != expectations.receipt_issuer() {
            bail!("receipt-issuer key file does not match authenticated expectations");
        }
        let evidence = KagemushaV4TairaCanaryEvidenceV1::try_sign(
            body,
            &issuer_key,
            &authorization.artifact,
            &authorization.exact_bytes,
            expectations,
            &receipt.artifact,
            &receipt.exact_bytes,
        )
        .wrap_err("failed to sign complete promotion-bound Taira canary evidence")?;
        let bytes = norito::encode_canonical(&evidence)
            .wrap_err("failed to encode promotion-bound Taira canary evidence")?;
        let verified = evidence
            .verify_exact(
                &bytes,
                &authorization.artifact,
                &authorization.exact_bytes,
                expectations,
                &receipt.artifact,
                &receipt.exact_bytes,
            )
            .wrap_err("promotion-bound Taira canary evidence failed exact verification")?;
        publish_root_owned(&self.output, &bytes, |published| {
            let evidence = KagemushaV4TairaCanaryEvidenceV1::decode_canonical(published)
                .map_err(|error| error.to_string())?;
            evidence
                .verify_exact(
                    published,
                    &authorization.artifact,
                    &authorization.exact_bytes,
                    expectations,
                    &receipt.artifact,
                    &receipt.exact_bytes,
                )
                .map(|_| ())
                .map_err(|error| error.to_string())
        })?;
        let report = norito::json!({
            "status": "finalized",
            "output": (self.output.display().to_string()),
            "byte_len": (u64::try_from(bytes.len()).unwrap_or(u64::MAX)),
            "sha256": (hex::encode(KagemushaExactBytesDigestV1::from_bytes(&bytes)?.sha256)),
            "promotion_id": (hex::encode(verified.promotion_id())),
            "finalized_height": (verified.finalized_height()),
            "finalized_block_hash": (verified.finalized_block_hash().to_string()),
            "activation_transaction_intent": (verified.activation_transaction_intent().to_string()),
            "canary_transaction_intent": (verified.canary_transaction_intent().to_string()),
            "query_started_at_unix_ms": (query_started_at_unix_ms),
            "query_completed_at_unix_ms": (query_completed_at_unix_ms),
        });
        context.print_data(&report).map_err(|error| {
            eyre!(PublicationError::CommitUncertain {
                path: self.output,
                detail: format!("published canary-evidence report failed: {error}"),
            })
        })
    }
}

struct LoadedVerifiedExpectations {
    artifact: KagemushaV4ActivationReceiptExpectationsArtifactV1,
    verified: KagemushaV4ActivationReceiptExpectationsV1,
    exact_bytes: Vec<u8>,
    controller: PublicKey,
    reservation_bytes: Vec<u8>,
}

struct LoadedVerifiedReceipt {
    artifact: KagemushaV4ActivationFinalityReceiptV1,
    exact_bytes: Vec<u8>,
    finalized_height: u64,
    finalized_block_hash: HashOf<iroha::data_model::block::BlockHeader>,
}

struct LoadedVerifiedCanaryAuthorization {
    artifact: KagemushaV4TairaCanaryAuthorizationV1,
    verified: KagemushaV4VerifiedTairaCanaryAuthorizationV1,
    exact_bytes: Vec<u8>,
}

fn load_verified_expectations(input: &TrustedInputs) -> Result<LoadedVerifiedExpectations> {
    let controller = parse_public_key(&input.promotion_controller, "promotion controller")?;
    let reservation_bytes = read_root_owned(
        &input.promotion_reservation,
        KAGEMUSHA_V4_PROMOTION_RESERVATION_MAX_BYTES,
        "promotion reservation",
    )?;
    let bytes = read_root_private_artifact(
        &input.expectations,
        KAGEMUSHA_V4_ACTIVATION_EXPECTATIONS_MAX_BYTES,
        "activation expectations",
    )?;
    let artifact = KagemushaV4ActivationReceiptExpectationsArtifactV1::decode_canonical(&bytes)
        .wrap_err("invalid activation expectations")?;
    let verified = artifact
        .verify_exact(&bytes, &controller, &reservation_bytes)
        .wrap_err("activation expectations failed exact verification")?;
    require_rollout_state_path(
        &input.expectations,
        verified.binding().promotion_id,
        EXPECTATIONS_FILE_NAME,
    )?;
    Ok(LoadedVerifiedExpectations {
        artifact,
        verified,
        exact_bytes: bytes,
        controller,
        reservation_bytes,
    })
}

fn load_verified_receipt(
    path: &Path,
    loaded: &LoadedVerifiedExpectations,
) -> Result<LoadedVerifiedReceipt> {
    require_rollout_state_path(
        path,
        loaded.verified.binding().promotion_id,
        RECEIPT_FILE_NAME,
    )?;
    let exact_bytes = read_root_private_artifact(
        path,
        KAGEMUSHA_V4_ACTIVATION_RECEIPT_MAX_BYTES,
        "activation-finality receipt",
    )?;
    let artifact = KagemushaV4ActivationFinalityReceiptV1::decode_canonical(&exact_bytes)
        .wrap_err("invalid activation-finality receipt")?;
    if norito::encode_canonical(&artifact)
        .wrap_err("failed to re-encode activation-finality receipt")?
        != exact_bytes
    {
        bail!("activation-finality receipt is not exact canonical Norito");
    }
    let verified = artifact
        .verify(&loaded.verified)
        .wrap_err("activation-finality receipt failed authenticated verification")?;
    Ok(LoadedVerifiedReceipt {
        finalized_height: verified.finalized_height(),
        finalized_block_hash: verified.finalized_block_hash(),
        artifact,
        exact_bytes,
    })
}

fn canonical_torii_origin(url: &url::Url) -> Result<String> {
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        bail!(
            "configured Torii URL must be one credential-free HTTP(S) origin with no path, query, or fragment"
        );
    }
    let origin = url.origin().ascii_serialization();
    if origin == "null" || url.as_str() != format!("{origin}/") {
        bail!("configured Torii URL is not the exact canonical origin form");
    }
    validate_kagemusha_v4_taira_canary_torii_origin(&origin)
        .wrap_err("configured Torii URL is not a qualified production canary origin")?;
    Ok(origin)
}

fn load_verified_canary_authorization(
    path: &Path,
    loaded: &LoadedVerifiedExpectations,
    receipt: &LoadedVerifiedReceipt,
) -> Result<LoadedVerifiedCanaryAuthorization> {
    require_rollout_state_path(
        path,
        loaded.verified.binding().promotion_id,
        CANARY_AUTHORIZATION_FILE_NAME,
    )?;
    let exact_bytes = read_root_private_artifact(
        path,
        KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_MAX_BYTES,
        "canary authorization",
    )?;
    let artifact = KagemushaV4TairaCanaryAuthorizationV1::decode_canonical(&exact_bytes)
        .wrap_err("invalid canary authorization")?;
    let verification_time_unix_ms = artifact.permit().body.authorized_at_unix_ms;
    let verified = artifact
        .verify_exact(
            &exact_bytes,
            &loaded.verified,
            &receipt.artifact,
            &receipt.exact_bytes,
            verification_time_unix_ms,
        )
        .wrap_err("canary authorization failed exact verification")?;
    Ok(LoadedVerifiedCanaryAuthorization {
        artifact,
        verified,
        exact_bytes,
    })
}

fn require_canary_client_binding(
    client: &Client,
    authorization: &LoadedVerifiedCanaryAuthorization,
) -> Result<()> {
    if &client.network_id != authorization.verified.network_id()
        || &client.account != &authorization.artifact.permit().body.canary_authority
        || canonical_torii_origin(&client.torii_url)?
            != authorization.verified.canonical_torii_origin()
    {
        bail!("configured Torii origin, network, or account differs from the canary authorization");
    }
    Ok(())
}

fn verify_canary_authorization_submission_transaction(
    transaction: &SignedTransaction,
    authorization: &LoadedVerifiedCanaryAuthorization,
    client: &Client,
) -> Result<()> {
    if transaction.network_id() != Some(&client.network_id)
        || transaction.authority() != &client.account
        || transaction.authority() != &authorization.artifact.permit().body.canary_authority
        || transaction.nonce().is_none()
        || transaction.time_to_live().is_none()
        || transaction.attachments().is_some()
        || transaction.admission_intent() != TransactionAdmissionIntent::Ordinary
    {
        bail!("journaled canary reservation transaction has an invalid envelope");
    }
    transaction
        .verify_signature()
        .wrap_err("journaled canary reservation transaction signature is invalid")?;
    let created_at = u64::try_from(transaction.creation_time().as_millis())
        .wrap_err("canary reservation creation time does not fit u64 milliseconds")?;
    let ttl = u64::try_from(
        transaction
            .time_to_live()
            .expect("checked above")
            .as_millis(),
    )
    .wrap_err("canary reservation TTL does not fit u64 milliseconds")?;
    if created_at
        .checked_add(ttl)
        .is_none_or(|expiry| expiry > authorization.verified.expires_at_unix_ms())
    {
        bail!("journaled canary reservation outlives the controller authorization");
    }
    let Executable::Instructions(instructions) = transaction.instructions() else {
        bail!("canary reservation transaction must contain direct instructions");
    };
    let [instruction] = instructions.as_ref() else {
        bail!("canary reservation transaction must contain exactly one instruction");
    };
    let Some(reservation) = instruction
        .as_any()
        .downcast_ref::<AuthorizeKagemushaTairaCanaryV4>()
    else {
        bail!("canary reservation transaction carries the wrong instruction");
    };
    if reservation.reservation() != authorization.artifact.reservation() {
        bail!("canary reservation transaction changed the exact signed hash projection");
    }
    Ok(())
}

fn verify_canary_authorization_submission_journal_bytes(
    bytes: &[u8],
    authorization: &LoadedVerifiedCanaryAuthorization,
    client: &Client,
) -> Result<SignedTransaction> {
    if bytes.is_empty() || bytes.len() > TRANSACTION_MAX_BYTES {
        bail!("canary reservation submission journal violates its fixed byte bound");
    }
    let transaction = SignedTransaction::decode_all_versioned(bytes)
        .map_err(|error| eyre!("invalid canary reservation submission journal: {error}"))?;
    if transaction
        .encode_wire_v1()
        .map_err(|error| eyre!("failed to re-encode canary reservation journal: {error}"))?
        != bytes
    {
        bail!("canary reservation submission journal is not exact canonical wire");
    }
    verify_canary_authorization_submission_transaction(&transaction, authorization, client)?;
    Ok(transaction)
}

fn load_canary_authorization_submission_journal(
    path: &Path,
    authorization: &LoadedVerifiedCanaryAuthorization,
    client: &Client,
) -> Result<Option<SignedTransaction>> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).wrap_err_with(|| {
            format!(
                "inspect canary authorization submission journal at `{}`",
                path.display()
            )
        }),
        Ok(_) => {
            let bytes = read_root_private_artifact(
                path,
                TRANSACTION_MAX_BYTES,
                "canary authorization submission journal",
            )?;
            verify_canary_authorization_submission_journal_bytes(&bytes, authorization, client)
                .map(Some)
        }
    }
}

fn require_finalized_canary_authorization(
    client: &Client,
    authorization: &LoadedVerifiedCanaryAuthorization,
    loaded: &LoadedVerifiedExpectations,
) -> Result<NonZeroU64> {
    let journal_path = rollout_state_path(
        authorization.verified.promotion_id(),
        CANARY_AUTHORIZATION_SUBMISSION_JOURNAL_FILE_NAME,
    )?;
    let transaction = load_canary_authorization_submission_journal(
        &journal_path,
        authorization,
        client,
    )?
    .ok_or_else(|| {
        eyre!("canary submission refused until the exact on-chain authorization journal exists")
    })?;
    let status = client
        .get_transaction_status_response_auto(transaction.hash())
        .wrap_err("failed to query exact on-chain canary authorization status")?
        .ok_or_else(|| eyre!("journaled on-chain canary authorization has no pipeline status"))?;
    require_journal_bound_status_response(
        &status,
        &transaction,
        &journal_path,
        "canary authorization prerequisite identity reconciliation",
    )?;
    if classify_reconciled_submission_status(Some(&status.status.kind))
        != ReconciledSubmissionStatus::Applied
    {
        bail!(
            "journaled on-chain canary authorization is not Applied: {}",
            status.status.kind
        );
    }
    let carrier_height = applied_carrier_height_for_submission(
        &status,
        &transaction,
        &journal_path,
        "canary authorization prerequisite carrier-height reconciliation",
    )?;
    require_canary_expiry_margin(
        carrier_height.get(),
        authorization.verified.expires_at_height(),
    )?;
    let exact = transaction.encode_wire_v1().map_err(|error| {
        eyre!("failed to encode journaled on-chain canary authorization: {error}")
    })?;
    collect_finalized_activation_evidence(
        client,
        &transaction,
        &exact,
        &loaded.verified,
        carrier_height,
    )
    .wrap_err("journaled on-chain canary authorization lacks exact four-validator finality")?;
    Ok(carrier_height)
}

fn verify_canary_submission_journal_bytes(
    bytes: &[u8],
    authorization: &LoadedVerifiedCanaryAuthorization,
    loaded: &LoadedVerifiedExpectations,
    receipt: &LoadedVerifiedReceipt,
    verification_time_unix_ms: u64,
) -> Result<()> {
    if bytes != authorization.exact_bytes {
        bail!("canary submission journal does not byte-match the authorization");
    }
    let artifact = KagemushaV4TairaCanaryAuthorizationV1::decode_canonical(bytes)
        .wrap_err("invalid canary submission-journal authorization")?;
    artifact
        .verify_exact(
            bytes,
            &loaded.verified,
            &receipt.artifact,
            &receipt.exact_bytes,
            verification_time_unix_ms,
        )
        .wrap_err("canary submission journal failed exact authorization reverification")?;
    Ok(())
}

fn inspect_canary_submission_journal(
    path: &Path,
    authorization: &LoadedVerifiedCanaryAuthorization,
    loaded: &LoadedVerifiedExpectations,
    receipt: &LoadedVerifiedReceipt,
    verification_time_unix_ms: u64,
) -> Result<SubmissionJournalObservation> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(SubmissionJournalObservation::Absent)
        }
        Err(error) => Err(error)
            .wrap_err_with(|| format!("inspect canary submission journal at `{}`", path.display())),
        Ok(_) => {
            let bytes = read_root_private_artifact(
                path,
                KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_MAX_BYTES,
                "canary submission journal",
            )?;
            if bytes != authorization.exact_bytes {
                return Ok(SubmissionJournalObservation::Mismatched);
            }
            verify_canary_submission_journal_bytes(
                &bytes,
                authorization,
                loaded,
                receipt,
                verification_time_unix_ms,
            )?;
            Ok(SubmissionJournalObservation::Matching)
        }
    }
}

fn publish_canary_submission_journal(
    path: &Path,
    authorization: &LoadedVerifiedCanaryAuthorization,
    loaded: &LoadedVerifiedExpectations,
    receipt: &LoadedVerifiedReceipt,
    client: &Client,
) -> Result<()> {
    publish_root_owned(path, &authorization.exact_bytes, |published| {
        let mut fresh_head = AuthenticatedCanaryHead::new(&loaded.verified, &receipt.artifact)
            .map_err(|error| error.to_string())?;
        let head = fresh_head
            .refresh(client, &loaded.verified)
            .map_err(|error| error.to_string())?;
        require_canary_expiry_margin(head, authorization.verified.expires_at_height())
            .map_err(|error| error.to_string())?;
        let verification_time_unix_ms = current_unix_ms().map_err(|error| error.to_string())?;
        verify_canary_submission_journal_bytes(
            published,
            authorization,
            loaded,
            receipt,
            verification_time_unix_ms,
        )
        .map_err(|error| error.to_string())?;
        require_canary_authorization_wall_margin(
            &authorization.verified,
            verification_time_unix_ms,
        )
        .map_err(|error| error.to_string())?;
        if client
            .get_transaction_status_response_auto(
                authorization.verified.canary_transaction().hash(),
            )
            .map_err(|error| error.to_string())?
            .is_some()
        {
            return Err("canary already has status before journal commit".to_owned());
        }
        Ok(())
    })
}

fn verify_submission_journal_bytes(
    bytes: &[u8],
    loaded: &LoadedVerifiedExpectations,
) -> Result<()> {
    if bytes != loaded.exact_bytes {
        bail!("submission journal does not byte-match the authenticated expectations");
    }
    let artifact = KagemushaV4ActivationReceiptExpectationsArtifactV1::decode_canonical(bytes)
        .wrap_err("invalid submission-journal expectations")?;
    artifact
        .verify_exact(bytes, &loaded.controller, &loaded.reservation_bytes)
        .wrap_err("submission journal failed exact expectations reverification")?;
    Ok(())
}

fn inspect_submission_journal(
    path: &Path,
    loaded: &LoadedVerifiedExpectations,
) -> Result<SubmissionJournalObservation> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(SubmissionJournalObservation::Absent)
        }
        Err(error) => Err(error)
            .wrap_err_with(|| format!("inspect submission journal at `{}`", path.display())),
        Ok(_) => {
            let bytes = read_root_private_artifact(
                path,
                KAGEMUSHA_V4_ACTIVATION_EXPECTATIONS_MAX_BYTES,
                "submission journal",
            )?;
            if bytes != loaded.exact_bytes {
                return Ok(SubmissionJournalObservation::Mismatched);
            }
            verify_submission_journal_bytes(&bytes, loaded)?;
            Ok(SubmissionJournalObservation::Matching)
        }
    }
}

fn publish_submission_journal(path: &Path, loaded: &LoadedVerifiedExpectations) -> Result<()> {
    // The journal payload is the complete canonical signed expectations artifact itself. Keeping
    // the exact bytes, rather than a digest-only wrapper, makes every resume re-run authentication
    // over the same authorization-bearing transaction bytes without introducing another schema.
    publish_root_owned(path, &loaded.exact_bytes, |published| {
        verify_submission_journal_bytes(published, loaded).map_err(|error| error.to_string())
    })
}

fn require_status_response_hash(
    status: &PipelineTransactionStatusResponse,
    transaction: &SignedTransaction,
) -> Result<()> {
    let expected = transaction.hash().to_string();
    if status.hash != expected {
        bail!(
            "pipeline status hash `{}` differs from requested transaction `{expected}`",
            status.hash
        );
    }
    Ok(())
}

fn require_journal_bound_status_response(
    status: &PipelineTransactionStatusResponse,
    transaction: &SignedTransaction,
    journal_path: &Path,
    stage: &'static str,
) -> Result<()> {
    require_status_response_hash(status, transaction).map_err(|error| {
        eyre!(SubmissionUncertain {
            transaction_hash: transaction.hash().to_string(),
            stage,
            journal: journal_path.to_path_buf(),
            detail: error.to_string(),
        })
    })
}

fn require_journal_bound_status_hash(
    observed_hash: &str,
    transaction: &SignedTransaction,
    journal_path: &Path,
    stage: &'static str,
) -> Result<()> {
    let expected = transaction.hash().to_string();
    if observed_hash != expected {
        return Err(eyre!(SubmissionUncertain {
            transaction_hash: expected.clone(),
            stage,
            journal: journal_path.to_path_buf(),
            detail: format!(
                "status envelope hash `{observed_hash}` differs from requested activation `{expected}`"
            ),
        }));
    }
    Ok(())
}

fn require_journal_bound_wait_outcome(
    outcome: &TransactionWaitOutcome,
    transaction: &SignedTransaction,
    journal_path: &Path,
) -> Result<()> {
    require_journal_bound_status_hash(
        &outcome.hash,
        transaction,
        journal_path,
        "terminal wait outcome identity reconciliation",
    )?;
    require_journal_bound_status_response(
        &outcome.r#final,
        transaction,
        journal_path,
        "terminal wait final status identity reconciliation",
    )?;
    if outcome.terminal_kind != outcome.r#final.status.kind
        || outcome.block_height != outcome.r#final.status.block_height
        || outcome.scope != outcome.r#final.scope
        || outcome.resolved_from != outcome.r#final.resolved_from
    {
        return Err(eyre!(SubmissionUncertain {
            transaction_hash: transaction.hash().to_string(),
            stage: "terminal wait outcome/final reconciliation",
            journal: journal_path.to_path_buf(),
            detail: "terminal wait summary differs from its final status response".to_owned(),
        }));
    }
    Ok(())
}

fn applied_carrier_height(status: &PipelineTransactionStatusResponse) -> Result<NonZeroU64> {
    if status.status.kind != "Applied" {
        bail!("pipeline status is not Applied: {}", status.status.kind);
    }
    NonZeroU64::new(
        status
            .status
            .block_height
            .ok_or_else(|| eyre!("Applied transaction has no carrier block height"))?,
    )
    .ok_or_else(|| eyre!("Applied transaction carrier height is zero"))
}

fn applied_carrier_height_for_submission(
    status: &PipelineTransactionStatusResponse,
    transaction: &SignedTransaction,
    journal_path: &Path,
    stage: &'static str,
) -> Result<NonZeroU64> {
    applied_carrier_height(status).map_err(|error| {
        eyre!(SubmissionUncertain {
            transaction_hash: transaction.hash().to_string(),
            stage,
            journal: journal_path.to_path_buf(),
            detail: error.to_string(),
        })
    })
}

fn wait_for_activation_terminal_status(
    client: &Client,
    hash: HashOf<SignedTransaction>,
) -> Result<TransactionWaitOutcome> {
    client.wait_for_transaction_terminal_status(
        hash,
        TransactionWaitOptions {
            timeout: client.transaction_status_timeout,
            terminal_statuses: vec![
                TransactionWaitTerminalStatus::Applied,
                TransactionWaitTerminalStatus::Rejected,
                TransactionWaitTerminalStatus::Expired,
            ],
            ..TransactionWaitOptions::default()
        },
    )
}

fn finish_waited_submission<C: RunContext>(
    context: &mut C,
    client: &Client,
    transaction: &SignedTransaction,
    exact_wire: &[u8],
    expectations: &KagemushaV4ActivationReceiptExpectationsV1,
    journal_path: &Path,
    outcome: TransactionWaitOutcome,
) -> Result<()> {
    require_journal_bound_wait_outcome(&outcome, transaction, journal_path)?;
    match classify_reconciled_submission_status(Some(&outcome.terminal_kind)) {
        ReconciledSubmissionStatus::Applied => {
            collect_finalized_activation_evidence(
                client,
                transaction,
                exact_wire,
                expectations,
                applied_carrier_height_for_submission(
                    &outcome.r#final,
                    transaction,
                    journal_path,
                    "Applied carrier-height reconciliation",
                )?,
            )
            .map_err(|error| {
                eyre!(SubmissionUncertain {
                    transaction_hash: transaction.hash().to_string(),
                    stage: "Applied proof collection",
                    journal: journal_path.to_path_buf(),
                    detail: error.to_string(),
                })
            })?;
            context.print_data(&outcome).map_err(|error| {
                eyre!(SubmissionUncertain {
                    transaction_hash: transaction.hash().to_string(),
                    stage: "proof-anchored Applied result reporting",
                    journal: journal_path.to_path_buf(),
                    detail: format!(
                        "activation is proof-anchored Applied, but reporting failed: {error}"
                    ),
                })
            })
        }
        ReconciledSubmissionStatus::Rejected | ReconciledSubmissionStatus::Expired => {
            bail!(
                "governed activation reached terminal status {} instead of Applied",
                outcome.terminal_kind
            )
        }
        ReconciledSubmissionStatus::Unresolved => Err(eyre!(SubmissionUncertain {
            transaction_hash: transaction.hash().to_string(),
            stage: "configured terminal wait",
            journal: journal_path.to_path_buf(),
            detail: format!(
                "wait stopped without a supported terminal status: {}",
                outcome.terminal_kind
            ),
        })),
    }
}

fn reconcile_after_failed_wait<C: RunContext>(
    context: &mut C,
    client: &Client,
    transaction: &SignedTransaction,
    exact_wire: &[u8],
    expectations: &KagemushaV4ActivationReceiptExpectationsV1,
    journal_path: &Path,
    wait_error: eyre::Report,
) -> Result<()> {
    let hash = transaction.hash();
    let status = client.get_transaction_status_response_auto(hash);
    match status {
        Ok(Some(status)) => {
            require_journal_bound_status_response(
                &status,
                transaction,
                journal_path,
                "failed wait status identity reconciliation",
            )?;
            match classify_reconciled_submission_status(Some(&status.status.kind)) {
                ReconciledSubmissionStatus::Applied => {
                    let evidence = collect_finalized_activation_evidence(
                        client,
                        transaction,
                        exact_wire,
                        expectations,
                        applied_carrier_height_for_submission(
                            &status,
                            transaction,
                            journal_path,
                            "failed wait carrier-height reconciliation",
                        )?,
                    )
                    .map_err(|error| {
                        eyre!(SubmissionUncertain {
                            transaction_hash: hash.to_string(),
                            stage: "failed wait proof reconciliation",
                            journal: journal_path.to_path_buf(),
                            detail: format!(
                                "terminal wait failed: {wait_error}; Applied evidence failed: {error}"
                            ),
                        })
                    })?;
                    let report = norito::json!({
                        "status": "Applied",
                        "transaction_hash": (hash.to_string()),
                        "carrier_height": (evidence.carrier_height.get()),
                        "reconciliation": "proof-anchored after failed terminal wait",
                        "submission_journal": (journal_path.display().to_string()),
                    });
                    context.print_data(&report).map_err(|error| {
                        eyre!(SubmissionUncertain {
                            transaction_hash: hash.to_string(),
                            stage: "proof-anchored Applied result reporting",
                            journal: journal_path.to_path_buf(),
                            detail: format!(
                                "activation is proof-anchored Applied, but reporting failed: {error}"
                            ),
                        })
                    })
                }
                ReconciledSubmissionStatus::Rejected | ReconciledSubmissionStatus::Expired => {
                    bail!(
                        "governed activation reached terminal status {} after the wait failed",
                        status.status.kind
                    )
                }
                ReconciledSubmissionStatus::Unresolved => Err(eyre!(SubmissionUncertain {
                    transaction_hash: hash.to_string(),
                    stage: "failed wait status reconciliation",
                    journal: journal_path.to_path_buf(),
                    detail: format!(
                        "terminal wait failed: {wait_error}; latest status is {}",
                        status.status.kind
                    ),
                })),
            }
        }
        Ok(None) => Err(eyre!(SubmissionUncertain {
            transaction_hash: hash.to_string(),
            stage: "failed wait status reconciliation",
            journal: journal_path.to_path_buf(),
            detail: format!("terminal wait failed: {wait_error}; no status is currently visible"),
        })),
        Err(status_error) => Err(eyre!(SubmissionUncertain {
            transaction_hash: hash.to_string(),
            stage: "failed wait transport reconciliation",
            journal: journal_path.to_path_buf(),
            detail: format!(
                "terminal wait failed: {wait_error}; final status query failed: {status_error}"
            ),
        })),
    }
}

struct FinalizedActivationEvidence {
    committed: CommittedTransaction,
    block_bytes: Vec<u8>,
    proofs: Vec<BridgeFinalityProof>,
    carrier_height: NonZeroU64,
}

struct FinalizedCanaryEvidence {
    committed: CommittedTransaction,
    block_bytes: Vec<u8>,
    proofs: Vec<BridgeFinalityProof>,
    carrier_height: NonZeroU64,
    transaction_details_response_norito: KagemushaExactBytesDigestV1,
    transaction_details_trigger_completion_count: u32,
}

struct AuthenticatedCanaryHead {
    verifier: BridgeFinalityVerifier,
    activation_height: u64,
    latest_height: u64,
}

impl AuthenticatedCanaryHead {
    fn new(
        expectations: &KagemushaV4ActivationReceiptExpectationsV1,
        receipt: &KagemushaV4ActivationFinalityReceiptV1,
    ) -> Result<Self> {
        let anchor = receipt
            .body
            .finality_proof_chain
            .last()
            .ok_or_else(|| eyre!("activation receipt finality chain is empty"))?;
        require_qualified_finality_context(anchor, expectations)?;
        let mut verifier = BridgeFinalityVerifier::with_context(
            expectations.binding().network_id.clone(),
            anchor.finality_artifact.context_id(),
        );
        verifier.verify(anchor).map_err(|error| {
            eyre!("activation receipt head anchor failed verification: {error}")
        })?;
        Ok(Self {
            verifier,
            activation_height: anchor.finality_artifact.height,
            latest_height: anchor.finality_artifact.height,
        })
    }

    fn refresh(
        &mut self,
        client: &Client,
        expectations: &KagemushaV4ActivationReceiptExpectationsV1,
    ) -> Result<u64> {
        let observed_height = client
            .get_status()
            .wrap_err("failed to observe Taira head before canary submission")?
            .blocks;
        if observed_height < self.latest_height {
            bail!("Taira status regressed behind the authenticated canary head");
        }
        let total_extension = observed_height
            .checked_sub(self.activation_height)
            .ok_or_else(|| eyre!("Taira status precedes the activation receipt"))?;
        if total_extension
            > u64::try_from(KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1)
                .expect("proof bound fits u64")
        {
            bail!("live Taira head is outside the bounded canary finality corridor");
        }
        let first_unverified = self
            .latest_height
            .checked_add(1)
            .ok_or_else(|| eyre!("authenticated Taira head height overflow"))?;
        for height in first_unverified..=observed_height {
            let height = NonZeroU64::new(height).expect("successor height is nonzero");
            let proof = client.get_next_bridge_finality_proof(height, &mut self.verifier)?;
            require_qualified_finality_context(&proof, expectations)?;
            self.latest_height = height.get();
        }
        Ok(self.latest_height)
    }
}

fn require_canary_expiry_margin(head: u64, expires_at_height: NonZeroU64) -> Result<()> {
    if head
        .checked_add(1)
        .is_none_or(|next_height| next_height >= expires_at_height.get())
    {
        bail!(
            "canary height expiry leaves no authenticated one-block inclusion margin after the live finalized head"
        );
    }
    Ok(())
}

fn require_canary_authorization_wall_margin(
    authorization: &KagemushaV4VerifiedTairaCanaryAuthorizationV1,
    now_unix_ms: u64,
) -> Result<()> {
    let transaction = authorization.canary_transaction();
    let transaction_created_at = u64::try_from(transaction.creation_time().as_millis())
        .wrap_err("canary creation time does not fit u64 milliseconds")?;
    let transaction_ttl = transaction
        .time_to_live()
        .ok_or_else(|| eyre!("canary transaction has no TTL"))?;
    let transaction_ttl = u64::try_from(transaction_ttl.as_millis())
        .wrap_err("canary transaction TTL does not fit u64 milliseconds")?;
    let transaction_expiry = transaction_created_at
        .checked_add(transaction_ttl)
        .ok_or_else(|| eyre!("canary transaction wall-clock expiry overflow"))?;
    let deadline = transaction_expiry.min(authorization.expires_at_unix_ms());
    require_canary_wall_deadline_margin(now_unix_ms, deadline)
}

fn require_canary_wall_deadline_margin(now_unix_ms: u64, deadline_unix_ms: u64) -> Result<()> {
    if now_unix_ms
        .checked_add(CANARY_CONSTRUCTION_HEADROOM_MS)
        .is_none_or(|minimum_deadline| minimum_deadline >= deadline_unix_ms)
    {
        bail!(
            "canary authorization leaves less than the fixed {} millisecond submission reserve",
            CANARY_CONSTRUCTION_HEADROOM_MS
        );
    }
    Ok(())
}

fn require_qualified_finality_context(
    proof: &BridgeFinalityProof,
    expectations: &KagemushaV4ActivationReceiptExpectationsV1,
) -> Result<()> {
    let context = &proof.finality_artifact.height_context;
    let runtime = &expectations.validator_bodies()[0].runtime_effective_config;
    if context.network_id != expectations.binding().network_id
        || context.mode != ConsensusMode::Permissioned
        || context.nexus_amx_context_hash
            != Hash::prehashed(runtime.genesis_context.nexus_amx_context_hash)
        || context.execution_policy_hash != expectations.binding().execution_policy_hash
        || context.da_layout != runtime.genesis_context.da_layout
        || context.snapshot_bootstrap.is_some()
        || context.roster.len() != KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT
        || proof.finality_artifact.validator_set_pops.len() != runtime.validators.len()
        || context
            .roster
            .iter()
            .zip(expectations.validator_bodies())
            .any(|(member, body)| member.power != 1 || member.validator != body.validator_id)
        || proof
            .finality_artifact
            .validator_set_pops
            .iter()
            .zip(&runtime.validators)
            .any(|(actual, expected)| actual != &expected.bls_pop)
    {
        bail!("finality proof leaves the authenticated validator qualification corridor");
    }
    Ok(())
}

fn collect_finalized_activation_evidence(
    client: &Client,
    transaction: &SignedTransaction,
    exact_wire: &[u8],
    expectations: &KagemushaV4ActivationReceiptExpectationsV1,
    carrier_height: NonZeroU64,
) -> Result<FinalizedActivationEvidence> {
    let details = client
        .get_successful_transaction_details(transaction.hash_as_entrypoint())
        .map_err(|error| eyre!("failed to fetch exact committed activation: {error}"))?;
    let committed = details.transaction;
    if committed.merge_inclusion.is_some() {
        bail!("activation transaction must be an ordinary block entrypoint");
    }
    require_committed_entrypoint_wire(&committed.entrypoint, exact_wire)?;

    let anchor = expectations.trusted_finality_anchor();
    let anchor_height = anchor.finality_artifact.height;
    let proof_count = carrier_height
        .get()
        .checked_sub(anchor_height)
        .ok_or_else(|| eyre!("activation carrier does not succeed the trusted anchor"))?;
    if proof_count == 0
        || proof_count
            > u64::try_from(KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1)
                .expect("proof bound fits u64")
    {
        bail!("activation finality successor count is out of bounds");
    }
    let first_successor = anchor_height
        .checked_add(1)
        .ok_or_else(|| eyre!("trusted finality anchor height overflow"))?;
    let mut verifier = BridgeFinalityVerifier::with_context(
        expectations.binding().network_id.clone(),
        anchor.finality_artifact.context_id(),
    );
    require_qualified_finality_context(anchor, expectations)?;
    verifier
        .verify(anchor)
        .map_err(|error| eyre!("trusted finality anchor failed verification: {error}"))?;
    let mut proofs = Vec::with_capacity(usize::try_from(proof_count)?);
    let mut proof_bytes = 0usize;
    for height in first_successor..=carrier_height.get() {
        let height = NonZeroU64::new(height).expect("successor height is nonzero");
        let proof = if height == carrier_height {
            client.get_bridge_finality_proof(
                height,
                committed.block_hash().clone(),
                &mut verifier,
            )?
        } else {
            client.get_next_bridge_finality_proof(height, &mut verifier)?
        };
        require_qualified_finality_context(&proof, expectations)?;
        proof_bytes = proof_bytes
            .checked_add(norito::encode_canonical(&proof)?.len())
            .ok_or_else(|| eyre!("activation proof byte count overflow"))?;
        if proof_bytes > KAGEMUSHA_V4_ACTIVATION_RECEIPT_MAX_BYTES {
            bail!("activation finality proofs exceed the complete receipt byte budget");
        }
        proofs.push(proof);
    }
    let block_bytes = client.get_canonical_executed_block_wire(carrier_height, &committed)?;
    if proof_bytes
        .checked_add(block_bytes.len())
        .is_none_or(|bytes| bytes > KAGEMUSHA_V4_ACTIVATION_RECEIPT_MAX_BYTES)
    {
        bail!("activation proof and block evidence exceed the complete receipt byte budget");
    }
    let block = norito::core::with_decode_limits_scope(
        norito::canonical_decode_limits(block_bytes.len()),
        || decode_framed_signed_block(&block_bytes),
    )
    .map_err(|error| eyre!("failed to decode canonical finalized SignedBlock wire: {error}"))?;
    if block
        .encode_wire()
        .map_err(|error| eyre!("failed to re-encode finalized SignedBlock wire: {error}"))?
        != block_bytes
    {
        bail!("finalized block wire is not byte-identical canonical SignedBlockWire");
    }
    if block
        .entrypoint_hashes()
        .filter(|entry_hash| entry_hash == committed.entrypoint_hash())
        .count()
        != 1
    {
        bail!("finalized block must contain exactly one activation entrypoint hash");
    }
    let finality_artifact = &proofs
        .last()
        .ok_or_else(|| eyre!("activation finality proof chain is empty"))?
        .finality_artifact;
    let proof_anchor = TrustedBlockProofAnchor::from_untrusted_finality_artifact(
        &block,
        finality_artifact,
        committed.entrypoint_hash(),
    )
    .map_err(|error| {
        eyre!("finality proof does not authenticate the exact carrier block: {error}")
    })?;
    let entry_index = usize::try_from(proof_anchor.entry_index())?;
    let block_entrypoint = block
        .entrypoints_cloned()
        .nth(entry_index)
        .ok_or_else(|| eyre!("proof-anchored activation entrypoint is absent from its block"))?;
    require_committed_entrypoint_wire(&block_entrypoint, exact_wire)
        .wrap_err("proof-anchored block entrypoint is not the authorized transaction wire")?;

    Ok(FinalizedActivationEvidence {
        committed,
        block_bytes,
        proofs,
        carrier_height,
    })
}

fn collect_finalized_canary_evidence(
    client: &Client,
    transaction: &SignedTransaction,
    exact_wire: &[u8],
    expectations: &KagemushaV4ActivationReceiptExpectationsV1,
    receipt: &KagemushaV4ActivationFinalityReceiptV1,
    carrier_height: NonZeroU64,
) -> Result<FinalizedCanaryEvidence> {
    let details = client
        .get_successful_transaction_details(transaction.hash_as_entrypoint())
        .map_err(|error| eyre!("failed to fetch exact committed canary: {error}"))?;
    let transaction_details_response_norito = exact_canonical_digest(&details)?;
    let transaction_details_trigger_completion_count =
        u32::try_from(details.trigger_completions.len())
            .wrap_err("canary transaction-details trigger completion count does not fit u32")?;
    let committed = details.transaction;
    if committed.merge_inclusion.is_some() {
        bail!("canary transaction must be an ordinary block entrypoint");
    }
    require_committed_entrypoint_wire(&committed.entrypoint, exact_wire)
        .wrap_err("committed canary differs from its controller authorization")?;

    let activation_anchor = receipt
        .body
        .finality_proof_chain
        .last()
        .ok_or_else(|| eyre!("activation receipt finality chain is empty"))?;
    let activation_height = activation_anchor.finality_artifact.height;
    let proof_count = carrier_height
        .get()
        .checked_sub(activation_height)
        .ok_or_else(|| eyre!("canary carrier precedes the authenticated activation"))?;
    if proof_count == 0
        || proof_count
            > u64::try_from(KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1)
                .expect("proof bound fits u64")
    {
        bail!("canary finality extension count is out of bounds");
    }
    let first_successor = activation_height
        .checked_add(1)
        .ok_or_else(|| eyre!("activation finality height overflow"))?;
    let mut verifier = BridgeFinalityVerifier::with_context(
        expectations.binding().network_id.clone(),
        activation_anchor.finality_artifact.context_id(),
    );
    require_qualified_finality_context(activation_anchor, expectations)?;
    verifier
        .verify(activation_anchor)
        .map_err(|error| eyre!("activation receipt anchor failed verification: {error}"))?;
    let mut proofs = Vec::with_capacity(usize::try_from(proof_count)?);
    let mut proof_bytes = 0usize;
    for height in first_successor..=carrier_height.get() {
        let height = NonZeroU64::new(height).expect("successor height is nonzero");
        let proof = if height == carrier_height {
            client.get_bridge_finality_proof(
                height,
                committed.block_hash().clone(),
                &mut verifier,
            )?
        } else {
            client.get_next_bridge_finality_proof(height, &mut verifier)?
        };
        require_qualified_finality_context(&proof, expectations)?;
        proof_bytes = proof_bytes
            .checked_add(norito::encode_canonical(&proof)?.len())
            .ok_or_else(|| eyre!("canary proof byte count overflow"))?;
        if proof_bytes > KAGEMUSHA_V4_ACTIVATION_RECEIPT_MAX_BYTES {
            bail!("canary finality extension exceeds the evidence byte budget");
        }
        proofs.push(proof);
    }
    let block_bytes = client.get_canonical_executed_block_wire(carrier_height, &committed)?;
    if proof_bytes
        .checked_add(block_bytes.len())
        .is_none_or(|bytes| bytes > KAGEMUSHA_V4_ACTIVATION_RECEIPT_MAX_BYTES)
    {
        bail!("canary proof and block evidence exceed the evidence byte budget");
    }
    let block = norito::core::with_decode_limits_scope(
        norito::canonical_decode_limits(block_bytes.len()),
        || decode_framed_signed_block(&block_bytes),
    )
    .map_err(|error| eyre!("failed to decode canonical canary SignedBlock wire: {error}"))?;
    if block
        .encode_wire()
        .map_err(|error| eyre!("failed to re-encode canary SignedBlock wire: {error}"))?
        != block_bytes
    {
        bail!("canary block wire is not byte-identical canonical SignedBlockWire");
    }
    if block
        .entrypoint_hashes()
        .filter(|entry_hash| entry_hash == committed.entrypoint_hash())
        .count()
        != 1
    {
        bail!("finalized block must contain exactly one canary entrypoint hash");
    }
    let finality_artifact = &proofs
        .last()
        .ok_or_else(|| eyre!("canary finality extension is empty"))?
        .finality_artifact;
    let proof_anchor = TrustedBlockProofAnchor::from_untrusted_finality_artifact(
        &block,
        finality_artifact,
        committed.entrypoint_hash(),
    )
    .map_err(|error| eyre!("canary proof does not authenticate its carrier block: {error}"))?;
    let entry_index = usize::try_from(proof_anchor.entry_index())?;
    let block_entrypoint = block
        .entrypoints_cloned()
        .nth(entry_index)
        .ok_or_else(|| eyre!("proof-anchored canary entrypoint is absent from its block"))?;
    require_committed_entrypoint_wire(&block_entrypoint, exact_wire)
        .wrap_err("proof-anchored canary is not the controller-authorized wire")?;

    Ok(FinalizedCanaryEvidence {
        committed,
        block_bytes,
        proofs,
        carrier_height,
        transaction_details_response_norito,
        transaction_details_trigger_completion_count,
    })
}

fn require_canary_block_within_authorization(
    block_bytes: &[u8],
    authorization: &KagemushaV4VerifiedTairaCanaryAuthorizationV1,
) -> Result<()> {
    let block = norito::core::with_decode_limits_scope(
        norito::canonical_decode_limits(block_bytes.len()),
        || decode_framed_signed_block(block_bytes),
    )
    .map_err(|error| eyre!("failed to decode canary block for expiry checks: {error}"))?;
    let block_time_unix_ms = u64::try_from(block.header().creation_time().as_millis())
        .wrap_err("canary block time does not fit u64 milliseconds")?;
    let transaction = authorization.canary_transaction();
    let transaction_time_unix_ms = u64::try_from(transaction.creation_time().as_millis())
        .wrap_err("canary transaction time does not fit u64 milliseconds")?;
    let transaction_ttl_ms = u64::try_from(
        transaction
            .time_to_live()
            .ok_or_else(|| eyre!("canary transaction has no TTL"))?
            .as_millis(),
    )
    .wrap_err("canary transaction TTL does not fit u64 milliseconds")?;
    let transaction_expiry = transaction_time_unix_ms
        .checked_add(transaction_ttl_ms)
        .ok_or_else(|| eyre!("canary transaction wall-clock expiry overflow"))?;
    if block_time_unix_ms < transaction_time_unix_ms
        || block_time_unix_ms >= transaction_expiry
        || block_time_unix_ms < authorization.authorized_at_unix_ms()
        || block_time_unix_ms >= authorization.expires_at_unix_ms()
    {
        bail!("canary carrier block lies outside the controller-authorized wall-clock interval");
    }
    Ok(())
}

fn exact_canonical_digest<T: norito::NoritoSerialize>(
    value: &T,
) -> Result<KagemushaExactBytesDigestV1> {
    let bytes =
        norito::encode_canonical(value).wrap_err("failed to encode exact query evidence")?;
    KagemushaExactBytesDigestV1::from_bytes(&bytes).map_err(Into::into)
}

fn current_unix_ms() -> Result<u64> {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .wrap_err("protected-host clock is before the Unix epoch")?
            .as_millis(),
    )
    .wrap_err("protected-host Unix millisecond clock does not fit u64")
}

fn rollout_state_path(promotion_id: [u8; 32], file_name: &str) -> Result<PathBuf> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        Ok(Path::new(ROLLOUT_STATE_ROOT)
            .join(hex::encode(promotion_id))
            .join(file_name))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (promotion_id, file_name);
        bail!("Kagemusha rollout state has no qualified platform root")
    }
}

fn require_rollout_state_path(path: &Path, promotion_id: [u8; 32], file_name: &str) -> Result<()> {
    if path != rollout_state_path(promotion_id, file_name)? {
        bail!("rollout artifact path must be the exact promotion-keyed `{file_name}` state path");
    }
    Ok(())
}

fn parse_public_key(value: &str, label: &str) -> Result<PublicKey> {
    let key: PublicKey = value
        .parse()
        .map_err(|_| eyre!("{label} is not a canonical public key"))?;
    if key.to_string() != value {
        bail!("{label} is not in canonical text form");
    }
    Ok(key)
}

fn require_committed_entrypoint_wire(
    entrypoint: &TransactionEntrypoint,
    exact_wire: &[u8],
) -> Result<()> {
    let TransactionEntrypoint::External(committed) = entrypoint else {
        bail!("committed entrypoint is not an external transaction");
    };
    let committed_wire = committed
        .encode_wire_v1()
        .map_err(|error| eyre!("failed to encode committed transaction: {error}"))?;
    if committed_wire != exact_wire {
        bail!("committed transaction wire differs from its exact authenticated wire");
    }
    Ok(())
}

fn require_root() -> Result<()> {
    #[cfg(unix)]
    if rustix::process::geteuid().as_raw() == 0 {
        return Ok(());
    }
    bail!("Kagemusha rollout artifact custody requires effective uid 0")
}

#[cfg(unix)]
fn inspect_root_private_key(path: &Path, label: &str) -> Result<fs::Metadata> {
    use std::os::unix::fs::MetadataExt as _;
    require_root()?;
    if !path.is_absolute() || fs::canonicalize(path)? != path {
        bail!("{label} must be one canonical absolute path");
    }
    let parent = path
        .parent()
        .ok_or_else(|| eyre!("{label} has no parent"))?;
    let ancestry = validate_owned_ancestry(parent, 0, label)?;
    let named = fs::symlink_metadata(path)?;
    let valid = |metadata: &fs::Metadata| {
        metadata.file_type().is_file()
            && metadata.uid() == 0
            && metadata.nlink() == 1
            && metadata.mode() & 0o7777 == 0o600
            && metadata.len() > 0
            && metadata.len() <= 4 * 1024
    };
    if !valid(&named) {
        bail!("{label} must be one bounded root-owned mode-0600 regular file");
    }
    let descriptor = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )?;
    let opened = File::from(descriptor);
    let metadata = opened.metadata()?;
    if !valid(&metadata)
        || metadata_identity(&named) != metadata_identity(&metadata)
        || ancestry != validate_owned_ancestry(parent, 0, label)?
    {
        bail!("{label} changed during descriptor-bound custody inspection");
    }
    require_no_xattrs(&opened, label)?;
    require_no_macos_acl(&opened, label)?;
    Ok(metadata)
}

fn load_root_custodied_key(path: &Path, label: &str) -> Result<KeyPair> {
    #[cfg(unix)]
    {
        let before = inspect_root_private_key(path, label)?;
        let key = load_operator_key_pair(path)
            .wrap_err_with(|| format!("failed to load runtime-only {label}"))?;
        let after = inspect_root_private_key(path, label)?;
        if metadata_identity(&before) != metadata_identity(&after) {
            bail!("{label} changed while it was loaded");
        }
        Ok(key)
    }
    #[cfg(not(unix))]
    {
        let _ = (path, label);
        bail!("root-private rollout keys require Unix descriptor APIs")
    }
}

fn read_root_owned(path: &Path, maximum: usize, label: &str) -> Result<Vec<u8>> {
    #[cfg(unix)]
    {
        read_owned(path, maximum, label, 0)
    }
    #[cfg(not(unix))]
    {
        let _ = (path, maximum, label);
        bail!("root-owned stable reads require Unix descriptor APIs")
    }
}

fn read_root_private_artifact(path: &Path, maximum: usize, label: &str) -> Result<Vec<u8>> {
    #[cfg(unix)]
    {
        read_owned_with_policy(path, maximum, label, 0, 0o400, true)
    }
    #[cfg(not(unix))]
    {
        let _ = (path, maximum, label);
        bail!("root-private stable reads require Unix descriptor APIs")
    }
}

#[cfg(unix)]
fn read_owned(path: &Path, maximum: usize, label: &str, uid: u32) -> Result<Vec<u8>> {
    read_owned_with_policy(path, maximum, label, uid, 0o444, false)
}

#[cfg(unix)]
fn read_owned_with_policy(
    path: &Path,
    maximum: usize,
    label: &str,
    uid: u32,
    expected_mode: u32,
    require_private_parent: bool,
) -> Result<Vec<u8>> {
    use std::os::unix::fs::MetadataExt as _;
    if !path.is_absolute() {
        bail!("{label} must be read through an absolute path");
    }
    let parent = path
        .parent()
        .ok_or_else(|| eyre!("{label} path has no parent"))?;
    let ancestry = validate_owned_ancestry(parent, uid, label)?;
    if require_private_parent
        && ancestry
            .last()
            .is_none_or(|(_, identity)| identity.2 & 0o7777 != 0o700 || identity.3 != uid)
    {
        bail!("{label} immediate parent must be an owner-held mode-0700 directory");
    }
    if fs::canonicalize(path)? != path {
        bail!("{label} path must be canonical and symlink-free");
    }
    let before = fs::symlink_metadata(path).wrap_err_with(|| format!("inspect {label}"))?;
    let valid = |metadata: &fs::Metadata| {
        metadata.file_type().is_file()
            && metadata.nlink() == 1
            && metadata.uid() == uid
            && metadata.mode() & 0o7777 == expected_mode
            && metadata.len() > 0
            && metadata.len() <= u64::try_from(maximum).unwrap_or(u64::MAX)
    };
    if !valid(&before) {
        bail!(
            "{label} must be a bounded, singly linked, owner-held mode-{expected_mode:04o} regular file"
        );
    }
    let descriptor = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
    )?;
    let mut file = File::from(descriptor);
    let opened = file.metadata()?;
    if !valid(&opened) || metadata_identity(&before) != metadata_identity(&opened) {
        bail!("{label} changed during secure open");
    }
    require_no_xattrs(&file, label)?;
    require_no_macos_acl(&file, label)?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(usize::try_from(opened.len())?)?;
    std::io::Read::by_ref(&mut file)
        .take(u64::try_from(maximum)?.saturating_add(1))
        .read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    let named_after = fs::symlink_metadata(path)?;
    if bytes.len() != usize::try_from(opened.len())?
        || !valid(&after)
        || metadata_identity(&opened) != metadata_identity(&after)
        || metadata_identity(&after) != metadata_identity(&named_after)
        || ancestry != validate_owned_ancestry(parent, uid, label)?
        || fs::canonicalize(path)? != path
    {
        bail!("{label} changed during bounded read");
    }
    require_no_xattrs(&file, label)?;
    require_no_macos_acl(&file, label)?;
    Ok(bytes)
}

#[cfg(unix)]
fn metadata_identity(
    metadata: &fs::Metadata,
) -> (u64, u64, u32, u32, u32, u64, u64, i64, i64, i64, i64) {
    use std::os::unix::fs::MetadataExt as _;
    (
        metadata.dev(),
        metadata.ino(),
        metadata.mode(),
        metadata.uid(),
        metadata.gid(),
        metadata.nlink(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    )
}

#[cfg(unix)]
fn custody_identity(metadata: &fs::Metadata) -> (u64, u64, u32, u32, u32, u64) {
    use std::os::unix::fs::MetadataExt as _;
    (
        metadata.dev(),
        metadata.ino(),
        metadata.mode(),
        metadata.uid(),
        metadata.gid(),
        metadata.nlink(),
    )
}

#[cfg(unix)]
fn validate_owned_ancestry(
    path: &Path,
    owner: u32,
    label: &str,
) -> Result<Vec<(PathBuf, (u64, u64, u32, u32, u32, u64))>> {
    use std::{os::unix::fs::MetadataExt as _, path::Component};
    if !path.is_absolute() || fs::canonicalize(path)? != path {
        bail!("{label} ancestry must be absolute, canonical, and symlink-free");
    }
    let mut current = PathBuf::from("/");
    let mut paths = vec![current.clone()];
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => {
                current.push(name);
                paths.push(current.clone());
            }
            _ => bail!("{label} ancestry contains a noncanonical component"),
        }
    }
    paths
        .into_iter()
        .map(|entry| {
            let metadata = fs::symlink_metadata(&entry)?;
            if !metadata.file_type().is_dir()
                || (metadata.uid() != 0 && metadata.uid() != owner)
                || metadata.mode() & 0o022 != 0
                || metadata.nlink() == 0
            {
                bail!(
                    "{label} ancestry has unsafe custody at `{}`",
                    entry.display()
                );
            }
            let descriptor = rustix::fs::open(
                &entry,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::DIRECTORY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )?;
            let opened = File::from(descriptor);
            if custody_identity(&opened.metadata()?) != custody_identity(&metadata) {
                bail!("{label} ancestry changed during secure open");
            }
            require_no_xattrs(&opened, label)?;
            require_no_macos_acl(&opened, label)?;
            Ok((entry, custody_identity(&metadata)))
        })
        .collect()
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[allow(
    unsafe_code,
    reason = "descriptor-bound xattr inspection requires the platform libc"
)]
fn require_no_xattrs(file: &File, label: &str) -> Result<()> {
    use std::os::fd::AsRawFd as _;
    #[cfg(target_os = "linux")]
    let count = unsafe { flistxattr(file.as_raw_fd(), std::ptr::null_mut(), 0) };
    #[cfg(target_os = "macos")]
    let count = unsafe {
        const XATTR_SHOWCOMPRESSION: std::os::raw::c_int = 0x20;
        flistxattr(
            file.as_raw_fd(),
            std::ptr::null_mut(),
            0,
            XATTR_SHOWCOMPRESSION,
        )
    };
    if count < 0 {
        return Err(std::io::Error::last_os_error())
            .wrap_err_with(|| format!("inspect {label} xattrs"));
    }
    if count != 0 {
        bail!("{label} must be xattr-free");
    }
    Ok(())
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
fn require_no_xattrs(_file: &File, _label: &str) -> Result<()> {
    bail!("descriptor-bound xattr inspection is unavailable on this Unix platform")
}

#[cfg(target_os = "macos")]
#[allow(
    unsafe_code,
    reason = "descriptor-bound macOS extended-ACL inspection requires libc"
)]
fn require_no_macos_acl(file: &File, label: &str) -> Result<()> {
    use std::os::fd::AsRawFd as _;
    const ACL_TYPE_EXTENDED: std::os::raw::c_int = 0x0000_0100;
    const ACL_FIRST_ENTRY: std::os::raw::c_int = 0;
    let acl = unsafe { acl_get_fd_np(file.as_raw_fd(), ACL_TYPE_EXTENDED) };
    if acl.is_null() {
        if std::io::Error::last_os_error().raw_os_error() == Some(2) {
            return Ok(());
        }
        bail!("failed to inspect {label} extended ACL");
    }
    let mut entry = std::ptr::null_mut();
    let status = unsafe { acl_get_entry(acl, ACL_FIRST_ENTRY, &raw mut entry) };
    let freed = unsafe { acl_free(acl) };
    if status < 0 || freed != 0 {
        bail!("failed to inspect {label} extended ACL");
    }
    if status == 0 {
        bail!("{label} must have no extended ACL");
    }
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn require_no_macos_acl(_file: &File, _label: &str) -> Result<()> {
    Ok(())
}

#[derive(Debug, thiserror::Error)]
enum PublicationError {
    #[error("publication failed before commit: {0}")]
    PreCommit(String),
    #[error("staging cleanup uncertain at `{path}`; reconcile before retry: {detail}")]
    CleanupUncertain { path: PathBuf, detail: String },
    #[error("publication commit-uncertain at `{path}`; do not retry automatically: {detail}")]
    CommitUncertain { path: PathBuf, detail: String },
}

fn publish_root_owned(
    path: &Path,
    bytes: &[u8],
    verify: impl Fn(&[u8]) -> std::result::Result<(), String>,
) -> Result<()> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        publish_owned_with(path, bytes, 0, verify, || Ok(())).map_err(|error| eyre!(error))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (path, bytes, verify);
        bail!("root-owned no-replace publication requires Unix descriptor APIs")
    }
}

fn preflight_root_owned_output(path: &Path) -> Result<()> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        open_owned_publication_destination(path, 0)
            .map(|_| ())
            .map_err(|error| eyre!(error))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = path;
        bail!("root-owned no-replace publication requires Unix descriptor APIs")
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "redox"))]
struct OwnedPublicationDestination {
    parent_path: PathBuf,
    parent: File,
    name: OsString,
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "redox"))]
fn open_owned_publication_destination(
    path: &Path,
    uid: u32,
) -> std::result::Result<OwnedPublicationDestination, PublicationError> {
    use std::os::unix::{ffi::OsStrExt as _, fs::MetadataExt as _};

    let pre = |detail: String| PublicationError::PreCommit(detail);
    if !path.is_absolute() || rustix::process::geteuid().as_raw() != uid {
        return Err(pre(
            "destination must be absolute and owned by the effective uid".to_owned(),
        ));
    }
    let parent_path = path
        .parent()
        .ok_or_else(|| pre("destination has no parent".to_owned()))?;
    let ancestry = validate_owned_ancestry(parent_path, uid, "publication destination")
        .map_err(|error| pre(error.to_string()))?;
    let name = path
        .file_name()
        .filter(|name| !name.as_bytes().is_empty())
        .ok_or_else(|| pre("destination has no file name".to_owned()))?;
    if parent_path.join(name).as_os_str().as_bytes() != path.as_os_str().as_bytes() {
        return Err(pre(
            "destination must contain only one exact normal final path component".to_owned(),
        ));
    }
    let parent_descriptor = rustix::fs::open(
        parent_path,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map_err(|error| pre(error.to_string()))?;
    let parent = File::from(parent_descriptor);
    let parent_meta = parent.metadata().map_err(|error| pre(error.to_string()))?;
    if !parent_meta.is_dir()
        || parent_meta.uid() != uid
        || parent_meta.mode() & 0o7777 != 0o700
        || ancestry.last().map(|(_, identity)| *identity) != Some(custody_identity(&parent_meta))
    {
        return Err(pre(
            "destination parent must be an owner-held mode-0700 directory".to_owned(),
        ));
    }
    match rustix::fs::statat(&parent, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW) {
        Err(error) if error == rustix::io::Errno::NOENT => {}
        Ok(_) => {
            return Err(pre(
                "destination already exists and will not be replaced".to_owned()
            ));
        }
        Err(error) => return Err(pre(error.to_string())),
    }
    Ok(OwnedPublicationDestination {
        parent_path: parent_path.to_owned(),
        parent,
        name: name.to_owned(),
    })
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "redox"))]
fn publish_owned_with(
    path: &Path,
    bytes: &[u8],
    uid: u32,
    verify: impl Fn(&[u8]) -> std::result::Result<(), String>,
    after_commit: impl FnOnce() -> std::io::Result<()>,
) -> std::result::Result<(), PublicationError> {
    use std::os::unix::fs::MetadataExt as _;

    let pre = |detail: String| PublicationError::PreCommit(detail);
    if bytes.is_empty() || bytes.len() > KAGEMUSHA_V4_ACTIVATION_RECEIPT_MAX_BYTES {
        return Err(pre("artifact is empty or oversized".to_owned()));
    }
    let OwnedPublicationDestination {
        parent_path,
        parent,
        name,
    } = open_owned_publication_destination(path, uid)?;
    let ancestry = validate_owned_ancestry(&parent_path, uid, "publication destination")
        .map_err(|error| pre(error.to_string()))?;
    let stage = OsString::from(format!(
        ".{}.{}.staging",
        name.to_string_lossy(),
        rand::random::<u64>()
    ));
    let descriptor = rustix::fs::openat(
        &parent,
        &stage,
        rustix::fs::OFlags::RDWR
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::EXCL
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::from_raw_mode(0o600),
    )
    .map_err(|error| pre(error.to_string()))?;
    let mut staging = File::from(descriptor);
    let initial = staging
        .metadata()
        .map_err(|error| PublicationError::CleanupUncertain {
            path: parent_path.join(&stage),
            detail: format!("staging identity could not be established: {error}"),
        })?;
    let identity = (initial.dev(), initial.ino());
    let staging_path = parent_path.join(&stage);
    let cleanup = || -> std::result::Result<(), String> {
        let current = rustix::fs::statat(&parent, &stage, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|error| format!("inspect staging inode before cleanup: {error}"))?;
        if (u64::try_from(current.st_dev).ok(), current.st_ino) != (Some(identity.0), identity.1) {
            return Err("staging name no longer identifies the owned inode".to_owned());
        }
        rustix::fs::unlinkat(&parent, &stage, rustix::fs::AtFlags::empty())
            .map_err(|error| format!("unlink staging inode: {error}"))?;
        parent
            .sync_all()
            .map_err(|error| format!("sync staging cleanup: {error}"))
    };
    let staged = (|| -> std::result::Result<(), String> {
        staging
            .write_all(bytes)
            .map_err(|error| error.to_string())?;
        staging.sync_all().map_err(|error| error.to_string())?;
        rustix::fs::fchmod(&staging, rustix::fs::Mode::from_raw_mode(0o400))
            .map_err(|error| error.to_string())?;
        staging.sync_all().map_err(|error| error.to_string())?;
        require_no_xattrs(&staging, "staged rollout artifact")
            .map_err(|error| error.to_string())?;
        require_no_macos_acl(&staging, "staged rollout artifact")
            .map_err(|error| error.to_string())?;
        let metadata = staging.metadata().map_err(|error| error.to_string())?;
        if !metadata.is_file()
            || metadata.dev() != identity.0
            || metadata.ino() != identity.1
            || metadata.uid() != uid
            || metadata.nlink() != 1
            || metadata.mode() & 0o7777 != 0o400
            || metadata.len() != u64::try_from(bytes.len()).map_err(|error| error.to_string())?
        {
            return Err("staged artifact custody changed".to_owned());
        }
        parent.sync_all().map_err(|error| error.to_string())
    })();
    if let Err(error) = staged {
        drop(staging);
        if let Err(cleanup) = cleanup() {
            return Err(PublicationError::CleanupUncertain {
                path: staging_path,
                detail: format!("{error}; {cleanup}"),
            });
        }
        return Err(pre(error));
    }
    let precommit = (|| -> std::result::Result<(), String> {
        staging
            .seek(SeekFrom::Start(0))
            .map_err(|error| error.to_string())?;
        let mut readback = Vec::new();
        std::io::Read::by_ref(&mut staging)
            .take(
                u64::try_from(bytes.len())
                    .unwrap_or(u64::MAX)
                    .saturating_add(1),
            )
            .read_to_end(&mut readback)
            .map_err(|error| error.to_string())?;
        if readback != bytes {
            return Err("staged artifact readback differs before commit".to_owned());
        }
        verify(&readback)
    })();
    if let Err(error) = precommit {
        drop(staging);
        if let Err(cleanup) = cleanup() {
            return Err(PublicationError::CleanupUncertain {
                path: staging_path,
                detail: format!("{error}; {cleanup}"),
            });
        }
        return Err(pre(error));
    }
    if let Err(error) = rustix::fs::renameat_with(
        &parent,
        &stage,
        &parent,
        &name,
        rustix::fs::RenameFlags::NOREPLACE,
    ) {
        drop(staging);
        if let Err(cleanup) = cleanup() {
            return Err(PublicationError::CleanupUncertain {
                path: staging_path,
                detail: format!("rename failed: {error}; {cleanup}"),
            });
        }
        return Err(pre(format!("no-replace rename failed: {error}")));
    }
    let post = (|| -> std::result::Result<(), String> {
        after_commit().map_err(|error| error.to_string())?;
        let named = rustix::fs::statat(&parent, &name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|error| error.to_string())?;
        if (u64::try_from(named.st_dev).ok(), named.st_ino) != (Some(identity.0), identity.1)
            || named.st_uid != uid
            || u32::from(named.st_mode) & 0o7777 != 0o400
            || u64::try_from(named.st_nlink).ok() != Some(1)
            || u64::try_from(named.st_size).ok() != u64::try_from(bytes.len()).ok()
        {
            return Err("published artifact identity or custody changed".to_owned());
        }
        parent.sync_all().map_err(|error| error.to_string())?;
        staging
            .seek(SeekFrom::Start(0))
            .map_err(|error| error.to_string())?;
        let mut readback = Vec::new();
        std::io::Read::by_ref(&mut staging)
            .take(
                u64::try_from(bytes.len())
                    .unwrap_or(u64::MAX)
                    .saturating_add(1),
            )
            .read_to_end(&mut readback)
            .map_err(|error| error.to_string())?;
        if readback != bytes {
            return Err("published artifact readback differs".to_owned());
        }
        verify(&readback)?;
        let opened = staging.metadata().map_err(|error| error.to_string())?;
        let path_metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
        if metadata_identity(&opened) != metadata_identity(&path_metadata)
            || opened.dev() != identity.0
            || opened.ino() != identity.1
            || opened.uid() != uid
            || opened.mode() & 0o7777 != 0o400
            || opened.nlink() != 1
            || opened.len() != u64::try_from(bytes.len()).map_err(|error| error.to_string())?
            || fs::canonicalize(path).map_err(|error| error.to_string())? != path
            || ancestry
                != validate_owned_ancestry(&parent_path, uid, "published artifact")
                    .map_err(|error| error.to_string())?
        {
            return Err("published pathname, ancestry, or opened identity changed".to_owned());
        }
        require_no_xattrs(&staging, "published rollout artifact")
            .map_err(|error| error.to_string())?;
        require_no_macos_acl(&staging, "published rollout artifact")
            .map_err(|error| error.to_string())?;
        Ok(())
    })();
    post.map_err(|detail| PublicationError::CommitUncertain {
        path: path.to_path_buf(),
        detail,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser as _;
    use iroha::data_model::{
        account::AccountId,
        transaction::{FeePaymentIntent, TransactionBuilder, signed::MultisigSignatures},
    };
    #[cfg(unix)]
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _, symlink};

    #[derive(clap::Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: Command,
    }

    #[cfg(unix)]
    fn safe_owned_tempdir() -> tempfile::TempDir {
        let uid = rustix::process::geteuid().as_raw();
        let parent = fs::canonicalize(std::env::current_dir().expect("test working directory"))
            .expect("canonical test working directory");
        validate_owned_ancestry(&parent, uid, "rollout test parent")
            .expect("tests require a safely owned working-directory ancestry");
        let directory = tempfile::Builder::new()
            .prefix(".kagemusha-rollout-test-")
            .tempdir_in(parent)
            .expect("safe rollout test directory");
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .expect("seal rollout test directory mode");
        directory
    }

    #[test]
    fn submit_requires_explicit_write_authorization() {
        let args = [
            "test",
            "submit",
            "--promotion-controller",
            "controller",
            "--promotion-reservation",
            "/reservation",
            "--expectations",
            "/expectations",
        ];
        assert!(TestCli::try_parse_from(args).is_err());
        let mut authorized = args.to_vec();
        authorized.push("--write-authorized");
        assert!(TestCli::try_parse_from(authorized).is_ok());
    }

    #[test]
    fn canary_commands_are_phase_separated_and_submission_is_explicit() {
        let trusted = [
            "--promotion-controller",
            "controller",
            "--promotion-reservation",
            "/reservation",
            "--expectations",
            "/expectations",
        ];
        let mut create = vec!["test", "create-canary-authorization"];
        create.extend(trusted);
        create.extend([
            "--activation-receipt",
            "/receipt",
            "--canary-ttl-ms",
            "120000",
            "--canary-expires-at-height",
            "42",
            "--controller-private-key-file",
            "/controller.key",
            "--output",
            "/authorization",
        ]);
        assert!(matches!(
            TestCli::try_parse_from(create)
                .expect("create authorization parses")
                .command,
            Command::CreateCanaryAuthorization(_)
        ));

        let mut reserve = vec!["test", "submit-canary-authorization"];
        reserve.extend(trusted);
        reserve.extend([
            "--activation-receipt",
            "/receipt",
            "--canary-authorization",
            "/authorization",
        ]);
        assert!(TestCli::try_parse_from(reserve.clone()).is_err());
        reserve.push("--write-authorized");
        assert!(matches!(
            TestCli::try_parse_from(reserve)
                .expect("explicit on-chain canary authorization parses")
                .command,
            Command::SubmitCanaryAuthorization(_)
        ));

        let mut submit = vec!["test", "submit-canary"];
        submit.extend(trusted);
        submit.extend([
            "--activation-receipt",
            "/receipt",
            "--canary-authorization",
            "/authorization",
        ]);
        assert!(TestCli::try_parse_from(submit.clone()).is_err());
        submit.push("--write-authorized");
        assert!(matches!(
            TestCli::try_parse_from(submit)
                .expect("explicit canary submission parses")
                .command,
            Command::SubmitCanary(_)
        ));

        let mut finalize = vec!["test", "finalize-canary-evidence"];
        finalize.extend(trusted);
        finalize.extend([
            "--activation-receipt",
            "/receipt",
            "--canary-authorization",
            "/authorization",
            "--issuer-private-key-file",
            "/issuer.key",
            "--output",
            "/evidence",
        ]);
        assert!(matches!(
            TestCli::try_parse_from(finalize)
                .expect("canary evidence finalizer parses")
                .command,
            Command::FinalizeCanaryEvidence(_)
        ));

        let mut liveness = vec!["test", "finalize-validator-liveness"];
        liveness.extend(trusted);
        liveness.extend([
            "--activation-receipt",
            "/receipt",
            "--canary-authorization",
            "/authorization",
            "--canary-evidence",
            "/evidence",
            "--validator",
            "peer-a=https://v1.example",
            "peer-b=https://v2.example",
            "peer-c=https://v3.example",
            "peer-d=https://v4.example",
            "--issuer-private-key-file",
            "/issuer.key",
            "--output",
            "/liveness",
        ]);
        assert!(matches!(
            TestCli::try_parse_from(liveness)
                .expect("four-validator liveness finalizer parses")
                .command,
            Command::FinalizeValidatorLiveness(_)
        ));
    }

    #[test]
    fn submission_journal_orders_publication_before_status_resume() {
        assert_eq!(
            decide_submission_journal(SubmissionJournalObservation::Absent, false),
            Ok(SubmissionJournalAction::Publish)
        );
        assert_eq!(
            decide_submission_journal(SubmissionJournalObservation::Matching, false),
            Ok(SubmissionJournalAction::Resume)
        );
        assert_eq!(
            decide_submission_journal(SubmissionJournalObservation::Matching, true),
            Ok(SubmissionJournalAction::Resume)
        );
    }

    #[test]
    fn submission_journal_rejects_mismatch_and_retrospective_status() {
        assert_eq!(
            decide_submission_journal(SubmissionJournalObservation::Mismatched, false),
            Err(SubmissionJournalDecisionError::Mismatch)
        );
        assert_eq!(
            decide_submission_journal(SubmissionJournalObservation::Absent, true),
            Err(SubmissionJournalDecisionError::Retrospective)
        );
    }

    #[test]
    fn canary_torii_origin_is_exact_https_dns_origin() {
        let valid = url::Url::parse("https://taira.sora.org/").unwrap();
        assert_eq!(
            canonical_torii_origin(&valid).unwrap(),
            "https://taira.sora.org"
        );
        for invalid in [
            "http://taira.sora.org/",
            "https://127.0.0.1/",
            "https://taira.sora.org/path/",
            "https://taira.sora.org/?query=1",
        ] {
            assert!(canonical_torii_origin(&url::Url::parse(invalid).unwrap()).is_err());
        }
    }

    #[test]
    fn canary_expiry_requires_one_full_inclusion_block_after_authenticated_head() {
        assert!(require_canary_expiry_margin(40, NonZeroU64::new(42).unwrap()).is_ok());
        assert!(require_canary_expiry_margin(40, NonZeroU64::new(41).unwrap()).is_err());
        assert!(require_canary_expiry_margin(u64::MAX, NonZeroU64::new(1).unwrap()).is_err());
    }

    #[test]
    fn canary_wall_expiry_requires_full_post_construction_reserve() {
        let now = 1_000_000;
        assert!(
            require_canary_wall_deadline_margin(now, now + CANARY_CONSTRUCTION_HEADROOM_MS + 1,)
                .is_ok()
        );
        assert!(
            require_canary_wall_deadline_margin(now, now + CANARY_CONSTRUCTION_HEADROOM_MS,)
                .is_err()
        );
        assert!(require_canary_wall_deadline_margin(u64::MAX, u64::MAX).is_err());
    }

    #[test]
    fn finalization_requires_the_exact_matching_submission_journal() {
        assert!(
            require_matching_submission_journal(SubmissionJournalObservation::Matching).is_ok()
        );
        assert!(require_matching_submission_journal(SubmissionJournalObservation::Absent).is_err());
        assert!(
            require_matching_submission_journal(SubmissionJournalObservation::Mismatched).is_err()
        );
    }

    #[test]
    fn terminal_reconciliation_classifies_only_provable_or_final_states() {
        assert_eq!(
            classify_reconciled_submission_status(Some("Applied")),
            ReconciledSubmissionStatus::Applied
        );
        assert_eq!(
            classify_reconciled_submission_status(Some("Rejected")),
            ReconciledSubmissionStatus::Rejected
        );
        assert_eq!(
            classify_reconciled_submission_status(Some("Expired")),
            ReconciledSubmissionStatus::Expired
        );
        for unresolved in [Some("Queued"), Some("Approved"), Some("Committed"), None] {
            assert_eq!(
                classify_reconciled_submission_status(unresolved),
                ReconciledSubmissionStatus::Unresolved
            );
        }
    }

    #[test]
    fn exact_entrypoint_wire_rejects_hash_equivalent_authorization_splice() {
        let key = KeyPair::try_from_seed(vec![0x42; 32], iroha_crypto::Algorithm::Ed25519)
            .expect("fixture key");
        let network_id = "a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5"
            .parse()
            .expect("fixture network id");
        let transaction = TransactionBuilder::new(
            network_id,
            AccountId::new(key.public_key().clone()),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .try_sign(key.private_key())
        .expect("fixture transaction");
        let exact = transaction
            .encode_wire_v1()
            .expect("exact transaction wire");
        let mut spliced = transaction.clone();
        spliced.set_multisig_signatures(MultisigSignatures::new(Vec::new()));
        assert_eq!(spliced.hash(), transaction.hash());
        assert_ne!(
            spliced.encode_wire_v1().expect("spliced transaction wire"),
            exact
        );
        assert!(
            require_committed_entrypoint_wire(&TransactionEntrypoint::External(spliced), &exact)
                .is_err()
        );
        assert!(
            require_committed_entrypoint_wire(
                &TransactionEntrypoint::External(transaction),
                &exact
            )
            .is_ok()
        );
    }

    #[test]
    fn journal_bound_status_identity_and_malformed_applied_are_uncertain() {
        let key = KeyPair::try_from_seed(vec![0x43; 32], iroha_crypto::Algorithm::Ed25519)
            .expect("fixture key");
        let network_id = "b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5"
            .parse()
            .expect("fixture network id");
        let transaction = TransactionBuilder::new(
            network_id,
            AccountId::new(key.public_key().clone()),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .try_sign(key.private_key())
        .expect("fixture transaction");
        let journal = Path::new(
            "/var/lib/iroha/kagemusha-rollout-v1/fixture/activation-submission-journal-v1.norito",
        );
        let mut status = PipelineTransactionStatusResponse::new(
            transaction.hash().to_string(),
            iroha_torii_shared::PipelineTransactionStatus {
                kind: "Applied".to_owned(),
                block_height: Some(7),
            },
            "auto".to_owned(),
            "state".to_owned(),
        );
        assert!(
            require_journal_bound_status_response(
                &status,
                &transaction,
                journal,
                "fixture status identity",
            )
            .is_ok()
        );

        let inconsistent = TransactionWaitOutcome {
            hash: transaction.hash().to_string(),
            terminal_kind: "Rejected".to_owned(),
            attempts: 1,
            elapsed_ms: 1,
            block_height: Some(7),
            scope: "auto".to_owned(),
            resolved_from: "state".to_owned(),
            r#final: status.clone(),
        };
        let error =
            require_journal_bound_wait_outcome(&inconsistent, &transaction, journal).unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains("submission-uncertain for activation"));
        assert!(message.contains("terminal wait outcome/final reconciliation"));
        assert!(message.contains(&journal.display().to_string()));

        status.hash = "00".repeat(32);
        let error = require_journal_bound_status_response(
            &status,
            &transaction,
            journal,
            "fixture status identity",
        )
        .unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains("submission-uncertain for activation"));
        assert!(message.contains("fixture status identity"));
        assert!(message.contains(&journal.display().to_string()));
        assert!(message.contains(&transaction.hash().to_string()));

        status.hash = transaction.hash().to_string();
        status.status.block_height = None;
        let error = applied_carrier_height_for_submission(
            &status,
            &transaction,
            journal,
            "fixture malformed Applied",
        )
        .unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains("submission-uncertain for activation"));
        assert!(message.contains("fixture malformed Applied"));
        assert!(message.contains(&journal.display().to_string()));
    }

    #[test]
    fn public_key_text_must_round_trip_canonically() {
        let key =
            iroha_crypto::KeyPair::try_from_seed(vec![0x41; 32], iroha_crypto::Algorithm::Ed25519)
                .unwrap();
        let canonical = key.public_key().to_string();
        assert_eq!(
            parse_public_key(&canonical, "fixture").unwrap(),
            key.public_key().clone()
        );
        assert!(parse_public_key(&format!("{canonical}\n"), "fixture").is_err());
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rollout_artifact_paths_are_canonical_and_promotion_keyed() {
        let promotion_id = [0x5a; 32];
        let expected = Path::new(ROLLOUT_STATE_ROOT)
            .join(hex::encode(promotion_id))
            .join(EXPECTATIONS_FILE_NAME);
        assert_eq!(
            rollout_state_path(promotion_id, EXPECTATIONS_FILE_NAME).unwrap(),
            expected
        );
        assert!(
            require_rollout_state_path(&expected, promotion_id, EXPECTATIONS_FILE_NAME).is_ok()
        );
        assert!(
            require_rollout_state_path(
                &expected.with_file_name("alternate.norito"),
                promotion_id,
                EXPECTATIONS_FILE_NAME,
            )
            .is_err()
        );
        assert_ne!(
            rollout_state_path([0x5b; 32], EXPECTATIONS_FILE_NAME).unwrap(),
            expected
        );
        let canary = expected.with_file_name(CANARY_EVIDENCE_FILE_NAME);
        assert_eq!(
            rollout_state_path(promotion_id, CANARY_EVIDENCE_FILE_NAME).unwrap(),
            canary
        );
        assert!(
            require_rollout_state_path(&canary, promotion_id, CANARY_EVIDENCE_FILE_NAME,).is_ok()
        );
        for file_name in [
            CANARY_AUTHORIZATION_FILE_NAME,
            CANARY_AUTHORIZATION_SUBMISSION_JOURNAL_FILE_NAME,
            CANARY_SUBMISSION_JOURNAL_FILE_NAME,
            CANARY_EVIDENCE_FILE_NAME,
            CANARY_VALIDATOR_LIVENESS_CHALLENGE_FILE_NAME,
            CANARY_VALIDATOR_LIVENESS_EVIDENCE_FILE_NAME,
        ] {
            let path = expected.with_file_name(file_name);
            assert_eq!(rollout_state_path(promotion_id, file_name).unwrap(), path);
            assert!(require_rollout_state_path(&path, promotion_id, file_name).is_ok());
            assert!(
                require_rollout_state_path(
                    &path.with_file_name("wrong.norito"),
                    promotion_id,
                    file_name,
                )
                .is_err()
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn stable_read_rejects_links_writable_inputs_and_overrun() {
        let dir = safe_owned_tempdir();
        let uid = rustix::process::geteuid().as_raw();
        let source = fs::canonicalize(dir.path()).unwrap().join("source");
        fs::write(&source, b"exact").unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o444)).unwrap();
        assert_eq!(read_owned(&source, 5, "fixture", uid).unwrap(), b"exact");
        let link = dir.path().join("link");
        symlink(&source, &link).unwrap();
        assert!(read_owned(&link, 5, "fixture", uid).is_err());
        fs::set_permissions(&source, fs::Permissions::from_mode(0o666)).unwrap();
        assert!(read_owned(&source, 5, "fixture", uid).is_err());
        fs::set_permissions(&source, fs::Permissions::from_mode(0o400)).unwrap();
        assert!(read_owned(&source, 5, "fixture", uid).is_err());
        fs::set_permissions(&source, fs::Permissions::from_mode(0o444)).unwrap();
        assert!(read_owned(&source, 4, "fixture", uid).is_err());

        let unsafe_parent = fs::canonicalize(dir.path()).unwrap().join("unsafe-parent");
        fs::create_dir(&unsafe_parent).unwrap();
        fs::set_permissions(&unsafe_parent, fs::Permissions::from_mode(0o777)).unwrap();
        let nested = unsafe_parent.join("artifact");
        fs::write(&nested, b"exact").unwrap();
        fs::set_permissions(&nested, fs::Permissions::from_mode(0o444)).unwrap();
        assert!(read_owned(&nested, 5, "fixture", uid).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn private_artifact_read_requires_mode_0400_and_private_parent() {
        let dir = safe_owned_tempdir();
        let uid = rustix::process::geteuid().as_raw();
        let root = fs::canonicalize(dir.path()).unwrap();
        let source = root.join("private");
        fs::write(&source, b"exact").unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o400)).unwrap();
        assert_eq!(
            read_owned_with_policy(&source, 5, "private fixture", uid, 0o400, true).unwrap(),
            b"exact"
        );

        fs::set_permissions(&source, fs::Permissions::from_mode(0o444)).unwrap();
        assert!(read_owned_with_policy(&source, 5, "private fixture", uid, 0o400, true).is_err());
        fs::set_permissions(&source, fs::Permissions::from_mode(0o400)).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(read_owned_with_policy(&source, 5, "private fixture", uid, 0o400, true).is_err());
    }

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "redox"))]
    #[test]
    fn publication_is_mode_0400_private_no_replace_and_commit_uncertain() {
        use std::cell::Cell;

        let dir = safe_owned_tempdir();
        let root = fs::canonicalize(dir.path()).unwrap();
        let uid = rustix::process::geteuid().as_raw();
        let output = root.join("artifact");
        assert!(open_owned_publication_destination(&output, uid).is_ok());
        let verification_calls = Cell::new(0_u8);
        publish_owned_with(
            &output,
            b"first",
            uid,
            |_| {
                verification_calls.set(verification_calls.get().saturating_add(1));
                Ok(())
            },
            || Ok(()),
        )
        .unwrap();
        assert_eq!(verification_calls.get(), 2);
        assert_eq!(fs::read(&output).unwrap(), b"first");
        assert_eq!(fs::metadata(&output).unwrap().mode() & 0o7777, 0o400);
        assert!(open_owned_publication_destination(&output, uid).is_err());
        assert!(publish_owned_with(&output, b"second", uid, |_| Ok(()), || Ok(())).is_err());
        assert_eq!(fs::read(&output).unwrap(), b"first");

        let trailing = PathBuf::from(format!("{}/trailing/", root.display()));
        assert!(publish_owned_with(&trailing, b"bad", uid, |_| Ok(()), || Ok(())).is_err());
        assert!(!root.join("trailing").exists());
        let dotted = PathBuf::from(format!("{}/dotted/.", root.display()));
        assert!(publish_owned_with(&dotted, b"bad", uid, |_| Ok(()), || Ok(())).is_err());
        assert!(!root.join("dotted").exists());

        let uncertain = root.join("uncertain");
        let error = publish_owned_with(
            &uncertain,
            b"committed",
            uid,
            |_| Ok(()),
            || Err(std::io::Error::other("post-commit probe")),
        )
        .unwrap_err();
        assert!(matches!(error, PublicationError::CommitUncertain { .. }));
        assert_eq!(fs::read(&uncertain).unwrap(), b"committed");

        let expired = root.join("expired");
        let error = publish_owned_with(
            &expired,
            b"expired",
            uid,
            |_| Err("freshness window elapsed".to_owned()),
            || Ok(()),
        )
        .unwrap_err();
        assert!(matches!(error, PublicationError::PreCommit(_)));
        assert!(!expired.exists());
    }

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "redox"))]
    #[test]
    fn publication_rejects_world_traversable_parent() {
        let dir = safe_owned_tempdir();
        let root = fs::canonicalize(dir.path()).unwrap();
        let uid = rustix::process::geteuid().as_raw();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        let output = root.join("artifact");
        assert!(open_owned_publication_destination(&output, uid).is_err());
        assert!(publish_owned_with(&output, b"secret", uid, |_| Ok(()), || Ok(())).is_err());
        assert!(!output.exists());
    }
}
