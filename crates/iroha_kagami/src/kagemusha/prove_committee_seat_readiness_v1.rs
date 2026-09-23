//! Offline possession proofs for one exact prepared validator seat.
//!
//! Public status is structurally and cryptographically checked here. Independent chain trust
//! remains mandatory at `iroha staking committee admit-seat`; this producer grants no activation.

use std::{
    fs::File,
    io::{BufWriter, Read, Write},
    path::PathBuf,
};

use clap::Args as ClapArgs;
use color_eyre::eyre::{Result, bail, ensure, eyre};
use iroha_core::{
    beacon::{
        GlobalThresholdBeaconSessionBindingV1, RuntimeGlobalThresholdBeaconShareCustodyV1,
        authenticated_global_threshold_beacon_roster_hash_v1,
        prove_global_threshold_beacon_seat_readiness_v1,
        validate_global_threshold_beacon_session_v1,
    },
    zk::kagemusha_v1_recursion::{
        prove_kagemusha_mint_finality_seat_readiness_v1,
        verify_kagemusha_mint_finality_candidate_possession_v1,
    },
};
use iroha_data_model::{
    consensus::GlobalThresholdBeaconKeySessionV1,
    nexus::{
        AdmitValidatorCommitteeSeatV1, ValidatorCommitteeSeatReadinessV1,
        ValidatorCommitteeStatusV1, ValidatorCommitteeTransitionV1,
    },
};
use zeroize::Zeroizing;

use crate::{Outcome, secure_fs::take_seed_pipe};

const MAX_STATUS_BYTES: u64 = 16 * 1024 * 1024;

/// Exact public status and transferred secret pipes; no signing secret is an argument.
#[derive(Debug, ClapArgs)]
pub(super) struct Args {
    /// Canonical ValidatorCommitteeStatusV1 from the independently anchored CLI status command
    #[arg(long, value_name = "FILE")]
    status: PathBuf,
    /// Zero-based seat in the exact immutable target roster
    #[arg(long)]
    validator_index: u32,
    /// Transferred read pipe containing exactly 32 generation-seed bytes, then EOF
    #[arg(long, value_name = "FD", value_parser = clap::value_parser!(i32).range(3..))]
    seed_fd: i32,
    /// Distinct transferred read pipe containing three canonical 32-byte beacon-share scalars
    #[arg(long, value_name = "FD", value_parser = clap::value_parser!(i32).range(3..))]
    beacon_share_fd: i32,
}

/// Produce public readiness only after proving the exact seed and threshold-share custody.
pub(super) fn run<T: Write>(args: Args, writer: &mut BufWriter<T>) -> Outcome {
    let seed_input = take_seed_pipe(args.seed_fd)?;
    ensure!(
        args.seed_fd != args.beacon_share_fd,
        "seed and beacon share require distinct pipe descriptors"
    );
    let share_input = take_seed_pipe(args.beacon_share_fd)?;
    let status = read_status(&args.status)?;
    let transition = validate_status(&status)?;
    let record = status
        .pending_beacon_session
        .as_ref()
        .ok_or_else(|| eyre!("the exact pending beacon transcript is unavailable"))?;
    let proof = {
        let seed = read_secret::<32>(seed_input)?;
        let share_bytes = read_secret::<96>(share_input)?;
        let mut components = Zeroizing::new([[0_u8; 32]; 3]);
        for (component, bytes) in components.iter_mut().zip(share_bytes.chunks_exact(32)) {
            component.copy_from_slice(bytes);
        }
        prove(transition, record, args.validator_index, &seed, components)?
    };
    // All secret byte storage, imported share custody and descriptors are gone before output.
    writer.write_all(norito::json::to_string(&proof)?.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

fn read_status(path: &PathBuf) -> Result<ValidatorCommitteeStatusV1> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_STATUS_BYTES + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        u64::try_from(bytes.len())? <= MAX_STATUS_BYTES,
        "committee status exceeds the byte limit"
    );
    Ok(norito::json::from_slice(&bytes)?)
}

fn read_secret<const N: usize>(input: impl Read) -> Result<Zeroizing<[u8; N]>> {
    let mut bytes = Zeroizing::new(Vec::new());
    input
        .take(u64::try_from(N)? + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| eyre!("readiness secret pipe read failed"))?;
    if bytes.len() != N {
        bail!("readiness secret pipe must contain exactly {N} bytes followed by EOF");
    }
    let mut result = Zeroizing::new([0_u8; N]);
    result.copy_from_slice(&bytes);
    Ok(result)
}

fn validate_status(status: &ValidatorCommitteeStatusV1) -> Result<&ValidatorCommitteeTransitionV1> {
    let selected = status
        .selected
        .as_ref()
        .ok_or_else(|| eyre!("no frozen preparation exists"))?;
    status.latest_finality.verify()?;
    selected.selecting_finality.verify()?;
    let transition = &selected.transition;
    transition.validate().map_err(|error| eyre!(error))?;
    ensure!(
        transition.outcome.is_none(),
        "the selected attempt already has a terminal outcome"
    );
    let preparation = &transition.preparation;
    let latest = &status.latest_finality.height_context;
    let current = latest
        .next_epoch_snapshot
        .as_ref()
        .map_or(&latest.kagemusha_mint_finality_authorization, |next| {
            &next.kagemusha_mint_finality_authorization
        });
    ensure!(
        status.network_id == preparation.network_id
            && latest.network_id == status.network_id
            && status.target_epoch == preparation.target_epoch
            && latest.height >= preparation.selection_height
            && latest.height < preparation.first_height - 1,
        "status differs from the exact preparation network, target or window"
    );
    preparation
        .validate_against_preparing_authorization(current)
        .map_err(|error| eyre!(error))?;
    let selecting = &selected.selecting_finality.height_context;
    let snapshot = selecting
        .next_epoch_snapshot
        .as_ref()
        .ok_or_else(|| eyre!("selection lacks a boundary snapshot"))?;
    ensure!(
        selecting.network_id == status.network_id
            && selecting.height == preparation.selection_height
            && snapshot.committee_preparation.as_ref() == Some(preparation),
        "selection snapshot differs from the exact frozen preparation"
    );
    preparation
        .validate_against_preparing_authorization(&snapshot.kagemusha_mint_finality_authorization)
        .map_err(|error| eyre!(error))?;
    iroha_data_model::block::consensus_v2::finality::verify_validator_power_roster_pops(
        &preparation.roster,
        &preparation.validator_set_pops,
    )?;
    ensure!(
        status
            .candidate_keys
            .windows(2)
            .all(|pair| pair[0].keys.validator < pair[1].keys.validator),
        "candidate publications must be distinct and strictly ordered"
    );
    let credentials = transition
        .credentials
        .as_ref()
        .ok_or_else(|| eyre!("credentials are not prepared"))?;
    for keys in &credentials.authority.validators {
        let candidate = status
            .candidate_keys
            .iter()
            .find(|candidate| &candidate.keys == keys)
            .ok_or_else(|| eyre!("prepared keys lack an exact candidate publication"))?;
        candidate.validate().map_err(|error| eyre!(error))?;
        ensure!(
            candidate.network_id == preparation.network_id
                && candidate.generation == preparation.authority_generation,
            "candidate differs from the exact network or generation"
        );
        candidate
            .peer_signature
            .verify(keys.validator.public_key(), &candidate.authorization())?;
        verify_kagemusha_mint_finality_candidate_possession_v1(
            candidate.network_id,
            candidate.generation,
            keys,
            &candidate.possession,
        )?;
    }
    Ok(transition)
}

fn prove(
    transition: &ValidatorCommitteeTransitionV1,
    record: &GlobalThresholdBeaconKeySessionV1,
    validator_index: u32,
    seed: &[u8; 32],
    components: Zeroizing<[[u8; 32]; 3]>,
) -> Result<AdmitValidatorCommitteeSeatV1> {
    transition.validate().map_err(|error| eyre!(error))?;
    ensure!(
        transition.outcome.is_none(),
        "the selected attempt already has a terminal outcome"
    );
    ensure!(
        !transition
            .readiness
            .iter()
            .any(|seat| seat.validator_index == validator_index),
        "this seat already has recorded readiness"
    );
    let context = transition
        .readiness_context(validator_index)
        .map_err(|error| eyre!(error))?;
    let preparation = &transition.preparation;
    let credentials = transition
        .credentials
        .as_ref()
        .ok_or_else(|| eyre!("credentials are not prepared"))?;
    ensure!(
        record.network_id == preparation.network_id
            && record.session_id
                == preparation
                    .beacon_session_id()
                    .map_err(|error| eyre!(error))?
            && record.session_id == credentials.beacon.session_id
            && record.transcript_hash == credentials.beacon.transcript_hash
            && record.adaptive_dkg.session.start_height > preparation.selection_height
            && record.adaptive_dkg.finalized_at_height < preparation.first_height - 1,
        "beacon transcript differs from the exact frozen preparation"
    );
    let peers = preparation
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    let binding = GlobalThresholdBeaconSessionBindingV1 {
        network_id: record.network_id,
        session_id: record.session_id,
        roster_hash: authenticated_global_threshold_beacon_roster_hash_v1(record, &peers)?,
        transcript_hash: record.transcript_hash,
    };
    let session = validate_global_threshold_beacon_session_v1(record.clone(), &binding)?;
    let pasta =
        prove_kagemusha_mint_finality_seat_readiness_v1(seed, &credentials.authority, &context)
            .map_err(|_| eyre!("generation seed does not prove exact prepared-seat custody"))?;
    let custody = RuntimeGlobalThresholdBeaconShareCustodyV1::new();
    let signer_index = u16::try_from(validator_index)?
        .checked_add(1)
        .ok_or_else(|| eyre!("seat index overflow"))?;
    custody
        .import_components(record.clone(), &binding, signer_index, components)
        .map_err(|_| eyre!("beacon components do not prove exact prepared-seat custody"))?;
    let beacon = prove_global_threshold_beacon_seat_readiness_v1(
        &custody,
        &session,
        &credentials.authority,
        &context,
    )?;
    Ok(AdmitValidatorCommitteeSeatV1 {
        transition_id: preparation.transition_id().map_err(|error| eyre!(error))?,
        target_epoch: preparation.target_epoch,
        readiness: ValidatorCommitteeSeatReadinessV1 {
            validator_index,
            pasta,
            beacon,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use iroha_core::{
        beacon::{
            AdaptiveGlobalThresholdBeaconDkgCryptoV1, GlobalThresholdBeaconDkgStateV1,
            global_threshold_beacon_roster_hash_v1,
            verify_global_threshold_beacon_seat_readiness_v1,
        },
        zk::kagemusha_v1_recursion::{
            derive_kagemusha_mint_finality_validator_keys_v1,
            verify_kagemusha_mint_finality_seat_readiness_v1,
        },
    };
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        NetworkId,
        block::consensus_v2::ValidatorPower,
        consensus::GlobalThresholdBeaconDkgSessionV1,
        isi::kagemusha_v1::{
            InstalledBeaconEpochBindingV1, KagemushaMintFinalityAuthorityGenerationV1,
        },
        nexus::{ValidatorCommitteeCredentialsV1, ValidatorCommitteePreparationV1},
    };
    use iroha_model_base::peer::PeerId;
    use std::io::Cursor;

    fn fixture() -> (
        ValidatorCommitteeTransitionV1,
        GlobalThresholdBeaconKeySessionV1,
        Zeroizing<[[u8; 32]; 3]>,
    ) {
        let network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"seat-proof-command",
        )));
        let mut peers = (1..=4_u8)
            .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
            .collect::<Vec<_>>();
        peers.sort_by(|a, b| a.public_key().cmp(b.public_key()));
        let roster = peers
            .iter()
            .map(|pair| ValidatorPower {
                validator: PeerId::new(pair.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let preparation = ValidatorCommitteePreparationV1 {
            version: 1,
            network_id,
            selection_epoch: 0,
            selection_height: 100,
            selection_anchor: HashOf::from_untyped_unchecked(Hash::new(b"height99")),
            target_epoch: 2,
            first_height: 201,
            last_height: 300,
            authority_generation: 1,
            preparing_authorization_id: [9; 32],
            election_seed: [8; 32],
            roster: roster.clone(),
            validator_set_pops: peers
                .iter()
                .map(|pair| iroha_crypto::bls_normal_pop_prove(pair.private_key()).unwrap())
                .collect(),
        };
        let dkg = GlobalThresholdBeaconDkgSessionV1 {
            version: 1,
            network_id,
            session_id: preparation.beacon_session_id().unwrap(),
            attempt_id: preparation.transition_id().unwrap(),
            authority_generation: preparation.authority_generation,
            roster_hash: global_threshold_beacon_roster_hash_v1(
                &roster
                    .iter()
                    .map(|v| v.validator.clone())
                    .collect::<Vec<_>>(),
            ),
            committee_size: 4,
            threshold: 2,
            start_height: 101,
            commitments_end_height: 120,
            deliveries_end_height: 140,
            acceptances_end_height: 160,
        };
        let (record, components) =
            iroha_core::beacon::complete_beacon_dkg_fixture_for_exact_session_v1(dkg, 1);
        let authority = KagemushaMintFinalityAuthorityGenerationV1 {
            version: 1,
            network_id,
            generation: 1,
            validators: roster
                .iter()
                .enumerate()
                .map(|(index, v)| {
                    derive_kagemusha_mint_finality_validator_keys_v1(
                        &[u8::try_from(index + 11).unwrap(); 32],
                        1,
                        v.validator.clone(),
                    )
                    .unwrap()
                })
                .collect(),
        };
        let transition = ValidatorCommitteeTransitionV1 {
            preparation,
            credentials: Some(ValidatorCommitteeCredentialsV1 {
                authority,
                beacon: InstalledBeaconEpochBindingV1 {
                    session_id: record.session_id,
                    transcript_hash: record.transcript_hash,
                },
            }),
            readiness: Vec::new(),
            outcome: None,
        };
        (transition, record, components)
    }

    #[test]
    fn readiness_producer_requires_actual_seed_share_and_exact_attempt() {
        let (transition, record, components) = fixture();
        let proof = prove(
            &transition,
            &record,
            0,
            &[11; 32],
            Zeroizing::new(*components),
        )
        .unwrap();
        assert_eq!(
            proof.transition_id,
            transition.preparation.transition_id().unwrap()
        );
        assert_eq!(proof.target_epoch, 2);
        let context = transition.readiness_context(0).unwrap();
        let authority = &transition.credentials.as_ref().unwrap().authority;
        let binding = GlobalThresholdBeaconSessionBindingV1 {
            network_id: record.network_id,
            session_id: record.session_id,
            roster_hash: record.roster_hash,
            transcript_hash: record.transcript_hash,
        };
        let session =
            validate_global_threshold_beacon_session_v1(record.clone(), &binding).unwrap();
        verify_kagemusha_mint_finality_seat_readiness_v1(
            authority,
            &context,
            &proof.readiness.pasta,
        )
        .unwrap();
        verify_global_threshold_beacon_seat_readiness_v1(
            &session,
            authority,
            &context,
            &proof.readiness.beacon,
        )
        .unwrap();
        assert!(
            prove(
                &transition,
                &record,
                0,
                &[12; 32],
                Zeroizing::new(*components)
            )
            .is_err()
        );
        assert!(
            prove(
                &transition,
                &record,
                1,
                &[12; 32],
                Zeroizing::new(*components)
            )
            .is_err()
        );
        assert!(
            prove(
                &transition,
                &record,
                0,
                &[11; 32],
                Zeroizing::new([[0xFF; 32]; 3])
            )
            .is_err()
        );
        let mut changed = transition.clone();
        changed.preparation.election_seed[0] ^= 1;
        assert!(prove(&changed, &record, 0, &[11; 32], Zeroizing::new(*components)).is_err());
        changed = transition.clone();
        changed.readiness.push(proof.readiness.clone());
        assert!(prove(&changed, &record, 0, &[11; 32], Zeroizing::new(*components)).is_err());
        let bytes = norito::json::to_vec(&proof).unwrap();
        assert_eq!(
            norito::json::from_slice::<AdmitValidatorCommitteeSeatV1>(&bytes).unwrap(),
            proof
        );
    }

    #[test]
    fn secret_reader_is_exact_bounded_and_redacts_io_errors() {
        assert_eq!(*read_secret::<32>(Cursor::new([7; 32])).unwrap(), [7; 32]);
        for length in [0, 31, 33, 1024] {
            let mut input = Cursor::new(vec![7; length]);
            assert!(read_secret::<32>(&mut input).is_err());
            assert!(input.position() <= 33);
        }
        struct Failed;
        impl Read for Failed {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("secret-marker"))
            }
        }
        assert_eq!(
            read_secret::<96>(Failed).unwrap_err().to_string(),
            "readiness secret pipe read failed"
        );
    }

    #[test]
    fn readiness_parser_requires_exact_public_status_and_two_numeric_pipes() {
        #[derive(Parser)]
        struct Cli {
            #[command(flatten)]
            args: Args,
        }
        assert!(
            Cli::try_parse_from([
                "prove",
                "--status",
                "status.json",
                "--validator-index",
                "0",
                "--seed-fd",
                "3",
                "--beacon-share-fd",
                "4"
            ])
            .is_ok()
        );
        assert!(
            Cli::try_parse_from([
                "prove",
                "--status",
                "status.json",
                "--validator-index",
                "0",
                "--seed-fd",
                "3"
            ])
            .is_err()
        );
        for value in ["2", "-1", "/run/share"] {
            assert!(
                Cli::try_parse_from([
                    "prove",
                    "--status",
                    "status.json",
                    "--validator-index",
                    "0",
                    "--seed-fd",
                    "3",
                    "--beacon-share-fd",
                    value
                ])
                .is_err()
            );
        }
    }
}
