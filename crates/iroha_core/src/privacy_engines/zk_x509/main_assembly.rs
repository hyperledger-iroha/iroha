//! Canonical production assembly for the zk-X509 MAIN aggregate.
//!
//! This module is the sole bridge from the canonical private witness and
//! governed public records to the fixed 49-registration aggregate.  It
//! assembles strict DER/RFC material, the seven projection calls, all 29 SHA
//! witnesses, five complete P-256 equations, compact CA membership, and one
//! deduplicated cross-segment byte-memory table.
//!
//! Assembly is not proof verification. Native reference validation here is a
//! prover-side differential invariant; the independent aggregate verifier
//! enforces the committed numeric constraints.
use iroha_data_model::privacy::IrohaZkX509StarkP256StatementV1;
use p256::ecdsa::Signature as P256Signature;
use sha2::{Digest as _, Sha256};
use thiserror::Error;
use super::{
    accumulator_air::{
        ZkX509AccumulatorAirErrorV1, ZkX509CaAccumulatorStatementV1, ZkX509CaAccumulatorTraceV1,
        ZkX509CaAccumulatorWitnessV1, build_ca_accumulator_trace_v1,
    },
    codec::ZkX509WitnessV1,
    der_air::{
        ZkX509DerAirErrorV1, ZkX509Rfc5280TraceV1, build_zk_x509_rfc5280_trace_v1,
        certificate_slot_2_active_v1, rfc5280_io_witnesses_v1,
    },
    der_stark::{ZkX509DerStarkBaseV1, ZkX509DerStarkErrorV1, build_zk_x509_der_stark_base_v1},
    io_air::{
        IoAccessV1, ZkX509IoAirErrorV1, ZkX509IoChannelDeclarationV1, ZkX509IoChannelWitnessV1,
        ZkX509IoSegmentRoleV1, build_zk_x509_io_base_tables_v1,
    },
    main_io::{
        ZkX509MainIoDeclarationsV1, ZkX509MainIoPlanErrorV1,
        compile_zk_x509_main_io_declarations_v1,
    },
    merkle::{
        ZK_X509_CA_SPKI_DER_BYTES_V1, ZkX509MerkleErrorV1, certificate_policy_record_preimage_v1,
        crl_commitment_preimage_v1, crl_issuer_spki_preimage_v1, crl_record_preimage_v1,
        trust_anchor_record_preimage_v1,
    },
    p256_ecdsa_air::{P256EcdsaRoleV1, P256EcdsaWitnessV1},
    p256_external_binding_air::{
        P256ExternalBindingErrorV1, P256OptionalCertificateSelectionV1,
        ZK_X509_P256_OPTIONAL_CERTIFICATE_DUMMY_DIGEST_V1,
        select_zk_x509_optional_certificate_p256_witness_v1,
    },
    p256_trace::{
        P256EcdsaTraceMaterialV1, P256TraceCompilerErrorV1, compile_p256_ecdsa_trace_material_v1,
    },
    projection_air::{
        ZK_X509_PROJECTION_HASH_BUFFER_BYTES_V1, ZkX509ProjectionAirErrorV1,
        ZkX509ProjectionTraceV1, ZkX509ProjectionWitnessV1, build_zk_x509_projection_trace_v1,
        projection_io_witnesses_v1,
    },
    relation::{
        ZkX509GovernanceV1, ZkX509RelationErrorV1, ZkX509RelationOutputV1,
        validate_reference_relation_v1,
    },
    rfc5280_stark::{
        ZkX509Rfc5280StarkBaseMaterialV1, ZkX509Rfc5280StarkErrorV1,
        build_zk_x509_rfc5280_stark_base_material_v1,
    },
    sha_call_bus_stark::{
        ZK_X509_SHA_CALL_COUNT_V1, ZkX509ShaCallBusStarkErrorV1, ZkX509ShaCallPublicShapeV1,
        ZkX509ShaCallScheduleV1, ZkX509ShaCallWitnessV1, validate_zk_x509_sha_call_witnesses_v1,
    },
    stark::{
        ZkX509MainVerifierProfileV1, ZkX509StarkErrorV1, construct_zk_x509_main_verifier_profile_v1,
    },
    verifier_profile::rfc_statement_with_crl_number_v1,
};
use crate::privacy_engines::transparent_stark::GoldilocksFieldV1 as F;
const P256_SIGNATURES_V1: usize = 5;
const PROJECTION_SHA_CALLS_V1: usize = 7;
const PROJECTION_SHARED_PREFIX_BASE_CHANNELS_V1: usize = 5;
/// Challenge-independent byte-memory material committed by MAIN.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ZkX509MainIoBaseMaterialV1 {
    /// Canonical channel witnesses after shared-prefix deduplication.
    pub(crate) witnesses: Vec<ZkX509IoChannelWitnessV1>,
    /// Verifier-visible declarations in sequential channel order.
    pub(crate) declarations: Vec<ZkX509IoChannelDeclarationV1>,
    /// Exact producer-plus-consumer byte accesses before fixed-capacity padding.
    pub(crate) logical_active_rows: usize,
    /// Exact endpoint-order byte events.
    pub(crate) execution: Vec<IoAccessV1>,
    /// Exact address-sorted byte events.
    pub(crate) sorted: Vec<IoAccessV1>,
}
/// Complete challenge-independent native material for one MAIN proof.
pub(crate) struct ZkX509MainTraceAssemblyV1 {
    /// Successful native differential projection.
    pub(crate) relation_output: ZkX509RelationOutputV1,
    /// Strict DER/RFC owner trace.
    pub(crate) rfc_trace: ZkX509Rfc5280TraceV1,
    /// Strict DER numeric base material.
    pub(crate) der_base: ZkX509DerStarkBaseV1,
    /// RFC 5280 numeric base material.
    pub(crate) rfc_base: ZkX509Rfc5280StarkBaseMaterialV1,
    /// Projection owner trace.
    pub(crate) projection_trace: ZkX509ProjectionTraceV1,
    /// Compact governed-CA membership trace.
    pub(crate) ca_accumulator_trace: ZkX509CaAccumulatorTraceV1,
    /// Verifier-owned 29-call schedule.
    pub(crate) sha_schedule: ZkX509ShaCallScheduleV1,
    /// Exact witnesses in call-index order.
    pub(crate) sha_witnesses: [ZkX509ShaCallWitnessV1; ZK_X509_SHA_CALL_COUNT_V1],
    /// Exact selected byte inputs in certificate, CRL, wallet order.
    pub(crate) p256_witnesses: [P256EcdsaWitnessV1; P256_SIGNATURES_V1],
    /// Complete native trace compiler output for all five equations.
    pub(crate) p256_materials: [P256EcdsaTraceMaterialV1; P256_SIGNATURES_V1],
    /// Algebraic depth-two/depth-three selection for certificate slot two.
    pub(crate) optional_certificate_selection: P256OptionalCertificateSelectionV1,
    /// One deduplicated global byte-memory material.
    pub(crate) io: ZkX509MainIoBaseMaterialV1,
    /// Complete verifier-owned registration and fixed-preprocessing profile.
    pub(crate) verifier_profile: ZkX509MainVerifierProfileV1,
}
impl ZkX509MainTraceAssemblyV1 {
    /// Recursively overwrite all source and derived private material.
    ///
    /// This is deliberately idempotent so error paths and `Drop` can share
    /// the same scrub routine without relying on allocator deallocation.
    pub(crate) fn zeroize_private_v1(&mut self) {
        self.relation_output.ownership_challenge_digest.fill(0);
        self.rfc_trace.zeroize_private_v1();
        self.der_base.zeroize_private_v1();
        self.rfc_base.zeroize_private_v1();
        self.projection_trace.zeroize_private_v1();
        self.ca_accumulator_trace.zeroize_private_v1();
        for witness in &mut self.sha_witnesses {
            witness.zeroize_private_v1();
        }
        for witness in &mut self.p256_witnesses {
            witness.zeroize_private_v1();
        }
        for material in &mut self.p256_materials {
            material.zeroize_private_v1();
        }
        self.optional_certificate_selection
            .real
            .zeroize_private_v1();
        self.optional_certificate_selection
            .selected
            .zeroize_private_v1();
        self.optional_certificate_selection.active = F::ZERO;
        for witness in &mut self.io.witnesses {
            witness.producer_value.fill(0);
            witness.producer_value.clear();
            for value in &mut witness.consumer_values {
                value.fill(0);
                value.clear();
            }
            witness.consumer_values.clear();
            if let Some(public_value) = &mut witness.declaration.public_value {
                public_value.fill(0);
                public_value.clear();
            }
        }
        self.io.witnesses.clear();
        for declaration in &mut self.io.declarations {
            if let Some(public_value) = &mut declaration.public_value {
                public_value.fill(0);
                public_value.clear();
            }
        }
        self.io.declarations.clear();
        for access in self.io.execution.iter_mut().chain(&mut self.io.sorted) {
            access.channel = F::ZERO;
            access.offset = F::ZERO;
            access.value = F::ZERO;
            access.is_write = F::ZERO;
        }
        self.io.execution.clear();
        self.io.sorted.clear();
    }
    #[cfg(test)]
    fn private_is_zeroized_v1(&self) -> bool {
        self.relation_output.ownership_challenge_digest == [0; 32]
            && self.rfc_trace.private_is_zeroized_v1()
            && self.der_base.private_is_zeroized_v1()
            && self.rfc_base.private_is_zeroized_v1()
            && self.projection_trace.private_is_zeroized_v1()
            && self.ca_accumulator_trace.private_is_zeroized_v1()
            && self
                .sha_witnesses
                .iter()
                .all(ZkX509ShaCallWitnessV1::private_is_zeroized_v1)
            && self
                .p256_witnesses
                .iter()
                .all(P256EcdsaWitnessV1::private_is_zeroized_v1)
            && self
                .p256_materials
                .iter()
                .all(P256EcdsaTraceMaterialV1::private_is_zeroized_v1)
            && self
                .optional_certificate_selection
                .real
                .private_is_zeroized_v1()
            && self
                .optional_certificate_selection
                .selected
                .private_is_zeroized_v1()
            && self.optional_certificate_selection.active == F::ZERO
            && self.io.witnesses.is_empty()
            && self.io.declarations.is_empty()
            && self.io.execution.is_empty()
            && self.io.sorted.is_empty()
    }
}
impl Drop for ZkX509MainTraceAssemblyV1 {
    fn drop(&mut self) {
        self.zeroize_private_v1();
    }
}
/// Canonical MAIN material construction failure.
#[derive(Debug, PartialEq, Eq, Error)]
pub(crate) enum ZkX509MainAssemblyErrorV1 {
    /// Native prover-side differential relation failed.
    #[error(transparent)]
    Relation(#[from] ZkX509RelationErrorV1),
    /// Strict DER/RFC owner material failed.
    #[error(transparent)]
    DerAir(#[from] ZkX509DerAirErrorV1),
    /// Strict DER numeric material failed.
    #[error(transparent)]
    DerStark(#[from] ZkX509DerStarkErrorV1),
    /// RFC 5280 numeric material failed.
    #[error(transparent)]
    RfcStark(#[from] ZkX509Rfc5280StarkErrorV1),
    /// Projection material failed.
    #[error(transparent)]
    Projection(#[from] ZkX509ProjectionAirErrorV1),
    /// Compact CA material failed.
    #[error(transparent)]
    Accumulator(#[from] ZkX509AccumulatorAirErrorV1),
    /// A governed hash frame failed.
    #[error(transparent)]
    Merkle(#[from] ZkX509MerkleErrorV1),
    /// The fixed SHA schedule or a call witness failed.
    #[error(transparent)]
    Sha(#[from] ZkX509ShaCallBusStarkErrorV1),
    /// A P-256 equation failed witness compilation.
    #[error(transparent)]
    P256(#[from] P256TraceCompilerErrorV1),
    /// Optional-certificate selection failed.
    #[error(transparent)]
    P256Selection(#[from] P256ExternalBindingErrorV1),
    /// Cross-segment byte-memory construction failed.
    #[error(transparent)]
    Io(#[from] ZkX509IoAirErrorV1),
    /// Verifier-owned byte-channel declaration compilation or binding failed.
    #[error(transparent)]
    IoPlan(#[from] ZkX509MainIoPlanErrorV1),
    /// The verifier-owned 49-registration topology is not exact.
    #[error("zk-X509 MAIN registration topology is invalid")]
    Registration,
    /// A canonical source, role, ordering, or fixed-width conversion failed.
    #[error("zk-X509 MAIN source assembly is invalid")]
    Source,
    /// Bounded allocation or checked arithmetic failed.
    #[error("zk-X509 MAIN assembly resource envelope is exceeded")]
    Resource,
}
impl From<ZkX509StarkErrorV1> for ZkX509MainAssemblyErrorV1 {
    fn from(_: ZkX509StarkErrorV1) -> Self {
        Self::Registration
    }
}
fn projection_witness_v1(
    statement: &IrohaZkX509StarkP256StatementV1,
    trace: &ZkX509Rfc5280TraceV1,
    witness: &ZkX509WitnessV1,
) -> Result<ZkX509ProjectionWitnessV1, ZkX509MainAssemblyErrorV1> {
    let leaf = trace
        .certificates
        .first()
        .ok_or(ZkX509MainAssemblyErrorV1::Source)?;
    if statement.disclosed_attributes.len() != witness.attribute_openings.len() {
        return Err(ZkX509MainAssemblyErrorV1::Source);
    }
    let mut disclosed_attribute_values = Vec::new();
    let mut attribute_salts = Vec::new();
    disclosed_attribute_values
        .try_reserve_exact(statement.disclosed_attributes.len())
        .map_err(|_| ZkX509MainAssemblyErrorV1::Resource)?;
    attribute_salts
        .try_reserve_exact(statement.disclosed_attributes.len())
        .map_err(|_| ZkX509MainAssemblyErrorV1::Resource)?;
    for (attribute, opening) in statement
        .disclosed_attributes
        .iter()
        .zip(&witness.attribute_openings)
    {
        if attribute.index != opening.index {
            return Err(ZkX509MainAssemblyErrorV1::Source);
        }
        disclosed_attribute_values.push(
            leaf.subject.attributes[usize::from(attribute.index)]
                .clone()
                .ok_or(ZkX509MainAssemblyErrorV1::Source)?,
        );
        attribute_salts.push(opening.salt);
    }
    Ok(ZkX509ProjectionWitnessV1 {
        chain_spki_der: trace
            .certificates
            .iter()
            .map(|certificate| certificate.spki_der.clone())
            .collect(),
        leaf_serial: leaf.serial.clone(),
        disclosed_attribute_values,
        attribute_salts,
    })
}
fn sha_witness_v1(
    schedule: &ZkX509ShaCallScheduleV1,
    call: usize,
    message: Vec<u8>,
) -> Result<ZkX509ShaCallWitnessV1, ZkX509MainAssemblyErrorV1> {
    let manifest = schedule.call(call)?;
    let digest = Sha256::digest(&message).into();
    Ok(ZkX509ShaCallWitnessV1 {
        role: manifest.role,
        message,
        digest,
    })
}
fn projection_sha_messages_v1(
    disclosed_attributes: usize,
    trace: &ZkX509ProjectionTraceV1,
) -> Result<[Vec<u8>; PROJECTION_SHA_CALLS_V1], ZkX509MainAssemblyErrorV1> {
    let prefix = PROJECTION_SHARED_PREFIX_BASE_CHANNELS_V1
        .checked_add(
            disclosed_attributes
                .checked_mul(2)
                .ok_or(ZkX509MainAssemblyErrorV1::Resource)?,
        )
        .ok_or(ZkX509MainAssemblyErrorV1::Resource)?;
    let active_slots = (0..2)
        .chain(2..2 + disclosed_attributes)
        .chain(core::iter::once(6))
        .collect::<Vec<_>>();
    let expected_channels = prefix
        .checked_add(
            active_slots
                .len()
                .checked_mul(3)
                .ok_or(ZkX509MainAssemblyErrorV1::Resource)?,
        )
        .ok_or(ZkX509MainAssemblyErrorV1::Resource)?;
    if trace.io_channels.len() != expected_channels {
        return Err(ZkX509MainAssemblyErrorV1::Source);
    }
    let mut messages: [Vec<u8>; PROJECTION_SHA_CALLS_V1] = core::array::from_fn(|_| Vec::new());
    for (ordinal, slot) in active_slots.into_iter().enumerate() {
        let first = prefix
            .checked_add(
                ordinal
                    .checked_mul(3)
                    .ok_or(ZkX509MainAssemblyErrorV1::Resource)?,
            )
            .ok_or(ZkX509MainAssemblyErrorV1::Resource)?;
        let padded = trace
            .io_channels
            .get(first)
            .ok_or(ZkX509MainAssemblyErrorV1::Source)?;
        let length = trace
            .io_channels
            .get(first + 1)
            .ok_or(ZkX509MainAssemblyErrorV1::Source)?;
        let digest = trace
            .io_channels
            .get(first + 2)
            .ok_or(ZkX509MainAssemblyErrorV1::Source)?;
        if padded.producer.role != ZkX509IoSegmentRoleV1::Projection
            || padded.consumers.len() != 1
            || padded.consumers[0].role != ZkX509IoSegmentRoleV1::Sha256
            || length.producer.role != ZkX509IoSegmentRoleV1::Projection
            || length.consumers.len() != 1
            || length.consumers[0].role != ZkX509IoSegmentRoleV1::Sha256
            || digest.producer.role != ZkX509IoSegmentRoleV1::Sha256
            || padded.value.len() != ZK_X509_PROJECTION_HASH_BUFFER_BYTES_V1
            || length.value.len() != 8
            || digest.value.len() != 32
        {
            return Err(ZkX509MainAssemblyErrorV1::Source);
        }
        let message_len = usize::try_from(u64::from_be_bytes(
            length
                .value
                .as_slice()
                .try_into()
                .map_err(|_| ZkX509MainAssemblyErrorV1::Source)?,
        ))
        .map_err(|_| ZkX509MainAssemblyErrorV1::Resource)?;
        if message_len == 0
            || message_len > padded.value.len()
            || padded.value[message_len..].iter().any(|byte| *byte != 0)
        {
            return Err(ZkX509MainAssemblyErrorV1::Source);
        }
        let message = padded.value[..message_len].to_vec();
        if digest.value.as_slice() != Sha256::digest(&message).as_slice() {
            return Err(ZkX509MainAssemblyErrorV1::Source);
        }
        messages[slot] = message;
    }
    Ok(messages)
}
fn build_sha_witnesses_v1(
    statement: &IrohaZkX509StarkP256StatementV1,
    governance: ZkX509GovernanceV1<'_>,
    witness: &ZkX509WitnessV1,
    rfc_trace: &ZkX509Rfc5280TraceV1,
    projection_trace: &ZkX509ProjectionTraceV1,
    ca_trace: &ZkX509CaAccumulatorTraceV1,
    schedule: &ZkX509ShaCallScheduleV1,
) -> Result<[ZkX509ShaCallWitnessV1; ZK_X509_SHA_CALL_COUNT_V1], ZkX509MainAssemblyErrorV1> {
    let mut calls = Vec::new();
    calls
        .try_reserve_exact(ZK_X509_SHA_CALL_COUNT_V1)
        .map_err(|_| ZkX509MainAssemblyErrorV1::Resource)?;
    for slot in 0..3 {
        calls.push(sha_witness_v1(
            schedule,
            slot,
            rfc_trace
                .certificates
                .get(slot)
                .map_or_else(Vec::new, |certificate| certificate.tbs_der.clone()),
        )?);
    }
    calls.push(sha_witness_v1(schedule, 3, rfc_trace.crl.tbs_der.clone())?);
    calls.push(sha_witness_v1(
        schedule,
        4,
        crl_commitment_preimage_v1(&witness.crl_der)?,
    )?);
    let projection_messages =
        projection_sha_messages_v1(statement.disclosed_attributes.len(), projection_trace)?;
    for (slot, message) in projection_messages.into_iter().enumerate() {
        calls.push(sha_witness_v1(schedule, 5 + slot, message)?);
    }
    let issuer = rfc_trace
        .certificates
        .get(1)
        .ok_or(ZkX509MainAssemblyErrorV1::Source)?;
    calls.push(sha_witness_v1(
        schedule,
        12,
        crl_issuer_spki_preimage_v1(&issuer.spki_der)?,
    )?);
    calls.push(sha_witness_v1(
        schedule,
        13,
        trust_anchor_record_preimage_v1(governance.trust_anchor)?,
    )?);
    calls.push(sha_witness_v1(
        schedule,
        14,
        certificate_policy_record_preimage_v1(governance.certificate_policy)?,
    )?);
    calls.push(sha_witness_v1(
        schedule,
        15,
        crl_record_preimage_v1(governance.crl)?,
    )?);
    calls.extend(ca_trace.hash_witnesses.iter().cloned());
    let calls: [ZkX509ShaCallWitnessV1; ZK_X509_SHA_CALL_COUNT_V1] = calls
        .try_into()
        .map_err(|_| ZkX509MainAssemblyErrorV1::Source)?;
    validate_zk_x509_sha_call_witnesses_v1(schedule, &calls)?;
    Ok(calls)
}
fn strict_signature_words_v1(
    encoded: &[u8],
) -> Result<([u8; 32], [u8; 32]), ZkX509MainAssemblyErrorV1> {
    let signature =
        P256Signature::from_der(encoded).map_err(|_| ZkX509MainAssemblyErrorV1::Source)?;
    if signature.to_der().as_bytes() != encoded {
        return Err(ZkX509MainAssemblyErrorV1::Source);
    }
    Ok((
        signature.r().to_bytes().into(),
        signature.s().to_bytes().into(),
    ))
}
fn public_key_words_v1(encoded: &[u8]) -> Result<([u8; 32], [u8; 32]), ZkX509MainAssemblyErrorV1> {
    if encoded.len() != 65 || encoded.first() != Some(&4) {
        return Err(ZkX509MainAssemblyErrorV1::Source);
    }
    let mut x = [0_u8; 32];
    let mut y = [0_u8; 32];
    x.copy_from_slice(&encoded[1..33]);
    y.copy_from_slice(&encoded[33..65]);
    Ok((x, y))
}
fn p256_witness_v1(
    public_key: &[u8],
    signature_der: &[u8],
    digest_be: [u8; 32],
) -> Result<P256EcdsaWitnessV1, ZkX509MainAssemblyErrorV1> {
    let (public_key_x_be, public_key_y_be) = public_key_words_v1(public_key)?;
    let (r_be, s_be) = strict_signature_words_v1(signature_der)?;
    Ok(P256EcdsaWitnessV1 {
        public_key_x_be,
        public_key_y_be,
        r_be,
        s_be,
        digest_be,
    })
}
fn p256_witness_rs_v1(
    public_key: &[u8],
    signature_rs: &[u8; 64],
    digest_be: [u8; 32],
) -> Result<P256EcdsaWitnessV1, ZkX509MainAssemblyErrorV1> {
    let (public_key_x_be, public_key_y_be) = public_key_words_v1(public_key)?;
    let signature =
        P256Signature::from_slice(signature_rs).map_err(|_| ZkX509MainAssemblyErrorV1::Source)?;
    Ok(P256EcdsaWitnessV1 {
        public_key_x_be,
        public_key_y_be,
        r_be: signature.r().to_bytes().into(),
        s_be: signature.s().to_bytes().into(),
        digest_be,
    })
}
fn build_p256_material_v1(
    witness: &ZkX509WitnessV1,
    trace: &ZkX509Rfc5280TraceV1,
    sha_witnesses: &[ZkX509ShaCallWitnessV1; ZK_X509_SHA_CALL_COUNT_V1],
) -> Result<
    (
        [P256EcdsaWitnessV1; P256_SIGNATURES_V1],
        [P256EcdsaTraceMaterialV1; P256_SIGNATURES_V1],
        P256OptionalCertificateSelectionV1,
    ),
    ZkX509MainAssemblyErrorV1,
> {
    let mut selected = Vec::new();
    selected
        .try_reserve_exact(P256_SIGNATURES_V1)
        .map_err(|_| ZkX509MainAssemblyErrorV1::Resource)?;
    for slot in 0..2 {
        let certificate = trace
            .certificates
            .get(slot)
            .ok_or(ZkX509MainAssemblyErrorV1::Source)?;
        let signer = trace.certificates.get(slot + 1).unwrap_or(certificate);
        selected.push(p256_witness_v1(
            &signer.public_key,
            &certificate.signature.encoded,
            sha_witnesses[slot].digest,
        )?);
    }
    let slot_2_active = certificate_slot_2_active_v1(trace)?;
    let slot_2_real = if slot_2_active == 1 {
        let certificate = trace
            .certificates
            .get(2)
            .ok_or(ZkX509MainAssemblyErrorV1::Source)?;
        p256_witness_v1(
            &certificate.public_key,
            &certificate.signature.encoded,
            sha_witnesses[2].digest,
        )?
    } else {
        if sha_witnesses[2].digest != ZK_X509_P256_OPTIONAL_CERTIFICATE_DUMMY_DIGEST_V1 {
            return Err(ZkX509MainAssemblyErrorV1::Source);
        }
        P256EcdsaWitnessV1 {
            public_key_x_be: [0; 32],
            public_key_y_be: [0; 32],
            r_be: [0; 32],
            s_be: [0; 32],
            digest_be: sha_witnesses[2].digest,
        }
    };
    let optional_certificate_selection =
        select_zk_x509_optional_certificate_p256_witness_v1(slot_2_active, slot_2_real)?;
    selected.push(optional_certificate_selection.selected);
    let issuer = trace
        .certificates
        .get(1)
        .ok_or(ZkX509MainAssemblyErrorV1::Source)?;
    selected.push(p256_witness_v1(
        &issuer.public_key,
        &trace.crl.signature.encoded,
        sha_witnesses[3].digest,
    )?);
    let leaf = trace
        .certificates
        .first()
        .ok_or(ZkX509MainAssemblyErrorV1::Source)?;
    selected.push(p256_witness_rs_v1(
        &leaf.public_key,
        &witness.wallet_ownership_signature_rs,
        sha_witnesses[11].digest,
    )?);
    let selected: [P256EcdsaWitnessV1; P256_SIGNATURES_V1] = selected
        .try_into()
        .map_err(|_| ZkX509MainAssemblyErrorV1::Source)?;
    let mut materials = Vec::new();
    materials
        .try_reserve_exact(P256_SIGNATURES_V1)
        .map_err(|_| ZkX509MainAssemblyErrorV1::Resource)?;
    for (index, p256_witness) in selected.iter().copied().enumerate() {
        let role = if index + 1 == P256_SIGNATURES_V1 {
            P256EcdsaRoleV1::WalletOwnership
        } else {
            P256EcdsaRoleV1::CertificateOrCrl
        };
        materials.push(compile_p256_ecdsa_trace_material_v1(role, p256_witness)?);
    }
    let materials: [P256EcdsaTraceMaterialV1; P256_SIGNATURES_V1] = materials
        .try_into()
        .map_err(|_| ZkX509MainAssemblyErrorV1::Source)?;
    Ok((selected, materials, optional_certificate_selection))
}
fn build_io_material_v1(
    plan: &ZkX509MainIoDeclarationsV1,
    rfc_trace: &ZkX509Rfc5280TraceV1,
    projection_trace: &ZkX509ProjectionTraceV1,
    disclosed_attributes: usize,
) -> Result<ZkX509MainIoBaseMaterialV1, ZkX509MainAssemblyErrorV1> {
    let mut rfc = rfc5280_io_witnesses_v1(rfc_trace, 0)?;
    let projection = projection_io_witnesses_v1(projection_trace.io_channels.clone(), 0)?;
    let shared_prefix = PROJECTION_SHARED_PREFIX_BASE_CHANNELS_V1
        .checked_add(
            disclosed_attributes
                .checked_mul(2)
                .ok_or(ZkX509MainAssemblyErrorV1::Resource)?,
        )
        .ok_or(ZkX509MainAssemblyErrorV1::Resource)?;
    if projection.len() < shared_prefix
        || rfc.len() < shared_prefix
        || rfc[..shared_prefix] != projection[..shared_prefix]
    {
        return Err(ZkX509MainAssemblyErrorV1::Source);
    }
    for mut extra in projection.into_iter().skip(shared_prefix) {
        extra.declaration.channel =
            u32::try_from(rfc.len()).map_err(|_| ZkX509MainAssemblyErrorV1::Resource)?;
        rfc.push(extra);
    }
    plan.validate_witness_declarations_v1(&rfc)?;
    let (declarations, execution, sorted) = build_zk_x509_io_base_tables_v1(&rfc)?;
    if declarations != plan.declarations
        || execution.len() != plan.logical_active_rows
        || sorted.len() != plan.logical_active_rows
    {
        return Err(ZkX509MainIoPlanErrorV1::Topology.into());
    }
    Ok(ZkX509MainIoBaseMaterialV1 {
        witnesses: rfc,
        declarations,
        logical_active_rows: plan.logical_active_rows,
        execution,
        sorted,
    })
}
/// Build the sole canonical challenge-independent MAIN trace assembly.
pub(crate) fn build_zk_x509_main_trace_assembly_v1(
    statement: &IrohaZkX509StarkP256StatementV1,
    governance: ZkX509GovernanceV1<'_>,
    witness: &ZkX509WitnessV1,
) -> Result<ZkX509MainTraceAssemblyV1, ZkX509MainAssemblyErrorV1> {
    let verifier_profile = construct_zk_x509_main_verifier_profile_v1()?;
    if verifier_profile.registration.logical_registrations != 49 {
        return Err(ZkX509MainAssemblyErrorV1::Registration);
    }
    let io_plan = compile_zk_x509_main_io_declarations_v1(statement)?;
    let relation_output = validate_reference_relation_v1(statement, governance, witness)?;
    let rfc_trace = build_zk_x509_rfc5280_trace_v1(
        &witness.certificate_chain_der,
        &witness.crl_der,
        rfc_statement_with_crl_number_v1(statement, governance.crl.crl_number),
    )?;
    let mut documents = witness
        .certificate_chain_der
        .iter()
        .map(Vec::as_slice)
        .collect::<Vec<_>>();
    documents
        .try_reserve_exact(1)
        .map_err(|_| ZkX509MainAssemblyErrorV1::Resource)?;
    documents.push(&witness.crl_der);
    let der_base = build_zk_x509_der_stark_base_v1(&documents)?;
    let rfc_base = build_zk_x509_rfc5280_stark_base_material_v1(&rfc_trace)?;
    let projection_trace = build_zk_x509_projection_trace_v1(
        statement,
        &projection_witness_v1(statement, &rfc_trace, witness)?,
    )?;
    let root = rfc_trace
        .certificates
        .last()
        .ok_or(ZkX509MainAssemblyErrorV1::Source)?;
    let root_spki_der: [u8; ZK_X509_CA_SPKI_DER_BYTES_V1] = root
        .spki_der
        .as_slice()
        .try_into()
        .map_err(|_| ZkX509MainAssemblyErrorV1::Source)?;
    let ca_accumulator_trace = build_ca_accumulator_trace_v1(
        ZkX509CaAccumulatorStatementV1 {
            governed_root: *statement.ca_membership_root.as_bytes(),
        },
        ZkX509CaAccumulatorWitnessV1 {
            root_spki_der,
            path: witness.ca_membership_path,
        },
    )?;
    let sha_schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
        disclosed_attributes: statement.disclosed_attributes.len(),
    })?;
    let sha_witnesses = build_sha_witnesses_v1(
        statement,
        governance,
        witness,
        &rfc_trace,
        &projection_trace,
        &ca_accumulator_trace,
        &sha_schedule,
    )?;
    let (p256_witnesses, p256_materials, optional_certificate_selection) =
        build_p256_material_v1(witness, &rfc_trace, &sha_witnesses)?;
    let io = build_io_material_v1(
        &io_plan,
        &rfc_trace,
        &projection_trace,
        statement.disclosed_attributes.len(),
    )?;
    Ok(ZkX509MainTraceAssemblyV1 {
        relation_output,
        rfc_trace,
        der_base,
        rfc_base,
        projection_trace,
        ca_accumulator_trace,
        sha_schedule,
        sha_witnesses,
        p256_witnesses,
        p256_materials,
        optional_certificate_selection,
        io,
        verifier_profile,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::privacy_engines::zk_x509::{
        main_io::tests::statement_with_disclosures_v1, relation::tests::fixture,
    };
    fn synthetic_witnesses_for_plan_v1(
        plan: &ZkX509MainIoDeclarationsV1,
    ) -> Vec<ZkX509IoChannelWitnessV1> {
        plan.declarations
            .iter()
            .map(|declaration| {
                let byte_len =
                    usize::try_from(declaration.byte_len).expect("test byte length fits usize");
                let value = declaration.public_value.clone().unwrap_or_else(|| {
                    vec![
                        u8::try_from(declaration.channel)
                            .expect("test channel fits u8")
                            .wrapping_add(1);
                        byte_len
                    ]
                });
                ZkX509IoChannelWitnessV1 {
                    declaration: declaration.clone(),
                    producer_value: value.clone(),
                    consumer_values: vec![value; declaration.consumers.len()],
                }
            })
            .collect()
    }
    #[test]
    fn verifier_plan_and_witness_declarations_match_for_every_disclosure_count() {
        for disclosures in 0..=iroha_data_model::privacy::ZK_X509_MAX_DISCLOSED_ATTRIBUTES_V1 {
            let statement = statement_with_disclosures_v1(disclosures);
            let plan = compile_zk_x509_main_io_declarations_v1(&statement)
                .expect("valid verifier-owned plan");
            let witnesses = synthetic_witnesses_for_plan_v1(&plan);
            plan.validate_witness_declarations_v1(&witnesses)
                .expect("exact witness declarations");
            let (declarations, execution, sorted) =
                build_zk_x509_io_base_tables_v1(&witnesses).expect("valid synthetic base tables");
            assert_eq!(declarations, plan.declarations);
            assert_eq!(execution.len(), plan.logical_active_rows);
            assert_eq!(sorted.len(), plan.logical_active_rows);
        }
    }
    #[test]
    fn verifier_plan_rejects_reordered_missing_extra_and_mutated_witness_metadata() {
        let statement = statement_with_disclosures_v1(4);
        let plan =
            compile_zk_x509_main_io_declarations_v1(&statement).expect("valid verifier-owned plan");
        let canonical = synthetic_witnesses_for_plan_v1(&plan);
        let mut mutations = Vec::new();
        let mut changed = canonical.clone();
        changed.swap(0, 1);
        mutations.push(changed);
        let mut changed = canonical.clone();
        changed.pop();
        mutations.push(changed);
        let mut changed = canonical.clone();
        changed.push(canonical[0].clone());
        mutations.push(changed);
        let mut changed = canonical.clone();
        changed[0].declaration.channel += 1;
        mutations.push(changed);
        let mut changed = canonical.clone();
        changed[0].declaration.producer.role = ZkX509IoSegmentRoleV1::Sha256;
        mutations.push(changed);
        let mut changed = canonical.clone();
        changed[0].declaration.consumers[0].role = ZkX509IoSegmentRoleV1::P256;
        mutations.push(changed);
        let mut changed = canonical.clone();
        changed[0].declaration.byte_len += 1;
        mutations.push(changed);
        let mut changed = canonical.clone();
        changed
            .iter_mut()
            .find(|witness| witness.declaration.public_value.is_some())
            .expect("public witness")
            .declaration
            .public_value
            .as_mut()
            .expect("public value")[0] ^= 1;
        mutations.push(changed);
        for mutation in mutations {
            assert_eq!(
                plan.validate_witness_declarations_v1(&mutation),
                Err(ZkX509MainIoPlanErrorV1::Topology)
            );
        }
    }
    #[test]
    fn complete_main_assembly_scrub_is_recursive_idempotent_and_preserves_public_topology() {
        let fixture = fixture();
        if construct_zk_x509_main_verifier_profile_v1().is_err() {
            assert!(matches!(
                build_zk_x509_main_trace_assembly_v1(
                    &fixture.statement,
                    fixture.governance(),
                    &fixture.witness,
                ),
                Err(ZkX509MainAssemblyErrorV1::Registration)
            ));
            return;
        }
        let mut assembly = build_zk_x509_main_trace_assembly_v1(
            &fixture.statement,
            fixture.governance(),
            &fixture.witness,
        )
        .expect("canonical MAIN assembly");
        assert!(!assembly.rfc_trace.private_is_zeroized_v1());
        assert!(!assembly.der_base.private_is_zeroized_v1());
        assert!(!assembly.rfc_base.private_is_zeroized_v1());
        assert!(!assembly.projection_trace.private_is_zeroized_v1());
        assert!(!assembly.ca_accumulator_trace.private_is_zeroized_v1());
        assert!(
            assembly
                .sha_witnesses
                .iter()
                .all(|witness| !witness.private_is_zeroized_v1())
        );
        assert!(
            assembly
                .p256_witnesses
                .iter()
                .all(|witness| !witness.private_is_zeroized_v1())
        );
        assert!(
            assembly
                .p256_materials
                .iter()
                .all(|material| !material.private_is_zeroized_v1())
        );
        assert!(!assembly.io.witnesses.is_empty());
        assert!(!assembly.io.declarations.is_empty());
        assert!(!assembly.io.execution.is_empty());
        assert!(!assembly.io.sorted.is_empty());
        assert_eq!(assembly.io.logical_active_rows, assembly.io.execution.len());
        assert_eq!(assembly.io.logical_active_rows, assembly.io.sorted.len());
        // Poison top-level byte leaves, including otherwise-empty optional
        // preimages, so the test cannot pass merely because a fixture field
        // happened to start at zero.
        assembly
            .relation_output
            .ownership_challenge_digest
            .fill(0xa5);
        for witness in &mut assembly.sha_witnesses {
            if witness.message.is_empty() {
                witness.message.push(0xa5);
            } else {
                witness.message.fill(0xa5);
            }
            witness.digest.fill(0xa5);
        }
        for witness in &mut assembly.p256_witnesses {
            witness.public_key_x_be.fill(0xa5);
            witness.public_key_y_be.fill(0xa5);
            witness.r_be.fill(0xa5);
            witness.s_be.fill(0xa5);
            witness.digest_be.fill(0xa5);
        }
        for witness in [
            &mut assembly.optional_certificate_selection.real,
            &mut assembly.optional_certificate_selection.selected,
        ] {
            witness.public_key_x_be.fill(0xa5);
            witness.public_key_y_be.fill(0xa5);
            witness.r_be.fill(0xa5);
            witness.s_be.fill(0xa5);
            witness.digest_be.fill(0xa5);
        }
        assembly.optional_certificate_selection.active = F::ONE;
        for witness in &mut assembly.io.witnesses {
            witness.producer_value.fill(0xa5);
            for value in &mut witness.consumer_values {
                value.fill(0xa5);
            }
            if let Some(value) = &mut witness.declaration.public_value {
                value.fill(0xa5);
            }
        }
        for declaration in &mut assembly.io.declarations {
            if let Some(value) = &mut declaration.public_value {
                value.fill(0xa5);
            }
        }
        for access in assembly
            .io
            .execution
            .iter_mut()
            .chain(&mut assembly.io.sorted)
        {
            access.channel = F(0xa5);
            access.offset = F(0xa5);
            access.value = F(0xa5);
            access.is_write = F::ONE;
        }
        let public_relation = (
            assembly.relation_output.subject_public_key_digest,
            assembly.relation_output.certificate_nullifier,
        );
        let rfc_statement = assembly.rfc_trace.statement.clone();
        let rfc_schedule = assembly.rfc_base.schedule.clone();
        let projection_fixed = assembly.projection_trace.fixed.clone();
        let ca_statement = assembly.ca_accumulator_trace.statement;
        let sha_schedule = assembly.sha_schedule.clone();
        let p256_topology = assembly
            .p256_materials
            .each_ref()
            .map(|material| (material.role, material.assigned));
        let verifier_profile = assembly.verifier_profile;
        let io_logical_active_rows = assembly.io.logical_active_rows;
        assembly.zeroize_private_v1();
        assert!(assembly.private_is_zeroized_v1());
        assembly.zeroize_private_v1();
        assert!(assembly.private_is_zeroized_v1());
        assert_eq!(
            (
                assembly.relation_output.subject_public_key_digest,
                assembly.relation_output.certificate_nullifier,
            ),
            public_relation
        );
        assert_eq!(assembly.rfc_trace.statement, rfc_statement);
        assert_eq!(assembly.rfc_base.schedule, rfc_schedule);
        assert_eq!(assembly.projection_trace.fixed, projection_fixed);
        assert_eq!(assembly.ca_accumulator_trace.statement, ca_statement);
        assert_eq!(assembly.sha_schedule, sha_schedule);
        assert_eq!(
            assembly
                .p256_materials
                .each_ref()
                .map(|material| (material.role, material.assigned)),
            p256_topology
        );
        assert_eq!(assembly.verifier_profile, verifier_profile);
        assert_eq!(assembly.io.logical_active_rows, io_logical_active_rows);
        assert_eq!(
            construct_zk_x509_main_verifier_profile_v1()
                .expect("public verifier profile remains reconstructible"),
            verifier_profile
        );
    }
}
