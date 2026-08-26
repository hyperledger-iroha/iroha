//! Opaque derivation of Kagemusha runtime-effective validator evidence.

use std::time::Duration;

use iroha_config::parameters::actual::{NodeRole, Root as Config};
use iroha_data_model::{
    NetworkId,
    block::consensus_v2::{
        ConsensusMode, SnapshotV2BootstrapRecord, SumeragiV2GenesisContextParameters,
    },
    isi::SetParameter,
    offline::{
        KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT, KagemushaV4RuntimeEffectiveConfigProjectionV1,
        KagemushaV4RuntimeValidatorProjectionV1,
    },
    parameter::{
        Parameter,
        system::{ConsensusHandshakeMetadata, SumeragiConsensusMode, consensus_metadata},
    },
    peer::PeerId,
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
        if metadata.mode != SumeragiConsensusMode::Permissioned
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
        require_signed_genesis_root(config, genesis)?;
        let context_validators = context
            .roster
            .iter()
            .map(|member| member.validator.clone())
            .collect::<Vec<_>>();
        let signed = signed_genesis_validator_pops(genesis)
            .map_err(|error| format!("invalid signed genesis voting authority: {error}"))?;
        let exact_pops = signed.len() == staged_pops.len()
            && signed
                .iter()
                .zip(context_validators.iter().zip(staged_pops))
                .all(|((signed_id, signed_pop), (staged_id, staged_pop))| {
                    signed_id == staged_id && signed_pop == staged_pop
                });
        if !exact_pops {
            return Err("staged validator topology or PoPs differ from signed genesis".to_owned());
        }
        let validator_pops = context_validators
            .into_iter()
            .zip(staged_pops.iter().cloned())
            .collect();
        Self::derive_from_authenticated_parts(
            config,
            context.network_id.clone(),
            context.mode,
            staged_parameters,
            Duration::from_millis(metadata.block_cadence_ms.get()),
            validator_pops,
        )
    }

    /// Derive the local gate identity from the exact signed-genesis authority.
    ///
    /// # Errors
    ///
    /// Returns an error unless signed genesis and the effective local runtime
    /// form the exact permissioned four-validator projection.
    pub fn derive_from_signed_genesis(
        config: &Config,
        genesis: &GenesisBlock,
    ) -> Result<Self, String> {
        let metadata = exact_signed_consensus_metadata(genesis)?;
        require_signed_genesis_root(config, genesis)?;
        let validator_pops = signed_genesis_validator_pops(genesis)
            .map_err(|error| format!("invalid signed genesis voting authority: {error}"))?
            .into_iter()
            .collect();
        Self::derive_from_authenticated_parts(
            config,
            NetworkId::from_genesis_hash(genesis.0.hash()),
            metadata.mode.into(),
            metadata.sumeragi_v2,
            Duration::from_millis(metadata.block_cadence_ms.get()),
            validator_pops,
        )
    }

    /// Derive the local gate identity from an already authenticated snapshot lineage.
    ///
    /// The caller must obtain `bootstrap` from
    /// [`crate::state::State::authenticated_snapshot_v2_bootstrap`] and
    /// `signed_genesis_context` from the exact verified local validator
    /// qualification seal. An unauthenticated snapshot candidate or caller
    /// assertion is not an admissible source.
    ///
    /// # Errors
    ///
    /// Returns an error unless the retained snapshot authority and effective
    /// local runtime form the exact permissioned four-validator projection.
    pub fn derive_from_authenticated_snapshot(
        config: &Config,
        bootstrap: &SnapshotV2BootstrapRecord,
        block_cadence: Duration,
        signed_genesis_context: SumeragiV2GenesisContextParameters,
    ) -> Result<Self, String> {
        bootstrap
            .validate()
            .map_err(|error| format!("invalid authenticated snapshot bootstrap: {error}"))?;
        let validator_pops = bootstrap
            .context
            .roster
            .iter()
            .map(|member| member.validator.clone())
            .zip(bootstrap.validator_set_pops.iter().cloned())
            .collect();
        Self::derive_from_authenticated_parts(
            config,
            bootstrap.context.network_id.clone(),
            bootstrap.context.mode,
            signed_genesis_context,
            block_cadence,
            validator_pops,
        )
    }

    fn derive_from_authenticated_parts(
        config: &Config,
        network_id: NetworkId,
        mode: ConsensusMode,
        genesis_context: SumeragiV2GenesisContextParameters,
        block_cadence: Duration,
        mut validator_pops: Vec<(PeerId, Vec<u8>)>,
    ) -> Result<Self, String> {
        if config.sumeragi.role != NodeRole::Validator
            || mode != ConsensusMode::Permissioned
            || network_id != NetworkId::from_genesis_hash(config.genesis.expected_hash)
            || validator_pops.len() != KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT
        {
            return Err(
                "Kagemusha runtime-effective config requires exact permissioned four-validator authority"
                    .to_owned(),
            );
        }
        genesis_context
            .validate()
            .map_err(|error| format!("invalid Kagemusha genesis context: {error}"))?;
        validator_pops.sort_by(|(left, _), (right, _)| left.cmp(right));

        let trusted = config.common.trusted_peers.value();
        let local_id = config.common.peer.id();
        if trusted.myself.id() != local_id
            || config.common.key_pair.public_key() != local_id.public_key()
        {
            return Err("effective local peer differs from trusted myself identity".to_owned());
        }
        let validator_ids = validator_pops
            .iter()
            .map(|(validator_id, _)| validator_id.clone())
            .collect::<Vec<_>>();
        let mut configured_validators = filter_validators_from_trusted(trusted);
        configured_validators.sort();
        let exact_pops = trusted.pops.len() == validator_pops.len()
            && validator_pops.iter().all(|(validator_id, pop)| {
                trusted.pops.get(validator_id.public_key()) == Some(pop)
                    && iroha_crypto::bls_normal_pop_verify(validator_id.public_key(), pop).is_ok()
            });
        if configured_validators != validator_ids
            || !validator_ids.contains(local_id)
            || !exact_pops
        {
            return Err(
                "effective trusted-validator topology or PoPs differ from authenticated authority"
                    .to_owned(),
            );
        }

        let validators = validator_pops
            .iter()
            .map(|(validator_id, pop)| {
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
                    bls_pop: pop.clone(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?
            .try_into()
            .map_err(|_| "effective validator projection is not exactly four peers".to_owned())?;
        let sumeragi = config
            .sumeragi
            .v2_config(block_cadence, mode)
            .map_err(|error| format!("invalid effective Sumeragi V2 config: {error}"))?;
        let projection = KagemushaV4RuntimeEffectiveConfigProjectionV1 {
            chain: config.common.chain.clone(),
            chain_discriminant: *config.common.chain_discriminant.value(),
            is_validator: true,
            genesis_public_key: config.genesis.public_key.clone(),
            genesis_expected_hash: config.genesis.expected_hash,
            validators,
            sumeragi_config_fingerprint: sumeragi.fingerprint(),
            genesis_context,
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

fn require_signed_genesis_root(config: &Config, genesis: &GenesisBlock) -> Result<(), String> {
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
    Ok(())
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
