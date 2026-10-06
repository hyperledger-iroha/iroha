//! Native epoch roots derived from exact authenticated signed genesis.
//!
//! Genesis execution results do not supply signing authority. This reader uses only signed
//! registrations, metadata and `NPoS` parameters; the executed overlay must independently match
//! the same context before its result or retained schedule can be published.

use crate::{
    NetworkId,
    block::SignedBlock,
    isi::SetParameter,
    parameter::{
        Parameter,
        system::{ConsensusMode, SumeragiNposParameters},
    },
    sumeragi::epoch::{
        ValidatorCommitteeMemberV1, ValidatorEpochAuthorizationV1, ValidatorEpochContextV1,
        ValidatorGenerationV1,
    },
    transaction::{Executable, TransactionDomain},
};
use iroha_crypto::Hash;

/// An invalid signed genesis or its original, unfinished JSON decoding attempt.
///
/// This local read error is not a signed rejection and has no wire codec. Callers must
/// classify the original decoder cause before publishing a deterministic verdict.
#[derive(Debug, Clone, thiserror::Error)]
pub enum GenesisReadError {
    /// Signed authority, commitments, metadata, or policy bounds are invalid.
    #[error("{0}")]
    Invalid(String),
    /// The original signed parameter decoder did not complete successfully.
    #[error("signed genesis JSON: {0}")]
    Json(#[from] norito::json::Error),
}

impl From<String> for GenesisReadError {
    fn from(error: String) -> Self {
        Self::Invalid(error)
    }
}
impl From<&str> for GenesisReadError {
    fn from(error: &str) -> Self {
        Self::Invalid(error.into())
    }
}

/// Authenticate the genesis body and reconstruct its complete native signing context.
/// The returned context is independent of result-only certificate data and mutable World.
/// Its network identity is the signed genesis header hash, not a caller-selected network.
///
/// # Errors
/// Rejects non-genesis input, invalid proposal commitments or original signatures,
/// ambiguous authority or consensus metadata, malformed signed parameters, and an
/// invalid reconstructed epoch or committee.
pub fn genesis_epoch(genesis: &SignedBlock) -> Result<ValidatorEpochContextV1, GenesisReadError> {
    genesis_epoch_with_validation(genesis, None)
}

// Source authentication and signed reconstruction always precede pure context reuse.
// Standalone readers retain their original independent validation-only path.
pub(super) fn genesis_epoch_with_validation(
    genesis: &SignedBlock,
    validation: Option<&super::EpochValidationScope>,
) -> Result<ValidatorEpochContextV1, GenesisReadError> {
    if !genesis.header().is_genesis() {
        return Err("native epoch root requires height-one signed genesis".into());
    }
    genesis.validate_proposal_commitments()?;
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
            let parameters = SumeragiNposParameters::from_custom_parameter(custom)?
                .ok_or("invalid signed genesis NPoS parameters")?;
            if npos.replace(parameters).is_some() {
                return Err("genesis repeats its signed NPoS parameter authority".into());
            }
        }
    }
    let metadata = signed_genesis_consensus_metadata(genesis)?;
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
    let committee: Vec<ValidatorCommitteeMemberV1> = super::genesis_registrations(genesis)
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(
            |(validator, proof_of_possession)| ValidatorCommitteeMemberV1 {
                validator,
                proof_of_possession,
            },
        )
        .collect();
    // Generation zero is the signed registered roster itself; no separate key template exists.
    let generation = ValidatorGenerationV1::from_committee(network_id, 0, &committee);
    let authorization = ValidatorEpochAuthorizationV1::genesis(&generation, last_height)
        .map_err(|error| error.to_string())?;
    let epoch = ValidatorEpochContextV1 {
        da_layout: metadata.sumeragi_context.da_layout,
        version: 1,
        network_id,
        mode,
        authorization,
        committee,
        leader_seed,
    };
    match validation {
        Some(validation) => validation.validate_known_or_fresh(&epoch)?,
        None => epoch.validate()?,
    }
    Ok(epoch)
}

/// Decode and validate the unique consensus metadata in an authenticated signed genesis body.
///
/// # Errors
/// Missing, duplicate, malformed or invalid current consensus metadata.
pub fn signed_genesis_consensus_metadata(
    block: &SignedBlock,
) -> Result<crate::parameter::system::ConsensusHandshakeMetadata, GenesisReadError> {
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
            let decoded =
                norito::json::from_str::<ConsensusHandshakeMetadata>(custom.payload().get())?;
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
