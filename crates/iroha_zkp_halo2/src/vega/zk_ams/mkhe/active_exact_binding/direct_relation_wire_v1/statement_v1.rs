use super::super::super::{
    ZkAmsMkheErrorV1,
    direct_collective_eval_ceremony::{
        ZkAmsMkheDirectCeremonyContextV1,
        direct_relation_contribution_statement_from_polynomials_v1,
    },
    direct_object_transport::{ZkAmsMkheDirectObjectKindV1, ZkAmsMkheDirectObjectPointerV1},
};
use super::super::{PersistentDirectRelationV1, VerifiedPersistentWitnessDirectRelationUseV1};
use super::{
    DIRECT_RELATION_CODEC_VERSION_V1, EXACT_POLYNOMIAL_OBJECT_BYTES_V1, FINAL_STATEMENT_DOMAIN_V1,
    MAX_STATEMENT_BYTES_V1, RELATION_CORE_DOMAIN_V1, RELATION_LINEAGE_DOMAIN_V1,
};
use crate::vega::sponge::Keccak256;
use core::marker::PhantomData;
#[path = "statement_v1/galois_b_replay_v1.rs"]
mod galois_b_replay_v1;
#[allow(
    unused_imports,
    reason = "candidate-only Galois statement replay seam is retained for the pending semantic verifier and cannot mint admission or release authority"
)]
pub(in super::super) use galois_b_replay_v1::DirectGaloisBStatementReplayV1;
#[path = "statement_v1/rkg_one_h0_h1_replay_v1.rs"]
mod rkg_one_h0_h1_replay_v1;
pub(in super::super) use rkg_one_h0_h1_replay_v1::DirectRkgOneH0H1StatementReplayV1;
#[path = "statement_v1/rkg_one_creator_core_v1.rs"]
mod rkg_one_creator_core_v1;
pub(in crate::vega::zk_ams::mkhe) use rkg_one_creator_core_v1::PreparedDirectRkgOneStatementCoreV1;
use rkg_one_creator_core_v1::build_statement_core_v1;
pub(in crate::vega::zk_ams::mkhe) mod object_role {
    use super::ZkAmsMkheDirectObjectKindV1;
    pub(in crate::vega::zk_ams::mkhe) trait Sealed {
        const KIND: ZkAmsMkheDirectObjectKindV1;
    }
}
macro_rules! direct_object_role {
    ($name:ident, $kind:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(in crate::vega::zk_ams::mkhe) enum $name {}
        impl object_role::Sealed for $name {
            const KIND: ZkAmsMkheDirectObjectKindV1 = ZkAmsMkheDirectObjectKindV1::$kind;
        }
    };
}
direct_object_role!(RkgH0ObjectRoleV1, RkgH0);
direct_object_role!(RkgH1ObjectRoleV1, RkgH1);
direct_object_role!(RkgKObjectRoleV1, RkgK);
direct_object_role!(RkgNormalizationObjectRoleV1, RkgNormalization);
direct_object_role!(GaloisBObjectRoleV1, GaloisB);
direct_object_role!(AggregateH0ObjectRoleV1, AggregateH0);
direct_object_role!(AggregateH1ObjectRoleV1, AggregateH1);
/// A role-typed public polynomial statement and its exact content address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::vega::zk_ams::mkhe) struct DirectPolynomialObjectV1<R: object_role::Sealed> {
    statement_digest: [u8; 32],
    pointer: ZkAmsMkheDirectObjectPointerV1,
    role: PhantomData<fn() -> R>,
}
impl<R: object_role::Sealed> DirectPolynomialObjectV1<R> {
    pub(in crate::vega::zk_ams::mkhe) fn new(
        statement_digest: [u8; 32],
        pointer: ZkAmsMkheDirectObjectPointerV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        if statement_digest == [0; 32]
            || pointer.kind() != R::KIND
            || pointer.payload_bytes() != EXACT_POLYNOMIAL_OBJECT_BYTES_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        ZkAmsMkheDirectObjectPointerV1::decode_exact(R::KIND, &pointer.encode())?;
        Ok(Self {
            statement_digest,
            pointer,
            role: PhantomData,
        })
    }
    const fn entry(self) -> CanonicalObjectEntryV1 {
        CanonicalObjectEntryV1 {
            statement_digest: self.statement_digest,
            pointer: self.pointer,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CanonicalObjectEntryV1 {
    statement_digest: [u8; 32],
    pointer: ZkAmsMkheDirectObjectPointerV1,
}
/// Exact role-shaped set of separately addressed public polynomials.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::vega::zk_ams::mkhe) enum DirectRelationPublicObjectsV1 {
    RkgRoundOne {
        h0: DirectPolynomialObjectV1<RkgH0ObjectRoleV1>,
        h1: DirectPolynomialObjectV1<RkgH1ObjectRoleV1>,
    },
    RkgRoundTwo {
        aggregate_h0: DirectPolynomialObjectV1<AggregateH0ObjectRoleV1>,
        aggregate_h1: DirectPolynomialObjectV1<AggregateH1ObjectRoleV1>,
        k: DirectPolynomialObjectV1<RkgKObjectRoleV1>,
    },
    RkgNormalize {
        aggregate_h1: DirectPolynomialObjectV1<AggregateH1ObjectRoleV1>,
        normalization: DirectPolynomialObjectV1<RkgNormalizationObjectRoleV1>,
    },
    Galois {
        b: DirectPolynomialObjectV1<GaloisBObjectRoleV1>,
    },
}
impl DirectRelationPublicObjectsV1 {
    const fn relation(self) -> PersistentDirectRelationV1 {
        match self {
            Self::RkgRoundOne { .. } => PersistentDirectRelationV1::RkgRoundOne,
            Self::RkgRoundTwo { .. } => PersistentDirectRelationV1::RkgRoundTwo,
            Self::RkgNormalize { .. } => PersistentDirectRelationV1::RkgNormalize,
            Self::Galois { .. } => PersistentDirectRelationV1::Galois,
        }
    }
    fn entries(self) -> ([Option<CanonicalObjectEntryV1>; 3], usize) {
        match self {
            Self::RkgRoundOne { h0, h1 } => ([Some(h0.entry()), Some(h1.entry()), None], 2),
            Self::RkgRoundTwo {
                aggregate_h0,
                aggregate_h1,
                k,
            } => (
                [
                    Some(aggregate_h0.entry()),
                    Some(aggregate_h1.entry()),
                    Some(k.entry()),
                ],
                3,
            ),
            Self::RkgNormalize {
                aggregate_h1,
                normalization,
            } => (
                [
                    Some(aggregate_h1.entry()),
                    Some(normalization.entry()),
                    None,
                ],
                2,
            ),
            Self::Galois { b } => ([Some(b.entry()), None, None], 1),
        }
    }
    fn local_polynomial_digests(self) -> ([[u8; 32]; 2], usize) {
        match self {
            Self::RkgRoundOne { h0, h1 } => ([h0.statement_digest, h1.statement_digest], 2),
            Self::RkgRoundTwo { k, .. } => ([k.statement_digest, [0; 32]], 1),
            Self::RkgNormalize { normalization, .. } => {
                ([normalization.statement_digest, [0; 32]], 1)
            }
            Self::Galois { b } => ([b.statement_digest, [0; 32]], 1),
        }
    }
}
pub(in super::super) struct ExpectedDirectRelationStatementV1 {
    bytes: [u8; MAX_STATEMENT_BYTES_V1],
    bytes_len: usize,
    relation: PersistentDirectRelationV1,
    core_digest: [u8; 32],
    statement_digest: [u8; 32],
    lineage_digest: [u8; 32],
}
impl ExpectedDirectRelationStatementV1 {
    #[cfg(test)]
    pub(super) fn layout_fixture(
        relation: PersistentDirectRelationV1,
        statement_digest: [u8; 32],
    ) -> Self {
        Self {
            bytes: [0; MAX_STATEMENT_BYTES_V1],
            bytes_len: relation.statement_bytes(),
            relation,
            core_digest: [1; 32],
            statement_digest,
            lineage_digest: [2; 32],
        }
    }
    pub(in super::super) fn new(
        context: ZkAmsMkheDirectCeremonyContextV1,
        capability: &VerifiedPersistentWitnessDirectRelationUseV1,
        objects: DirectRelationPublicObjectsV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        capability.validate()?;
        let selector = capability.selector;
        selector.validate()?;
        if objects.relation() != selector.relation
            || selector.context_digest != context.digest()
            || selector.evaluated_key_ordinal != context.evaluated_key_ordinal()
            || selector.digit_index != context.digit_index()
            || selector.galois_exponent != context.galois_exponent()
            || capability.binding_set_root != context.secret_lineage_root()
            || capability.collective_public_key_digest != context.collective_public_key_digest()
            || context.direct_secret_lineage_digest(capability.party_index)?
                != capability.secret_identity_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        validate_object_selector_axes(objects, capability)?;
        let (local_digests, local_count) = objects.local_polynomial_digests();
        let contribution = direct_relation_contribution_statement_from_polynomials_v1(
            context,
            selector.relation.ceremony_round(),
            selector.prior_round_digest,
            capability.party_index,
            capability.party,
            &local_digests[..local_count],
        )?;
        if contribution != selector.contribution_statement_digest {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let relation = selector.relation;
        let bytes_len = relation.statement_bytes();
        let mut bytes = [0_u8; MAX_STATEMENT_BYTES_V1];
        let core_end = build_statement_core_v1(
            &mut bytes,
            context,
            relation,
            selector.prior_round_digest,
            capability.ephemeral_commitments.is_some(),
            objects,
            |output| {
                put(output, 212, &capability.binding_set_root);
                put(output, 244, &capability.collective_public_key_digest);
                output[276] = capability.party_index;
                output[277] = selector.evaluated_key_ordinal;
                output[278] = selector.digit_index;
                put(output, 280, &selector.galois_exponent.to_be_bytes());
                put(
                    output,
                    284,
                    &capability.ephemeral_record_index.to_be_bytes(),
                );
                put(output, 288, &capability.party.to_bytes());
                put(output, 320, &capability.secret_identity_digest);
                put(output, 352, &capability.ephemeral_identity_digest);
                put(output, 384, &capability.ephemeral_source_context_digest);
                put(output, 416, &capability.ephemeral_source_statement_digest);
                put(output, 448, &selector.common_a_statement_digest);
                put(output, 480, &selector.target_a_statement_digest);
                put(output, 512, &selector.contribution_statement_digest);
                Ok(())
            },
        )?;
        put(&mut bytes, core_end, &capability.use_digest);
        put(
            &mut bytes,
            core_end + 32,
            &selector.proof_commitment_transcript_digest,
        );
        if core_end + 64 != bytes_len {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let core_digest = domain_hash(RELATION_CORE_DOMAIN_V1, &bytes[..core_end]);
        let statement_digest = domain_hash(FINAL_STATEMENT_DOMAIN_V1, &bytes[..bytes_len]);
        let lineage_digest = relation_lineage_digest(capability)?;
        if core_digest == [0; 32] || statement_digest == [0; 32] || lineage_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(Self {
            bytes,
            bytes_len,
            relation,
            core_digest,
            statement_digest,
            lineage_digest,
        })
    }
    pub(in super::super) fn bytes(&self) -> &[u8] {
        &self.bytes[..self.bytes_len]
    }
    pub(in super::super) const fn relation(&self) -> PersistentDirectRelationV1 {
        self.relation
    }
    pub(in super::super) const fn core_digest(&self) -> [u8; 32] {
        self.core_digest
    }
    pub(in super::super) const fn statement_digest(&self) -> [u8; 32] {
        self.statement_digest
    }
    pub(in super::super) const fn lineage_digest(&self) -> [u8; 32] {
        self.lineage_digest
    }
}
fn validate_object_selector_axes(
    objects: DirectRelationPublicObjectsV1,
    capability: &VerifiedPersistentWitnessDirectRelationUseV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    let selector = capability.selector;
    let valid = match objects {
        DirectRelationPublicObjectsV1::RkgRoundOne { .. } => {
            selector.relation == PersistentDirectRelationV1::RkgRoundOne
        }
        DirectRelationPublicObjectsV1::RkgRoundTwo {
            aggregate_h0,
            aggregate_h1,
            ..
        } => {
            selector.relation == PersistentDirectRelationV1::RkgRoundTwo
                && aggregate_h0.statement_digest == selector.aggregate_h0_statement_digest
                && aggregate_h1.statement_digest == selector.aggregate_h1_statement_digest
        }
        DirectRelationPublicObjectsV1::RkgNormalize { aggregate_h1, .. } => {
            selector.relation == PersistentDirectRelationV1::RkgNormalize
                && aggregate_h1.statement_digest == selector.aggregate_h1_statement_digest
        }
        DirectRelationPublicObjectsV1::Galois { .. } => {
            selector.relation == PersistentDirectRelationV1::Galois
        }
    };
    if valid {
        Ok(())
    } else {
        Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    }
}
fn relation_lineage_digest(
    capability: &VerifiedPersistentWitnessDirectRelationUseV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    capability.validate()?;
    let mut hash = Keccak256::new();
    hash.update(RELATION_LINEAGE_DOMAIN_V1);
    hash.update(&[
        DIRECT_RELATION_CODEC_VERSION_V1,
        capability.selector.relation as u8,
    ]);
    hash.update(&capability.binding_set_root);
    hash.update(&capability.secret_identity_digest);
    hash.update(&capability.secret_commitment_set_digest);
    hash.update(&[u8::from(capability.ephemeral_commitments.is_some())]);
    hash.update(&capability.ephemeral_identity_digest);
    hash.update(&capability.ephemeral_commitment_set_digest);
    hash.update(&capability.ephemeral_source_context_digest);
    hash.update(&capability.ephemeral_source_statement_digest);
    hash.update(&capability.ephemeral_record_index.to_be_bytes());
    Ok(hash.finalize())
}
fn domain_hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(domain);
    hash.update(&[DIRECT_RELATION_CODEC_VERSION_V1]);
    hash.update(bytes);
    hash.finalize()
}
#[cfg(test)]
pub(super) fn domain_hash_for_test(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    domain_hash(domain, bytes)
}
fn put<const N: usize>(bytes: &mut [u8], offset: usize, value: &[u8; N]) {
    bytes[offset..offset + N].copy_from_slice(value);
}
