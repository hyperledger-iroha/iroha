//! Current embedded-certificate finality for the real Parliament corridor.
//! Genesis comes from the harness provisioning bundle, never from an HTTP trust claim.

use super::*;
use iroha_data_model::sumeragi_finality::{
    FinalityValidator, SumeragiFinalityProof, SumeragiFinalityVerifier, VerifiedSumeragiBlock,
};
use iroha_sumeragi::availability::recommended_data_availability_layout;

/// Authenticate one exact block through the bounded contiguous native prefix.
/// Each HTTP proof remains untrusted until the existing verifier admits its committee and result.
pub(super) async fn certified_block(
    network: &iroha_test_network::Network,
    client: &Client,
    height: u64,
) -> Result<(SumeragiFinalityProof, VerifiedSumeragiBlock)> {
    const MAX_CORRIDOR_HEIGHT: u64 = 4_096;
    if !(1..=MAX_CORRIDOR_HEIGHT).contains(&height) {
        return Err(eyre!(
            "Parliament finality height exceeds the bounded corridor"
        ));
    }
    let provisioned = network.native_genesis_provisioning_bundle()?;
    let manifest: iroha_genesis::RawGenesisTransaction =
        norito::json::from_slice(&provisioned.manifest_json)?;
    let genesis = iroha_genesis::validate_prepared_genesis_bundle(
        &provisioned.signed_wire,
        &manifest,
        &provisioned.public_key,
        provisioned.block_hash,
    )?;
    if iroha_data_model::NetworkId::from_genesis_hash(genesis.expected_hash())
        != network.network_id()
        || genesis.consensus_metadata().sumeragi_context.da_layout
            != recommended_data_availability_layout()
    {
        return Err(eyre!(
            "Parliament signed genesis network or mandatory RS16 layout differs"
        ));
    }
    let validators = genesis
        .validator_pops()
        .iter()
        .map(|(public_key, proof_of_possession)| FinalityValidator {
            public_key: public_key.clone(),
            proof_of_possession: proof_of_possession.clone(),
        })
        .collect::<Vec<_>>();
    if validators.len() != VALIDATOR_COUNT {
        return Err(eyre!(
            "Parliament requires exactly four signed-genesis validators"
        ));
    }
    let expected_members = validators
        .iter()
        .map(|validator| PeerId::new(validator.public_key.clone()))
        .collect::<std::collections::BTreeSet<_>>();
    let mut verifier = SumeragiFinalityVerifier::new(
        genesis.block(),
        &network.chain_id().to_string(),
        validators,
    )?;
    let client = client.client().clone();
    let deadline = Instant::now() + OPERATION_TIMEOUT;
    read_on_dedicated_thread(move || {
        let client = client.with_request_deadline(deadline);
        let mut tip = None;
        for next in 1..=height {
            let proof = client.get_sumeragi_finality_proof(NonZeroU64::new(next).unwrap())?;
            let verified = verifier.verify(&proof)?;
            let members = proof
                .committee
                .iter()
                .map(|validator| PeerId::new(validator.public_key.clone()))
                .collect::<std::collections::BTreeSet<_>>();
            if proof.committee.len() != VALIDATOR_COUNT || members != expected_members {
                return Err(eyre!("certified Parliament committee differs from the independent four-validator roster"));
            }
            if next > 1 {
                let certificate = verified.block().commit_certificate().ok_or_else(|| {
                    eyre!("verified Parliament successor lacks its embedded certificate")
                })?;
                let qc: iroha_sumeragi::message::Qc =
                    norito::decode_canonical(certificate.commit_qc())?;
                if qc.signers.count_ones() != 3 {
                    return Err(eyre!("Parliament successor requires exactly three equal validator votes"));
                }
            }
            if Instant::now() >= deadline {
                return Err(eyre!("Parliament contiguous finality verification exceeded its deadline"));
            }
            tip = Some((proof, verified));
        }
        tip.ok_or_else(|| eyre!("Parliament finality prefix is empty"))
    })
    .await
}
