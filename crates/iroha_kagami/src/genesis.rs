use crate::{Outcome, RunArgs};
use clap::Subcommand;
use color_eyre::eyre::eyre;
use iroha_data_model::{nexus::DataSpaceId, prelude::RoleId};
use iroha_genesis::RawGenesisTransaction;
use std::io::{BufWriter, Write};

pub(super) fn ensure_kagemusha_mint_finality_epoch_zero_authority_matches_topology(
    manifest: &RawGenesisTransaction,
    topology: &[iroha_data_model::peer::PeerId],
) -> color_eyre::Result<()> {
    iroha_core::zk::kagemusha_v1_recursion::validate_kagemusha_mint_finality_genesis_parameter_keys_v1(
        manifest.kagemusha_mint_finality_genesis_parameters(),
    )
    .map_err(|error| eyre!("invalid KAGEMUSHA mint-finality public parameters: {error}"))?;
    let mut expected = topology.to_vec();
    expected.sort();
    let parameters = manifest.kagemusha_mint_finality_genesis_parameters();
    let current = parameters
        .epoch_roster
        .validators
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    if current != expected {
        return Err(eyre!(
            "signed KAGEMUSHA mint-finality epoch-zero authority does not match the exact genesis topology"
        ));
    }
    Ok(())
}

pub(super) fn ensure_kagemusha_mint_finality_schedule_matches_consensus(
    manifest: &RawGenesisTransaction,
) -> color_eyre::Result<()> {
    manifest.validate_mode_specific_consensus_parameters()
}

#[cfg(test)]
fn complete_test_genesis_builder(
    builder: iroha_genesis::GenesisBuilder,
) -> iroha_genesis::GenesisBuilder {
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::peer::PeerId;

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
fn complete_test_genesis_builder_for_peers(
    builder: iroha_genesis::GenesisBuilder,
    mut validators: Vec<iroha_data_model::peer::PeerId>,
) -> iroha_genesis::GenesisBuilder {
    use iroha_data_model::{
        block::consensus_v2::SumeragiV2GenesisContextParameters,
        isi::kagemusha_v1::{
            KAGEMUSHA_CHAIN_VERSION_V1, KagemushaMintFinalityEpochRosterTemplateV1,
            KagemushaMintFinalityGenesisParametersV1,
        },
    };

    validators.sort();
    let validators = validators
        .into_iter()
        .enumerate()
        .map(|(index, validator)| {
            iroha_core::zk::kagemusha_v1_recursion::derive_kagemusha_mint_finality_validator_keys_v1(
                &[0xA0_u8.wrapping_add(u8::try_from(index).expect("small test roster")); 32],
                0,
                validator,
            )
            .expect("derive deterministic Kagami test Pasta keys")
        })
        .collect();
    builder
        .with_sumeragi_v2_context_parameters(SumeragiV2GenesisContextParameters::recommended())
        .with_kagemusha_mint_finality_genesis_parameters(KagemushaMintFinalityGenesisParametersV1 {
            epoch_roster: KagemushaMintFinalityEpochRosterTemplateV1 {
                version: KAGEMUSHA_CHAIN_VERSION_V1,
                epoch: 0,
                validators,
            },
            next_epoch_roster: None,
        })
}

#[cfg(test)]
trait CompleteTestGenesisBuilder {
    fn complete_for_test(self) -> Self;
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
    use iroha_data_model::{
        ChainId,
        isi::kagemusha_v1::{
            KAGEMUSHA_CHAIN_VERSION_V1, KagemushaMintFinalityEpochRosterTemplateV1,
        },
        parameter::{
            Parameter,
            system::{SumeragiConsensusMode, SumeragiNposParameters},
        },
        peer::PeerId,
    };
    use iroha_genesis::GenesisBuilder;
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

    fn next_epoch_roster(validators: Vec<PeerId>) -> KagemushaMintFinalityEpochRosterTemplateV1 {
        KagemushaMintFinalityEpochRosterTemplateV1 {
            version: KAGEMUSHA_CHAIN_VERSION_V1,
            epoch: 1,
            validators: validators
                .into_iter()
                .enumerate()
                .map(|(index, validator)| {
                    iroha_core::zk::kagemusha_v1_recursion::derive_kagemusha_mint_finality_validator_keys_v1(
                        &[0xC0_u8.wrapping_add(u8::try_from(index).expect("small test roster")); 32],
                        1,
                        validator,
                    )
                    .expect("derive deterministic epoch-one Pasta keys")
                })
                .collect(),
        }
    }

    #[test]
    fn epoch_zero_topology_check_allows_distinct_epoch_one_authority() {
        let current = test_peers(0x30);
        let next = test_peers(0x50);
        let mut npos_parameters = SumeragiNposParameters::default();
        npos_parameters.epoch_length_blocks = NonZeroU64::new(1).expect("non-zero epoch length");
        npos_parameters.evidence_horizon_blocks = 1;
        npos_parameters.slashing_delay_blocks = 1;
        let manifest = complete_test_genesis_builder_for_peers(
            GenesisBuilder::new_without_executor(
                ChainId::from("epoch-one-authority"),
                PathBuf::from("."),
            )
            .append_parameter(Parameter::Custom(npos_parameters.into_custom_parameter())),
            current.clone(),
        )
        .build_raw()
        .expect("build authority test manifest")
        .with_consensus_mode(SumeragiConsensusMode::Npos);
        let error = ensure_kagemusha_mint_finality_schedule_matches_consensus(&manifest)
            .expect_err("height-one NPoS boundary requires a successor authority");
        assert!(error.to_string().contains("must be present"));

        let mut parameters = manifest
            .kagemusha_mint_finality_genesis_parameters()
            .clone();
        parameters.next_epoch_roster = Some(next_epoch_roster(next));
        let manifest = manifest.with_kagemusha_mint_finality_genesis_parameters(parameters);

        ensure_kagemusha_mint_finality_epoch_zero_authority_matches_topology(&manifest, &current)
            .expect("epoch-one authority is checked against its finalized successor snapshot");
        ensure_kagemusha_mint_finality_schedule_matches_consensus(&manifest)
            .expect("height-one NPoS boundary carries a successor authority");
    }

    #[test]
    fn schedule_rejects_successor_authority_outside_height_one_boundary() {
        let current = test_peers(0x70);
        let manifest = complete_test_genesis_builder_for_peers(
            GenesisBuilder::new_without_executor(
                ChainId::from("unexpected-epoch-one-authority"),
                PathBuf::from("."),
            )
            .append_parameter(Parameter::Custom(
                SumeragiNposParameters::default().into_custom_parameter(),
            )),
            current,
        )
        .build_raw()
        .expect("build authority schedule test manifest")
        .with_consensus_mode(SumeragiConsensusMode::Npos);
        let mut parameters = manifest
            .kagemusha_mint_finality_genesis_parameters()
            .clone();
        parameters.next_epoch_roster = Some(next_epoch_roster(test_peers(0x90)));
        let manifest = manifest.with_kagemusha_mint_finality_genesis_parameters(parameters);

        let error = ensure_kagemusha_mint_finality_schedule_matches_consensus(&manifest)
            .expect_err("successor authority is forbidden outside a height-one boundary");
        assert!(error.to_string().contains("must be null unless"));
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
pub use sign::{
    bind_and_sign_staged_sumeragi_v2_context, staged_signed_sumeragi_v2_context_hashes,
};
mod validate;
pub use generate::{ConsensusPolicy, generate_default, validate_consensus_mode};
pub use npos::{ensure_npos_parameters, has_npos_parameters};
pub use profile::{
    GenesisProfile, PUBLIC_NEXUS_CHAIN_ID, PUBLIC_XOR_ALIAS, ProfileDefaults,
    TAIRA_XOR_ASSET_DEFINITION_ID, parse_vrf_seed_hex, profile_defaults, profile_requires_npos,
    profile_uses_public_xor, public_xor_profile_for_chain_id, reject_retired_public_chain_id,
    resolve_vrf_seed,
};
/// Deterministic role used to authorize restricted-dataspace reads at the
/// universal Torii ingress hop for a private localnet profile.
pub fn private_dataspace_reader_role_id(alias: &str, dataspace: DataSpaceId) -> RoleId {
    format!(
        "private_{alias}_dataspace_{}_restricted_reader",
        dataspace.as_u64()
    )
    .parse()
    .expect("private localnet aliases must produce a valid role id")
}
fn require_v2_wire_protocol_only(manifest: &RawGenesisTransaction) -> color_eyre::Result<()> {
    let expected = u32::from(iroha_data_model::block::consensus_v2::PROTOCOL_VERSION);
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
