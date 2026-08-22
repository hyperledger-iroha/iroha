//! Opaque derivation of Kagemusha runtime-effective validator evidence.

use std::time::Duration;

use iroha_config::parameters::actual::{NodeRole, Root as Config};
use iroha_data_model::{
    NetworkId,
    block::consensus_v2::{ConsensusMode, SumeragiV2GenesisContextParameters},
    isi::{Instruction as _, SetParameter},
    offline::{
        KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT, KagemushaV4RuntimeEffectiveConfigProjectionV1,
        KagemushaV4RuntimeValidatorProjectionV1,
    },
    parameter::{
        Parameter,
        system::{ConsensusHandshakeMetadata, SumeragiConsensusMode, consensus_metadata},
    },
    transaction::Executable,
};
use iroha_genesis::GenesisBlock;

use crate::sumeragi::{
    GenesisV2Bootstrap, filter_validators_from_trusted, signed_genesis_validator_pops,
};

/// Non-forgeable projection derived from final config and frozen signed genesis.
#[derive(Debug)]
pub struct VerifiedKagemushaV4RuntimeEffectiveConfigV1 {
    projection: KagemushaV4RuntimeEffectiveConfigProjectionV1,
}

impl VerifiedKagemushaV4RuntimeEffectiveConfigV1 {
    /// Derive the only projection accepted by the production qualification signer.
    ///
    /// # Errors
    ///
    /// Returns an error unless config, signed genesis, staged bootstrap, exact
    /// four-validator topology, advertised endpoints, and PoPs agree.
    pub fn derive(
        config: &Config,
        genesis: &GenesisBlock,
        bootstrap: &GenesisV2Bootstrap,
    ) -> Result<Self, String> {
        let metadata = exact_signed_consensus_metadata(genesis)?;
        let context = bootstrap.context();
        let staged_pops = bootstrap.proofs_of_possession();
        let staged_parameters = SumeragiV2GenesisContextParameters {
            da_layout: context.da_layout,
            nexus_amx_context_hash: *context.nexus_amx_context_hash.as_ref(),
            execution_policy_hash: *context.execution_policy_hash.as_ref(),
        };
        if config.sumeragi.role != NodeRole::Validator
            || metadata.mode != SumeragiConsensusMode::Permissioned
            || context.mode != ConsensusMode::Permissioned
            || metadata.sumeragi_v2 != staged_parameters
            || context.network_id != NetworkId::from_genesis_hash(genesis.0.hash())
            || context.roster.len() != KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT
            || staged_pops.len() != context.roster.len()
            || context.roster.iter().any(|member| member.power != 1)
        {
            return Err(
                "Kagemusha runtime-effective config requires exact staged permissioned genesis"
                    .to_owned(),
            );
        }
        let genesis_authority = genesis
            .0
            .external_transactions()
            .next()
            .and_then(|transaction| transaction.authority().try_signatory())
            .ok_or_else(|| "signed genesis has no canonical root authority".to_owned())?;
        if config.genesis.expected_hash != genesis.0.hash()
            || &config.genesis.public_key != genesis_authority
        {
            return Err("effective genesis roots differ from signed genesis".to_owned());
        }

        let trusted = config.common.trusted_peers.value();
        let local_id = config.common.peer.id();
        if trusted.myself.id() != local_id
            || config.common.key_pair.public_key() != local_id.public_key()
        {
            return Err("effective local peer differs from trusted myself identity".to_owned());
        }
        let context_validators = context
            .roster
            .iter()
            .map(|member| member.validator.clone())
            .collect::<Vec<_>>();
        let mut configured_validators = filter_validators_from_trusted(trusted);
        configured_validators.sort();
        let signed = signed_genesis_validator_pops(genesis)
            .map_err(|error| format!("invalid signed genesis voting authority: {error}"))?;
        let exact_pops = signed.len() == staged_pops.len()
            && signed.len() == trusted.pops.len()
            && signed
                .iter()
                .zip(context_validators.iter().zip(staged_pops))
                .all(|((signed_id, signed_pop), (staged_id, staged_pop))| {
                    signed_id == staged_id
                        && signed_pop == staged_pop
                        && trusted.pops.get(signed_id.public_key()) == Some(signed_pop)
                });
        if configured_validators != context_validators
            || !context_validators.contains(local_id)
            || !exact_pops
        {
            return Err(
                "effective trusted-validator topology or PoPs differ from staged signed genesis"
                    .to_owned(),
            );
        }

        let validators = context_validators
            .iter()
            .zip(staged_pops)
            .map(|(validator_id, staged_pop)| {
                let public_address = if validator_id == local_id {
                    config.network.public_address.value().clone()
                } else {
                    trusted
                        .others
                        .iter()
                        .find(|peer| peer.id() == validator_id)
                        .map(|peer| peer.address().clone())
                        .ok_or_else(|| {
                            "effective remote validator has no advertised endpoint".to_owned()
                        })?
                };
                Ok(KagemushaV4RuntimeValidatorProjectionV1 {
                    validator_id: validator_id.clone(),
                    public_address,
                    bls_pop: staged_pop.clone(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?
            .try_into()
            .map_err(|_| "effective validator projection is not exactly four peers".to_owned())?;
        let sumeragi = config
            .sumeragi
            .v2_config(
                Duration::from_millis(metadata.block_cadence_ms.get()),
                context.mode,
            )
            .map_err(|error| format!("invalid effective Sumeragi V2 config: {error}"))?;
        let projection = KagemushaV4RuntimeEffectiveConfigProjectionV1 {
            chain: config.common.chain.clone(),
            chain_discriminant: *config.common.chain_discriminant.value(),
            is_validator: true,
            genesis_public_key: config.genesis.public_key.clone(),
            genesis_expected_hash: config.genesis.expected_hash,
            validators,
            sumeragi_config_fingerprint: sumeragi.fingerprint(),
            genesis_context: staged_parameters,
            kagemusha_max_decoded_bytes: config.settlement.offline.kagemusha_max_decoded_bytes,
        };
        projection.validate().map_err(|error| error.to_string())?;
        Ok(Self { projection })
    }

    /// Borrow the config-derived public projection for sealing.
    #[must_use]
    pub const fn projection(&self) -> &KagemushaV4RuntimeEffectiveConfigProjectionV1 {
        &self.projection
    }
}

fn exact_signed_consensus_metadata(
    genesis: &GenesisBlock,
) -> Result<ConsensusHandshakeMetadata, String> {
    let mut found = None;
    for transaction in genesis.0.external_transactions() {
        let Executable::Instructions(instructions) = transaction.instructions() else {
            return Err("signed genesis metadata is not an instruction batch".to_owned());
        };
        for instruction in instructions {
            let Some(set_parameter) = instruction.as_any().downcast_ref::<SetParameter>() else {
                continue;
            };
            let Parameter::Custom(custom) = set_parameter.inner() else {
                continue;
            };
            if custom.id() != &consensus_metadata::handshake_meta_id() {
                continue;
            }
            let metadata: ConsensusHandshakeMetadata = custom
                .payload()
                .try_into_any()
                .map_err(|error| format!("invalid signed consensus metadata: {error}"))?;
            metadata
                .validate()
                .map_err(|error| format!("invalid signed consensus metadata: {error}"))?;
            if found.replace(metadata).is_some() {
                return Err("signed genesis repeats consensus metadata".to_owned());
            }
        }
    }
    found.ok_or_else(|| "signed genesis omits consensus metadata".to_owned())
}
