//! Sole portable untrusted ordinary public State carrier and canonical codec.
//! Decoding these data never creates Native ownership, a current clock/FI loan or proof admission.
//! Stateful lineage admission and Native-candidate projections retain their genuine Unix boundary.
#[cfg(test)]
use super::KagemushaPastaParityV1;
use super::{
    DigestV1, KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1, KagemushaPairedProofV1,
    state_relation::{RECURSIVE_SEMANTIC_PUBLIC_INSTANCE_COUNT, public_instance as s},
};
use crate::kagemusha_v1_poseidon::decode;
#[cfg(test)]
use crate::kagemusha_v1_poseidon::from_u128;
use ff::PrimeField;
use halo2_proofs::halo2curves::pasta::{Fp, Fq};
use norito::codec::{Decode, Encode};

const CELLS: usize = RECURSIVE_SEMANTIC_PUBLIC_INSTANCE_COUNT;
const PROJECTION_MAX: usize = 16 * 1024;
type Column = [[u8; 32]; CELLS];
type Result<T> = core::result::Result<T, String>;

/// Public, data-only State projection. Each parity has exactly93 canonical field encodings.
/// Only cells30/31 are parity-native full field elements; all others are shared128-bit cells.
/// This record has no constructor that admits a proof or grants a Native financial capability.
#[derive(Clone, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_recursion::OrdinaryLineageStateProjectionV1",
    frame = "iroha.kagemusha.core.v1.ordinary-lineage-state-projection"
)]
pub struct KagemushaOrdinaryLineageStateProjectionV1 {
    pub(super) version: u16,
    pub(super) eq: Column,
    pub(super) ep: Column,
}
impl KagemushaOrdinaryLineageStateProjectionV1 {
    /// Encode the actual already-admitted Native candidate's public projection, without secrets.
    #[cfg(unix)]
    pub(crate) fn from_admitted_candidate(
        candidate: &super::KagemushaAuthenticatedOrdinaryCashCandidateV1,
    ) -> Result<Self> {
        let p = candidate.public_inputs();
        Self::from_fields(
            p.recursive_semantic_public_instances::<Fp>()?,
            p.recursive_semantic_public_instances::<Fq>()?,
        )
    }
    pub(super) fn from_fields(eq: Vec<Fp>, ep: Vec<Fq>) -> Result<Self> {
        if eq.len() != CELLS || ep.len() != CELLS {
            return reject();
        }
        let value = Self {
            version: 1,
            eq: core::array::from_fn(|i| eq[i].to_repr()),
            ep: core::array::from_fn(|i| ep[i].to_repr()),
        };
        value.fields()?;
        Ok(value)
    }
    /// Strict bounded decoder for public data; it performs no proof or authority admission.
    /// # Errors
    /// Refuses oversized, noncanonical, substituted parity or scalar encodings.
    pub fn decode_original(original: &[u8]) -> Result<Self> {
        if original.is_empty() || original.len() > PROJECTION_MAX {
            return reject();
        }
        let value: Self = norito::decode_canonical_with_limits(
            original,
            norito::canonical_decode_limits(original.len()),
        )
        .map_err(|e| e.to_string())?;
        if value.canonical_bytes()? != original {
            return reject();
        }
        Ok(value)
    }
    /// Encode the sole bounded public data original; this does not authenticate the projection.
    /// # Errors
    /// Refuses invalid scalar/pair shape or a canonical codec failure.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        self.fields()?;
        let raw = norito::encode_canonical(self).map_err(|e| e.to_string())?;
        if raw.len() > PROJECTION_MAX {
            return reject();
        }
        Ok(raw)
    }
    pub(super) fn fields(&self) -> Result<(Vec<Fp>, Vec<Fq>)> {
        if self.version != 1 {
            return reject();
        }
        let eq = self
            .eq
            .iter()
            .map(|b| decode::<Fp>(*b).ok_or_else(rejection))
            .collect::<Result<Vec<_>>>()?;
        let ep = self
            .ep
            .iter()
            .map(|b| decode::<Fq>(*b).ok_or_else(rejection))
            .collect::<Result<Vec<_>>>()?;
        for i in 0..CELLS {
            if matches!(i, s::PREDECESSOR_STATE | s::SUCCESSOR_STATE) {
                continue;
            }
            if self.eq[i] != self.ep[i] || self.eq[i][16..].iter().any(|b| *b != 0) {
                return reject();
            }
        }
        for (column, before, after) in [
            (
                &self.eq,
                s::PREDECESSOR_EQ_COMPONENT_LO,
                s::SUCCESSOR_EQ_COMPONENT_LO,
            ),
            (
                &self.ep,
                s::PREDECESSOR_EP_COMPONENT_LO,
                s::SUCCESSOR_EP_COMPONENT_LO,
            ),
        ] {
            if digest_at(column, before)? != column[s::PREDECESSOR_STATE]
                || digest_at(column, after)? != column[s::SUCCESSOR_STATE]
            {
                return reject();
            }
        }
        Ok((eq, ep))
    }
    pub(super) fn digest(&self, low: usize) -> Result<DigestV1> {
        digest_at(&self.eq, low)
    }
    pub(super) fn integer(&self, at: usize) -> Result<u128> {
        if at >= CELLS || self.eq[at][16..].iter().any(|b| *b != 0) {
            return reject();
        }
        Ok(u128::from_le_bytes(
            self.eq[at][..16].try_into().map_err(|_| rejection())?,
        ))
    }
    pub(super) fn require_digest(&self, low: usize, expected: DigestV1) -> Result<()> {
        if self.digest(low)? != expected {
            return reject();
        }
        Ok(())
    }
}

/// Full public paired State original: exact semantic projection and both actual outer proofs/history.
/// This is an untrusted canonical data carrier, never a decoded Native owner or DATA authority.
#[derive(Clone, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_recursion::OrdinaryLineageStateOriginalV1",
    frame = "iroha.kagemusha.core.v1.ordinary-lineage-state-original"
)]
pub struct KagemushaOrdinaryLineageStateOriginalV1 {
    pub(super) version: u16,
    pub(super) projection: KagemushaOrdinaryLineageStateProjectionV1,
    pub(super) proof: KagemushaPairedProofV1,
}
impl KagemushaOrdinaryLineageStateOriginalV1 {
    /// Data-only full original from the zero selection's already-verified public instance.
    /// The caller retains the actual Bootstrap owner; this does not convert its W into money.
    pub(crate) fn from_bootstrap_public_inputs(
        inputs: &super::KagemushaStateRelationPublicInputsV1,
        proof: &KagemushaPairedProofV1,
    ) -> Result<Self> {
        let projection = KagemushaOrdinaryLineageStateProjectionV1::from_fields(
            inputs.recursive_semantic_public_instances::<Fp>()?,
            inputs.recursive_semantic_public_instances::<Fq>()?,
        )?;
        let value = Self {
            version: 1,
            projection,
            proof: proof.clone(),
        };
        value.canonical_bytes()?;
        Ok(value)
    }

    /// Copy public data from the genuine already-admitted Native candidate, granting no new loan.
    #[cfg(unix)]
    pub(crate) fn from_admitted_candidate(
        candidate: &super::KagemushaAuthenticatedOrdinaryCashCandidateV1,
    ) -> Result<Self> {
        let value = Self {
            version: 1,
            projection: KagemushaOrdinaryLineageStateProjectionV1::from_admitted_candidate(
                candidate,
            )?,
            proof: candidate.proof().clone(),
        };
        value.canonical_bytes()?;
        Ok(value)
    }
    /// Sole bounded complete public original, including exact current proofs and full histories.
    /// # Errors
    /// Refuses malformed version/scalars/proof envelope or noncanonical encoding.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        if self.version != 1 {
            return reject();
        }
        self.projection.fields()?;
        self.proof
            .validate_shape_for_semantic_digest(self.proof.semantic_digest)
            .map_err(|e| e.to_string())?;
        let raw = norito::encode_canonical(self).map_err(|e| e.to_string())?;
        if raw.len() > 32 * 1024 {
            return reject();
        }
        Ok(raw)
    }
    /// Public data operands of the exact paired projection; no proof or owner admission.
    pub(crate) fn public_columns(&self) -> Result<(Vec<Fp>, Vec<Fq>)> {
        self.projection.fields()
    }
    /// Actual paired proof data contained in this original, still requiring genuine verification.
    pub(crate) fn proof(&self) -> &KagemushaPairedProofV1 {
        &self.proof
    }
    /// Sole encoder-derived structural descriptors of the nested projection and complete original.
    /// This method intentionally does not admit any State, proof, clock or Native custody.
    pub(super) fn canonical_field_grammars(
        &self,
    ) -> Result<(
        iroha_data_model::kagemusha::KagemushaOrdinaryCanonicalFieldStreamGrammarV1,
        iroha_data_model::kagemusha::KagemushaOrdinaryCanonicalFieldStreamGrammarV1,
    )> {
        fn bare<T: norito::NoritoSerialize>(v: &T) -> Result<Vec<u8>> {
            let mut bytes = Vec::new();
            norito::codec::encode_adaptive_into(v, &mut bytes).map_err(|e| e.to_string())?;
            Ok(bytes)
        }
        use iroha_data_model::kagemusha::KagemushaOrdinaryCanonicalFieldStreamGrammarV1 as Grammar;
        let projection = Grammar::from_sole_encoded_fields(
            &self.projection,
            vec![
                bare(&self.projection.version)?,
                bare(&self.projection.eq)?,
                bare(&self.projection.ep)?,
            ],
        )?;
        let original = Grammar::from_sole_encoded_fields(
            self,
            vec![
                bare(&self.version)?,
                bare(&self.projection)?,
                bare(&self.proof)?,
            ],
        )?;
        Ok((projection, original))
    }
    /// Private deterministic codec padding, deliberately version0 and incapable of State admission.
    /// All active semantics and nested proof operands are assigned by the circuit, not by this data.
    pub(super) fn canonical_codec_padding() -> Self {
        Self {
            version: 0,
            projection: KagemushaOrdinaryLineageStateProjectionV1 {
                version: 0,
                eq: [[0; 32]; CELLS],
                ep: [[0; 32]; CELLS],
            },
            proof: KagemushaPairedProofV1 {
                version: 0,
                eq_protocol_digest: [0; 32],
                ep_protocol_digest: [0; 32],
                semantic_digest: [0; 32],
                guard_eq_credential_audit: [0; 32],
                guard_ep_credential_audit: [0; 32],
                eq_deferred_audit: [0; 32],
                ep_deferred_audit: [0; 32],
                eq_proof: Vec::new(),
                ep_proof: Vec::new(),
                eq_history: vec![0; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
                ep_history: vec![0; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
            },
        }
    }

    /// Strict bounded data decoder; callers must still admit all actual proofs and original joins.
    /// # Errors
    /// Refuses oversized, noncanonical or malformed public originals.
    pub fn decode_original(raw: &[u8]) -> Result<Self> {
        if raw.is_empty() || raw.len() > 32 * 1024 {
            return reject();
        }
        let value: Self =
            norito::decode_canonical_with_limits(raw, norito::canonical_decode_limits(raw.len()))
                .map_err(|e| e.to_string())?;
        if value.canonical_bytes()? != raw {
            return reject();
        }
        Ok(value)
    }
}

fn digest_at(column: &Column, low: usize) -> Result<DigestV1> {
    if low + 1 >= CELLS
        || column[low][16..]
            .iter()
            .chain(&column[low + 1][16..])
            .any(|b| *b != 0)
    {
        return reject();
    }
    let mut digest = [0; 32];
    digest[..16].copy_from_slice(&column[low][..16]);
    digest[16..].copy_from_slice(&column[low + 1][..16]);
    Ok(digest)
}
fn rejection() -> String {
    "ordinary stateless lineage proof binding rejected".into()
}
fn reject<T>() -> Result<T> {
    Err(rejection())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ff::Field as _;
    #[test]
    fn public_projection_rejects_noncanonical_shared_and_parity_component_cells() {
        let value = KagemushaOrdinaryLineageStateProjectionV1::from_fields(
            vec![Fp::ZERO; CELLS],
            vec![Fq::ZERO; CELLS],
        )
        .unwrap();
        let raw = value.canonical_bytes().unwrap();
        assert!(KagemushaOrdinaryLineageStateProjectionV1::decode_original(&raw).is_ok());
        for change in 0..5 {
            let mut bad = value.clone();
            match change {
                0 => bad.version = 2,
                1 => bad.ep[4][0] = 1,
                2 => {
                    bad.eq[4][16] = 1;
                    bad.ep[4][16] = 1;
                }
                3 => bad.eq[s::PREDECESSOR_STATE][0] = 1,
                _ => bad.ep[s::SUCCESSOR_STATE] = [0xff; 32],
            }
            assert!(bad.fields().is_err());
        }
        let mut trailing = raw;
        trailing.push(0);
        assert!(KagemushaOrdinaryLineageStateProjectionV1::decode_original(&trailing).is_err());
        assert!(
            KagemushaOrdinaryLineageStateProjectionV1::decode_original(&vec![
                0;
                PROJECTION_MAX + 1
            ])
            .is_err()
        );
    }
}

#[cfg(test)]
mod parent_codec_tests {
    use super::*;
    use ff::PrimeField as _;
    use halo2_base::gates::RangeInstructions as _;
    #[test]
    fn ordinary_parent_codec_inventory_uses_sole_complete_schema_and_array_framing() {
        let padding = KagemushaOrdinaryLineageStateOriginalV1::canonical_codec_padding();
        assert!(padding.canonical_bytes().is_err());
        assert!(padding.public_columns().is_err());
        let (projection, full) = padding.canonical_field_grammars().unwrap();
        assert_eq!(projection.fields().len(), 3);
        assert_eq!(full.fields().len(), 3);
        for column in &projection.fields()[1..] {
            assert_eq!(column.len(), CELLS * 65);
            for cell in column.chunks_exact(65) {
                assert_eq!(cell[0], 64);
                for byte in cell[1..].chunks_exact(2) {
                    assert_eq!(byte, &[1, 0]);
                }
            }
        }
        let mut fields = full.fields().to_vec();
        fields.swap(0, 1);
        assert!(iroha_data_model::kagemusha::KagemushaOrdinaryCanonicalFieldStreamGrammarV1::from_sole_encoded_fields(&padding,fields).is_err());
    }
    // These are inert codec operands, never admitted as a current proof or Native State owner.
    fn outer_original_codec_fixture() -> KagemushaOrdinaryLineageStateOriginalV1 {
        let mut projection = KagemushaOrdinaryLineageStateProjectionV1 {
            version: 1,
            eq: core::array::from_fn(|i| from_u128::<Fp>((i + 1) as u128).to_repr()),
            ep: core::array::from_fn(|i| from_u128::<Fq>((i + 1) as u128).to_repr()),
        };
        for (c, before, after) in [
            (
                &mut projection.eq,
                s::PREDECESSOR_EQ_COMPONENT_LO,
                s::SUCCESSOR_EQ_COMPONENT_LO,
            ),
            (
                &mut projection.ep,
                s::PREDECESSOR_EP_COMPONENT_LO,
                s::SUCCESSOR_EP_COMPONENT_LO,
            ),
        ] {
            c[s::PREDECESSOR_STATE] = digest_at(c, before).unwrap();
            c[s::SUCCESSOR_STATE] = digest_at(c, after).unwrap();
        }
        projection.fields().unwrap();
        let proof = KagemushaPairedProofV1 {
            version: 1,
            eq_protocol_digest: projection.digest(s::EQ_PROTOCOL_LO).unwrap(),
            ep_protocol_digest: projection.digest(s::EP_PROTOCOL_LO).unwrap(),
            semantic_digest: projection.digest(s::TRANSPORT_LO).unwrap(),
            guard_eq_credential_audit: projection.digest(s::GUARD_EQ_CREDENTIAL_AUDIT_LO).unwrap(),
            guard_ep_credential_audit: projection.digest(s::GUARD_EP_CREDENTIAL_AUDIT_LO).unwrap(),
            eq_deferred_audit: projection.digest(s::EQ_DEFERRED_AUDIT_LO).unwrap(),
            ep_deferred_audit: projection.digest(s::EP_DEFERRED_AUDIT_LO).unwrap(),
            eq_proof: vec![0x21; 128],
            ep_proof: vec![0x42; 160],
            eq_history: vec![0x31; 544],
            ep_history: vec![0x53; 544],
        };
        KagemushaOrdinaryLineageStateOriginalV1 {
            version: 1,
            projection,
            proof,
        }
    }
    fn outer_original_stream_case<F: crate::kagemusha_v1_poseidon::KagemushaPoseidonFieldV1>(
        parity: KagemushaPastaParityV1,
        change: usize,
    ) -> bool {
        use super::super::ordinary_parent_canonical_stream::{
            OrdinaryOuterParentCanonicalSourcesV1, constrain_outer_parent_public_original_v1,
        };
        use crate::pasta_sha256::PastaSha256JobsV1;
        use halo2_base::{
            QuantumCell,
            gates::{GateInstructions as _, circuit::builder::BaseCircuitBuilder},
        };
        use halo2_proofs::dev::MockProver;
        const K: usize = 17;
        let value = outer_original_codec_fixture();
        let mut b = BaseCircuitBuilder::<F>::new(false)
            .use_k(K)
            .use_lookup_bits(16)
            .use_instance_columns(1);
        let range = b.range_chip();
        let ctx = b.main(0);
        let mut semantic = if parity == KagemushaPastaParityV1::Eq {
            value.projection.eq
        } else {
            value.projection.ep
        };
        let mut current = if parity == KagemushaPastaParityV1::Eq {
            value.proof.eq_proof.clone()
        } else {
            value.proof.ep_proof.clone()
        };
        let mut other = if parity == KagemushaPastaParityV1::Eq {
            value.proof.ep_proof.clone()
        } else {
            value.proof.eq_proof.clone()
        };
        let mut history = if parity == KagemushaPastaParityV1::Eq {
            value.proof.eq_history.clone()
        } else {
            value.proof.ep_history.clone()
        };
        let mut other_history = if parity == KagemushaPastaParityV1::Eq {
            value.proof.ep_history.clone()
        } else {
            value.proof.eq_history.clone()
        };
        match change {
            0 => {}
            1 => current[17] ^= 1,
            2 => other[31] ^= 1,
            3 => history[543] ^= 1,
            4 => other_history[543] ^= 1,
            5 => semantic[s::EQ_PROTOCOL_LO][0] ^= 1,
            6 => semantic[s::POLICY_EPOCH][0] ^= 1,
            _ => panic!("bounded mutation"),
        }
        let canonical = semantic
            .iter()
            .map(|bytes| {
                super::super::guard_bundle::assign_bytes(ctx, &range, bytes)
                    .try_into()
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let mut column = semantic
            .iter()
            .map(|bytes| ctx.load_witness(decode::<F>(*bytes).unwrap()))
            .collect::<Vec<_>>();
        column.extend(history.chunks_exact(16).map(|bytes| {
            ctx.load_witness(from_u128::<F>(u128::from_le_bytes(
                bytes.try_into().unwrap(),
            )))
        }));
        let current = super::super::guard_bundle::assign_bytes(ctx, &range, &current);
        let mut jobs = PastaSha256JobsV1::default();
        let (stream, _) = constrain_outer_parent_public_original_v1(
            ctx,
            &range,
            &mut jobs,
            OrdinaryOuterParentCanonicalSourcesV1 {
                column: &column,
                canonical_column: &canonical,
                canonical_current_original: &current,
                counterpart_current_original: &other,
                counterpart_history: &other_history,
                proof_widths: [128, 160],
                parity,
            },
            &value,
        )
        .unwrap();
        let mut raw = norito::encode_canonical(&value).unwrap();
        let len = raw.len();
        raw.resize(stream.bytes().len(), 0);
        if change == 0 {
            assert!(jobs.canonical_messages().is_err());
            assert_eq!(
                jobs.canonical_plan_messages().unwrap(),
                vec![crate::pasta_sha256::PastaSha256PlanMessageV1::Bounded {
                    logical_message: raw[..len].to_vec(),
                    capacity: stream.bytes().len(),
                    selected_block: (len + 9).div_ceil(64) - 1,
                    max_blocks: (stream.bytes().len() + 9).div_ceil(64),
                }]
            );
        }
        let mut public = vec![F::from(len as u64)];
        public.extend(raw.into_iter().map(|v| F::from(u64::from(v))));
        let mut cells = vec![stream.actual_len()];
        cells.extend(stream.bytes().iter().map(|v| {
            range
                .gate()
                .add(ctx, v.quantum_cell(), QuantumCell::Constant(F::ZERO))
        }));
        b.assigned_instances = vec![cells];
        b.calculate_params(Some(9));
        MockProver::run(K as u32, &b, vec![public])
            .unwrap()
            .verify()
            .is_ok()
    }
    #[test]
    fn outer_parent_original_stream_matches_sole_encoder_for_both_scalar_fields_and_parities() {
        for p in [KagemushaPastaParityV1::Eq, KagemushaPastaParityV1::Ep] {
            assert!(outer_original_stream_case::<Fp>(p, 0));
            assert!(outer_original_stream_case::<Fq>(p, 0));
        }
    }
    #[test]
    fn outer_parent_original_stream_rejects_changed_current_history_protocol_and_shared_cells() {
        for p in [KagemushaPastaParityV1::Eq, KagemushaPastaParityV1::Ep] {
            for mutation in 1..=6 {
                assert!(!outer_original_stream_case::<Fp>(p, mutation));
                assert!(!outer_original_stream_case::<Fq>(p, mutation));
            }
        }
    }
}
