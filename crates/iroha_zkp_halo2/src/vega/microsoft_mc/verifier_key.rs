//! Exact bounded Microsoft Vega-MC verifier-key codec and digest.

use super::super::{
    VegaMdlProofDimensionsV1, VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar,
};
use super::{
    sha256::Sha256,
    wire::{McCodecError, Reader, write_len, write_point, write_scalar},
};

const DEFAULT_COMMITMENT_WIDTH: usize = 2_048;
const MAX_VERIFIER_KEY_BYTES: usize = 512 * 1024 * 1024;
const MAX_KEY_COLUMNS: usize = DEFAULT_COMMITMENT_WIDTH;
const MAX_MATRIX_ENTRIES: usize = 1 << 26;
const MAX_MATRIX_ROWS: usize = 1 << 22;
const MAX_VERIFIER_ROUNDS: usize = 256;

/// Canonical Hyrax commitment or verifier key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct HyraxKeyWire {
    pub(super) columns: usize,
    pub(super) generators: Vec<Point>,
    pub(super) hiding_generator: Point,
}

/// Canonical sparse matrix in compressed-row form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SparseMatrixWire {
    pub(super) data: Vec<Scalar>,
    pub(super) indices: Vec<usize>,
    pub(super) row_offsets: Vec<usize>,
    pub(super) columns: usize,
}

impl SparseMatrixWire {
    pub(super) fn rows(&self) -> usize {
        self.row_offsets.len() - 1
    }

    pub(super) fn row_entries(
        &self,
        row: usize,
    ) -> Option<impl Iterator<Item = (usize, Scalar)> + '_> {
        let bounds = self.row_offsets.get(row..=row + 1)?;
        Some((bounds[0]..bounds[1]).map(|index| (self.indices[index], self.data[index])))
    }

    pub(super) fn evaluate(
        &self,
        row_weights: &[Scalar],
        column_weights: &[Scalar],
    ) -> Result<Scalar, McCodecError> {
        if row_weights.len() < self.rows() || column_weights.len() < self.columns {
            return Err(McCodecError::InvalidEncoding);
        }
        let mut result = Scalar::zero();
        for (row, row_weight) in row_weights.iter().copied().take(self.rows()).enumerate() {
            for (column, coefficient) in
                self.row_entries(row).ok_or(McCodecError::InvalidEncoding)?
            {
                result += row_weight * coefficient * column_weights[column];
            }
        }
        Ok(result)
    }
}

/// Application split-R1CS shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SplitShapeWire {
    pub(super) constraints: usize,
    pub(super) constraints_unpadded: usize,
    pub(super) shared_unpadded: usize,
    pub(super) precommitted_unpadded: usize,
    pub(super) rest_unpadded: usize,
    pub(super) shared: usize,
    pub(super) precommitted: usize,
    pub(super) rest: usize,
    pub(super) public_values: usize,
    pub(super) challenges: usize,
    pub(super) a: SparseMatrixWire,
    pub(super) b: SparseMatrixWire,
    pub(super) c: SparseMatrixWire,
}

impl SplitShapeWire {
    pub(super) fn variables(&self) -> Result<usize, McCodecError> {
        self.shared
            .checked_add(self.precommitted)
            .and_then(|value| value.checked_add(self.rest))
            .ok_or(McCodecError::InvalidEncoding)
    }
}

/// Multi-round verifier-circuit shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct MultiRoundShapeWire {
    pub(super) constraints: usize,
    pub(super) constraints_unpadded: usize,
    pub(super) rounds: usize,
    pub(super) variables_per_round_unpadded: Vec<usize>,
    pub(super) variables_per_round: Vec<usize>,
    pub(super) challenges_per_round: Vec<usize>,
    pub(super) public_values: usize,
    pub(super) commitment_width: usize,
    pub(super) a: SparseMatrixWire,
    pub(super) b: SparseMatrixWire,
    pub(super) c: SparseMatrixWire,
}

/// Regular relaxed verifier-circuit shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RegularShapeWire {
    pub(super) constraints: usize,
    pub(super) variables: usize,
    pub(super) public_values: usize,
    pub(super) a: SparseMatrixWire,
    pub(super) b: SparseMatrixWire,
    pub(super) c: SparseMatrixWire,
}

/// Exact canonical Microsoft Vega-MC verifier key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct McVerifierKeyWire {
    pub(super) application_key: HyraxKeyWire,
    pub(super) evaluation_key: HyraxKeyWire,
    pub(super) step_shape: SplitShapeWire,
    pub(super) core_shape: SplitShapeWire,
    pub(super) verifier_shape: MultiRoundShapeWire,
    pub(super) verifier_regular_shape: RegularShapeWire,
    pub(super) verifier_commitment_key: HyraxKeyWire,
    pub(super) verifier_evaluation_key: HyraxKeyWire,
    pub(super) num_steps: usize,
}

impl McVerifierKeyWire {
    /// Decode one trusted-key candidate under absolute allocation bounds.
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, McCodecError> {
        if bytes.is_empty() || bytes.len() > MAX_VERIFIER_KEY_BYTES {
            return Err(McCodecError::InvalidEncoding);
        }
        let mut reader = Reader::new(bytes);
        let key = Self {
            application_key: read_hyrax_key(&mut reader)?,
            evaluation_key: read_hyrax_key(&mut reader)?,
            step_shape: read_split_shape(&mut reader)?,
            core_shape: read_split_shape(&mut reader)?,
            verifier_shape: read_multi_round_shape(&mut reader)?,
            verifier_regular_shape: read_regular_shape(&mut reader)?,
            verifier_commitment_key: read_hyrax_key(&mut reader)?,
            verifier_evaluation_key: read_hyrax_key(&mut reader)?,
            num_steps: reader.encoded_len()?,
        };
        reader.finish()?;
        key.validate()?;
        Ok(key)
    }

    /// Encode the ordinary fixed-little-endian verifier-key representation.
    pub(super) fn encode(&self) -> Result<Vec<u8>, McCodecError> {
        self.validate()?;
        let mut output = Vec::new();
        write_hyrax_key(&mut output, &self.application_key)?;
        write_hyrax_key(&mut output, &self.evaluation_key)?;
        write_split_shape(&mut output, &self.step_shape)?;
        write_split_shape(&mut output, &self.core_shape)?;
        write_multi_round_shape(&mut output, &self.verifier_shape)?;
        write_regular_shape(&mut output, &self.verifier_regular_shape)?;
        write_hyrax_key(&mut output, &self.verifier_commitment_key)?;
        write_hyrax_key(&mut output, &self.verifier_evaluation_key)?;
        write_usize(&mut output, self.num_steps)?;
        if output.len() > MAX_VERIFIER_KEY_BYTES {
            return Err(McCodecError::InvalidEncoding);
        }
        Ok(output)
    }

    /// Compute the exact mixed raw/bincode SHA-256 verifier-key digest.
    pub(super) fn digest(&self) -> Result<[u8; 32], McCodecError> {
        self.validate()?;
        let mut digest = Sha256::new();
        digest_hyrax_key(&mut digest, &self.application_key)?;
        digest_hyrax_key(&mut digest, &self.evaluation_key)?;
        digest_split_shape_raw(&mut digest, &self.step_shape)?;
        digest_split_shape_raw(&mut digest, &self.core_shape)?;
        digest_multi_round_shape(&mut digest, &self.verifier_shape)?;
        digest_regular_shape(&mut digest, &self.verifier_regular_shape)?;
        digest_hyrax_key(&mut digest, &self.verifier_commitment_key)?;
        digest_hyrax_key(&mut digest, &self.verifier_evaluation_key)?;
        digest_usize(&mut digest, self.num_steps)?;
        Ok(digest.finalize())
    }

    /// Derive all exact proof sequence lengths from the verifier key.
    pub(super) fn proof_dimensions(&self) -> Result<VegaMdlProofDimensionsV1, McCodecError> {
        self.validate()?;
        let commitment_points = |values: usize, width: usize| values.div_ceil(width);
        let width = self.verifier_shape.commitment_width;
        let verifier_round_commitment_points = self
            .verifier_shape
            .variables_per_round
            .iter()
            .map(|values| commitment_points(*values, width))
            .collect();
        let step_variables = self.step_shape.variables()?;
        let core_variables = self.core_shape.variables()?;
        Ok(VegaMdlProofDimensionsV1 {
            num_steps: self.num_steps,
            shared_variables: self.step_shape.shared,
            step_precommitted_variables: self.step_shape.precommitted,
            step_rest_variables: self.step_shape.rest,
            core_precommitted_variables: self.core_shape.precommitted,
            core_rest_variables: self.core_shape.rest,
            step_constraints: self.step_shape.constraints,
            step_variables,
            core_constraints: self.core_shape.constraints,
            core_variables,
            shared_commitment_points: commitment_points(
                self.step_shape.shared,
                DEFAULT_COMMITMENT_WIDTH,
            ),
            step_precommitted_points: commitment_points(
                self.step_shape.precommitted,
                DEFAULT_COMMITMENT_WIDTH,
            ),
            step_rest_points: commitment_points(self.step_shape.rest, DEFAULT_COMMITMENT_WIDTH),
            step_public_values: self.step_shape.public_values,
            step_challenges: self.step_shape.challenges,
            core_precommitted_points: commitment_points(
                self.core_shape.precommitted,
                DEFAULT_COMMITMENT_WIDTH,
            ),
            core_rest_points: commitment_points(self.core_shape.rest, DEFAULT_COMMITMENT_WIDTH),
            core_public_values: self.core_shape.public_values,
            core_challenges: self.core_shape.challenges,
            evaluation_response_scalars: DEFAULT_COMMITMENT_WIDTH,
            verifier_round_commitment_points,
            verifier_public_values: self.verifier_shape.public_values,
            verifier_challenges_per_round: self.verifier_shape.challenges_per_round.clone(),
            nova_cross_term_points: commitment_points(
                self.verifier_regular_shape.constraints,
                width,
            ),
            random_witness_commitment_points: commitment_points(
                self.verifier_regular_shape.variables,
                width,
            ),
            random_error_commitment_points: commitment_points(
                self.verifier_regular_shape.constraints,
                width,
            ),
            random_public_values: self.verifier_regular_shape.public_values,
            verifier_constraints: self.verifier_regular_shape.constraints,
            verifier_variables: self.verifier_regular_shape.variables,
            relaxed_outer_rounds: log2_exact(self.verifier_regular_shape.constraints)?,
            relaxed_outer_coefficients: 3,
            relaxed_inner_rounds: log2_exact(
                self.verifier_regular_shape
                    .variables
                    .checked_next_power_of_two()
                    .ok_or(McCodecError::InvalidEncoding)?,
            )?
            .checked_add(1)
            .ok_or(McCodecError::InvalidEncoding)?,
            relaxed_inner_coefficients: 2,
            relaxed_opening_scalars: width,
        })
    }

    fn validate(&self) -> Result<(), McCodecError> {
        validate_hyrax_key(&self.application_key)?;
        validate_hyrax_key(&self.evaluation_key)?;
        validate_hyrax_key(&self.verifier_commitment_key)?;
        validate_hyrax_key(&self.verifier_evaluation_key)?;
        if self.application_key != self.evaluation_key
            || self.verifier_commitment_key != self.verifier_evaluation_key
            || self.num_steps < 2
        {
            return Err(McCodecError::InvalidEncoding);
        }
        validate_split_shape(&self.step_shape)?;
        validate_split_shape(&self.core_shape)?;
        validate_multi_round_shape(&self.verifier_shape)?;
        validate_regular_shape(&self.verifier_regular_shape)?;
        if self.step_shape.shared != self.core_shape.shared
            || self.step_shape.constraints != self.core_shape.constraints
            || self.step_shape.variables()? != self.core_shape.variables()?
            || self.application_key.columns != DEFAULT_COMMITMENT_WIDTH
            || self.verifier_commitment_key.columns != self.verifier_shape.commitment_width
            || self.verifier_shape.constraints != self.verifier_regular_shape.constraints
            || self
                .verifier_shape
                .variables_per_round
                .iter()
                .try_fold(0_usize, |total, value| total.checked_add(*value))
                .ok_or(McCodecError::InvalidEncoding)?
                != self.verifier_regular_shape.variables
            || self
                .verifier_shape
                .challenges_per_round
                .iter()
                .try_fold(self.verifier_shape.public_values, |total, value| {
                    total.checked_add(*value)
                })
                .ok_or(McCodecError::InvalidEncoding)?
                != self.verifier_regular_shape.public_values
        {
            return Err(McCodecError::InvalidEncoding);
        }
        Ok(())
    }
}

fn read_hyrax_key(reader: &mut Reader<'_>) -> Result<HyraxKeyWire, McCodecError> {
    let columns = reader.encoded_len()?;
    if columns == 0 || columns > MAX_KEY_COLUMNS {
        return Err(McCodecError::InvalidEncoding);
    }
    let generators = read_points(reader, columns)?;
    Ok(HyraxKeyWire {
        columns,
        generators,
        hiding_generator: reader.point()?,
    })
}

fn read_points(reader: &mut Reader<'_>, expected: usize) -> Result<Vec<Point>, McCodecError> {
    if reader.encoded_len()? != expected
        || expected
            .checked_mul(33)
            .is_none_or(|bytes| bytes > reader.remaining())
    {
        return Err(McCodecError::InvalidEncoding);
    }
    let mut points = Vec::with_capacity(expected);
    for _ in 0..expected {
        points.push(reader.point()?);
    }
    Ok(points)
}

fn read_sparse_matrix(reader: &mut Reader<'_>) -> Result<SparseMatrixWire, McCodecError> {
    let data_len = bounded_count(reader, 32, MAX_MATRIX_ENTRIES)?;
    let mut data = Vec::with_capacity(data_len);
    for _ in 0..data_len {
        data.push(reader.scalar()?);
    }
    let indices_len = bounded_count(reader, 8, MAX_MATRIX_ENTRIES)?;
    if indices_len != data_len {
        return Err(McCodecError::InvalidEncoding);
    }
    let mut indices = Vec::with_capacity(indices_len);
    for _ in 0..indices_len {
        indices.push(reader.encoded_len()?);
    }
    let offsets_len = bounded_count(reader, 8, MAX_MATRIX_ROWS + 1)?;
    if offsets_len == 0 {
        return Err(McCodecError::InvalidEncoding);
    }
    let mut row_offsets = Vec::with_capacity(offsets_len);
    for _ in 0..offsets_len {
        row_offsets.push(reader.encoded_len()?);
    }
    let columns = reader.encoded_len()?;
    let matrix = SparseMatrixWire {
        data,
        indices,
        row_offsets,
        columns,
    };
    validate_sparse_matrix(&matrix)?;
    Ok(matrix)
}

fn bounded_count(
    reader: &mut Reader<'_>,
    element_bytes: usize,
    maximum: usize,
) -> Result<usize, McCodecError> {
    let count = reader.encoded_len()?;
    if count > maximum
        || count
            .checked_mul(element_bytes)
            .is_none_or(|bytes| bytes > reader.remaining())
    {
        return Err(McCodecError::InvalidEncoding);
    }
    Ok(count)
}

fn read_split_shape(reader: &mut Reader<'_>) -> Result<SplitShapeWire, McCodecError> {
    let shape = SplitShapeWire {
        constraints: reader.encoded_len()?,
        constraints_unpadded: reader.encoded_len()?,
        shared_unpadded: reader.encoded_len()?,
        precommitted_unpadded: reader.encoded_len()?,
        rest_unpadded: reader.encoded_len()?,
        shared: reader.encoded_len()?,
        precommitted: reader.encoded_len()?,
        rest: reader.encoded_len()?,
        public_values: reader.encoded_len()?,
        challenges: reader.encoded_len()?,
        a: read_sparse_matrix(reader)?,
        b: read_sparse_matrix(reader)?,
        c: read_sparse_matrix(reader)?,
    };
    validate_split_shape(&shape)?;
    Ok(shape)
}

fn read_multi_round_shape(reader: &mut Reader<'_>) -> Result<MultiRoundShapeWire, McCodecError> {
    let constraints = reader.encoded_len()?;
    let constraints_unpadded = reader.encoded_len()?;
    let rounds = reader.encoded_len()?;
    if rounds == 0 || rounds > MAX_VERIFIER_ROUNDS {
        return Err(McCodecError::InvalidEncoding);
    }
    let shape = MultiRoundShapeWire {
        constraints,
        constraints_unpadded,
        rounds,
        variables_per_round_unpadded: read_usize_vec(reader, rounds)?,
        variables_per_round: read_usize_vec(reader, rounds)?,
        challenges_per_round: read_usize_vec(reader, rounds)?,
        public_values: reader.encoded_len()?,
        commitment_width: reader.encoded_len()?,
        a: read_sparse_matrix(reader)?,
        b: read_sparse_matrix(reader)?,
        c: read_sparse_matrix(reader)?,
    };
    validate_multi_round_shape(&shape)?;
    Ok(shape)
}

fn read_regular_shape(reader: &mut Reader<'_>) -> Result<RegularShapeWire, McCodecError> {
    let shape = RegularShapeWire {
        constraints: reader.encoded_len()?,
        variables: reader.encoded_len()?,
        public_values: reader.encoded_len()?,
        a: read_sparse_matrix(reader)?,
        b: read_sparse_matrix(reader)?,
        c: read_sparse_matrix(reader)?,
    };
    validate_regular_shape(&shape)?;
    Ok(shape)
}

fn read_usize_vec(reader: &mut Reader<'_>, expected: usize) -> Result<Vec<usize>, McCodecError> {
    if expected > MAX_VERIFIER_ROUNDS || reader.encoded_len()? != expected {
        return Err(McCodecError::InvalidEncoding);
    }
    let mut values = Vec::with_capacity(expected);
    for _ in 0..expected {
        values.push(reader.encoded_len()?);
    }
    Ok(values)
}

fn validate_hyrax_key(key: &HyraxKeyWire) -> Result<(), McCodecError> {
    if key.columns == 0
        || key.columns > MAX_KEY_COLUMNS
        || key.generators.len() != key.columns
        || key.hiding_generator.is_identity()
    {
        return Err(McCodecError::InvalidEncoding);
    }
    for (index, point) in key.generators.iter().copied().enumerate() {
        if point.is_identity()
            || key
                .generators
                .iter()
                .copied()
                .skip(index + 1)
                .any(|other| other == point || other == point.negate())
            || point == key.hiding_generator
            || point == key.hiding_generator.negate()
        {
            return Err(McCodecError::InvalidEncoding);
        }
    }
    Ok(())
}

fn validate_sparse_matrix(matrix: &SparseMatrixWire) -> Result<(), McCodecError> {
    if matrix.columns == 0
        || matrix.data.len() != matrix.indices.len()
        || matrix.row_offsets.is_empty()
        || matrix.row_offsets[0] != 0
        || matrix.row_offsets.last().copied() != Some(matrix.data.len())
        || matrix
            .row_offsets
            .windows(2)
            .any(|window| window[0] > window[1])
        || matrix.indices.iter().any(|index| *index >= matrix.columns)
    {
        return Err(McCodecError::InvalidEncoding);
    }
    Ok(())
}

fn validate_split_shape(shape: &SplitShapeWire) -> Result<(), McCodecError> {
    let variables = shape.variables()?;
    let columns = variables
        .checked_add(1)
        .and_then(|value| value.checked_add(shape.public_values))
        .and_then(|value| value.checked_add(shape.challenges))
        .ok_or(McCodecError::InvalidEncoding)?;
    if shape.constraints == 0
        || variables == 0
        || !shape.constraints.is_power_of_two()
        || !variables.is_power_of_two()
        || shape.constraints_unpadded > shape.constraints
        || shape.shared_unpadded > shape.shared
        || shape.precommitted_unpadded > shape.precommitted
        || shape.rest_unpadded > shape.rest
        || [&shape.a, &shape.b, &shape.c]
            .iter()
            .any(|matrix| matrix.rows() != shape.constraints || matrix.columns != columns)
    {
        return Err(McCodecError::InvalidEncoding);
    }
    Ok(())
}

fn validate_multi_round_shape(shape: &MultiRoundShapeWire) -> Result<(), McCodecError> {
    let variable_total = shape
        .variables_per_round
        .iter()
        .try_fold(0_usize, |total, value| total.checked_add(*value))
        .ok_or(McCodecError::InvalidEncoding)?;
    let challenge_total = shape
        .challenges_per_round
        .iter()
        .try_fold(0_usize, |total, value| total.checked_add(*value))
        .ok_or(McCodecError::InvalidEncoding)?;
    let columns = variable_total
        .checked_add(1)
        .and_then(|value| value.checked_add(shape.public_values))
        .and_then(|value| value.checked_add(challenge_total))
        .ok_or(McCodecError::InvalidEncoding)?;
    if shape.constraints == 0
        || !shape.constraints.is_power_of_two()
        || shape.constraints_unpadded > shape.constraints
        || shape.rounds == 0
        || shape.rounds > MAX_VERIFIER_ROUNDS
        || shape.variables_per_round_unpadded.len() != shape.rounds
        || shape.variables_per_round.len() != shape.rounds
        || shape.challenges_per_round.len() != shape.rounds
        || shape.commitment_width == 0
        || !shape.commitment_width.is_power_of_two()
        || shape
            .variables_per_round_unpadded
            .iter()
            .zip(&shape.variables_per_round)
            .any(|(unpadded, padded)| *unpadded > *padded || *padded == 0)
        || [&shape.a, &shape.b, &shape.c]
            .iter()
            .any(|matrix| matrix.rows() != shape.constraints || matrix.columns != columns)
    {
        return Err(McCodecError::InvalidEncoding);
    }
    Ok(())
}

fn validate_regular_shape(shape: &RegularShapeWire) -> Result<(), McCodecError> {
    let columns = shape
        .variables
        .checked_add(1)
        .and_then(|value| value.checked_add(shape.public_values))
        .ok_or(McCodecError::InvalidEncoding)?;
    if shape.constraints == 0
        || shape.variables == 0
        || !shape.constraints.is_power_of_two()
        || [&shape.a, &shape.b, &shape.c]
            .iter()
            .any(|matrix| matrix.rows() != shape.constraints || matrix.columns != columns)
    {
        return Err(McCodecError::InvalidEncoding);
    }
    Ok(())
}

fn write_hyrax_key(output: &mut Vec<u8>, key: &HyraxKeyWire) -> Result<(), McCodecError> {
    write_usize(output, key.columns)?;
    write_len(output, key.generators.len())?;
    for point in &key.generators {
        write_point(output, *point)?;
    }
    write_point(output, key.hiding_generator)
}

fn write_sparse_matrix(
    output: &mut Vec<u8>,
    matrix: &SparseMatrixWire,
) -> Result<(), McCodecError> {
    write_len(output, matrix.data.len())?;
    for value in &matrix.data {
        write_scalar(output, *value);
    }
    write_len(output, matrix.indices.len())?;
    for index in &matrix.indices {
        write_usize(output, *index)?;
    }
    write_len(output, matrix.row_offsets.len())?;
    for offset in &matrix.row_offsets {
        write_usize(output, *offset)?;
    }
    write_usize(output, matrix.columns)
}

fn write_split_shape(output: &mut Vec<u8>, shape: &SplitShapeWire) -> Result<(), McCodecError> {
    for value in split_dimensions(shape) {
        write_usize(output, value)?;
    }
    write_sparse_matrix(output, &shape.a)?;
    write_sparse_matrix(output, &shape.b)?;
    write_sparse_matrix(output, &shape.c)
}

fn write_multi_round_shape(
    output: &mut Vec<u8>,
    shape: &MultiRoundShapeWire,
) -> Result<(), McCodecError> {
    write_usize(output, shape.constraints)?;
    write_usize(output, shape.constraints_unpadded)?;
    write_usize(output, shape.rounds)?;
    write_usize_vec(output, &shape.variables_per_round_unpadded)?;
    write_usize_vec(output, &shape.variables_per_round)?;
    write_usize_vec(output, &shape.challenges_per_round)?;
    write_usize(output, shape.public_values)?;
    write_usize(output, shape.commitment_width)?;
    write_sparse_matrix(output, &shape.a)?;
    write_sparse_matrix(output, &shape.b)?;
    write_sparse_matrix(output, &shape.c)
}

fn write_regular_shape(output: &mut Vec<u8>, shape: &RegularShapeWire) -> Result<(), McCodecError> {
    write_usize(output, shape.constraints)?;
    write_usize(output, shape.variables)?;
    write_usize(output, shape.public_values)?;
    write_sparse_matrix(output, &shape.a)?;
    write_sparse_matrix(output, &shape.b)?;
    write_sparse_matrix(output, &shape.c)
}

fn write_usize(output: &mut Vec<u8>, value: usize) -> Result<(), McCodecError> {
    output.extend_from_slice(
        &u64::try_from(value)
            .map_err(|_| McCodecError::InvalidEncoding)?
            .to_le_bytes(),
    );
    Ok(())
}

fn write_usize_vec(output: &mut Vec<u8>, values: &[usize]) -> Result<(), McCodecError> {
    write_len(output, values.len())?;
    for value in values {
        write_usize(output, *value)?;
    }
    Ok(())
}

fn digest_hyrax_key(digest: &mut Sha256, key: &HyraxKeyWire) -> Result<(), McCodecError> {
    digest_usize(digest, key.columns)?;
    digest_usize(digest, key.generators.len())?;
    for point in &key.generators {
        digest
            .update(&point.to_non_identity_wire_bytes()?)
            .map_err(|_| McCodecError::InvalidEncoding)?;
    }
    digest
        .update(&key.hiding_generator.to_non_identity_wire_bytes()?)
        .map_err(|_| McCodecError::InvalidEncoding)
}

fn digest_split_shape_raw(digest: &mut Sha256, shape: &SplitShapeWire) -> Result<(), McCodecError> {
    for value in split_dimensions(shape) {
        digest_usize(digest, value)?;
    }
    digest_sparse_matrix_raw(digest, &shape.a)?;
    digest_sparse_matrix_raw(digest, &shape.b)?;
    digest_sparse_matrix_raw(digest, &shape.c)
}

fn digest_sparse_matrix_raw(
    digest: &mut Sha256,
    matrix: &SparseMatrixWire,
) -> Result<(), McCodecError> {
    digest_usize(digest, matrix.data.len())?;
    digest_usize(digest, matrix.indices.len())?;
    digest_usize(digest, matrix.row_offsets.len())?;
    digest_usize(digest, matrix.columns)?;
    for value in &matrix.data {
        digest
            .update(&value.to_le_bytes())
            .map_err(|_| McCodecError::InvalidEncoding)?;
    }
    for index in &matrix.indices {
        digest_usize(digest, *index)?;
    }
    for offset in &matrix.row_offsets {
        digest_usize(digest, *offset)?;
    }
    Ok(())
}

fn digest_multi_round_shape(
    digest: &mut Sha256,
    shape: &MultiRoundShapeWire,
) -> Result<(), McCodecError> {
    let mut encoded = Vec::new();
    write_multi_round_shape(&mut encoded, shape)?;
    digest
        .update(&encoded)
        .map_err(|_| McCodecError::InvalidEncoding)
}

fn digest_regular_shape(digest: &mut Sha256, shape: &RegularShapeWire) -> Result<(), McCodecError> {
    let mut encoded = Vec::new();
    write_regular_shape(&mut encoded, shape)?;
    digest
        .update(&encoded)
        .map_err(|_| McCodecError::InvalidEncoding)
}

fn digest_usize(digest: &mut Sha256, value: usize) -> Result<(), McCodecError> {
    digest
        .update(
            &u64::try_from(value)
                .map_err(|_| McCodecError::InvalidEncoding)?
                .to_le_bytes(),
        )
        .map_err(|_| McCodecError::InvalidEncoding)
}

fn split_dimensions(shape: &SplitShapeWire) -> [usize; 10] {
    [
        shape.constraints,
        shape.constraints_unpadded,
        shape.shared_unpadded,
        shape.precommitted_unpadded,
        shape.rest_unpadded,
        shape.shared,
        shape.precommitted,
        shape.rest,
        shape.public_values,
        shape.challenges,
    ]
}

fn log2_exact(value: usize) -> Result<usize, McCodecError> {
    if value == 0 || !value.is_power_of_two() {
        return Err(McCodecError::InvalidEncoding);
    }
    usize::try_from(value.ilog2()).map_err(|_| McCodecError::InvalidEncoding)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vega::microsoft_mc::{sha256::sha256, wire::McProofWire};

    const PYTHON_VK: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../vendor/vega-prover/reference/fixtures/cubic/python_vk.bin"
    ));
    const PYTHON_PROOF: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../vendor/vega-prover/reference/fixtures/cubic/python_standalone_proof.bin"
    ));

    #[test]
    fn python_verifier_key_sections_decode_and_validate_independently() {
        let mut reader = Reader::new(PYTHON_VK);
        let application_key = read_hyrax_key(&mut reader).expect("application key");
        let evaluation_key = read_hyrax_key(&mut reader).expect("application evaluation key");
        let step_shape = read_split_shape(&mut reader).expect("step shape");
        let core_shape = read_split_shape(&mut reader).expect("core shape");
        let verifier_shape = read_multi_round_shape(&mut reader).expect("verifier split shape");
        let verifier_regular_shape =
            read_regular_shape(&mut reader).expect("verifier regular shape");
        let verifier_commitment_key = read_hyrax_key(&mut reader).expect("verifier commitment key");
        let verifier_evaluation_key = read_hyrax_key(&mut reader).expect("verifier evaluation key");
        let num_steps = reader.encoded_len().expect("step count");
        reader.finish().expect("no trailing verifier-key bytes");

        validate_hyrax_key(&application_key).expect("valid application key");
        validate_hyrax_key(&evaluation_key).expect("valid application evaluation key");
        validate_split_shape(&step_shape).expect("valid step shape");
        validate_split_shape(&core_shape).expect("valid core shape");
        validate_multi_round_shape(&verifier_shape).expect("valid verifier split shape");
        validate_regular_shape(&verifier_regular_shape).expect("valid verifier regular shape");
        validate_hyrax_key(&verifier_commitment_key).expect("valid verifier commitment key");
        validate_hyrax_key(&verifier_evaluation_key).expect("valid verifier evaluation key");
        assert_eq!(application_key, evaluation_key);
        assert_eq!(verifier_commitment_key, verifier_evaluation_key);
        assert_eq!(num_steps, 2);
    }

    #[test]
    fn python_verifier_key_and_proof_roundtrip_exactly() {
        assert_eq!(
            hex::encode(sha256(PYTHON_VK).expect("bounded fixture")),
            "fdb982961889d7fe5757bf12b12a3a8b9fb18f764c024ad179d5eb145dec5b2e"
        );
        let key = McVerifierKeyWire::decode(PYTHON_VK).expect("canonical Python key");
        assert_eq!(key.encode().expect("canonical key encoding"), PYTHON_VK);
        assert_eq!(
            hex::encode(key.digest().expect("canonical key digest")),
            "b752511606285b40d5a1ea19ba3f6b4e7d6f90cc29036cf4b59cfd5121dc2729"
        );
        let dimensions = key.proof_dimensions().expect("key-derived bounds");
        let proof = McProofWire::decode(PYTHON_PROOF, &dimensions)
            .expect("canonical independent Python proof");
        assert_eq!(
            proof.encode().expect("canonical proof encoding"),
            PYTHON_PROOF
        );
    }

    #[test]
    fn verifier_key_decoder_rejects_trailing_truncated_and_length_bomb_inputs() {
        let mut trailing = PYTHON_VK.to_vec();
        trailing.push(0);
        assert_eq!(
            McVerifierKeyWire::decode(&trailing),
            Err(McCodecError::InvalidEncoding)
        );
        for cut in [0, 1, 7, PYTHON_VK.len() / 2, PYTHON_VK.len() - 1] {
            assert_eq!(
                McVerifierKeyWire::decode(&PYTHON_VK[..cut]),
                Err(McCodecError::InvalidEncoding)
            );
        }
        let mut bomb = PYTHON_VK.to_vec();
        bomb[8..16].copy_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(
            McVerifierKeyWire::decode(&bomb),
            Err(McCodecError::InvalidEncoding)
        );
    }
}
