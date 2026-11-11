//! Four-validator certified committee rotation of the mandatory `NPoS` threshold beacon.
//!
//! The first `NPoS` boundary freezes a committee-transition preparation for the epoch after
//! next. Inside the preparing epoch the harness runs the signed all-edge fixture ceremony of
//! that preparation's exact successor session, the incumbent committee certifies it as the
//! pending beacon key, a target owner binds it as the attempt's credentials, and every target
//! seat proves custody of its own share. The incumbent boundary certificate then activates the
//! prepared generation together with the successor session. The mandatory pre-boundary pulse
//! finalizes on the bootstrap session up to the activating boundary and on the successor
//! session after it. Requires the `parliament-test-signers` feature.

use super::*;
use iroha::data_model::nexus::{
    AdmitValidatorCommitteeSeatV1, PrepareValidatorCommitteeCredentialsV1,
    ValidatorCommitteeCredentialsV1, ValidatorCommitteeOperationV1,
    ValidatorCommitteeSeatReadinessV1, ValidatorCommitteeStatusV1,
};
use iroha_core::beacon::{
    ValidatedGlobalThresholdBeaconSessionV1, global_threshold_beacon_roster_hash_v1,
    parliament_test_network_signer::{
        TestNetworkParliamentBeaconPartialSignerV1,
        deterministic_parliament_beacon_key_record_for_session_v1,
    },
    prove_global_threshold_beacon_seat_readiness_v1,
};
use iroha_data_model::{
    consensus::{
        FinalizedGlobalThresholdBeaconPulseV1, GLOBAL_THRESHOLD_BEACON_VERSION_V1,
        GlobalThresholdBeaconDkgSessionV1, GlobalThresholdBeaconKeySessionV1,
    },
    sumeragi::epoch::{
        BeaconEpochBindingV1, InstalledBeaconEpochBindingV1, ValidatorEpochDecisionV1,
    },
};

// The preparing epoch carries, each on its own height before the epoch's last height, the
// successor transcript's four DKG windows, the incumbent certificate, the credentials and one
// readiness proof per target seat. Sixteen blocks keep that run clear of the pre-boundary pulse.
const ROTATION_NPOS_EPOCH_LENGTH_BLOCKS: u64 = 16;

// A boundary freezes the committee of the epoch after next; the epoch between prepares it.
const ROTATION_TARGET_EPOCH: u64 = 2;

/// Replay one exact public key-session transcript, as ordinary execution does on install.
fn validated_beacon_session(
    session: &GlobalThresholdBeaconKeySessionV1,
    budget: &iroha_allocation::AllocationBudget,
) -> Result<ValidatedGlobalThresholdBeaconSessionV1> {
    let binding = GlobalThresholdBeaconSessionBindingV1 {
        network_id: session.network_id,
        session_id: session.session_id,
        roster_hash: session.roster_hash,
        transcript_hash: session.transcript_hash,
    };
    validate_global_threshold_beacon_session_v1(session, &binding, budget)
        .wrap_err("replay the exact public beacon key-session transcript")
}

/// Read the public committee-transition projection of one target epoch.
///
/// The projection is a progress observation; the certified boundary results decide the
/// rotation. Only the server's explicit backpressure is retried, inside one bounded window.
async fn committee_transition(
    network: &iroha_test_network::Network,
    client: &Client,
    target_epoch: u64,
) -> Result<ValidatorCommitteeStatusV1> {
    let deadline = Instant::now() + OPERATION_TIMEOUT;
    loop {
        let read = tokio::time::timeout(
            deadline.saturating_duration_since(Instant::now()),
            client
                .client()
                .nexus()
                .validator_committee(Some(target_epoch)),
        )
        .await
        .map_err(|_| eyre!("committee-transition read exceeded {OPERATION_TIMEOUT:?}"))?;
        match read {
            Ok(status) => {
                if status.network_id != network.network_id() || status.target_epoch != target_epoch
                {
                    return Err(eyre!(
                        "committee-transition read answered another network or target epoch"
                    ));
                }
                return Ok(status);
            }
            Err(iroha::Error::Http {
                status: 429,
                retry_after,
                ..
            }) => {
                let delay = retry_after
                    .unwrap_or(Duration::from_millis(150))
                    .max(Duration::from_millis(150));
                if Instant::now() + delay >= deadline {
                    return Err(eyre!(
                        "committee-transition read stayed backpressured for {OPERATION_TIMEOUT:?}"
                    ));
                }
                tokio::time::sleep(delay).await;
            }
            Err(error) => return Err(error.into()),
        }
    }
}

/// The selected attempt of one committee-transition projection.
fn selected_transition(
    status: &ValidatorCommitteeStatusV1,
    label: &str,
) -> Result<iroha::data_model::nexus::ValidatorCommitteeTransitionV1> {
    status
        .selected
        .as_ref()
        .map(|selected| selected.transition.clone())
        .ok_or_else(|| eyre!("{label}: the frozen committee attempt is absent"))
}

/// Sign as the account that owns one bonded target validator.
///
/// The transaction enters through the validator behind `network.client()`, so its Applied
/// response and the corridor's later reads observe one State cut.
fn target_owner_client(
    network: &iroha_test_network::Network,
    validator: &PeerId,
) -> Result<Client> {
    let owner = network
        .peers()
        .iter()
        .find(|peer| peer.id() == *validator)
        .ok_or_else(|| eyre!("target seat {validator} has no validator process"))?;
    let ingress = network
        .peers()
        .first()
        .ok_or_else(|| eyre!("the corridor network has no ingress validator"))?;
    Ok(ingress.client_for(
        &owner.account_id(),
        owner.streaming_key_pair().private_key().clone(),
    ))
}

/// Wait until every validator holds `height` before reading it from each of them.
async fn await_every_validator(
    network: &iroha_test_network::Network,
    height: u64,
    label: &str,
) -> Result<()> {
    for peer in network.peers() {
        tokio::time::timeout(network.sync_timeout(), peer.once_block(height))
            .await
            .map_err(|_| eyre!("{label}: a validator did not commit height {height}"))?;
    }
    Ok(())
}

/// The one pulse every validator finalized at an exact mandatory pre-boundary height.
async fn agreed_mandatory_pulse(
    network: &iroha_test_network::Network,
    height: u64,
    label: &str,
) -> Result<FinalizedGlobalThresholdBeaconPulseV1> {
    await_every_validator(network, height, label).await?;
    let mut pulses = Vec::with_capacity(network.peers().len());
    for peer in network.peers() {
        pulses.push(pulse_at(&peer.client(), height, network).await?);
    }
    if !pulses.windows(2).all(|pair| pair[0] == pair[1]) {
        return Err(eyre!(
            "{label}: validators disagree on the pulse at {height}"
        ));
    }
    let pulse = pulses.swap_remove(0);
    if pulse.height != height
        || pulse.network_id != network.network_id()
        || pulse.round != 0
        || pulse.finalized_chain_anchor.height.checked_add(1) != Some(height)
    {
        return Err(eyre!(
            "{label}: height {height} carries no exact view-independent mandatory pulse"
        ));
    }
    Ok(pulse)
}

/// Verify a pulse under one replayed session, against its certified header and parent.
async fn verify_pulse_on_session(
    network: &iroha_test_network::Network,
    client: &Client,
    pulse: &FinalizedGlobalThresholdBeaconPulseV1,
    session: &ValidatedGlobalThresholdBeaconSessionV1,
    label: &str,
) -> Result<()> {
    let (expected_anchor, expected_context) =
        finality::certified_pulse_context(network, client, pulse.height, pulse).await?;
    verify_finalized_global_threshold_beacon_pulse_v1(
        session,
        pulse,
        expected_anchor,
        &expected_context,
    )
    .wrap_err_with(|| format!("{label}: independently verify the mandatory pulse"))?;
    Ok(())
}

#[test]
fn four_validator_certified_rotation_moves_mandatory_pulse_to_successor_session() -> Result<()> {
    let name =
        stringify!(four_validator_certified_rotation_moves_mandatory_pulse_to_successor_session);
    let handle = std::thread::Builder::new()
        .name(name.to_owned())
        .stack_size(PARLIAMENT_NETWORK_STACK_BYTES)
        .spawn(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .thread_stack_size(PARLIAMENT_NETWORK_STACK_BYTES)
                .enable_all()
                .build()
                .expect("build four-validator committee-rotation test runtime")
                .block_on(
                    four_validator_certified_rotation_moves_mandatory_pulse_to_successor_session_impl(),
                )
        })
        .expect("spawn four-validator committee-rotation test thread");
    match handle.join() {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

async fn four_validator_certified_rotation_moves_mandatory_pulse_to_successor_session_impl()
-> Result<()> {
    let builder = with_signed_npos_epoch(
        NetworkBuilder::new()
            .with_peers(VALIDATOR_COUNT)
            .with_auto_populated_trusted_peers()
            .with_npos_consensus()
            .with_parliament_beacon_signer_modes(POSITIVE_BEACON_SIGNER_MODES)
            .with_block_cadence(EXACT_HEIGHT_SUBMISSION_CADENCE)
            .with_config_layer(|layer| {
                layer
                    .write(
                        ["concurrency", "rayon_global_threads"],
                        PARLIAMENT_NETWORK_RAYON_THREADS_PER_PEER,
                    )
                    .write(
                        [
                            "network",
                            "soranet_handshake",
                            "pow",
                            "puzzle",
                            "memory_kib",
                        ],
                        i64::from(iroha_crypto::soranet::puzzle::MIN_MEMORY_KIB),
                    )
                    .write(
                        ["network", "soranet_handshake", "pow", "puzzle", "time_cost"],
                        1_i64,
                    )
                    .write(
                        ["network", "soranet_handshake", "pow", "puzzle", "lanes"],
                        1_i64,
                    )
                    .write(
                        ["nexus", "storage", "local_budget_bytes"],
                        TEST_NEXUS_LOCAL_STORAGE_BUDGET_BYTES,
                    );
            }),
        ROTATION_NPOS_EPOCH_LENGTH_BLOCKS,
    );
    let context =
        stringify!(four_validator_certified_rotation_moves_mandatory_pulse_to_successor_session);
    let network = sandbox::start_network_async_or_skip(builder, context).await?;
    let Some(network) = sandbox::enforce_network_start_requirement(network, context)? else {
        return Ok(());
    };
    assert_eq!(network.peers().len(), VALIDATOR_COUNT);
    network.ensure_blocks(1).await?;
    let client = network.client();
    assert_eq!(
        certified_npos_epoch_length(&network, &client).await?,
        ROTATION_NPOS_EPOCH_LENGTH_BLOCKS,
    );

    // The genesis committee installs the bootstrap key session exactly as the mandatory
    // corridor does; it signs every pulse until a certified boundary replaces it.
    let incumbent_roster = ordered_validator_roster(&network, &client).await?;
    let beacon_budget = iroha_allocation::AllocationBudget::new(64 * 1024 * 1024);
    let incumbent_record = deterministic_parliament_beacon_key_record_v1(
        network.network_id(),
        &ordered_validator_signing_keys(&network, &incumbent_roster)?,
        &beacon_budget,
    )
    .wrap_err("derive the bootstrap beacon fixture")?;
    let incumbent_session = validated_beacon_session(&incumbent_record.session, &beacon_budget)?;
    let incumbent_binding = InstalledBeaconEpochBindingV1 {
        session_id: incumbent_record.session.session_id,
        transcript_hash: incumbent_record.session.transcript_hash,
    };
    let install_height = next_execution_height(
        &client,
        incumbent_record.session.adaptive_dkg.finalized_at_height,
        "bootstrap beacon-key installation",
    )
    .await?;
    submit_parliament_instructions(
        &client,
        [lifecycle_certificate(
            &network,
            &incumbent_roster,
            ThresholdKeyLifecycleActionV1::FinalizeGlobalBeaconKey,
            incumbent_record.session.session_id,
            incumbent_record.session.transcript_hash,
            norito::encode_canonical(&incumbent_record)?,
            install_height,
        )?],
    )
    .await?;
    observe_exact_height(&client, install_height, "bootstrap beacon-key installation").await?;
    let activation_height = install_height
        .checked_add(1)
        .ok_or_else(|| eyre!("bootstrap beacon-key activation height overflow"))?;
    advance_to_height_with_carriers(
        &network,
        &client,
        activation_height,
        "bootstrap beacon-key activation",
    )
    .await?;

    let selecting_boundary = ROTATION_NPOS_EPOCH_LENGTH_BLOCKS;
    let first_pulse_height = selecting_boundary - 1;
    // A pulse never creates a block (`specs/sumeragi.md`): real work carries every pulse
    // and boundary height of this corridor.
    advance_to_height_with_carriers(
        &network,
        &client,
        first_pulse_height,
        "bootstrap-session mandatory pre-boundary pulse",
    )
    .await?;
    let first_pulse = agreed_mandatory_pulse(
        &network,
        first_pulse_height,
        "bootstrap-session mandatory pre-boundary pulse",
    )
    .await?;
    assert_eq!(first_pulse.session_id, incumbent_record.session.session_id);
    verify_pulse_on_session(
        &network,
        &client,
        &first_pulse,
        &incumbent_session,
        "bootstrap-session mandatory pre-boundary pulse",
    )
    .await?;

    // The selecting boundary freezes the complete attempt for the epoch after next, and the
    // epoch between is the preparing epoch whose current quorum decides that attempt.
    advance_to_height_with_carriers(
        &network,
        &client,
        selecting_boundary,
        "selecting NPoS boundary",
    )
    .await?;
    let preparing_first_height = selecting_boundary + 1;
    advance_to_height_with_carriers(
        &network,
        &client,
        preparing_first_height,
        "first height of the preparing epoch",
    )
    .await?;
    let (_, selecting) = finality::certified_block(&network, &client, selecting_boundary).await?;
    let preparation = selecting
        .commitment()
        .schedule
        .boundary
        .as_ref()
        .and_then(|boundary| boundary.preparation.clone())
        .ok_or_else(|| eyre!("the selecting boundary froze no committee-transition attempt"))?;
    let (_, preparing) =
        finality::certified_block(&network, &client, preparing_first_height).await?;
    let preparing_authorization = preparing.commitment().schedule.current.authorization;
    preparation
        .validate_against_preparing_authorization(&preparing_authorization)
        .map_err(|error| eyre!("frozen attempt is not bound to its preparing epoch: {error}"))?;
    assert_eq!(preparation.network_id, network.network_id());
    assert_eq!(preparation.selection_height, selecting_boundary);
    assert_eq!(preparation.target_epoch, ROTATION_TARGET_EPOCH);
    assert_eq!(
        preparation.first_height,
        ROTATION_TARGET_EPOCH * ROTATION_NPOS_EPOCH_LENGTH_BLOCKS + 1
    );
    assert_eq!(
        preparation.last_height,
        (ROTATION_TARGET_EPOCH + 1) * ROTATION_NPOS_EPOCH_LENGTH_BLOCKS
    );
    assert_eq!(preparation.authority_generation, 1);
    assert_eq!(
        preparing_authorization.beacon,
        BeaconEpochBindingV1::Installed(incumbent_binding),
        "the preparing epoch is signed under the installed bootstrap session",
    );
    let target_roster = preparation
        .committee
        .iter()
        .map(|seat| seat.validator.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        target_roster
            .iter()
            .collect::<std::collections::BTreeSet<_>>(),
        incumbent_roster
            .iter()
            .collect::<std::collections::BTreeSet<_>>(),
        "the four bonded validators are the whole candidate pool of the election",
    );
    let target_generation = preparation.generation();
    let target_authority_id = target_generation
        .generation_id()
        .map_err(|error| eyre!("target generation identity: {error}"))?;
    let transition_id = preparation
        .transition_id()
        .map_err(|error| eyre!("frozen attempt identity: {error}"))?;
    let frozen = committee_transition(&network, &client, ROTATION_TARGET_EPOCH).await?;
    let frozen_transition = selected_transition(&frozen, "frozen attempt")?;
    assert_eq!(frozen_transition.preparation, preparation);
    assert!(
        frozen_transition.credentials.is_none()
            && frozen_transition.readiness.is_empty()
            && frozen_transition.outcome.is_none()
            && frozen.pending_beacon_session.is_none(),
        "a freshly frozen attempt carries no credentials, readiness or outcome",
    );

    // Core installs a successor key only as the session of a certified committee-transition
    // preparation: its session and attempt identifiers, its authority generation, its ordered
    // target roster and DKG windows after the selecting boundary and before the cutoff.
    let dkg_start_height = preparing_first_height;
    let successor_dkg = GlobalThresholdBeaconDkgSessionV1 {
        version: GLOBAL_THRESHOLD_BEACON_VERSION_V1,
        network_id: preparation.network_id,
        session_id: preparation
            .beacon_session_id()
            .map_err(|error| eyre!("frozen attempt beacon session: {error}"))?,
        attempt_id: transition_id,
        authority_generation: preparation.authority_generation,
        roster_hash: global_threshold_beacon_roster_hash_v1(&target_roster),
        committee_size: u16::try_from(target_roster.len())?,
        threshold: u16::try_from((target_roster.len() - 1) / 3 + 1)?,
        start_height: dkg_start_height,
        commitments_end_height: dkg_start_height + 1,
        deliveries_end_height: dkg_start_height + 2,
        acceptances_end_height: dkg_start_height + 3,
    };
    assert!(
        successor_dkg.start_height > preparation.selection_height
            && successor_dkg.acceptances_end_height < preparation.first_height - 1,
        "the successor DKG must close inside the preparing epoch",
    );
    let successor_record = deterministic_parliament_beacon_key_record_for_session_v1(
        successor_dkg,
        &ordered_validator_signing_keys(&network, &target_roster)?,
        &beacon_budget,
    )
    .wrap_err("run the signed all-edge ceremony of the certified successor session")?;
    assert_eq!(successor_record.session.adaptive_dkg.session, successor_dkg);
    assert_eq!(
        successor_record.session.session_id,
        successor_dkg.session_id
    );
    assert_eq!(
        successor_record.session.roster_hash,
        successor_dkg.roster_hash
    );
    assert_eq!(successor_record.session.committee_size, 4);
    assert_eq!(successor_record.session.threshold, 2);
    assert_eq!(
        successor_record.session.adaptive_dkg.finalized_at_height,
        successor_dkg.acceptances_end_height,
    );
    assert_ne!(
        successor_record.session.group_public_key, incumbent_record.session.group_public_key,
        "the successor is a second key, not an alias of the bootstrap key",
    );
    let successor_session = validated_beacon_session(&successor_record.session, &beacon_budget)?;
    let successor_binding = InstalledBeaconEpochBindingV1 {
        session_id: successor_record.session.session_id,
        transcript_hash: successor_record.session.transcript_hash,
    };

    // The incumbent committee certifies the successor transcript as the pending key; the
    // compare-and-set predecessor is the active bootstrap session, which stays active.
    let certificate_height = successor_record
        .session
        .adaptive_dkg
        .finalized_at_height
        .max(current_height(&client).await? + 1);
    advance_to_execution_predecessor(
        &network,
        &client,
        certificate_height,
        "successor beacon-key certificate predecessor",
    )
    .await?;
    let authorizing_roster = ordered_validator_roster(&network, &client).await?;
    assert_eq!(
        authorizing_roster, incumbent_roster,
        "the preparing epoch retains the genesis committee and its signing order",
    );
    submit_parliament_instructions(
        &client,
        [lifecycle_certificate_replacing(
            &network,
            &authorizing_roster,
            ThresholdKeyLifecycleActionV1::FinalizeGlobalBeaconKey,
            (
                Some(incumbent_record.session.session_id),
                successor_record.session.session_id,
            ),
            successor_record.session.transcript_hash,
            norito::encode_canonical(&successor_record)?,
            certificate_height,
        )?],
    )
    .await?;
    observe_exact_height(
        &client,
        certificate_height,
        "successor beacon-key certificate",
    )
    .await?;
    let certified = committee_transition(&network, &client, ROTATION_TARGET_EPOCH).await?;
    assert_eq!(
        certified.pending_beacon_session.as_ref(),
        Some(&successor_record.session),
        "the incumbent-certified successor transcript is pending on chain",
    );
    for peer in network.peers() {
        let (_, horizon) =
            require_beacon_horizon(&peer.client(), "pending successor beacon session").await?;
        assert_eq!(
            horizon.active_session_id,
            Some(incumbent_record.session.session_id),
            "a certified successor stays pending until the boundary activates it",
        );
    }

    // One target owner binds the finalized successor transcript as the attempt's credentials.
    let credentials = ValidatorCommitteeCredentialsV1 {
        beacon: successor_binding,
    };
    submit_parliament_instructions(
        &target_owner_client(&network, &target_roster[0])?,
        [SetParameter::new(Parameter::Custom(
            ValidatorCommitteeOperationV1::PrepareCredentials(
                PrepareValidatorCommitteeCredentialsV1 {
                    transition_id,
                    target_epoch: preparation.target_epoch,
                    credentials,
                },
            )
            .into_custom_parameter(),
        ))],
    )
    .await?;
    let credentialed = selected_transition(
        &committee_transition(&network, &client, ROTATION_TARGET_EPOCH).await?,
        "credentialed attempt",
    )?;
    assert_eq!(credentialed.credentials, Some(credentials));
    assert!(credentialed.readiness.is_empty());

    // Every target seat proves custody of its own successor share under the separate
    // readiness domain. The fixture custody of a seat is the provider its daemon signs with;
    // the seat's daemon mode decides only whether it contributes pulse shares later.
    for (index, seat) in target_roster.iter().enumerate() {
        let validator_index = u32::try_from(index)?;
        let readiness_context = credentialed
            .readiness_context(validator_index)
            .map_err(|error| eyre!("target seat {index} readiness context: {error}"))?;
        let seat_custody = TestNetworkParliamentBeaconPartialSignerV1::try_new(
            network.network_id(),
            &target_roster,
            seat,
        )
        .wrap_err_with(|| format!("bind the fixture custody of target seat {index}"))?;
        let beacon = prove_global_threshold_beacon_seat_readiness_v1(
            &seat_custody,
            &successor_session,
            &target_generation,
            &readiness_context,
        )
        .wrap_err_with(|| format!("prove the successor share custody of target seat {index}"))?;
        submit_parliament_instructions(
            &target_owner_client(&network, seat)?,
            [SetParameter::new(Parameter::Custom(
                ValidatorCommitteeOperationV1::AdmitSeat(AdmitValidatorCommitteeSeatV1 {
                    transition_id,
                    target_epoch: preparation.target_epoch,
                    readiness: ValidatorCommitteeSeatReadinessV1 {
                        validator_index,
                        beacon,
                    },
                })
                .into_custom_parameter(),
            ))],
        )
        .await?;
    }
    let ready = selected_transition(
        &committee_transition(&network, &client, ROTATION_TARGET_EPOCH).await?,
        "ready attempt",
    )?;
    assert_eq!(ready.credentials, Some(credentials));
    assert_eq!(
        ready
            .readiness
            .iter()
            .map(|seat| seat.validator_index)
            .collect::<Vec<_>>(),
        (0..u32::try_from(VALIDATOR_COUNT)?).collect::<Vec<_>>(),
        "activation requires readiness for every target seat",
    );
    assert!(ready.outcome.is_none());
    let activating_boundary = preparation.first_height - 1;
    let cutoff_pulse_height = activating_boundary - 1;
    let prepared_height = current_height(&client).await?;
    assert!(
        prepared_height < cutoff_pulse_height,
        "the attempt must be ready before the pre-boundary pulse of its preparing epoch",
    );

    // The incumbent session still signs the pulse that selects the activating boundary.
    advance_to_height_with_carriers(
        &network,
        &client,
        cutoff_pulse_height,
        "incumbent-session pulse before the activating boundary",
    )
    .await?;
    let cutoff_pulse = agreed_mandatory_pulse(
        &network,
        cutoff_pulse_height,
        "incumbent-session pulse before the activating boundary",
    )
    .await?;
    assert_eq!(cutoff_pulse.session_id, incumbent_record.session.session_id);
    assert_eq!(
        cutoff_pulse.roster_hash,
        incumbent_record.session.roster_hash
    );
    assert_eq!(
        cutoff_pulse.transcript_hash,
        incumbent_record.session.transcript_hash
    );
    verify_pulse_on_session(
        &network,
        &client,
        &cutoff_pulse,
        &incumbent_session,
        "incumbent-session pulse before the activating boundary",
    )
    .await?;

    // The incumbent quorum's boundary certificate activates the prepared generation and the
    // successor session as one effect, and freezes the next attempt for the epoch after.
    advance_to_height_with_carriers(
        &network,
        &client,
        activating_boundary,
        "activating NPoS boundary",
    )
    .await?;
    let (_, activating) = finality::certified_block(&network, &client, activating_boundary).await?;
    let boundary = activating
        .commitment()
        .schedule
        .boundary
        .clone()
        .ok_or_else(|| eyre!("the activating boundary carries no certified epoch effect"))?;
    let activated = boundary.next.authorization;
    assert_eq!(activated.decision, ValidatorEpochDecisionV1::Activate);
    assert_eq!(activated.epoch, ROTATION_TARGET_EPOCH);
    assert_eq!(activated.first_height, preparation.first_height);
    assert_eq!(activated.last_height, preparation.last_height);
    assert_eq!(
        activated.authority_generation,
        preparation.authority_generation
    );
    assert_eq!(activated.authority_id, target_authority_id);
    assert_eq!(activated.transition_id, transition_id);
    assert_eq!(
        activated.beacon,
        BeaconEpochBindingV1::Installed(successor_binding)
    );
    assert_eq!(boundary.next.committee, preparation.committee);
    assert_eq!(
        boundary.next.leader_seed,
        global_threshold_beacon_npos_successor_seed_v1(
            &cutoff_pulse,
            activating_boundary,
            ROTATION_TARGET_EPOCH,
        ),
    );
    let next_attempt = boundary
        .preparation
        .clone()
        .ok_or_else(|| eyre!("the activating boundary froze no next committee attempt"))?;
    assert_eq!(next_attempt.target_epoch, ROTATION_TARGET_EPOCH + 1);
    assert_eq!(
        next_attempt.authority_generation,
        preparation.authority_generation + 1
    );
    let next_transition_id = next_attempt
        .transition_id()
        .map_err(|error| eyre!("next attempt identity: {error}"))?;

    let rotated_height = preparation.first_height;
    advance_to_height_with_carriers(
        &network,
        &client,
        rotated_height,
        "first height of the rotated committee",
    )
    .await?;
    await_every_validator(
        &network,
        rotated_height,
        "first height of the rotated committee",
    )
    .await?;
    for peer in network.peers() {
        let (status, horizon) =
            require_beacon_horizon(&peer.client(), "rotated committee readiness").await?;
        assert!(
            !status.is_halted(),
            "a certified committee rotation must not fail-stop a validator",
        );
        assert!(status.committed_height >= rotated_height);
        assert_eq!(
            horizon.active_session_id,
            Some(successor_record.session.session_id),
            "the boundary activates the certified successor session",
        );
        let (_, verified) =
            finality::certified_block(&network, &peer.client(), rotated_height).await?;
        let context = &verified.commitment().schedule.current;
        assert_eq!(context.authorization, activated);
        assert_eq!(context.committee, preparation.committee);
    }
    let terminal = selected_transition(
        &committee_transition(&network, &client, ROTATION_TARGET_EPOCH).await?,
        "activated attempt",
    )?;
    assert_eq!(terminal.outcome, Some(activated));

    // Across the rotation the mandatory pre-boundary pulse finalizes on the successor
    // session under the rotated committee, and it schedules the following epoch.
    let successor_pulse_height = preparation.last_height - 1;
    advance_to_height_with_carriers(
        &network,
        &client,
        successor_pulse_height,
        "successor-session mandatory pre-boundary pulse",
    )
    .await?;
    let successor_pulse = agreed_mandatory_pulse(
        &network,
        successor_pulse_height,
        "successor-session mandatory pre-boundary pulse",
    )
    .await?;
    assert_eq!(
        successor_pulse.session_id,
        successor_record.session.session_id
    );
    assert_eq!(
        successor_pulse.roster_hash,
        successor_record.session.roster_hash
    );
    assert_eq!(
        successor_pulse.transcript_hash,
        successor_record.session.transcript_hash
    );
    assert_ne!(successor_pulse.session_id, cutoff_pulse.session_id);
    verify_pulse_on_session(
        &network,
        &client,
        &successor_pulse,
        &successor_session,
        "successor-session mandatory pre-boundary pulse",
    )
    .await?;

    let retained_boundary = preparation.last_height;
    let retained_epoch = ROTATION_TARGET_EPOCH + 1;
    let retained_seed = global_threshold_beacon_npos_successor_seed_v1(
        &successor_pulse,
        retained_boundary,
        retained_epoch,
    );
    advance_to_height_with_carriers(
        &network,
        &client,
        retained_boundary + 1,
        "successor-session NPoS boundary",
    )
    .await?;
    await_every_validator(
        &network,
        retained_boundary + 1,
        "successor-session NPoS boundary",
    )
    .await?;
    for peer in network.peers() {
        let status = read_on_dedicated_thread({
            let client = peer.client().client().clone();
            move || client.get_sumeragi_status()
        })
        .await?;
        assert!(!status.is_halted());
        assert!(status.committed_height >= retained_boundary + 1);
        let (_, verified) =
            finality::certified_block(&network, &peer.client(), retained_boundary + 1).await?;
        let context = &verified.commitment().schedule.current;
        let authorization = &context.authorization;
        assert_eq!(authorization.epoch, retained_epoch);
        assert_eq!(context.leader_seed, retained_seed);
        // The uncredentialed follow-up attempt is cancelled; the rotated generation and
        // its successor session are retained.
        assert_eq!(
            authorization.decision,
            ValidatorEpochDecisionV1::RetainAndCancel
        );
        assert_eq!(authorization.transition_id, next_transition_id);
        assert_eq!(
            authorization.authority_generation,
            preparation.authority_generation
        );
        assert_eq!(authorization.authority_id, target_authority_id);
        assert_eq!(
            authorization.beacon,
            BeaconEpochBindingV1::Installed(successor_binding)
        );
        assert_eq!(context.committee, preparation.committee);
    }

    // Authenticate the whole prefix against the provisioned signed genesis: the pulse
    // heights and their sessions, each epoch's authority, beacon and decision.
    let expected_pulses = BTreeMap::from([
        (first_pulse_height, incumbent_record.session.session_id),
        (cutoff_pulse_height, incumbent_record.session.session_id),
        (successor_pulse_height, successor_record.session.session_id),
    ]);
    let genesis_roster = incumbent_roster.clone();
    let rotated_roster = target_roster.clone();
    let mut genesis_authority_id = None;
    finality::visit_certified_prefix(
        &network,
        &client,
        retained_boundary + 1,
        move |_, verified| {
            let height = verified.height();
            assert_eq!(
                verified.commitment().beacon.map(|pulse| pulse.session_id),
                expected_pulses.get(&height).copied(),
                "height {height} must carry exactly its mandatory pulse session",
            );
            let context = &verified.commitment().schedule.current;
            let authorization = &context.authorization;
            let committee = context
                .committee
                .iter()
                .map(|seat| seat.validator.clone())
                .collect::<Vec<_>>();
            assert_eq!(context.da_layout, recommended_data_availability_layout());
            assert_eq!(context.committee.len(), VALIDATOR_COUNT);
            if height < rotated_height {
                assert_eq!(committee, genesis_roster);
                assert_eq!(authorization.authority_generation, 0);
                let authority_id = *genesis_authority_id.get_or_insert(authorization.authority_id);
                assert_eq!(authorization.authority_id, authority_id);
            } else {
                assert_eq!(committee, rotated_roster);
                assert_eq!(authorization.authority_generation, 1);
                assert_eq!(authorization.authority_id, target_authority_id);
            }
            let (expected_decision, expected_transition, expected_beacon) =
                match authorization.epoch {
                    0 => (
                        ValidatorEpochDecisionV1::Genesis,
                        [0; 32],
                        BeaconEpochBindingV1::Bootstrap,
                    ),
                    1 => (
                        ValidatorEpochDecisionV1::Retain,
                        [0; 32],
                        BeaconEpochBindingV1::Installed(incumbent_binding),
                    ),
                    ROTATION_TARGET_EPOCH => (
                        ValidatorEpochDecisionV1::Activate,
                        transition_id,
                        BeaconEpochBindingV1::Installed(successor_binding),
                    ),
                    epoch => {
                        assert_eq!(epoch, retained_epoch);
                        (
                            ValidatorEpochDecisionV1::RetainAndCancel,
                            next_transition_id,
                            BeaconEpochBindingV1::Installed(successor_binding),
                        )
                    }
                };
            assert_eq!(authorization.decision, expected_decision);
            assert_eq!(authorization.transition_id, expected_transition);
            assert_eq!(authorization.beacon, expected_beacon);
            Ok(())
        },
    )
    .await
    .wrap_err("authenticate the certified committee rotation from signed genesis")?;

    network.shutdown().await;
    Ok(())
}
