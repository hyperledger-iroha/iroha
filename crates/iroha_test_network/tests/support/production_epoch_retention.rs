//! Verify incumbent retention through four independently anchored finality chains.
//! Scheduling advances under certified authorizations without per-epoch key writes.

use super::*;
use iroha_core::release_identity::BuildIdentity;
use iroha_data_model::{
    NetworkId,
    bridge::BridgeFinalityVerifier,
    isi::kagemusha_v1::{
        BeaconEpochBindingV1, InstalledBeaconEpochBindingV1,
        KagemushaMintFinalityAuthorityGenerationV1, KagemushaMintFinalityEpochAuthorizationV1,
        KagemushaMintFinalityEpochDecisionV1,
    },
};
use iroha_model_base::peer::PeerId;
use std::{collections::BTreeMap, num::NonZeroU64};

pub(super) const EPOCH_LENGTH: u64 = 11;

/// Admit immutable source identity before any generated custody or child exists.
pub(super) fn admit_build_identity(identity: BuildIdentity) -> Result<BuildIdentity> {
    identity.release_source_commit().wrap_err(
        "retained-generation beacon fixture requires an exact compiled Git source commit before setup; use maintained Taira checks, not --stable-local-metadata",
    )?;
    Ok(identity)
}

fn require_retained_successor(
    authority: &KagemushaMintFinalityAuthorityGenerationV1,
    current: &KagemushaMintFinalityEpochAuthorizationV1,
    next: &KagemushaMintFinalityEpochAuthorizationV1,
    installed: InstalledBeaconEpochBindingV1,
    boundary_height: u64,
) -> Result<()> {
    current.validate_against_authority(authority)?;
    next.validate_against_authority(authority)?;
    next.validate_successor(current)?;
    ensure!(
        current.last_height == boundary_height
            && next.last_height
                == boundary_height
                    .checked_add(EPOCH_LENGTH)
                    .ok_or_else(|| eyre!("retained fixture epoch height overflow"))?
            && next.decision == KagemushaMintFinalityEpochDecisionV1::Retain
            && next.beacon == BeaconEpochBindingV1::Installed(installed),
        "certified boundary changed incumbent authority, installed beacon or signed epoch schedule"
    );
    Ok(())
}

pub(super) async fn verify_boundary_chain(
    prepared: &prepare::Prepared,
    clients: &[iroha::client::Client],
    installed: &beacon::FinalizedGlobalThresholdBeaconKeySessionRecordV1,
    deadline: Instant,
) -> Result<()> {
    ensure!(
        clients.len() == 4,
        "epoch retention requires all four validators"
    );
    let genesis_bytes = fs::read(prepared.genesis_directory.join("genesis.signed.nrt"))?;
    let (genesis_identity, metadata) = iroha_core::release_identity::genesis_identity(
        &genesis_bytes,
        &prepared.genesis_public_key,
    )?;
    let genesis = iroha_genesis::decode_signed_genesis(&genesis_bytes)?;
    let network = prepared.network_id;
    let genesis_hash = genesis.hash();
    ensure!(
        genesis_identity == iroha_crypto::Hash::from(network.into_genesis_hash())
            && genesis_hash == network.into_genesis_hash(),
        "epoch proof chain must bind the independently signed genesis network"
    );
    let authority = metadata
        .kagemusha_mint_finality
        .authority_generation
        .bind_network_id(network)?;
    let initial = KagemushaMintFinalityEpochAuthorizationV1::genesis(&authority, EPOCH_LENGTH)?;
    let genesis_pops = iroha_genesis::signed_genesis_validator_pops(&genesis)?
        .into_iter()
        .map(|(key, pop)| (PeerId::new(key), pop))
        .collect::<BTreeMap<_, _>>();
    ensure!(
        genesis_pops.len() == 4,
        "genesis proof authority is not four peers"
    );
    let (roster, pops): (Vec<_>, Vec<_>) = genesis_pops
        .into_iter()
        .map(|(validator, pop)| {
            (
                iroha_data_model::block::consensus_v2::ValidatorPower {
                    validator,
                    power: 1,
                },
                pop,
            )
        })
        .unzip();
    installed.validate()?;
    ensure!(
        installed.session.network_id == network
            && installed.session.committee_size == 4
            && installed.session.roster_hash
                == beacon::global_threshold_beacon_roster_hash_v1(
                    &roster
                        .iter()
                        .map(|entry| entry.validator.clone())
                        .collect::<Vec<_>>()
                ),
        "retained beacon session differs from independently selected genesis authority"
    );
    let installed_binding = InstalledBeaconEpochBindingV1 {
        session_id: installed.session.session_id,
        transcript_hash: installed.session.transcript_hash,
    };
    let height = status_height(clients, deadline).await?;
    ensure!(
        height > EPOCH_LENGTH,
        "fixture did not apply a successor epoch"
    );
    let tips = try_join_all(clients.iter().enumerate().map(|(index, client)| {
        let client = client.clone();
        let roster = roster.clone();
        let pops = pops.clone();
        let authority = authority.clone();
        let path = prepared.directory.join(format!("epoch-retention-proof-peer{index}.json"));
        iroha_test_network::read_on_dedicated_thread(move || {
            let bounded = || -> Result<iroha::client::Client> {
                let remaining = deadline.saturating_duration_since(Instant::now());
                ensure!(!remaining.is_zero(), "epoch proof chain exceeded its original audit deadline");
                let mut builder = client.to_builder();
                builder.torii_request_timeout = iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT.min(remaining);
                Ok(builder.build()?)
            };
            let (first, hash) = bounded()?.get_bridge_finality_anchor(NonZeroU64::new(1).unwrap(), network)?;
            ensure!(
                hash == genesis_hash
                    && first.block_header.hash() == genesis_hash
                    && first.finality_artifact.height_context.roster == roster
                    && first.finality_artifact.validator_set_pops == pops
                    && first.finality_artifact.height_context.kagemusha_mint_finality_authority == authority
                    && first.finality_artifact.height_context.kagemusha_mint_finality_authorization == initial,
                "epoch proof chain is not anchored to exact signed genesis authority and schedule"
            );
            let mut verifier = BridgeFinalityVerifier::with_context(network, first.finality_artifact.context_id());
            verifier.verify(&first)?;
            let mut proofs = vec![first];
            let mut expected = initial;
            for next_height in 2..=height {
                let proof = bounded()?.get_next_bridge_finality_proof(
                    NonZeroU64::new(next_height).unwrap(), &mut verifier,
                )?;
                let context = &proof.finality_artifact.height_context;
                ensure!(
                    context.kagemusha_mint_finality_authority == authority
                        && context.kagemusha_mint_finality_authorization == expected
                        && context.roster == roster
                        && proof.finality_artifact.validator_set_pops == pops,
                    "authenticated context changed the retained generation or its certified scheduling authorization"
                );
                if next_height == expected.last_height {
                    let transition = context.next_epoch_snapshot.as_ref()
                        .ok_or_else(|| eyre!("authenticated epoch boundary omitted its retention"))?;
                    ensure!(
                        transition.kagemusha_mint_finality_authority == authority
                            && transition.roster == roster
                            && transition.validator_set_pops == pops,
                        "retained boundary changed original keys, validators or proofs of possession"
                    );
                    require_retained_successor(&authority, &expected,
                        &transition.kagemusha_mint_finality_authorization, installed_binding, next_height)?;
                    expected = transition.kagemusha_mint_finality_authorization;
                }
                proofs.push(proof);
            }
            let tip = proofs.last().unwrap().block_header.hash();
            private_file(&path, &json::to_vec(&proofs)?)?;
            Ok(tip)
        })
    })).await?;
    ensure!(
        tips.len() == 4 && tips.iter().all(|tip| *tip == tips[0]),
        "retained epoch proof tips differ across validators"
    );
    Ok(())
}

#[test]
fn production_epoch_retention_requires_exact_source_identity_before_setup() -> Result<()> {
    use iroha_core::release_identity::BuildIdentityError;
    let development = BuildIdentity::from_compiled_parts(
        "fixture-test",
        Some("local-fast-build"),
        None,
        None,
        None,
        None,
    )?;
    let error =
        admit_build_identity(development).expect_err("development label is not source provenance");
    assert_eq!(
        error.downcast_ref::<BuildIdentityError>(),
        Some(&BuildIdentityError::DevelopmentSource)
    );
    assert!(error.to_string().contains("before setup"));
    // Syntax-only public revision fixture, never selected as executable evidence.
    let exact = BuildIdentity::from_compiled_parts(
        "fixture-test",
        Some("592c6e0e5adcd2ff5e0492d971bfbb179f591b53"),
        None,
        None,
        None,
        None,
    )?;
    assert_eq!(admit_build_identity(exact)?, exact);
    Ok(())
}

fn authorization_fixture() -> Result<(
    KagemushaMintFinalityAuthorityGenerationV1,
    KagemushaMintFinalityEpochAuthorizationV1,
    KagemushaMintFinalityEpochAuthorizationV1,
    InstalledBeaconEpochBindingV1,
)> {
    let mut validators = (1_u8..=4).map(|seed| {
        let key = KeyPair::from_seed(vec![seed; 32], iroha_crypto::Algorithm::BlsNormal);
        iroha_core::zk::kagemusha_v1_recursion::derive_kagemusha_mint_finality_validator_keys_v1(
            &[seed; 32], 0, PeerId::new(key.public_key().clone()),
        )
    }).collect::<std::result::Result<Vec<_>, _>>()?;
    validators.sort_by(|left, right| left.validator.cmp(&right.validator));
    let authority = KagemushaMintFinalityAuthorityGenerationV1 {
        version: iroha_data_model::isi::kagemusha_v1::KAGEMUSHA_CHAIN_VERSION_V1,
        network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::new(b"retained-generation-test"),
        )),
        generation: 0,
        validators,
    };
    let current = KagemushaMintFinalityEpochAuthorizationV1::genesis(&authority, EPOCH_LENGTH)?;
    let installed = InstalledBeaconEpochBindingV1 {
        session_id: [7; 32],
        transcript_hash: [8; 32],
    };
    let next = KagemushaMintFinalityEpochAuthorizationV1 {
        epoch: 1,
        first_height: EPOCH_LENGTH + 1,
        last_height: 2 * EPOCH_LENGTH,
        beacon: BeaconEpochBindingV1::Installed(installed),
        previous_authorization_id: current.authorization_id()?,
        decision: KagemushaMintFinalityEpochDecisionV1::Retain,
        ..current
    };
    Ok((authority, current, next, installed))
}

#[test]
fn production_epoch_retention_binds_exact_generation_beacon_and_interval() -> Result<()> {
    let (authority, current, next, installed) = authorization_fixture()?;
    require_retained_successor(&authority, &current, &next, installed, EPOCH_LENGTH)?;
    let later = KagemushaMintFinalityEpochAuthorizationV1 {
        epoch: 2,
        first_height: 2 * EPOCH_LENGTH + 1,
        last_height: 3 * EPOCH_LENGTH,
        previous_authorization_id: next.authorization_id()?,
        ..next
    };
    require_retained_successor(&authority, &next, &later, installed, 2 * EPOCH_LENGTH)?;
    assert_eq!(later.authority_generation, 0);
    assert_ne!(later.authorization_id()?, next.authorization_id()?);
    Ok(())
}

#[test]
fn production_epoch_retention_rejects_changed_generation_beacon_parent_and_schedule() -> Result<()>
{
    let (authority, current, valid, installed) = authorization_fixture()?;
    for mutation in 0..9 {
        let mut next = valid;
        match mutation {
            0 => next.authority_generation += 1,
            1 => next.authority_id = [9; 32],
            2 => next.beacon = BeaconEpochBindingV1::Bootstrap,
            3 => {
                next.beacon = BeaconEpochBindingV1::Installed(InstalledBeaconEpochBindingV1 {
                    session_id: [9; 32],
                    ..installed
                })
            }
            4 => {
                next.beacon = BeaconEpochBindingV1::Installed(InstalledBeaconEpochBindingV1 {
                    transcript_hash: [9; 32],
                    ..installed
                })
            }
            5 => next.previous_authorization_id = [9; 32],
            6 => next.first_height += 1,
            7 => next.last_height += 1,
            _ => {
                next.decision = KagemushaMintFinalityEpochDecisionV1::RetainAndCancel;
                next.transition_id = [9; 32];
            }
        }
        assert!(
            require_retained_successor(&authority, &current, &next, installed, EPOCH_LENGTH)
                .is_err(),
            "mutation {mutation}"
        );
    }
    assert!(
        require_retained_successor(&authority, &current, &valid, installed, EPOCH_LENGTH + 1)
            .is_err()
    );
    Ok(())
}
