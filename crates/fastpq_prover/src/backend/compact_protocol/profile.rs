//! One mandatory six-lane binding for the fixed compact V1 arithmetic engine.
//!
//! Complete context and geometry are caller-fixed. This implementation does not
//! qualify production admission; its proof-size and concrete-security obligations
//! remain explicit even though no alternate cryptographic descriptor is accepted.

use super::*;
use crate::backend::{compact_v1 as compact, merkle_multiproof::MultiproofPlan};

pub(super) const QUERY_COUNT: usize = 375;

pub(super) fn check_geometry(geometry: &Geometry) -> Result<()> {
    if geometry.schema.trace_rows != 65_536
        || geometry.schema.width != 342
        || geometry.schema.constraints != 923
        || geometry.lde_rows != 524_288
        || FASTPQ_FINAL_V1.fri.arity != 2
        || FASTPQ_FINAL_V1.fri.blowup_factor != 8
        || geometry.fri_lengths != (0..=17).map(|r| 524_288 >> r).collect::<Vec<_>>()
        || geometry.terminal_degree != 1
    {
        return Err(shape(
            "compact V1 requires its exact fixed AIR/FRI geometry",
        ));
    }
    Ok(())
}

#[derive(NoritoSerialize, norito::NoritoSchema)]
#[norito_schema(
    name = "fastpq_prover::backend::compact_protocol::profile::StatementContext",
    frame = "fastpq_prover::compact_v1::EngineStatementV1"
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

/// Immutable complete binding reused for every oracle in one proof context.
#[derive(Clone, Debug)]
pub(super) struct Binding {
    context: compact::Context,
}

impl Binding {
    /// Bind every fixed relation, algebraic-domain and complete public-context field.
    pub(super) fn new(relation: &impl FixedAir, geometry: &Geometry) -> Result<Self> {
        if relation.schema() != geometry.schema {
            return Err(shape(
                "compact binding requires the geometry of this exact relation",
            ));
        }
        check_geometry(geometry)?;
        check_limit(
            "max_compact_public_bytes",
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
            queries: QUERY_COUNT as u32,
            statement: relation.statement_bytes().to_vec(),
        };
        // The context cap applies to the complete canonical frame, including
        // its geometry. Raw caller bytes do not receive an extra hidden budget.
        let encoded = norito::encode_canonical(&context)?;
        Ok(Self {
            context: compact::Context::new(&encoded).map_err(candidate_error)?,
        })
    }

    /// Hash the complete canonical row and its exact position.
    pub(super) fn row(&self, index: usize, values: &[u64]) -> Result<Digest> {
        let bytes: Vec<_> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        self.context
            .hash_leaf(compact::Oracle::Row, coordinate(index)?, &bytes)
            .map_err(candidate_error)
    }

    /// Hash the distinct mixed oracle.
    pub(super) fn mixed(&self, index: usize, value: GoldilocksFp4V1) -> Result<Digest> {
        self.context
            .hash_leaf(
                compact::Oracle::Mixed,
                coordinate(index)?,
                &value.to_le_bytes(),
            )
            .map_err(candidate_error)
    }

    /// Hash the distinct quotient oracle.
    pub(super) fn quotient(&self, index: usize, value: GoldilocksFp4V1) -> Result<Digest> {
        self.context
            .hash_leaf(
                compact::Oracle::Quotient,
                coordinate(index)?,
                &value.to_le_bytes(),
            )
            .map_err(candidate_error)
    }

    /// Hash a strided binary FRI leaf or the complete terminal vector.
    pub(super) fn fri(
        &self,
        round: usize,
        index: usize,
        values: &[GoldilocksFp4V1],
    ) -> Result<Digest> {
        let bytes: Vec<_> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        self.context
            .hash_leaf(fri_oracle(round)?, coordinate(index)?, &bytes)
            .map_err(candidate_error)
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
        self.context
            .hash_parent(
                oracle(role)?,
                coordinate(level)?,
                coordinate(index)?,
                left,
                right,
            )
            .map_err(candidate_error)
    }

    /// Authenticate the canonical frontier with unchanged plan/work accounting.
    pub(super) fn verify_tree(
        &self,
        plan: &MultiproofPlan,
        role: MerkleTreeRoleV1,
        root: Digest,
        leaves: &[Digest],
        siblings: &[Digest],
    ) -> Result<crate::backend::merkle_multiproof::MultiproofWork> {
        plan.verify_with(root, leaves, siblings, |level, index, left, right| {
            self.parent(role, level, index, left, right)
        })
    }

    /// Build a prover-owned tree under the same canonical owner and exact shape.
    pub(super) fn tree(&self, leaves: &[Digest], role: MerkleTreeRoleV1) -> Result<CommittedTree> {
        let expected = match oracle(role)? {
            compact::Oracle::Row | compact::Oracle::Mixed | compact::Oracle::Quotient => 524_288,
            compact::Oracle::Fri(round @ 0..=16) => 524_288 >> (round + 1),
            compact::Oracle::Fri(17) => 1,
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

    /// Start the fixed whole-message transcript and bind the first row root.
    pub(super) fn transcript(
        &self,
        _relation: &impl FixedAir,
        geometry: &Geometry,
        row_root: Digest,
    ) -> Result<ProtocolTranscript> {
        check_geometry(geometry)?;
        let mut state = compact::Transcript::new(self.context.clone());
        if state.challenge().map_err(candidate_error)? != compact::Message::Dummy {
            return Err(shape(
                "compact V1 requires its initial dummy verifier message",
            ));
        }
        state.commit(row_root).map_err(candidate_error)?;
        Ok(ProtocolTranscript {
            state,
            next_message: 2,
            schema: geometry.schema,
            domain: geometry.lde_rows,
            folds: geometry.fri_lengths.len() - 1,
        })
    }
}

fn coordinate(value: usize) -> Result<u32> {
    u32::try_from(value).map_err(|_| Error::QueryIndexOverflow { index: value })
}

fn fri_oracle(round: usize) -> Result<compact::Oracle> {
    let round = u8::try_from(round).map_err(|_| shape("candidate FRI ordinal overflows"))?;
    if round > 17 {
        return Err(shape("candidate FRI ordinal exceeds its fixed count"));
    }
    Ok(compact::Oracle::Fri(round))
}

fn oracle(role: MerkleTreeRoleV1) -> Result<compact::Oracle> {
    match role {
        MerkleTreeRoleV1::AirTrace => Ok(compact::Oracle::Row),
        MerkleTreeRoleV1::Lde => Ok(compact::Oracle::Mixed),
        MerkleTreeRoleV1::AirComposition => Ok(compact::Oracle::Quotient),
        MerkleTreeRoleV1::Fri(round) => fri_oracle(round as usize),
        _ => Err(shape("tree role is outside the fixed candidate")),
    }
}

fn candidate_error(error: compact::CandidateError) -> Error {
    Error::InvalidTraceShape {
        details: format!("six-lane compact V1: {error}"),
    }
}

/// Fixed root/challenge schedule shared by prover and verifier orchestration.
pub(super) struct ProtocolTranscript {
    state: compact::Transcript,
    next_message: usize,
    schema: FixedAirSchema,
    domain: usize,
    folds: usize,
}

impl ProtocolTranscript {
    fn expect(&self, ordinal: usize) -> Result<()> {
        if self.next_message != ordinal {
            return Err(shape("compact protocol transcript phase mismatch"));
        }
        Ok(())
    }

    fn fields(transcript: &mut compact::Transcript, count: usize) -> Result<Vec<GoldilocksFp4V1>> {
        match transcript.challenge().map_err(candidate_error)? {
            compact::Message::Fields(fields) if fields.len() == count => Ok(fields),
            _ => Err(shape("compact verifier message has another field count")),
        }
    }

    /// Derive all column coefficients after the row commitment.
    pub(super) fn columns(&mut self) -> Result<Vec<GoldilocksFp4V1>> {
        self.expect(2)?;
        let values = Self::fields(&mut self.state, self.schema.width)?;
        self.next_message = 3;
        Ok(values)
    }

    /// Bind the mixed root before all constraint coefficients.
    pub(super) fn alphas(&mut self, root: Digest) -> Result<Vec<GoldilocksFp4V1>> {
        self.expect(3)?;
        self.state.commit(root).map_err(candidate_error)?;
        let values = Self::fields(&mut self.state, self.schema.constraints)?;
        self.next_message = 4;
        Ok(values)
    }

    /// Bind the quotient root before both joint-degree coefficients.
    pub(super) fn joint(&mut self, root: Digest) -> Result<JointFriBatch> {
        self.expect(4)?;
        self.state.commit(root).map_err(candidate_error)?;
        let values = Self::fields(&mut self.state, 2)?;
        let joint =
            JointFriBatch::from_challenges(&FASTPQ_FINAL_V1, self.domain, values[0], values[1])?;
        self.next_message = 5;
        Ok(joint)
    }

    /// Bind exactly the next FRI root and derive one coefficient.
    pub(super) fn beta(&mut self, round: usize, root: Digest) -> Result<GoldilocksFp4V1> {
        if round >= self.folds {
            return Err(shape("compact transcript has an extra FRI fold"));
        }
        self.expect(5 + round)?;
        self.state.commit(root).map_err(candidate_error)?;
        let value = Self::fields(&mut self.state, 1)?[0];
        self.next_message += 1;
        Ok(value)
    }

    /// Bind the terminal root and complete the exact fixed query set.
    pub(super) fn queries(&mut self, root: Digest) -> Result<Vec<usize>> {
        self.expect(5 + self.folds)?;
        self.state.commit(root).map_err(candidate_error)?;
        let compact::Message::Queries(indices) = self.state.challenge().map_err(candidate_error)?
        else {
            return Err(shape("compact final message must contain query positions"));
        };
        if indices.len() != QUERY_COUNT {
            return Err(shape(
                "compact transcript produced another fixed query count",
            ));
        }
        self.next_message += 1;
        Ok(indices.into_iter().map(|i| i as usize).collect())
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

    fn relation() -> StatementOnly {
        StatementOnly {
            schema: FixedAirSchema {
                trace_rows: 65_536,
                width: 342,
                constraints: 923,
                identity: "profile-binding:fixed-candidate:v1",
            },
            public: b"complete public context without a private witness".to_vec(),
        }
    }

    fn root(value: u64) -> Digest {
        Digest::new([value; 6]).unwrap()
    }

    fn message_fields(message: compact::Message) -> Vec<GoldilocksFp4V1> {
        match message {
            compact::Message::Fields(values) => values,
            _ => panic!("complete field tape"),
        }
    }

    #[test]
    fn fixed_candidate_geometry_rejects_every_other_dimension_before_binding() {
        let air = relation();
        let geometry = Geometry::new(&air).unwrap();
        assert_eq!(QUERY_COUNT, 375);
        for change in [
            (32_768, 342, 923),
            (65_536, 341, 923),
            (65_536, 342, 922),
            (16, 1, 1),
        ] {
            let mut bad = relation();
            bad.schema.trace_rows = change.0;
            bad.schema.width = change.1;
            bad.schema.constraints = change.2;
            assert!(Geometry::new(&bad).is_err());
            assert!(Binding::new(&bad, &geometry).is_err());
        }
        let mut bad = Geometry::new(&air).unwrap();
        bad.fri_lengths[3] += 1;
        assert!(check_geometry(&bad).is_err());
    }

    #[test]
    fn candidate_adapter_preserves_whole_tapes_and_rejects_wrong_method_order() {
        let air = relation();
        let geometry = Geometry::new(&air).unwrap();
        let binding = Binding::new(&air, &geometry).unwrap();
        let mut actual = binding.transcript(&air, &geometry, root(1)).unwrap();
        let mut reference = compact::Transcript::new(binding.context.clone());
        assert_eq!(reference.challenge().unwrap(), compact::Message::Dummy);
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
        let compact::Message::Queries(indices) = reference.challenge().unwrap() else {
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
        let air = relation();
        let geometry = Geometry::new(&air).unwrap();
        let a = Binding::new(&air, &geometry).unwrap();
        for changed_identity in [false, true] {
            let mut changed = relation();
            if changed_identity {
                changed.schema.identity = "another fixed circuit identity";
            } else {
                changed.public.push(0);
            }
            let changed_geometry = Geometry::new(&changed).unwrap();
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
        let mut too_large = relation();
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
        let air = relation();
        let geometry = Geometry::new(&air).unwrap();
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
    fn common_fold_matches_direct_extension_polynomial_evaluation() {
        for length in [16, 128, 512] {
            let domain = FriDomain::from_lde_parameters(
                FASTPQ_FINAL_V1.lde_root,
                FASTPQ_FINAL_V1.lde_log_size,
                length,
                FASTPQ_FINAL_V1.omega_coset,
            )
            .unwrap();
            let intercept = GoldilocksFp4V1::new([11, 2, 3, 4]).unwrap();
            let slope = GoldilocksFp4V1::new([29, 7, 8, 9]).unwrap();
            let beta = GoldilocksFp4V1::new([31, 5, 7, 11]).unwrap();
            let values = (0..length)
                .map(|i| intercept.add(slope.mul_base(domain.point(i))))
                .collect::<Vec<_>>();
            let expected = intercept.add(beta.mul(slope));
            assert_eq!(
                crate::backend::fold_round(&values, 2, beta, domain).unwrap(),
                vec![expected; length / 2]
            );
            assert_eq!(
                crate::backend::fold_round(&vec![intercept; length], 2, beta, domain).unwrap(),
                vec![intercept; length / 2]
            );
            assert!(crate::backend::fold_round(&values[..length - 1], 2, beta, domain).is_err());
        }
    }

    #[test]
    fn candidate_preparation_rejects_bad_columns_and_cross_context_reuse() {
        let air = relation();
        assert!(prepare_trace(&air, &[]).is_err());
        assert!(prepare_trace(&air, &vec![Vec::new(); 342]).is_err());
        let geometry = Geometry::new(&air).unwrap();
        let binding = Binding::new(&air, &geometry).unwrap();
        // This intentionally absent witness/row tree is never touched: schema
        // and complete context must reject reuse before any prover work.
        let trace = PreparedTrace {
            geometry,
            binding,
            bound_statement: air.public.clone(),
            columns: Vec::new(),
            rows: CommittedTree {
                levels: Vec::new(),
                leaf_count: 0,
            },
        };
        let mut changed = relation();
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
    fn structured_context_roots_match_independent_norito_six_lane_vectors() {
        // Independent Python encodes all seventeen StatementContext fields,
        // the complete Norito prefix/body frames and exact six-lane arithmetic. These
        // vectors bind its 225-byte context frame and 395-byte prefix, including
        // schema names, String/Vec lengths, field order, domain and CRCs.
        let air = relation();
        let geometry = Geometry::new(&air).unwrap();
        let binding = Binding::new(&air, &geometry).unwrap();
        for (role, actual, expected) in [
            (
                "row",
                binding.row(0, &[0; 342]).unwrap(),
                "1d5c05f912e530c411d0b19abbf4aaa40ee005ba407aaecf434663e41e47c7ae3d2a694b4b9016a93353ce00a494204d",
            ),
            (
                "mixed",
                binding.mixed(0, GoldilocksFp4V1::ZERO).unwrap(),
                "86fdaf108ebd108506822935d939ee420652f4aa9b4ffca044b4787c6c07fb01e6e83489aa75b8c8246aa7e6daed2af8",
            ),
            (
                "quotient",
                binding.quotient(0, GoldilocksFp4V1::ZERO).unwrap(),
                "23bf2973c5d016c920501619fca508cb66595901c65281f657543b7b48a741ade1f949811b9675460205524e0ee2824b",
            ),
            (
                "fri0",
                binding.fri(0, 0, &[GoldilocksFp4V1::ZERO; 2]).unwrap(),
                "af444f0e2e9244cd32ebf77503c846ca549e5b14c1a3b4c12bf8cabdd99833db7620dc54fc9cc0c248fd52b92763ad5a",
            ),
            (
                "terminal",
                binding.fri(17, 0, &[GoldilocksFp4V1::ZERO; 4]).unwrap(),
                "706d8408f3774e277e03474ebbecf69f602af51fe0df65b047e40feb607da9cad3edd74690e0e60344ba4b1f1150fcf6",
            ),
        ] {
            assert_eq!(hex::encode(actual.to_le_bytes()), expected, "{role}");
        }
    }
}
