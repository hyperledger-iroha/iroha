//! Native epoch roots derived from exact authenticated signed genesis.
//!
//! Genesis execution results do not supply signing authority. This reader uses only signed
//! registrations, metadata and NPoS parameters; the executed overlay must independently match
//! the same context before its result or retained schedule can be published.

use crate::{
    NetworkId,
    block::SignedBlock,
    isi::{SetParameter, kagemusha_v1::KagemushaMintFinalityEpochAuthorizationV1},
    parameter::{
        Parameter,
        system::{ConsensusMode, SumeragiNposParameters},
    },
    sumeragi::epoch::{ValidatorCommitteeMemberV1, ValidatorEpochContextV1},
    transaction::{Executable, TransactionDomain},
};
use iroha_crypto::Hash;

/// Authenticate the genesis body and reconstruct its complete native signing context.
/// The returned context is independent of result-only certificate data and mutable World.
/// Its network identity is the signed genesis header hash, not a caller-selected network.
pub fn genesis_epoch(genesis: &SignedBlock) -> Result<ValidatorEpochContextV1, String> {
    if !genesis.header().is_genesis() {
        return Err("native epoch root requires height-one signed genesis".into());
    }
    genesis
        .validate_proposal_commitments()
        .map_err(|error| error.to_string())?;
    let first = genesis
        .external_transactions()
        .next()
        .ok_or("signed genesis is empty")?;
    let signer = first
        .authority()
        .try_signatory()
        .ok_or("genesis requires one exact signing authority")?;
    let mut signatures = genesis.signatures();
    let signature = signatures
        .next()
        .ok_or("signed genesis has no block signature")?;
    if signature.index() != 0 || signatures.next().is_some() {
        return Err("genesis must carry exactly its index-zero authority signature".into());
    }
    signature
        .signature()
        .verify_hash(signer, genesis.hash())
        .map_err(|error| error.to_string())?;
    let mut npos = None;
    for transaction in genesis.external_transactions() {
        if transaction.authority().try_signatory() != Some(signer)
            || transaction.domain() != &TransactionDomain::Genesis
        {
            return Err("genesis transaction changes its signing authority or domain".into());
        }
        transaction
            .verify_signature()
            .map_err(|error| error.to_string())?;
        let Executable::Instructions(instructions) = transaction.instructions() else {
            return Err("genesis epoch authority requires explicit signed instructions".into());
        };
        for instruction in instructions {
            let Some(set) = instruction.as_any().downcast_ref::<SetParameter>() else {
                continue;
            };
            let Parameter::Custom(custom) = set.inner() else {
                continue;
            };
            if custom.id() != &SumeragiNposParameters::parameter_id() {
                continue;
            }
            let parameters = SumeragiNposParameters::from_custom_parameter(custom)
                .ok_or("invalid signed genesis NPoS parameters")?;
            if npos.replace(parameters).is_some() {
                return Err("genesis repeats its signed NPoS parameter authority".into());
            }
        }
    }
    let metadata = signed_genesis_consensus_metadata(genesis).map_err(|error| error.to_string())?;
    if metadata.wire_protocol_version != u32::from(crate::sumeragi::PROTOCOL_VERSION) {
        return Err("genesis metadata does not bind the current native protocol".into());
    }
    let mode = ConsensusMode::from(metadata.mode);
    let network_id = NetworkId::from_genesis_hash(genesis.hash());
    let (last_height, leader_seed) = match mode {
        ConsensusMode::Permissioned => (
            u64::MAX,
            Hash::new_from_chunks(&[
                b"iroha:native-permissioned-leader:v1",
                &[0],
                network_id.as_bytes(),
            ])
            .into(),
        ),
        ConsensusMode::Npos => {
            let parameters = npos.ok_or("NPoS genesis omits its signed epoch length and seed")?;
            let policy =
                crate::nexus::ValidatorElectionPolicyV1::from_npos_parameters(&parameters)?;
            (policy.epoch_length_blocks, parameters.epoch_seed)
        }
    };
    let authority = metadata
        .kagemusha_mint_finality
        .authority_generation
        .bind_network_id(network_id)
        .map_err(|error| error.to_string())?;
    let authorization = KagemushaMintFinalityEpochAuthorizationV1::genesis(&authority, last_height)
        .map_err(|error| error.to_string())?;
    let committee = super::genesis_registrations(genesis)
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(
            |(validator, proof_of_possession)| ValidatorCommitteeMemberV1 {
                validator,
                proof_of_possession,
            },
        )
        .collect();
    let epoch = ValidatorEpochContextV1 {
        da_layout: metadata.sumeragi_context.da_layout,
        version: 1,
        network_id,
        mode,
        authority,
        authorization,
        committee,
        leader_seed,
    };
    epoch.validate()?;
    Ok(epoch)
}

/// Decode and validate the unique consensus metadata in an authenticated signed genesis body.
///
/// # Errors
/// Missing, duplicate, malformed or invalid current consensus metadata.
pub fn signed_genesis_consensus_metadata(
    block: &SignedBlock,
) -> Result<crate::parameter::system::ConsensusHandshakeMetadata, String> {
    use crate::parameter::system::{ConsensusHandshakeMetadata, consensus_metadata};
    let mut metadata = None;
    for transaction in block.external_transactions() {
        let Executable::Instructions(instructions) = transaction.instructions() else {
            continue;
        };
        for instruction in instructions {
            let Some(set) = instruction.as_any().downcast_ref::<SetParameter>() else {
                continue;
            };
            let Parameter::Custom(custom) = set.inner() else {
                continue;
            };
            if custom.id() != &consensus_metadata::handshake_meta_id() {
                continue;
            }
            let decoded = custom
                .payload()
                .try_into_any::<ConsensusHandshakeMetadata>()
                .map_err(|error| format!("decode signed genesis consensus metadata: {error}"))?;
            if metadata.replace(decoded).is_some() {
                return Err(
                    "signed genesis contains more than one consensus metadata instruction".into(),
                );
            }
        }
    }
    let metadata = metadata.ok_or("signed genesis contains no consensus metadata instruction")?;
    metadata
        .validate()
        .map_err(|error| format!("invalid signed genesis consensus metadata: {error}"))?;
    Ok(metadata)
}
