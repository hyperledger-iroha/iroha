//! SHAKE256 transcript and transparent matrix expansion for Bootle/Lantern.
//!
//! Every field is framed as an unsigned 32-bit big-endian byte length followed
//! by the field itself. Matrix coefficients are independently derived from the
//! complete tuple
//! `(domain, parameter_digest, ppseed, role, rows, columns, row, column,
//! coefficient, rejection_counter)`. This makes parallel expansion and random
//! access byte-for-byte identical and prevents stream-position ambiguity.

use sha3::{
    Shake256,
    digest::{ExtendableOutput, Update, XofReader},
};
use thiserror::Error;

use super::{
    params::{APPLICATION_MODULUS_V1, APPLICATION_RING_DEGREE_V1, PROOF_MODULUS_V1},
    ring::{ApplicationPolynomialV1, ProofPolynomialV1},
};

const MATRIX_DOMAIN_V1: &[u8] = b"iroha.privacy.bootle-lantern.matrix.v1";
const PRESENTATION_CHALLENGE_DOMAIN_V1: &[u8] =
    b"iroha.privacy.bootle-lantern.presentation-challenge.v1";
const PRESENTATION_STAGE_DOMAIN_V1: &[u8] =
    b"iroha.privacy.bootle-lantern.presentation-stage.v1";
const MAX_UNIFORM_REJECTION_ATTEMPTS_V1: u32 = 4_096;
const APPLICATION_ACCEPTANCE_LIMIT_V1: u16 = 61_445;
const PROOF_ACCEPTANCE_LIMIT_V1: u64 = 70_931_694_131_122_923;

/// Closed transparent-matrix roles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatrixRoleV1 {
    /// `A_r` in `R_p^(8 x 16)`.
    ApplicationRandomness,
    /// `A_m` in `R_p^(8 x 8)`.
    ApplicationAttributes,
    /// `A_tau` in `R_p^(8 x 8)`.
    ApplicationTag,
    /// Internal `A_1` in `R_q^(20 x 50)`.
    InternalA1,
    /// Internal `A'_2` in `R_q^(20 x 44)`.
    InternalA2Prime,
    /// Internal `B'` in `R_q^(12 x 44)`.
    InternalBPrime,
}

impl MatrixRoleV1 {
    /// Stable one-byte derivation tag.
    #[must_use]
    pub const fn tag(self) -> u8 {
        match self {
            Self::ApplicationRandomness => 0x01,
            Self::ApplicationAttributes => 0x02,
            Self::ApplicationTag => 0x03,
            Self::InternalA1 => 0x11,
            Self::InternalA2Prime => 0x12,
            Self::InternalBPrime => 0x13,
        }
    }

    /// Exact matrix dimensions `(rows, columns)`.
    #[must_use]
    pub const fn dimensions(self) -> (u16, u16) {
        match self {
            Self::ApplicationRandomness => (8, 16),
            Self::ApplicationAttributes | Self::ApplicationTag => (8, 8),
            Self::InternalA1 => (20, 50),
            Self::InternalA2Prime => (20, 44),
            Self::InternalBPrime => (12, 44),
        }
    }

    /// Whether this role belongs to the application ring.
    #[must_use]
    pub const fn is_application(self) -> bool {
        matches!(
            self,
            Self::ApplicationRandomness | Self::ApplicationAttributes | Self::ApplicationTag
        )
    }
}

/// Complete seed material for transparent matrices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatrixSeedV1 {
    parameter_digest: [u8; 32],
    public_parameter_seed: [u8; 32],
}

impl MatrixSeedV1 {
    /// Construct non-zero governed seed material.
    ///
    /// # Errors
    ///
    /// Rejects either all-zero digest independently.
    pub fn new(
        parameter_digest: [u8; 32],
        public_parameter_seed: [u8; 32],
    ) -> Result<Self, TranscriptErrorV1> {
        if parameter_digest == [0; 32] {
            return Err(TranscriptErrorV1::ZeroDigest {
                field: "parameter_digest",
            });
        }
        if public_parameter_seed == [0; 32] {
            return Err(TranscriptErrorV1::ZeroDigest { field: "ppseed" });
        }
        Ok(Self {
            parameter_digest,
            public_parameter_seed,
        })
    }

    /// Governed parameter-manifest digest.
    #[must_use]
    pub const fn parameter_digest(&self) -> &[u8; 32] {
        &self.parameter_digest
    }

    /// Public expansion seed.
    #[must_use]
    pub const fn public_parameter_seed(&self) -> &[u8; 32] {
        &self.public_parameter_seed
    }
}

/// Row-major application-ring matrix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationMatrixV1 {
    role: MatrixRoleV1,
    rows: u16,
    columns: u16,
    entries: Box<[ApplicationPolynomialV1]>,
}

impl ApplicationMatrixV1 {
    /// Exact closed role.
    #[must_use]
    pub const fn role(&self) -> MatrixRoleV1 {
        self.role
    }

    /// Exact row count.
    #[must_use]
    pub const fn rows(&self) -> u16 {
        self.rows
    }

    /// Exact column count.
    #[must_use]
    pub const fn columns(&self) -> u16 {
        self.columns
    }

    /// Borrow one entry, returning `None` for an out-of-range coordinate.
    #[must_use]
    pub fn get(&self, row: u16, column: u16) -> Option<&ApplicationPolynomialV1> {
        matrix_index(self.rows, self.columns, row, column).and_then(|index| self.entries.get(index))
    }

    /// Borrow row-major entries.
    #[must_use]
    pub fn entries(&self) -> &[ApplicationPolynomialV1] {
        &self.entries
    }
}

/// Row-major internal proof-ring matrix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProofMatrixV1 {
    role: MatrixRoleV1,
    rows: u16,
    columns: u16,
    entries: Box<[ProofPolynomialV1]>,
}

impl ProofMatrixV1 {
    /// Exact closed role.
    #[must_use]
    pub const fn role(&self) -> MatrixRoleV1 {
        self.role
    }

    /// Exact row count.
    #[must_use]
    pub const fn rows(&self) -> u16 {
        self.rows
    }

    /// Exact column count.
    #[must_use]
    pub const fn columns(&self) -> u16 {
        self.columns
    }

    /// Borrow one entry, returning `None` for an out-of-range coordinate.
    #[must_use]
    pub fn get(&self, row: u16, column: u16) -> Option<&ProofPolynomialV1> {
        matrix_index(self.rows, self.columns, row, column).and_then(|index| self.entries.get(index))
    }

    /// Borrow row-major entries.
    #[must_use]
    pub fn entries(&self) -> &[ProofPolynomialV1] {
        &self.entries
    }
}

/// Expand one complete application matrix.
///
/// # Errors
///
/// Rejects an internal-proof role or an exhausted fixed rejection bound.
pub fn expand_application_matrix_v1(
    seed: MatrixSeedV1,
    role: MatrixRoleV1,
) -> Result<ApplicationMatrixV1, TranscriptErrorV1> {
    if !role.is_application() {
        return Err(TranscriptErrorV1::WrongMatrixRing { role });
    }
    let (rows, columns) = role.dimensions();
    let mut entries = Vec::with_capacity(usize::from(rows) * usize::from(columns));
    for row in 0..rows {
        for column in 0..columns {
            entries.push(derive_application_polynomial_v1(seed, role, row, column)?);
        }
    }
    Ok(ApplicationMatrixV1 {
        role,
        rows,
        columns,
        entries: entries.into_boxed_slice(),
    })
}

/// Expand one complete internal proof matrix.
///
/// # Errors
///
/// Rejects an application role or an exhausted fixed rejection bound.
pub fn expand_proof_matrix_v1(
    seed: MatrixSeedV1,
    role: MatrixRoleV1,
) -> Result<ProofMatrixV1, TranscriptErrorV1> {
    if role.is_application() {
        return Err(TranscriptErrorV1::WrongMatrixRing { role });
    }
    let (rows, columns) = role.dimensions();
    let mut entries = Vec::with_capacity(usize::from(rows) * usize::from(columns));
    for row in 0..rows {
        for column in 0..columns {
            entries.push(derive_proof_polynomial_v1(seed, role, row, column)?);
        }
    }
    Ok(ProofMatrixV1 {
        role,
        rows,
        columns,
        entries: entries.into_boxed_slice(),
    })
}

/// Derive one random-access application matrix entry.
///
/// # Errors
///
/// Rejects a wrong-ring role, an out-of-range coordinate, or exhaustion of
/// the fixed rejection bound.
pub fn derive_application_polynomial_v1(
    seed: MatrixSeedV1,
    role: MatrixRoleV1,
    row: u16,
    column: u16,
) -> Result<ApplicationPolynomialV1, TranscriptErrorV1> {
    validate_coordinate(role, row, column)?;
    if !role.is_application() {
        return Err(TranscriptErrorV1::WrongMatrixRing { role });
    }
    let mut coefficients = [0_u16; APPLICATION_RING_DEGREE_V1];
    for (index, coefficient) in coefficients.iter_mut().enumerate() {
        *coefficient = derive_application_coefficient(
            seed,
            role,
            row,
            column,
            u8::try_from(index).expect("ring index fits u8"),
        )?;
    }
    ApplicationPolynomialV1::new(coefficients).map_err(|_| TranscriptErrorV1::InternalInvariant)
}

/// Derive one random-access internal proof matrix entry.
///
/// # Errors
///
/// Rejects a wrong-ring role, an out-of-range coordinate, or exhaustion of
/// the fixed rejection bound.
pub fn derive_proof_polynomial_v1(
    seed: MatrixSeedV1,
    role: MatrixRoleV1,
    row: u16,
    column: u16,
) -> Result<ProofPolynomialV1, TranscriptErrorV1> {
    validate_coordinate(role, row, column)?;
    if role.is_application() {
        return Err(TranscriptErrorV1::WrongMatrixRing { role });
    }
    let mut coefficients = [0_u64; APPLICATION_RING_DEGREE_V1];
    for (index, coefficient) in coefficients.iter_mut().enumerate() {
        *coefficient = derive_proof_coefficient(
            seed,
            role,
            row,
            column,
            u8::try_from(index).expect("ring index fits u8"),
        )?;
    }
    ProofPolynomialV1::new(coefficients).map_err(|_| TranscriptErrorV1::InternalInvariant)
}

fn validate_coordinate(role: MatrixRoleV1, row: u16, column: u16) -> Result<(), TranscriptErrorV1> {
    let (rows, columns) = role.dimensions();
    if row >= rows || column >= columns {
        return Err(TranscriptErrorV1::MatrixCoordinateOutOfRange {
            role,
            row,
            column,
            rows,
            columns,
        });
    }
    Ok(())
}

fn derive_application_coefficient(
    seed: MatrixSeedV1,
    role: MatrixRoleV1,
    row: u16,
    column: u16,
    coefficient: u8,
) -> Result<u16, TranscriptErrorV1> {
    for counter in 0..MAX_UNIFORM_REJECTION_ATTEMPTS_V1 {
        let candidate = candidate_bytes::<2>(seed, role, row, column, coefficient, counter);
        let candidate = u16::from_be_bytes(candidate);
        if candidate < APPLICATION_ACCEPTANCE_LIMIT_V1 {
            return Ok(candidate % APPLICATION_MODULUS_V1);
        }
    }
    Err(TranscriptErrorV1::UniformRejectionExhausted)
}

fn derive_proof_coefficient(
    seed: MatrixSeedV1,
    role: MatrixRoleV1,
    row: u16,
    column: u16,
    coefficient: u8,
) -> Result<u64, TranscriptErrorV1> {
    for counter in 0..MAX_UNIFORM_REJECTION_ATTEMPTS_V1 {
        let candidate = candidate_bytes::<7>(seed, role, row, column, coefficient, counter);
        let mut wide = [0_u8; 8];
        wide[1..].copy_from_slice(&candidate);
        let candidate = u64::from_be_bytes(wide);
        if candidate < PROOF_ACCEPTANCE_LIMIT_V1 {
            return Ok(candidate % PROOF_MODULUS_V1);
        }
    }
    Err(TranscriptErrorV1::UniformRejectionExhausted)
}

fn candidate_bytes<const N: usize>(
    seed: MatrixSeedV1,
    role: MatrixRoleV1,
    row: u16,
    column: u16,
    coefficient: u8,
    counter: u32,
) -> [u8; N] {
    let (rows, columns) = role.dimensions();
    let mut state = Shake256::default();
    absorb_frame(&mut state, MATRIX_DOMAIN_V1);
    absorb_frame(&mut state, &seed.parameter_digest);
    absorb_frame(&mut state, &seed.public_parameter_seed);
    absorb_frame(&mut state, &[role.tag()]);
    absorb_frame(&mut state, &rows.to_be_bytes());
    absorb_frame(&mut state, &columns.to_be_bytes());
    absorb_frame(&mut state, &row.to_be_bytes());
    absorb_frame(&mut state, &column.to_be_bytes());
    absorb_frame(&mut state, &[coefficient]);
    absorb_frame(&mut state, &counter.to_be_bytes());
    let mut reader = state.finalize_xof();
    let mut output = [0_u8; N];
    reader.read(&mut output);
    output
}

/// Complete public binding for the presentation Fiat--Shamir challenge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresentationChallengeBindingV1 {
    /// Governed parameter-manifest digest.
    pub parameter_digest: [u8; 32],
    /// Canonical statement digest.
    pub statement_digest: [u8; 32],
    /// Trusted current issuer-policy-record digest.
    pub issuer_policy_record_digest: [u8; 32],
    /// Verifier-recomputed transaction-intent digest.
    pub transaction_intent_digest: [u8; 32],
}

impl PresentationChallengeBindingV1 {
    fn validate(self) -> Result<(), TranscriptErrorV1> {
        for (field, digest) in [
            ("parameter_digest", self.parameter_digest),
            ("statement_digest", self.statement_digest),
            (
                "issuer_policy_record_digest",
                self.issuer_policy_record_digest,
            ),
            ("transaction_intent_digest", self.transaction_intent_digest),
        ] {
            if digest == [0; 32] {
                return Err(TranscriptErrorV1::ZeroDigest { field });
            }
        }
        Ok(())
    }
}

/// Complete public prefix shared by every presentation transcript stage.
///
/// The canonical statement digest binds chain id, action index, transaction
/// intent, compiled profile, verifier, schema, manifest, issuer identity,
/// policy identity, policy epoch, issuer parameters, the committed policy
/// digest, and disclosures.  The extra relation digest commits the exact
/// verifier-compiled matrix and public offset, while `matrix_seed` commits the
/// transparent CRS seed used to expand all matrices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresentationTranscriptV1 {
    binding: PresentationChallengeBindingV1,
    matrix_seed: MatrixSeedV1,
    relation_digest: [u8; 32],
}

impl PresentationTranscriptV1 {
    /// Construct one fully bound presentation transcript.
    ///
    /// # Errors
    ///
    /// Rejects any zero digest or a matrix seed for another parameter
    /// manifest.
    pub fn new(
        binding: PresentationChallengeBindingV1,
        matrix_seed: MatrixSeedV1,
        relation_digest: [u8; 32],
    ) -> Result<Self, TranscriptErrorV1> {
        binding.validate()?;
        if relation_digest == [0; 32] {
            return Err(TranscriptErrorV1::ZeroDigest {
                field: "relation_digest",
            });
        }
        if binding.parameter_digest != *matrix_seed.parameter_digest() {
            return Err(TranscriptErrorV1::MatrixParameterBindingMismatch);
        }
        Ok(Self {
            binding,
            matrix_seed,
            relation_digest,
        })
    }

    /// Return the final challenge binding.
    #[must_use]
    pub const fn binding(&self) -> PresentationChallengeBindingV1 {
        self.binding
    }

    /// Return the transparent matrix seed.
    #[must_use]
    pub const fn matrix_seed(&self) -> MatrixSeedV1 {
        self.matrix_seed
    }

    /// Return the exact compiled-relation digest.
    #[must_use]
    pub const fn relation_digest(&self) -> [u8; 32] {
        self.relation_digest
    }

    /// Derive arbitrary public stage bytes with strict field framing.
    ///
    /// # Errors
    ///
    /// Rejects an empty stage tag or a field whose length cannot be encoded.
    pub fn derive_bytes(
        &self,
        stage: &[u8],
        components: &[&[u8]],
        output: &mut [u8],
    ) -> Result<(), TranscriptErrorV1> {
        if stage.is_empty() {
            return Err(TranscriptErrorV1::EmptyStageTag);
        }
        let mut state = Shake256::default();
        absorb_frame_checked(&mut state, PRESENTATION_STAGE_DOMAIN_V1)?;
        absorb_frame_checked(&mut state, stage)?;
        absorb_frame_checked(&mut state, &self.binding.parameter_digest)?;
        absorb_frame_checked(&mut state, &self.binding.statement_digest)?;
        absorb_frame_checked(
            &mut state,
            &self.binding.issuer_policy_record_digest,
        )?;
        absorb_frame_checked(&mut state, &self.binding.transaction_intent_digest)?;
        absorb_frame_checked(&mut state, self.matrix_seed.parameter_digest())?;
        absorb_frame_checked(&mut state, self.matrix_seed.public_parameter_seed())?;
        absorb_frame_checked(&mut state, &self.relation_digest)?;
        let component_count =
            u32::try_from(components.len()).map_err(|_| TranscriptErrorV1::FieldTooLarge)?;
        absorb_frame_checked(&mut state, &component_count.to_be_bytes())?;
        for component in components {
            absorb_frame_checked(&mut state, component)?;
        }
        let mut reader = state.finalize_xof();
        reader.read(output);
        Ok(())
    }

    /// Derive one ternary projection row in `{-1,0,1}^columns`.
    ///
    /// # Errors
    ///
    /// Rejects an empty row, an oversized coordinate, or transcript framing
    /// failure.
    pub fn derive_ternary_row(
        &self,
        stage: &[u8],
        components: &[&[u8]],
        row: u16,
        columns: usize,
    ) -> Result<Vec<i8>, TranscriptErrorV1> {
        if columns == 0 {
            return Err(TranscriptErrorV1::EmptyProjectionRow);
        }
        let columns_u32 =
            u32::try_from(columns).map_err(|_| TranscriptErrorV1::FieldTooLarge)?;
        let mut coordinate = [0_u8; 6];
        coordinate[..2].copy_from_slice(&row.to_be_bytes());
        coordinate[2..].copy_from_slice(&columns_u32.to_be_bytes());
        let mut state = Shake256::default();
        absorb_stage_prefix(self, &mut state, stage, components)?;
        absorb_frame_checked(&mut state, &coordinate)?;
        let mut reader = state.finalize_xof();
        let mut output = Vec::with_capacity(columns);
        while output.len() < columns {
            let mut byte = [0_u8; 1];
            reader.read(&mut byte);
            if byte[0] < 255 {
                output.push(i8::try_from(byte[0] % 3).expect("ternary residue fits i8") - 1);
            }
        }
        Ok(output)
    }

    /// Derive uniform proof-ring polynomials.
    ///
    /// # Errors
    ///
    /// Returns transcript framing or bounded uniform-rejection failure.
    pub fn derive_uniform_polynomials(
        &self,
        stage: &[u8],
        components: &[&[u8]],
        count: usize,
    ) -> Result<Vec<ProofPolynomialV1>, TranscriptErrorV1> {
        let count_u32 = u32::try_from(count).map_err(|_| TranscriptErrorV1::FieldTooLarge)?;
        let mut state = Shake256::default();
        absorb_stage_prefix(self, &mut state, stage, components)?;
        absorb_frame_checked(&mut state, &count_u32.to_be_bytes())?;
        let mut reader = state.finalize_xof();
        let mut output = Vec::with_capacity(count);
        for _ in 0..count {
            let mut coefficients = [0_u64; APPLICATION_RING_DEGREE_V1];
            for coefficient in &mut coefficients {
                let mut accepted = None;
                for _ in 0..MAX_UNIFORM_REJECTION_ATTEMPTS_V1 {
                    let mut bytes = [0_u8; 7];
                    reader.read(&mut bytes);
                    let mut wide = [0_u8; 8];
                    wide[1..].copy_from_slice(&bytes);
                    let candidate = u64::from_be_bytes(wide);
                    if candidate < PROOF_ACCEPTANCE_LIMIT_V1 {
                        accepted = Some(candidate % PROOF_MODULUS_V1);
                        break;
                    }
                }
                *coefficient =
                    accepted.ok_or(TranscriptErrorV1::UniformRejectionExhausted)?;
            }
            output.push(
                ProofPolynomialV1::new(coefficients)
                    .map_err(|_| TranscriptErrorV1::InternalInvariant)?,
            );
        }
        Ok(output)
    }

    /// Derive the final auto-stable challenge from this complete prefix.
    ///
    /// # Errors
    ///
    /// Rejects empty commitments or framing failure.
    pub fn derive_final_challenge(
        &self,
        pre_challenge_commitments: &[u8],
    ) -> Result<ProofPolynomialV1, TranscriptErrorV1> {
        if pre_challenge_commitments.is_empty() {
            return Err(TranscriptErrorV1::EmptyPreChallengeCommitments);
        }
        let mut prefixed = Vec::with_capacity(32 * 3 + pre_challenge_commitments.len());
        prefixed.extend_from_slice(self.matrix_seed.parameter_digest());
        prefixed.extend_from_slice(self.matrix_seed.public_parameter_seed());
        prefixed.extend_from_slice(&self.relation_digest);
        prefixed.extend_from_slice(pre_challenge_commitments);
        derive_presentation_challenge_v1(self.binding, &prefixed)
    }
}

fn absorb_stage_prefix(
    transcript: &PresentationTranscriptV1,
    state: &mut Shake256,
    stage: &[u8],
    components: &[&[u8]],
) -> Result<(), TranscriptErrorV1> {
    if stage.is_empty() {
        return Err(TranscriptErrorV1::EmptyStageTag);
    }
    absorb_frame_checked(state, PRESENTATION_STAGE_DOMAIN_V1)?;
    absorb_frame_checked(state, stage)?;
    absorb_frame_checked(state, &transcript.binding.parameter_digest)?;
    absorb_frame_checked(state, &transcript.binding.statement_digest)?;
    absorb_frame_checked(state, &transcript.binding.issuer_policy_record_digest)?;
    absorb_frame_checked(state, &transcript.binding.transaction_intent_digest)?;
    absorb_frame_checked(state, transcript.matrix_seed.parameter_digest())?;
    absorb_frame_checked(state, transcript.matrix_seed.public_parameter_seed())?;
    absorb_frame_checked(state, &transcript.relation_digest)?;
    let component_count =
        u32::try_from(components.len()).map_err(|_| TranscriptErrorV1::FieldTooLarge)?;
    absorb_frame_checked(state, &component_count.to_be_bytes())?;
    for component in components {
        absorb_frame_checked(state, component)?;
    }
    Ok(())
}

/// Derive the unique auto-stable 64-coefficient challenge over the proof
/// modulus from the exact public binding and pre-challenge commitment wire.
///
/// The first 32 coefficients are uniform in `[-8, 8]`, coefficient 32 is
/// zero, and the remaining 31 coefficients are the required antisymmetric
/// image. The commitment wire is framed as one field, so component
/// concatenation cannot collide.
///
/// # Errors
///
/// Rejects a zero binding digest, empty commitment wire, or a commitment wire
/// whose length cannot be represented in the canonical frame.
pub fn derive_presentation_challenge_v1(
    binding: PresentationChallengeBindingV1,
    pre_challenge_commitments: &[u8],
) -> Result<ProofPolynomialV1, TranscriptErrorV1> {
    binding.validate()?;
    if pre_challenge_commitments.is_empty() {
        return Err(TranscriptErrorV1::EmptyPreChallengeCommitments);
    }
    let mut state = Shake256::default();
    absorb_frame_checked(&mut state, PRESENTATION_CHALLENGE_DOMAIN_V1)?;
    absorb_frame_checked(&mut state, &binding.parameter_digest)?;
    absorb_frame_checked(&mut state, &binding.statement_digest)?;
    absorb_frame_checked(&mut state, &binding.issuer_policy_record_digest)?;
    absorb_frame_checked(&mut state, &binding.transaction_intent_digest)?;
    absorb_frame_checked(&mut state, pre_challenge_commitments)?;
    let mut reader = state.finalize_xof();

    let mut challenge = [0_u64; APPLICATION_RING_DEGREE_V1];
    for coefficient in &mut challenge[..32] {
        let candidate = loop {
            let mut byte = [0_u8; 1];
            reader.read(&mut byte);
            if byte[0] < 255 {
                break i16::from(byte[0] % 17) - 8;
            }
        };
        *coefficient = if candidate < 0 {
            PROOF_MODULUS_V1 - u64::try_from(-candidate).expect("challenge magnitude fits u64")
        } else {
            u64::try_from(candidate).expect("challenge magnitude fits u64")
        };
    }
    challenge[32] = 0;
    for index in 33..APPLICATION_RING_DEGREE_V1 {
        let source = APPLICATION_RING_DEGREE_V1 - index;
        challenge[index] = if challenge[source] == 0 {
            0
        } else {
            PROOF_MODULUS_V1 - challenge[source]
        };
    }
    ProofPolynomialV1::new(challenge).map_err(|_| TranscriptErrorV1::InternalInvariant)
}

fn matrix_index(rows: u16, columns: u16, row: u16, column: u16) -> Option<usize> {
    if row >= rows || column >= columns {
        return None;
    }
    Some(usize::from(row) * usize::from(columns) + usize::from(column))
}

fn absorb_frame(state: &mut Shake256, bytes: &[u8]) {
    let length = u32::try_from(bytes.len()).expect("fixed transcript field fits u32");
    state.update(&length.to_be_bytes());
    state.update(bytes);
}

fn absorb_frame_checked(state: &mut Shake256, bytes: &[u8]) -> Result<(), TranscriptErrorV1> {
    let length = u32::try_from(bytes.len()).map_err(|_| TranscriptErrorV1::FieldTooLarge)?;
    state.update(&length.to_be_bytes());
    state.update(bytes);
    Ok(())
}

/// Transparent-expansion or Fiat--Shamir transcript failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum TranscriptErrorV1 {
    /// A mandatory governed digest was zero.
    #[error("Bootle/Lantern transcript digest `{field}` must be non-zero")]
    ZeroDigest {
        /// Stable field name.
        field: &'static str,
    },
    /// A closed matrix role was used with the other modulus.
    #[error("Bootle/Lantern matrix role {role:?} was requested from the wrong ring")]
    WrongMatrixRing {
        /// Rejected role.
        role: MatrixRoleV1,
    },
    /// A coordinate was outside the role's fixed dimensions.
    #[error(
        "Bootle/Lantern matrix role {role:?} coordinate ({row},{column}) is outside {rows}x{columns}"
    )]
    MatrixCoordinateOutOfRange {
        /// Matrix role.
        role: MatrixRoleV1,
        /// Rejected row.
        row: u16,
        /// Rejected column.
        column: u16,
        /// Exact rows.
        rows: u16,
        /// Exact columns.
        columns: u16,
    },
    /// Uniform coefficient rejection sampling exceeded its fixed work bound.
    #[error("Bootle/Lantern uniform matrix sampling exhausted its fixed work bound")]
    UniformRejectionExhausted,
    /// A transcript field exceeded the fixed 32-bit frame length.
    #[error("Bootle/Lantern transcript field is too large")]
    FieldTooLarge,
    /// No pre-challenge commitment bytes were supplied.
    #[error("Bootle/Lantern pre-challenge commitment wire must not be empty")]
    EmptyPreChallengeCommitments,
    /// A presentation stage tag was empty.
    #[error("Bootle/Lantern presentation transcript stage tag must not be empty")]
    EmptyStageTag,
    /// A projection row requested zero columns.
    #[error("Bootle/Lantern ternary projection row must not be empty")]
    EmptyProjectionRow,
    /// The matrix seed and challenge binding selected different parameters.
    #[error("Bootle/Lantern matrix seed does not match the presentation parameter binding")]
    MatrixParameterBindingMismatch,
    /// An internal canonicality invariant was violated.
    #[error("Bootle/Lantern internal transcript invariant failed")]
    InternalInvariant,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed() -> MatrixSeedV1 {
        MatrixSeedV1::new([0x31; 32], [0x72; 32]).expect("nonzero seed")
    }

    fn binding() -> PresentationChallengeBindingV1 {
        PresentationChallengeBindingV1 {
            parameter_digest: [0x11; 32],
            statement_digest: [0x22; 32],
            issuer_policy_record_digest: [0x33; 32],
            transaction_intent_digest: [0x44; 32],
        }
    }

    fn presentation_transcript() -> PresentationTranscriptV1 {
        PresentationTranscriptV1::new(binding(), seed(), [0x95; 32])
            .expect("fully bound transcript")
    }

    #[test]
    fn seed_rejects_each_zero_digest() {
        assert_eq!(
            MatrixSeedV1::new([0; 32], [1; 32]),
            Err(TranscriptErrorV1::ZeroDigest {
                field: "parameter_digest"
            })
        );
        assert_eq!(
            MatrixSeedV1::new([1; 32], [0; 32]),
            Err(TranscriptErrorV1::ZeroDigest { field: "ppseed" })
        );
    }

    #[test]
    fn roles_have_the_exact_closed_tags_and_dimensions() {
        assert_eq!(MatrixRoleV1::ApplicationRandomness.dimensions(), (8, 16));
        assert_eq!(MatrixRoleV1::ApplicationAttributes.dimensions(), (8, 8));
        assert_eq!(MatrixRoleV1::ApplicationTag.dimensions(), (8, 8));
        assert_eq!(MatrixRoleV1::InternalA1.dimensions(), (20, 50));
        assert_eq!(MatrixRoleV1::InternalA2Prime.dimensions(), (20, 44));
        assert_eq!(MatrixRoleV1::InternalBPrime.dimensions(), (12, 44));
        assert_eq!(
            [
                MatrixRoleV1::ApplicationRandomness.tag(),
                MatrixRoleV1::ApplicationAttributes.tag(),
                MatrixRoleV1::ApplicationTag.tag(),
                MatrixRoleV1::InternalA1.tag(),
                MatrixRoleV1::InternalA2Prime.tag(),
                MatrixRoleV1::InternalBPrime.tag(),
            ],
            [0x01, 0x02, 0x03, 0x11, 0x12, 0x13]
        );
    }

    #[test]
    fn wrong_ring_roles_and_out_of_range_coordinates_fail_closed() {
        assert_eq!(
            derive_application_polynomial_v1(seed(), MatrixRoleV1::InternalA1, 0, 0),
            Err(TranscriptErrorV1::WrongMatrixRing {
                role: MatrixRoleV1::InternalA1
            })
        );
        assert_eq!(
            derive_proof_polynomial_v1(seed(), MatrixRoleV1::ApplicationRandomness, 0, 0),
            Err(TranscriptErrorV1::WrongMatrixRing {
                role: MatrixRoleV1::ApplicationRandomness
            })
        );
        assert!(matches!(
            derive_application_polynomial_v1(seed(), MatrixRoleV1::ApplicationRandomness, 8, 0),
            Err(TranscriptErrorV1::MatrixCoordinateOutOfRange { .. })
        ));
        assert!(matches!(
            derive_proof_polynomial_v1(seed(), MatrixRoleV1::InternalA1, 0, 50),
            Err(TranscriptErrorV1::MatrixCoordinateOutOfRange { .. })
        ));
    }

    #[test]
    fn matrix_expansion_has_exact_shapes_and_canonical_residues() {
        for role in [
            MatrixRoleV1::ApplicationRandomness,
            MatrixRoleV1::ApplicationAttributes,
            MatrixRoleV1::ApplicationTag,
        ] {
            let matrix = expand_application_matrix_v1(seed(), role).expect("expansion");
            let (rows, columns) = role.dimensions();
            assert_eq!(matrix.rows(), rows);
            assert_eq!(matrix.columns(), columns);
            assert_eq!(
                matrix.entries().len(),
                usize::from(rows) * usize::from(columns)
            );
            assert!(matrix.get(rows, 0).is_none());
            assert!(matrix.get(0, columns).is_none());
            assert!(matrix.entries().iter().all(|polynomial| {
                polynomial
                    .coefficients()
                    .iter()
                    .all(|coefficient| *coefficient < APPLICATION_MODULUS_V1)
            }));
        }
        for role in [
            MatrixRoleV1::InternalA1,
            MatrixRoleV1::InternalA2Prime,
            MatrixRoleV1::InternalBPrime,
        ] {
            let matrix = expand_proof_matrix_v1(seed(), role).expect("expansion");
            let (rows, columns) = role.dimensions();
            assert_eq!(matrix.rows(), rows);
            assert_eq!(matrix.columns(), columns);
            assert_eq!(
                matrix.entries().len(),
                usize::from(rows) * usize::from(columns)
            );
            assert!(matrix.get(rows, 0).is_none());
            assert!(matrix.get(0, columns).is_none());
            assert!(matrix.entries().iter().all(|polynomial| {
                polynomial
                    .coefficients()
                    .iter()
                    .all(|coefficient| *coefficient < PROOF_MODULUS_V1)
            }));
        }
    }

    #[test]
    fn every_seed_role_and_coordinate_dimension_is_domain_separated() {
        let base =
            derive_application_polynomial_v1(seed(), MatrixRoleV1::ApplicationRandomness, 0, 0)
                .expect("expansion");
        assert_ne!(
            base,
            derive_application_polynomial_v1(seed(), MatrixRoleV1::ApplicationRandomness, 0, 1)
                .expect("expansion")
        );
        assert_ne!(
            base,
            derive_application_polynomial_v1(seed(), MatrixRoleV1::ApplicationRandomness, 1, 0)
                .expect("expansion")
        );
        assert_ne!(
            base,
            derive_application_polynomial_v1(seed(), MatrixRoleV1::ApplicationAttributes, 0, 0)
                .expect("expansion")
        );
        assert_ne!(
            base,
            derive_application_polynomial_v1(
                MatrixSeedV1::new([0x32; 32], [0x72; 32]).expect("seed"),
                MatrixRoleV1::ApplicationRandomness,
                0,
                0
            )
            .expect("expansion")
        );
        assert_ne!(
            base,
            derive_application_polynomial_v1(
                MatrixSeedV1::new([0x31; 32], [0x73; 32]).expect("seed"),
                MatrixRoleV1::ApplicationRandomness,
                0,
                0
            )
            .expect("expansion")
        );
    }

    #[test]
    fn challenge_is_autostable_canonical_deterministic_and_fully_bound() {
        let challenge = derive_presentation_challenge_v1(binding(), b"canonical commitments")
            .expect("challenge");
        let coefficients = challenge.coefficients();
        for coefficient in &coefficients[..32] {
            let centered = if *coefficient <= PROOF_MODULUS_V1 / 2 {
                i64::try_from(*coefficient).expect("fits")
            } else {
                i64::try_from(*coefficient).expect("fits")
                    - i64::try_from(PROOF_MODULUS_V1).expect("fits")
            };
            assert!((-8..=8).contains(&centered));
        }
        assert_eq!(coefficients[32], 0);
        for index in 33..APPLICATION_RING_DEGREE_V1 {
            let source = APPLICATION_RING_DEGREE_V1 - index;
            let expected = if coefficients[source] == 0 {
                0
            } else {
                PROOF_MODULUS_V1 - coefficients[source]
            };
            assert_eq!(coefficients[index], expected);
        }
        assert_eq!(
            challenge,
            derive_presentation_challenge_v1(binding(), b"canonical commitments")
                .expect("challenge")
        );

        let mut mutations = Vec::new();
        let base = binding();
        let mut changed = base;
        changed.parameter_digest[0] ^= 1;
        mutations.push(changed);
        changed = base;
        changed.statement_digest[0] ^= 1;
        mutations.push(changed);
        changed = base;
        changed.issuer_policy_record_digest[0] ^= 1;
        mutations.push(changed);
        changed = base;
        changed.transaction_intent_digest[0] ^= 1;
        mutations.push(changed);
        for changed in mutations {
            assert_ne!(
                challenge,
                derive_presentation_challenge_v1(changed, b"canonical commitments")
                    .expect("challenge")
            );
        }
        assert_ne!(
            challenge,
            derive_presentation_challenge_v1(binding(), b"canonical commitmentt")
                .expect("challenge")
        );
    }

    #[test]
    fn challenge_rejects_every_zero_binding_and_empty_commitments() {
        let base = binding();
        for field in 0..4 {
            let mut changed = base;
            match field {
                0 => changed.parameter_digest = [0; 32],
                1 => changed.statement_digest = [0; 32],
                2 => changed.issuer_policy_record_digest = [0; 32],
                3 => changed.transaction_intent_digest = [0; 32],
                _ => unreachable!(),
            }
            assert!(matches!(
                derive_presentation_challenge_v1(changed, b"commitments"),
                Err(TranscriptErrorV1::ZeroDigest { .. })
            ));
        }
        assert_eq!(
            derive_presentation_challenge_v1(base, b""),
            Err(TranscriptErrorV1::EmptyPreChallengeCommitments)
        );
    }

    #[test]
    fn matrix_and_challenge_known_answer_prefixes_are_frozen() {
        let application =
            derive_application_polynomial_v1(seed(), MatrixRoleV1::ApplicationRandomness, 0, 0)
                .expect("expansion");
        let proof =
            derive_proof_polynomial_v1(seed(), MatrixRoleV1::InternalA1, 0, 0).expect("expansion");
        let challenge = derive_presentation_challenge_v1(binding(), b"canonical commitments")
            .expect("challenge");

        assert_eq!(
            &application.coefficients()[..4],
            &[3_766, 7_759, 1_604, 8_810]
        );
        assert_eq!(
            &proof.coefficients()[..4],
            &[
                81_301_060_368_069,
                36_322_145_752_893,
                359_779_698_830_871,
                64_982_682_605_156,
            ]
        );
        assert_eq!(
            &challenge.coefficients()[..8],
            &[
                1_125_899_906_843_217,
                3,
                4,
                4,
                8,
                1_125_899_906_843_214,
                1_125_899_906_843_220,
                1_125_899_906_843_220,
            ]
        );
    }

    #[test]
    fn staged_transcript_is_framed_deterministic_and_fully_bound() {
        let transcript = presentation_transcript();
        let mut first = [0_u8; 64];
        let mut second = [0_u8; 64];
        transcript
            .derive_bytes(b"stage-a", &[b"ab", b"c"], &mut first)
            .expect("stage");
        transcript
            .derive_bytes(b"stage-a", &[b"ab", b"c"], &mut second)
            .expect("stage");
        assert_eq!(first, second);
        transcript
            .derive_bytes(b"stage-a", &[b"a", b"bc"], &mut second)
            .expect("stage");
        assert_ne!(first, second);
        transcript
            .derive_bytes(b"stage-b", &[b"ab", b"c"], &mut second)
            .expect("stage");
        assert_ne!(first, second);

        let mut changed_binding = binding();
        changed_binding.statement_digest[0] ^= 1;
        let changed =
            PresentationTranscriptV1::new(changed_binding, seed(), [0x95; 32]).expect("binding");
        changed
            .derive_bytes(b"stage-a", &[b"ab", b"c"], &mut second)
            .expect("stage");
        assert_ne!(first, second);

        let changed = PresentationTranscriptV1::new(binding(), seed(), [0x94; 32])
            .expect("relation binding");
        changed
            .derive_bytes(b"stage-a", &[b"ab", b"c"], &mut second)
            .expect("stage");
        assert_ne!(first, second);
    }

    #[test]
    fn staged_uniform_and_ternary_expansion_is_canonical_and_random_access() {
        let transcript = presentation_transcript();
        let first = transcript
            .derive_ternary_row(b"projection", &[b"commitment"], 17, 1_024)
            .expect("row");
        let second = transcript
            .derive_ternary_row(b"projection", &[b"commitment"], 17, 1_024)
            .expect("row");
        assert_eq!(first, second);
        assert!(first.iter().all(|value| (-1..=1).contains(value)));
        assert_ne!(
            first,
            transcript
                .derive_ternary_row(b"projection", &[b"commitment"], 18, 1_024)
                .expect("row")
        );

        let polynomials = transcript
            .derive_uniform_polynomials(b"weights", &[b"commitment"], 9)
            .expect("uniform polynomials");
        assert_eq!(polynomials.len(), 9);
        assert!(polynomials.iter().all(|polynomial| {
            polynomial
                .coefficients()
                .iter()
                .all(|coefficient| *coefficient < PROOF_MODULUS_V1)
        }));
        assert_eq!(
            polynomials,
            transcript
                .derive_uniform_polynomials(b"weights", &[b"commitment"], 9)
                .expect("uniform polynomials")
        );
    }

    #[test]
    fn staged_transcript_rejects_mismatched_or_empty_inputs() {
        assert_eq!(
            PresentationTranscriptV1::new(
                binding(),
                MatrixSeedV1::new([0x12; 32], [0x72; 32]).expect("seed"),
                [0x95; 32]
            ),
            Err(TranscriptErrorV1::MatrixParameterBindingMismatch)
        );
        assert_eq!(
            PresentationTranscriptV1::new(binding(), seed(), [0; 32]),
            Err(TranscriptErrorV1::ZeroDigest {
                field: "relation_digest"
            })
        );
        assert_eq!(
            presentation_transcript().derive_bytes(b"", &[], &mut [0_u8; 1]),
            Err(TranscriptErrorV1::EmptyStageTag)
        );
        assert_eq!(
            presentation_transcript().derive_ternary_row(b"r", &[], 0, 0),
            Err(TranscriptErrorV1::EmptyProjectionRow)
        );
        assert_eq!(
            presentation_transcript().derive_final_challenge(b""),
            Err(TranscriptErrorV1::EmptyPreChallengeCommitments)
        );
    }
}
