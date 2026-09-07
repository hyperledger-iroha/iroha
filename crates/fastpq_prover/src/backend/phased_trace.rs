//! Consuming commitment phases for future base and extension-field trace oracles.
//!
//! Base roots and the statement digest precede pair challenges. The auxiliary
//! root precedes independent column mixing, and the mixed root precedes AIR
//! alphas. Every root and leaf binds exact geometry, schema and typed role.
//! These states expose no mutable transcript or root replacement operation.
//!
//! TODO: Integrate the authenticated semantic statement and fixed column/constraint
//! ledger, complete compact semantic/hash AIR, typed openings, full Fp4 quotient
//! evaluation and joint degree proof in a hard-cut V1 proof schema. Keep production
//! witness replay mandatory until equivalent semantics and public boundaries are
//! proved. A committed statement-table root is not authenticated by this helper.
//! A public table may instead be evaluated by sparse Lagrange interpolation at
//! queries; use zero committed table columns in that route and bind its exact
//! authenticated bytes/count/schema through `statement_digest` and `schema_id`.
//! TODO: Qualify combined resource limits, all Fiat-Shamir/batching failure terms,
//! and deterministic acceleration before any production admission change.

use super::{AirQuotientDomain, GoldilocksDigest384V1, Transcript, hash_bytes_v1};
use crate::{Error, GoldilocksFp4V1, Result, trace::DEFAULT_MAX_TRACE_COLUMNS};

const PHASE_TAG: &str = "fastpq:v1:phased-trace";
/// Local preparation bound; this does not expand any production proof limit.
const MAX_COMPOSITION_CHALLENGES: usize = 65_536;

/// Challenge-independent schema and statement binding supplied by the caller.
///
/// The caller must obtain this identity and digest from its authenticated statement
/// and bind exact public table bytes/counts, packing, selectors and constraint order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PhaseSchema {
    /// Identity of the complete fixed semantic and polynomial schema.
    pub(crate) schema_id: [u8; 32],
    /// Commitment to the exact caller-authenticated public statement.
    pub(crate) statement_digest: GoldilocksDigest384V1,
    /// Number of execution base-field columns.
    pub(crate) base_columns: usize,
    /// Number of separately committed base-field table columns, possibly zero.
    pub(crate) table_columns: usize,
    /// Number of complete postchallenge Fp4 columns, possibly zero.
    pub(crate) auxiliary_columns: usize,
    /// Exact independently challenged constraint count from the fixed ledger.
    pub(crate) composition_constraints: usize,
}

/// Validated common subgroup/coset geometry and canonical schema frame.
#[derive(Clone, Debug)]
pub(crate) struct PhaseLayout {
    schema: PhaseSchema,
    trace_rows: usize,
    lde_rows: usize,
    frame: Vec<u8>,
}

impl PhaseLayout {
    /// Validate dimensions before allocation and pin all canonical framing bytes.
    pub(crate) fn new(schema: PhaseSchema, trace_rows: usize) -> Result<Self> {
        let params = fastpq_isi::FASTPQ_FINAL_V1;
        if !trace_rows.is_power_of_two() || trace_rows.ilog2() > params.trace_log_size {
            return Err(shape_error(
                "phased trace requires a supported power-of-two subgroup",
            ));
        }
        if schema.base_columns == 0 {
            return Err(shape_error(
                "phased trace requires at least one base column",
            ));
        }
        // Count the physical base-field coefficients, not one Fp4 as one u64.
        let coefficient_columns = schema
            .auxiliary_columns
            .checked_mul(4)
            .and_then(|auxiliary| {
                schema
                    .base_columns
                    .checked_add(schema.table_columns)?
                    .checked_add(auxiliary)
            })
            .ok_or_else(|| shape_error("phased trace column geometry overflows"))?;
        if coefficient_columns > DEFAULT_MAX_TRACE_COLUMNS {
            return Err(Error::VerifierLimitExceeded {
                limit: "phased_trace_coefficient_columns",
                actual: coefficient_columns,
                max: DEFAULT_MAX_TRACE_COLUMNS,
            });
        }
        if schema.composition_constraints == 0
            || schema.composition_constraints > MAX_COMPOSITION_CHALLENGES
        {
            return Err(shape_error(
                "phased trace constraint count exceeds the local preparation bound",
            ));
        }
        validate_digest(schema.statement_digest)?;
        let lde_rows = trace_rows
            .checked_mul(params.fri.blowup_factor as usize)
            .ok_or(Error::TraceLengthOverflow { rows: trace_rows })?;
        AirQuotientDomain::new(&params, lde_rows)?;
        let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        let frame = norito::core::to_bytes(&(
            PHASE_TAG,
            schema.schema_id,
            schema.statement_digest.to_le_bytes(),
            trace_rows as u64,
            lde_rows as u64,
            schema.base_columns as u64,
            schema.table_columns as u64,
            schema.auxiliary_columns as u64,
            schema.composition_constraints as u64,
        ))?;
        Ok(Self {
            schema,
            trace_rows,
            lde_rows,
            frame,
        })
    }

    /// Exact common subgroup length of every committed polynomial.
    pub(crate) fn trace_rows(&self) -> usize {
        self.trace_rows
    }

    /// Exact common number of row evaluations on the disjoint LDE coset.
    pub(crate) fn lde_rows(&self) -> usize {
        self.lde_rows
    }

    fn width(&self, role: OracleRole) -> usize {
        match role {
            OracleRole::Base => self.schema.base_columns,
            OracleRole::Table => self.schema.table_columns,
            OracleRole::Auxiliary => self.schema.auxiliary_columns,
            OracleRole::Mixed => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OracleRole {
    Base,
    Table,
    Auxiliary,
    Mixed,
}

impl OracleRole {
    fn bytes(self) -> &'static [u8] {
        match self {
            Self::Base => b"phased-execution-base",
            Self::Table => b"phased-statement-table",
            Self::Auxiliary => b"phased-extension-trace",
            Self::Mixed => b"phased-mixed-trace",
        }
    }
}

/// Independently sampled complete Fp4 pair-table challenges.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PairTableChallenges {
    /// Tuple-compression coefficient, fixed after both base roots.
    pub(crate) compression: GoldilocksFp4V1,
    /// Independent additive factor shift; never resampled on a zero factor.
    pub(crate) shift: GoldilocksFp4V1,
}

/// Immutable base roots fixed before any pair-table challenge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BaseRoots {
    /// Complete execution base-field row root.
    pub(crate) execution: GoldilocksDigest384V1,
    /// Complete table row root; transcript binding alone does not authenticate it.
    pub(crate) table: GoldilocksDigest384V1,
}

/// First state: base roots are fixed and pair-table challenges are available.
pub(crate) struct BaseCommitted {
    transcript: Transcript,
    layout: PhaseLayout,
    roots: BaseRoots,
    pair: PairTableChallenges,
}

impl BaseCommitted {
    /// Commit canonical column-major base/table LDE evaluations before challenges.
    pub(crate) fn commit(
        transcript: Transcript,
        layout: PhaseLayout,
        base: &[Vec<u64>],
        table: &[Vec<u64>],
    ) -> Result<Self> {
        validate_columns(&layout, OracleRole::Base, base)?;
        validate_columns(&layout, OracleRole::Table, table)?;
        let roots = BaseRoots {
            execution: commit_columns(&layout, OracleRole::Base, base)?,
            table: commit_columns(&layout, OracleRole::Table, table)?,
        };
        Self::replay(transcript, layout, roots)
    }

    /// Replay root binding without reconstructing private evaluations.
    ///
    /// This authenticates neither rows nor public table provenance. The surrounding
    /// verifier must validate all openings and public-table polynomial relations.
    pub(crate) fn replay(
        mut transcript: Transcript,
        layout: PhaseLayout,
        roots: BaseRoots,
    ) -> Result<Self> {
        validate_root(&layout, OracleRole::Base, roots.execution)?;
        validate_root(&layout, OracleRole::Table, roots.table)?;
        transcript.append_message("fastpq:v1:phased:schema", &layout.frame);
        append_root(&mut transcript, &layout, OracleRole::Base, roots.execution)?;
        append_root(&mut transcript, &layout, OracleRole::Table, roots.table)?;
        let pair = PairTableChallenges {
            compression: transcript.challenge_extension("fastpq:v1:phased:pair:compression"),
            shift: transcript.challenge_extension("fastpq:v1:phased:pair:shift"),
        };
        Ok(Self {
            transcript,
            layout,
            roots,
            pair,
        })
    }

    /// Read the two already fixed roots.
    pub(crate) fn roots(&self) -> BaseRoots {
        self.roots
    }

    /// Read the challenges needed to construct factors and inclusive products.
    pub(crate) fn pair_challenges(&self) -> PairTableChallenges {
        self.pair
    }

    /// Consume the base phase and commit every complete Fp4 auxiliary row.
    pub(crate) fn commit_auxiliary(
        self,
        columns: &[Vec<GoldilocksFp4V1>],
    ) -> Result<AuxiliaryCommitted> {
        validate_columns(&self.layout, OracleRole::Auxiliary, columns)?;
        let root = commit_columns(&self.layout, OracleRole::Auxiliary, columns)?;
        self.replay_auxiliary(root)
    }

    /// Consume the base phase and bind the verifier's fixed auxiliary root.
    pub(crate) fn replay_auxiliary(
        mut self,
        root: GoldilocksDigest384V1,
    ) -> Result<AuxiliaryCommitted> {
        validate_root(&self.layout, OracleRole::Auxiliary, root)?;
        append_root(
            &mut self.transcript,
            &self.layout,
            OracleRole::Auxiliary,
            root,
        )?;
        let mixing = ColumnMixing {
            base: derive_challenges(
                &mut self.transcript,
                "mix:base",
                self.layout.schema.base_columns,
            ),
            table: derive_challenges(
                &mut self.transcript,
                "mix:table",
                self.layout.schema.table_columns,
            ),
            auxiliary: derive_challenges(
                &mut self.transcript,
                "mix:auxiliary",
                self.layout.schema.auxiliary_columns,
            ),
        };
        Ok(AuxiliaryCommitted {
            base: self,
            auxiliary_root: root,
            mixing,
        })
    }
}

/// Independent coefficients in the exact base, table, auxiliary column order.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ColumnMixing {
    base: Vec<GoldilocksFp4V1>,
    table: Vec<GoldilocksFp4V1>,
    auxiliary: Vec<GoldilocksFp4V1>,
}

/// Second state: all column roots are fixed and their mixing is available.
pub(crate) struct AuxiliaryCommitted {
    base: BaseCommitted,
    auxiliary_root: GoldilocksDigest384V1,
    mixing: ColumnMixing,
}

impl AuxiliaryCommitted {
    /// Read the fixed complete Fp4 row root.
    pub(crate) fn auxiliary_root(&self) -> GoldilocksDigest384V1 {
        self.auxiliary_root
    }

    /// Mix one exact-width row using base embedding and full auxiliary products.
    ///
    /// The caller must independently authenticate sampled rows under their roots;
    /// this arithmetic helper does not replace Merkle opening verification.
    pub(crate) fn mix_row(
        &self,
        base: &[u64],
        table: &[u64],
        auxiliary: &[GoldilocksFp4V1],
    ) -> Result<GoldilocksFp4V1> {
        let mut mixed = GoldilocksFp4V1::ZERO;
        for (values, coefficients) in [(base, &self.mixing.base), (table, &self.mixing.table)] {
            if values.len() != coefficients.len() {
                return Err(shape_error("phased mixed base row differs from its schema"));
            }
            for (column, (&value, &coefficient)) in values.iter().zip(coefficients).enumerate() {
                value.validate(0, column)?;
                mixed = mixed.add(coefficient.mul(value.embedded()));
            }
        }
        if auxiliary.len() != self.mixing.auxiliary.len() {
            return Err(shape_error(
                "phased mixed auxiliary row differs from its schema",
            ));
        }
        for (column, (&value, &coefficient)) in
            auxiliary.iter().zip(&self.mixing.auxiliary).enumerate()
        {
            value.validate(0, column)?;
            mixed = mixed.add(coefficient.mul(value));
        }
        Ok(mixed)
    }

    /// Commit a canonical mixed row oracle before deriving any AIR alpha.
    ///
    /// Sampled equality to `mix_row` remains a required surrounding proof check.
    pub(crate) fn commit_mixed(self, values: &[GoldilocksFp4V1]) -> Result<CompositionChallenges> {
        if values.len() != self.base.layout.lde_rows {
            return Err(shape_error("phased mixed oracle has the wrong LDE length"));
        }
        for (row, &value) in values.iter().enumerate() {
            value.validate(row, 0)?;
        }
        let root = commit_oracle(&self.base.layout, OracleRole::Mixed, |row, _| values[row])?;
        self.replay_mixed(root)
    }

    /// Bind the verifier's mixed root once, then derive the exact alpha vector.
    pub(crate) fn replay_mixed(
        mut self,
        root: GoldilocksDigest384V1,
    ) -> Result<CompositionChallenges> {
        validate_digest(root)?;
        append_root(
            &mut self.base.transcript,
            &self.base.layout,
            OracleRole::Mixed,
            root,
        )?;
        let alphas = derive_challenges(
            &mut self.base.transcript,
            "alpha",
            self.base.layout.schema.composition_constraints,
        );
        Ok(CompositionChallenges {
            auxiliary: self,
            mixed_root: root,
            alphas,
        })
    }
}

/// Final preparation state: all oracle roots, mixes and composition alphas fixed.
pub(crate) struct CompositionChallenges {
    auxiliary: AuxiliaryCommitted,
    mixed_root: GoldilocksDigest384V1,
    alphas: Vec<GoldilocksFp4V1>,
}

impl CompositionChallenges {
    /// Read the independently challenged complete constraint coefficients.
    pub(crate) fn alphas(&self) -> &[GoldilocksFp4V1] {
        &self.alphas
    }

    /// Read the fixed mixed-trace row root.
    pub(crate) fn mixed_root(&self) -> GoldilocksDigest384V1 {
        self.mixed_root
    }

    /// Continue with quotient commitment and joint FRI only after these phases.
    ///
    /// TODO: A complete protocol typestate must enforce Q-root before joint FRI
    /// challenges and every FRI root before its folding/query challenges.
    pub(crate) fn into_transcript(self) -> Transcript {
        self.auxiliary.base.transcript
    }
}

trait RowValue: Copy {
    fn validate(self, row: usize, column: usize) -> Result<()>;
    fn append_bytes(self, bytes: &mut Vec<u8>);
    fn embedded(self) -> GoldilocksFp4V1;
}

impl RowValue for u64 {
    fn validate(self, row: usize, column: usize) -> Result<()> {
        if self >= crate::GOLDILOCKS_MODULUS_V1 {
            return Err(Error::NonCanonicalGoldilocksElement {
                context: "phased_base_row",
                indices: vec![row, column],
            });
        }
        Ok(())
    }
    fn append_bytes(self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.to_le_bytes());
    }
    fn embedded(self) -> GoldilocksFp4V1 {
        GoldilocksFp4V1::from_base(self).expect("validated base value")
    }
}

impl RowValue for GoldilocksFp4V1 {
    fn validate(self, row: usize, column: usize) -> Result<()> {
        for (lane, coefficient) in self.coefficients().into_iter().enumerate() {
            if coefficient >= crate::GOLDILOCKS_MODULUS_V1 {
                return Err(Error::NonCanonicalGoldilocksElement {
                    context: "phased_extension_row",
                    indices: vec![row, column, lane],
                });
            }
        }
        Ok(())
    }
    fn append_bytes(self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.to_le_bytes());
    }
    fn embedded(self) -> GoldilocksFp4V1 {
        self
    }
}

fn validate_columns<T: RowValue>(
    layout: &PhaseLayout,
    role: OracleRole,
    columns: &[Vec<T>],
) -> Result<()> {
    if columns.len() != layout.width(role)
        || columns.iter().any(|column| column.len() != layout.lde_rows)
    {
        return Err(shape_error(
            "phased oracle columns differ from exact committed geometry",
        ));
    }
    for (column, values) in columns.iter().enumerate() {
        for (row, &value) in values.iter().enumerate() {
            value.validate(row, column)?;
        }
    }
    Ok(())
}

fn commit_columns<T: RowValue>(
    layout: &PhaseLayout,
    role: OracleRole,
    columns: &[Vec<T>],
) -> Result<GoldilocksDigest384V1> {
    commit_oracle(layout, role, |row, column| columns[column][row])
}

fn commit_oracle<T: RowValue>(
    layout: &PhaseLayout,
    role: OracleRole,
    value: impl Fn(usize, usize) -> T,
) -> Result<GoldilocksDigest384V1> {
    if layout.width(role) == 0 {
        return hash_bytes_v1(role.bytes(), b"empty-oracle", 0, 0, 0, &[&layout.frame]);
    }
    let mut leaves = Vec::with_capacity(layout.lde_rows);
    for row in 0..layout.lde_rows {
        let mut bytes = Vec::new();
        for column in 0..layout.width(role) {
            value(row, column).append_bytes(&mut bytes);
        }
        leaves.push(hash_bytes_v1(
            role.bytes(),
            b"row-leaf",
            0,
            row,
            0,
            &[&layout.frame, &bytes],
        )?);
    }
    let mut level = 1;
    while leaves.len() > 1 {
        leaves = leaves
            .chunks_exact(2)
            .enumerate()
            .map(|(index, pair)| {
                hash_bytes_v1(
                    role.bytes(),
                    b"binary-node",
                    level,
                    index,
                    0,
                    &[
                        &layout.frame,
                        &pair[0].to_le_bytes(),
                        &pair[1].to_le_bytes(),
                    ],
                )
            })
            .collect::<Result<Vec<_>>>()?;
        level += 1;
    }
    // Geometry fixes L >= 8 to a power of two; no sole/odd-leaf ambiguity exists.
    Ok(leaves[0])
}

fn append_root(
    transcript: &mut Transcript,
    layout: &PhaseLayout,
    role: OracleRole,
    root: GoldilocksDigest384V1,
) -> Result<()> {
    let message = hash_bytes_v1(
        role.bytes(),
        b"commitment",
        0,
        0,
        0,
        &[&layout.frame, &root.to_le_bytes()],
    )?;
    transcript.append_message("fastpq:v1:phased:oracle-root", &message.to_le_bytes());
    Ok(())
}

fn derive_challenges(
    transcript: &mut Transcript,
    family: &str,
    count: usize,
) -> Vec<GoldilocksFp4V1> {
    (0..count)
        .map(|index| transcript.challenge_extension(&format!("fastpq:v1:phased:{family}:{index}")))
        .collect()
}

fn validate_digest(digest: GoldilocksDigest384V1) -> Result<()> {
    if digest
        .words()
        .iter()
        .any(|word| *word >= crate::GOLDILOCKS_MODULUS_V1)
    {
        return Err(shape_error(
            "phased root or statement digest is noncanonical",
        ));
    }
    Ok(())
}

fn validate_root(
    layout: &PhaseLayout,
    role: OracleRole,
    root: GoldilocksDigest384V1,
) -> Result<()> {
    validate_digest(root)?;
    if layout.width(role) == 0 {
        let expected = hash_bytes_v1(role.bytes(), b"empty-oracle", 0, 0, 0, &[&layout.frame])?;
        if root != expected {
            return Err(shape_error(
                "empty phased oracle must use its canonical typed root",
            ));
        }
    }
    Ok(())
}

fn shape_error(details: &str) -> Error {
    Error::InvalidTraceShape {
        details: details.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(seed: u64) -> GoldilocksDigest384V1 {
        GoldilocksDigest384V1::new(core::array::from_fn(|lane| seed + lane as u64)).unwrap()
    }
    fn schema() -> PhaseSchema {
        PhaseSchema {
            schema_id: [11; 32],
            statement_digest: digest(20),
            base_columns: 2,
            table_columns: 1,
            auxiliary_columns: 2,
            composition_constraints: 3,
        }
    }
    fn transcript() -> Transcript {
        Transcript::initialise(
            &crate::proof::PublicIO::default(),
            fastpq_isi::FASTPQ_FINAL_V1.name,
            1,
            "phased-test",
        )
        .unwrap()
    }
    fn base(rows: usize) -> Vec<Vec<u64>> {
        (0..2)
            .map(|column| {
                (0..rows)
                    .map(|row| 3 + (column * rows + row) as u64)
                    .collect()
            })
            .collect()
    }
    fn table(rows: usize) -> Vec<Vec<u64>> {
        vec![vec![7; rows]]
    }
    fn auxiliary(rows: usize) -> Vec<Vec<GoldilocksFp4V1>> {
        (0..2)
            .map(|column| {
                (0..rows)
                    .map(|row| {
                        GoldilocksFp4V1::new(core::array::from_fn(|lane| {
                            13 + (column * rows * 4 + row * 4 + lane) as u64
                        }))
                        .unwrap()
                    })
                    .collect()
            })
            .collect()
    }
    fn committed() -> BaseCommitted {
        BaseCommitted::commit(
            transcript(),
            PhaseLayout::new(schema(), 1).unwrap(),
            &base(8),
            &table(8),
        )
        .unwrap()
    }
    fn finish(aux: AuxiliaryCommitted) -> CompositionChallenges {
        let b = base(8);
        let t = table(8);
        let a = auxiliary(8);
        let mixed = (0..8)
            .map(|row| {
                aux.mix_row(
                    &[b[0][row], b[1][row]],
                    &[t[0][row]],
                    &[a[0][row], a[1][row]],
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        aux.commit_mixed(&mixed).unwrap()
    }

    #[test]
    fn phase_commitments_replay_identical_full_extension_challenges() {
        let base = committed();
        let roots = base.roots();
        let pair = base.pair_challenges();
        assert_ne!(pair.compression, pair.shift);
        let aux = base.commit_auxiliary(&auxiliary(8)).unwrap();
        let auxiliary_root = aux.auxiliary_root();
        let replay =
            BaseCommitted::replay(transcript(), PhaseLayout::new(schema(), 1).unwrap(), roots)
                .unwrap();
        assert_eq!(pair, replay.pair_challenges());
        let replay = replay.replay_auxiliary(auxiliary_root).unwrap();
        assert_eq!(aux.mixing, replay.mixing);
        let final_phase = finish(aux);
        let replay = replay.replay_mixed(final_phase.mixed_root()).unwrap();
        assert_eq!(final_phase.alphas(), replay.alphas());
        let mut all = final_phase.auxiliary.mixing.base.clone();
        all.extend_from_slice(&final_phase.auxiliary.mixing.table);
        all.extend_from_slice(&final_phase.auxiliary.mixing.auxiliary);
        all.extend_from_slice(final_phase.alphas());
        all.extend([pair.compression, pair.shift]);
        assert_eq!(
            all.iter().collect::<std::collections::BTreeSet<_>>().len(),
            all.len()
        );
        assert!(
            all.iter()
                .all(|value| value.coefficients()[1..].iter().any(|word| *word != 0))
        );
        assert_eq!(
            final_phase.into_transcript().challenge_extension("next"),
            replay.into_transcript().challenge_extension("next")
        );
    }

    #[test]
    fn base_roots_schema_statement_and_geometry_bind_pair_challenges() {
        let original = committed();
        let roots = original.roots();
        let expected = original.pair_challenges();
        for changed in [
            BaseRoots {
                execution: digest(101),
                ..roots
            },
            BaseRoots {
                table: digest(201),
                ..roots
            },
            BaseRoots {
                execution: roots.table,
                table: roots.execution,
            },
        ] {
            assert_ne!(
                expected,
                BaseCommitted::replay(
                    transcript(),
                    PhaseLayout::new(schema(), 1).unwrap(),
                    changed
                )
                .unwrap()
                .pair_challenges()
            );
        }
        for change in 0..7 {
            let mut changed = schema();
            let rows = if change == 0 { 2 } else { 1 };
            match change {
                1 => changed.schema_id[31] ^= 1,
                2 => changed.statement_digest = digest(40),
                3 => changed.base_columns += 1,
                4 => changed.table_columns += 1,
                5 => changed.auxiliary_columns += 1,
                6 => changed.composition_constraints += 1,
                _ => {}
            }
            assert_ne!(
                expected,
                BaseCommitted::replay(
                    transcript(),
                    PhaseLayout::new(changed, rows).unwrap(),
                    roots
                )
                .unwrap()
                .pair_challenges()
            );
        }
    }

    #[test]
    fn auxiliary_and_mixed_roots_affect_only_later_phase_challenges() {
        let roots = committed().roots();
        let make_base = || {
            BaseCommitted::replay(transcript(), PhaseLayout::new(schema(), 1).unwrap(), roots)
                .unwrap()
        };
        let expected_pair = make_base().pair_challenges();
        let a = make_base().replay_auxiliary(digest(100)).unwrap();
        let b = make_base().replay_auxiliary(digest(200)).unwrap();
        assert_eq!(a.base.pair_challenges(), expected_pair);
        assert_eq!(b.base.pair_challenges(), expected_pair);
        assert_ne!(a.mixing, b.mixing);
        assert_ne!(
            a.replay_mixed(digest(300)).unwrap().alphas(),
            b.replay_mixed(digest(300)).unwrap().alphas()
        );
        let make_aux = || make_base().replay_auxiliary(digest(100)).unwrap();
        assert_ne!(
            make_aux().replay_mixed(digest(300)).unwrap().alphas(),
            make_aux().replay_mixed(digest(400)).unwrap().alphas()
        );
    }

    #[test]
    fn row_commitments_bind_roles_indices_width_and_every_extension_coordinate() {
        let layout = PhaseLayout::new(schema(), 1).unwrap();
        assert_eq!(layout.trace_rows(), 1);
        assert_eq!(layout.lde_rows(), 8);
        let mut same_width = schema();
        same_width.table_columns = 2;
        let same_width = PhaseLayout::new(same_width, 1).unwrap();
        assert_ne!(
            commit_columns(&same_width, OracleRole::Base, &base(8)).unwrap(),
            commit_columns(&same_width, OracleRole::Table, &base(8)).unwrap()
        );
        let original = auxiliary(8);
        let expected = commit_columns(&layout, OracleRole::Auxiliary, &original).unwrap();
        for lane in 0..4 {
            let mut changed = original.clone();
            let mut coefficients = changed[1][3].coefficients();
            coefficients[lane] += 1;
            changed[1][3] = GoldilocksFp4V1::new(coefficients).unwrap();
            assert_ne!(
                expected,
                commit_columns(&layout, OracleRole::Auxiliary, &changed).unwrap()
            );
        }
        let mut changed = original.clone();
        changed[0].swap(0, 1);
        assert_ne!(
            expected,
            commit_columns(&layout, OracleRole::Auxiliary, &changed).unwrap()
        );
        let mut changed = original;
        changed.swap(0, 1);
        assert_ne!(
            expected,
            commit_columns(&layout, OracleRole::Auxiliary, &changed).unwrap()
        );
    }

    #[test]
    fn mixed_rows_use_canonical_base_embedding_and_full_extension_products() {
        let aux = committed().commit_auxiliary(&auxiliary(8)).unwrap();
        let values = [
            GoldilocksFp4V1::new([0, 1, 2, 3]).unwrap(),
            GoldilocksFp4V1::new([0, 0, 0, 1]).unwrap(),
        ];
        let expected = aux.mixing.base[0]
            .mul_base(3)
            .add(aux.mixing.base[1].mul_base(5))
            .add(aux.mixing.table[0].mul_base(7))
            .add(aux.mixing.auxiliary[0].mul(values[0]))
            .add(aux.mixing.auxiliary[1].mul(values[1]));
        assert_eq!(aux.mix_row(&[3, 5], &[7], &values).unwrap(), expected);
        let embedded = [
            GoldilocksFp4V1::from_base(11).unwrap(),
            GoldilocksFp4V1::from_base(13).unwrap(),
        ];
        assert_eq!(
            aux.mix_row(&[0, 0], &[0], &embedded).unwrap(),
            aux.mixing.auxiliary[0]
                .mul_base(11)
                .add(aux.mixing.auxiliary[1].mul_base(13))
        );
        assert!(aux.mix_row(&[3], &[7], &values).is_err());
        assert!(aux.mix_row(&[3, 5], &[], &values).is_err());
        assert!(aux.mix_row(&[3, 5], &[7], &values[..1]).is_err());
        assert!(
            aux.mix_row(&[crate::GOLDILOCKS_MODULUS_V1, 5], &[7], &values)
                .is_err()
        );
    }

    #[test]
    fn malformed_geometry_lengths_and_noncanonical_coordinates_are_rejected() {
        for rows in [0, 3, 1 << 17, usize::MAX] {
            assert!(PhaseLayout::new(schema(), rows).is_err());
        }
        for (base_columns, table_columns, auxiliary_columns, composition_constraints) in [
            (0, 1, 2, 3),
            (512, 1, 0, 3),
            (1, 0, 128, 3),
            (usize::MAX, 1, 1, 3),
            (1, 0, usize::MAX, 3),
            (1, 0, 0, 0),
            (1, 0, 0, MAX_COMPOSITION_CHALLENGES + 1),
        ] {
            assert!(
                PhaseLayout::new(
                    PhaseSchema {
                        base_columns,
                        table_columns,
                        auxiliary_columns,
                        composition_constraints,
                        ..schema()
                    },
                    1
                )
                .is_err()
            );
        }
        for rows in [0, 7, 9] {
            assert!(
                BaseCommitted::commit(
                    transcript(),
                    PhaseLayout::new(schema(), 1).unwrap(),
                    &base(rows),
                    &table(8)
                )
                .is_err()
            );
            assert!(committed().commit_auxiliary(&auxiliary(rows)).is_err());
            assert!(
                committed()
                    .commit_auxiliary(&auxiliary(8))
                    .unwrap()
                    .commit_mixed(&vec![GoldilocksFp4V1::ZERO; rows])
                    .is_err()
            );
        }
        assert!(
            BaseCommitted::commit(
                transcript(),
                PhaseLayout::new(schema(), 1).unwrap(),
                &base(8)[..1],
                &table(8)
            )
            .is_err()
        );
        assert!(
            BaseCommitted::commit(
                transcript(),
                PhaseLayout::new(schema(), 1).unwrap(),
                &base(8),
                &[]
            )
            .is_err()
        );
        assert!(committed().commit_auxiliary(&auxiliary(8)[..1]).is_err());
        let mut bad_base = base(8);
        bad_base[1][4] = crate::GOLDILOCKS_MODULUS_V1;
        assert!(
            BaseCommitted::commit(
                transcript(),
                PhaseLayout::new(schema(), 1).unwrap(),
                &bad_base,
                &table(8)
            )
            .is_err()
        );
        for lane in 0..4 {
            let mut words = [0; 4];
            words[lane] = crate::GOLDILOCKS_MODULUS_V1;
            let bad = GoldilocksFp4V1::from_coefficients_unchecked_for_test(words);
            let mut columns = auxiliary(8);
            columns[1][4] = bad;
            assert!(committed().commit_auxiliary(&columns).is_err());
            let aux = committed().commit_auxiliary(&auxiliary(8)).unwrap();
            assert!(
                aux.mix_row(&[3, 5], &[7], &[bad, GoldilocksFp4V1::ZERO])
                    .is_err()
            );
            assert!(aux.commit_mixed(&[bad; 8]).is_err());
        }
    }

    #[test]
    fn empty_optional_oracles_have_typed_geometry_bound_roots() {
        let schema = PhaseSchema {
            table_columns: 0,
            auxiliary_columns: 0,
            ..schema()
        };
        let layout = PhaseLayout::new(schema, 1).unwrap();
        let phase = BaseCommitted::commit(transcript(), layout, &base(8), &[]).unwrap();
        let mut bad_roots = phase.roots();
        bad_roots.table = digest(900);
        assert!(
            BaseCommitted::replay(
                transcript(),
                PhaseLayout::new(schema, 1).unwrap(),
                bad_roots
            )
            .is_err()
        );
        assert!(
            BaseCommitted::replay(
                transcript(),
                PhaseLayout::new(schema, 1).unwrap(),
                phase.roots()
            )
            .unwrap()
            .replay_auxiliary(digest(901))
            .is_err()
        );
        let table_root = phase.roots().table;
        let aux = phase.commit_auxiliary(&[]).unwrap();
        assert_ne!(table_root, aux.auxiliary_root());
        assert!(aux.mixing.table.is_empty());
        assert!(aux.mixing.auxiliary.is_empty());
        assert!(aux.mix_row(&[3, 5], &[], &[]).is_ok());
        let other = BaseCommitted::commit(
            transcript(),
            PhaseLayout::new(schema, 2).unwrap(),
            &base(16),
            &[],
        )
        .unwrap();
        assert_ne!(table_root, other.roots().table);
    }

    #[test]
    fn phase_frames_and_replay_ignore_and_restore_ambient_norito_flags() {
        let baseline = finish(committed().commit_auxiliary(&auxiliary(8)).unwrap());
        let expected_root = baseline.mixed_root();
        let expected_alphas = baseline.alphas().to_vec();
        let expected_next = baseline.into_transcript().challenge_extension("next");
        for flags in [
            0,
            norito::core::header_flags::PACKED_SEQ,
            norito::core::header_flags::PACKED_STRUCT | norito::core::header_flags::COMPACT_LEN,
        ] {
            let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
            let before = norito::core::to_bytes(&vec![1_u64, 2, 3]).unwrap();
            let phase = finish(committed().commit_auxiliary(&auxiliary(8)).unwrap());
            assert_eq!(phase.mixed_root(), expected_root);
            assert_eq!(phase.alphas(), expected_alphas);
            assert_eq!(
                phase.into_transcript().challenge_extension("next"),
                expected_next
            );
            assert_eq!(norito::core::to_bytes(&vec![1_u64, 2, 3]).unwrap(), before);
        }
    }
}
