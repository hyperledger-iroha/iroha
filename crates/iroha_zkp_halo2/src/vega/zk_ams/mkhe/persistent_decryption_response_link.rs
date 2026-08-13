//! Static direct response-link prerequisite for persistent decryption. It freezes a prospective proof of
//! ```text
//! C^s_j = <s_j, G> + r_j H,       C^Y_j = <Y_j, G> + u_j H,
//! Z_s = Y + D * s  in Z[X]/(X^N + 1),
//! ```
//! Here `N=131072`, chunks have 16,384 coefficients, and `D` has 20 signed monomials. After `Z_s` is fixed, `beta` reduces the equations:
//! ```text
//! sum_k beta^k Y_k + sum_j adj_D(beta)_j s_j - sum_k beta^k Z_s[k] = 0.
//! ```
//! The BP has one constraint, 16 vector commitments, dimension 16,384, no scalar commitments, a 2,437-byte core, and a 2,708-byte tail.
//! Earliest hook: the CPK prover owner retains all eight `s` openings/blindings. Production lacks `C^Y` before `D`; required order is `C^s,C^Y,A_pk,A_share -> D -> Z_s -> beta -> BP`; seals are uninhabited and every gate false. This does not close persistent-decryption audit bit 7.

#![allow(dead_code, reason = "production response-link seals are uninhabited")]
use crate::{
    generalized_bulletproof::{
        ArithmeticCircuitStatement, GeneralizedBulletproofErrorV1, LinComb, ProofSuite, Variable,
        VerifierTranscript,
    },
    vega::{
        VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar,
        bulletproof_t256::{ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1, ZkAmsT256BulletproofSuiteV1},
        sponge::{Keccak256, keccak256},
    },
};
use core::convert::Infallible;
const RESPONSE_LINK_VERSION_V1: u8 = 1;
const RESPONSE_LINK_WIRE_TAG_V1: [u8; 4] = *b"ZPRL";
const RESPONSE_LINK_WIRE_FLAGS_V1: u8 = 0;
const RESPONSE_LINK_PARTIES_V1: usize = 8;
const RESPONSE_LINK_CHUNKS_V1: usize = 8;
const RESPONSE_LINK_COMMITMENTS_V1: usize = 16;
const RESPONSE_LINK_CHUNK_COEFFICIENTS_V1: usize = 16_384;
const RESPONSE_LINK_RING_DEGREE_V1: usize = 131_072;
const RESPONSE_LINK_CHALLENGE_WEIGHT_V1: usize = 20;
const RESPONSE_LINK_SECRET_MASK_BOUND_V1: i64 = 335_544_320;
const RESPONSE_LINK_SECRET_RESPONSE_BOUND_V1: i64 = 335_544_300;
const RESPONSE_LINK_INTEGER_LIFT_BOUND_V1: u64 = 671_088_640;
const RESPONSE_LINK_CORE_POINTS_V1: usize = 69;
const RESPONSE_LINK_CORE_SCALARS_V1: usize = 5;
const RESPONSE_LINK_CORE_BYTES_V1: usize = 2_437;
const RESPONSE_LINK_HEADER_BYTES_V1: usize = 7;
const RESPONSE_LINK_MASK_COMMITMENT_BYTES_V1: usize = 8 * 33;
const RESPONSE_LINK_TAIL_BYTES_V1: usize = 2_708;
const EXISTING_PROOF_BYTES_V1: usize = 33_030_199;
const CONDITIONAL_PROOF_BYTES_V1: usize = 33_032_907;
const PROOF_WIRE_CAP_BYTES_V1: usize = 32 * 1_048_576;
const OBJECT_CAP_BYTES_V1: usize = 64 * 1_048_576;
const PROOF_WIRE_MARGIN_BYTES_V1: usize = 521_525;
const OBJECT_MARGIN_BYTES_V1: usize = 34_075_957;
const EIGHT_PARTY_TAIL_BYTES_V1: usize = 21_664;
const PROVER_PEAK_HEAP_BOUND_BYTES_V1: usize = 132_300_000;
const VERIFIER_PEAK_HEAP_BOUND_BYTES_V1: usize = 136_038_231;
const WORKER_HEAP_CAP_BYTES_V1: usize = 160 * 1_048_576;
const PROVER_HEAP_MARGIN_BYTES_V1: usize = 35_472_160;
const VERIFIER_HEAP_MARGIN_BYTES_V1: usize = 31_733_929;
const PROVER_MAIN_SCALAR_WORK_ESTIMATE_V1: u64 = 9_437_183;
const PROVER_GROUP_TERM_ESTIMATE_V1: u64 = 508_000;
const PIT_NUMERATOR_V1: u64 = 131_071;
const PIT_SOUNDNESS_BITS_FLOOR_V1: u16 = 238;
const TRANSCRIPT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.persistent-decryption.response-link.transcript\0";
const CHALLENGE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.persistent-decryption.response-link.challenge\0";
const EQUATION_V1: &[u8] = b"Zs=Y+D*s;D=sum_h epsilon_h X^h;w_j=sum_h epsilon_h(beta^(j+h) if j+h<N else -beta^(j+h-N));BP language is over F_pT;the signed-small two-fork residual is at most 671088640<pT, so its embedding is injective";
const CONDITIONAL_TWO_FORK_ASSUMPTIONS_V1: &[u8] = b"ROM forking after identical Cs,CY,A_pk,A_share;fixed CY and RNS first messages make Y cancel, so no CY-to-RNS-first-message cross-opening is needed;distinct sparse D;Pedersen representation binding;generalized-BP knowledge soundness and ZK;existing RNS two-fork extractor;bounded signed-small lift;cyclotomic-domain argument independently certified";
const SIGNED_SMALL_SEPARATION_V1: &[u8] = b"only signed-i64 secret_response enters this link;the approximately 1855-bit smudge_response is a distinct vector and never enters C^Y,Z_s,or the T256 projection";
const STATE_HOOK_WIRED_V1: bool = false;
const DIRECT_EQUALITY_VERIFIED_V1: bool = false;
const ATOMIC_REPLAY_WIRED_V1: bool = false;
const VERIFIED_RECEIPT_CONSUMED_V1: bool = false;
const PRODUCTION_RSS_QUALIFIED_V1: bool = false;
const PRODUCTION_KAT_QUALIFIED_V1: bool = false;
const ZERO_KNOWLEDGE_ACCEPTED_V1: bool = false;
const RELEASE_READY_V1: bool = false;
const _: () = {
    assert!(
        RESPONSE_LINK_RING_DEGREE_V1
            == RESPONSE_LINK_CHUNKS_V1 * RESPONSE_LINK_CHUNK_COEFFICIENTS_V1
    );
    assert!(RESPONSE_LINK_COMMITMENTS_V1 == 2 * RESPONSE_LINK_CHUNKS_V1);
    assert!(
        RESPONSE_LINK_CORE_BYTES_V1
            == RESPONSE_LINK_CORE_POINTS_V1 * 33 + RESPONSE_LINK_CORE_SCALARS_V1 * 32
            && RESPONSE_LINK_CORE_POINTS_V1 == 3 + 38 + 2 * 14
            && RESPONSE_LINK_CORE_SCALARS_V1 == 3 + 2
    );
    assert!(
        RESPONSE_LINK_TAIL_BYTES_V1
            == RESPONSE_LINK_HEADER_BYTES_V1
                + RESPONSE_LINK_MASK_COMMITMENT_BYTES_V1
                + RESPONSE_LINK_CORE_BYTES_V1
    );
    assert!(CONDITIONAL_PROOF_BYTES_V1 == EXISTING_PROOF_BYTES_V1 + RESPONSE_LINK_TAIL_BYTES_V1);
    assert!(PROOF_WIRE_MARGIN_BYTES_V1 == PROOF_WIRE_CAP_BYTES_V1 - CONDITIONAL_PROOF_BYTES_V1);
    assert!(OBJECT_MARGIN_BYTES_V1 == OBJECT_CAP_BYTES_V1 - CONDITIONAL_PROOF_BYTES_V1);
    assert!(EIGHT_PARTY_TAIL_BYTES_V1 == RESPONSE_LINK_PARTIES_V1 * RESPONSE_LINK_TAIL_BYTES_V1);
    assert!(
        RESPONSE_LINK_INTEGER_LIFT_BOUND_V1
            == 2 * RESPONSE_LINK_SECRET_RESPONSE_BOUND_V1 as u64
                + 2 * RESPONSE_LINK_CHALLENGE_WEIGHT_V1 as u64
    );
    assert!(!STATE_HOOK_WIRED_V1 && !DIRECT_EQUALITY_VERIFIED_V1 && !ATOMIC_REPLAY_WIRED_V1);
    assert!(
        !VERIFIED_RECEIPT_CONSUMED_V1
            && !PRODUCTION_RSS_QUALIFIED_V1
            && !PRODUCTION_KAT_QUALIFIED_V1
    );
    assert!(!ZERO_KNOWLEDGE_ACCEPTED_V1 && !RELEASE_READY_V1);
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResponseLinkErrorV1 {
    Shape,
    Context,
    ResponseBound,
    PointEncoding,
    ScalarEncoding,
    ProofEncoding,
    StatementMismatch,
    Backend(GeneralizedBulletproofErrorV1),
}
impl From<GeneralizedBulletproofErrorV1> for ResponseLinkErrorV1 {
    fn from(error: GeneralizedBulletproofErrorV1) -> Self {
        Self::Backend(error)
    }
}
#[derive(Clone, Copy)]
struct ResponseLinkAxesV1 {
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_context_digest: [u8; 32],
    cpk_transcript_digest: [u8; 32],
    decryption_statement_digest: [u8; 32],
    public_key_first_message_digest: [u8; 32],
    share_first_message_digest: [u8; 32],
    epoch: u64,
    party_index: u8,
}
impl ResponseLinkAxesV1 {
    fn validate_v1(self) -> Result<(), ResponseLinkErrorV1> {
        if [
            self.profile_digest,
            self.roster_digest,
            self.key_context_digest,
            self.cpk_transcript_digest,
            self.decryption_statement_digest,
            self.public_key_first_message_digest,
            self.share_first_message_digest,
        ]
        .contains(&[0; 32])
            || self.epoch == 0
            || usize::from(self.party_index) >= RESPONSE_LINK_PARTIES_V1
        {
            return Err(ResponseLinkErrorV1::Context);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SparseChallengeTermV1 {
    shift: u32,
    sign: i8,
}
#[derive(Clone, Copy)]
struct SparseChallengeV1 {
    seed: [u8; 32],
    terms: [SparseChallengeTermV1; RESPONSE_LINK_CHALLENGE_WEIGHT_V1],
}
impl SparseChallengeV1 {
    fn validate_v1(self) -> Result<(), ResponseLinkErrorV1> {
        if self.seed == [0; 32] {
            return Err(ResponseLinkErrorV1::Context);
        }
        let mut previous = None;
        for term in self.terms {
            let shift = usize::try_from(term.shift).map_err(|_| ResponseLinkErrorV1::Shape)?;
            if shift >= RESPONSE_LINK_RING_DEGREE_V1
                || ![-1, 1].contains(&term.sign)
                || previous.is_some_and(|prior| prior >= term.shift)
            {
                return Err(ResponseLinkErrorV1::Shape);
            }
            previous = Some(term.shift);
        }
        Ok(())
    }
}
enum ResponseLinkSourceSealV1 {
    Production {
        verified_cpk_owner: Infallible,
        retained_openings: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}
enum ResponseLinkUseSealV1 {
    Production {
        mask_committed_before_challenge: Infallible,
        exact_response_owner: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}
enum ResponseLinkIntegrationSealV1 {
    Production {
        decryption_state_hook: Infallible,
        atomic_semantic_replay: Infallible,
        receipt_consumer: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}
struct PersistentDecryptionResponseLinkSourceV1 {
    axes: ResponseLinkAxesV1,
    cpk_secret_commitments: [Point; RESPONSE_LINK_CHUNKS_V1],
    seal: ResponseLinkSourceSealV1,
}
struct PersistentDecryptionResponseLinkMaskCommittedUseV1<'a> {
    source: &'a PersistentDecryptionResponseLinkSourceV1,
    mask_commitments: [Point; RESPONSE_LINK_CHUNKS_V1],
    seal: ResponseLinkUseSealV1,
}
struct PersistentDecryptionResponseLinkChallengeFixedUseV1<'a> {
    mask_stage: PersistentDecryptionResponseLinkMaskCommittedUseV1<'a>,
    challenge: SparseChallengeV1,
}
struct PersistentDecryptionResponseLinkResponseFixedUseV1<'a> {
    challenge_stage: PersistentDecryptionResponseLinkChallengeFixedUseV1<'a>,
    secret_response: &'a [i64],
}
struct PersistentDecryptionResponseLinkCommitmentViewV1<'a> {
    response: &'a PersistentDecryptionResponseLinkResponseFixedUseV1<'a>,
}
impl PersistentDecryptionResponseLinkResponseFixedUseV1<'_> {
    fn validate_v1(&self) -> Result<(), ResponseLinkErrorV1> {
        self.challenge_stage.mask_stage.source.axes.validate_v1()?;
        self.challenge_stage.challenge.validate_v1()?;
        if self.secret_response.len() != RESPONSE_LINK_RING_DEGREE_V1 {
            return Err(ResponseLinkErrorV1::Shape);
        }
        if self
            .secret_response
            .iter()
            .any(|value| value.unsigned_abs() > RESPONSE_LINK_SECRET_RESPONSE_BOUND_V1 as u64)
        {
            return Err(ResponseLinkErrorV1::ResponseBound);
        }
        for point in self
            .challenge_stage
            .mask_stage
            .source
            .cpk_secret_commitments
            .iter()
            .chain(&self.challenge_stage.mask_stage.mask_commitments)
        {
            if point.is_identity() {
                return Err(ResponseLinkErrorV1::PointEncoding);
            }
        }
        Ok(())
    }
    fn commitments_v1(&self) -> PersistentDecryptionResponseLinkCommitmentViewV1<'_> {
        PersistentDecryptionResponseLinkCommitmentViewV1 { response: self }
    }
}
impl PersistentDecryptionResponseLinkCommitmentViewV1<'_> {
    fn point_v1(&self, index: usize) -> Result<Point, ResponseLinkErrorV1> {
        if index < RESPONSE_LINK_CHUNKS_V1 {
            Ok(self
                .response
                .challenge_stage
                .mask_stage
                .source
                .cpk_secret_commitments[index])
        } else if index < RESPONSE_LINK_COMMITMENTS_V1 {
            Ok(self.response.challenge_stage.mask_stage.mask_commitments
                [index - RESPONSE_LINK_CHUNKS_V1])
        } else {
            Err(ResponseLinkErrorV1::Shape)
        }
    }
}
fn scalar_from_signed_v1(value: i64) -> Scalar {
    let magnitude = Scalar::from_u64(value.unsigned_abs());
    if value < 0 { -magnitude } else { magnitude }
}
fn powers_v1(beta: Scalar, count: usize) -> Vec<Scalar> {
    let mut powers = Vec::with_capacity(count);
    let mut power = Scalar::one();
    for _ in 0..count {
        powers.push(power);
        power *= beta;
    }
    powers
}
fn adjoint_weight_v1(
    powers: &[Scalar],
    terms: &[SparseChallengeTermV1],
    source_index: usize,
) -> Result<Scalar, ResponseLinkErrorV1> {
    let degree = powers.len();
    if degree == 0 || source_index >= degree || !degree.is_power_of_two() {
        return Err(ResponseLinkErrorV1::Shape);
    }
    let mut weight = Scalar::zero();
    for term in terms {
        let shift = usize::try_from(term.shift).map_err(|_| ResponseLinkErrorV1::Shape)?;
        if shift >= degree || ![-1, 1].contains(&term.sign) {
            return Err(ResponseLinkErrorV1::Shape);
        }
        let exponent = source_index + shift;
        let mut contribution = powers[exponent % degree];
        if exponent >= degree {
            contribution = -contribution;
        }
        if term.sign < 0 {
            contribution = -contribution;
        }
        weight += contribution;
    }
    Ok(weight)
}
fn response_link_constraint_v1(
    beta: Scalar,
    challenge: SparseChallengeV1,
    secret_response: &[i64],
) -> Result<LinComb<Scalar>, ResponseLinkErrorV1> {
    challenge.validate_v1()?;
    if beta.is_zero() || secret_response.len() != RESPONSE_LINK_RING_DEGREE_V1 {
        return Err(ResponseLinkErrorV1::Shape);
    }
    let powers = powers_v1(beta, RESPONSE_LINK_RING_DEGREE_V1);
    let mut constraint = LinComb::empty();
    for global in 0..RESPONSE_LINK_RING_DEGREE_V1 {
        let chunk = global / RESPONSE_LINK_CHUNK_COEFFICIENTS_V1;
        let coordinate = global % RESPONSE_LINK_CHUNK_COEFFICIENTS_V1;
        let secret_weight = adjoint_weight_v1(&powers, &challenge.terms, global)?;
        constraint = constraint
            .term(
                secret_weight,
                Variable::CG {
                    commitment: chunk,
                    index: coordinate,
                },
            )
            .term(
                powers[global],
                Variable::CG {
                    commitment: RESPONSE_LINK_CHUNKS_V1 + chunk,
                    index: coordinate,
                },
            )
            .constant(-(powers[global] * scalar_from_signed_v1(secret_response[global])));
    }
    Ok(constraint)
}
fn arithmetic_statement_v1(
    response: &PersistentDecryptionResponseLinkResponseFixedUseV1<'_>,
    beta: Scalar,
) -> Result<ArithmeticCircuitStatement<'static, ZkAmsT256BulletproofSuiteV1>, ResponseLinkErrorV1> {
    response.validate_v1()?;
    let view = response.commitments_v1();
    let commitments = (0..RESPONSE_LINK_COMMITMENTS_V1)
        .map(|index| view.point_v1(index))
        .collect::<Result<Vec<_>, _>>()?;
    let constraint = response_link_constraint_v1(
        beta,
        response.challenge_stage.challenge,
        response.secret_response,
    )?;
    Ok(ArithmeticCircuitStatement::new(
        ZkAmsT256BulletproofSuiteV1::generators().reduce(RESPONSE_LINK_CHUNK_COEFFICIENTS_V1)?,
        vec![constraint],
        commitments,
        Vec::new(),
    )?)
}
fn frame_header_v1(label: &[u8], payload_bytes: usize) -> Result<Vec<u8>, ResponseLinkErrorV1> {
    let mut header = Vec::with_capacity(1 + 2 + label.len() + 8);
    header.push(0x52);
    header.extend_from_slice(
        &u16::try_from(label.len())
            .map_err(|_| ResponseLinkErrorV1::Shape)?
            .to_be_bytes(),
    );
    header.extend_from_slice(label);
    header.extend_from_slice(
        &u64::try_from(payload_bytes)
            .map_err(|_| ResponseLinkErrorV1::Shape)?
            .to_be_bytes(),
    );
    Ok(header)
}
fn absorb_frame_v1(
    state: &mut Keccak256,
    label: &[u8],
    payload: &[u8],
) -> Result<(), ResponseLinkErrorV1> {
    state.update(&frame_header_v1(label, payload.len())?);
    state.update(payload);
    Ok(())
}
fn absorb_points_v1(
    state: &mut Keccak256,
    label: &[u8],
    points: &[Point],
) -> Result<(), ResponseLinkErrorV1> {
    state.update(&frame_header_v1(label, points.len() * 33)?);
    for point in points {
        state.update(
            &point
                .to_non_identity_wire_bytes()
                .map_err(|_| ResponseLinkErrorV1::PointEncoding)?,
        );
    }
    Ok(())
}
fn derive_nonzero_challenge_v1(
    state: &mut Keccak256,
    ordinal: &mut u32,
    label: &[u8],
) -> Result<Scalar, ResponseLinkErrorV1> {
    for attempt in 0_u8..=127 {
        let mut wide = [0_u8; 64];
        for branch in 0_u8..=1 {
            let mut fork = state.fork_v1();
            fork.update(CHALLENGE_DOMAIN_V1);
            fork.update(&ordinal.to_be_bytes());
            fork.update(&[attempt, branch]);
            absorb_frame_v1(&mut fork, b"challenge-purpose", label)?;
            let digest = fork.finalize();
            let start = usize::from(branch) * 32;
            wide[start..start + 32].copy_from_slice(&digest);
        }
        let challenge = Scalar::from_uniform_le_bytes(wide);
        wide.fill(0);
        if !challenge.is_zero() {
            absorb_frame_v1(state, b"challenge-purpose", label)?;
            absorb_frame_v1(state, b"challenge-ordinal", &ordinal.to_be_bytes())?;
            absorb_frame_v1(state, b"challenge-attempt", &[attempt])?;
            absorb_frame_v1(state, b"challenge-scalar", &challenge.to_le_bytes())?;
            *ordinal = ordinal.checked_add(1).ok_or(ResponseLinkErrorV1::Shape)?;
            return Ok(challenge);
        }
    }
    Err(ResponseLinkErrorV1::Backend(
        GeneralizedBulletproofErrorV1::TranscriptChallengeExhausted,
    ))
}
struct ResponseLinkTranscriptSeedV1 {
    state: Keccak256,
    challenge_ordinal: u32,
    beta: Scalar,
}
impl ResponseLinkTranscriptSeedV1 {
    fn new_v1(
        response: &PersistentDecryptionResponseLinkResponseFixedUseV1<'_>,
    ) -> Result<Self, ResponseLinkErrorV1> {
        response.validate_v1()?;
        let source = response.challenge_stage.mask_stage.source;
        let axes = source.axes;
        let mut state = Keccak256::new();
        state.update(TRANSCRIPT_DOMAIN_V1);
        state.update(&[RESPONSE_LINK_VERSION_V1]);
        absorb_frame_v1(&mut state, b"equation", EQUATION_V1)?;
        absorb_frame_v1(
            &mut state,
            b"generator-basis",
            &ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1,
        )?;
        for (label, digest) in [
            (b"profile".as_slice(), axes.profile_digest),
            (b"roster".as_slice(), axes.roster_digest),
            (b"key-context".as_slice(), axes.key_context_digest),
            (b"cpk-transcript".as_slice(), axes.cpk_transcript_digest),
            (
                b"decryption-statement".as_slice(),
                axes.decryption_statement_digest,
            ),
        ] {
            absorb_frame_v1(&mut state, label, &digest)?;
        }
        absorb_frame_v1(&mut state, b"epoch", &axes.epoch.to_be_bytes())?;
        absorb_frame_v1(&mut state, b"party-index", &[axes.party_index])?;
        absorb_points_v1(
            &mut state,
            b"cpk-secret-commitments",
            &source.cpk_secret_commitments,
        )?;
        absorb_points_v1(
            &mut state,
            b"mask-commitments",
            &response.challenge_stage.mask_stage.mask_commitments,
        )?;
        absorb_frame_v1(
            &mut state,
            b"public-key-first-message",
            &axes.public_key_first_message_digest,
        )?;
        absorb_frame_v1(
            &mut state,
            b"share-first-message",
            &axes.share_first_message_digest,
        )?;
        let challenge = response.challenge_stage.challenge;
        state.update(&frame_header_v1(
            b"sparse-challenge",
            32 + 4 + RESPONSE_LINK_CHALLENGE_WEIGHT_V1 * 5,
        )?);
        state.update(&challenge.seed);
        state.update(&(RESPONSE_LINK_CHALLENGE_WEIGHT_V1 as u32).to_be_bytes());
        for term in challenge.terms {
            state.update(&term.shift.to_be_bytes());
            state.update(&term.sign.to_be_bytes());
        }
        state.update(&frame_header_v1(
            b"secret-response",
            RESPONSE_LINK_RING_DEGREE_V1 * 8,
        )?);
        for coefficient in response.secret_response {
            state.update(&coefficient.to_be_bytes());
        }
        let mut challenge_ordinal = 0;
        let beta = derive_nonzero_challenge_v1(
            &mut state,
            &mut challenge_ordinal,
            b"response-projection-beta",
        )?;
        Ok(Self {
            state,
            challenge_ordinal,
            beta,
        })
    }
    fn binding_digest_v1(&self) -> [u8; 32] {
        self.state.fork_v1().finalize()
    }
}
struct ResponseLinkVerifierTranscriptV1<'a> {
    state: Keccak256,
    proof: &'a [u8],
    cursor: usize,
    challenge_ordinal: u32,
}
impl<'a> ResponseLinkVerifierTranscriptV1<'a> {
    fn from_seed_v1(seed: &ResponseLinkTranscriptSeedV1, proof: &'a [u8]) -> Self {
        Self {
            state: seed.state.fork_v1(),
            proof,
            cursor: 0,
            challenge_ordinal: seed.challenge_ordinal,
        }
    }
    fn take_v1(&mut self, count: usize) -> Result<&'a [u8], GeneralizedBulletproofErrorV1> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?;
        let bytes =
            self.proof
                .get(self.cursor..end)
                .ok_or(GeneralizedBulletproofErrorV1::ProofLength {
                    actual: self.proof.len(),
                    expected: end,
                })?;
        self.cursor = end;
        Ok(bytes)
    }
    fn finish_v1(self) -> Result<[u8; 32], ResponseLinkErrorV1> {
        if self.cursor != self.proof.len() || self.cursor != RESPONSE_LINK_CORE_BYTES_V1 {
            return Err(ResponseLinkErrorV1::ProofEncoding);
        }
        Ok(self.state.finalize())
    }
}
impl VerifierTranscript<ZkAmsT256BulletproofSuiteV1> for ResponseLinkVerifierTranscriptV1<'_> {
    fn read_scalar(&mut self) -> Result<Scalar, GeneralizedBulletproofErrorV1> {
        let bytes: [u8; 32] = self
            .take_v1(32)?
            .try_into()
            .map_err(|_| GeneralizedBulletproofErrorV1::ScalarEncoding)?;
        let scalar = Scalar::from_le_bytes_exact(bytes)
            .map_err(|_| GeneralizedBulletproofErrorV1::ScalarEncoding)?;
        absorb_frame_v1(&mut self.state, b"bp-scalar", &bytes)
            .map_err(|_| GeneralizedBulletproofErrorV1::ArithmeticInvariant)?;
        Ok(scalar)
    }
    fn read_point(&mut self) -> Result<Point, GeneralizedBulletproofErrorV1> {
        let bytes: [u8; 33] = self
            .take_v1(33)?
            .try_into()
            .map_err(|_| GeneralizedBulletproofErrorV1::PointEncoding)?;
        let point = Point::from_non_identity_wire_bytes_exact(&bytes)
            .map_err(|_| GeneralizedBulletproofErrorV1::PointEncoding)?;
        absorb_frame_v1(&mut self.state, b"bp-point", &bytes)
            .map_err(|_| GeneralizedBulletproofErrorV1::ArithmeticInvariant)?;
        Ok(point)
    }
    fn challenge(&mut self) -> Result<Scalar, GeneralizedBulletproofErrorV1> {
        derive_nonzero_challenge_v1(
            &mut self.state,
            &mut self.challenge_ordinal,
            b"generalized-bulletproof",
        )
        .map_err(|_| GeneralizedBulletproofErrorV1::TranscriptChallengeExhausted)
    }
}
struct PersistentDecryptionResponseLinkProofV1 {
    wire: [u8; RESPONSE_LINK_TAIL_BYTES_V1],
    mask_commitments: [Point; RESPONSE_LINK_CHUNKS_V1],
}
fn validate_core_v1(core: &[u8]) -> Result<(), ResponseLinkErrorV1> {
    if core.len() != RESPONSE_LINK_CORE_BYTES_V1 {
        return Err(ResponseLinkErrorV1::ProofEncoding);
    }
    let mut cursor = 0;
    {
        let mut take_point = || -> Result<(), ResponseLinkErrorV1> {
            let end = cursor + 33;
            Point::from_non_identity_wire_bytes_exact(&core[cursor..end])
                .map_err(|_| ResponseLinkErrorV1::PointEncoding)?;
            cursor = end;
            Ok(())
        };
        for _ in 0..41 {
            take_point()?;
        }
    }
    for _ in 0..3 {
        let end = cursor + 32;
        let bytes: [u8; 32] = core[cursor..end]
            .try_into()
            .map_err(|_| ResponseLinkErrorV1::ScalarEncoding)?;
        Scalar::from_le_bytes_exact(bytes).map_err(|_| ResponseLinkErrorV1::ScalarEncoding)?;
        cursor = end;
    }
    for _ in 0..28 {
        let end = cursor + 33;
        Point::from_non_identity_wire_bytes_exact(&core[cursor..end])
            .map_err(|_| ResponseLinkErrorV1::PointEncoding)?;
        cursor = end;
    }
    for _ in 0..2 {
        let end = cursor + 32;
        let bytes: [u8; 32] = core[cursor..end]
            .try_into()
            .map_err(|_| ResponseLinkErrorV1::ScalarEncoding)?;
        Scalar::from_le_bytes_exact(bytes).map_err(|_| ResponseLinkErrorV1::ScalarEncoding)?;
        cursor = end;
    }
    if cursor != core.len() {
        return Err(ResponseLinkErrorV1::ProofEncoding);
    }
    Ok(())
}
impl PersistentDecryptionResponseLinkProofV1 {
    fn from_wire_bytes_exact_v1(bytes: &[u8]) -> Result<Self, ResponseLinkErrorV1> {
        if bytes.len() != RESPONSE_LINK_TAIL_BYTES_V1
            || bytes[..4] != RESPONSE_LINK_WIRE_TAG_V1
            || bytes[4] != RESPONSE_LINK_VERSION_V1
            || bytes[5] != RESPONSE_LINK_WIRE_FLAGS_V1
            || usize::from(bytes[6]) != RESPONSE_LINK_CHUNKS_V1
        {
            return Err(ResponseLinkErrorV1::ProofEncoding);
        }
        let mut cursor = RESPONSE_LINK_HEADER_BYTES_V1;
        let mut mask_commitments =
            [Point::from_non_identity_wire_bytes_exact(&bytes[cursor..cursor + 33])
                .map_err(|_| ResponseLinkErrorV1::PointEncoding)?;
                RESPONSE_LINK_CHUNKS_V1];
        for point in &mut mask_commitments {
            *point = Point::from_non_identity_wire_bytes_exact(&bytes[cursor..cursor + 33])
                .map_err(|_| ResponseLinkErrorV1::PointEncoding)?;
            cursor += 33;
        }
        validate_core_v1(&bytes[cursor..])?;
        let wire: [u8; RESPONSE_LINK_TAIL_BYTES_V1] = bytes
            .try_into()
            .map_err(|_| ResponseLinkErrorV1::ProofEncoding)?;
        Ok(Self {
            wire,
            mask_commitments,
        })
    }
    fn core_v1(&self) -> &[u8] {
        &self.wire[RESPONSE_LINK_HEADER_BYTES_V1 + RESPONSE_LINK_MASK_COMMITMENT_BYTES_V1..]
    }
    fn wire_v1(&self) -> &[u8; RESPONSE_LINK_TAIL_BYTES_V1] {
        &self.wire
    }
}
struct VerifiedPersistentDecryptionResponseLinkReceiptV1 {
    statement_binding_digest: [u8; 32],
    proof_digest: [u8; 32],
    transcript_digest: [u8; 32],
    integration_seal: ResponseLinkIntegrationSealV1,
}
fn verify_response_link_v1(
    response: PersistentDecryptionResponseLinkResponseFixedUseV1<'_>,
    proof: PersistentDecryptionResponseLinkProofV1,
    integration_seal: ResponseLinkIntegrationSealV1,
) -> Result<VerifiedPersistentDecryptionResponseLinkReceiptV1, ResponseLinkErrorV1> {
    response.validate_v1()?;
    for index in 0..RESPONSE_LINK_CHUNKS_V1 {
        if proof.mask_commitments[index]
            != response
                .commitments_v1()
                .point_v1(RESPONSE_LINK_CHUNKS_V1 + index)?
        {
            return Err(ResponseLinkErrorV1::StatementMismatch);
        }
    }
    let seed = ResponseLinkTranscriptSeedV1::new_v1(&response)?;
    let mut transcript = ResponseLinkVerifierTranscriptV1::from_seed_v1(&seed, proof.core_v1());
    arithmetic_statement_v1(&response, seed.beta)?.verify(&mut transcript)?;
    Ok(VerifiedPersistentDecryptionResponseLinkReceiptV1 {
        statement_binding_digest: seed.binding_digest_v1(),
        proof_digest: keccak256(proof.wire_v1()),
        transcript_digest: transcript.finish_v1()?,
        integration_seal,
    })
}
// Ordered audit records: proof/core/total/margins/batch/heaps/work estimates, followed by
// PIT numerator/bits, exact-lift bound, and three forbidden assumptions.
const RESPONSE_LINK_RESOURCE_RECORD_V1: [u64; 10] = [
    RESPONSE_LINK_CORE_BYTES_V1 as u64,
    RESPONSE_LINK_TAIL_BYTES_V1 as u64,
    CONDITIONAL_PROOF_BYTES_V1 as u64,
    PROOF_WIRE_MARGIN_BYTES_V1 as u64,
    OBJECT_MARGIN_BYTES_V1 as u64,
    EIGHT_PARTY_TAIL_BYTES_V1 as u64,
    PROVER_PEAK_HEAP_BOUND_BYTES_V1 as u64,
    VERIFIER_PEAK_HEAP_BOUND_BYTES_V1 as u64,
    PROVER_MAIN_SCALAR_WORK_ESTIMATE_V1,
    PROVER_GROUP_TERM_ESTIMATE_V1,
];
const RESPONSE_LINK_SOUNDNESS_RECORD_V1: (u64, u16, u64, bool, bool, bool) = (
    PIT_NUMERATOR_V1,
    PIT_SOUNDNESS_BITS_FLOOR_V1,
    RESPONSE_LINK_INTEGER_LIFT_BOUND_V1,
    false, // scalar commitments
    false, // sparse-challenge inversion
    false, // SIS uniqueness
);
#[cfg(test)]
#[path = "persistent_decryption_response_link_tests.rs"]
mod tests;
