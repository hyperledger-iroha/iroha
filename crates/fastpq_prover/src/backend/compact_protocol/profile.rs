//! Caller-fixed hash/transcript binding for the one compact arithmetic engine.
//!
//! Neither descriptor is production-qualified. The candidate fixes its complete
//! geometry; the proof cannot select a descriptor or change its query policy.

use super::*;
use crate::backend::{compact_shake_candidate as shake, merkle_multiproof::MultiproofPlan};

/// Internal descriptor selected by a trusted entry point, never decoded from a proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Protocol {
    /// Existing diagnostic profile retained for exact regression comparison.
    #[cfg(test)]
    Prototype,
    /// Fixed 375-query SHAKE candidate with complete context and whole tapes.
    ShakeCandidate,
}

impl Protocol {
    /// Exact initial position count under this fixed descriptor.
    pub(super) fn query_count(self, _domain: usize) -> usize {
        match self {
            #[cfg(test)]
            Self::Prototype => (FASTPQ_FINAL_V1.fri.queries as usize).min(_domain),
            Self::ShakeCandidate => 375,
        }
    }

    /// Reject any geometry outside the candidate theorem and tape schedule.
    pub(super) fn check_geometry(self, geometry: &Geometry) -> Result<()> {
        if self == Self::ShakeCandidate
            && (geometry.schema.trace_rows != 65_536
                || geometry.schema.width != 342
                || geometry.schema.constraints != 923
                || geometry.lde_rows != 524_288
                || FASTPQ_FINAL_V1.fri.arity != 2
                || FASTPQ_FINAL_V1.fri.blowup_factor != 8
                || geometry.fri_lengths != (0..=17).map(|r| 524_288 >> r).collect::<Vec<_>>()
                || geometry.terminal_degree != 1)
        {
            return Err(shape(
                "SHAKE candidate requires its exact fixed AIR/FRI geometry",
            ));
        }
        Ok(())
    }
}

#[derive(NoritoSerialize, norito::NoritoSchema)]
#[norito_schema(
    name = "fastpq_prover::backend::compact_protocol::profile::StatementContext",
    frame = "fastpq_prover::compact_candidate::ShakeEngineStatementV1"
)]
struct StatementContext {
    relation: String,
    trace_rows: u32,
    lde_rows: u32,
    width: u32,
    constraints: u32,
    base_modulus: u64,
    extension_nonresidue: u64,
    lde_root: u64,
    lde_log_size: u32,
    coset_offset: u64,
    blowup: u32,
    arity: u32,
    folds: u32,
    terminal_values: u32,
    terminal_degree: u32,
    queries: u32,
    statement: Vec<u8>,
}

/// Immutable binding reused for every oracle in a single proof context.
#[derive(Clone, Debug)]
pub(super) struct Binding {
    protocol: Protocol,
    shake: Option<shake::Context>,
}

impl Binding {
    /// Bind the exact fixed schema, algebraic domain and complete public bytes.
    /// This does not authenticate the caller's public statement or approve its AIR.
    pub(super) fn new(relation: &impl FixedAir, geometry: &Geometry) -> Result<Self> {
        if relation.schema() != geometry.schema {
            return Err(shape(
                "compact binding requires the geometry of this exact relation",
            ));
        }
        geometry.protocol.check_geometry(geometry)?;
        let shake = match geometry.protocol {
            Protocol::ShakeCandidate => {
                check_limit(
                    "max_shake_public_bytes",
                    relation.statement_bytes().len(),
                    256 * 1024,
                )?;
                let context = StatementContext {
                    relation: geometry.schema.identity.to_owned(),
                    trace_rows: geometry.schema.trace_rows as u32,
                    lde_rows: geometry.lde_rows as u32,
                    width: geometry.schema.width as u32,
                    constraints: geometry.schema.constraints as u32,
                    base_modulus: GOLDILOCKS_MODULUS,
                    extension_nonresidue: 7,
                    lde_root: FASTPQ_FINAL_V1.lde_root,
                    lde_log_size: FASTPQ_FINAL_V1.lde_log_size,
                    coset_offset: FASTPQ_FINAL_V1.omega_coset,
                    blowup: 8,
                    arity: 2,
                    folds: 17,
                    terminal_values: 4,
                    terminal_degree: 1,
                    queries: 375,
                    statement: relation.statement_bytes().to_vec(),
                };
                Some(
                    shake::Context::new(&norito::encode_canonical(&context)?)
                        .map_err(candidate_error)?,
                )
            }
            #[cfg(test)]
            Protocol::Prototype => None,
        };
        Ok(Self {
            protocol: geometry.protocol,
            shake,
        })
    }

    /// Hash a complete canonical row in its context and exact position.
    pub(super) fn row(&self, index: usize, values: &[u64]) -> Result<Digest> {
        match &self.shake {
            #[cfg(test)]
            None => hash_air_trace_row(index, values),
            #[cfg(not(test))]
            None => Err(shape("candidate binding is missing its fixed context")),
            Some(context) => {
                let bytes: Vec<_> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
                context
                    .hash_leaf(shake::Oracle::Row, coordinate(index)?, &bytes)
                    .map_err(candidate_error)
            }
        }
    }

    /// Hash one mixed-oracle extension value under its distinct role.
    pub(super) fn mixed(&self, index: usize, value: GoldilocksFp4V1) -> Result<Digest> {
        match &self.shake {
            #[cfg(test)]
            None => hash_lde_chunk_fp4(index, &[value]),
            #[cfg(not(test))]
            None => Err(shape("candidate binding is missing its fixed context")),
            Some(context) => context
                .hash_leaf(
                    shake::Oracle::Mixed,
                    coordinate(index)?,
                    &value.to_le_bytes(),
                )
                .map_err(candidate_error),
        }
    }

    /// Hash one quotient-oracle extension value under its distinct role.
    pub(super) fn quotient(&self, index: usize, value: GoldilocksFp4V1) -> Result<Digest> {
        match &self.shake {
            #[cfg(test)]
            None => hash_air_composition_leaf(index, value),
            #[cfg(not(test))]
            None => Err(shape("candidate binding is missing its fixed context")),
            Some(context) => context
                .hash_leaf(
                    shake::Oracle::Quotient,
                    coordinate(index)?,
                    &value.to_le_bytes(),
                )
                .map_err(candidate_error),
        }
    }

    /// Hash a strided binary FRI group or the one complete terminal leaf.
    pub(super) fn fri(
        &self,
        round: usize,
        index: usize,
        values: &[GoldilocksFp4V1],
    ) -> Result<Digest> {
        match &self.shake {
            #[cfg(test)]
            None => crate::backend::hash_fri_chunk(round, index, values),
            #[cfg(not(test))]
            None => Err(shape("candidate binding is missing its fixed context")),
            Some(context) => {
                let bytes: Vec<_> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
                context
                    .hash_leaf(fri_oracle(round)?, coordinate(index)?, &bytes)
                    .map_err(candidate_error)
            }
        }
    }

    /// Bind both complete children and their caller-derived tree coordinates.
    pub(super) fn parent(
        &self,
        role: MerkleTreeRoleV1,
        level: usize,
        index: usize,
        left: Digest,
        right: Digest,
    ) -> Result<Digest> {
        match &self.shake {
            #[cfg(test)]
            None => crate::backend::merkle_node_hash(role, level, index, left, right),
            #[cfg(not(test))]
            None => Err(shape("candidate binding is missing its fixed context")),
            Some(context) => context
                .hash_parent(
                    oracle(role)?,
                    coordinate(level)?,
                    coordinate(index)?,
                    left,
                    right,
                )
                .map_err(candidate_error),
        }
    }

    /// Authenticate one canonical frontier with unchanged plan/work accounting.
    pub(super) fn verify_tree(
        &self,
        plan: &MultiproofPlan,
        role: MerkleTreeRoleV1,
        root: Digest,
        leaves: &[Digest],
        siblings: &[Digest],
    ) -> Result<crate::backend::merkle_multiproof::MultiproofWork> {
        #[cfg(test)]
        if self.shake.is_none() {
            return plan.verify(role, root, leaves, siblings);
        }
        plan.verify_with(root, leaves, siblings, |level, index, left, right| {
            self.parent(role, level, index, left, right)
        })
    }

    /// Build a prover-owned tree with exact candidate geometry and ordered parents.
    pub(super) fn tree(&self, leaves: &[Digest], role: MerkleTreeRoleV1) -> Result<CommittedTree> {
        #[cfg(test)]
        if self.shake.is_none() {
            return CommittedTree::from_leaves(leaves, role);
        }
        let expected = match oracle(role)? {
            shake::Oracle::Row | shake::Oracle::Mixed | shake::Oracle::Quotient => 524_288,
            shake::Oracle::Fri(round @ 0..=16) => 524_288 >> (round + 1),
            shake::Oracle::Fri(17) => 1,
            _ => return Err(shape("invalid candidate tree role")),
        };
        if leaves.len() != expected {
            return Err(shape("candidate tree has another exact leaf count"));
        }
        let mut first = leaves.to_vec();
        if first.len() == 1 {
            first.push(first[0]);
        }
        let mut levels = vec![first];
        while levels.last().expect("first level exists").len() > 1 {
            let level = levels.len();
            let results: Vec<Result<Digest>> = levels
                .last()
                .unwrap()
                .par_chunks_exact(2)
                .enumerate()
                .map(|(index, pair)| self.parent(role, level, index, pair[0], pair[1]))
                .collect();
            levels.push(results.into_iter().collect::<Result<_>>()?);
        }
        Ok(CommittedTree {
            levels,
            leaf_count: leaves.len(),
        })
    }

    /// Start the matching fixed transcript and bind its first committed root.
    pub(super) fn transcript(
        &self,
        _relation: &impl FixedAir,
        geometry: &Geometry,
        row_root: Digest,
    ) -> Result<ProtocolTranscript> {
        let state = match &self.shake {
            #[cfg(test)]
            None => {
                TranscriptState::Prototype(initialise_transcript(_relation, geometry, row_root)?)
            }
            #[cfg(not(test))]
            None => return Err(shape("candidate binding is missing its fixed context")),
            Some(context) => {
                let mut transcript = shake::Transcript::new(context.clone());
                if transcript.challenge().map_err(candidate_error)? != shake::Message::Dummy {
                    return Err(shape(
                        "candidate requires its initial dummy verifier message",
                    ));
                }
                transcript.commit(row_root).map_err(candidate_error)?;
                TranscriptState::Shake(transcript)
            }
        };
        Ok(ProtocolTranscript {
            state,
            next_message: 2,
            schema: geometry.schema,
            domain: geometry.lde_rows,
            folds: geometry.fri_lengths.len() - 1,
            protocol: self.protocol,
        })
    }
}

fn coordinate(value: usize) -> Result<u32> {
    u32::try_from(value).map_err(|_| Error::QueryIndexOverflow { index: value })
}

fn fri_oracle(round: usize) -> Result<shake::Oracle> {
    let round = u8::try_from(round).map_err(|_| shape("candidate FRI ordinal overflows"))?;
    if round > 17 {
        return Err(shape("candidate FRI ordinal exceeds its fixed count"));
    }
    Ok(shake::Oracle::Fri(round))
}

fn oracle(role: MerkleTreeRoleV1) -> Result<shake::Oracle> {
    match role {
        MerkleTreeRoleV1::AirTrace => Ok(shake::Oracle::Row),
        MerkleTreeRoleV1::Lde => Ok(shake::Oracle::Mixed),
        MerkleTreeRoleV1::AirComposition => Ok(shake::Oracle::Quotient),
        MerkleTreeRoleV1::Fri(round) => fri_oracle(round as usize),
        _ => Err(shape("tree role is outside the fixed candidate")),
    }
}

fn candidate_error(error: shake::CandidateError) -> Error {
    Error::InvalidTraceShape {
        details: format!("SHAKE compact candidate: {error}"),
    }
}

enum TranscriptState {
    #[cfg(test)]
    Prototype(Transcript),
    Shake(shake::Transcript),
}

/// One root/challenge schedule shared by prover and verifier orchestration.
pub(super) struct ProtocolTranscript {
    state: TranscriptState,
    next_message: usize,
    schema: FixedAirSchema,
    domain: usize,
    folds: usize,
    protocol: Protocol,
}

impl ProtocolTranscript {
    fn expect(&self, ordinal: usize) -> Result<()> {
        if self.next_message != ordinal {
            return Err(shape("compact protocol transcript phase mismatch"));
        }
        Ok(())
    }

    fn fields(transcript: &mut shake::Transcript, count: usize) -> Result<Vec<GoldilocksFp4V1>> {
        match transcript.challenge().map_err(candidate_error)? {
            shake::Message::Fields(fields) if fields.len() == count => Ok(fields),
            _ => Err(shape("candidate verifier message has another field count")),
        }
    }

    /// Derive the complete column mixing vector after the row commitment.
    pub(super) fn columns(&mut self) -> Result<Vec<GoldilocksFp4V1>> {
        self.expect(2)?;
        let values = match &mut self.state {
            #[cfg(test)]
            TranscriptState::Prototype(t) => challenges(t, "compact:column-mix", self.schema.width),
            TranscriptState::Shake(t) => Self::fields(t, self.schema.width)?,
        };
        self.next_message = 3;
        Ok(values)
    }

    /// Bind the mixed root and derive every independent constraint coefficient.
    pub(super) fn alphas(&mut self, mixed_root: Digest) -> Result<Vec<GoldilocksFp4V1>> {
        self.expect(3)?;
        let values = match &mut self.state {
            #[cfg(test)]
            TranscriptState::Prototype(t) => {
                t.append_message("compact:mixed-root", &mixed_root.to_le_bytes());
                challenges(t, "compact:constraint-alpha", self.schema.constraints)
            }
            TranscriptState::Shake(t) => {
                t.commit(mixed_root).map_err(candidate_error)?;
                Self::fields(t, self.schema.constraints)?
            }
        };
        self.next_message = 4;
        Ok(values)
    }

    /// Bind the quotient root and derive the two joint-degree coefficients.
    pub(super) fn joint(&mut self, quotient_root: Digest) -> Result<JointFriBatch> {
        self.expect(4)?;
        let joint = match &mut self.state {
            #[cfg(test)]
            TranscriptState::Prototype(t) => {
                t.append_message("compact:quotient-root", &quotient_root.to_le_bytes());
                JointFriBatch::from_transcript(&FASTPQ_FINAL_V1, self.domain, t)?
            }
            TranscriptState::Shake(t) => {
                t.commit(quotient_root).map_err(candidate_error)?;
                let values = Self::fields(t, 2)?;
                JointFriBatch::from_challenges(&FASTPQ_FINAL_V1, self.domain, values[0], values[1])?
            }
        };
        self.next_message = 5;
        Ok(joint)
    }

    /// Bind exactly the next FRI root and return its one fold coefficient.
    pub(super) fn beta(&mut self, round: usize, root: Digest) -> Result<GoldilocksFp4V1> {
        if round >= self.folds {
            return Err(shape("compact transcript has an extra FRI fold"));
        }
        self.expect(5 + round)?;
        let beta = match &mut self.state {
            #[cfg(test)]
            TranscriptState::Prototype(t) => {
                t.append_fri_layer(round, root);
                t.challenge_beta(round)
            }
            TranscriptState::Shake(t) => {
                t.commit(root).map_err(candidate_error)?;
                Self::fields(t, 1)?[0]
            }
        };
        self.next_message += 1;
        Ok(beta)
    }

    /// Bind the terminal root and finish with the exact fixed query set.
    pub(super) fn queries(&mut self, terminal_root: Digest) -> Result<Vec<usize>> {
        self.expect(5 + self.folds)?;
        let indices: Vec<usize> = match &mut self.state {
            #[cfg(test)]
            TranscriptState::Prototype(t) => {
                t.append_fri_final(terminal_root);
                sample_queries(self.domain, FASTPQ_FINAL_V1.fri.queries as usize, t)?
            }
            TranscriptState::Shake(t) => {
                t.commit(terminal_root).map_err(candidate_error)?;
                match t.challenge().map_err(candidate_error)? {
                    shake::Message::Queries(indices) => {
                        indices.into_iter().map(|i| i as usize).collect()
                    }
                    _ => {
                        return Err(shape(
                            "candidate final message must contain query positions",
                        ));
                    }
                }
            }
        };
        if indices.len() != self.protocol.query_count(self.domain) {
            return Err(shape(
                "compact transcript produced another fixed query count",
            ));
        }
        self.next_message += 1;
        Ok(indices)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::merkle_multiproof::MultiproofLimits;

    struct StatementOnly {
        schema: FixedAirSchema,
        public: Vec<u8>,
    }

    impl FixedAir for StatementOnly {
        fn schema(&self) -> FixedAirSchema {
            self.schema
        }
        fn statement_bytes(&self) -> &[u8] {
            &self.public
        }
        fn evaluate(&self, _: u64, _: &[u64], _: &[u64]) -> Result<Vec<u64>> {
            panic!("binding/transcript tests must never construct or evaluate a witness")
        }
    }

    fn relation(protocol: Protocol) -> StatementOnly {
        StatementOnly {
            schema: match protocol {
                Protocol::Prototype => FixedAirSchema {
                    trace_rows: 16,
                    width: 1,
                    constraints: 1,
                    identity: "profile-binding:small-native:v1",
                },
                Protocol::ShakeCandidate => FixedAirSchema {
                    trace_rows: 65_536,
                    width: 342,
                    constraints: 923,
                    identity: "profile-binding:fixed-candidate:v1",
                },
            },
            public: b"complete public context without a private witness".to_vec(),
        }
    }

    fn root(value: u64) -> Digest {
        Digest::new([value; 6]).unwrap()
    }

    fn message_fields(message: shake::Message) -> Vec<GoldilocksFp4V1> {
        match message {
            shake::Message::Fields(values) => values,
            _ => panic!("complete field tape"),
        }
    }

    #[test]
    fn fixed_candidate_geometry_rejects_every_other_dimension_before_binding() {
        let air = relation(Protocol::ShakeCandidate);
        let geometry = Geometry::for_protocol(&air, Protocol::ShakeCandidate).unwrap();
        assert_eq!(Protocol::ShakeCandidate.query_count(geometry.lde_rows), 375);
        for change in [(32_768, 342, 923), (65_536, 341, 923), (65_536, 342, 922)] {
            let mut bad = relation(Protocol::ShakeCandidate);
            bad.schema.trace_rows = change.0;
            bad.schema.width = change.1;
            bad.schema.constraints = change.2;
            assert!(Geometry::for_protocol(&bad, Protocol::ShakeCandidate).is_err());
            assert!(Binding::new(&bad, &geometry).is_err());
        }
        let mut bad = Geometry::for_protocol(&air, Protocol::ShakeCandidate).unwrap();
        bad.fri_lengths[3] += 1;
        assert!(Protocol::ShakeCandidate.check_geometry(&bad).is_err());
        let native = relation(Protocol::Prototype);
        assert!(Geometry::for_protocol(&native, Protocol::ShakeCandidate).is_err());
        assert_eq!(Protocol::Prototype.query_count(128), 128);
        assert_eq!(Protocol::Prototype.query_count(512), 136);
    }

    #[test]
    fn native_transcript_adapter_preserves_every_original_message_and_query() {
        let air = relation(Protocol::Prototype);
        let geometry = Geometry::new(&air).unwrap();
        let binding = Binding::new(&air, &geometry).unwrap();
        let mut actual = binding.transcript(&air, &geometry, root(1)).unwrap();
        let mut original = initialise_transcript(&air, &geometry, root(1)).unwrap();
        assert_eq!(
            actual.columns().unwrap(),
            challenges(&mut original, "compact:column-mix", 1)
        );
        original.append_message("compact:mixed-root", &root(2).to_le_bytes());
        assert_eq!(
            actual.alphas(root(2)).unwrap(),
            challenges(&mut original, "compact:constraint-alpha", 1)
        );
        original.append_message("compact:quotient-root", &root(3).to_le_bytes());
        let a = actual.joint(root(3)).unwrap();
        let b = JointFriBatch::from_transcript(&FASTPQ_FINAL_V1, geometry.lde_rows, &mut original)
            .unwrap();
        for i in 0..geometry.lde_rows {
            assert_eq!(
                a.value_at(i, GoldilocksFp4V1::ONE, GoldilocksFp4V1::ONE)
                    .unwrap(),
                b.value_at(i, GoldilocksFp4V1::ONE, GoldilocksFp4V1::ONE)
                    .unwrap()
            );
        }
        for round in 0..geometry.fri_lengths.len() - 1 {
            let value = root(round as u64 + 4);
            original.append_fri_layer(round, value);
            assert_eq!(
                actual.beta(round, value).unwrap(),
                original.challenge_beta(round)
            );
        }
        original.append_fri_final(root(21));
        assert_eq!(
            actual.queries(root(21)).unwrap(),
            sample_queries(
                geometry.lde_rows,
                FASTPQ_FINAL_V1.fri.queries as usize,
                &mut original
            )
            .unwrap()
        );
    }

    #[test]
    fn candidate_adapter_preserves_whole_tapes_and_rejects_wrong_method_order() {
        let air = relation(Protocol::ShakeCandidate);
        let geometry = Geometry::for_protocol(&air, Protocol::ShakeCandidate).unwrap();
        let binding = Binding::new(&air, &geometry).unwrap();
        let mut actual = binding.transcript(&air, &geometry, root(1)).unwrap();
        let mut reference = shake::Transcript::new(binding.shake.clone().unwrap());
        assert_eq!(reference.challenge().unwrap(), shake::Message::Dummy);
        reference.commit(root(1)).unwrap();
        assert!(actual.alphas(root(2)).is_err());
        assert!(actual.beta(0, root(4)).is_err());
        assert_eq!(
            actual.columns().unwrap(),
            message_fields(reference.challenge().unwrap())
        );
        assert!(actual.columns().is_err());
        reference.commit(root(2)).unwrap();
        assert_eq!(
            actual.alphas(root(2)).unwrap(),
            message_fields(reference.challenge().unwrap())
        );
        reference.commit(root(3)).unwrap();
        let coefficients = message_fields(reference.challenge().unwrap());
        let a = actual.joint(root(3)).unwrap();
        let b = JointFriBatch::from_challenges(
            &FASTPQ_FINAL_V1,
            geometry.lde_rows,
            coefficients[0],
            coefficients[1],
        )
        .unwrap();
        for i in [0, 1, 7, 8, 524_287] {
            assert_eq!(
                a.value_at(i, GoldilocksFp4V1::ONE, GoldilocksFp4V1::ONE)
                    .unwrap(),
                b.value_at(i, GoldilocksFp4V1::ONE, GoldilocksFp4V1::ONE)
                    .unwrap()
            );
        }
        for round in 0..17 {
            assert!(actual.beta(round + 1, root(99)).is_err());
            let value = root(round as u64 + 4);
            reference.commit(value).unwrap();
            assert_eq!(
                actual.beta(round, value).unwrap(),
                message_fields(reference.challenge().unwrap())[0]
            );
        }
        reference.commit(root(21)).unwrap();
        let shake::Message::Queries(indices) = reference.challenge().unwrap() else {
            panic!("query tape");
        };
        let actual_indices = actual.queries(root(21)).unwrap();
        assert_eq!(
            actual_indices,
            indices.into_iter().map(|i| i as usize).collect::<Vec<_>>()
        );
        assert_eq!(actual_indices.len(), 375);
        assert!(actual_indices.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(actual.queries(root(21)).is_err());
    }

    #[test]
    fn full_context_and_relation_identity_bind_every_candidate_oracle() {
        let air = relation(Protocol::ShakeCandidate);
        let geometry = Geometry::for_protocol(&air, Protocol::ShakeCandidate).unwrap();
        let a = Binding::new(&air, &geometry).unwrap();
        for changed_identity in [false, true] {
            let mut changed = relation(Protocol::ShakeCandidate);
            if changed_identity {
                changed.schema.identity = "another fixed circuit identity";
            } else {
                changed.public.push(0);
            }
            let changed_geometry =
                Geometry::for_protocol(&changed, Protocol::ShakeCandidate).unwrap();
            let b = Binding::new(&changed, &changed_geometry).unwrap();
            assert_ne!(a.row(0, &[0; 342]).unwrap(), b.row(0, &[0; 342]).unwrap());
            assert_ne!(
                a.mixed(0, GoldilocksFp4V1::ZERO).unwrap(),
                b.mixed(0, GoldilocksFp4V1::ZERO).unwrap()
            );
            assert_ne!(
                a.quotient(0, GoldilocksFp4V1::ZERO).unwrap(),
                b.quotient(0, GoldilocksFp4V1::ZERO).unwrap()
            );
            assert_ne!(
                a.fri(0, 0, &[GoldilocksFp4V1::ZERO; 2]).unwrap(),
                b.fri(0, 0, &[GoldilocksFp4V1::ZERO; 2]).unwrap()
            );
            assert_ne!(
                a.parent(MerkleTreeRoleV1::AirTrace, 1, 0, root(1), root(2))
                    .unwrap(),
                b.parent(MerkleTreeRoleV1::AirTrace, 1, 0, root(1), root(2))
                    .unwrap()
            );
        }
        let mut too_large = relation(Protocol::ShakeCandidate);
        too_large.public = vec![0; 256 * 1024 + 1];
        assert!(Binding::new(&too_large, &geometry).is_err());
        let expected = a.row(0, &[0; 342]).unwrap();
        let _ambient = norito::core::DecodeFlagsGuard::enter(0);
        assert_eq!(
            Binding::new(&air, &geometry)
                .unwrap()
                .row(0, &[0; 342])
                .unwrap(),
            expected
        );
    }

    #[test]
    fn candidate_tree_owner_and_bounded_plan_share_exact_parent_coordinates() {
        let air = relation(Protocol::ShakeCandidate);
        let geometry = Geometry::for_protocol(&air, Protocol::ShakeCandidate).unwrap();
        let binding = Binding::new(&air, &geometry).unwrap();
        for (round, count, width) in [(15, 8, 2), (16, 4, 2), (17, 1, 4)] {
            let role = MerkleTreeRoleV1::Fri(round as u32);
            let leaves: Vec<_> = (0..count)
                .map(|i| {
                    binding
                        .fri(
                            round,
                            i,
                            &vec![GoldilocksFp4V1::from_base(i as u64).unwrap(); width],
                        )
                        .unwrap()
                })
                .collect();
            let tree = binding.tree(&leaves, role).unwrap();
            assert!(binding.tree(&leaves[..leaves.len() - 1], role).is_err());
            let indices: Vec<_> = (0..count)
                .filter(|i| i % 3 == 0 || *i + 1 == count)
                .collect();
            let plan = MultiproofPlan::new(
                count,
                &indices,
                MultiproofLimits {
                    max_depth: 19,
                    max_queried_leaves: 8,
                    max_siblings: 32,
                    max_parent_hashes: 32,
                },
            )
            .unwrap();
            let siblings = plan
                .open_with(&tree.levels, |level, index, left, right| {
                    binding.parent(role, level, index, left, right)
                })
                .unwrap();
            let selected: Vec<_> = indices.iter().map(|&i| leaves[i]).collect();
            assert_eq!(
                binding
                    .verify_tree(&plan, role, tree.root(), &selected, &siblings)
                    .unwrap(),
                plan.work()
            );
            assert!(
                binding
                    .verify_tree(&plan, role, root(99), &selected, &siblings)
                    .is_err()
            );
        }
        assert!(
            binding
                .parent(MerkleTreeRoleV1::Trace, 1, 0, root(0), root(0))
                .is_err()
        );
        assert!(fri_oracle(18).is_err());
        assert!(coordinate(usize::MAX).is_err());
    }

    #[test]
    fn common_fold_preserves_original_layers_challenges_queries_and_paths() {
        let air = relation(Protocol::Prototype);
        let geometry = Geometry::new(&air).unwrap();
        let binding = Binding::new(&air, &geometry).unwrap();
        let mut actual = binding.transcript(&air, &geometry, root(1)).unwrap();
        actual.columns().unwrap();
        actual.alphas(root(2)).unwrap();
        actual.joint(root(3)).unwrap();
        let mut original = initialise_transcript(&air, &geometry, root(1)).unwrap();
        challenges(&mut original, "compact:column-mix", 1);
        original.append_message("compact:mixed-root", &root(2).to_le_bytes());
        challenges(&mut original, "compact:constraint-alpha", 1);
        original.append_message("compact:quotient-root", &root(3).to_le_bytes());
        JointFriBatch::from_transcript(&FASTPQ_FINAL_V1, geometry.lde_rows, &mut original).unwrap();
        let values: Vec<_> = (0..geometry.lde_rows)
            .map(|i| GoldilocksFp4V1::new([geometry.domain.point(i), 2, 3, 4]).unwrap())
            .collect();
        let (mut a, indices) =
            fold_protocol_layers(&values, &geometry, &binding, &mut actual).unwrap();
        let mut b = crate::backend::fold_with_fri_opening_layers(
            &values,
            &FASTPQ_FINAL_V1,
            &mut original,
            ExecutionMode::Cpu,
        )
        .unwrap();
        assert_eq!(a.layer_values, b.layer_values);
        assert_eq!(a.roots, b.roots);
        assert_eq!(a.betas, b.betas);
        assert_eq!(
            indices,
            sample_queries(geometry.lde_rows, 136, &mut original).unwrap()
        );
        assert_eq!(
            a.open_query_chains(&indices, 2).unwrap(),
            b.open_query_chains(&indices, 2).unwrap()
        );
        assert!(
            fold_protocol_layers(
                &values[..values.len() - 1],
                &geometry,
                &binding,
                &mut actual
            )
            .is_err()
        );
    }

    #[test]
    fn candidate_preparation_rejects_bad_columns_and_cross_context_reuse() {
        let air = relation(Protocol::ShakeCandidate);
        assert!(prepare_trace_for(&air, &[], Protocol::ShakeCandidate).is_err());
        assert!(prepare_trace_for(&air, &vec![Vec::new(); 342], Protocol::ShakeCandidate).is_err());
        let geometry = Geometry::for_protocol(&air, Protocol::ShakeCandidate).unwrap();
        let binding = Binding::new(&air, &geometry).unwrap();
        // This intentionally absent witness/row tree is never touched: schema
        // and complete context must reject reuse before any prover work.
        let trace = PreparedTrace {
            geometry,
            binding,
            bound_statement: Some(air.public.clone()),
            columns: Vec::new(),
            rows: CommittedTree {
                levels: Vec::new(),
                leaf_count: 0,
            },
        };
        let mut changed = relation(Protocol::ShakeCandidate);
        changed.public.push(0);
        assert!(
            matches!(prove_prepared(&changed, &trace), Err(Error::InvalidTraceShape { details })
            if details.contains("another public statement"))
        );
        changed.schema.identity = "another fixed relation";
        assert!(
            matches!(prove_prepared(&changed, &trace), Err(Error::InvalidTraceShape { details })
            if details.contains("another fixed schema"))
        );
    }

    #[test]
    fn structured_context_roots_match_independent_norito_shake_vectors() {
        // Independent Python encodes all seventeen StatementContext fields,
        // the complete Norito prefix/body frames and hashlib SHAKE256. These
        // vectors bind its 225-byte context frame and 382-byte prefix, including
        // schema names, String/Vec lengths, field order, domain and CRCs.
        let air = relation(Protocol::ShakeCandidate);
        let geometry = Geometry::for_protocol(&air, Protocol::ShakeCandidate).unwrap();
        let binding = Binding::new(&air, &geometry).unwrap();
        for (role, actual, expected) in [
            (
                "row",
                binding.row(0, &[0; 342]).unwrap(),
                "b46a0302a2054203efc8e8e4efc034d0d41df4e289af4799c76e63d73072364cfdea264a713b93294634c85f8246a31b",
            ),
            (
                "mixed",
                binding.mixed(0, GoldilocksFp4V1::ZERO).unwrap(),
                "74ce1c1f2a26bc6b49b5cf91a98f9ba951006c06f153abbcf4fe943742e8e9536ec0278da7e1896d09ba5aec5ffb728e",
            ),
            (
                "quotient",
                binding.quotient(0, GoldilocksFp4V1::ZERO).unwrap(),
                "5b1429cce353590adbfb099dfbaeac29c3365addf2781a39d4b1bcbbd4817cf8cdebb8a4b5a13fd754823161d80da41e",
            ),
            (
                "fri0",
                binding.fri(0, 0, &[GoldilocksFp4V1::ZERO; 2]).unwrap(),
                "490044daca5b1017f7a1191b8d2a3718db199cf388337c09cd6cc863e21d6e1a738f405d46fff73b5f7250cfe10d456a",
            ),
            (
                "terminal",
                binding.fri(17, 0, &[GoldilocksFp4V1::ZERO; 4]).unwrap(),
                "0a812fa9936286dc4beb14e3cc646cfbdac68d554607867ace2fcea5233d0bb87be416416609b1d3e3375e4212ecfb0a",
            ),
        ] {
            assert_eq!(hex::encode(actual.to_le_bytes()), expected, "{role}");
        }
    }
}
