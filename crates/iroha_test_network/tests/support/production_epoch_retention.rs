//! Verify current certificates, unchanged committee/schedule and exact threshold pulses.
//! The current protocol carries no retired per-epoch KAGEMUSHA authorization; this gate
//! claims only evidence present in its independently anchored contiguous current proofs.
use super::*;
use iroha_core::release_identity::BuildIdentity;
use iroha_data_model::sumeragi_finality::{
    FinalityValidator, SumeragiFinalityProof, SumeragiFinalityVerifier, VerifiedSumeragiBlock,
};
use std::num::NonZeroU64;

pub(super) const EPOCH_LENGTH: u64 = 7;

pub(super) fn admit_build_identity(identity: BuildIdentity) -> Result<BuildIdentity> {
    identity.release_source_commit().wrap_err(
        "current beacon fixture requires an exact compiled Git source commit before setup; use maintained Taira checks, not --stable-local-metadata",
    )?;
    Ok(identity)
}

fn verify_retained_schedule(
    proof: &SumeragiFinalityProof,
    verified: &VerifiedSumeragiBlock,
    validators: &[FinalityValidator],
    epoch_length: u64,
) -> Result<()> {
    ensure!(
        proof.committee == validators
            && verified.commitment().next_params.epoch_length_blocks == epoch_length,
        "current certified committee or signed epoch schedule changed"
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
        "current proof audit requires four validators"
    );
    let wire = fs::read(prepared.genesis_directory.join("genesis.signed.nrt"))?;
    let manifest = iroha_genesis::RawGenesisTransaction::from_path(
        prepared.genesis_directory.join("genesis.json"),
    )?;
    let genesis = iroha_genesis::validate_prepared_genesis_bundle(
        &wire,
        &manifest,
        &prepared.genesis_public_key,
        prepared.network_id.into_genesis_hash(),
    )?;
    let validators = genesis
        .validator_pops()
        .iter()
        .map(|(public_key, proof_of_possession)| FinalityValidator {
            public_key: public_key.clone(),
            proof_of_possession: proof_of_possession.clone(),
        })
        .collect::<Vec<_>>();
    let roster = validators
        .iter()
        .map(|validator| iroha_model_base::peer::PeerId::new(validator.public_key.clone()))
        .collect::<Vec<_>>();
    installed.validate()?;
    let roster_hash = beacon::global_threshold_beacon_roster_hash_v1(&roster);
    ensure!(
        installed.session.roster_hash == roster_hash
            && usize::from(installed.session.committee_size) == roster.len(),
        "installed threshold session differs from the exact signed-genesis committee"
    );
    let session = beacon::validate_global_threshold_beacon_session_v1(
        installed.session.clone(),
        &beacon::GlobalThresholdBeaconSessionBindingV1 {
            network_id: prepared.network_id,
            session_id: installed.session.session_id,
            roster_hash,
            transcript_hash: installed.session.transcript_hash,
        },
    )?;
    let height = status_height(clients, deadline).await?;
    ensure!(
        height > EPOCH_LENGTH,
        "real fixture work has not crossed the scheduling boundary"
    );
    let tips = try_join_all(clients.iter().enumerate().map(|(index, client)| {
        let client = client.clone();
        let validators = validators.clone();
        let trusted = genesis.block().clone();
        let chain_id = manifest.chain_id().to_string();
        let session = session.clone();
        let path = prepared
            .directory
            .join(format!("current-boundary-proof-peer{index}.json"));
        iroha_test_network::read_on_dedicated_thread(move || {
            let mut verifier =
                SumeragiFinalityVerifier::new(&trusted, &chain_id, validators.clone())?;
            let mut proofs = Vec::new();
            let mut parent = None;
            let mut tip = None;
            for next_height in 1..=height {
                let remaining = deadline.saturating_duration_since(Instant::now());
                ensure!(
                    !remaining.is_zero(),
                    "current proof audit exceeded its original deadline"
                );
                let mut builder = client.to_builder();
                builder.torii_request_timeout =
                    iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT.min(remaining);
                let proof = builder
                    .build()?
                    .get_sumeragi_finality_proof(NonZeroU64::new(next_height).unwrap())?;
                let verified = verifier.verify(&proof)?;
                verify_retained_schedule(&proof, &verified, &validators, EPOCH_LENGTH)?;
                if next_height > 1 {
                    ensure!(
                        verified.block().network_entrypoint_count() > 0,
                        "current proof chain contains an empty block"
                    );
                    if (next_height + 1) % EPOCH_LENGTH == 0 {
                        let pulse = verified
                            .block()
                            .global_beacon_pulse()
                            .ok_or_else(|| eyre!("required current threshold pulse missing"))?;
                        beacon::verify_finalized_global_threshold_beacon_pulse_v1(
                            &session,
                            pulse,
                            iroha_data_model::consensus::GlobalThresholdBeaconChainAnchorV1 {
                                height: next_height - 1,
                                block_hash: parent
                                    .ok_or_else(|| eyre!("current pulse parent missing"))?,
                            },
                        )?;
                    }
                }
                parent = Some(verified.header().hash());
                tip = Some(iroha_crypto::Hash::new(verified.canonical_executed_wire()?));
                proofs.push(proof);
            }
            private_file(&path, &json::to_vec(&proofs)?)?;
            tip.ok_or_else(|| eyre!("current proof prefix is empty"))
        })
    }))
    .await?;
    ensure!(
        tips.len() == 4 && tips.iter().all(|tip| *tip == tips[0]),
        "current certified executed tips differ across validators"
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

fn certified_schedule_fixture() -> Result<(SumeragiFinalityProof, VerifiedSumeragiBlock)> {
    use iroha_core::{
        state::World,
        sumeragi::{
            finality::build_proof,
            test_chain::{CertifiedTestChain, TestChainConfig},
        },
    };
    let config = TestChainConfig::new(World::default(), 10_000);
    let chain_id = config.chain_id.to_string();
    let mut chain = CertifiedTestChain::start(config).map_err(|failure| eyre!(failure.error))?;
    // This inserts a signed clock Log transaction, never an empty block.
    chain.commit_at(20_000, Vec::new());
    let validators = chain
        .validators()
        .iter()
        .map(|(peer, pop)| FinalityValidator {
            public_key: peer.public_key().clone(),
            proof_of_possession: pop.clone(),
        })
        .collect::<Vec<_>>();
    let mut verifier = SumeragiFinalityVerifier::new(chain.genesis(), &chain_id, validators)?;
    verifier.verify(&build_proof(&chain.state().view(), 1)?)?;
    let proof = build_proof(&chain.state().view(), 2)?;
    let verified = verifier.verify(&proof)?;
    ensure!(
        verified.block().network_entrypoint_count() > 0,
        "schedule fixture must execute real work"
    );
    Ok((proof, verified))
}

#[test]
fn production_current_boundary_binds_exact_certified_committee_and_schedule() -> Result<()> {
    let (proof, verified) = certified_schedule_fixture()?;
    verify_retained_schedule(
        &proof,
        &verified,
        &proof.committee,
        verified.commitment().next_params.epoch_length_blocks,
    )?;
    ensure!(
        verified.height() == 2 && proof.committee.len() == 4,
        "current boundary fixture must certify real four-validator work"
    );
    Ok(())
}

#[test]
fn production_current_boundary_rejects_changed_committee_or_schedule() -> Result<()> {
    let (proof, verified) = certified_schedule_fixture()?;
    let epoch = verified.commitment().next_params.epoch_length_blocks;
    let mut other_roster = proof.committee.clone();
    other_roster.swap(0, 1);
    assert!(verify_retained_schedule(&proof, &verified, &other_roster, epoch).is_err());
    assert!(verify_retained_schedule(&proof, &verified, &proof.committee, epoch + 1).is_err());
    Ok(())
}
