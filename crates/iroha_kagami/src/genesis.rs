//! Genesis authority provisioning, validation, and signing commands.
use crate::{Outcome, RunArgs};
use clap::Subcommand;
use color_eyre::eyre::eyre;
use iroha_genesis::RawGenesisTransaction;
use std::io::{BufWriter, Write};

pub fn ensure_kagemusha_mint_finality_generation_zero_authority_matches_topology(
    manifest: &RawGenesisTransaction,
    topology: &[iroha_model_base::peer::PeerId],
) -> color_eyre::Result<()> {
    iroha_core_zk::kagemusha_v1_recursion::validate_kagemusha_mint_finality_genesis_parameter_keys_v1(
        manifest.kagemusha_mint_finality_genesis_parameters(),
    )
    .map_err(|error| eyre!("invalid KAGEMUSHA mint-finality public parameters: {error}"))?;
    let mut expected = topology.to_vec();
    expected.sort();
    let parameters = manifest.kagemusha_mint_finality_genesis_parameters();
    let current = parameters
        .authority_generation
        .validators
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    if current != expected {
        return Err(eyre!(
            "signed KAGEMUSHA mint-finality generation-zero authority does not match the exact genesis topology"
        ));
    }
    Ok(())
}

pub fn ensure_kagemusha_mint_finality_schedule_matches_consensus(
    manifest: &RawGenesisTransaction,
) -> color_eyre::Result<()> {
    manifest.validate_mode_specific_consensus_parameters()
}

#[cfg(test)]
fn complete_test_genesis_builder(
    builder: iroha_genesis::GenesisBuilder,
) -> iroha_genesis::GenesisBuilder {
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_model_base::peer::PeerId;

    let validators = (0_u8..4)
        .map(|index| {
            PeerId::new(
                KeyPair::try_from_seed(vec![0x20_u8.wrapping_add(index); 32], Algorithm::BlsNormal)
                    .expect("derive deterministic Kagami test validator")
                    .public_key()
                    .clone(),
            )
        })
        .collect::<Vec<_>>();
    complete_test_genesis_builder_for_peers(builder, validators)
}

#[cfg(test)]
/// Complete fixture context and mint-finality authority for the exact supplied peers.
pub fn complete_test_genesis_builder_for_peers(
    builder: iroha_genesis::GenesisBuilder,
    mut validators: Vec<iroha_model_base::peer::PeerId>,
) -> iroha_genesis::GenesisBuilder {
    use iroha_data_model::{
        block::consensus::SumeragiGenesisContextParameters,
        isi::kagemusha_v1::{
            KAGEMUSHA_CHAIN_VERSION_V1, KagemushaMintFinalityAuthorityGenerationTemplateV1,
            KagemushaMintFinalityGenesisParametersV1,
        },
    };

    validators.sort();
    let validators = validators
        .into_iter()
        .enumerate()
        .map(|(index, validator)| {
            iroha_core_zk::kagemusha_v1_recursion::derive_kagemusha_mint_finality_validator_keys_v1(
                &[0xA0_u8.wrapping_add(u8::try_from(index).expect("small test roster")); 32],
                0,
                validator,
            )
            .expect("derive deterministic Kagami test Pasta keys")
        })
        .collect();
    builder
        .with_sumeragi_context_parameters(SumeragiGenesisContextParameters::recommended())
        .with_kagemusha_mint_finality_genesis_parameters(KagemushaMintFinalityGenesisParametersV1 {
            authority_generation: KagemushaMintFinalityAuthorityGenerationTemplateV1 {
                version: KAGEMUSHA_CHAIN_VERSION_V1,
                generation: 0,
                validators,
            },
        })
}

#[cfg(test)]
/// Complete test genesis builders with the required first-release authority.
pub trait CompleteTestGenesisBuilder {
    /// Install required context and authority for a deterministic four-validator fixture.
    fn complete_for_test(self) -> Self;
    /// Install the supplied topology and its matching context and mint-finality authority.
    fn set_topology_for_test(self, topology: Vec<iroha_genesis::GenesisTopologyEntry>) -> Self;
}

#[cfg(test)]
impl CompleteTestGenesisBuilder for iroha_genesis::GenesisBuilder {
    fn complete_for_test(self) -> Self {
        complete_test_genesis_builder(self)
    }

    fn set_topology_for_test(self, topology: Vec<iroha_genesis::GenesisTopologyEntry>) -> Self {
        let validators = topology.iter().map(|entry| entry.peer.clone()).collect();
        complete_test_genesis_builder_for_peers(self.set_topology(topology), validators)
    }
}

#[cfg(test)]
mod authority_tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::parameter::{
        Parameter,
        system::{SumeragiConsensusMode, SumeragiNposParameters},
    };
    use iroha_genesis::GenesisBuilder;
    use iroha_model_base::chain::ChainId;
    use iroha_model_base::peer::PeerId;
    use std::{num::NonZeroU64, path::PathBuf};

    fn test_peers(seed_prefix: u8) -> Vec<PeerId> {
        let mut peers = (0_u8..4)
            .map(|index| {
                PeerId::new(
                    KeyPair::try_from_seed(
                        vec![seed_prefix.wrapping_add(index); 32],
                        Algorithm::BlsNormal,
                    )
                    .expect("derive deterministic authority test validator")
                    .public_key()
                    .clone(),
                )
            })
            .collect::<Vec<_>>();
        peers.sort();
        peers
    }

    #[test]
    fn genesis_topology_checks_generation_zero_independently_of_epoch_length() {
        let current = test_peers(0x30);
        for epoch_length in [3, 3_600] {
            let npos = SumeragiNposParameters {
                epoch_length_blocks: NonZeroU64::new(epoch_length).unwrap(),
                evidence_horizon_blocks: 1,
                slashing_delay_blocks: 1,
                ..SumeragiNposParameters::default()
            };
            let manifest = complete_test_genesis_builder_for_peers(
                GenesisBuilder::new_without_executor(
                    ChainId::from("generation-zero-authority"),
                    PathBuf::from("."),
                )
                .append_parameter(Parameter::Custom(npos.into_custom_parameter()))
                .append_parameter(Parameter::Sumeragi(
                    iroha_data_model::parameter::system::SumeragiParameter::EpochLengthBlocks(
                        NonZeroU64::new(epoch_length).unwrap(),
                    ),
                )),
                current.clone(),
            )
            .build_raw()
            .expect("complete generation-zero fixture")
            .with_consensus_mode(SumeragiConsensusMode::Npos);
            ensure_kagemusha_mint_finality_generation_zero_authority_matches_topology(
                &manifest, &current,
            )
            .expect("generation-zero authority matches the genesis topology");
            ensure_kagemusha_mint_finality_schedule_matches_consensus(&manifest)
                .expect("scheduling epochs do not create extra key generations");
        }
    }

    #[test]
    fn genesis_topology_check_rejects_another_authority() {
        let current = test_peers(0x70);
        let manifest = complete_test_genesis_builder_for_peers(
            GenesisBuilder::new_without_executor(
                ChainId::from("wrong-genesis-authority"),
                PathBuf::from("."),
            ),
            current,
        )
        .build_raw()
        .expect("complete authority fixture");
        let _ = ensure_kagemusha_mint_finality_generation_zero_authority_matches_topology(
            &manifest,
            &test_peers(0x90),
        )
        .expect_err("another committee cannot replace the genesis authority");
    }

    #[test]
    fn generation_zero_topology_requires_the_exact_initial_authority() {
        let current = test_peers(0x30);
        let manifest = complete_test_genesis_builder_for_peers(
            GenesisBuilder::new_without_executor(
                ChainId::from("initial-authority"),
                PathBuf::from("."),
            ),
            current.clone(),
        )
        .build_raw()
        .expect("complete generation-zero manifest");
        ensure_kagemusha_mint_finality_generation_zero_authority_matches_topology(
            &manifest, &current,
        )
        .expect("the exact initial committee owns the generation-zero keys");
        assert!(
            ensure_kagemusha_mint_finality_generation_zero_authority_matches_topology(
                &manifest,
                &test_peers(0x50)
            )
            .is_err()
        );
        let mut parameters = manifest
            .kagemusha_mint_finality_genesis_parameters()
            .clone();
        parameters.authority_generation.generation = 1;
        let invalid = manifest.with_kagemusha_mint_finality_genesis_parameters(parameters);
        assert!(
            ensure_kagemusha_mint_finality_generation_zero_authority_matches_topology(
                &invalid, &current
            )
            .is_err()
        );
    }

    #[test]
    fn genesis_epoch_requires_room_for_committed_beacon_authority() {
        for length in [1, 2, 3] {
            let npos_parameters = SumeragiNposParameters {
                epoch_length_blocks: NonZeroU64::new(length).unwrap(),
                evidence_horizon_blocks: length,
                slashing_delay_blocks: length,
                ..SumeragiNposParameters::default()
            };
            let manifest = complete_test_genesis_builder_for_peers(
                GenesisBuilder::new_without_executor(
                    ChainId::from("initial-beacon-window"),
                    PathBuf::from("."),
                )
                .append_parameter(Parameter::Custom(npos_parameters.into_custom_parameter()))
                .append_parameter(Parameter::Sumeragi(
                    iroha_data_model::parameter::system::SumeragiParameter::EpochLengthBlocks(
                        NonZeroU64::new(length).unwrap(),
                    ),
                )),
                test_peers(0x70),
            )
            .build_raw()
            .expect("complete authority manifest")
            .with_consensus_mode(SumeragiConsensusMode::Npos);
            let result = ensure_kagemusha_mint_finality_schedule_matches_consensus(&manifest);
            if length < 3 {
                assert!(
                    result
                        .expect_err("missing committed beacon anchor")
                        .to_string()
                        .contains("epoch_length_blocks >= 3")
                );
            } else {
                result.expect(
                    "initial authority does not require a precomputed successor generation",
                );
            }
        }
    }
}
mod embed_pop;
mod generate;
mod materialize;
mod normalize;
mod npos;
mod prepared;
pub mod profile;
mod sign;
#[cfg(test)]
pub use iroha_deploy::genesis::staging::bind_and_sign_staged_sumeragi_context;
pub use iroha_deploy::genesis::staging::staged_signed_sumeragi_context_hashes;
#[cfg(test)]
pub use sign::{prepared_native_test_chain, tests::native_genesis_fixture_with_instructions};
mod validate;
#[cfg(test)]
pub use iroha_deploy::genesis::generate_default;
pub use iroha_deploy::genesis::{ConsensusPolicy, validate_consensus_mode};
pub use npos::ensure_npos_parameters;
pub use profile::{
    GenesisProfile, PUBLIC_NEXUS_CHAIN_ID, PUBLIC_XOR_ALIAS, ProfileDefaults,
    TAIRA_XOR_ASSET_DEFINITION_ID, parse_vrf_seed_hex, profile_defaults, profile_requires_npos,
    profile_uses_public_xor, reject_retired_public_chain_id, resolve_vrf_seed,
};
fn require_native_wire_protocol(manifest: &RawGenesisTransaction) -> color_eyre::Result<()> {
    let expected = u32::from(iroha_data_model::sumeragi::PROTOCOL_VERSION);
    if manifest.wire_protocol_version() != expected {
        return Err(eyre!(
            "fresh genesis must advertise wire_protocol_version = {expected}; legacy plural and downgrade protocol shapes are prohibited"
        ));
    }
    Ok(())
}
#[derive(Subcommand)]
pub enum Args {
    Sign(sign::Args),
    Generate(generate::Args),
    /// Materialize an incomplete source template with operator-provisioned public authority
    Materialize(materialize::Args),
    /// Validate a genesis JSON file and report invalid identifiers
    Validate(validate::Args),
    /// Verify one exact bound-manifest/signed-genesis/signer/hash bundle
    ValidatePrepared(prepared::Args),
    /// Embed one or more PoPs into a genesis JSON manifest (inline `topology` entries carrying `pop_hex`)
    EmbedPop(embed_pop::Args),
    /// Expand a genesis manifest and show the final ordered transactions
    Normalize(normalize::Args),
}
impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        match self {
            Args::Sign(args) => args.run(writer),
            Args::Generate(args) => args.run(writer),
            Args::Materialize(args) => args.run(writer),
            Args::Validate(args) => args.run(writer),
            Args::ValidatePrepared(args) => args.run(writer),
            Args::EmbedPop(args) => args.run(writer),
            Args::Normalize(args) => args.run(writer),
        }
    }
}
