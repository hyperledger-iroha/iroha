//! Finality-anchored committee observations and exact preparation instructions.

use crate::{Run, RunContext};
use eyre::{Result, WrapErr, ensure, eyre};
use iroha::client::Client;
use iroha_core::{
    beacon::{
        GlobalThresholdBeaconSessionBindingV1, ValidatedGlobalThresholdBeaconSessionV1,
        global_threshold_beacon_roster_hash_v1, validate_global_threshold_beacon_session_v1,
        verify_global_threshold_beacon_seat_readiness_v1,
    },
    zk::kagemusha_v1_recursion::{
        verify_kagemusha_mint_finality_candidate_possession_v1,
        verify_kagemusha_mint_finality_seat_readiness_v1,
    },
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    block::consensus_v2::{HeightContext, HeightContextId, finality::V2FinalityArtifact},
    bridge::BridgeFinalityVerifier,
    isi::{InstructionBox, SetParameter},
    nexus::{
        AdmitValidatorCommitteeSeatV1, PrepareValidatorCommitteeCredentialsV1,
        ValidatorCandidateKeysV1, ValidatorCommitteeCredentialsV1, ValidatorCommitteeOperationV1,
        ValidatorCommitteeStatusV1, ValidatorCommitteeTransitionV1,
    },
    parameter::Parameter,
};
use norito::json::{self, JsonDeserialize};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Write},
    num::NonZeroU64,
    path::PathBuf,
    time::{Duration, Instant},
};

const MAX_PUBLIC_BYTES: u64 = 16 * 1024 * 1024;
const MAX_FINALITY_HEIGHTS: u64 = 1_048_576;

/// Committee operations never activate or cancel a committee; boundary finality owns that decision.
#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Inspect exact finalized selection and cryptographically verified preparation progress
    Status(QueryArgs),
    /// Publish one candidate's generation keys and paired possession proof
    PublishCandidate(PublishCandidateArgs),
    /// Submit complete credentials derived only from the exact selected candidates and beacon transcript
    PrepareCredentials(QueryArgs),
    /// Submit one verified seat proof under the exact pending transition
    AdmitSeat(AdmitSeatArgs),
    /// Export independently anchored, incumbent-authorized public custody evidence
    ExportCustodyEvidence(ExportCustodyEvidenceArgs),
    /// Export the certified frozen roster before any DKG transcript exists
    ExportSelectionEvidence(ExportSelectionEvidenceArgs),
}

/// Explicit external trust and bounded finality work shared by every committee operation.
#[derive(clap::Args, Debug)]
pub struct QueryArgs {
    /// Independently trusted HeightContextId, never copied from the status response
    #[arg(long)]
    trusted_context_id: Hash,
    /// Exact height governed by that independent context pin
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
    anchor_height: u64,
    /// Target scheduling epoch; defaults to the authenticated current epoch plus one
    #[arg(long)]
    target_epoch: Option<u64>,
    /// Maximum contiguous heights to verify from the original anchor
    #[arg(long, default_value_t = 65_536, value_parser = clap::value_parser!(u64).range(1..=MAX_FINALITY_HEIGHTS))]
    max_finality_heights: u64,
    /// Total deadline for status and contiguous proof reads
    #[arg(long, default_value_t = 120_000, value_parser = clap::value_parser!(u64).range(1..))]
    timeout_ms: u64,
}

/// Publish one independently provisioned, peer-consented candidate.
#[derive(clap::Args, Debug)]
pub struct PublishCandidateArgs {
    #[command(flatten)]
    query: QueryArgs,
    /// Canonical ValidatorCandidateKeysV1 output from Kagami independent candidate provisioning
    #[arg(long, value_name = "FILE")]
    candidate: PathBuf,
}

/// Admit actual key and share custody for one exact selected seat.
#[derive(clap::Args, Debug)]
pub struct AdmitSeatArgs {
    #[command(flatten)]
    query: QueryArgs,
    /// Canonical AdmitValidatorCommitteeSeatV1 output from offline exact-attempt proof production
    #[arg(long, value_name = "FILE")]
    readiness: PathBuf,
}

/// Public evidence for the daemon's offline pending-custody provisioning command.
#[derive(clap::Args, Debug)]
pub struct ExportCustodyEvidenceArgs {
    #[command(flatten)]
    query: QueryArgs,
    /// Independently selected exact transition identifier
    #[arg(long)]
    transition_id: Hash,
    /// Canonical public incumbent-QC FinalizeGlobalBeaconKey certificate JSON
    #[arg(long, value_name = "FILE")]
    beacon_finalization_certificate: PathBuf,
    /// New canonical binary Norito evidence file
    #[arg(long, value_name = "FILE")]
    out: PathBuf,
}

/// Public finality evidence authorizing only DKG for one frozen selection.
#[derive(clap::Args, Debug)]
pub struct ExportSelectionEvidenceArgs {
    #[command(flatten)]
    query: QueryArgs,
    /// Independently selected exact transition identifier
    #[arg(long)]
    transition_id: Hash,
    /// New canonical binary Norito evidence file
    #[arg(long, value_name = "FILE")]
    out: PathBuf,
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let query = match &self {
            Self::Status(args) | Self::PrepareCredentials(args) => args,
            Self::PublishCandidate(args) => &args.query,
            Self::AdmitSeat(args) => &args.query,
            Self::ExportCustodyEvidence(args) => &args.query,
            Self::ExportSelectionEvidence(args) => &args.query,
        };
        let client = context.client_from_config()?;
        let retain_chain = matches!(
            &self,
            Self::ExportCustodyEvidence(_) | Self::ExportSelectionEvidence(_)
        );
        if retain_chain {
            ensure!(
                query.target_epoch.is_some(),
                "committee evidence export requires explicit --target-epoch"
            );
            ensure!(query.max_finality_heights <= iroha_core::validator_committee_evidence::COMMITTEE_PROVISIONING_FINALITY_MAX_COUNT_V1 as u64,
                "committee evidence export finality budget exceeds the canonical evidence limit");
        }
        let (status, finality_chain) =
            observe(&client, context.config().network_id, query, retain_chain)?;
        let operation = match self {
            Self::Status(_) => return context.print_data(&status),
            Self::ExportSelectionEvidence(args) => {
                use iroha_core::validator_committee_evidence::{
                    COMMITTEE_PROVISIONING_EVIDENCE_MAX_BYTES_V1,
                    ValidatorCommitteeSelectionEvidenceV1,
                    verify_validator_committee_selection_evidence_v1,
                };
                let evidence = ValidatorCommitteeSelectionEvidenceV1 {
                    status,
                    finality_chain,
                };
                verify_validator_committee_selection_evidence_v1(
                    &evidence,
                    context.config().network_id,
                    HeightContextId(HashOf::from_untyped_unchecked(
                        args.query.trusted_context_id,
                    )),
                    args.query.anchor_height,
                    args.query.target_epoch.expect("checked explicit target"),
                    args.transition_id.into(),
                )
                .map_err(|error| eyre!(error))?;
                let bytes = norito::encode_canonical(&evidence)?;
                ensure!(
                    bytes.len() <= COMMITTEE_PROVISIONING_EVIDENCE_MAX_BYTES_V1,
                    "selection evidence exceeds its canonical byte limit"
                );
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&args.out)
                    .wrap_err("cannot create new selection evidence file")?;
                file.write_all(&bytes)?;
                file.sync_all()?;
                return context.println(format!(
                    "Exported certified frozen selection: {} (DKG preparation only)",
                    args.out.display()
                ));
            }
            Self::ExportCustodyEvidence(args) => {
                use iroha_core::validator_committee_evidence::{
                    COMMITTEE_PROVISIONING_EVIDENCE_MAX_BYTES_V1,
                    ValidatorCommitteeProvisioningEvidenceV1,
                    verify_validator_committee_provisioning_evidence_v1,
                };
                let evidence = ValidatorCommitteeProvisioningEvidenceV1 {
                    status,
                    finality_chain,
                    beacon_finalization: read_public(&args.beacon_finalization_certificate)?,
                };
                verify_validator_committee_provisioning_evidence_v1(
                    &evidence,
                    context.config().network_id,
                    HeightContextId(HashOf::from_untyped_unchecked(
                        args.query.trusted_context_id,
                    )),
                    args.query.anchor_height,
                    args.query.target_epoch.expect("checked explicit target"),
                    args.transition_id.into(),
                )
                .map_err(|error| eyre!(error))?;
                let bytes = norito::encode_canonical(&evidence)?;
                ensure!(
                    bytes.len() <= COMMITTEE_PROVISIONING_EVIDENCE_MAX_BYTES_V1,
                    "custody evidence exceeds its canonical byte limit"
                );
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&args.out)
                    .wrap_err("cannot create new custody evidence file")?;
                file.write_all(&bytes)?;
                file.sync_all()?;
                return context.println(format!("Exported authorized pending-custody evidence: {} (no activation or transaction-inclusion claim)", args.out.display()));
            }
            Self::PublishCandidate(args) => {
                let candidate: ValidatorCandidateKeysV1 = read_public(&args.candidate)?;
                verify_candidate(&candidate, status.network_id, next_generation(&status)?)?;
                ValidatorCommitteeOperationV1::PublishCandidate(candidate)
            }
            Self::PrepareCredentials(_) => {
                let mut transition = pending_transition(&status)?.clone();
                ensure!(
                    transition.credentials.is_none(),
                    "credentials are already prepared for this exact attempt"
                );
                let preparation = &transition.preparation;
                let validators = preparation
                    .roster
                    .iter()
                    .map(|voter| {
                        status
                            .candidate_keys
                            .iter()
                            .find(|candidate| candidate.keys.validator == voter.validator)
                            .map(|candidate| candidate.keys.clone())
                            .ok_or_else(|| {
                                eyre!(
                                    "selected peer {} has not published exact generation keys",
                                    voter.validator
                                )
                            })
                    })
                    .collect::<Result<Vec<_>>>()?;
                let session = status
                    .pending_beacon_session
                    .as_ref()
                    .ok_or_else(|| eyre!("the exact pending beacon session is not finalized"))?;
                let credentials = ValidatorCommitteeCredentialsV1 {
                    authority: iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalityAuthorityGenerationV1 {
                        version: 1, network_id: status.network_id,
                        generation: preparation.authority_generation, validators,
                    },
                    beacon: iroha_data_model::isi::kagemusha_v1::InstalledBeaconEpochBindingV1 {
                        session_id: session.session_id, transcript_hash: session.transcript_hash,
                    },
                };
                transition.credentials = Some(credentials.clone());
                verify_progress(&status, &transition)?;
                ValidatorCommitteeOperationV1::PrepareCredentials(
                    PrepareValidatorCommitteeCredentialsV1 {
                        transition_id: transition
                            .preparation
                            .transition_id()
                            .map_err(|e| eyre!(e))?,
                        target_epoch: transition.preparation.target_epoch,
                        credentials,
                    },
                )
            }
            Self::AdmitSeat(args) => {
                let mut transition = pending_transition(&status)?.clone();
                let admission: AdmitValidatorCommitteeSeatV1 = read_public(&args.readiness)?;
                ensure!(
                    admission.transition_id
                        == transition
                            .preparation
                            .transition_id()
                            .map_err(|e| eyre!(e))?
                        && admission.target_epoch == transition.preparation.target_epoch,
                    "readiness instruction differs from the exact selected attempt"
                );
                let readiness = admission.readiness;
                ensure!(
                    !transition
                        .readiness
                        .iter()
                        .any(|row| row.validator_index == readiness.validator_index),
                    "seat readiness is already recorded for this attempt"
                );
                transition.readiness.push(readiness.clone());
                transition.readiness.sort_by_key(|row| row.validator_index);
                verify_progress(&status, &transition)?;
                ValidatorCommitteeOperationV1::AdmitSeat(AdmitValidatorCommitteeSeatV1 {
                    transition_id: transition
                        .preparation
                        .transition_id()
                        .map_err(|e| eyre!(e))?,
                    target_epoch: transition.preparation.target_epoch,
                    readiness,
                })
            }
        };
        // Render the exact public effect before the ordinary fee quote and signing boundary.
        eprintln!("{}", json::to_json(&operation)?);
        context.finish(vec![operation_instruction(operation)])
    }
}

fn read_public<T: JsonDeserialize>(path: &PathBuf) -> Result<T> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_PUBLIC_BYTES + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        u64::try_from(bytes.len())? <= MAX_PUBLIC_BYTES,
        "committee public input exceeds the byte limit"
    );
    json::from_slice(&bytes).wrap_err("committee public input must use its exact canonical schema")
}

fn operation_instruction(operation: ValidatorCommitteeOperationV1) -> InstructionBox {
    SetParameter::new(Parameter::Custom(operation.into_custom_parameter())).into()
}

fn current_authorization(
    status: &ValidatorCommitteeStatusV1,
) -> &iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalityEpochAuthorizationV1 {
    let context = &status.latest_finality.height_context;
    context
        .next_epoch_snapshot
        .as_ref()
        .map_or(&context.kagemusha_mint_finality_authorization, |next| {
            &next.kagemusha_mint_finality_authorization
        })
}

fn next_generation(status: &ValidatorCommitteeStatusV1) -> Result<u64> {
    current_authorization(status)
        .authority_generation
        .checked_add(1)
        .ok_or_else(|| eyre!("authority generation overflow"))
}

fn pending_transition(
    status: &ValidatorCommitteeStatusV1,
) -> Result<&ValidatorCommitteeTransitionV1> {
    let transition = &status
        .selected
        .as_ref()
        .ok_or_else(|| eyre!("no frozen preparation exists for the requested target"))?
        .transition;
    ensure!(
        transition.outcome.is_none(),
        "the selected transition already has a certified terminal outcome"
    );
    let current = current_authorization(status);
    ensure!(
        current.epoch.checked_add(1) == Some(transition.preparation.target_epoch)
            && status.latest_finality.height_context.height
                < transition.preparation.first_height - 1,
        "the exact target is outside its preparation window"
    );
    transition
        .preparation
        .validate_against_preparing_authorization(current)
        .map_err(|e| eyre!(e))?;
    Ok(transition)
}

fn validate_finality_budget(args: &QueryArgs, last: u64) -> Result<()> {
    ensure!(
        args.anchor_height > 0,
        "finality anchor height must be nonzero"
    );
    ensure!(
        (1..=MAX_FINALITY_HEIGHTS).contains(&args.max_finality_heights),
        "invalid finality height budget"
    );
    let count = last
        .checked_sub(args.anchor_height)
        .and_then(|n| n.checked_add(1))
        .ok_or_else(|| eyre!("status tip precedes the original finality anchor"))?;
    ensure!(
        count <= args.max_finality_heights,
        "committee finality work exceeds the explicit height budget"
    );
    Ok(())
}

fn observe(
    client: &Client,
    network: NetworkId,
    args: &QueryArgs,
    retain_chain: bool,
) -> Result<(
    ValidatorCommitteeStatusV1,
    Vec<iroha_data_model::bridge::BridgeFinalityProof>,
)> {
    let deadline = Instant::now()
        .checked_add(Duration::from_millis(args.timeout_ms))
        .ok_or_else(|| eyre!("committee observation deadline overflow"))?;
    let client = client.with_request_deadline(deadline);
    let status = client.get_validator_committee_status(args.target_epoch)?;
    ensure!(
        status.network_id == network && status.latest_finality.height_context.network_id == network,
        "committee status belongs to another network"
    );
    let last = status.latest_finality.height_context.height;
    validate_finality_budget(args, last)?;
    if let Some(selected) = &status.selected {
        selected.transition.validate().map_err(|e| eyre!(e))?;
    }
    let selected_height = status
        .selected
        .as_ref()
        .map(|selected| selected.transition.preparation.selection_height);
    ensure!(
        selected_height.is_none_or(|height| height >= args.anchor_height),
        "trusted anchor must precede or equal the selecting boundary"
    );
    let outcome_height = status.selected.as_ref().and_then(|s| {
        s.transition
            .outcome
            .as_ref()
            .map(|_| s.transition.preparation.first_height - 1)
    });
    let trusted = HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(
        args.trusted_context_id,
    ));
    let mut verifier = BridgeFinalityVerifier::with_context(network, trusted);
    let mut authenticated = BTreeMap::<u64, V2FinalityArtifact>::new();
    let mut finality_chain = Vec::new();
    let mut retained_bytes = 0_usize;
    for height in args.anchor_height..=last {
        ensure!(
            Instant::now() < deadline,
            "committee finality observation deadline exceeded"
        );
        let proof = client
            .get_next_bridge_finality_proof(NonZeroU64::new(height).unwrap(), &mut verifier)?;
        if height == last || Some(height) == selected_height || Some(height) == outcome_height {
            authenticated.insert(height, proof.finality_artifact.clone());
        }
        if retain_chain {
            retained_bytes = retained_bytes
                .checked_add(norito::encode_canonical(&proof)?.len())
                .ok_or_else(|| eyre!("custody evidence size overflow"))?;
            ensure!(retained_bytes <= iroha_core::validator_committee_evidence::COMMITTEE_PROVISIONING_EVIDENCE_MAX_BYTES_V1,
                "custody evidence chain exceeds its canonical byte limit");
            finality_chain.push(proof);
        }
    }
    ensure!(
        authenticated.get(&last) == Some(&status.latest_finality),
        "reported committee tip differs from anchored finality"
    );
    let current = current_authorization(&status);
    let expected_target = args.target_epoch.map_or_else(
        || {
            current
                .epoch
                .checked_add(1)
                .ok_or_else(|| eyre!("target epoch overflow"))
        },
        Ok,
    )?;
    ensure!(
        status.target_epoch == expected_target,
        "committee query selected another target"
    );
    if let Some(selected) = &status.selected {
        ensure!(
            status.candidate_keys.len() <= selected.transition.preparation.roster.len()
                && status.candidate_keys.iter().all(|candidate| selected
                    .transition
                    .preparation
                    .roster
                    .iter()
                    .any(|seat| seat.validator == candidate.keys.validator)),
            "status contains publications outside the exact selected roster"
        );
    } else {
        ensure!(
            status.candidate_keys.is_empty(),
            "status without a selected roster cannot enumerate candidate publications"
        );
    }
    ensure!(
        status
            .candidate_keys
            .windows(2)
            .all(|pair| pair[0].keys.validator < pair[1].keys.validator),
        "candidate publications must be unique and strictly ordered"
    );
    for candidate in &status.candidate_keys {
        ensure!(
            Instant::now() < deadline,
            "committee candidate verification deadline exceeded"
        );
        let generation = status
            .selected
            .as_ref()
            .expect("nonempty publications require a validated selected roster")
            .transition
            .preparation
            .authority_generation;
        verify_candidate(candidate, network, generation)?;
    }
    if let Some(selected) = &status.selected {
        let preparation = &selected.transition.preparation;
        ensure!(
            preparation.network_id == network && preparation.target_epoch == status.target_epoch,
            "selected committee belongs to another network or target"
        );
        ensure!(
            authenticated.get(&preparation.selection_height) == Some(&selected.selecting_finality),
            "selection artifact differs from anchored finality"
        );
        let snapshot = selected
            .selecting_finality
            .height_context
            .next_epoch_snapshot
            .as_ref()
            .ok_or_else(|| eyre!("selection artifact has no boundary snapshot"))?;
        ensure!(
            snapshot.committee_preparation.as_ref() == Some(preparation),
            "boundary did not certify this exact preparation"
        );
        preparation
            .validate_against_preparing_authorization(
                &snapshot.kagemusha_mint_finality_authorization,
            )
            .map_err(|e| eyre!(e))?;
        if let Some(outcome) = &selected.transition.outcome {
            let artifact = authenticated
                .get(&(preparation.first_height - 1))
                .ok_or_else(|| eyre!("transition outcome lacks its authenticated boundary"))?;
            ensure!(
                artifact
                    .height_context
                    .next_epoch_snapshot
                    .as_ref()
                    .is_some_and(|next| &next.kagemusha_mint_finality_authorization == outcome),
                "transition terminal body differs from its certified boundary"
            );
        }
        verify_progress(&status, &selected.transition)?;
    } else {
        ensure!(
            status.pending_beacon_session.is_none(),
            "beacon session cannot establish an absent preparation"
        );
    }
    ensure!(
        Instant::now() < deadline,
        "committee observation verification deadline exceeded"
    );
    Ok((status, finality_chain))
}

fn verify_candidate(
    candidate: &ValidatorCandidateKeysV1,
    network: NetworkId,
    generation: u64,
) -> Result<()> {
    candidate.validate().map_err(|e| eyre!(e))?;
    candidate
        .peer_signature
        .verify(
            candidate.keys.validator.public_key(),
            &candidate.authorization(),
        )
        .map_err(|e| eyre!("candidate peer consent verification failed: {e}"))?;
    ensure!(
        candidate.network_id == network && candidate.generation == generation,
        "candidate publication differs from the exact network and generation"
    );
    verify_kagemusha_mint_finality_candidate_possession_v1(
        network,
        generation,
        &candidate.keys,
        &candidate.possession,
    )
    .map_err(|e| eyre!("candidate possession verification failed: {e}"))
}

fn verify_session(
    status: &ValidatorCommitteeStatusV1,
    transition: &ValidatorCommitteeTransitionV1,
) -> Result<ValidatedGlobalThresholdBeaconSessionV1> {
    let preparation = &transition.preparation;
    let session = status
        .pending_beacon_session
        .as_ref()
        .ok_or_else(|| eyre!("exact pending beacon session is unavailable"))?;
    ensure!(
        session.network_id == preparation.network_id
            && session.session_id == preparation.beacon_session_id().map_err(|e| eyre!(e))?
            && session.adaptive_dkg.session.start_height > preparation.selection_height
            && session.adaptive_dkg.finalized_at_height < preparation.first_height - 1,
        "pending beacon transcript is outside this exact preparation window"
    );
    let peers = preparation
        .roster
        .iter()
        .map(|voter| voter.validator.clone())
        .collect::<Vec<_>>();
    let roster_hash = global_threshold_beacon_roster_hash_v1(&peers);
    ensure!(
        session.roster_hash == roster_hash && usize::from(session.committee_size) == peers.len(),
        "pending beacon roster does not match frozen selection"
    );
    validate_global_threshold_beacon_session_v1(
        session.clone(),
        &GlobalThresholdBeaconSessionBindingV1 {
            network_id: preparation.network_id,
            session_id: session.session_id,
            roster_hash,
            transcript_hash: session.transcript_hash,
        },
    )
    .map_err(|e| eyre!(e))
}

fn verify_progress(
    status: &ValidatorCommitteeStatusV1,
    transition: &ValidatorCommitteeTransitionV1,
) -> Result<()> {
    transition.validate().map_err(|e| eyre!(e))?;
    iroha_data_model::block::consensus_v2::finality::verify_validator_power_roster_pops(
        &transition.preparation.roster,
        &transition.preparation.validator_set_pops,
    )?;
    let session = if status.pending_beacon_session.is_some() {
        Some(verify_session(status, transition)?)
    } else {
        None
    };
    let Some(credentials) = &transition.credentials else {
        return Ok(());
    };
    for keys in &credentials.authority.validators {
        let candidate = status
            .candidate_keys
            .iter()
            .find(|candidate| &candidate.keys == keys)
            .ok_or_else(|| eyre!("prepared keys lack an exact candidate publication"))?;
        verify_candidate(
            candidate,
            status.network_id,
            credentials.authority.generation,
        )?;
    }
    let session = session
        .ok_or_else(|| eyre!("prepared credentials lack their complete beacon transcript"))?;
    ensure!(
        status
            .pending_beacon_session
            .as_ref()
            .is_some_and(|record| record.session_id == credentials.beacon.session_id
                && record.transcript_hash == credentials.beacon.transcript_hash),
        "prepared beacon binding differs from its transcript"
    );
    for seat in &transition.readiness {
        let context = transition
            .readiness_context(seat.validator_index)
            .map_err(|e| eyre!(e))?;
        verify_kagemusha_mint_finality_seat_readiness_v1(
            &credentials.authority,
            &context,
            &seat.pasta,
        )
        .map_err(|e| eyre!("Pasta seat possession failed: {e}"))?;
        verify_global_threshold_beacon_seat_readiness_v1(
            &session,
            &credentials.authority,
            &context,
            &seat.beacon,
        )
        .map_err(|e| eyre!("beacon seat possession failed: {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use iroha_core::zk::kagemusha_v1_recursion::{
        derive_kagemusha_mint_finality_validator_keys_v1,
        prove_kagemusha_mint_finality_candidate_possession_v1,
    };
    use iroha_crypto::{Algorithm, KeyPair, SignatureOf};
    use iroha_data_model::nexus::ValidatorCandidateKeyAuthorizationV1;
    use iroha_model_base::peer::PeerId;

    #[derive(Parser)]
    struct Cli {
        #[command(subcommand)]
        command: Command,
    }

    #[test]
    fn custody_evidence_export_requires_exact_attempt_certificate_and_output() {
        let hash = Hash::new(b"export-context").to_string();
        let args = [
            "committee",
            "export-custody-evidence",
            "--trusted-context-id",
            &hash,
            "--anchor-height",
            "1",
            "--target-epoch",
            "2",
            "--transition-id",
            &hash,
            "--beacon-finalization-certificate",
            "certificate.json",
            "--out",
            "custody.norito",
        ];
        assert!(Cli::try_parse_from(args).is_ok());
        assert!(Cli::try_parse_from(&args[..10]).is_err());
    }

    #[test]
    fn selection_evidence_export_requires_independent_context_target_and_attempt() {
        let hash = Hash::new(b"selection-export-context").to_string();
        let args = [
            "committee",
            "export-selection-evidence",
            "--trusted-context-id",
            &hash,
            "--anchor-height",
            "10",
            "--target-epoch",
            "2",
            "--transition-id",
            &hash,
            "--out",
            "selection.norito",
        ];
        assert!(Cli::try_parse_from(args).is_ok());
        assert!(Cli::try_parse_from(&args[..10]).is_err());
        let mut without_target = args.to_vec();
        without_target.drain(6..8);
        assert!(
            Cli::try_parse_from(without_target).is_ok(),
            "common query accepts omitted target for status, but execution rejects it for export"
        );
    }

    fn network(label: &[u8]) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(label)))
    }

    fn candidate() -> (ValidatorCandidateKeysV1, KeyPair) {
        let peer_key = KeyPair::from_seed(vec![0x31; 32], Algorithm::BlsNormal);
        let network_id = network(b"committee-cli-fixture");
        let generation = 7;
        let keys = derive_kagemusha_mint_finality_validator_keys_v1(
            &[0x41; 32],
            generation,
            PeerId::new(peer_key.public_key().clone()),
        )
        .unwrap();
        let possession = prove_kagemusha_mint_finality_candidate_possession_v1(
            &[0x41; 32],
            network_id,
            generation,
            &keys,
        )
        .unwrap();
        let peer_signature = SignatureOf::try_new(
            peer_key.private_key(),
            &ValidatorCandidateKeyAuthorizationV1::new(
                network_id,
                generation,
                keys.clone(),
                possession,
            ),
        )
        .unwrap();
        (
            ValidatorCandidateKeysV1 {
                network_id,
                generation,
                keys,
                possession,
                peer_signature,
            },
            peer_key,
        )
    }

    fn query() -> QueryArgs {
        QueryArgs {
            trusted_context_id: Hash::new(b"independently-installed-context"),
            anchor_height: 10,
            target_epoch: None,
            max_finality_heights: 21,
            timeout_ms: 1000,
        }
    }

    #[test]
    fn committee_parser_requires_external_anchor_for_every_operation() {
        for (command, file_flag) in [
            ("status", None),
            ("publish-candidate", Some("--candidate")),
            ("prepare-credentials", None),
            ("admit-seat", Some("--readiness")),
        ] {
            let mut argv = vec!["committee".to_owned(), command.to_owned()];
            if let Some(flag) = file_flag {
                argv.extend([flag.to_owned(), "public.json".to_owned()]);
            }
            assert!(Cli::try_parse_from(&argv).is_err());
            argv.extend([
                "--trusted-context-id".to_owned(),
                query().trusted_context_id.to_string(),
            ]);
            assert!(Cli::try_parse_from(&argv).is_err());
            argv.extend(["--anchor-height".to_owned(), "10".to_owned()]);
            assert!(Cli::try_parse_from(&argv).is_ok());
            let mut zero = argv.clone();
            *zero.last_mut().unwrap() = "0".to_owned();
            assert!(Cli::try_parse_from(zero).is_err());
            let mut unbounded = argv.clone();
            unbounded.extend([
                "--max-finality-heights".to_owned(),
                (MAX_FINALITY_HEIGHTS + 1).to_string(),
            ]);
            assert!(Cli::try_parse_from(unbounded).is_err());
        }
        for forbidden in ["activate", "cancel", "renew-epoch", "schedule"] {
            assert!(Cli::try_parse_from(["committee", forbidden]).is_err());
        }
    }

    #[test]
    fn finality_budget_is_inclusive_and_rejects_stale_or_unbounded_ranges() {
        let args = query();
        validate_finality_budget(&args, 10).unwrap();
        validate_finality_budget(&args, 30).unwrap();
        assert!(validate_finality_budget(&args, 31).is_err());
        assert!(validate_finality_budget(&args, 9).is_err());
        let mut zero = query();
        zero.anchor_height = 0;
        assert!(validate_finality_budget(&zero, 10).is_err());
        let mut huge = query();
        huge.max_finality_heights = u64::MAX;
        assert!(validate_finality_budget(&huge, u64::MAX).is_err());
    }

    #[test]
    fn candidate_verification_binds_peer_consent_network_generation_and_pasta_custody() {
        let (candidate, pair) = candidate();
        verify_candidate(&candidate, candidate.network_id, candidate.generation).unwrap();
        assert!(
            verify_candidate(
                &candidate,
                network(b"another-network"),
                candidate.generation
            )
            .is_err()
        );
        assert!(
            verify_candidate(&candidate, candidate.network_id, candidate.generation + 1).is_err()
        );
        let mut foreign_peer = candidate.clone();
        foreign_peer.peer_signature = SignatureOf::try_new(
            KeyPair::from_seed(vec![0x32; 32], Algorithm::BlsNormal).private_key(),
            &candidate.authorization(),
        )
        .unwrap();
        assert!(
            verify_candidate(&foreign_peer, candidate.network_id, candidate.generation).is_err()
        );
        let mut wrong_keys = candidate.clone();
        wrong_keys.keys = derive_kagemusha_mint_finality_validator_keys_v1(
            &[0x42; 32],
            candidate.generation,
            candidate.keys.validator.clone(),
        )
        .unwrap();
        wrong_keys.peer_signature =
            SignatureOf::try_new(pair.private_key(), &wrong_keys.authorization()).unwrap();
        assert!(verify_candidate(&wrong_keys, candidate.network_id, candidate.generation).is_err());
        let mut replay = candidate.clone();
        replay.generation += 1;
        assert!(verify_candidate(&replay, replay.network_id, replay.generation).is_err());
    }

    #[test]
    fn publication_emits_exact_reserved_operation_envelope() {
        let (candidate, _) = candidate();
        let operation = ValidatorCommitteeOperationV1::PublishCandidate(candidate);
        let parameter = operation.clone().into_custom_parameter();
        assert_eq!(
            ValidatorCommitteeOperationV1::from_custom_parameter(&parameter).unwrap(),
            Some(operation.clone())
        );
        assert_eq!(
            operation_instruction(operation),
            InstructionBox::from(SetParameter::new(Parameter::Custom(parameter)))
        );
    }

    #[test]
    fn candidate_file_requires_exact_schema_and_peer_consent() {
        let (candidate, _) = candidate();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("candidate.json");
        std::fs::write(&path, json::to_vec(&candidate).unwrap()).unwrap();
        assert_eq!(
            read_public::<ValidatorCandidateKeysV1>(&path).unwrap(),
            candidate
        );
        let mut body = json::to_value(&candidate).unwrap();
        body.as_object_mut()
            .unwrap()
            .insert("private_seed".to_owned(), json::Value::Null);
        std::fs::write(&path, json::to_vec(&body).unwrap()).unwrap();
        assert!(read_public::<ValidatorCandidateKeysV1>(&path).is_err());
        body.as_object_mut().unwrap().remove("private_seed");
        body.as_object_mut().unwrap().remove("peer_signature");
        std::fs::write(&path, json::to_vec(&body).unwrap()).unwrap();
        assert!(read_public::<ValidatorCandidateKeysV1>(&path).is_err());
        let file = File::create(&path).unwrap();
        file.set_len(MAX_PUBLIC_BYTES + 1).unwrap();
        assert!(
            read_public::<ValidatorCandidateKeysV1>(&path)
                .unwrap_err()
                .to_string()
                .contains("byte limit")
        );
    }
}
