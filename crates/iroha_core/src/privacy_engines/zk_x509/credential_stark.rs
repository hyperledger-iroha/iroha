//! Canonical full-credential proof envelope and cross-subproof binding.
//!
//! `X5S1` is the sole first-release credential container. It carries exactly
//! two ordered, length-delimited proof records: one main aggregate proof and
//! one dedicated `X5C1` compact-CA proof. Public statement material is repeated
//! in the fixed header so decode cannot silently pair a proof with another
//! statement, root, or root-SPKI channel. The cryptographic verifier must still
//! derive and compare that material from its trusted statement.

use iroha_data_model::privacy::{IrohaZkX509StarkP256StatementV1, PrivacyStatementV1};
use thiserror::Error;

use super::{
    accumulator_stark::{
        ZK_X509_CA_ACCUMULATOR_MAX_PROOF_BYTES_V1,
        ZK_X509_CA_ACCUMULATOR_ROOT_SPKI_BASE_CHANNEL_V1,
        ZK_X509_CA_ACCUMULATOR_ROOT_SPKI_IO_EVENTS_V1, ZkX509CaAccumulatorStarkPublicV1,
        ZkX509CaAccumulatorSubproofBindingV1,
    },
    merkle::hash_frame_v1,
    profile::{ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1, ZK_X509_PROOF_VERSION_V1},
    sha_call_bus_stark::{
        ZK_X509_SHA_CA_LEAF_CALL_V1, ZK_X509_SHA_CA_NODE_CALL_START_V1, ZK_X509_SHA_CALL_COUNT_V1,
        ZkX509ShaCallRoleV1, ZkX509ShaCallTerminalV1,
    },
};
use crate::privacy_engines::transparent_stark::GoldilocksFieldV1 as F;

const CREDENTIAL_MAGIC_V1: [u8; 4] = *b"X5S1";
const MAIN_AGGREGATE_MAGIC_V1: [u8; 4] = *b"X5M1";
const CA_SUBPROOF_MAGIC_V1: [u8; 4] = *b"X5C1";
const SUBPROOF_COUNT_V1: u16 = 2;
const MAIN_SUBPROOF_KIND_V1: u16 = 1;
const CA_SUBPROOF_KIND_V1: u16 = 2;
const SUBPROOF_INSTANCE_V1: u16 = 0;
const CONSENSUS_CONTEXT_DIGEST_DOMAIN_V1: &[u8] = b"iroha.zk-x509.credential-consensus-context.v1";
const FIXED_HEADER_BYTES_V1: usize = 4 + 2 + 2 + 32 + 32 + 4;
const SUBPROOF_HEADER_BYTES_V1: usize = 2 + 2 + 4;
/// Exact outer bytes added around the two already-encoded inner proofs.
///
/// This is the sole source of truth for the consensus profile's combined
/// proof-size arithmetic. It includes the fixed public header and both
/// ordered `(kind, instance, length)` records, but no inner proof byte.
pub(crate) const ZK_X509_CREDENTIAL_ENVELOPE_FRAMING_BYTES_V1: usize =
    FIXED_HEADER_BYTES_V1 + 2 * SUBPROOF_HEADER_BYTES_V1;
/// Exact hard ceiling for the main aggregate section inside `X5S1`.
///
/// The full credential ceiling is partitioned rather than shared dynamically:
/// a caller cannot steal the compact-CA verifier's budget for an oversized
/// main proof, or vice versa.
pub(crate) const ZK_X509_MAIN_AGGREGATE_MAX_PROOF_BYTES_V1: usize =
    ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1 as usize
        - ZK_X509_CREDENTIAL_ENVELOPE_FRAMING_BYTES_V1
        - ZK_X509_CA_ACCUMULATOR_MAX_PROOF_BYTES_V1;
const MINIMUM_ENCODED_BYTES_V1: usize = ZK_X509_CREDENTIAL_ENVELOPE_FRAMING_BYTES_V1 + 2 * 4;
const _: () = assert!(
    ZK_X509_CREDENTIAL_ENVELOPE_FRAMING_BYTES_V1
        + ZK_X509_MAIN_AGGREGATE_MAX_PROOF_BYTES_V1
        + ZK_X509_CA_ACCUMULATOR_MAX_PROOF_BYTES_V1
        == ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1 as usize
);

/// Compute the exact encoded outer-envelope length without allocation.
pub(crate) const fn zk_x509_credential_envelope_encoded_len_v1(
    main_aggregate_bytes: usize,
    ca_subproof_bytes: usize,
) -> Option<usize> {
    match ZK_X509_CREDENTIAL_ENVELOPE_FRAMING_BYTES_V1.checked_add(main_aggregate_bytes) {
        Some(bytes) => bytes.checked_add(ca_subproof_bytes),
        None => None,
    }
}

/// Fixed verifier-derived public material repeated in the `X5S1` header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509CredentialPublicBindingV1 {
    /// Canonical digest of the complete typed statement and committed genesis.
    pub(crate) consensus_context_digest: [u8; 32],
    /// Governed compact-CA root as exact bytes.
    pub(crate) governed_ca_root: [u8; 32],
    /// Canonical root-SPKI byte channel.
    pub(crate) root_spki_channel: u32,
}

impl ZkX509CredentialPublicBindingV1 {
    /// Derive all header material from verifier-owned consensus context.
    pub(crate) fn from_consensus_context_v1(
        statement: &IrohaZkX509StarkP256StatementV1,
        genesis_hash: [u8; 32],
    ) -> Result<Self, ZkX509CredentialProofErrorV1> {
        if genesis_hash == [0; 32] {
            return Err(ZkX509CredentialProofErrorV1::InvalidStatement);
        }
        let statement_digest = PrivacyStatementV1::IrohaZkX509StarkP256V0(statement.clone())
            .digest()
            .map_err(|_| ZkX509CredentialProofErrorV1::InvalidStatement)?
            .into_bytes();
        let consensus_context_digest = hash_frame_v1(
            CONSENSUS_CONTEXT_DIGEST_DOMAIN_V1,
            &[&statement_digest, &genesis_hash],
        )
        .map_err(|_| ZkX509CredentialProofErrorV1::InvalidStatement)?;
        let disclosed = u32::try_from(statement.disclosed_attributes.len())
            .map_err(|_| ZkX509CredentialProofErrorV1::InvalidStatement)?;
        let root_spki_channel = disclosed
            .checked_mul(2)
            .and_then(|channels| {
                ZK_X509_CA_ACCUMULATOR_ROOT_SPKI_BASE_CHANNEL_V1.checked_add(channels)
            })
            .ok_or(ZkX509CredentialProofErrorV1::InvalidStatement)?;
        Ok(Self {
            consensus_context_digest,
            governed_ca_root: *statement.ca_membership_root.as_bytes(),
            root_spki_channel,
        })
    }

    /// Convert the byte-level public binding to the compact-CA field input.
    pub(crate) fn ca_public_v1(self) -> ZkX509CaAccumulatorStarkPublicV1 {
        ZkX509CaAccumulatorStarkPublicV1 {
            governed_root: self.governed_ca_root.map(|byte| F(u64::from(byte))),
            root_spki_channel: F(u64::from(self.root_spki_channel)),
        }
    }
}

/// Proof-derived terminals from the main aggregate that must equal `X5C1`.
///
/// The main verifier constructs this value only after verifying all opened-row
/// relations. No witness-fed expectation is accepted at this boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509MainCaBindingV1 {
    /// Public statement material bound by the main transcript.
    pub(crate) public: ZkX509CredentialPublicBindingV1,
    /// Exact ordered main-SHA terminals for calls 16 through 28.
    pub(crate) sha_terminals:
        [ZkX509ShaCallTerminalV1; ZK_X509_SHA_CALL_COUNT_V1 - ZK_X509_SHA_CA_LEAF_CALL_V1],
    /// Main-RFC terminal reserved for the root-SPKI consumer.
    pub(crate) root_spki_consumer_products: [F; 4],
}

/// Borrowed exact contents of a canonical `X5S1` envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509CredentialEnvelopeV1<'a> {
    /// Header binding checked against the verifier-owned statement.
    pub(crate) public: ZkX509CredentialPublicBindingV1,
    /// Exact main aggregate proof bytes.
    pub(crate) main_aggregate: &'a [u8],
    /// Exact compact-CA `X5C1` proof bytes.
    pub(crate) ca_subproof: &'a [u8],
}

/// Canonical credential envelope or cross-subproof verification failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(crate) enum ZkX509CredentialProofErrorV1 {
    /// The verifier-owned public statement cannot define the fixed profile.
    #[error("zk-X509 credential statement is invalid")]
    InvalidStatement,
    /// The exact `X5S1` framing, order, count, or inner proof identity is invalid.
    #[error("zk-X509 credential proof envelope is malformed")]
    MalformedEnvelope,
    /// The combined proof or an individual section exceeds its byte ceiling.
    #[error("zk-X509 credential proof exceeds its byte ceiling")]
    ProofTooLarge,
    /// Header material does not equal the verifier-derived statement binding.
    #[error("zk-X509 credential proof public binding is invalid")]
    PublicBindingMismatch,
    /// The main aggregate proof did not verify.
    #[error("zk-X509 main aggregate proof is invalid")]
    MainProof,
    /// The compact-CA proof did not verify.
    #[error("zk-X509 compact-CA subproof is invalid")]
    CaProof,
    /// Proof-derived main and compact-CA terminals do not match exactly.
    #[error("zk-X509 credential cross-subproof terminals do not match")]
    CrossSubproofMismatch,
}

fn append_u16_v1(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_be_bytes());
}

fn append_u32_v1(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_be_bytes());
}

fn read_u16_v1(encoded: &[u8], cursor: &mut usize) -> Result<u16, ZkX509CredentialProofErrorV1> {
    let end = cursor
        .checked_add(2)
        .ok_or(ZkX509CredentialProofErrorV1::MalformedEnvelope)?;
    let bytes = encoded
        .get(*cursor..end)
        .ok_or(ZkX509CredentialProofErrorV1::MalformedEnvelope)?;
    *cursor = end;
    Ok(u16::from_be_bytes(bytes.try_into().map_err(|_| {
        ZkX509CredentialProofErrorV1::MalformedEnvelope
    })?))
}

fn read_u32_v1(encoded: &[u8], cursor: &mut usize) -> Result<u32, ZkX509CredentialProofErrorV1> {
    let end = cursor
        .checked_add(4)
        .ok_or(ZkX509CredentialProofErrorV1::MalformedEnvelope)?;
    let bytes = encoded
        .get(*cursor..end)
        .ok_or(ZkX509CredentialProofErrorV1::MalformedEnvelope)?;
    *cursor = end;
    Ok(u32::from_be_bytes(bytes.try_into().map_err(|_| {
        ZkX509CredentialProofErrorV1::MalformedEnvelope
    })?))
}

fn read_array_v1<const N: usize>(
    encoded: &[u8],
    cursor: &mut usize,
) -> Result<[u8; N], ZkX509CredentialProofErrorV1> {
    let end = cursor
        .checked_add(N)
        .ok_or(ZkX509CredentialProofErrorV1::MalformedEnvelope)?;
    let value = encoded
        .get(*cursor..end)
        .ok_or(ZkX509CredentialProofErrorV1::MalformedEnvelope)?
        .try_into()
        .map_err(|_| ZkX509CredentialProofErrorV1::MalformedEnvelope)?;
    *cursor = end;
    Ok(value)
}

fn read_subproof_v1<'a>(
    encoded: &'a [u8],
    cursor: &mut usize,
    expected_kind: u16,
    expected_magic: [u8; 4],
    maximum_length: usize,
) -> Result<&'a [u8], ZkX509CredentialProofErrorV1> {
    if read_u16_v1(encoded, cursor)? != expected_kind
        || read_u16_v1(encoded, cursor)? != SUBPROOF_INSTANCE_V1
    {
        return Err(ZkX509CredentialProofErrorV1::MalformedEnvelope);
    }
    let length = usize::try_from(read_u32_v1(encoded, cursor)?)
        .map_err(|_| ZkX509CredentialProofErrorV1::MalformedEnvelope)?;
    if length > maximum_length {
        return Err(ZkX509CredentialProofErrorV1::ProofTooLarge);
    }
    if length < expected_magic.len() {
        return Err(ZkX509CredentialProofErrorV1::MalformedEnvelope);
    }
    let end = cursor
        .checked_add(length)
        .ok_or(ZkX509CredentialProofErrorV1::MalformedEnvelope)?;
    let proof = encoded
        .get(*cursor..end)
        .ok_or(ZkX509CredentialProofErrorV1::MalformedEnvelope)?;
    if proof.get(..expected_magic.len()) != Some(expected_magic.as_slice()) {
        return Err(ZkX509CredentialProofErrorV1::MalformedEnvelope);
    }
    *cursor = end;
    Ok(proof)
}

/// Encode exactly one main aggregate followed by exactly one `X5C1` proof.
pub(crate) fn encode_zk_x509_credential_envelope_v1(
    public: ZkX509CredentialPublicBindingV1,
    main_aggregate: &[u8],
    ca_subproof: &[u8],
) -> Result<Vec<u8>, ZkX509CredentialProofErrorV1> {
    if main_aggregate.get(..4) != Some(MAIN_AGGREGATE_MAGIC_V1.as_slice())
        || ca_subproof.get(..4) != Some(CA_SUBPROOF_MAGIC_V1.as_slice())
    {
        return Err(ZkX509CredentialProofErrorV1::MalformedEnvelope);
    }
    if main_aggregate.len() > ZK_X509_MAIN_AGGREGATE_MAX_PROOF_BYTES_V1
        || ca_subproof.len() > ZK_X509_CA_ACCUMULATOR_MAX_PROOF_BYTES_V1
    {
        return Err(ZkX509CredentialProofErrorV1::ProofTooLarge);
    }
    let main_length = u32::try_from(main_aggregate.len())
        .map_err(|_| ZkX509CredentialProofErrorV1::ProofTooLarge)?;
    let ca_length = u32::try_from(ca_subproof.len())
        .map_err(|_| ZkX509CredentialProofErrorV1::ProofTooLarge)?;
    let encoded_length =
        zk_x509_credential_envelope_encoded_len_v1(main_aggregate.len(), ca_subproof.len())
            .ok_or(ZkX509CredentialProofErrorV1::ProofTooLarge)?;
    if encoded_length > ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1 as usize {
        return Err(ZkX509CredentialProofErrorV1::ProofTooLarge);
    }

    let mut encoded = Vec::new();
    encoded
        .try_reserve_exact(encoded_length)
        .map_err(|_| ZkX509CredentialProofErrorV1::ProofTooLarge)?;
    encoded.extend_from_slice(&CREDENTIAL_MAGIC_V1);
    append_u16_v1(&mut encoded, ZK_X509_PROOF_VERSION_V1);
    append_u16_v1(&mut encoded, SUBPROOF_COUNT_V1);
    encoded.extend_from_slice(&public.consensus_context_digest);
    encoded.extend_from_slice(&public.governed_ca_root);
    append_u32_v1(&mut encoded, public.root_spki_channel);
    append_u16_v1(&mut encoded, MAIN_SUBPROOF_KIND_V1);
    append_u16_v1(&mut encoded, SUBPROOF_INSTANCE_V1);
    append_u32_v1(&mut encoded, main_length);
    encoded.extend_from_slice(main_aggregate);
    append_u16_v1(&mut encoded, CA_SUBPROOF_KIND_V1);
    append_u16_v1(&mut encoded, SUBPROOF_INSTANCE_V1);
    append_u32_v1(&mut encoded, ca_length);
    encoded.extend_from_slice(ca_subproof);
    if encoded.len() != encoded_length {
        return Err(ZkX509CredentialProofErrorV1::MalformedEnvelope);
    }
    Ok(encoded)
}

/// Decode the sole exact, bounded `X5S1` credential container.
pub(crate) fn decode_zk_x509_credential_envelope_v1(
    encoded: &[u8],
) -> Result<ZkX509CredentialEnvelopeV1<'_>, ZkX509CredentialProofErrorV1> {
    if encoded.len() > ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1 as usize {
        return Err(ZkX509CredentialProofErrorV1::ProofTooLarge);
    }
    if encoded.len() < MINIMUM_ENCODED_BYTES_V1
        || encoded.get(..4) != Some(CREDENTIAL_MAGIC_V1.as_slice())
    {
        return Err(ZkX509CredentialProofErrorV1::MalformedEnvelope);
    }
    let mut cursor = 4;
    if read_u16_v1(encoded, &mut cursor)? != ZK_X509_PROOF_VERSION_V1
        || read_u16_v1(encoded, &mut cursor)? != SUBPROOF_COUNT_V1
    {
        return Err(ZkX509CredentialProofErrorV1::MalformedEnvelope);
    }
    let public = ZkX509CredentialPublicBindingV1 {
        consensus_context_digest: read_array_v1(encoded, &mut cursor)?,
        governed_ca_root: read_array_v1(encoded, &mut cursor)?,
        root_spki_channel: read_u32_v1(encoded, &mut cursor)?,
    };
    let main_aggregate = read_subproof_v1(
        encoded,
        &mut cursor,
        MAIN_SUBPROOF_KIND_V1,
        MAIN_AGGREGATE_MAGIC_V1,
        ZK_X509_MAIN_AGGREGATE_MAX_PROOF_BYTES_V1,
    )?;
    let ca_subproof = read_subproof_v1(
        encoded,
        &mut cursor,
        CA_SUBPROOF_KIND_V1,
        CA_SUBPROOF_MAGIC_V1,
        ZK_X509_CA_ACCUMULATOR_MAX_PROOF_BYTES_V1,
    )?;
    if cursor != encoded.len() {
        return Err(ZkX509CredentialProofErrorV1::MalformedEnvelope);
    }
    Ok(ZkX509CredentialEnvelopeV1 {
        public,
        main_aggregate,
        ca_subproof,
    })
}

/// Validate the exact public and terminal equality binding between verified proofs.
///
/// `main` and `ca` must be reconstructed independently from successfully
/// verified proof openings. This pure boundary additionally fixes the semantic
/// SHA call identities and root-SPKI metadata so equal but mislabelled
/// terminals cannot be paired.
pub(crate) fn validate_cross_subproof_binding_v1(
    expected_public: ZkX509CredentialPublicBindingV1,
    main: ZkX509MainCaBindingV1,
    ca: ZkX509CaAccumulatorSubproofBindingV1,
) -> Result<(), ZkX509CredentialProofErrorV1> {
    if main.public != expected_public || ca.public != expected_public.ca_public_v1() {
        return Err(ZkX509CredentialProofErrorV1::PublicBindingMismatch);
    }
    for (index, (main_terminal, ca_terminal)) in
        main.sha_terminals.iter().zip(ca.sha_terminals).enumerate()
    {
        let expected_call_index = if index == 0 {
            ZK_X509_SHA_CA_LEAF_CALL_V1
        } else {
            ZK_X509_SHA_CA_NODE_CALL_START_V1 + index - 1
        };
        let expected_call = u8::try_from(expected_call_index)
            .map_err(|_| ZkX509CredentialProofErrorV1::CrossSubproofMismatch)?;
        let expected_role = if index == 0 {
            ZkX509ShaCallRoleV1::CaLeaf
        } else {
            ZkX509ShaCallRoleV1::CaNode(
                u8::try_from(index - 1)
                    .map_err(|_| ZkX509CredentialProofErrorV1::CrossSubproofMismatch)?,
            )
        };
        if main_terminal.call != expected_call
            || main_terminal.role != expected_role
            || ca_terminal.call != expected_call
            || ca_terminal.role != expected_role
            || main_terminal.call != ca_terminal.call
            || main_terminal.role != ca_terminal.role
            || main_terminal.source_products != ca_terminal.source_products
            || main_terminal.digest_products != ca_terminal.digest_products
        {
            return Err(ZkX509CredentialProofErrorV1::CrossSubproofMismatch);
        }
    }
    if ca.root_spki_terminal.channel != expected_public.root_spki_channel
        || ca.root_spki_terminal.event_count != ZK_X509_CA_ACCUMULATOR_ROOT_SPKI_IO_EVENTS_V1
        || ca.root_spki_terminal.consumer_products != main.root_spki_consumer_products
    {
        return Err(ZkX509CredentialProofErrorV1::CrossSubproofMismatch);
    }
    Ok(())
}

/// Verify the exact envelope and pair independently verified main/CA bindings.
///
/// The two callbacks are cryptographic verifier boundaries, not witness
/// providers. Each must return terminals reconstructed from proof openings.
pub(crate) fn verify_zk_x509_credential_envelope_with_v1<MainVerifier, CaVerifier>(
    expected_public: ZkX509CredentialPublicBindingV1,
    encoded: &[u8],
    mut verify_main: MainVerifier,
    mut verify_ca: CaVerifier,
) -> Result<(), ZkX509CredentialProofErrorV1>
where
    MainVerifier: FnMut(&[u8]) -> Result<ZkX509MainCaBindingV1, ZkX509CredentialProofErrorV1>,
    CaVerifier:
        FnMut(&[u8]) -> Result<ZkX509CaAccumulatorSubproofBindingV1, ZkX509CredentialProofErrorV1>,
{
    let envelope = decode_zk_x509_credential_envelope_v1(encoded)?;
    if envelope.public != expected_public {
        return Err(ZkX509CredentialProofErrorV1::PublicBindingMismatch);
    }
    let main = verify_main(envelope.main_aggregate)
        .map_err(|_| ZkX509CredentialProofErrorV1::MainProof)?;
    let ca = verify_ca(envelope.ca_subproof).map_err(|_| ZkX509CredentialProofErrorV1::CaProof)?;
    validate_cross_subproof_binding_v1(expected_public, main, ca)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::privacy_engines::zk_x509::{
        accumulator_air::ZK_X509_CA_ACCUMULATOR_ACTIVE_ROWS_V1,
        accumulator_stark::{ZkX509CaAccumulatorCallTerminalV1, ZkX509CaAccumulatorIoTerminalV1},
        profile::{
            ZK_X509_CA_CLAIM_ENVELOPE_BYTES_V1, ZK_X509_CA_PRE_DEEP_MAXIMUM_BYTES_V1,
            ZK_X509_DEEP_OPENING_BYTES_V1, ZK_X509_MAIN_CLAIM_ENVELOPE_BYTES_V1,
            ZK_X509_MAIN_FIXED_ORACLE_MAXIMUM_BYTES_V1, ZK_X509_MAIN_PRE_DEEP_MAXIMUM_BYTES_V1,
            ZK_X509_MAX_PROOF_BYTES_V1,
        },
    };

    fn public(seed: u8) -> ZkX509CredentialPublicBindingV1 {
        ZkX509CredentialPublicBindingV1 {
            consensus_context_digest: [seed; 32],
            governed_ca_root: [seed.wrapping_add(1); 32],
            root_spki_channel: 36,
        }
    }

    fn role(index: usize) -> ZkX509ShaCallRoleV1 {
        if index == 0 {
            ZkX509ShaCallRoleV1::CaLeaf
        } else {
            ZkX509ShaCallRoleV1::CaNode(u8::try_from(index - 1).expect("fixture level"))
        }
    }

    fn main_binding(public: ZkX509CredentialPublicBindingV1, seed: u64) -> ZkX509MainCaBindingV1 {
        ZkX509MainCaBindingV1 {
            public,
            sha_terminals: core::array::from_fn(|index| ZkX509ShaCallTerminalV1 {
                call: u8::try_from(ZK_X509_SHA_CA_LEAF_CALL_V1 + index).expect("fixture call"),
                role: role(index),
                source_products: core::array::from_fn(|lane| {
                    F(seed + index as u64 * 16 + lane as u64)
                }),
                digest_products: core::array::from_fn(|lane| {
                    F(seed + 8 + index as u64 * 16 + lane as u64)
                }),
            }),
            root_spki_consumer_products: core::array::from_fn(|lane| F(seed + 1_000 + lane as u64)),
        }
    }

    fn ca_binding(main: ZkX509MainCaBindingV1) -> ZkX509CaAccumulatorSubproofBindingV1 {
        ZkX509CaAccumulatorSubproofBindingV1 {
            public: main.public.ca_public_v1(),
            sha_terminals: core::array::from_fn(|index| {
                let terminal = main.sha_terminals[index];
                ZkX509CaAccumulatorCallTerminalV1 {
                    call: terminal.call,
                    role: terminal.role,
                    source_products: terminal.source_products,
                    digest_products: terminal.digest_products,
                }
            }),
            root_spki_terminal: ZkX509CaAccumulatorIoTerminalV1 {
                channel: main.public.root_spki_channel,
                event_count: ZK_X509_CA_ACCUMULATOR_ROOT_SPKI_IO_EVENTS_V1,
                consumer_products: main.root_spki_consumer_products,
            },
        }
    }

    fn proof_fixture() -> (
        ZkX509CredentialPublicBindingV1,
        ZkX509MainCaBindingV1,
        ZkX509CaAccumulatorSubproofBindingV1,
        Vec<u8>,
    ) {
        let public = public(7);
        let main = main_binding(public, 101);
        let ca = ca_binding(main);
        let encoded =
            encode_zk_x509_credential_envelope_v1(public, b"X5M1main-proof", b"X5C1ca-proof")
                .expect("fixture envelope");
        (public, main, ca, encoded)
    }

    fn verify_fixture(
        expected: ZkX509CredentialPublicBindingV1,
        main: ZkX509MainCaBindingV1,
        ca: ZkX509CaAccumulatorSubproofBindingV1,
        encoded: &[u8],
    ) -> Result<(), ZkX509CredentialProofErrorV1> {
        verify_zk_x509_credential_envelope_with_v1(
            expected,
            encoded,
            |proof| {
                (proof == b"X5M1main-proof")
                    .then_some(main)
                    .ok_or(ZkX509CredentialProofErrorV1::MainProof)
            },
            |proof| {
                (proof == b"X5C1ca-proof")
                    .then_some(ca)
                    .ok_or(ZkX509CredentialProofErrorV1::CaProof)
            },
        )
    }

    fn assert_direct_and_callback_cross_binding_result(
        expected_public: ZkX509CredentialPublicBindingV1,
        main: ZkX509MainCaBindingV1,
        ca: ZkX509CaAccumulatorSubproofBindingV1,
        expected_error: ZkX509CredentialProofErrorV1,
        case: &str,
    ) {
        assert_eq!(
            validate_cross_subproof_binding_v1(expected_public, main, ca),
            Err(expected_error),
            "{case}: direct validation result"
        );

        let encoded = encode_zk_x509_credential_envelope_v1(
            expected_public,
            b"X5M1main-proof",
            b"X5C1ca-proof",
        )
        .expect("cross-binding fixture envelope");
        let mut main_calls = 0_u8;
        let mut ca_calls = 0_u8;
        let result = verify_zk_x509_credential_envelope_with_v1(
            expected_public,
            &encoded,
            |proof| {
                main_calls += 1;
                assert_eq!(proof, b"X5M1main-proof", "{case}: main proof slice");
                Ok(main)
            },
            |proof| {
                ca_calls += 1;
                assert_eq!(proof, b"X5C1ca-proof", "{case}: CA proof slice");
                Ok(ca)
            },
        );
        assert_eq!(result, Err(expected_error), "{case}: callback path");
        assert_eq!(main_calls, 1, "{case}: main callback count");
        assert_eq!(ca_calls, 1, "{case}: CA callback count");
    }

    fn wrong_role(index: usize) -> ZkX509ShaCallRoleV1 {
        if index == 0 {
            ZkX509ShaCallRoleV1::CaNode(0)
        } else {
            ZkX509ShaCallRoleV1::CaLeaf
        }
    }

    #[test]
    fn canonical_envelope_round_trips_and_binds_exactly_two_proofs() {
        let (public, main, ca, encoded) = proof_fixture();
        let decoded = decode_zk_x509_credential_envelope_v1(&encoded).expect("canonical envelope");
        assert_eq!(decoded.public, public);
        assert_eq!(decoded.main_aggregate, b"X5M1main-proof");
        assert_eq!(decoded.ca_subproof, b"X5C1ca-proof");
        validate_cross_subproof_binding_v1(public, main, ca)
            .expect("direct canonical cross-subproof binding");
        verify_fixture(public, main, ca, &encoded).expect("bound credential proof");
        assert_eq!(
            ZK_X509_CA_ACCUMULATOR_ACTIVE_ROWS_V1,
            main.sha_terminals.len()
        );
        assert_eq!(
            usize::from(main.sha_terminals[0].call),
            ZK_X509_SHA_CA_LEAF_CALL_V1
        );
        assert_eq!(
            usize::from(main.sha_terminals[1].call),
            ZK_X509_SHA_CA_NODE_CALL_START_V1
        );
    }

    #[test]
    fn every_truncation_and_any_trailing_suffix_is_rejected() {
        let (_, _, _, encoded) = proof_fixture();
        for length in 0..encoded.len() {
            assert!(
                decode_zk_x509_credential_envelope_v1(&encoded[..length]).is_err(),
                "truncation at {length} accepted"
            );
        }
        let mut trailing = encoded;
        trailing.push(0);
        assert_eq!(
            decode_zk_x509_credential_envelope_v1(&trailing),
            Err(ZkX509CredentialProofErrorV1::MalformedEnvelope)
        );
    }

    #[test]
    fn malformed_duplicate_reordered_and_excess_subproofs_are_rejected() {
        let (_, _, _, encoded) = proof_fixture();
        for offset in [
            0_usize,
            4,
            6,
            FIXED_HEADER_BYTES_V1,
            FIXED_HEADER_BYTES_V1 + 2,
        ] {
            let mut changed = encoded.clone();
            changed[offset] ^= 1;
            assert!(
                decode_zk_x509_credential_envelope_v1(&changed).is_err(),
                "header mutation at {offset} accepted"
            );
        }

        let main_length = b"X5M1main-proof".len();
        let second_kind = FIXED_HEADER_BYTES_V1 + SUBPROOF_HEADER_BYTES_V1 + main_length;
        let mut nonzero_ca_instance = encoded.clone();
        nonzero_ca_instance[second_kind + 2..second_kind + 4].copy_from_slice(&1_u16.to_be_bytes());
        assert_eq!(
            decode_zk_x509_credential_envelope_v1(&nonzero_ca_instance),
            Err(ZkX509CredentialProofErrorV1::MalformedEnvelope)
        );

        let mut duplicate_main = encoded.clone();
        duplicate_main[second_kind..second_kind + 2]
            .copy_from_slice(&MAIN_SUBPROOF_KIND_V1.to_be_bytes());
        assert!(decode_zk_x509_credential_envelope_v1(&duplicate_main).is_err());

        let mut duplicate_ca = encoded.clone();
        duplicate_ca[FIXED_HEADER_BYTES_V1..FIXED_HEADER_BYTES_V1 + 2]
            .copy_from_slice(&CA_SUBPROOF_KIND_V1.to_be_bytes());
        assert!(decode_zk_x509_credential_envelope_v1(&duplicate_ca).is_err());

        let main_record = &encoded[FIXED_HEADER_BYTES_V1..second_kind];
        let ca_record = &encoded[second_kind..];
        let mut swapped = encoded[..FIXED_HEADER_BYTES_V1].to_vec();
        swapped.extend_from_slice(ca_record);
        swapped.extend_from_slice(main_record);
        assert_eq!(
            decode_zk_x509_credential_envelope_v1(&swapped),
            Err(ZkX509CredentialProofErrorV1::MalformedEnvelope)
        );

        let mut excess = encoded;
        excess[6..8].copy_from_slice(&3_u16.to_be_bytes());
        assert!(decode_zk_x509_credential_envelope_v1(&excess).is_err());
    }

    #[test]
    fn every_public_header_byte_is_bound_to_the_verifier_owned_context() {
        let (public, main, ca, encoded) = proof_fixture();
        for offset in 8..FIXED_HEADER_BYTES_V1 {
            let mut changed = encoded.clone();
            changed[offset] ^= 1;
            assert_eq!(
                verify_fixture(public, main, ca, &changed),
                Err(ZkX509CredentialProofErrorV1::PublicBindingMismatch),
                "public header byte {offset} was not bound"
            );
        }
    }

    #[test]
    fn consensus_context_derivation_binds_statement_profile_intent_and_genesis() {
        let (statement, _) = crate::privacy_engines::zk_x509::projection_air::tests::fixture();
        let genesis = [0x91; 32];
        let canonical =
            ZkX509CredentialPublicBindingV1::from_consensus_context_v1(&statement, genesis)
                .expect("canonical consensus context");

        let mut changed_intent = statement.clone();
        changed_intent.context.transaction_intent_digest =
            iroha_data_model::privacy::PrivacyTransactionIntentDigestV1::new([0xA1; 32]);
        assert_ne!(
            ZkX509CredentialPublicBindingV1::from_consensus_context_v1(&changed_intent, genesis)
                .expect("changed transaction intent"),
            canonical
        );

        let mut changed_profile = statement.clone();
        changed_profile.context.parameter_digest =
            iroha_data_model::privacy::PrivacyParameterDigestV1::new([0xA2; 32]);
        assert_ne!(
            ZkX509CredentialPublicBindingV1::from_consensus_context_v1(&changed_profile, genesis)
                .expect("changed parameter digest"),
            canonical
        );

        let mut changed_manifest = statement.clone();
        changed_manifest.context.engine_manifest_digest =
            iroha_data_model::privacy::PrivacyEngineManifestDigestV1::new([0xA3; 32]);
        assert_ne!(
            ZkX509CredentialPublicBindingV1::from_consensus_context_v1(&changed_manifest, genesis)
                .expect("changed engine manifest digest"),
            canonical
        );

        assert_ne!(
            ZkX509CredentialPublicBindingV1::from_consensus_context_v1(&statement, [0x92; 32],)
                .expect("changed committed genesis"),
            canonical
        );
        assert_eq!(
            ZkX509CredentialPublicBindingV1::from_consensus_context_v1(&statement, [0; 32]),
            Err(ZkX509CredentialProofErrorV1::InvalidStatement)
        );
    }

    #[test]
    fn every_one_sided_sha_terminal_field_mutation_is_rejected_on_both_paths() {
        let (public, main, ca, _) = proof_fixture();

        for index in 0..main.sha_terminals.len() {
            let mut corrupt_main = main;
            corrupt_main.sha_terminals[index].call =
                corrupt_main.sha_terminals[index].call.wrapping_add(1);
            assert_direct_and_callback_cross_binding_result(
                public,
                corrupt_main,
                ca,
                ZkX509CredentialProofErrorV1::CrossSubproofMismatch,
                &format!("main terminal {index} call"),
            );

            let mut corrupt_ca = ca;
            corrupt_ca.sha_terminals[index].call =
                corrupt_ca.sha_terminals[index].call.wrapping_add(1);
            assert_direct_and_callback_cross_binding_result(
                public,
                main,
                corrupt_ca,
                ZkX509CredentialProofErrorV1::CrossSubproofMismatch,
                &format!("CA terminal {index} call"),
            );

            let mut corrupt_main = main;
            corrupt_main.sha_terminals[index].role = wrong_role(index);
            assert_direct_and_callback_cross_binding_result(
                public,
                corrupt_main,
                ca,
                ZkX509CredentialProofErrorV1::CrossSubproofMismatch,
                &format!("main terminal {index} role"),
            );

            let mut corrupt_ca = ca;
            corrupt_ca.sha_terminals[index].role = wrong_role(index);
            assert_direct_and_callback_cross_binding_result(
                public,
                main,
                corrupt_ca,
                ZkX509CredentialProofErrorV1::CrossSubproofMismatch,
                &format!("CA terminal {index} role"),
            );

            for lane in 0..main.sha_terminals[index].source_products.len() {
                let mut corrupt_main = main;
                corrupt_main.sha_terminals[index].source_products[lane] =
                    corrupt_main.sha_terminals[index].source_products[lane].add(F::ONE);
                assert_direct_and_callback_cross_binding_result(
                    public,
                    corrupt_main,
                    ca,
                    ZkX509CredentialProofErrorV1::CrossSubproofMismatch,
                    &format!("main terminal {index} source lane {lane}"),
                );

                let mut corrupt_ca = ca;
                corrupt_ca.sha_terminals[index].source_products[lane] =
                    corrupt_ca.sha_terminals[index].source_products[lane].add(F::ONE);
                assert_direct_and_callback_cross_binding_result(
                    public,
                    main,
                    corrupt_ca,
                    ZkX509CredentialProofErrorV1::CrossSubproofMismatch,
                    &format!("CA terminal {index} source lane {lane}"),
                );

                let mut corrupt_main = main;
                corrupt_main.sha_terminals[index].digest_products[lane] =
                    corrupt_main.sha_terminals[index].digest_products[lane].add(F::ONE);
                assert_direct_and_callback_cross_binding_result(
                    public,
                    corrupt_main,
                    ca,
                    ZkX509CredentialProofErrorV1::CrossSubproofMismatch,
                    &format!("main terminal {index} digest lane {lane}"),
                );

                let mut corrupt_ca = ca;
                corrupt_ca.sha_terminals[index].digest_products[lane] =
                    corrupt_ca.sha_terminals[index].digest_products[lane].add(F::ONE);
                assert_direct_and_callback_cross_binding_result(
                    public,
                    main,
                    corrupt_ca,
                    ZkX509CredentialProofErrorV1::CrossSubproofMismatch,
                    &format!("CA terminal {index} digest lane {lane}"),
                );
            }
        }
    }

    #[test]
    fn every_root_spki_product_and_metadata_mutation_is_rejected_on_both_paths() {
        let (public, main, ca, _) = proof_fixture();

        for lane in 0..main.root_spki_consumer_products.len() {
            let mut corrupt_main = main;
            corrupt_main.root_spki_consumer_products[lane] =
                corrupt_main.root_spki_consumer_products[lane].add(F::ONE);
            assert_direct_and_callback_cross_binding_result(
                public,
                corrupt_main,
                ca,
                ZkX509CredentialProofErrorV1::CrossSubproofMismatch,
                &format!("main root-SPKI consumer lane {lane}"),
            );

            let mut corrupt_ca = ca;
            corrupt_ca.root_spki_terminal.consumer_products[lane] =
                corrupt_ca.root_spki_terminal.consumer_products[lane].add(F::ONE);
            assert_direct_and_callback_cross_binding_result(
                public,
                main,
                corrupt_ca,
                ZkX509CredentialProofErrorV1::CrossSubproofMismatch,
                &format!("CA root-SPKI consumer lane {lane}"),
            );
        }

        let mut wrong_channel = ca;
        wrong_channel.root_spki_terminal.channel =
            wrong_channel.root_spki_terminal.channel.wrapping_add(1);
        assert_direct_and_callback_cross_binding_result(
            public,
            main,
            wrong_channel,
            ZkX509CredentialProofErrorV1::CrossSubproofMismatch,
            "root-SPKI channel",
        );

        let mut wrong_event_count = ca;
        wrong_event_count.root_spki_terminal.event_count = wrong_event_count
            .root_spki_terminal
            .event_count
            .wrapping_add(1);
        assert_direct_and_callback_cross_binding_result(
            public,
            main,
            wrong_event_count,
            ZkX509CredentialProofErrorV1::CrossSubproofMismatch,
            "root-SPKI event count",
        );
    }

    #[test]
    fn coordinated_semantic_mutations_cannot_bypass_pure_validation() {
        let (public, main, ca, _) = proof_fixture();

        for index in 0..main.sha_terminals.len() {
            let mut corrupt_main = main;
            let mut corrupt_ca = ca;
            let wrong_call = corrupt_main.sha_terminals[index].call.wrapping_add(1);
            corrupt_main.sha_terminals[index].call = wrong_call;
            corrupt_ca.sha_terminals[index].call = wrong_call;
            assert_direct_and_callback_cross_binding_result(
                public,
                corrupt_main,
                corrupt_ca,
                ZkX509CredentialProofErrorV1::CrossSubproofMismatch,
                &format!("coordinated terminal {index} call"),
            );

            let mut corrupt_main = main;
            let mut corrupt_ca = ca;
            let wrong_role = wrong_role(index);
            corrupt_main.sha_terminals[index].role = wrong_role;
            corrupt_ca.sha_terminals[index].role = wrong_role;
            assert_direct_and_callback_cross_binding_result(
                public,
                corrupt_main,
                corrupt_ca,
                ZkX509CredentialProofErrorV1::CrossSubproofMismatch,
                &format!("coordinated terminal {index} role"),
            );

            let target = (index + 1) % main.sha_terminals.len();
            let mut corrupt_main = main;
            let mut corrupt_ca = ca;
            corrupt_main.sha_terminals[index].call = main.sha_terminals[target].call;
            corrupt_main.sha_terminals[index].role = main.sha_terminals[target].role;
            corrupt_ca.sha_terminals[index].call = ca.sha_terminals[target].call;
            corrupt_ca.sha_terminals[index].role = ca.sha_terminals[target].role;
            assert_direct_and_callback_cross_binding_result(
                public,
                corrupt_main,
                corrupt_ca,
                ZkX509CredentialProofErrorV1::CrossSubproofMismatch,
                &format!("coordinated terminal {index} identity substitution"),
            );
        }

        let changed_public = ZkX509CredentialPublicBindingV1 {
            consensus_context_digest: [0xA5; 32],
            governed_ca_root: [0x5A; 32],
            root_spki_channel: public.root_spki_channel + 2,
        };
        let mut corrupt_main = main;
        corrupt_main.public = changed_public;
        let mut corrupt_ca = ca_binding(corrupt_main);
        corrupt_ca.root_spki_terminal.channel = changed_public.root_spki_channel;
        assert_direct_and_callback_cross_binding_result(
            public,
            corrupt_main,
            corrupt_ca,
            ZkX509CredentialProofErrorV1::PublicBindingMismatch,
            "coordinated public and root-SPKI channel",
        );
    }

    #[test]
    fn mismatched_public_bindings_are_rejected_before_or_after_callbacks() {
        let (public, main, ca, encoded) = proof_fixture();

        for (index, changed_public) in [
            ZkX509CredentialPublicBindingV1 {
                consensus_context_digest: [9; 32],
                ..public
            },
            ZkX509CredentialPublicBindingV1 {
                governed_ca_root: [9; 32],
                ..public
            },
            ZkX509CredentialPublicBindingV1 {
                root_spki_channel: public.root_spki_channel + 1,
                ..public
            },
        ]
        .into_iter()
        .enumerate()
        {
            let mut corrupt_main = main;
            corrupt_main.public = changed_public;
            assert_direct_and_callback_cross_binding_result(
                public,
                corrupt_main,
                ca,
                ZkX509CredentialProofErrorV1::PublicBindingMismatch,
                &format!("main public field {index}"),
            );
        }

        for lane in 0..ca.public.governed_root.len() {
            let mut corrupt_ca = ca;
            corrupt_ca.public.governed_root[lane] =
                corrupt_ca.public.governed_root[lane].add(F::ONE);
            assert_direct_and_callback_cross_binding_result(
                public,
                main,
                corrupt_ca,
                ZkX509CredentialProofErrorV1::PublicBindingMismatch,
                &format!("CA governed-root lane {lane}"),
            );
        }
        let mut corrupt_ca = ca;
        corrupt_ca.public.root_spki_channel = corrupt_ca.public.root_spki_channel.add(F::ONE);
        assert_direct_and_callback_cross_binding_result(
            public,
            main,
            corrupt_ca,
            ZkX509CredentialProofErrorV1::PublicBindingMismatch,
            "CA public root-SPKI channel",
        );

        let mismatched_expected = ZkX509CredentialPublicBindingV1 {
            consensus_context_digest: [0x3C; 32],
            ..public
        };
        let mut main_calls = 0_u8;
        let mut ca_calls = 0_u8;
        assert_eq!(
            verify_zk_x509_credential_envelope_with_v1(
                mismatched_expected,
                &encoded,
                |_| {
                    main_calls += 1;
                    Ok(main)
                },
                |_| {
                    ca_calls += 1;
                    Ok(ca)
                },
            ),
            Err(ZkX509CredentialProofErrorV1::PublicBindingMismatch)
        );
        assert_eq!(main_calls, 0, "MAIN callback ran after header mismatch");
        assert_eq!(ca_calls, 0, "CA callback ran after header mismatch");
    }

    #[test]
    fn inner_payload_bit_corruption_is_rejected_by_independent_verifiers() {
        let (public, main, ca, encoded) = proof_fixture();
        let main_payload = FIXED_HEADER_BYTES_V1 + SUBPROOF_HEADER_BYTES_V1;
        let ca_payload = main_payload + b"X5M1main-proof".len() + SUBPROOF_HEADER_BYTES_V1;
        for offset in [main_payload + 6, ca_payload + 6] {
            let mut corrupt = encoded.clone();
            corrupt[offset] ^= 0x80;
            assert!(verify_fixture(public, main, ca, &corrupt).is_err());
        }
    }

    #[test]
    fn encoder_rejects_wrong_inner_identity_and_global_resource_overflow() {
        let public = public(1);
        assert_eq!(
            encode_zk_x509_credential_envelope_v1(public, b"X5S1main", b"X5C1ca"),
            Err(ZkX509CredentialProofErrorV1::MalformedEnvelope)
        );
        assert_eq!(
            encode_zk_x509_credential_envelope_v1(public, b"X5M1main", b"X5C2ca"),
            Err(ZkX509CredentialProofErrorV1::MalformedEnvelope)
        );
        let mut oversized = vec![0_u8; ZK_X509_MAX_PROOF_BYTES_V1 as usize];
        oversized[..4].copy_from_slice(b"X5C1");
        assert_eq!(
            encode_zk_x509_credential_envelope_v1(public, b"X5M1", &oversized),
            Err(ZkX509CredentialProofErrorV1::ProofTooLarge)
        );
    }

    #[test]
    fn section_specific_resource_caps_are_enforced_before_payload_slicing() {
        let (public, _, _, encoded) = proof_fixture();
        let main_length_offset = FIXED_HEADER_BYTES_V1 + 4;
        for declared in [0_u32, 1, 2, 3] {
            let mut too_short = encoded.clone();
            too_short[main_length_offset..main_length_offset + 4]
                .copy_from_slice(&declared.to_be_bytes());
            assert_eq!(
                decode_zk_x509_credential_envelope_v1(&too_short),
                Err(ZkX509CredentialProofErrorV1::MalformedEnvelope)
            );
        }
        let mut declared_oversized_main = encoded.clone();
        declared_oversized_main[main_length_offset..main_length_offset + 4].copy_from_slice(
            &u32::try_from(ZK_X509_MAIN_AGGREGATE_MAX_PROOF_BYTES_V1 + 1)
                .expect("main cap fits u32")
                .to_be_bytes(),
        );
        assert_eq!(
            decode_zk_x509_credential_envelope_v1(&declared_oversized_main),
            Err(ZkX509CredentialProofErrorV1::ProofTooLarge)
        );

        let ca_length_offset =
            FIXED_HEADER_BYTES_V1 + SUBPROOF_HEADER_BYTES_V1 + b"X5M1main-proof".len() + 4;
        for declared in [0_u32, 1, 2, 3] {
            let mut too_short = encoded.clone();
            too_short[ca_length_offset..ca_length_offset + 4]
                .copy_from_slice(&declared.to_be_bytes());
            assert_eq!(
                decode_zk_x509_credential_envelope_v1(&too_short),
                Err(ZkX509CredentialProofErrorV1::MalformedEnvelope)
            );
        }
        let mut declared_oversized_ca = encoded;
        declared_oversized_ca[ca_length_offset..ca_length_offset + 4].copy_from_slice(
            &u32::try_from(ZK_X509_CA_ACCUMULATOR_MAX_PROOF_BYTES_V1 + 1)
                .expect("CA cap fits u32")
                .to_be_bytes(),
        );
        assert_eq!(
            decode_zk_x509_credential_envelope_v1(&declared_oversized_ca),
            Err(ZkX509CredentialProofErrorV1::ProofTooLarge)
        );

        let mut oversized_main = vec![0; ZK_X509_MAIN_AGGREGATE_MAX_PROOF_BYTES_V1 + 1];
        oversized_main[..4].copy_from_slice(&MAIN_AGGREGATE_MAGIC_V1);
        assert_eq!(
            encode_zk_x509_credential_envelope_v1(public, &oversized_main, b"X5C1"),
            Err(ZkX509CredentialProofErrorV1::ProofTooLarge)
        );

        let mut oversized_ca = vec![0; ZK_X509_CA_ACCUMULATOR_MAX_PROOF_BYTES_V1 + 1];
        oversized_ca[..4].copy_from_slice(&CA_SUBPROOF_MAGIC_V1);
        assert_eq!(
            encode_zk_x509_credential_envelope_v1(public, b"X5M1", &oversized_ca),
            Err(ZkX509CredentialProofErrorV1::ProofTooLarge)
        );
    }

    #[test]
    fn exact_maximum_envelope_includes_the_single_authoritative_outer_frame() {
        assert_eq!(ZK_X509_CREDENTIAL_ENVELOPE_FRAMING_BYTES_V1, 92);
        let maximum_inner = ZK_X509_MAIN_PRE_DEEP_MAXIMUM_BYTES_V1
            + ZK_X509_CA_PRE_DEEP_MAXIMUM_BYTES_V1
            + ZK_X509_DEEP_OPENING_BYTES_V1
            + ZK_X509_CA_CLAIM_ENVELOPE_BYTES_V1
            + ZK_X509_MAIN_CLAIM_ENVELOPE_BYTES_V1
            + ZK_X509_MAIN_FIXED_ORACLE_MAXIMUM_BYTES_V1;
        assert_eq!(
            maximum_inner as usize + ZK_X509_CREDENTIAL_ENVELOPE_FRAMING_BYTES_V1,
            ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1 as usize
        );
        assert_eq!(
            zk_x509_credential_envelope_encoded_len_v1(
                maximum_inner as usize - ZK_X509_CA_ACCUMULATOR_MAX_PROOF_BYTES_V1,
                ZK_X509_CA_ACCUMULATOR_MAX_PROOF_BYTES_V1,
            ),
            Some(ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1 as usize)
        );
        assert!(ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1 < ZK_X509_MAX_PROOF_BYTES_V1);

        let main_bytes = ZK_X509_MAIN_AGGREGATE_MAX_PROOF_BYTES_V1;
        assert_eq!(
            main_bytes,
            maximum_inner as usize - ZK_X509_CA_ACCUMULATOR_MAX_PROOF_BYTES_V1
        );
        let mut main = vec![0_u8; main_bytes];
        let mut ca = vec![0_u8; ZK_X509_CA_ACCUMULATOR_MAX_PROOF_BYTES_V1];
        main[..4].copy_from_slice(&MAIN_AGGREGATE_MAGIC_V1);
        ca[..4].copy_from_slice(&CA_SUBPROOF_MAGIC_V1);
        let encoded = encode_zk_x509_credential_envelope_v1(public(3), &main, &ca)
            .expect("exact maximum outer envelope");
        assert_eq!(
            encoded.len(),
            ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1 as usize
        );
        drop(encoded);

        main.push(0);
        assert_eq!(
            encode_zk_x509_credential_envelope_v1(public(3), &main, &ca),
            Err(ZkX509CredentialProofErrorV1::ProofTooLarge)
        );
    }
}
